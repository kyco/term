/// Dedicated argument structures for each subcommand
/// Provides better type safety and organization compared to inline field definitions
use clap::Args;

/// Arguments for the setup wizard command
#[derive(Args, Debug, Clone)]
pub struct SetupArgs {
    /// Skip API key validation (for testing)
    #[arg(long, hide = true)]
    pub skip_validation: bool,

    /// Force reconfiguration even if already configured
    #[arg(long)]
    pub force: bool,

    /// Automatically accept defaults (non-interactive mode)
    #[arg(long)]
    pub auto_accept: bool,
}

/// Arguments for interactive chat sessions
#[derive(Args, Debug, Clone)]
pub struct ChatArgs {
    /// Initial input message for the chat session
    pub input: Option<String>,

    /// Local directory to include as context
    pub directory: Option<String>,

    /// Multiple directories to include as context
    #[arg(short = 'd', long, value_delimiter = ',')]
    pub directories: Vec<String>,

    /// Patterns to exclude from context
    #[arg(short, long, value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// System prompt to use for this session
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Session name to continue/create (continues previous chat history if session exists)
    #[arg(long, short = 's', alias = "continue", alias = "resume")]
    pub session: Option<String>,

    /// Resume the last/most recent session automatically
    #[arg(short = 'c', long = "last", conflicts_with = "session")]
    pub last_session: bool,

    /// Do not save this conversation (chats are saved by default)
    #[arg(long, conflicts_with_all = ["session", "last_session"])]
    pub temporary: bool,

    /// Enable smart context discovery (automatically finds relevant files)
    #[arg(long)]
    pub smart_context: bool,

    /// Query to guide smart context selection
    #[arg(long, requires = "smart_context")]
    pub context_query: Option<String>,

    /// Maximum tokens for smart context (default: 4000)
    #[arg(long, requires = "smart_context")]
    pub max_context_tokens: Option<usize>,

    /// Preview selected context before proceeding
    #[arg(long, requires = "smart_context")]
    pub preview_context: bool,

    /// Enable chunked analysis for large projects (breaks into manageable pieces)
    #[arg(long, requires = "smart_context")]
    pub chunked_analysis: bool,

    /// Chunking strategy: module, functional, token, hierarchical
    #[arg(long, requires = "chunked_analysis", default_value = "hierarchical")]
    pub chunk_strategy: String,
}

impl ChatArgs {
    /// Apply environment variable fallbacks to chat arguments
    pub fn with_env_fallbacks(self) -> Self {
        self
    }
}

/// Arguments for one-shot ask questions
#[derive(Args, Debug, Clone)]
pub struct AskArgs {
    /// The question to ask
    pub question: String,

    /// Local directory to include as context
    pub directory: Option<String>,

    /// Multiple directories to include as context  
    #[arg(short = 'd', long, value_delimiter = ',')]
    pub directories: Vec<String>,

    /// Patterns to exclude from context
    #[arg(short, long, value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// System prompt to use for this query
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Session name to save this interaction to
    #[arg(long)]
    pub session: Option<String>,

    /// Enable smart context discovery (automatically finds relevant files)
    #[arg(long)]
    pub smart_context: bool,

    /// Query to guide smart context selection
    #[arg(long, requires = "smart_context")]
    pub context_query: Option<String>,

    /// Maximum tokens for smart context (default: 4000)
    #[arg(long, requires = "smart_context")]
    pub max_context_tokens: Option<usize>,

    /// Preview selected context before proceeding
    #[arg(long, requires = "smart_context")]
    pub preview_context: bool,

    /// Enable chunked analysis for large projects (breaks into manageable pieces)
    #[arg(long, requires = "smart_context")]
    pub chunked_analysis: bool,

    /// Chunking strategy: module, functional, token, hierarchical
    #[arg(long, requires = "chunked_analysis", default_value = "hierarchical")]
    pub chunk_strategy: String,
}

