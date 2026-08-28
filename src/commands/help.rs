/// Contextual help system providing usage examples and tips for each subcommand
use colored::*;

#[allow(dead_code)]
pub struct HelpSystem;

#[allow(dead_code)]
impl HelpSystem {
    /// Show contextual help for the setup command
    pub fn show_setup_help() {
        println!("{}", "🚀 Setup Command Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Interactive setup wizard to configure TermAI with AI providers");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!("   {}", "termai setup".cyan());
        println!();
        println!("{}", "WHAT IT DOES:".bright_yellow().bold());
        println!("   • Guides you through API key configuration");
        println!("   • Validates API keys with live API calls");
        println!("   • Sets up default provider preferences");
        println!("   • Handles both Claude and OpenAI configuration");
        println!();
        println!("{}", "EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}                    # Run the setup wizard",
            "termai setup".cyan()
        );
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Have your API keys ready before running");
        println!("   • Ensure stable internet connection for validation");
        println!("   • You can reconfigure anytime by running setup again");
        println!();
    }

    /// Show contextual help for the config command
    pub fn show_config_help() {
        println!("{}", "⚙️  Configuration Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Manage TermAI configuration settings");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!("   {}", "termai config <COMMAND>".cyan());
        println!();
        println!("{}", "COMMANDS:".bright_yellow().bold());
        println!(
            "   {}          Show current configuration",
            "show".bright_white().bold()
        );
        println!(
            "   {}          Edit user or active project config",
            "edit".bright_white().bold()
        );
        println!(
            "   {}      Validate the active project config",
            "validate".bright_white().bold()
        );
        println!(
            "   {}           Create .termai.toml in the repo root",
            "init".bright_white().bold()
        );
        println!(
            "   {}        Import legacy DB defaults into config.toml",
            "migrate".bright_white().bold()
        );
        println!();
        println!("{}", "EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}                 # View current settings",
            "termai config show".cyan()
        );
        println!(
            "   {}                 # Edit your settings",
            "termai config edit".cyan()
        );
        println!(
            "   {}             # Create project config",
            "termai config init --project-type rust".cyan()
        );
        println!(
            "   {}             # Validate project config",
            "termai config validate".cyan()
        );
        println!(
            "   {}             # Migrate legacy defaults",
            "termai config migrate".cyan()
        );
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Durable defaults live in ~/.config/termai/config.toml");
        println!("   • Repo-specific context/privacy rules live in .termai.toml");
        println!("   • Secrets and Codex OAuth stay in the auth/state store");
        println!("   • Use 'termai auth' for provider credential management");
        println!();
    }

    /// Show contextual help for the auth command
    pub fn show_auth_help() {
        println!("{}", "🔐 Authentication Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Manage provider authentication from one place");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!("   {}", "termai auth <COMMAND> <PROVIDER>".cyan());
        println!();
        println!("{}", "COMMANDS:".bright_yellow().bold());
        println!(
            "   {}  Start authentication for a provider",
            "login".bright_white().bold()
        );
        println!(
            "   {}  Clear or revoke provider authentication",
            "logout".bright_white().bold()
        );
        println!(
            "   {}  Show the current authentication state",
            "status".bright_white().bold()
        );
        println!();
        println!("{}", "PROVIDERS:".bright_yellow().bold());
        println!(
            "   {}       Anthropic Claude API keys",
            "claude".bright_white().bold()
        );
        println!(
            "   {}        OpenAI API keys",
            "openai".bright_white().bold()
        );
        println!(
            "   {}          OpenAI Codex OAuth",
            "codex".bright_white().bold()
        );
        println!();
        println!("{}", "EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}               # Check Codex state",
            "termai auth status codex".cyan()
        );
        println!(
            "   {}               # Check OpenAI key state",
            "termai auth status openai".cyan()
        );
        println!(
            "   {}               # Check Claude key state",
            "termai auth status claude".cyan()
        );
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Codex uses OAuth; Claude and OpenAI use stored API keys");
        println!("   • Use config commands if you need to update raw stored keys");
        println!("   • Hidden config auth shims remain for compatibility");
        println!();
    }

