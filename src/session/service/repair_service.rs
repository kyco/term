//! Repair for conversations damaged by the re-insert bug.
//!
//! Before ids were written back into the live conversation, every message
//! still looked unsaved on the next turn, so each turn re-inserted the whole
//! conversation. A 20-turn chat became 420 stored rows, and resuming it
//! replayed every message several times over.
//!
//! Duplicates are *marked*, never deleted: the text stays in the database,
//! out of the conversation, and `--undo` puts it all back.

use crate::config::repository::ConfigRepository;
use crate::config::service::redacted_config;
use crate::session::model::session::Session;
use crate::session::repository::message_repository::StoredRow;
use crate::session::repository::{MessageRepository, SessionRepository};
use anyhow::Result;
use colored::*;

/// What a repair would do, or did, to one session.
pub struct SessionRepair {
    pub name: String,
    pub stored: usize,
    pub duplicates: Vec<String>,
}

impl SessionRepair {
    fn kept(&self) -> usize {
        self.stored - self.duplicates.len()
    }
}

/// Rebuild a conversation from a table that also contains its replays.
///
/// Each turn re-inserted every message stored so far, so the conversation
/// appears as a series of nested prefixes: `B1 B2 … Bk`, where every block
/// opens with the one before it and the last block is the conversation as it
/// finally stood.
///
/// The signature that leaves is a chunk immediately followed by an identical
/// chunk. At each position take the *longest* such chunk, drop it as a replay
/// and carry on; when there is none, the row is real and is kept. Nothing is
/// assumed about how many messages a turn adds, so tool calls, retries and
/// compaction do not throw it off, and anything that does not fit the pattern
/// is kept untouched.
fn keep_indices(rows: &[StoredRow], redactions: &[String]) -> Vec<usize> {
    // Compare on hashes of (role, redaction-normalised content): whole-chunk
    // equality at an exact offset is the evidence, and hashing keeps the
    // search cheap on long sessions.
    let keys: Vec<u64> = rows
        .iter()
        .map(|row| hash_key(&row.role, &normalise(&row.content, redactions)))
        .collect();
    // Sessions resumed many times interleave several runs' replays, and
    // removing one round can bring the next pair of copies together. Repeat
    // until a pass finds nothing, so the result does not depend on how many
    // times the conversation happened to be reopened.
    let mut surviving: Vec<usize> = (0..rows.len()).collect();
    loop {
        let pass = strip_adjacent_replays(&surviving.iter().map(|i| keys[*i]).collect::<Vec<u64>>());
        if pass.len() == surviving.len() {
            return surviving;
        }
        surviving = pass.into_iter().map(|i| surviving[i]).collect();
    }
}

/// One pass: drop every chunk that is immediately followed by a copy of
/// itself, longest chunk first.
fn strip_adjacent_replays(keys: &[u64]) -> Vec<usize> {
    let total = keys.len();
    let mut keep = Vec::new();
    let mut cursor = 0;

    while cursor < total {
        // A single repeated message is far more likely to be something the
        // user actually said twice, so only chunks of two or more count.
        let longest_replay = (2..=(total - cursor) / 2)
            .rev()
            .find(|len| keys[cursor..cursor + len] == keys[cursor + len..cursor + 2 * len]);

        match longest_replay {
            Some(len) => cursor += len,
            None => {
                keep.push(cursor);
                cursor += 1;
            }
        }
    }

    keep
}

fn hash_key(role: &str, content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    role.hash(&mut hasher);
    content.hash(&mut hasher);
    hasher.finish()
}

/// Find the re-inserted rows in one session.
fn find_duplicates<MR: MessageRepository>(
    message_repository: &MR,
    session: &Session,
    redactions: &[String],
) -> Result<SessionRepair> {
    let rows = message_repository
        .fetch_conversation_rows(&session.id)
        .map_err(|err| anyhow::anyhow!("could not read session '{}': {:?}", session.name, err))?;

    let keep: std::collections::HashSet<usize> =
        keep_indices(&rows, redactions).into_iter().collect();

    // Rows a previous repair already set aside stay set aside; only newly
    // identified re-inserts are reported, so re-running is a no-op.
    let duplicates = rows
        .iter()
        .enumerate()
        .filter(|(index, row)| {
            row.message_type != crate::session::repository::message_repository::DUPLICATE
                && !keep.contains(index)
        })
        .map(|(_, row)| row.id.clone())
        .collect::<Vec<String>>();

    let active = rows
        .iter()
        .filter(|row| {
            row.message_type != crate::session::repository::message_repository::DUPLICATE
        })
        .count();

    Ok(SessionRepair {
        name: session.name.clone(),
        stored: active,
        duplicates,
    })
}

