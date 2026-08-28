/// Handler for the Chat command - interactive conversation mode
/// This delegates to the existing InteractiveSession implementation
use crate::args::ChatArgs;
use crate::chat::InteractiveSession;
use crate::path::extract::extract_content;
use crate::path::model::Files;
use crate::preset::builtin::BuiltinPresets;
use crate::repository::db::SqliteRepository;
use crate::session::model::session::Session;
use crate::session::repository::SessionRepository;
use crate::session::service::sessions_service;
use anyhow::Result;
use chrono::Local;
use colored::*;

/// Handle the chat command for interactive conversations
pub async fn handle_chat_command(args: &ChatArgs, repo: &SqliteRepository) -> Result<()> {
    // Apply environment variable fallbacks
    let args = args.clone().with_env_fallbacks();

    let input = &args.input;
    let session_name = &args.session;
    let last_session = args.last_session;
    let directory = &args.directory;
    let directories = &args.directories;
    let exclude = &args.exclude;

    // Extract context files if directory is provided
    let context_files = if directory.is_some() || !directories.is_empty() {
        extract_content(directory, directories, exclude).unwrap_or_default()
    } else {
        Vec::<Files>::new()
    };

    // Get or create session.
    //
    // A bare `termai chat` used to build a throwaway session that was never
    // written to the database: the conversation only existed in memory, and a
    // dropped connection took all of it. Chats are saved by default now, and
    // `--temporary` is the opt-in for a conversation you do not want kept.
    let session = if last_session {
        let session = sessions_service::get_most_recent_session(repo, repo)?;
        println!(
            "🔄 {} '{}'",
            "Resuming".bright_green(),
            session.name.bright_cyan()
        );
        replay_recent(&session);
        session
    } else if let Some(name) = session_name {
        // Warn *before* creating it: a mistyped name used to silently produce
        // a new empty session, which looks identical to a lost conversation.
        let suggestions = if repo.fetch_session_by_name(name).is_err() {
            sessions_service::suggest_similar_sessions(repo, name)
        } else {
            Vec::new()
        };

        let session = sessions_service::session(repo, repo, name)?;
        if session.messages.is_empty() {
            println!(
                "🆕 {} '{}'",
                "New session".bright_green(),
                name.bright_cyan()
            );
            if !suggestions.is_empty() {
                println!(
                    "   {} {}",
                    "Did you mean:".bright_yellow(),
                    suggestions.join(", ").bright_cyan()
                );
                println!(
                    "   {}",
                    "Exit and re-run with the right name if so — this one is empty.".dimmed()
                );
            }
        } else {
            println!(
                "📝 {} '{}'",
                "Continuing".bright_green(),
                name.bright_cyan()
            );
            replay_recent(&session);
        }
        session
    } else if args.temporary {
        println!(
            "⚠️  {} — nothing from this chat will be saved.",
            "Temporary chat".bright_yellow()
        );
        println!(
            "   {}",
            "Use /save <name> at any point to keep it.".dimmed()
        );
        println!();
        Session::new_temporary()
    } else {
        let name = format!("chat-{}", Local::now().format("%Y%m%d-%H%M%S"));
        let session = sessions_service::session(repo, repo, &name)?;
        println!(
            "💾 {} '{}' {}",
            "Saving to".bright_green(),
            session.name.bright_cyan(),
            "(renamed from your first message)".dimmed()
        );
        println!();
        session
    };

    // Anything typed but never answered — a dropped connection, a closed
    // terminal — is handed back rather than quietly discarded.
    let unsent = sessions_service::recover_unsent_messages(repo, &session);
    if !unsent.is_empty() {
        println!(
            "{} {}",
            "↩".bright_yellow(),
            format!(
                "{} message(s) from an interrupted turn were recovered — see /unsent",
                unsent.len()
            )
            .bright_yellow()
        );
        println!();
    }

    // Show preset suggestions before starting interactive session
    show_preset_suggestions(
        &context_files,
        directory.is_some() || !directories.is_empty(),
    )
    .await?;

    // Create interactive session
    let mut interactive_session =
        InteractiveSession::new(repo, repo, repo, repo, session, context_files)?
            .with_initial_input(input.clone());

    // Start the interactive session
    interactive_session.run().await
}

