use crate::common::unique_id::generate_uuid_v4;
use crate::llm::common::model::role::Role;
use crate::session::model::message::Message;
use crate::session::repository::MessageRepository;
use crate::session::{model::session::Session, repository::SessionRepository};
use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use colored::*;

/// A session row as the listing needs it: no message bodies, just the
/// counts and the one line of context that makes it recognisable.
pub struct SessionSummary {
    pub name: String,
    pub current: bool,
    pub last_used_at: NaiveDateTime,
    pub message_count: i64,
    pub preview: String,
}

fn summarise<MR: MessageRepository>(message_repository: &MR, session: &Session) -> SessionSummary {
    let message_count = message_repository
        .count_messages_for_session(&session.id)
        .unwrap_or(0);
    let preview = message_repository
        .fetch_first_user_message(&session.id)
        .ok()
        .flatten()
        .map(|content| first_line(&content))
        .unwrap_or_default();

    SessionSummary {
        name: session.name.clone(),
        current: session.current,
        last_used_at: session.last_used_at,
        message_count,
        preview,
    }
}

/// Collapse a prompt to a single readable line for the listing.
fn first_line(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect()
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut out: String = value.chars().take(width.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// "3 minutes ago" beats a timestamp the reader has to subtract from today.
fn humanise_age(when: NaiveDateTime) -> String {
    let seconds = (Utc::now().naive_utc() - when).num_seconds();
    if seconds < 0 {
        return "just now".to_string();
    }
    let (value, unit) = match seconds {
        s if s < 60 => return "just now".to_string(),
        s if s < 3600 => (s / 60, "minute"),
        s if s < 86_400 => (s / 3600, "hour"),
        s if s < 2_592_000 => (s / 86_400, "day"),
        s if s < 31_536_000 => (s / 2_592_000, "month"),
        s => (s / 31_536_000, "year"),
    };
    format!(
        "{} {}{} ago",
        value,
        unit,
        if value == 1 { "" } else { "s" }
    )
}

fn print_session_table(summaries: &[SessionSummary]) {
    if summaries.is_empty() {
        println!();
        println!("  No sessions yet.");
        println!();
        println!(
            "  {}",
            "Every chat is saved automatically — run `termai chat` to start one.".dimmed()
        );
        println!();
        return;
    }

    let name_width = summaries
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 40);
    let age_width = summaries
        .iter()
        .map(|s| humanise_age(s.last_used_at).chars().count())
        .max()
        .unwrap_or(9)
        .max(9);

    println!();
    println!(
        "  {:<name_width$}  {:<age_width$}  {:>4}  {}",
        "SESSION".bold(),
        "LAST USED".bold(),
        "MSGS".bold(),
        "STARTED WITH".bold(),
        name_width = name_width,
        age_width = age_width,
    );

    for summary in summaries {
        let marker = if summary.current { "*" } else { " " };
        let preview = if summary.preview.is_empty() {
            "—".dimmed().to_string()
        } else {
            truncate(&summary.preview, 48).dimmed().to_string()
        };
        println!(
            "{} {:<name_width$}  {:<age_width$}  {:>4}  {}",
            marker.bright_green(),
            truncate(&summary.name, name_width).bright_cyan(),
            humanise_age(summary.last_used_at),
            summary.message_count,
            preview,
            name_width = name_width,
            age_width = age_width,
        );
    }

    println!();
    println!(
        "  {} {}",
        "Resume:".dimmed(),
        "termai chat --session <name>".bright_cyan()
    );
    println!(
        "  {} {}",
        "Search: ".dimmed(),
        "termai sessions find \"<text>\"".bright_cyan()
    );
    println!();
}

pub fn fetch_all_sessions<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
) -> Result<()> {
    fetch_sessions_with_options(
        session_repo,
        message_repository,
        None,
        None,
        &crate::args::SessionSortOrder::Date,
    )
}