/// `termai sessions repair`.
pub fn repair<SR: SessionRepository, MR: MessageRepository, CR: ConfigRepository>(
    session_repo: &SR,
    message_repository: &MR,
    config_repo: &CR,
    apply: bool,
    undo: bool,
) -> Result<()> {
    if undo {
        let restored = message_repository
            .restore_duplicates()
            .map_err(|err| anyhow::anyhow!("could not restore messages: {:?}", err))?;
        println!();
        println!(
            "  {} {} message(s) put back into their conversations.",
            "↩".bright_green(),
            restored.to_string().bright_yellow()
        );
        println!();
        return Ok(());
    }

    let sessions = session_repo
        .fetch_all_sessions()
        .map_err(|err| anyhow::anyhow!("could not list sessions: {:?}", err))?
        .iter()
        .map(Session::from)
        .collect::<Vec<Session>>();

    let redactions = redacted_config::fetch_redactions(config_repo);
    let mut repairs = Vec::new();
    for session in &sessions {
        let repair = find_duplicates(message_repository, session, &redactions)?;
        if !repair.duplicates.is_empty() {
            repairs.push(repair);
        }
    }

    if repairs.is_empty() {
        println!();
        println!("  {} No duplicated messages found.", "✓".green());
        println!();
        return Ok(());
    }

    repairs.sort_by_key(|repair| std::cmp::Reverse(repair.duplicates.len()));
    let total: usize = repairs.iter().map(|repair| repair.duplicates.len()).sum();

    println!();
    println!(
        "  {} {} duplicated message(s) across {} session(s)",
        if apply { "Repairing" } else { "Found" }.bold(),
        total.to_string().bright_yellow(),
        repairs.len().to_string().bright_yellow()
    );
    println!();
    println!(
        "  {:<32}  {:>7}  {:>7}  {:>7}",
        "SESSION".bold(),
        "STORED".bold(),
        "REAL".bold(),
        "EXTRA".bold()
    );
    for repair in &repairs {
        println!(
            "  {:<32}  {:>7}  {:>7}  {:>7}",
            truncate(&repair.name, 32).bright_cyan(),
            repair.stored,
            repair.kept(),
            repair.duplicates.len().to_string().bright_red()
        );
    }
    println!();

    if !apply {
        println!(
            "  {} {}",
            "Nothing changed.".dimmed(),
            "Re-run with --apply to clean these up.".bright_cyan()
        );
        println!(
            "  {}",
            "Duplicates are marked, not deleted — `termai sessions repair --undo` reverses it."
                .dimmed()
        );
        println!();
        return Ok(());
    }

    for repair in &repairs {
        message_repository
            .mark_duplicates(&repair.duplicates)
            .map_err(|err| anyhow::anyhow!("could not repair '{}': {:?}", repair.name, err))?;
    }

    println!(
        "  {} {} message(s) removed from their conversations.",
        "✓".green(),
        total.to_string().bright_yellow()
    );
    println!(
        "  {}",
        "Nothing was deleted — `termai sessions repair --undo` puts them back.".dimmed()
    );
    println!();
    Ok(())
}

/// Compare messages as they would read with redaction damage removed.
///
/// Redaction generated a fresh UUID per turn and the mapping was never
/// stored, so the *same* message re-inserted later carries different
/// placeholders — and where an old placeholder could no longer be reversed,
/// one copy kept the substitution while another did not. Every UUID collapses
/// to one marker, and every configured redaction term collapses to the same
/// marker, so both copies read alike whether or not the substitution stuck.
fn normalise(content: &str, redactions: &[String]) -> String {
    let mut out = collapse_uuids(content);
    // Longest term first. The original redaction walked a HashMap, so on one
    // turn "db systel" was replaced whole and on another "db" went first and
    // left "systel" behind; collapsing the longest match makes both copies
    // normalise the same way.
    let mut terms: Vec<&str> = redactions
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .collect();
    terms.sort_by_key(|term| std::cmp::Reverse(term.len()));

    for term in terms {
        out = replace_ignoring_case(&out, term, MARKER);
    }
    out
}

/// Case-insensitive substring replacement, matching how redaction is applied.
fn replace_ignoring_case(haystack: &str, needle: &str, replacement: &str) -> String {
    let lower_haystack = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut cursor = 0;
    while let Some(found) = lower_haystack[cursor..].find(&lower_needle) {
        let start = cursor + found;
        out.push_str(&haystack[cursor..start]);
        out.push_str(replacement);
        cursor = start + needle.len();
    }
    out.push_str(&haystack[cursor..]);
    out
}

const MARKER: &str = "\u{0}x\u{0}";

