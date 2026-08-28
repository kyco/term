/// Represents different slash commands available in chat mode, organized by category
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatCommand {
    // General
    Help,
    Commands,
    Exit,
    #[allow(dead_code)]
    Quit,

    // Session management
    Save(Option<String>),
    /// List recent sessions without leaving the chat.
    Sessions,
    /// Show prompts that were stored but never answered.
    Unsent,
    Clear,
    Retry,
    Branch(Option<String>),

    // Context management
    Context,
    AddContext(String),
    RemoveContext(String),

    // AI settings
    Model(Option<String>),
    Provider(Option<String>),
    Effort(Option<String>),
    Tools(Option<bool>),
    Status,
    Theme(Option<String>),
    Streaming(Option<bool>),
    Settings,
}

/// Categories for grouping commands in the palette
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    General,
    Session,
    Context,
    AiSettings,
}

impl CommandCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Session => "Session",
            Self::Context => "Context",
            Self::AiSettings => "AI & Settings",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::General => ">>",
            Self::Session => "[]",
            Self::Context => "()",
            Self::AiSettings => "{}",
        }
    }

    /// Return all categories in display order
    pub fn all() -> &'static [CommandCategory] {
        &[
            Self::General,
            Self::Session,
            Self::Context,
            Self::AiSettings,
        ]
    }
}

/// A command entry for display in the palette
pub struct CommandEntry {
    pub command: &'static str,
    pub aliases: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
}

impl ChatCommand {
    /// Parse a line of input to determine if it's a slash command.
    /// Also supports `?` as a shortcut for opening the command palette.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();

        // Support `?` as a shortcut for the command palette
        if input == "?" {
            return Some(ChatCommand::Commands);
        }

        if !input.starts_with('/') {
            return None;
        }