/// List sessions applying the `sessions list` flags: `--filter` (name
/// substring), `--sort` (name | date | messages) and `--limit` (row cap).
pub fn fetch_sessions_with_options<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
    filter: Option<&str>,
    limit: Option<usize>,
    sort: &crate::args::SessionSortOrder,
) -> Result<()> {
    let session_entities = session_repo.fetch_all_sessions().unwrap_or_else(|_| vec![]);
    let needle = filter.map(str::to_lowercase);
    let mut summaries = session_entities
        .iter()
        .map(Session::from)
        .map(|session| summarise(message_repository, &session))
        .filter(|summary| match &needle {
            // Match the preview too: a month later the name is rarely what
            // the user remembers about a conversation.
            Some(needle) => {
                summary.name.to_lowercase().contains(needle)
                    || summary.preview.to_lowercase().contains(needle)
            }
            None => true,
        })
        .collect::<Vec<SessionSummary>>();

    match sort {
        crate::args::SessionSortOrder::Name => summaries.sort_by(|a, b| a.name.cmp(&b.name)),
        crate::args::SessionSortOrder::Date => {
            summaries.sort_by_key(|summary| std::cmp::Reverse(summary.last_used_at))
        }
        crate::args::SessionSortOrder::Messages => {
            summaries.sort_by_key(|summary| std::cmp::Reverse(summary.message_count))
        }
    }

    if let Some(limit) = limit {
        summaries.truncate(limit);
    }

    print_session_table(&summaries);
    Ok(())
}

/// Find sessions by what was actually said in them.
pub fn search_sessions<MR: MessageRepository>(
    message_repository: &MR,
    query: &str,
    limit: usize,
) -> Result<()> {
    let matches = message_repository
        .search_messages(query, limit)
        .map_err(|err| anyhow::anyhow!("could not search messages: {:?}", err))?;

    if matches.is_empty() {
        println!();
        println!("  No conversation mentions {}.", query.bright_yellow());
        println!();
        return Ok(());
    }

    println!();
    println!(
        "  {} {} for {}",
        matches.len().to_string().bright_yellow(),
        if matches.len() == 1 { "match" } else { "matches" },
        query.bright_yellow()
    );
    println!();

    for hit in &matches {
        println!(
            "  {} {}",
            hit.session_name.bright_cyan().bold(),
            format!("({})", hit.role).dimmed()
        );
        println!("    {}", truncate(&first_line(&hit.content), 100).dimmed());
        println!();
    }

    println!(
        "  {} {}",
        "Resume:".dimmed(),
        "termai chat --session <name>".bright_cyan()
    );
    println!();
    Ok(())
}

pub fn get_most_recent_session<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
) -> Result<Session> {
    let session_entities = session_repo
        .fetch_all_sessions()
        .map_err(|e| anyhow::anyhow!("Failed to fetch sessions: {:?}", e))?;

    let mut sessions = session_entities
        .iter()
        .map(Session::from)
        .collect::<Vec<Session>>();
    sessions.sort_by_key(|session| std::cmp::Reverse(session.last_used_at));

    // An empty session carries nothing to resume, and a freshly created one
    // would otherwise outrank every real conversation.
    let most_recent = sessions
        .iter()
        .find(|session| {
            message_repository
                .count_messages_for_session(&session.id)
                .unwrap_or(0)
                > 0
        })
        .or_else(|| sessions.first())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No previous sessions found. Start one with: termai chat --session <name>"
            )
        })?;

    Ok(session_with_messages(message_repository, most_recent))
}

pub fn session<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
    name: &str,
) -> Result<Session> {
    let session = match session_repo.fetch_session_by_name(name) {
        Err(_) => {
            let id = generate_uuid_v4().to_string();
            let now = Utc::now().naive_utc();

            session_repo
                .remove_current_from_all()
                .map_err(|err| anyhow::anyhow!("could not clear the current session: {:?}", err))?;
            session_repo
                .add_session(&id, name, now, true)
                .map_err(|err| anyhow::anyhow!("could not create session '{}': {:?}", name, err))?;

            let session = session_repo.fetch_session_by_name(name).map_err(|err| {
                anyhow::anyhow!("could not read back session '{}': {:?}", name, err)
            })?;
            Session::from(&session)
        }
        Ok(session) => Session::from(&session),
    };

    let session = session_with_messages(message_repository, &session);
    Ok(session)
}

