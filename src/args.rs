/// Argument parsing module with dedicated structures for each subcommand
pub mod structs;
pub mod validation;

pub use structs::*;
pub use validation::ArgumentValidator;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug, Default)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    // Legacy support for backwards compatibility during transition
    #[arg(long, hide = true)]
    pub chat_gpt_api_key: Option<String>,
    #[arg(long, hide = true)]
    pub claude_api_key: Option<String>,
    #[arg(long)]
    pub system_prompt: Option<String>,
    #[arg(long, hide = true)]
    pub redact_add: Option<String>,
    #[arg(long, hide = true)]
    pub redact_remove: Option<String>,
    #[arg(long, hide = true)]
    pub redact_list: bool,
    #[arg(short, long, hide = true)]
    pub print_config: bool,
    #[arg(long, hide = true)]
    pub sessions_all: bool,
    #[arg(long)]
    pub session: Option<String>,
    pub data: Option<String>,
    pub(crate) directory: Option<String>,
    #[arg(short, long, value_delimiter = ',')]
    pub(crate) exclude: Vec<String>,
    #[arg(long, value_enum, hide = true)]
    pub provider: Option<Provider>,
    #[arg(short = 'd', long, value_delimiter = ',')]
    pub(crate) directories: Vec<String>,

    /// Enable smart context discovery (automatically finds relevant files)
    #[arg(long)]
    pub smart_context: bool,

    /// Maximum tokens for smart context (default: 4000)
    #[arg(long, requires = "smart_context")]
    pub max_context_tokens: Option<usize>,

    /// Preview selected context before proceeding
    #[arg(long, requires = "smart_context")]
    pub preview_context: bool,

    /// Enable chunked analysis for large projects
    #[arg(long, requires = "smart_context")]
    pub chunked_analysis: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Interactive setup wizard to configure TermAI
    Setup(SetupArgs),

    /// Manage configuration settings
    Config {
        #[command(flatten)]
        args: ConfigArgs,
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage provider authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Manage redaction patterns for privacy
    Redact {
        #[command(flatten)]
        args: RedactArgs,
        #[command(subcommand)]
        action: RedactAction,
    },

    /// Manage chat sessions
    Sessions {
        #[command(flatten)]
        args: SessionArgs,
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Generate shell completion scripts
    Completion {
        #[command(flatten)]
        args: CompletionArgs,
        #[command(subcommand)]
        action: CompletionAction,
    },

    /// Ask a one-shot question with optional context
    Ask(AskArgs),

    /// Start an interactive chat session
    Chat(ChatArgs),

    /// Internal completion helper (hidden)
    #[command(hide = true)]
    Complete {
        /// Command line arguments to complete
        args: Vec<String>,
    },

    /// Generate AI-powered commit messages
    Commit(CommitArgs),

    /// Review staged changes with AI analysis
    Review(ReviewArgs),

    /// Analyze Git branches and generate insights
    #[command(name = "branch-summary")]
    BranchSummary(BranchArgs),

    /// Manage Git hooks integration
    Hooks(HooksArgs),

    /// Manage Git stash with AI assistance
    Stash(StashArgs),

    /// Manage Git tags and releases with AI assistance
    Tag(TagArgs),

    /// Interactive Git rebase with AI assistance
    Rebase(RebaseArgs),

    /// Git merge conflict resolution with AI assistance
    Conflicts(ConflictsArgs),

    /// Manage templates and presets for reusable AI interactions
    Preset(PresetArgs),

    /// Show command discovery help and suggestions (hidden)
    #[command(hide = true, name = "discovery")]
    Help,

    /// Generate man page documentation (hidden)
    #[command(hide = true, name = "man")]
    Man {
        /// Output file path (optional, defaults to stdout)
        #[arg(long)]
        output: Option<String>,

        /// Show installation instructions
        #[arg(long)]
        install_help: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set OpenAI API key
    #[command(hide = true)]
    SetOpenai { api_key: String },
    /// Set Claude API key  
    #[command(hide = true)]
    SetClaude { api_key: String },
    /// Set default provider
    #[command(hide = true)]
    SetProvider {
        #[arg(value_enum)]
        provider: Provider,
    },
    /// Reset all configuration (clears API keys and settings)
    #[command(hide = true)]
    Reset,
    /// Show environment variable help and current values
    #[command(hide = true)]
    Env,

    // Project configuration commands
    /// Initialize project configuration (.termai.toml)
    Init {
        /// Project type (rust, javascript, python, etc.)
        #[arg(long)]
        project_type: Option<String>,
        /// Use template for project type
        #[arg(long)]
        template: Option<String>,
        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,
    },
    /// Show project configuration details
    #[command(hide = true)]
    Project,
    /// Validate project configuration
    Validate,
    /// Edit project configuration in default editor
    Edit,
    /// Migrate legacy configuration into the simplified TOML model
    Migrate,
    /// Export project configuration to file
    #[command(hide = true)]
    Export {
        /// Output file path
        #[arg(long)]
        file: Option<String>,
        /// Export format (toml, json, yaml)
        #[arg(long, default_value = "toml")]
        format: String,
    },
    /// Import project configuration from file
    #[command(hide = true)]
    Import {
        /// Input file path
        file: String,
        /// Merge with existing configuration instead of replacing
        #[arg(long)]
        merge: bool,
    },
    /// Login to OpenAI Codex using OAuth (ChatGPT Plus/Pro)
    #[command(hide = true)]
    #[command(name = "login-codex")]
    LoginCodex,
    /// Logout from OpenAI Codex (clear OAuth tokens)
    #[command(hide = true)]
    #[command(name = "logout-codex")]
    LogoutCodex,
    /// Show OpenAI Codex authentication status
    #[command(hide = true)]
    #[command(name = "codex-status")]
    CodexStatus,
    /// Set default model for current provider
    #[command(name = "set-model")]
    SetModel {
        /// Optional model name (omit to open the interactive selector)
        model: Option<String>,
    },
    /// Set reasoning effort for OpenAI/Codex requests
    #[command(name = "set-effort")]
    SetEffort {
        /// Effort level (none|low|medium|high|xhigh|max|ultra), or "default" to clear
        effort: String,
    },
    /// List available models
    #[command(name = "list-models")]
    ListModels {
        /// Filter by provider
        #[arg(long, value_enum)]
        provider: Option<Provider>,
        /// Force refresh from API (ignore cache)
        #[arg(long)]
        refresh: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum AuthAction {
    /// Log in to a provider
    Login(AuthProviderArgs),
    /// Log out from a provider
    Logout(AuthProviderArgs),
    /// Show authentication status for a provider
    Status(AuthProviderArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct AuthProviderArgs {
    /// Provider to manage authentication for
    #[arg(value_enum)]
    pub provider: Provider,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RedactAction {
    /// Add a new redaction pattern
    Add { pattern: String },
    /// Remove a redaction pattern
    Remove { pattern: String },
    /// List all redaction patterns
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SessionAction {
    /// List all sessions
    List {
        /// Filter by session name or opening message
        #[arg(long)]
        filter: Option<String>,
        /// Limit the number of sessions shown
        #[arg(long)]
        limit: Option<usize>,
        /// Sort by: name, date, messages
        #[arg(long, value_enum)]
        sort: Option<SessionSortOrder>,
    },
    /// Find and undo duplicated messages left by older versions
    Repair {
        /// Apply the repair (default is a preview)
        #[arg(long)]
        apply: bool,
        /// Put previously repaired messages back
        #[arg(long, conflicts_with = "apply")]
        undo: bool,
    },
    /// Find sessions by what was said in them
    Find {
        /// Text to look for in message content
        query: String,
        /// Maximum number of matches to show
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Delete a specific session
    Delete {
        /// Name of the session to delete
        name: String,
    },
    /// Show detailed information about a session
    Show {
        /// Name of the session to show
        name: String,
    },
    /// Create a new conversation branch
    Branch {
        /// Source session name to branch from
        session: String,
        /// Name for the new branch (optional)
        #[arg(long)]
        name: Option<String>,
        /// Description for the branch (optional)
        #[arg(long)]
        description: Option<String>,
        /// Branch from specific message index (0-based)
        #[arg(long)]
        from_message: Option<usize>,
    },
    /// Show branch tree visualization for a session
    Tree {
        /// Session name to show tree for
        session: String,
        /// Show interactive navigation interface
        #[arg(long)]
        interactive: bool,
        /// Highlight specific branch in tree
        #[arg(long)]
        highlight: Option<String>,
    },
    /// List all branches in a session
    Branches {
        /// Session name to list branches for
        session: String,
        /// Show detailed branch information
        #[arg(long)]
        detailed: bool,
        /// Filter by branch status (active, archived, etc.)
        #[arg(long)]
        status: Option<String>,
    },
    /// Switch to a different branch in a session
    Switch {
        /// Session name containing the target branch
        session: String,
        /// Branch name or ID to switch to
        branch: String,
        /// Create a new chat session on the target branch
        #[arg(long)]
        new_session: bool,
    },
    /// Bookmark a branch for quick access
    Bookmark {
        /// Session name containing the target branch
        session: String,
        /// Branch name or ID to bookmark
        branch: String,
        /// Bookmark name (optional, uses branch name if not provided)
        #[arg(long)]
        name: Option<String>,
        /// Remove the bookmark instead of adding it
        #[arg(long)]
        remove: bool,
    },
    /// Search branches by name, description, or bookmark
    Search {
        /// Session name to search within
        session: String,
        /// Search query (searches name, description, and bookmarks)
        query: String,
        /// Filter by branch status (active, archived, etc.)
        #[arg(long)]
        status: Option<String>,
        /// Show detailed results
        #[arg(long)]
        detailed: bool,
    },
    /// Show branch statistics and analytics
    Stats {
        /// Session name to analyze
        session: String,
        /// Show detailed analytics
        #[arg(long)]
        detailed: bool,
    },
    /// Compare branches side-by-side
    Compare {
        /// Session name containing branches to compare
        session: String,
        /// Branch names or IDs to compare (space-separated)
        branches: Vec<String>,
        /// Show side-by-side comparison view
        #[arg(long)]
        side_by_side: bool,
        /// Compare only outcomes/conclusions
        #[arg(long)]
        outcomes_only: bool,
        /// Show detailed quality analysis
        #[arg(long)]
        detailed: bool,
    },
    /// Merge branches with conflict resolution
    Merge {
        /// Session name containing branches to merge
        session: String,
        /// Source branch names or IDs to merge from
        source_branches: Vec<String>,
        /// Target branch name or ID to merge into
        #[arg(long)]
        into: String,
        /// Merge strategy: sequential, intelligent, selective, summary, best-of
        #[arg(long, default_value = "intelligent")]
        strategy: String,
        /// Preview merge before applying
        #[arg(long)]
        preview: bool,
        /// Auto-confirm merge if no conflicts
        #[arg(long)]
        auto_confirm: bool,
    },
    /// Perform selective merge (cherry-pick messages)
    SelectiveMerge {
        /// Session name containing branches
        session: String,
        /// Source branch name or ID
        source: String,
        /// Target branch name or ID
        target: String,
        /// Message indices to merge (comma-separated, 0-based)
        #[arg(long, value_delimiter = ',')]
        messages: Vec<usize>,
        /// Preview selected messages before merge
        #[arg(long)]
        preview: bool,
    },
    /// Archive branches after successful merge
    Archive {
        /// Session name containing branches
        session: String,
        /// Branch names or IDs to archive
        branches: Vec<String>,
        /// Reason for archiving
        #[arg(long)]
        reason: Option<String>,
    },
    /// Clean up old or unused branches
    Cleanup {
        /// Session name to clean up
        session: String,
        /// Cleanup strategy: archive-old, remove-empty, consolidate-similar, remove-duplicates
        #[arg(long, default_value = "archive-old")]
        strategy: String,
        /// Days threshold for archive-old strategy
        #[arg(long, default_value = "30")]
        days: i64,
        /// Preview cleanup actions before applying
        #[arg(long)]
        preview: bool,
    },
    /// Export branches to external formats
    Export {
        /// Session name containing branches
        session: String,
        /// Branch names or IDs to export
        branches: Vec<String>,
        /// Export format: json, markdown, csv, text
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file path (defaults to stdout)
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CompletionAction {
    /// Generate Bash completion script
    Bash,
    /// Generate Zsh completion script
    Zsh,
    /// Generate Fish completion script
    Fish,
    /// Generate PowerShell completion script
    PowerShell,
    /// Generate enhanced completion with dynamic features
    Enhanced {
        /// Shell type for enhanced completion
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, ValueEnum)]
pub enum Provider {
    Openai,
    Claude,
    /// OpenAI Codex - uses OAuth with ChatGPT Plus/Pro subscription
    #[value(name = "codex", alias = "openai-codex", alias = "openai_codex")]
    OpenaiCodex,
}

impl Provider {
    #[allow(dead_code)]
    pub fn new(s: &str) -> Provider {
        match s {
            "openai" => Provider::Openai,
            "openai-codex" | "openai_codex" | "codex" => Provider::OpenaiCodex,
            _ => Provider::Claude,
        }
    }

    #[allow(dead_code)]
    pub fn to_str(self) -> &'static str {
        match self {
            Provider::Openai => "openai",
            Provider::Claude => "claude",
            Provider::OpenaiCodex => "codex",
        }
    }
}

#[allow(dead_code)]
impl Args {
    pub fn is_redaction(&self) -> bool {
        matches!(self.command, Some(Commands::Redact { .. }))
            || self.redact_add.is_some()
            || self.redact_remove.is_some()
            || self.redact_list
    }

    pub fn is_chat_gpt_api_key(&self) -> bool {
        matches!(
            self.command,
            Some(Commands::Config {
                action: ConfigAction::SetOpenai { .. },
                ..
            })
        ) || self.chat_gpt_api_key.is_some()
    }

    pub fn is_claude_api_key(&self) -> bool {
        matches!(
            self.command,
            Some(Commands::Config {
                action: ConfigAction::SetClaude { .. },
                ..
            })
        ) || self.claude_api_key.is_some()
    }

    pub fn is_sessions_all(&self) -> bool {
        matches!(
            self.command,
            Some(Commands::Sessions {
                action: SessionAction::List { .. },
                ..
            })
        ) || self.sessions_all
    }

    pub fn is_session(&self) -> bool {
        self.session.is_some()
    }

    pub fn is_provider(&self) -> bool {
        matches!(
            self.command,
            Some(Commands::Config {
                action: ConfigAction::SetProvider { .. },
                ..
            })
        ) || self.provider.is_some()
    }

    #[allow(dead_code)]
    pub fn is_setup(&self) -> bool {
        matches!(self.command, Some(Commands::Setup(_)))
    }

    pub fn is_config_show(&self) -> bool {
        matches!(
            self.command,
            Some(Commands::Config {
                action: ConfigAction::Show,
                ..
            })
        ) || self.print_config
    }

    #[allow(dead_code)]
    pub fn should_handle_chat(&self) -> bool {
        match &self.command {
            Some(Commands::Chat(_)) => true,
            None => true, // Default to chat if no subcommand
            _ => false,
        }
    }

    pub fn get_chat_data(&self) -> Option<String> {
        match &self.command {
            Some(Commands::Chat(args)) => args.input.clone(),
            _ => self.data.clone(),
        }
    }

    pub fn get_chat_directory(&self) -> Option<String> {
        match &self.command {
            Some(Commands::Chat(args)) => args.directory.clone(),
            _ => self.directory.clone(),
        }
    }

    pub fn get_chat_directories(&self) -> Vec<String> {
        match &self.command {
            Some(Commands::Chat(args)) => args.directories.clone(),
            _ => self.directories.clone(),
        }
    }

    pub fn get_chat_exclude(&self) -> Vec<String> {
        match &self.command {
            Some(Commands::Chat(args)) => args.exclude.clone(),
            _ => self.exclude.clone(),
        }
    }

    pub fn get_chat_system_prompt(&self) -> Option<String> {
        match &self.command {
            Some(Commands::Chat(args)) => args.system_prompt.clone(),
            _ => self.system_prompt.clone(),
        }
    }

    pub fn get_chat_session(&self) -> Option<String> {
        match &self.command {
            Some(Commands::Chat(args)) => args.session.clone(),
            _ => self.session.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn get_smart_context(&self) -> bool {
        match &self.command {
            Some(Commands::Chat(args)) => args.smart_context,
            _ => self.smart_context,
        }
    }

    #[allow(dead_code)]
    pub fn get_max_context_tokens(&self) -> Option<usize> {
        match &self.command {
            Some(Commands::Chat(args)) => args.max_context_tokens,
            _ => self.max_context_tokens,
        }
    }

    #[allow(dead_code)]
    pub fn get_preview_context(&self) -> bool {
        match &self.command {
            Some(Commands::Chat(args)) => args.preview_context,
            _ => self.preview_context,
        }
    }

    #[allow(dead_code)]
    pub fn get_chunked_analysis(&self) -> bool {
        match &self.command {
            Some(Commands::Chat(args)) => args.chunked_analysis,
            _ => self.chunked_analysis,
        }
    }
}