impl AskArgs {
    /// Apply environment variable fallbacks to ask arguments
    pub fn with_env_fallbacks(self) -> Self {
        self
    }
}

/// Arguments for session management operations
#[derive(Args, Debug, Clone)]
pub struct SessionArgs {
    /// Show additional details in list view
    #[arg(long)]
    pub verbose: bool,

    /// Filter sessions by pattern (name matching)
    #[arg(long)]
    pub filter: Option<String>,

    /// Limit number of sessions shown in list
    #[arg(long)]
    pub limit: Option<usize>,

    /// Sort sessions by: name, date, messages
    #[arg(long, value_enum, default_value = "date")]
    pub sort: SessionSortOrder,
}

/// Arguments for configuration management
#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    /// Export configuration to file
    #[arg(long)]
    pub export: Option<String>,

    /// Import configuration from file
    #[arg(long)]
    pub import: Option<String>,

    /// Backup current configuration before changes
    #[arg(long)]
    pub backup: bool,

    /// Validate configuration without making changes
    #[arg(long)]
    pub validate: bool,
}

/// Arguments for redaction pattern management
#[derive(Args, Debug, Clone)]
pub struct RedactArgs {
    /// Show redaction statistics
    #[arg(long)]
    pub stats: bool,

    /// Test redaction patterns against input text
    #[arg(long)]
    pub test: Option<String>,

    /// Export redaction patterns to file
    #[arg(long)]
    pub export: Option<String>,

    /// Import redaction patterns from file
    #[arg(long)]
    pub import: Option<String>,
}

/// Arguments for shell completion generation
#[derive(Args, Debug, Clone)]
pub struct CompletionArgs {
    /// Output file for the completion script
    #[arg(short, long)]
    pub output: Option<String>,

    /// Show installation instructions
    #[arg(long)]
    pub show_install: bool,
}

/// Sort order for session listing
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SessionSortOrder {
    /// Sort by session name
    Name,
    /// Sort by creation/modification date
    Date,
    /// Sort by number of messages
    Messages,
}

/// Arguments for Git commit message generation
#[derive(Args, Debug, Clone)]
pub struct CommitArgs {
    /// Automatically commit with generated message (no confirmation)
    #[arg(short, long)]
    pub auto: bool,

    /// Force generation even if no staged changes
    #[arg(long)]
    pub force: bool,

    /// Use a specific commit message template
    #[arg(long)]
    pub template: Option<String>,

    /// Add all changes before committing
    #[arg(long)]
    pub add_all: bool,

    /// Commit message type (feat, fix, docs, etc.)
    #[arg(long)]
    pub message_type: Option<String>,

    /// Scope for the commit (optional)
    #[arg(long)]
    pub scope: Option<String>,
}

/// Arguments for Git branch analysis
#[derive(Args, Debug, Clone)]
pub struct BranchArgs {
    /// Branch name to analyze (defaults to current branch)
    pub branch: Option<String>,

    /// Generate release notes between tags
    #[arg(long)]
    pub release_notes: bool,

    /// From tag for release notes
    #[arg(long, requires = "release_notes")]
    pub from_tag: Option<String>,

    /// To tag for release notes (defaults to HEAD)
    #[arg(long, requires = "release_notes")]
    pub to_tag: Option<String>,

    /// Generate PR/MR description for the current branch
    #[arg(long)]
    pub pr_description: bool,

    /// Base branch to compare against for PR (defaults to main/master)
    #[arg(long, requires = "pr_description")]
    pub base_branch: Option<String>,

    /// Generate branch naming suggestions
    #[arg(long)]
    pub suggest_name: bool,

    /// Context for branch naming (what you're working on)
    #[arg(long, requires = "suggest_name")]
    pub context: Option<String>,
}

/// Arguments for Git hooks management
#[derive(Args, Debug, Clone)]
pub struct HooksArgs {
    /// Action to perform (status, install, install-all, uninstall)
    pub action: String,