/// Write every not-yet-stored message and refresh the session row.
///
/// Takes `&mut Session` on purpose: the generated row id is written back into
/// the in-memory message. Without that, `message.id` stays empty forever, every
/// message looks new on the next turn, and the whole conversation is inserted
/// again — turning an N-turn chat into N(N+1)/2 stored messages.
pub fn persist_session<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
    session: &mut Session,
) -> Result<()> {
    if session.temporary {
        return Ok(());
    }

    let now = Utc::now().naive_utc();
    // Upsert, not update: a session that was never inserted used to swallow
    // its own messages, leaving rows pointing at a session that did not exist.
    session_repo
        .upsert_session(&session.id, &session.name, now, session.current)
        .map_err(|err| anyhow::anyhow!("could not save session '{}': {:?}", session.name, err))?;

    for message in session.messages.iter_mut() {
        if !message.id.is_empty() {
            continue;
        }
        let id = generate_uuid_v4().to_string();
        let stored = message.copy_with_id(id.clone());
        message_repository
            .add_message_to_session(&stored.to_entity(&session.id))
            .map_err(|err| anyhow::anyhow!("could not save message: {:?}", err))?;
        message.id = id;
    }

    // The user turn was written ahead of the provider call; it is a real part
    // of the conversation now that the turn completed. Only this turn is
    // promoted — prompts left unanswered by an earlier run stay pending so
    // they never silently reappear in the middle of a conversation.
    if let Some(id) = session
        .messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| message.id.clone())
        .filter(|id| !id.is_empty())
    {
        message_repository
            .promote_message(&id)
            .map_err(|err| anyhow::anyhow!("could not finalise your message: {:?}", err))?;
    }

    session.last_used_at = now;
    Ok(())
}

/// Store the user's turn *before* the provider is called.
///
/// A dropped connection, a closed terminal or a crash between here and the
/// response used to take the typed prompt with it. The row is marked pending
/// so it stays out of the conversation until the turn actually completes.
pub fn write_ahead_user_message<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
    session: &mut Session,
) -> Result<()> {
    if session.temporary {
        return Ok(());
    }

    let now = Utc::now().naive_utc();
    session_repo
        .upsert_session(&session.id, &session.name, now, session.current)
        .map_err(|err| anyhow::anyhow!("could not save session '{}': {:?}", session.name, err))?;

    let Some(message) = session.messages.last_mut() else {
        return Ok(());
    };
    if !message.id.is_empty() || message.role != Role::User {
        return Ok(());
    }

    let id = generate_uuid_v4().to_string();
    let mut entity = message.copy_with_id(id.clone()).to_entity(&session.id);
    entity.message_type = crate::session::repository::message_repository::PENDING.to_string();
    message_repository
        .add_message_to_session(&entity)
        .map_err(|err| anyhow::anyhow!("could not save your message: {:?}", err))?;
    message.id = id;

    session.last_used_at = now;
    Ok(())
}

/// Prompts that were stored but never answered, oldest first.
pub fn recover_unsent_messages<MR: MessageRepository>(
    message_repository: &MR,
    session: &Session,
) -> Vec<String> {
    if session.temporary {
        return Vec::new();
    }
    message_repository
        .fetch_pending_messages(&session.id)
        .unwrap_or_default()
}

/// Drop recovered prompts once the user has been given them back.
pub fn clear_unsent_messages<MR: MessageRepository>(message_repository: &MR, session: &Session) {
    if !session.temporary {
        let _ = message_repository.discard_pending_messages(&session.id);
    }
}

/// Rename a session, refusing to collide with an existing one.
pub fn rename_session<SR: SessionRepository>(
    session_repo: &SR,
    session: &mut Session,
    new_name: &str,
) -> Result<()> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err(anyhow::anyhow!("a session name cannot be empty"));
    }
    if new_name == session.name {
        return Ok(());
    }
    if let Ok(existing) = session_repo.fetch_session_by_name(new_name) {
        if existing.id != session.id {
            return Err(anyhow::anyhow!(
                "a different session is already called '{}' — pick another name",
                new_name
            ));
        }
    }

    let now = Utc::now().naive_utc();
    session_repo
        .upsert_session(&session.id, new_name, now, session.current)
        .map_err(|err| anyhow::anyhow!("could not rename session: {:?}", err))?;
    session.name = new_name.to_string();
    session.last_used_at = now;
    Ok(())
}

