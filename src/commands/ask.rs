/// Handler for the Ask command - one-shot questions with optional context
/// Integrates with existing LLM infrastructure for direct question answering
use crate::args::AskArgs;
use crate::config::model::keys::ConfigKeys;
use crate::config::service::config_service;
use crate::config::settings::{ResolvedSettings, SettingsOverrides, SettingsProvider};
use crate::llm::common::constants::SYSTEM_PROMPT;
use crate::llm::common::model::role::Role;
use crate::path::extract::extract_content;
use crate::path::model::Files;
use crate::repository::db::SqliteRepository;
use crate::session::model::message::Message;
use crate::session::model::session::Session;
use crate::session::service::sessions_service;
use anyhow::{Context, Result};
use colored::*;
use uuid::Uuid;

/// Handle the ask command for one-shot questions
pub async fn handle_ask_command(args: &AskArgs, repo: &SqliteRepository) -> Result<()> {
    let args = args.clone().with_env_fallbacks();
    let settings = ResolvedSettings::load_for_current_dir_with_repo(
        repo,
        SettingsOverrides {
            smart_context: args.smart_context.then_some(true),
            token_budget: args.max_context_tokens,
            ..Default::default()
        },
    )?;

    let question = &args.question;
    let directory = &args.directory;
    let directories = &args.directories;
    let exclude = &args.exclude;
    let session_name = &args.session;
    let system_prompt = &args.system_prompt;
    let smart_context = settings.smart_context;
    let selected_model = settings.selected_model();

    println!("{}", "🤖 Ask Command".bright_cyan().bold());
    println!("{}", "═════════════".white().dimmed());
    println!();
    println!(
        "{}  {}",
        "Question:".bright_green().bold(),
        question.bright_white()
    );

    // Extract context files if directory is provided
    let mut context_files = Vec::<Files>::new();
    let mut context_included = false;

    if directory.is_some() || !directories.is_empty() {
        println!();
        println!("{}", "🔍 Extracting context...".bright_yellow());

        context_files = extract_content(directory, directories, exclude)
            .context("Failed to extract context from specified directories")?;

        if !context_files.is_empty() {
            context_included = true;
            println!("   📁 {} file(s) included as context", context_files.len());
            for file in &context_files {
                println!("   • {}", file.path.bright_blue());
            }
        }
    }

    // Handle smart context discovery
    if smart_context {
        println!();
        println!(
            "{}",
            "⚠️  Smart context discovery is not yet fully implemented".yellow()
        );
        println!("💡 Use directory flags (-d) to include specific files as context");
    }

    // Prepare the system prompt
    let effective_system_prompt = system_prompt
        .clone()
        .unwrap_or_else(|| SYSTEM_PROMPT.to_string());

    // Create messages for the request
    let mut messages = vec![Message::new(
        Uuid::new_v4().to_string(),
        Role::System,
        effective_system_prompt,
    )];

    // Add context if available
    if context_included && !context_files.is_empty() {
        let mut context_content = String::new();
        context_content.push_str("Here is the relevant code context:\n\n");

        for file in &context_files {
            context_content.push_str(&format!("# File: {}\n", file.path));
            context_content.push_str("```\n");
            context_content.push_str(&file.content);
            context_content.push_str("\n```\n\n");
        }

        context_content
            .push_str("Please answer the following question with reference to this context.\n\n");

        messages.push(Message::new(
            Uuid::new_v4().to_string(),
            Role::User,
            context_content,
        ));
    }

    // Add the main question
    messages.push(Message::new(
        Uuid::new_v4().to_string(),
        Role::User,
        question.to_string(),
    ));

    println!();
    println!("{}", "💭 Processing your question...".bright_yellow());

    // Get or create session if specified
    let mut session = if let Some(name) = session_name {
        println!("   📋 Saving to session: {}", name.bright_cyan());
        sessions_service::session(repo, repo, name)?
    } else {
        Session::new_temporary()
    };

    // Continue the stored conversation instead of starting from scratch every
    // time: `--session` promised continuity and only ever delivered a label.
    let mut request_messages = session.messages.clone();
    request_messages.extend(messages.iter().cloned());

    // Record the new turns on the session with empty ids so they are picked up
    // as unsaved, then write the question down before the request goes out.
    for message in &messages {
        session.add_raw_message(message.content.clone(), message.role.clone());
    }
    if let Err(e) = sessions_service::write_ahead_user_message(repo, repo, &mut session) {
        eprintln!("{} {}", "⚠".yellow(), format!("Could not save your question: {}", e).yellow());
    }

    // Route to appropriate LLM based on provider
    let response = match settings.default_provider {
        SettingsProvider::Claude => call_claude_api(repo, request_messages, &selected_model)
            .await
            .context("Failed to get response from Claude API")?,
        SettingsProvider::Openai => call_openai_api(repo, request_messages, &selected_model)
            .await
            .context("Failed to get response from OpenAI API")?,
        SettingsProvider::Codex => call_codex_api(repo, request_messages, &selected_model)
            .await
            .context("Failed to get response from Codex API")?,
    };

    // Display the response
    println!();
    println!("{}", "🤖 Response:".bright_green().bold());
    println!("{}", "═══════════".white().dimmed());
    println!();
    println!("{}", response.bright_white());

    // Save to session if specified
    if let Some(name) = session_name {
        session.add_raw_message(response.clone(), Role::Assistant);
        sessions_service::persist_session(repo, repo, &mut session)
            .context("Failed to save the conversation")?;

        // Report what is actually stored, not what we hoped would be.
        let stored = sessions_service::stored_message_count(repo, &session)?;
        println!();
        if stored > 0 {
            println!(
                "{}",
                format!("💾 Conversation saved to session: {} ({} messages)", name, stored).green()
            );
            println!(
                "   {}",
                format!("Continue with: termai chat --session {}", name).cyan()
            );
        } else {
            println!(
                "{}",
                format!("⚠ Nothing reached the database — session '{}' was NOT saved.", name).red()
            );
        }
    }

    println!();
    println!("{}", "💡 Quick actions:".bright_yellow().bold());
    println!(
        "   {}                      # Start interactive chat",
        "termai chat".cyan()
    );
    println!(
        "   {}         # View configuration",
        "termai config show".cyan()
    );
    if session_name.is_none() {
        println!(
            "   {}  # Save future questions to session",
            "termai ask --session <name> \"question\"".cyan()
        );
    }

    Ok(())
}