    /// Hook type (pre-commit, commit-msg, pre-push, post-commit)
    pub hook_type: Option<String>,
}

/// Arguments for code review functionality
#[derive(Args, Debug, Clone)]
pub struct ReviewArgs {
    /// Review depth level
    #[arg(short, long, value_enum, default_value = "standard")]
    pub depth: ReviewDepth,

    /// Focus on specific file patterns
    #[arg(long)]
    pub files: Vec<String>,

    /// Include security analysis
    #[arg(long)]
    pub security: bool,

    /// Include performance analysis
    #[arg(long)]
    pub performance: bool,

    /// Output format for review results
    #[arg(long, value_enum, default_value = "text")]
    pub format: ReviewFormat,

    /// Save review results to file
    #[arg(long)]
    pub output: Option<String>,
}

/// Review depth options
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ReviewDepth {
    /// Quick surface-level review
    Quick,
    /// Standard thorough review
    Standard,
    /// Deep comprehensive review
    Deep,
}

/// Review output format options
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ReviewFormat {
    /// Human-readable text format
    Text,
    /// JSON format for tooling integration
    Json,
    /// Markdown format for documentation
    Markdown,
}

/// Arguments for Git stash management
#[derive(Args, Debug, Clone)]
pub struct StashArgs {
    /// Stash action to perform
    pub action: String,

    /// Stash name/message for push operations
    #[arg(long, short)]
    pub message: Option<String>,

    /// Include untracked files when stashing
    #[arg(long, short = 'u')]
    pub include_untracked: bool,

    /// Interactive stash selection
    #[arg(long, short)]
    pub interactive: bool,

    /// Specific stash index (for pop, apply, drop operations)
    pub stash_index: Option<usize>,

    /// Skip confirmation prompts and proceed as if confirmed (for scripting/CI)
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Arguments for Git tag and release management
#[derive(Args, Debug, Clone)]
pub struct TagArgs {
    /// Tag action to perform
    pub action: String,

    /// Tag name for create/delete operations
    pub tag_name: Option<String>,

    /// Tag message for annotated tags
    #[arg(long, short)]
    pub message: Option<String>,

    /// Create lightweight tag (non-annotated)
    #[arg(long)]
    pub lightweight: bool,

    /// Force tag creation (overwrite existing)
    #[arg(long, short)]
    pub force: bool,

    /// From tag for release notes comparison
    #[arg(long)]
    pub from_tag: Option<String>,

    /// To tag for release notes comparison  
    #[arg(long)]
    pub to_tag: Option<String>,

    /// Output format for release notes
    #[arg(long, value_enum, default_value = "markdown")]
    pub format: TagFormat,

    /// Skip confirmation prompts and proceed as if confirmed (for scripting/CI)
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Tag output format options
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum TagFormat {
    /// Markdown format for GitHub/GitLab
    Markdown,
    /// Plain text format
    Text,
    /// JSON format for automation
    Json,
}

/// Arguments for Git interactive rebase assistance
#[derive(Args, Debug, Clone)]
pub struct RebaseArgs {
    /// Rebase action to perform
    pub action: String,

    /// Target branch or commit for rebase
    pub target: Option<String>,

    /// Number of commits to include in interactive rebase
    #[arg(short, long)]
    pub count: Option<usize>,

    /// Continue interrupted rebase operation
    #[arg(long)]
    pub continue_rebase: bool,

    /// Abort current rebase operation
    #[arg(long)]
    pub abort: bool,

    /// Skip current commit in rebase
    #[arg(long)]
    pub skip: bool,

    /// Enable AI-powered commit message suggestions during rebase
    #[arg(long)]
    pub ai_suggestions: bool,

    /// Interactive mode with step-by-step guidance
    #[arg(long)]
    pub interactive: bool,