/// Existing names close enough to be what the user meant.
///
/// `chat --session <typo>` silently created a brand-new empty session, so a
/// mistyped name looked exactly like a lost conversation.
pub fn suggest_similar_sessions<SR: SessionRepository>(
    session_repo: &SR,
    name: &str,
) -> Vec<String> {
    let needle = name.to_lowercase();
    let mut scored = session_repo
        .fetch_all_sessions()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|session| {
            let candidate = session.name.to_lowercase();
            if candidate.contains(&needle) || needle.contains(&candidate) {
                return Some((0, session.name));
            }
            let distance = edit_distance(&needle, &candidate);
            // Allow roughly one typo per four characters.
            (distance <= 1 + candidate.len() / 4).then_some((distance, session.name))
        })
        .collect::<Vec<(usize, String)>>();

    scored.sort();
    scored.truncate(3);
    scored.into_iter().map(|(_, name)| name).collect()
}

/// Levenshtein distance, two rows at a time.
fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (i, left_char) in left.chars().enumerate() {
        current[0] = i + 1;
        for (j, right_char) in right_chars.iter().enumerate() {
            let substitution = previous[j] + usize::from(left_char != *right_char);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right_chars.len()]
}

/// Remove a session that was created for a chat which never happened.
///
/// Opening a chat writes the row up front so the first prompt can be stored
/// before the provider is called; a chat abandoned before saying anything
/// would otherwise leave an empty session behind for good.
pub fn discard_if_empty<SR: SessionRepository, MR: MessageRepository>(
    session_repo: &SR,
    message_repository: &MR,
    session: &Session,
) {
    if session.temporary || !session.messages.is_empty() {
        return;
    }
    let empty = message_repository
        .count_messages_for_session(&session.id)
        .unwrap_or(1)
        == 0
        && message_repository
            .fetch_pending_messages(&session.id)
            .map(|pending| pending.is_empty())
            .unwrap_or(false);
    if empty {
        let _ = session_repo.delete_session(&session.id);
    }
}

/// Confirm from the database that a session really holds what we think it
/// does. Every "saved" message the user ever saw was printed without checking.
pub fn stored_message_count<MR: MessageRepository>(
    message_repository: &MR,
    session: &Session,
) -> Result<i64> {
    message_repository
        .count_messages_for_session(&session.id)
        .map_err(|err| anyhow::anyhow!("could not read back the session: {:?}", err))
}