/// Call Claude API with the given messages
async fn call_claude_api(
    repo: &SqliteRepository,
    messages: Vec<Message>,
    model: &str,
) -> Result<String> {
    let api_key = config_service::fetch_with_env_fallback(repo, &ConfigKeys::ClaudeApiKey.to_key())
        .context("Claude API key not configured. Run 'termai setup' to add your API key or set CLAUDE_API_KEY environment variable.")?;

    // Create a temporary session to use the existing chat service
    let mut session = Session::new_temporary();
    session.messages = messages;

    // Use the existing Claude chat service
    crate::llm::claude::service::chat::chat_with_model(&api_key.value, &mut session, Some(model))
        .await?;

    // Extract the assistant's response from the updated session
    if let Some(last_message) = session.messages.last() {
        if last_message.role == Role::Assistant {
            return Ok(last_message.content.clone());
        }
    }

    Err(anyhow::anyhow!("No response received from Claude API"))
}

/// Call OpenAI API with the given messages
async fn call_openai_api(
    repo: &SqliteRepository,
    messages: Vec<Message>,
    model: &str,
) -> Result<String> {
    let api_key = config_service::fetch_with_env_fallback(repo, &ConfigKeys::ChatGptApiKey.to_key())
        .context("OpenAI API key not configured. Run 'termai setup' to add your API key or set OPENAI_API_KEY environment variable.")?;

    // Create a temporary session to use the existing chat service
    let mut session = Session::new_temporary();
    session.messages = messages;

    // Use the existing OpenAI chat service
    crate::llm::openai::service::chat::chat_with_model(&api_key.value, &mut session, Some(model))
        .await?;

    // Extract the assistant's response from the updated session
    if let Some(last_message) = session.messages.last() {
        if last_message.role == Role::Assistant {
            return Ok(last_message.content.clone());
        }
    }

    Err(anyhow::anyhow!("No response received from OpenAI API"))
}

/// Call Codex API with the given messages (OAuth authentication)
async fn call_codex_api(
    repo: &SqliteRepository,
    messages: Vec<Message>,
    model: &str,
) -> Result<String> {
    use crate::auth::token_manager::TokenManager;

    // Get valid access token (auto-refreshes if needed)
    let token_manager = TokenManager::new(repo);
    let access_token = token_manager
        .get_valid_token()
        .await
        .context("Failed to get Codex access token")?
        .ok_or_else(|| anyhow::anyhow!(
            "Not authenticated with Codex. Run 'termai auth login codex' to authenticate with your ChatGPT Plus/Pro subscription."
        ))?;

    // Create a temporary session to use the Codex chat service
    let mut session = Session::new_temporary();
    session.messages = messages;

    // Use the Codex chat service, honoring any configured reasoning effort
    let effort = crate::config::service::config_service::fetch_reasoning_effort(repo);
    crate::llm::openai::service::codex::chat(&access_token, &mut session, Some(model), effort)
        .await?;

    // Extract the assistant's response from the updated session
    if let Some(last_message) = session.messages.last() {
        if last_message.role == Role::Assistant {
            return Ok(last_message.content.clone());
        }
    }

    Err(anyhow::anyhow!("No response received from Codex API"))
}
