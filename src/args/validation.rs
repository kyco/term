/// Intelligent argument validation and conflict detection system
use crate::args::{Args, AskArgs, ChatArgs, Commands};
use anyhow::Result;
use colored::*;

/// Validation errors with actionable guidance
#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
    pub suggestions: Vec<String>,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            suggestions: Vec::new(),
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions.extend(suggestions);
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.suggestions.is_empty() {
            write!(f, "\n\n{}", "💡 Suggestions:".bright_yellow().bold())?;
            for suggestion in &self.suggestions {
                write!(f, "\n   • {}", suggestion.cyan())?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ValidationError {}

/// Argument validator with intelligent conflict detection
pub struct ArgumentValidator;

impl ArgumentValidator {
    /// Validate arguments and detect conflicts
    pub fn validate(args: &Args) -> Result<(), ValidationError> {
        if let Some(ref command) = args.command {
            match command {
                Commands::Chat(chat_args) => {
                    Self::validate_chat_args(chat_args)?;
                    Self::validate_context_args_common(
                        &chat_args.directory,
                        &chat_args.directories,
                        &chat_args.exclude,
                        chat_args.smart_context,
                        chat_args.max_context_tokens,
                        chat_args.preview_context,
                    )?;
                }
                Commands::Ask(ask_args) => {
                    Self::validate_ask_args(ask_args)?;
                    Self::validate_context_args_common(
                        &ask_args.directory,
                        &ask_args.directories,
                        &ask_args.exclude,
                        ask_args.smart_context,
                        ask_args.max_context_tokens,
                        ask_args.preview_context,
                    )?;
                }
                Commands::Setup(setup_args) => {
                    Self::validate_setup_args(setup_args)?;
                }
                Commands::Config {
                    args: _config_args,
                    action: _action,
                } => {
                    // Config commands are generally safe, minimal validation needed
                }
                Commands::Auth { action: _action } => {
                    // Authentication commands are generally safe, minimal validation needed
                }
                Commands::Sessions {
                    args: _session_args,
                    action: _action,
                } => {
                    // Session commands are generally safe, minimal validation needed
                }
                Commands::Redact {
                    args: _redact_args,
                    action: _action,
                } => {
                    // Redact commands are generally safe, minimal validation needed
                }
                Commands::Completion {
                    args: _completion_args,
                    action: _action,
                } => {
                    // Completion commands are generally safe, minimal validation needed
                }
                Commands::Complete { args: _args } => {
                    // Internal completion helper, no validation needed
                }
                Commands::Commit(commit_args) => {
                    Self::validate_commit_args(commit_args)?;
                }
                Commands::Review(review_args) => {
                    Self::validate_review_args(review_args)?;
                }
                Commands::BranchSummary(_branch_args) => {
                    // Branch summary commands are generally safe, minimal validation needed
                }
                Commands::Hooks(_hooks_args) => {
                    // Hooks commands are generally safe, minimal validation needed
                }
                Commands::Stash(_stash_args) => {
                    // Stash commands are generally safe, minimal validation needed
                }
                Commands::Tag(_tag_args) => {
                    // Tag commands are generally safe, minimal validation needed
                }
                Commands::Rebase(_rebase_args) => {
                    // Rebase commands are generally safe, minimal validation needed
                }
                Commands::Conflicts(_conflicts_args) => {
                    // Conflicts commands are generally safe, minimal validation needed
                }
                Commands::Help => {
                    // Internal help command, no validation needed
                }
                Commands::Man {
                    output: _,
                    install_help: _,
                } => {
                    // Internal man page command, no validation needed
                }
                Commands::Preset(_) => {
                    // Preset command validation handled internally
                }
            }
        }

        // Validate global argument conflicts
        Self::validate_global_conflicts(args)?;

        Ok(())
    }

    /// Validate chat-specific arguments
    fn validate_chat_args(args: &ChatArgs) -> Result<(), ValidationError> {
        // Check chunked analysis requires smart context
        if args.chunked_analysis && !args.smart_context {
            return Err(ValidationError::new(
                "Chunked analysis requires smart context to be enabled",
            )
            .with_suggestion("Add --smart-context flag to enable smart context discovery")
            .with_suggestion("Or remove --chunked-analysis if you don't need it"));
        }

        // Check context query requires smart context
        if args.context_query.is_some() && !args.smart_context {
            return Err(
                ValidationError::new("Context query requires smart context to be enabled")
                    .with_suggestion("Add --smart-context flag to enable context discovery")
                    .with_suggestion(
                        "Or remove --context-query if you don't need targeted context",
                    ),
            );
        }

        // Check preview context requires smart context
        if args.preview_context && !args.smart_context {
            return Err(ValidationError::new(
                "Context preview requires smart context to be enabled",
            )
            .with_suggestion("Add --smart-context flag to enable context discovery")
            .with_suggestion("Or remove --preview-context if preview isn't needed"));
        }

        // Validate chunking strategy
        if args.chunked_analysis {
            let valid_strategies = ["module", "functional", "token", "hierarchical"];
            if !valid_strategies.contains(&args.chunk_strategy.as_str()) {
                return Err(ValidationError::new(format!(
                    "Invalid chunk strategy: '{}'",
                    args.chunk_strategy
                ))
                .with_suggestions(
                    valid_strategies
                        .iter()
                        .map(|s| format!("Use --chunk-strategy {}", s))
                        .collect(),
                ));
            }
        }

        Ok(())
    }

    /// Validate ask-specific arguments
    fn validate_ask_args(args: &AskArgs) -> Result<(), ValidationError> {
        // Check question is not empty
        if args.question.trim().is_empty() {
            return Err(ValidationError::new("Question cannot be empty")
                .with_suggestion("Provide a question: termai ask \"What does this code do?\"")
                .with_suggestion(
                    "Use quotes if your question contains spaces or special characters",
                ));
        }

        // Check question length (reasonable limits)
        if args.question.len() > 5000 {
            return Err(
                ValidationError::new("Question is too long (maximum 5000 characters)")
                    .with_suggestion("Break down your question into smaller, more focused queries")
                    .with_suggestion("Use 'termai chat' for longer conversations"),
            );
        }

        // Same context validation as chat
        if args.chunked_analysis && !args.smart_context {
            return Err(ValidationError::new(
                "Chunked analysis requires smart context to be enabled",
            )
            .with_suggestion("Add --smart-context flag")
            .with_suggestion("Or remove --chunked-analysis"));
        }

        if args.context_query.is_some() && !args.smart_context {
            return Err(
                ValidationError::new("Context query requires smart context to be enabled")
                    .with_suggestion("Add --smart-context flag")
                    .with_suggestion("Or remove --context-query"),
            );
        }

        if args.preview_context && !args.smart_context {
            return Err(ValidationError::new(
                "Context preview requires smart context to be enabled",
            )
            .with_suggestion("Add --smart-context flag")
            .with_suggestion("Or remove --preview-context"));
        }

        Ok(())
    }

    /// Validate setup-specific arguments
    fn validate_setup_args(args: &crate::args::SetupArgs) -> Result<(), ValidationError> {
        // Check for conflicting setup flags
        if args.skip_validation && args.auto_accept {
            return Err(
                ValidationError::new("Cannot use --skip-validation with --auto-accept")
                    .with_suggestion("Use --skip-validation for testing without validation")
                    .with_suggestion("Use --auto-accept for non-interactive setup with validation")
                    .with_suggestion("Remove one of these flags"),
            );
        }

        Ok(())
    }

    /// Validate common context arguments
    fn validate_context_args_common(
        directory: &Option<String>,
        directories: &[String],
        exclude: &[String],
        smart_context: bool,
        max_context_tokens: Option<usize>,
        preview_context: bool,
    ) -> Result<(), ValidationError> {
        // Check for conflicting directory specifications
        if directory.is_some() && !directories.is_empty() {
            return Err(ValidationError::new(
                "Cannot specify both single directory and multiple directories",
            )
            .with_suggestion("Use --directory for a single directory")
            .with_suggestion("Use --directories dir1,dir2,dir3 for multiple directories")
            .with_suggestion("Combine into --directories if you need multiple paths"));
        }

        // Validate context token limits
        if let Some(tokens) = max_context_tokens {
            if tokens == 0 {
                return Err(
                    ValidationError::new("Maximum context tokens cannot be zero")
                        .with_suggestion("Remove --max-context-tokens to use default limit")
                        .with_suggestion("Use a positive number for token limit"),
                );
            }
            if tokens > 100_000 {
                return Err(ValidationError::new(
                    "Maximum context tokens is too high (limit: 100,000)",
                )
                .with_suggestion("Use a smaller token limit for better performance")
                .with_suggestion("Consider using --chunked-analysis for large contexts"));
            }
        }

        // Check exclude patterns are meaningful
        for pattern in exclude {
            if pattern.trim().is_empty() {
                return Err(ValidationError::new("Empty exclude pattern is not allowed")
                    .with_suggestion("Remove empty exclude patterns")
                    .with_suggestion("Use specific patterns like '*.log' or 'target/'"));
            }
        }

        // Warn about potential conflicts
        if !smart_context && max_context_tokens.is_some() {
            return Err(ValidationError::new(
                "Context token limits are only useful with smart context",
            )
            .with_suggestion("Add --smart-context to enable intelligent context selection")
            .with_suggestion("Or remove --max-context-tokens if not using smart context"));
        }

        if !smart_context && preview_context {
            // This is already handled in the specific command validators, but good to double-check
            return Err(
                ValidationError::new("Context preview requires smart context")
                    .with_suggestion("Add --smart-context flag"),
            );
        }

        Ok(())
    }

    /// Validate global argument conflicts
    fn validate_global_conflicts(args: &Args) -> Result<(), ValidationError> {
        // Check smart context conflicts with legacy behavior
        if args.smart_context && (args.directory.is_some() || !args.directories.is_empty()) {
            return Err(ValidationError::new(
                "Smart context conflicts with explicit directory specification",
            )
            .with_suggestion("Use smart context for automatic discovery")
            .with_suggestion("Or use explicit directories without --smart-context")
            .with_suggestion("Smart context will automatically find relevant files"));
        }

        // Check for reasonable exclude pattern syntax
        for pattern in &args.exclude {
            if pattern.contains("**") && pattern.matches("**").count() > 2 {
                return Err(ValidationError::new(format!(
                    "Complex glob pattern may be inefficient: '{}'",
                    pattern
                ))
                .with_suggestion("Use simpler patterns like '*.ext' or 'dir/'")
                .with_suggestion("Avoid excessive recursive wildcards"));
            }
        }

        Ok(())
    }

    /// Display validation error with enhanced formatting
    pub fn display_validation_error(error: &ValidationError) {
        eprintln!(
            "{} {}",
            "❌ Validation Error:".red().bold(),
            error.message.white()
        );

        if !error.suggestions.is_empty() {
            eprintln!();
            eprintln!("{}", "💡 Suggestions:".bright_yellow().bold());
            for suggestion in &error.suggestions {
                eprintln!("   • {}", suggestion.cyan());
            }
        }

        eprintln!();
        eprintln!(
            "{} Use {} for more information about command options.",
            "ℹ️ ".blue(),
            "termai <command> --help".bright_cyan()
        );
    }

    /// Validate commit-specific arguments
    fn validate_commit_args(args: &crate::args::CommitArgs) -> Result<(), ValidationError> {
        // Check for conflicting options
        if args.auto && args.template.is_some() {
            return Err(ValidationError::new("Cannot use --auto with --template")
                .with_suggestion(
                    "Use --auto for automatic commit or --template for custom template, not both",
                )
                .with_suggestion("Remove one of these options"));
        }

        // Check message type if provided
        if let Some(ref msg_type) = args.message_type {
            let valid_types = [
                "feat", "fix", "docs", "style", "refactor", "test", "chore", "build", "ci",
            ];
            if !valid_types.contains(&msg_type.as_str()) {
                return Err(ValidationError::new(format!(
                    "Invalid commit message type: '{}'",
                    msg_type
                ))
                .with_suggestions(
                    valid_types
                        .iter()
                        .map(|t| format!("Use --message-type {}", t))
                        .collect(),
                ));
            }
        }

        Ok(())
    }

    /// Validate review-specific arguments
    fn validate_review_args(args: &crate::args::ReviewArgs) -> Result<(), ValidationError> {
        // Check if specific files are provided but don't exist
        for file_pattern in &args.files {
            if file_pattern.trim().is_empty() {
                return Err(ValidationError::new("Empty file pattern is not allowed")
                    .with_suggestion(
                        "Provide specific file patterns like 'src/*.rs' or remove --files",
                    ));
            }
        }

        // Check output file path if provided
        if let Some(ref output_path) = args.output {
            if output_path.trim().is_empty() {
                return Err(ValidationError::new("Output file path cannot be empty")
                    .with_suggestion(
                        "Provide a valid file path for --output or remove the option",
                    ));
            }

            // Check for potentially dangerous paths
            if output_path.contains("..") {
                return Err(ValidationError::new(
                    "Output path contains potentially unsafe directory traversal",
                )
                .with_suggestion("Use a simple filename or relative path without '..'"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Args, Commands};

    #[test]
    fn test_empty_ask_question() {
        let args = Args {
            command: Some(Commands::Ask(AskArgs {
                question: "".to_string(),
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

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.message.contains("Question cannot be empty"));
        assert!(!error.suggestions.is_empty());
    }

    #[test]
    fn test_chunked_analysis_without_smart_context() {
        let args = Args {
            command: Some(Commands::Chat(ChatArgs {
                temporary: false,
                input: None,
                directory: None,
                directories: vec![],
                exclude: vec![],
                system_prompt: None,
                session: None,
                last_session: false,
                smart_context: false,
                context_query: None,
                max_context_tokens: None,
                preview_context: false,
                chunked_analysis: true,
                chunk_strategy: "hierarchical".to_string(),
            })),
            ..Default::default()
        };

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .message
            .contains("Chunked analysis requires smart context"));
    }

    #[test]
    fn test_conflicting_directory_args() {
        let args = Args {
            command: Some(Commands::Chat(ChatArgs {
                temporary: false,
                input: None,
                directory: Some("src/".to_string()),
                directories: vec!["tests/".to_string()],
                exclude: vec![],
                system_prompt: None,
                session: None,
                last_session: false,
                smart_context: false,
                context_query: None,
                max_context_tokens: None,
                preview_context: false,
                chunked_analysis: false,
                chunk_strategy: "hierarchical".to_string(),
            })),
            ..Default::default()
        };

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .message
            .contains("Cannot specify both single directory"));
    }

    #[test]
    fn test_invalid_chunk_strategy() {
        let args = Args {
            command: Some(Commands::Chat(ChatArgs {
                temporary: false,
                input: None,
                directory: None,
                directories: vec![],
                exclude: vec![],
                system_prompt: None,
                session: None,
                last_session: false,
                smart_context: true,
                context_query: None,
                max_context_tokens: None,
                preview_context: false,
                chunked_analysis: true,
                chunk_strategy: "invalid".to_string(),
            })),
            ..Default::default()
        };

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.message.contains("Invalid chunk strategy"));
    }

    #[test]
    fn test_zero_context_tokens() {
        let args = Args {
            command: Some(Commands::Ask(AskArgs {
                question: "test".to_string(),
                directory: None,
                directories: vec![],
                exclude: vec![],
                system_prompt: None,
                session: None,
                smart_context: true,
                context_query: None,
                max_context_tokens: Some(0),
                preview_context: false,
                chunked_analysis: false,
                chunk_strategy: "hierarchical".to_string(),
            })),
            ..Default::default()
        };

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .message
            .contains("Maximum context tokens cannot be zero"));
    }

    #[test]
    fn test_valid_args() {
        let args = Args {
            command: Some(Commands::Ask(AskArgs {
                question: "What does this code do?".to_string(),
                directory: None,
                directories: vec![],
                exclude: vec![],
                system_prompt: None,
                session: None,
                smart_context: true,
                context_query: Some("rust functions".to_string()),
                max_context_tokens: Some(4000),
                preview_context: true,
                chunked_analysis: true,
                chunk_strategy: "hierarchical".to_string(),
            })),
            ..Default::default()
        };

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_conflicting_setup_args() {
        let args = Args {
            command: Some(Commands::Setup(crate::args::SetupArgs {
                skip_validation: true,
                force: false,
                auto_accept: true,
            })),
            ..Default::default()
        };

        let result = ArgumentValidator::validate(&args);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .message
            .contains("Cannot use --skip-validation with --auto-accept"));
    }
}
