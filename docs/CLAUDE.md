# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TermAI is a terminal-based AI assistant built in Rust that supports both OpenAI and Claude APIs. It provides privacy-focused chat capabilities with local context awareness, session management, and redaction features.

## Development Commands

### Build and Testing
- `cargo build --release` - Build the project in release mode
- `cargo test` - Run the test suite
- `cargo run --release -- [args]` - Run the application with arguments
- `just build` - Alternative build command using justfile
- `just test` - Run tests via justfile
- `just run [args]` - Run with arguments via justfile

### Code Quality
- `cargo fmt` - Format code
- `cargo clippy -- -D warnings` - Run clippy linter with warnings as errors
- `just fmt` - Format code via justfile  
- `just clippy` - Run clippy via justfile

### Dependencies
- `cargo update` - Update dependencies
- `just update-deps` - Update dependencies via justfile
- `just outdated` - Check for outdated dependencies

### Documentation
- `cargo doc --open` - Generate and open documentation
- `just doc` - Generate documentation via justfile

## User Commands

TermAI uses an intuitive subcommand structure for better discoverability and organization:

### Setup and Configuration
- `termai setup` - Interactive setup wizard (recommended for first-time setup)
- `termai config show` - Display current configuration with visual status
- `termai auth login <provider>` - Authenticate a provider (claude | openai | codex)
- `termai auth status <provider>` - Show authentication status
- `termai auth logout <provider>` - Remove stored credentials
- `termai config set-model [model]` - Set default model (interactive if omitted)
- `termai config list-models` - List available models
- (Legacy, hidden but still working: `config set-claude/set-openai/set-provider/reset/env`)

### One-Shot Questions
- `termai ask "your question"` - Quick question without starting interactive session
- `termai ask "question" src/` - Ask with specific directory context
- `termai ask -d src/ -d docs/ "question"` - Multiple directories as context
- `termai ask --session mywork "question"` - Save question to named session
- `termai ask --smart-context "question"` - Use smart context discovery
- `termai ask --preview-context "question" src/` - Preview context before sending

### Interactive Chat
- `termai chat` - Start interactive chat session
- `termai chat --session mywork` - Continue specific session
- `termai chat src/` - Start chat with directory context
- `termai chat --smart-context` - Enable smart context discovery
- `termai chat -d src/ -d tests/` - Multiple directories as context

### Session Management
- `termai sessions list` - List all saved sessions with details
- `termai sessions show <name>` - View session details and message history
- `termai sessions delete <name>` - Delete session (with confirmation)

### Privacy and Redaction
- `termai redact add "pattern"` - Add pattern to be redacted (e.g., email, name)
- `termai redact remove "pattern"` - Remove redaction pattern
- `termai redact list` - List all active redaction patterns with usage info

### Shell Completion
- `termai completion bash` - Generate Bash completion script
- `termai completion zsh` - Generate Zsh completion script  
- `termai completion fish` - Generate Fish completion script
- `termai completion powershell` - Generate PowerShell completion script

### Quick Examples
```bash
# First time setup
termai setup

# Quick questions
termai ask "What is Rust?"
termai ask "Explain this code" src/main.rs

# Interactive sessions
termai chat
termai chat --session debugging

# With context
termai ask "Review this function" -d src/commands/
termai chat --session review src/

# Privacy protection  
termai redact add "mycompany@example.com"
termai ask "analyze logs" --preview-context logs/
```

## Architecture

TermAI follows a layered architecture with clear separation of concerns:

### Core Layers
- **main.rs**: Entry point, argument parsing, and orchestration
- **Repository Layer**: Data access abstraction using SQLite (repository/db.rs)
- **Service Layer**: Business logic for config, sessions, and LLM interactions
- **Adapter Layer**: API integrations for OpenAI and Claude

### Key Modules

#### LLM Integration (`src/llm/`)
- **claude/**: Claude API adapter, models, and chat service
- **openai/**: OpenAI API adapter, models, and chat service  
- **common/**: Shared models and constants for both providers

#### Configuration Management (`src/config/`)
- **service/**: Provider configuration, API keys, redaction settings
- **repository/**: Config data access layer
- **entity/**: Config data models

#### Session Management (`src/session/`)
- **service/**: Session and message management
- **repository/**: Session persistence layer
- **entity/**: Session and message entities

#### Command Structure (`src/commands/`)
- **mod.rs**: Command dispatcher with enhanced error handling and routing
- **ask.rs**: One-shot question handler with full LLM integration
- **chat.rs**: Interactive chat session handler
- **config.rs**: Configuration and redaction management with enhanced UX
- **session.rs**: Session management (list, show, delete) with confirmations
- **setup.rs**: Setup wizard delegation to existing implementation
- **completion.rs**: Shell completion script generation
- **help.rs**: Contextual help system with examples and guidance

#### Supporting Modules
- **path/**: File content extraction for local context
- **redactions/**: Privacy protection through text redaction
- **output/**: Message formatting and display
- **ui/**: User interface components (thinking timer)

### Data Flow
1. **Command Parsing**: Arguments parsed via clap with subcommand routing (args.rs)
2. **Repository Initialization**: SQLite repository initialized for persistence
3. **Command Dispatch**: Commands routed to appropriate handlers with enhanced error handling
4. **Configuration Loading**: API keys, provider preferences, and redaction patterns loaded
5. **Context Extraction**: Local context extracted from specified files/directories (if requested)
6. **Input Processing**: User input processed and redacted for privacy
7. **LLM Integration**: Provider called (Claude or OpenAI) via service layer
8. **Response Processing**: Response formatted and displayed with enhanced UX
9. **Persistence**: The user's prompt is written before the provider call; the turn is stored when it completes. Only `--temporary` sessions skip this.

### Key Design Patterns
- **Command Pattern**: Subcommand structure with dedicated handlers for each operation
- **Repository Pattern**: Data access through traits (ConfigRepository, SessionRepository, MessageRepository)
- **Adapter Pattern**: LLM provider abstraction in claude/ and openai/ modules
- **Service Layer**: Business logic separation from data and presentation layers
- **Enhanced Error Handling**: Context-aware error messages with actionable guidance

## Database
- SQLite database stored in `~/.config/termai/app.db`
- Handles configuration, sessions, messages, and redaction patterns
- Repository pattern provides abstraction over direct database access