    /// Automatically squash fixup commits
    #[arg(long)]
    pub autosquash: bool,
}

/// Arguments for Git conflict resolution assistance
#[derive(Args, Debug, Clone)]
pub struct ConflictsArgs {
    /// Conflict resolution action to perform
    pub action: String,

    /// Specific file to analyze/resolve
    #[arg(short, long)]
    pub file: Option<String>,

    /// Show detailed analysis with AI insights
    #[arg(long)]
    pub detailed: bool,

    /// Auto-resolve simple conflicts where possible
    #[arg(long)]
    pub auto_resolve: bool,

    /// Preferred merge strategy (ours, theirs, manual)
    #[arg(long)]
    pub strategy: Option<String>,

    /// Interactive resolution with step-by-step guidance
    #[arg(long)]
    pub interactive: bool,

    /// Generate merge resolution documentation
    #[arg(long)]
    pub document: bool,
}

/// Arguments for preset and template management
#[derive(Args, Debug, Clone)]
pub struct PresetArgs {
    /// Preset action to perform
    #[command(subcommand)]
    pub action: PresetAction,
}

/// Preset management actions
#[derive(clap::Subcommand, Debug, Clone)]
pub enum PresetAction {
    /// List available presets
    List {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Search presets by name or description
        #[arg(long)]
        search: Option<String>,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },

    /// Use a preset interactively
    Use {
        /// Preset name to use
        name: String,

        /// Local directory to include as context
        directory: Option<String>,

        /// Multiple directories to include as context
        #[arg(short = 'd', long, value_delimiter = ',')]
        directories: Vec<String>,

        /// Enable smart context discovery
        #[arg(long)]
        smart_context: bool,

        /// Session name to use or create
        #[arg(long)]
        session: Option<String>,

        /// Skip variable prompting and use defaults
        #[arg(long)]
        use_defaults: bool,

        /// Preview template before execution
        #[arg(long)]
        preview: bool,

        /// Use with Git staged changes
        #[arg(long)]
        git_staged: bool,

        /// Specify variables directly (key=value format)
        #[arg(long, value_delimiter = ',')]
        variables: Vec<String>,
    },

    /// Create a new preset
    Create {
        /// Preset name
        name: String,

        /// Preset description
        #[arg(long)]
        description: Option<String>,

        /// Preset category
        #[arg(long)]
        category: Option<String>,

        /// Template content (if not provided, will prompt)
        #[arg(long)]
        template: Option<String>,

        /// Create from current session
        #[arg(long)]
        from_session: Option<String>,

        /// Open editor for template creation
        #[arg(long)]
        edit: bool,
    },

    /// Show detailed information about a preset
    Show {
        /// Preset name to show
        name: String,

        /// Show template content
        #[arg(long)]
        template: bool,

        /// Show usage statistics
        #[arg(long)]
        stats: bool,
    },

    /// Delete a preset (user presets only)
    Delete {
        /// Preset name to delete
        name: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },

    /// Export a preset to file
    Export {
        /// Preset name to export
        name: String,

        /// Output file path
        #[arg(long)]
        file: String,

        /// Export format
        #[arg(long, value_enum, default_value = "yaml")]
        format: PresetFormat,
    },

    /// Import a preset from file
    Import {
        /// Preset file path
        file: String,

        /// Force overwrite if preset already exists
        #[arg(long)]
        force: bool,
    },

    /// Edit an existing preset
    Edit {
        /// Preset name to edit
        name: String,

        /// Open editor for template modification
        #[arg(long)]
        template: bool,

        /// Edit preset metadata
        #[arg(long)]
        metadata: bool,
    },

    /// Search presets by name, description, or content
    Search {
        /// Search query
        query: String,

        /// Search in template content too
        #[arg(long)]
        content: bool,

        /// Filter by category
        #[arg(long)]
        category: Option<String>,
    },
}

/// Preset export/import format options
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum PresetFormat {
    /// YAML format (default)
    Yaml,
    /// JSON format
    Json,
}