fn session_with_messages<MR: MessageRepository>(
    message_repository: &MR,
    session: &Session,
) -> Session {
    let messages = message_repository
        .fetch_messages_for_session(&session.id)
        .unwrap_or_default()
        .iter()
        .map(Message::from)
        .collect::<Vec<Message>>();

    session.copy_with_messages(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::common::model::role::Role;
    use crate::repository::db::SqliteRepository;

    fn repo() -> SqliteRepository {
        SqliteRepository::new(":memory:").expect("in-memory database")
    }

    fn row_counts(repo: &SqliteRepository) -> (i64, i64) {
        let sessions = repo
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        let messages = repo
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap();
        (sessions, messages)
    }

    /// The whole point: a chat that is not explicitly temporary reaches disk.
    #[test]
    fn a_conversation_is_written_to_the_database() {
        let repo = repo();
        let mut chat = session(&repo, &repo, "work").unwrap();
        chat.add_raw_message("hello".to_string(), Role::User);
        chat.add_raw_message("hi there".to_string(), Role::Assistant);

        persist_session(&repo, &repo, &mut chat).unwrap();

        assert_eq!(row_counts(&repo), (1, 2));
        assert_eq!(stored_message_count(&repo, &chat).unwrap(), 2);
    }

    /// Ids are written back into the live conversation, so a message is
    /// stored once and not re-inserted on every following turn.
    #[test]
    fn messages_are_stored_once_per_turn() {
        let repo = repo();
        let mut chat = session(&repo, &repo, "work").unwrap();

        for turn in 1..=5 {
            chat.add_raw_message(format!("q{turn}"), Role::User);
            chat.add_raw_message(format!("a{turn}"), Role::Assistant);
            persist_session(&repo, &repo, &mut chat).unwrap();
        }

        assert_eq!(row_counts(&repo).1, 10, "one row per message, not N(N+1)/2");

        let reloaded = session(&repo, &repo, "work").unwrap();
        assert_eq!(reloaded.messages.len(), 10);
    }

    #[test]
    fn a_reloaded_conversation_keeps_its_order() {
        let repo = repo();
        let mut chat = session(&repo, &repo, "work").unwrap();
        for turn in 1..=4 {
            chat.add_raw_message(format!("q{turn}"), Role::User);
            chat.add_raw_message(format!("a{turn}"), Role::Assistant);
            persist_session(&repo, &repo, &mut chat).unwrap();
        }

        let reloaded = session(&repo, &repo, "work").unwrap();
        let contents: Vec<&str> = reloaded
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect();
        assert_eq!(
            contents,
            vec!["q1", "a1", "q2", "a2", "q3", "a3", "q4", "a4"]
        );
    }

    /// A dropped connection between the prompt and the response must not take
    /// the prompt with it.
    #[test]
    fn an_unanswered_prompt_survives_a_failed_turn() {
        let repo = repo();
        let mut chat = session(&repo, &repo, "work").unwrap();
        chat.add_raw_message("expensive question".to_string(), Role::User);

        write_ahead_user_message(&repo, &repo, &mut chat).unwrap();
        // ... the provider call fails here, and the turn is rolled back.
        chat.messages.pop();

        // It is not part of the conversation ...
        let reloaded = session(&repo, &repo, "work").unwrap();
        assert!(reloaded.messages.is_empty());
        assert_eq!(stored_message_count(&repo, &reloaded).unwrap(), 0);

        // ... but the text is still recoverable.
        assert_eq!(
            recover_unsent_messages(&repo, &reloaded),
            vec!["expensive question".to_string()]
        );
    }

    /// A prompt left unanswered by an earlier run must not silently reappear
    /// inside a later conversation.
    #[test]
    fn a_recovered_prompt_is_not_folded_into_a_later_turn() {
        let repo = repo();
        let mut chat = session(&repo, &repo, "work").unwrap();
        chat.add_raw_message("lost prompt".to_string(), Role::User);
        write_ahead_user_message(&repo, &repo, &mut chat).unwrap();
        chat.messages.pop();

        let mut chat = session(&repo, &repo, "work").unwrap();
        chat.add_raw_message("new question".to_string(), Role::User);
        write_ahead_user_message(&repo, &repo, &mut chat).unwrap();
        chat.add_raw_message("answer".to_string(), Role::Assistant);
        persist_session(&repo, &repo, &mut chat).unwrap();

        let reloaded = session(&repo, &repo, "work").unwrap();
        let contents: Vec<&str> = reloaded
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect();
        assert_eq!(contents, vec!["new question", "answer"]);
        assert_eq!(
            recover_unsent_messages(&repo, &reloaded),
            vec!["lost prompt".to_string()]
        );
    }

    #[test]
    fn temporary_sessions_are_never_written() {
        let repo = repo();
        let mut chat = Session::new_temporary();
        chat.add_raw_message("hello".to_string(), Role::User);
        chat.add_raw_message("hi".to_string(), Role::Assistant);

        persist_session(&repo, &repo, &mut chat).unwrap();
        write_ahead_user_message(&repo, &repo, &mut chat).unwrap();

        assert_eq!(row_counts(&repo), (0, 0));
    }

    #[test]
    fn renaming_onto_another_session_is_refused() {
        let repo = repo();
        let _taken = session(&repo, &repo, "taken").unwrap();
        let mut chat = session(&repo, &repo, "work").unwrap();

        let error = rename_session(&repo, &mut chat, "taken").unwrap_err();
        assert!(error.to_string().contains("already called"));
        assert_eq!(chat.name, "work");

        rename_session(&repo, &mut chat, "renamed").unwrap();
        assert_eq!(chat.name, "renamed");
        assert!(session_repo_has(&repo, "renamed"));
        assert!(!session_repo_has(&repo, "work"));
    }

    fn session_repo_has(repo: &SqliteRepository, name: &str) -> bool {
        repo.fetch_session_by_name(name).is_ok()
    }

    /// `--last` used to hand back whichever session was created most
    /// recently, including an empty one that had never been used.
    #[test]
    fn the_last_session_is_one_with_a_conversation_in_it() {
        let repo = repo();
        let mut real = session(&repo, &repo, "real-work").unwrap();
        real.add_raw_message("a question".to_string(), Role::User);
        real.add_raw_message("an answer".to_string(), Role::Assistant);
        persist_session(&repo, &repo, &mut real).unwrap();

        let _empty = session(&repo, &repo, "opened-by-accident").unwrap();

        let resumed = get_most_recent_session(&repo, &repo).unwrap();
        assert_eq!(resumed.name, "real-work");
        assert_eq!(resumed.messages.len(), 2);
    }
}
