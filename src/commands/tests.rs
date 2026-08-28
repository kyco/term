#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::{dispatch_command, handle_legacy_patterns};
    use crate::args::{
        Args, AskArgs, AuthAction, ChatArgs, Commands, CompletionAction, ConfigAction, ConfigArgs,
        Provider, RedactAction, SessionAction, SetupArgs,
    };
    use crate::config::entity::config_entity::ConfigEntity;
    use crate::config::repository::ConfigRepository;
    use crate::repository::db::SqliteRepository;
    use crate::session::entity::session_entity::SessionEntity;
    use crate::session::repository::{MessageRepository, SessionRepository};
    use anyhow::Result;
    use chrono::NaiveDateTime;
    use clap::Parser;
    use rusqlite::Connection;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    // Mock repository for testing command dispatcher
    #[derive(Debug, Clone)]
    pub struct MockRepository {
        configs: Arc<Mutex<HashMap<String, ConfigEntity>>>,
        sessions: Arc<Mutex<HashMap<String, SessionEntity>>>,
        messages: Arc<Mutex<Vec<crate::session::entity::message_entity::MessageEntity>>>,
        next_id: Arc<Mutex<i64>>,
    }

    #[allow(dead_code)]
    impl MockRepository {
        pub fn new() -> Self {
            Self {
                configs: Arc::new(Mutex::new(HashMap::new())),
                sessions: Arc::new(Mutex::new(HashMap::new())),
                messages: Arc::new(Mutex::new(Vec::new())),
                next_id: Arc::new(Mutex::new(1)),
            }
        }

        pub fn with_config(self, key: &str, value: &str) -> Self {
            {
                let mut configs = self.configs.lock().unwrap();
                let mut next_id = self.next_id.lock().unwrap();
                let entity = ConfigEntity::new_with_id(*next_id, key, value);
                configs.insert(key.to_string(), entity);
                *next_id += 1;
            }
            self
        }
    }

    impl ConfigRepository for MockRepository {
        type Error = anyhow::Error;

        fn fetch_all_configs(&self) -> Result<Vec<ConfigEntity>, Self::Error> {
            let configs = self.configs.lock().unwrap();
            Ok(configs.values().cloned().collect())
        }

        fn fetch_by_key(&self, key: &str) -> Result<ConfigEntity, Self::Error> {
            let configs = self.configs.lock().unwrap();
            configs
                .get(key)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Key not found: {}", key))
        }

        fn add_config(&self, key: &str, value: &str) -> Result<(), Self::Error> {
            let mut configs = self.configs.lock().unwrap();
            let mut next_id = self.next_id.lock().unwrap();
            let entity = ConfigEntity::new_with_id(*next_id, key, value);
            configs.insert(key.to_string(), entity);
            *next_id += 1;
            Ok(())
        }

        fn update_config(&self, id: i64, key: &str, value: &str) -> Result<(), Self::Error> {
            let mut configs = self.configs.lock().unwrap();
            let entity = ConfigEntity::new_with_id(id, key, value);
            configs.insert(key.to_string(), entity);
            Ok(())
        }
    }

    impl SessionRepository for MockRepository {
        type Error = anyhow::Error;

        fn fetch_all_sessions(&self) -> Result<Vec<SessionEntity>, Self::Error> {
            let sessions = self.sessions.lock().unwrap();
            Ok(sessions.values().cloned().collect())
        }

        fn fetch_session_by_name(&self, name: &str) -> Result<SessionEntity, Self::Error> {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .values()
                .find(|s| s.name == name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Session not found: {}", name))
        }

        fn add_session(
            &self,
            id: &str,
            name: &str,
            last_used_at: NaiveDateTime,
            current: bool,
        ) -> Result<(), Self::Error> {
            self.upsert_session(id, name, last_used_at, current)
        }

        fn upsert_session(
            &self,
            id: &str,
            name: &str,
            last_used_at: NaiveDateTime,
            current: bool,
        ) -> Result<(), Self::Error> {
            let mut sessions = self.sessions.lock().unwrap();
            // Keyed by id so a rename does not leave the old name behind.
            sessions.retain(|_, session| session.id != id);
            let entity = SessionEntity::new(
                id.to_string(),
                name.to_string(),
                last_used_at + chrono::Duration::hours(24),
                if current { 1 } else { 0 },
                last_used_at,
            );
            sessions.insert(name.to_string(), entity);
            Ok(())
        }

        fn remove_current_from_all(&self) -> Result<(), Self::Error> {
            let mut sessions = self.sessions.lock().unwrap();
            for session in sessions.values_mut() {
                session.current = 0;
            }
            Ok(())
        }

        fn delete_session(&self, session_id: &str) -> Result<(), Self::Error> {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.retain(|_, session| session.id != session_id);
            Ok(())
        }
    }

    impl MessageRepository for MockRepository {
        type Error = anyhow::Error;

        fn fetch_messages_for_session(
            &self,
            session_id: &str,
        ) -> Result<Vec<crate::session::entity::message_entity::MessageEntity>, Self::Error>
        {
            let messages = self.messages.lock().unwrap();
            Ok(messages
                .iter()
                .filter(|m| m.session_id == session_id && m.message_type != "pending")
                .cloned()
                .collect())
        }

        fn add_message_to_session(
            &self,
            message: &crate::session::entity::message_entity::MessageEntity,
        ) -> Result<(), Self::Error> {
            let mut messages = self.messages.lock().unwrap();
            messages.push(message.clone());
            Ok(())
        }

        fn count_messages_for_session(&self, session_id: &str) -> Result<i64, Self::Error> {
            let messages = self.messages.lock().unwrap();
            Ok(messages
                .iter()
                .filter(|m| m.session_id == session_id && m.message_type != "pending")
                .count() as i64)
        }

        fn fetch_first_user_message(
            &self,
            session_id: &str,
        ) -> Result<Option<String>, Self::Error> {
            let messages = self.messages.lock().unwrap();
            Ok(messages
                .iter()
                .find(|m| {
                    m.session_id == session_id && m.role == "user" && m.message_type != "pending"
                })
                .map(|m| m.content.clone()))
        }

        fn promote_message(&self, message_id: &str) -> Result<(), Self::Error> {
            let mut messages = self.messages.lock().unwrap();
            for message in messages.iter_mut() {
                if message.id == message_id && message.message_type == "pending" {
                    message.message_type = "standard".to_string();
                }
            }
            Ok(())
        }

        fn fetch_pending_messages(&self, session_id: &str) -> Result<Vec<String>, Self::Error> {
            let messages = self.messages.lock().unwrap();
            Ok(messages
                .iter()
                .filter(|m| m.session_id == session_id && m.message_type == "pending")
                .map(|m| m.content.clone())
                .collect())
        }

        fn discard_pending_messages(&self, session_id: &str) -> Result<(), Self::Error> {
            let mut messages = self.messages.lock().unwrap();
            messages.retain(|m| !(m.session_id == session_id && m.message_type == "pending"));
            Ok(())
        }

        fn fetch_conversation_rows(
            &self,
            session_id: &str,
        ) -> Result<Vec<crate::session::repository::message_repository::StoredRow>, Self::Error>
        {
            let messages = self.messages.lock().unwrap();
            Ok(messages
                .iter()
                .filter(|m| m.session_id == session_id && m.message_type != "pending")
                .map(|m| crate::session::repository::message_repository::StoredRow {
                    id: m.id.clone(),
                    role: m.role.clone(),
                    content: m.content.clone(),
                    message_type: m.message_type.clone(),
                })
                .collect())
        }

        fn mark_duplicates(&self, ids: &[String]) -> Result<(), Self::Error> {
            let mut messages = self.messages.lock().unwrap();
            for message in messages.iter_mut() {
                if ids.contains(&message.id) {
                    message.message_type = "duplicate".to_string();
                }
            }
            Ok(())
        }

        fn restore_duplicates(&self) -> Result<usize, Self::Error> {
            let mut messages = self.messages.lock().unwrap();
            let mut restored = 0;
            for message in messages.iter_mut() {
                if message.message_type == "duplicate" {
                    message.message_type = "standard".to_string();
                    restored += 1;
                }
            }
            Ok(restored)
        }

        fn search_messages(
            &self,
            query: &str,
            limit: usize,
        ) -> Result<
            Vec<crate::session::repository::message_repository::MessageMatch>,
            Self::Error,
        > {
            let sessions = self.sessions.lock().unwrap();
            let messages = self.messages.lock().unwrap();
            Ok(messages
                .iter()
                .filter(|m| m.content.contains(query) && m.message_type != "pending")
                .take(limit)
                .map(|m| {
                    let session_name = sessions
                        .values()
                        .find(|s| s.id == m.session_id)
                        .map(|s| s.name.clone())
                        .unwrap_or_default();
                    crate::session::repository::message_repository::MessageMatch {
                        session_name,
                        role: m.role.clone(),
                        content: m.content.clone(),
                    }
                })
                .collect())
        }
    }

    fn create_temp_sqlite_repo() -> Result<SqliteRepository> {
        let conn = Connection::open_in_memory()?;

        // Initialize the database schema
        conn.execute(
            "CREATE TABLE config (
                id INTEGER PRIMARY KEY,
                key TEXT UNIQUE NOT NULL,
                value TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                expires_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create a temporary file for the repository since SqliteRepository expects a path
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Close the in-memory connection and create a file-based one
        drop(conn);

        // Create the file-based repository
        let repo = SqliteRepository::new(db_path.to_str().unwrap())?;
        Ok(repo)
    }

    #[test]
    fn test_args_struct_creation() {
        // Test SetupArgs
        let setup_args = SetupArgs {
            skip_validation: true,
            force: false,
            auto_accept: true,
        };
        assert!(setup_args.skip_validation);
        assert!(!setup_args.force);
        assert!(setup_args.auto_accept);

        // Test ChatArgs
        let chat_args = ChatArgs {
            temporary: false,
            input: Some("Hello".to_string()),
            directory: Some("src/".to_string()),
            directories: vec!["tests/".to_string()],
            exclude: vec!["*.log".to_string()],
            system_prompt: None,
            session: Some("test_session".to_string()),
            last_session: false,
            smart_context: true,
            context_query: Some("rust code".to_string()),
            max_context_tokens: Some(4000),
            preview_context: false,
            chunked_analysis: false,
            chunk_strategy: "hierarchical".to_string(),
        };
        assert_eq!(chat_args.input, Some("Hello".to_string()));
        assert_eq!(chat_args.directories, vec!["tests/"]);
        assert!(chat_args.smart_context);

        // Test AskArgs
        let ask_args = AskArgs {
            question: "What is this code doing?".to_string(),
            directory: Some("src/".to_string()),
            directories: vec![],
            exclude: vec![],
            system_prompt: None,
            session: None,
            smart_context: false,
            context_query: None,
            max_context_tokens: None,
            preview_context: false,
            chunked_analysis: false,
            chunk_strategy: "hierarchical".to_string(),
        };
        assert_eq!(ask_args.question, "What is this code doing?");
        assert!(!ask_args.smart_context);
    }

    #[test]
    fn test_commands_enum_structure() {
        // Test Commands enum variants with dedicated args structs
        let setup_command = Commands::Setup(SetupArgs {
            skip_validation: false,
            force: true,
            auto_accept: false,
        });

        match setup_command {
            Commands::Setup(args) => {
                assert!(args.force);
                assert!(!args.skip_validation);
            }
            _ => panic!("Expected Setup command"),
        }

        let config_command = Commands::Config {
            args: ConfigArgs {
                export: None,
                import: None,
                backup: false,
                validate: true,
            },
            action: ConfigAction::Show,
        };

        match config_command {
            Commands::Config { args, action } => {
                assert!(args.validate);
                assert!(!args.backup);
                match action {
                    ConfigAction::Show => (),
                    _ => panic!("Expected Show action"),
                }
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[tokio::test]
    async fn test_command_dispatch_setup() -> Result<()> {
        let _repo = create_temp_sqlite_repo()?;
        let args = Args {
            command: Some(Commands::Setup(SetupArgs {
                skip_validation: true,
                force: false,
                auto_accept: true,
            })),
            ..Default::default()
        };

        // Note: This test would normally require interactive input
        // For now, we just verify the dispatch routing works
        // In a real test environment, we'd mock the SetupWizard
        // let result = dispatch_command(&args, &repo).await;
        // For now, just test that args are parsed correctly
        match &args.command {
            Some(Commands::Setup(setup_args)) => {
                assert!(setup_args.skip_validation);
                assert!(setup_args.auto_accept);
                assert!(!setup_args.force);
            }
            _ => panic!("Expected Setup command"),
        }

        Ok(())
    }

    #[test]
    fn test_command_dispatch_ask_args() -> Result<()> {
        let args = Args {
            command: Some(Commands::Ask(AskArgs {
                question: "Test question".to_string(),
                directory: None,
                directories: vec![],
                exclude: vec![],
                system_prompt: None,
                session: None,
                smart_context: false,
                context_query: None,
                max_context_tokens: None,
                preview_context: false,
                chunked_analysis: false,
                chunk_strategy: "hierarchical".to_string(),
            })),
            ..Default::default()
        };

        // Test that args are correctly structured
        match &args.command {
            Some(Commands::Ask(ask_args)) => {
                assert_eq!(ask_args.question, "Test question");
                assert!(!ask_args.smart_context);
                assert_eq!(ask_args.chunk_strategy, "hierarchical");
            }
            _ => panic!("Expected Ask command"),
        }

        Ok(())
    }

    #[test]
    fn test_config_action_enum() {
        let show_action = ConfigAction::Show;
        match show_action {
            ConfigAction::Show => (),
            _ => panic!("Expected Show action"),
        }

        let set_claude_action = ConfigAction::SetClaude {
            api_key: "test-key".to_string(),
        };
        match set_claude_action {
            ConfigAction::SetClaude { api_key } => {
                assert_eq!(api_key, "test-key");
            }
            _ => panic!("Expected SetClaude action"),
        }

        let set_provider_action = ConfigAction::SetProvider {
            provider: Provider::Claude,
        };
        match set_provider_action {
            ConfigAction::SetProvider { provider } => {
                assert_eq!(provider, Provider::Claude);
            }
            _ => panic!("Expected SetProvider action"),
        }
    }

    #[test]
    fn test_auth_command_parsing_and_compatibility_shims() {
        let auth_args = Args::try_parse_from(["termai", "auth", "login", "codex"]).unwrap();
        match auth_args.command {
            Some(Commands::Auth {
                action: AuthAction::Login(args),
            }) => {
                assert_eq!(args.provider, Provider::OpenaiCodex);
            }
            _ => panic!("Expected auth login codex command"),
        }

        let shim_args = Args::try_parse_from(["termai", "config", "login-codex"]).unwrap();
        match shim_args.command {
            Some(Commands::Config { action, .. }) => {
                assert!(matches!(action, ConfigAction::LoginCodex));
            }
            _ => panic!("Expected config login-codex compatibility shim"),
        }
    }

    #[test]
    fn test_config_migrate_command_parsing() {
        let args = Args::try_parse_from(["termai", "config", "migrate"]).unwrap();
        match args.command {
            Some(Commands::Config { action, .. }) => {
                assert!(matches!(action, ConfigAction::Migrate));
            }
            _ => panic!("Expected config migrate command"),
        }
    }

    #[test]
    fn test_session_action_enum() {
        let list_action = SessionAction::List {
            filter: None,
            limit: None,
            sort: None,
        };
        match list_action {
            SessionAction::List { .. } => (),
            _ => panic!("Expected List action"),
        }

        let delete_action = SessionAction::Delete {
            name: "test_session".to_string(),
        };
        match delete_action {
            SessionAction::Delete { name } => {
                assert_eq!(name, "test_session");
            }
            _ => panic!("Expected Delete action"),
        }

        let show_action = SessionAction::Show {
            name: "my_session".to_string(),
        };
        match show_action {
            SessionAction::Show { name } => {
                assert_eq!(name, "my_session");
            }
            _ => panic!("Expected Show action"),
        }
    }

    #[test]
    fn test_completion_action_enum() {
        let bash_action = CompletionAction::Bash;
        match bash_action {
            CompletionAction::Bash => (),
            _ => panic!("Expected Bash action"),
        }

        let zsh_action = CompletionAction::Zsh;
        match zsh_action {
            CompletionAction::Zsh => (),
            _ => panic!("Expected Zsh action"),
        }

        let fish_action = CompletionAction::Fish;
        match fish_action {
            CompletionAction::Fish => (),
            _ => panic!("Expected Fish action"),
        }

        let powershell_action = CompletionAction::PowerShell;
        match powershell_action {
            CompletionAction::PowerShell => (),
            _ => panic!("Expected PowerShell action"),
        }
    }

    #[test]
    fn test_redact_action_enum() {
        let add_action = RedactAction::Add {
            pattern: "test@example.com".to_string(),
        };
        match add_action {
            RedactAction::Add { pattern } => {
                assert_eq!(pattern, "test@example.com");
            }
            _ => panic!("Expected Add action"),
        }

        let remove_action = RedactAction::Remove {
            pattern: "secret_key".to_string(),
        };
        match remove_action {
            RedactAction::Remove { pattern } => {
                assert_eq!(pattern, "secret_key");
            }
            _ => panic!("Expected Remove action"),
        }

        let list_action = RedactAction::List;
        match list_action {
            RedactAction::List => (),
            _ => panic!("Expected List action"),
        }
    }

    #[test]
    fn test_provider_enum() {
        assert_eq!(Provider::Claude.to_str(), "claude");
        assert_eq!(Provider::Openai.to_str(), "openai");
        assert_eq!(Provider::OpenaiCodex.to_str(), "codex");

        assert_eq!(Provider::new("claude"), Provider::Claude);
        assert_eq!(Provider::new("openai"), Provider::Openai);
        assert_eq!(Provider::new("codex"), Provider::OpenaiCodex);
        assert_eq!(Provider::new("openai-codex"), Provider::OpenaiCodex);
        assert_eq!(Provider::new("unknown"), Provider::Claude); // Default fallback
    }

    #[test]
    fn test_session_sort_order_enum() {
        use crate::args::SessionSortOrder;

        // Test that enum variants exist and can be matched
        let name_sort = SessionSortOrder::Name;
        let date_sort = SessionSortOrder::Date;
        let messages_sort = SessionSortOrder::Messages;

        match name_sort {
            SessionSortOrder::Name => (),
            _ => panic!("Expected Name sort order"),
        }

        match date_sort {
            SessionSortOrder::Date => (),
            _ => panic!("Expected Date sort order"),
        }

        match messages_sort {
            SessionSortOrder::Messages => (),
            _ => panic!("Expected Messages sort order"),
        }
    }

    // Integration test for error handling in command dispatch
    #[tokio::test]
    async fn test_command_dispatch_error_handling() -> Result<()> {
        let repo = create_temp_sqlite_repo()?;
        let args = Args {
            command: None, // No command provided
            ..Default::default()
        };

        let result = dispatch_command(&args, &repo).await?;
        assert!(!result); // Should return false when no command is provided

        Ok(())
    }

    // Test legacy pattern handling
    #[tokio::test]
    async fn test_legacy_patterns() -> Result<()> {
        let repo = create_temp_sqlite_repo()?;
        let args = Args {
            command: None,
            chat_gpt_api_key: Some("legacy-key".to_string()),
            ..Default::default()
        };

        let result = handle_legacy_patterns(&args, &repo)?;
        assert!(result); // Should handle legacy pattern

        // Verify the key was stored (this would normally require checking the database)
        // For this test, we just verify the logic path was taken

        Ok(())
    }
}