    /// Show contextual help for the ask command
    pub fn show_ask_help() {
        println!("{}", "❓ Ask Command Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Ask a one-shot question with optional context (currently in development)");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!(
            "   {}",
            "termai ask [OPTIONS] <QUESTION> [DIRECTORY]".cyan()
        );
        println!();
        println!("{}", "OPTIONS:".bright_yellow().bold());
        println!(
            "   {}      Include specific directories as context",
            "-d, --directories <DIRS>".bright_white().bold()
        );
        println!(
            "   {}          Exclude files matching patterns",
            "-e, --exclude <PATTERNS>".bright_white().bold()
        );
        println!(
            "   {}        Enable smart context discovery",
            "--smart-context".bright_white().bold()
        );
        println!(
            "   {}    Preview context before proceeding",
            "--preview-context".bright_white().bold()
        );
        println!(
            "   {}      Enable chunked analysis",
            "--chunked-analysis".bright_white().bold()
        );
        println!();
        println!("{}", "PLANNED EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}             # Simple question",
            "termai ask \"What is Rust?\"".cyan()
        );
        println!(
            "   {}    # Ask about specific code",
            "termai ask \"Explain this\" src/".cyan()
        );
        println!(
            "   {} # Smart context",
            "termai ask --smart-context \"Debug this error\"".cyan()
        );
        println!(
            "   {}   # Multiple directories",
            "termai ask -d src/ -d tests/ \"Review code\"".cyan()
        );
        println!();
        println!("{}", "STATUS:".bright_red().bold());
        println!("   🚧 This command is under active development");
        println!("   💡 For now, please use: {}", "termai chat".cyan());
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Quote questions containing spaces or special characters");
        println!("   • Use --preview-context to see what files will be included");
        println!("   • Smart context automatically finds relevant files");
        println!();
    }

    /// Show contextual help for the chat command
    pub fn show_chat_help() {
        println!("{}", "💬 Chat Command Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Start an interactive chat session with AI");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!("   {}", "termai chat [OPTIONS] [DIRECTORY]".cyan());
        println!();
        println!("{}", "OPTIONS:".bright_yellow().bold());
        println!(
            "   {}        Use specific session name",
            "--session <NAME>".bright_white().bold()
        );
        println!(
            "   {}      Include directories as context",
            "-d, --directories <DIRS>".bright_white().bold()
        );
        println!(
            "   {}          Exclude files matching patterns",
            "-e, --exclude <PATTERNS>".bright_white().bold()
        );
        println!(
            "   {}        Enable smart context discovery",
            "--smart-context".bright_white().bold()
        );
        println!(
            "   {}    Preview context before proceeding",
            "--preview-context".bright_white().bold()
        );
        println!();
        println!("{}", "EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}                      # Start new session",
            "termai chat".cyan()
        );
        println!(
            "   {}            # Use specific session",
            "termai chat --session debug".cyan()
        );
        println!(
            "   {}                 # Include src/ as context",
            "termai chat src/".cyan()
        );
        println!(
            "   {}        # Smart context discovery",
            "termai chat --smart-context".cyan()
        );
        println!(
            "   {}    # Multiple directories",
            "termai chat -d src/ -d docs/".cyan()
        );
        println!();
        println!("{}", "INTERACTIVE COMMANDS:".bright_yellow().bold());
        println!(
            "   {}                  Exit the session",
            "/exit".bright_white().bold()
        );
        println!(
            "   {}                  Show help",
            "/help".bright_white().bold()
        );
        println!(
            "   {}                Clear conversation",
            "/clear".bright_white().bold()
        );
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Sessions are automatically saved and can be resumed");
        println!("   • Use smart context for automatic relevant file discovery");
        println!("   • Include multiple directories with -d flag");
        println!("   • Preview context with --preview-context flag");
        println!();
    }

    /// Show contextual help for the session command
    pub fn show_session_help() {
        println!("{}", "📋 Session Command Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Manage chat sessions and conversation history");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!("   {}", "termai sessions <COMMAND>".cyan());
        println!();
        println!("{}", "COMMANDS:".bright_yellow().bold());
        println!(
            "   {}    List all available sessions",
            "list".bright_white().bold()
        );
        println!(
            "   {} Delete a specific session",
            "delete <name>".bright_white().bold()
        );
        println!(
            "   {}   Show detailed session info",
            "show <name>".bright_white().bold()
        );
        println!();
        println!("{}", "EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}                   # List all sessions",
            "termai sessions list".cyan()
        );
        println!(
            "   {}        # Delete specific session",
            "termai sessions delete debug".cyan()
        );
        println!(
            "   {}          # Show session details",
            "termai sessions show debug".cyan()
        );
        println!();
        println!("{}", "SESSION INFO INCLUDES:".bright_yellow().bold());
        println!("   • Session name and unique ID");
        println!("   • Creation and expiration dates");
        println!("   • Message count and history");
        println!("   • Current/temporary status");
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Sessions are kept until you delete them — nothing expires");
        println!("   • Use descriptive names for better organization");
        println!("   • Show command displays full message history");
        println!("   • Deleted sessions cannot be recovered");
        println!();
    }