fn collapse_uuids(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let bytes: Vec<char> = content.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(width) = uuid_width(&bytes[index..]) {
            out.push_str(MARKER);
            index += width;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    out
}

/// Length of a canonical 8-4-4-4-12 hex UUID starting here, if there is one.
fn uuid_width(chars: &[char]) -> Option<usize> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut offset = 0;
    for (group, len) in GROUPS.iter().enumerate() {
        if group > 0 {
            if chars.get(offset) != Some(&'-') {
                return None;
            }
            offset += 1;
        }
        for _ in 0..*len {
            match chars.get(offset) {
                Some(c) if c.is_ascii_hexdigit() => offset += 1,
                _ => return None,
            }
        }
    }
    Some(offset)
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut out: String = value.chars().take(width.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(role: &str, content: &str) -> StoredRow {
        StoredRow {
            id: format!("{role}:{content}"),
            role: role.to_string(),
            content: content.to_string(),
            message_type: "standard".to_string(),
        }
    }

    /// Build the exact shape the bug produced: after turn k the whole
    /// conversation so far is inserted again.
    fn replayed(turns: usize) -> Vec<StoredRow> {
        let mut rows = Vec::new();
        for turn in 1..=turns {
            for i in 1..=turn {
                rows.push(row("user", &format!("q{i}")));
                rows.push(row("assistant", &format!("a{i}")));
            }
        }
        rows
    }

    fn kept_contents(rows: &[StoredRow], redactions: &[String]) -> Vec<String> {
        keep_indices(rows, redactions)
            .into_iter()
            .map(|index| rows[index].content.clone())
            .collect()
    }

    #[test]
    fn a_replayed_conversation_is_reduced_to_what_was_actually_said() {
        let rows = replayed(5);
        assert_eq!(rows.len(), 30);
        assert_eq!(
            kept_contents(&rows, &[]),
            vec!["q1", "a1", "q2", "a2", "q3", "a3", "q4", "a4", "q5", "a5"]
        );
    }

    #[test]
    fn a_clean_conversation_is_left_alone() {
        let rows = vec![
            row("user", "q1"),
            row("assistant", "a1"),
            row("user", "q2"),
            row("assistant", "a2"),
        ];
        assert_eq!(kept_contents(&rows, &[]), vec!["q1", "a1", "q2", "a2"]);
    }

    /// The user asking the same thing twice in a row is not a replay.
    #[test]
    fn a_genuinely_repeated_message_survives() {
        let rows = vec![
            row("user", "retry please"),
            row("assistant", "sure"),
            row("user", "retry please"),
            row("assistant", "here you go"),
        ];
        assert_eq!(
            kept_contents(&rows, &[]),
            vec!["retry please", "sure", "retry please", "here you go"]
        );
    }

    /// Two runs against the same session each leave their own replays.
    #[test]
    fn replays_from_separate_runs_are_both_removed() {
        let mut rows = replayed(2);
        rows.extend(vec![
            row("user", "q9"),
            row("assistant", "a9"),
            row("user", "q9"),
            row("assistant", "a9"),
            row("user", "q10"),
            row("assistant", "a10"),
        ]);
        assert_eq!(
            kept_contents(&rows, &[]),
            vec!["q1", "a1", "q2", "a2", "q9", "a9", "q10", "a10"]
        );
    }

    /// Copies of one message can carry different redaction placeholders, and
    /// a term that could no longer be reversed stays substituted in only some
    /// of them. They are still the same message.
    #[test]
    fn copies_damaged_by_redaction_still_match() {
        let redactions = vec!["rim".to_string()];
        let rows = vec![
            row("user", "check the primitives in 550e8400-e29b-41d4-a716-446655440000"),
            row("assistant", "ok"),
            row("user", "check the p2b7d4f9c-1111-4222-8333-9444aaaa5555itives in 6ba7b810-9dad-11d1-80b4-00c04fd430c8"),
            row("assistant", "ok"),
            row("user", "second question"),
            row("assistant", "second answer"),
        ];
        assert_eq!(
            kept_contents(&rows, &redactions).len(),
            4,
            "the damaged copy of the first turn should be recognised as a replay"
        );
    }

    #[test]
    fn normalise_collapses_uuids_and_redaction_terms() {
        let redactions = vec!["systel".to_string(), "db systel".to_string()];
        assert_eq!(
            normalise("hello 550e8400-e29b-41d4-a716-446655440000", &[]),
            format!("hello {MARKER}")
        );
        // The longest term wins, whichever order the original redaction used.
        assert_eq!(
            normalise("db systel here", &redactions),
            format!("{MARKER} here")
        );
    }
}