        let parts: Vec<&str> = input[1..].split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0].to_lowercase().as_str() {
            // General
            "help" | "h" => Some(ChatCommand::Help),
            "commands" | "cmd" => Some(ChatCommand::Commands),
            "exit" | "quit" | "q" => Some(ChatCommand::Exit),

            // Session
            "save" | "s" => {
                let name = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                Some(ChatCommand::Save(name))
            }
            "sessions" | "ls" => Some(ChatCommand::Sessions),
            "unsent" => Some(ChatCommand::Unsent),
            "clear" | "c" => Some(ChatCommand::Clear),
            "retry" | "r" => Some(ChatCommand::Retry),
            "branch" | "b" => {
                let name = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                Some(ChatCommand::Branch(name))
            }

            // Context
            "context" | "ctx" => Some(ChatCommand::Context),
            "add" => {
                if parts.len() > 1 {
                    Some(ChatCommand::AddContext(parts[1..].join(" ")))
                } else {
                    None
                }
            }
            "remove" | "rm" => {
                if parts.len() > 1 {
                    Some(ChatCommand::RemoveContext(parts[1..].join(" ")))
                } else {
                    None
                }
            }

            // AI settings
            "model" | "m" => {
                let model = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                Some(ChatCommand::Model(model))
            }
            "provider" | "p" => {
                let provider = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                Some(ChatCommand::Provider(provider))
            }
            "effort" => {
                let effort = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                Some(ChatCommand::Effort(effort))
            }
            "tools" | "t" => {
                let setting = if parts.len() > 1 {
                    match parts[1].to_lowercase().as_str() {
                        "on" | "true" | "enable" | "enabled" | "1" => Some(true),
                        "off" | "false" | "disable" | "disabled" | "0" => Some(false),
                        _ => None,
                    }
                } else {
                    None
                };
                Some(ChatCommand::Tools(setting))
            }
            "status" => Some(ChatCommand::Status),
            "theme" => {
                let theme = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };
                Some(ChatCommand::Theme(theme))
            }
            "streaming" => {
                let setting = if parts.len() > 1 {
                    match parts[1].to_lowercase().as_str() {
                        "on" | "true" | "enable" | "enabled" | "1" => Some(true),
                        "off" | "false" | "disable" | "disabled" | "0" => Some(false),
                        _ => None,
                    }
                } else {
                    None
                };
                Some(ChatCommand::Streaming(setting))
            }
            "settings" | "config" => Some(ChatCommand::Settings),
            _ => None,
        }
    }

    /// Get help text for a specific command
    #[allow(dead_code)]
    pub fn help_text(&self) -> &'static str {
        match self {
            ChatCommand::Help => "Show quick help",
            ChatCommand::Commands => "Open command palette with all commands",
            ChatCommand::Exit | ChatCommand::Quit => "Exit chat mode",
            ChatCommand::Save(_) => "Save current session with optional name",
            ChatCommand::Sessions => "List recent sessions",
            ChatCommand::Unsent => "Show messages that never got a reply",
            ChatCommand::Clear => "Clear conversation history",
            ChatCommand::Retry => "Regenerate the last AI response",
            ChatCommand::Branch(_) => "Create a new conversation branch",
            ChatCommand::Context => "Show current context information",
            ChatCommand::AddContext(_) => "Add file or directory to context",
            ChatCommand::RemoveContext(_) => "Remove file or directory from context",
            ChatCommand::Model(_) => "Switch AI model or show current",
            ChatCommand::Provider(_) => "Switch AI provider or show current",
            ChatCommand::Effort(_) => "Set reasoning effort for this session or show current",
            ChatCommand::Tools(_) => "Toggle tool usage (OpenAI only)",
            ChatCommand::Status => "Show current session status",
            ChatCommand::Theme(_) => "Switch display theme or list themes",
            ChatCommand::Streaming(_) => "Toggle streaming output",
            ChatCommand::Settings => "Show all current settings",
        }
    }

    /// Get the full command catalogue, organized by category
    pub fn command_palette() -> Vec<CommandEntry> {
        vec![
            // General
            CommandEntry {
                command: "/help",
                aliases: "/h",
                description: "Show quick help",
                category: CommandCategory::General,
            },
            CommandEntry {
                command: "/commands",
                aliases: "/cmd, ?",
                description: "Open this command palette",
                category: CommandCategory::General,
            },
            CommandEntry {
                command: "/exit",
                aliases: "/quit, /q",
                description: "Exit chat mode",
                category: CommandCategory::General,
            },
            // Session
            CommandEntry {
                command: "/save [name]",
                aliases: "/s",
                description: "Save session with optional name",
                category: CommandCategory::Session,
            },
            CommandEntry {
                command: "/sessions",
                aliases: "/ls",
                description: "List recent sessions",
                category: CommandCategory::Session,
            },
            CommandEntry {
                command: "/unsent",
                aliases: "",
                description: "Show messages that never got a reply",
                category: CommandCategory::Session,
            },
            CommandEntry {
                command: "/clear",
                aliases: "/c",
                description: "Clear conversation history",
                category: CommandCategory::Session,
            },
            CommandEntry {
                command: "/retry",
                aliases: "/r",
                description: "Regenerate last AI response",
                category: CommandCategory::Session,
            },
            CommandEntry {
                command: "/branch [name]",
                aliases: "/b",
                description: "Create conversation branch",
                category: CommandCategory::Session,
            },
            // Context
            CommandEntry {
                command: "/context",
                aliases: "/ctx",
                description: "Show current context info",
                category: CommandCategory::Context,
            },
            CommandEntry {
                command: "/add <path>",
                aliases: "",
                description: "Add file/directory to context",
                category: CommandCategory::Context,
            },
            CommandEntry {
                command: "/remove <path>",
                aliases: "/rm",
                description: "Remove from context",
                category: CommandCategory::Context,
            },
            // AI settings
            CommandEntry {
                command: "/model [name]",
                aliases: "/m",
                description: "Switch model or show current",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/provider [name]",
                aliases: "/p",
                description: "Switch provider (claude/openai/codex)",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/effort [level]",
                aliases: "",
                description: "Set reasoning effort (max/ultra: Sol only)",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/tools [on|off]",
                aliases: "/t",
                description: "Toggle tool usage (OpenAI)",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/status",
                aliases: "",
                description: "Show session status overview",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/theme [name]",
                aliases: "",
                description: "Switch theme or list themes",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/streaming [on|off]",
                aliases: "",
                description: "Toggle streaming output",
                category: CommandCategory::AiSettings,
            },
            CommandEntry {
                command: "/settings",
                aliases: "/config",
                description: "Show all current settings",
                category: CommandCategory::AiSettings,
            },
        ]
    }

    /// Legacy flat list for backward compat (used by /help)
    pub fn all_commands() -> Vec<(&'static str, &'static str)> {
        Self::command_palette()
            .iter()
            .map(|entry| {
                if entry.aliases.is_empty() {
                    (entry.command, entry.description)
                } else {
                    // Leak a combined string so we can return &'static str
                    // This is fine since it's only called for display and the set is fixed
                    let combined: &'static str =
                        Box::leak(format!("{}, {}", entry.command, entry.aliases).into_boxed_str());
                    (combined, entry.description)
                }
            })
            .collect()
    }
}

/// Represents the result of processing user input
#[derive(Debug, Clone)]
pub enum InputType {
    Command(ChatCommand),
    Message(String),
}