/// Print the tail of a resumed conversation.
///
/// Dumping every stored message meant a long session buried the prompt under
/// hundreds of lines before the user could type anything.
fn replay_recent(session: &Session) {
    const SHOWN: usize = 6;

    if session.messages.is_empty() {
        return;
    }

    let total = session.messages.len();
    let skipped = total.saturating_sub(SHOWN);
    println!(
        "   {} {}",
        total.to_string().bright_yellow(),
        "messages in this conversation".dimmed()
    );
    if skipped > 0 {
        println!("   {}", format!("… {} earlier messages", skipped).dimmed());
    }
    println!();

    for message in session.messages.iter().skip(skipped) {
        let role_display = match message.role.to_string().as_str() {
            "user" => "you".bright_green().bold(),
            "assistant" => "ai ".bright_magenta().bold(),
            "system" => "sys".bright_yellow().bold(),
            other => other.white().bold(),
        };
        let preview: String = message
            .content
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
            .chars()
            .take(140)
            .collect();
        let ellipsis = if message.content.chars().count() > preview.chars().count() {
            "…"
        } else {
            ""
        };
        println!("  {} › {}{}", role_display, preview.dimmed(), ellipsis.dimmed());
    }
    println!();
}

/// Show preset suggestions based on context
async fn show_preset_suggestions(files: &[Files], has_directory_context: bool) -> Result<()> {
    if files.is_empty() && !has_directory_context {
        return Ok(());
    }

    // Analyze context to suggest relevant presets
    let suggested_presets = suggest_presets_for_context(files).await?;

    if suggested_presets.is_empty() {
        return Ok(());
    }

    println!("{}", "💡 Preset Suggestions".bright_yellow().bold());
    println!("{}", "═".repeat(20).dimmed());
    println!("Based on your context, these presets might be helpful:");
    println!();

    for (preset_name, reason) in suggested_presets {
        println!(
            "  {} {} - {}",
            "📦".bright_blue(),
            preset_name.bright_green().bold(),
            reason.dimmed()
        );
    }

    println!();
    println!(
        "{} Use a preset: {}",
        "🚀".bright_green(),
        "termai preset use \"<name>\"".bright_cyan()
    );
    println!("{} {} {}", "💡".bright_yellow(), "Tip:".bold(), "You can also use presets with your current context via --smart-context or --git-staged flags".dimmed());
    println!();

    Ok(())
}

/// Suggest presets based on file context analysis
async fn suggest_presets_for_context(files: &[Files]) -> Result<Vec<(String, String)>> {
    if files.is_empty() {
        return Ok(vec![]);
    }

    let mut suggestions = Vec::new();
    let builtin_presets = BuiltinPresets::get_all();

    // Analyze file extensions and content patterns
    let mut has_code = false;
    let mut has_tests = false;
    let mut _has_config = false;
    let mut has_docs = false;
    let mut has_errors_or_logs = false;
    let mut languages = std::collections::HashSet::new();

    for file in files {
        let path = std::path::Path::new(&file.path);

        // Analyze file extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "rs" | "js" | "ts" | "py" | "go" | "java" | "kt" | "cpp" | "c" | "cs" | "php"
                | "rb" | "swift" => {
                    has_code = true;
                    languages.insert(ext.to_string());
                }
                "yaml" | "yml" | "toml" | "json" | "cfg" | "conf" | "ini" => _has_config = true,
                "md" | "txt" | "rst" => has_docs = true,
                _ => {}
            }
        }

        // Analyze filename patterns
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if filename.contains("test")
            || filename.contains("spec")
            || path.to_string_lossy().contains("/tests/")
        {
            has_tests = true;
        }

        // Analyze file content patterns
        let content_lower = file.content.to_lowercase();
        if content_lower.contains("error")
            || content_lower.contains("exception")
            || content_lower.contains("failed")
            || content_lower.contains("bug")
            || content_lower.contains("issue")
            || content_lower.contains("panic")
        {
            has_errors_or_logs = true;
        }
    }

    // Generate suggestions based on analysis
    if has_code {
        for preset in &builtin_presets {
            match preset.name.as_str() {
                "Code Review Assistant" => {
                    suggestions.push((
                        preset.name.clone(),
                        "Perfect for reviewing code changes".to_string(),
                    ));
                }
                "Refactoring Assistant" => {
                    suggestions.push((
                        preset.name.clone(),
                        "Helps improve code structure and quality".to_string(),
                    ));
                }
                _ => {}
            }
        }
    }

    if has_tests || has_code {
        for preset in &builtin_presets {
            if preset.name == "Test Generator" {
                let reason = if has_tests {
                    "Can help improve existing tests"
                } else {
                    "Generate tests for your code"
                };
                suggestions.push((preset.name.clone(), reason.to_string()));
                break;
            }
        }
    }

    if has_errors_or_logs {
        for preset in &builtin_presets {
            if preset.name == "Debugging Assistant" {
                suggestions.push((
                    preset.name.clone(),
                    "Ideal for analyzing errors and debugging issues".to_string(),
                ));
                break;
            }
        }
    }

    if has_code && !has_docs {
        for preset in &builtin_presets {
            if preset.name == "Documentation Generator" {
                suggestions.push((
                    preset.name.clone(),
                    "Generate documentation for your code".to_string(),
                ));
                break;
            }
        }
    }

    // Limit to top 3 suggestions to avoid overwhelming users
    suggestions.truncate(3);

    Ok(suggestions)
}
