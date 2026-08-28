use crate::common;
use crate::config::repository::ConfigRepository;
use crate::config::service::redacted_config;
use crate::llm::common::model::role::Role;
use crate::redactions::common::redaction_map;
use crate::redactions::redact::redact;
use crate::redactions::revert::unredact;
use crate::session::entity::session_entity::SessionEntity;
use crate::session::model::message::Message;
use crate::session::model::smart_context::SessionSmartContext;
use chrono::{Duration, NaiveDateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub expires_at: NaiveDateTime,
    pub current: bool,
    pub messages: Vec<Message>,
    /// Opt-in "never touch the database" mode (`chat --temporary`, and the
    /// throwaway sessions internal callers build to reuse the chat plumbing).
    /// It is not the default for interactive chat: a session the user cannot
    /// find afterwards is indistinguishable from one that was lost.
    pub temporary: bool,
    pub redaction_mapping: Option<HashMap<String, String>>,
    pub smart_context: Option<SessionSmartContext>,
    pub last_used_at: NaiveDateTime,
}

impl From<&SessionEntity> for Session {
    fn from(value: &SessionEntity) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            expires_at: value.expires_at,
            current: value.current == 1,
            messages: Vec::new(),
            temporary: false,
            redaction_mapping: None,
            smart_context: None,
            last_used_at: value.last_used_at,
        }
    }
}

impl Session {
    /// A session that is never written to disk. Used by one-shot internal
    /// callers (commit message generation, compaction, `ask` without
    /// `--session`) and by `chat --temporary`.
    pub fn new_temporary() -> Self {
        let now = Utc::now().naive_utc();
        Self {
            id: common::unique_id::generate_uuid_v4().to_string(),
            name: "temporary".to_string(),
            expires_at: now + Duration::hours(24),
            current: true,
            messages: Vec::new(),
            temporary: true,
            redaction_mapping: None,
            smart_context: None,
            last_used_at: now,
        }
    }

    pub fn copy_with_messages(&self, messages: Vec<Message>) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            expires_at: self.expires_at,
            current: self.current,
            messages: messages.to_vec(),
            temporary: self.temporary,
            redaction_mapping: self.redaction_mapping.clone(),
            smart_context: self.smart_context.clone(),
            last_used_at: self.last_used_at,
        }
    }

    pub fn add_raw_message(&mut self, message: String, role: Role) {
        self.messages
            .push(Message::new("".to_string(), role, message));
    }

    pub fn redact<R: ConfigRepository>(&mut self, repo: &R) {
        let mut redacted_messages = Vec::with_capacity(self.messages.len());
        let redactions = redacted_config::fetch_redactions(repo);
        let mapped_redactions = redaction_map(redactions);
        for message in self.messages.iter() {
            let redacted_input = redact(&message.content, &mapped_redactions);
            redacted_messages.push(Message {
                id: message.id.to_string(),
                role: message.role.clone(),
                content: redacted_input,
                message_type: message.message_type.clone(),
            });
        }

        self.redaction_mapping = Some(mapped_redactions);
        self.messages = redacted_messages;
    }

    pub fn unredact(&mut self) {
        let mut unredacted = Vec::with_capacity(self.messages.len());
        for message in self.messages.iter() {
            match &self.redaction_mapping {
                Some(redaction_mapping) => {
                    let content = unredact(redaction_mapping, &message.content);
                    unredacted.push(Message {
                        id: message.id.to_string(),
                        role: message.role.clone(),
                        content,
                        message_type: message.message_type.clone(),
                    });
                }
                None => {
                    println!("no redaction mapping: {:#?}", message);
                    unredacted.push(message.clone())
                }
            }
        }

        self.messages = unredacted;
    }

    /// Set smart context information for this session
    #[allow(dead_code)]
    pub fn set_smart_context(&mut self, smart_context: SessionSmartContext) {
        self.smart_context = Some(smart_context);
    }

    /// Get smart context information if available
    #[allow(dead_code)]
    pub fn get_smart_context(&self) -> Option<&SessionSmartContext> {
        self.smart_context.as_ref()
    }

    /// Clear smart context information
    #[allow(dead_code)]
    pub fn clear_smart_context(&mut self) {
        self.smart_context = None;
    }

    /// Check if this session has valid smart context for a query
    #[allow(dead_code)]
    pub fn has_valid_smart_context(
        &self,
        query_hash: Option<&str>,
        config_hash: Option<&str>,
    ) -> bool {
        match &self.smart_context {
            Some(context) => context.is_valid_for_query(query_hash, config_hash),
            None => false,
        }
    }

    /// Get a display summary of the smart context
    #[allow(dead_code)]
    pub fn get_smart_context_summary(&self) -> Option<String> {
        self.smart_context.as_ref().map(|ctx| ctx.get_summary())
    }

    /// Add a message with smart context metadata
    #[allow(dead_code)]
    pub fn add_message_with_smart_context(
        &mut self,
        message: String,
        role: Role,
        include_context_info: bool,
    ) {
        let mut final_message = message;

        // If this is a user message and we have smart context, optionally include context info
        if matches!(role, Role::User) && include_context_info {
            if let Some(context) = &self.smart_context {
                let context_info = format!("\n\n[Smart Context: {}]", context.get_summary());
                final_message.push_str(&context_info);
            }
        }

        self.add_raw_message(final_message, role);
    }

    /// Update smart context results (when context is re-run)
    #[allow(dead_code)]
    pub fn update_smart_context_results(
        &mut self,
        selected_files: Vec<String>,
        total_tokens: Option<usize>,
        query_hash: Option<String>,
    ) {
        if let Some(context) = &mut self.smart_context {
            context.update_results(selected_files, total_tokens, query_hash);
        }
    }
}