    /// Show contextual help for the redact command
    pub fn show_redact_help() {
        println!("{}", "🔒 Redaction Command Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "DESCRIPTION:".bright_yellow().bold());
        println!("   Manage redaction patterns for privacy protection");
        println!();
        println!("{}", "USAGE:".bright_yellow().bold());
        println!("   {}", "termai redact <COMMAND>".cyan());
        println!();
        println!("{}", "COMMANDS:".bright_yellow().bold());
        println!(
            "   {}      Add a new redaction pattern",
            "add <pattern>".bright_white().bold()
        );
        println!(
            "   {}   Remove a redaction pattern",
            "remove <pattern>".bright_white().bold()
        );
        println!(
            "   {}      List all redaction patterns",
            "list".bright_white().bold()
        );
        println!();
        println!("{}", "EXAMPLES:".bright_yellow().bold());
        println!(
            "   {}              # Add email redaction",
            "termai redact add myemail@domain.com".cyan()
        );
        println!(
            "   {}                 # Add name redaction",
            "termai redact add \"John Smith\"".cyan()
        );
        println!(
            "   {}           # Remove pattern",
            "termai redact remove myemail@domain.com".cyan()
        );
        println!(
            "   {}                    # List all patterns",
            "termai redact list".cyan()
        );
        println!();
        println!("{}", "HOW REDACTION WORKS:".bright_yellow().bold());
        println!("   • Patterns are replaced with [REDACTED] in AI requests");
        println!("   • Case-insensitive matching by default");
        println!("   • Protects sensitive information from being sent to AI");
        println!("   • Applied to both context files and user messages");
        println!();
        println!("{}", "TIPS:".bright_green().bold());
        println!("   • Add personal information like names, emails, addresses");
        println!("   • Use quotes for patterns with spaces");
        println!("   • Review patterns regularly to ensure proper coverage");
        println!("   • Test redaction with preview-context flag");
        println!();
    }

    /// Show general help with all available commands
    pub fn show_general_help() {
        println!("{}", "🤖 TermAI Command Help".bright_cyan().bold());
        println!("{}", "══════════════════════".white().dimmed());
        println!();
        println!("{}", "AVAILABLE COMMANDS:".bright_yellow().bold());
        println!();
        println!(
            "   {}     Interactive setup wizard",
            "setup".bright_white().bold()
        );
        println!(
            "   {}      Manage configuration settings",
            "config".bright_white().bold()
        );
        println!(
            "   {}      Manage provider authentication",
            "auth".bright_white().bold()
        );
        println!(
            "   {}     Manage redaction patterns",
            "redact".bright_white().bold()
        );
        println!(
            "   {}   Manage chat sessions",
            "session".bright_white().bold()
        );
        println!(
            "   {}       Ask one-shot questions (in development)",
            "ask".bright_white().bold()
        );
        println!(
            "   {}       Start interactive chat",
            "chat".bright_white().bold()
        );
        println!();
        println!("{}", "GET DETAILED HELP:".bright_yellow().bold());
        println!(
            "   {}          # Detailed command help",
            "termai <command> --help".cyan()
        );
        println!(
            "   {}     # Setup wizard guidance",
            "termai setup --help".cyan()
        );
        println!(
            "   {}     # Configuration help",
            "termai config --help".cyan()
        );
        println!(
            "   {}         # Authentication help",
            "termai auth --help".cyan()
        );
        println!(
            "   {}       # Chat session help",
            "termai chat --help".cyan()
        );
        println!();
        println!("{}", "QUICK START:".bright_yellow().bold());
        println!(
            "   1. {}               # Configure API keys",
            "termai setup".cyan()
        );
        println!(
            "   2. {}         # View configuration",
            "termai config show".cyan()
        );
        println!(
            "   3. {}                # Start chatting",
            "termai chat".cyan()
        );
        println!();
        println!("{}", "NEED MORE HELP?".bright_green().bold());
        println!("   • Check the README for detailed documentation");
        println!("   • Run any command with --help for specific options");
        println!("   • Use 'termai config show' to debug configuration issues");
        println!();
    }
}