impl InputType {
    /// Classify user input as either a command or a regular message
    pub fn classify(input: &str) -> Self {
        if let Some(command) = ChatCommand::parse(input) {
            InputType::Command(command)
        } else {
            InputType::Message(input.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_parsing() {
        assert_eq!(ChatCommand::parse("/help"), Some(ChatCommand::Help));
        assert_eq!(ChatCommand::parse("/h"), Some(ChatCommand::Help));
        assert_eq!(ChatCommand::parse("/save"), Some(ChatCommand::Save(None)));
        assert_eq!(
            ChatCommand::parse("/save my_session"),
            Some(ChatCommand::Save(Some("my_session".to_string())))
        );
        assert_eq!(ChatCommand::parse("/context"), Some(ChatCommand::Context));
        assert_eq!(ChatCommand::parse("/clear"), Some(ChatCommand::Clear));
        assert_eq!(ChatCommand::parse("/exit"), Some(ChatCommand::Exit));
        assert_eq!(ChatCommand::parse("/quit"), Some(ChatCommand::Exit));
        assert_eq!(ChatCommand::parse("/retry"), Some(ChatCommand::Retry));
        assert_eq!(
            ChatCommand::parse("/add src/main.rs"),
            Some(ChatCommand::AddContext("src/main.rs".to_string()))
        );
        assert_eq!(
            ChatCommand::parse("/model gpt-5.2"),
            Some(ChatCommand::Model(Some("gpt-5.2".to_string())))
        );
        assert_eq!(ChatCommand::parse("/model"), Some(ChatCommand::Model(None)));
        assert_eq!(
            ChatCommand::parse("/provider openai"),
            Some(ChatCommand::Provider(Some("openai".to_string())))
        );
        assert_eq!(
            ChatCommand::parse("/provider"),
            Some(ChatCommand::Provider(None))
        );

        // Non-commands
        assert_eq!(ChatCommand::parse("hello world"), None);
        assert_eq!(ChatCommand::parse("not a command"), None);
        assert_eq!(ChatCommand::parse(""), None);
    }

    #[test]
    fn test_question_mark_shortcut() {
        assert_eq!(ChatCommand::parse("?"), Some(ChatCommand::Commands));
    }

    #[test]
    fn test_new_commands() {
        assert_eq!(ChatCommand::parse("/commands"), Some(ChatCommand::Commands));
        assert_eq!(ChatCommand::parse("/cmd"), Some(ChatCommand::Commands));
        assert_eq!(ChatCommand::parse("/status"), Some(ChatCommand::Status));
        assert_eq!(ChatCommand::parse("/settings"), Some(ChatCommand::Settings));
        assert_eq!(ChatCommand::parse("/config"), Some(ChatCommand::Settings));
        assert_eq!(ChatCommand::parse("/theme"), Some(ChatCommand::Theme(None)));
        assert_eq!(
            ChatCommand::parse("/theme dark"),
            Some(ChatCommand::Theme(Some("dark".to_string())))
        );
        assert_eq!(
            ChatCommand::parse("/streaming on"),
            Some(ChatCommand::Streaming(Some(true)))
        );
        assert_eq!(
            ChatCommand::parse("/streaming off"),
            Some(ChatCommand::Streaming(Some(false)))
        );
        assert_eq!(
            ChatCommand::parse("/streaming"),
            Some(ChatCommand::Streaming(None))
        );
    }

    #[test]
    fn test_input_classification() {
        match InputType::classify("/help") {
            InputType::Command(ChatCommand::Help) => (),
            _ => panic!("Expected Help command"),
        }

        match InputType::classify("?") {
            InputType::Command(ChatCommand::Commands) => (),
            _ => panic!("Expected Commands command from ?"),
        }

        match InputType::classify("Hello, how are you?") {
            InputType::Message(msg) => assert_eq!(msg, "Hello, how are you?"),
            _ => panic!("Expected regular message"),
        }
    }

    #[test]
    fn test_effort_command_parsing() {
        assert_eq!(
            ChatCommand::parse("/effort"),
            Some(ChatCommand::Effort(None))
        );
        assert_eq!(
            ChatCommand::parse("/effort ultra"),
            Some(ChatCommand::Effort(Some("ultra".to_string())))
        );
        assert_eq!(
            ChatCommand::parse("/effort max"),
            Some(ChatCommand::Effort(Some("max".to_string())))
        );
        assert_eq!(
            ChatCommand::parse("/effort default"),
            Some(ChatCommand::Effort(Some("default".to_string())))
        );
    }

    #[test]
    fn test_command_palette_contains_effort() {
        let palette = ChatCommand::command_palette();
        let entry = palette
            .iter()
            .find(|e| e.command.starts_with("/effort"))
            .expect("palette should contain /effort");
        assert_eq!(entry.category, CommandCategory::AiSettings);
    }

    #[test]
    fn test_command_palette_has_all_categories() {
        let palette = ChatCommand::command_palette();
        for cat in CommandCategory::all() {
            assert!(
                palette.iter().any(|e| e.category == *cat),
                "Missing commands for category {:?}",
                cat
            );
        }
    }
}
