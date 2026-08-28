//! End-to-end tests for `termai sessions` (including conversation branching)
//! and `termai preset` management.
//!
//! Sessions can normally only be created through `ask`/`chat`, which require
//! API keys and network access. To stay deterministic these tests seed
//! sessions/messages directly into the SQLite database inside an isolated
//! temporary HOME, then drive the real binary against the seeded data.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// An isolated HOME directory with an initialised termai database.
struct TestHome {
    dir: TempDir,
}

impl TestHome {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".config").join("termai")).unwrap();
        let home = Self { dir };
        // Run a cheap command once so the binary creates the database schema.
        home.cmd().arg("--print-config").assert().success();
        home
    }

    /// A termai command pointed at this isolated HOME.
    fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = cargo_bin_cmd!("termai");
        cmd.env("HOME", self.dir.path())
            .env("XDG_CONFIG_HOME", self.dir.path().join(".config"))
            .env("NO_COLOR", "1");
        cmd
    }

    fn db_path(&self) -> PathBuf {
        self.dir
            .path()
            .join(".config")
            .join("termai")
            .join("app.db")
    }

    fn db(&self) -> Connection {
        Connection::open(self.db_path()).unwrap()
    }

    /// Seed a session row. `expires_at` must use the `%Y-%m-%d %H:%M:%S` format.
    fn seed_session(&self, id: &str, name: &str, expires_at: &str, current: bool) {
        self.db()
            .execute(
                "INSERT INTO sessions (id, name, expires_at, current) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, name, expires_at, current as i32],
            )
            .unwrap();
    }

    fn seed_message(&self, id: &str, session_id: &str, role: &str, content: &str) {
        self.db()
            .execute(
                "INSERT INTO messages (id, session_id, role, content, message_type)
                 VALUES (?1, ?2, ?3, ?4, 'standard')",
                rusqlite::params![id, session_id, role, content],
            )
            .unwrap();
    }

    /// Seed a current session named `name` with two messages.
    fn seed_default_session(&self, id: &str, name: &str) {
        self.seed_session(id, name, "2099-01-01 00:00:00", true);
        self.seed_message(&format!("{id}-m1"), id, "user", "Hello world question");
        self.seed_message(&format!("{id}-m2"), id, "assistant", "Hello world answer");
    }

    fn count(&self, sql: &str) -> i64 {
        self.db().query_row(sql, [], |row| row.get(0)).unwrap()
    }
}

// ---------------------------------------------------------------------------
// sessions: listing and basic management
// ---------------------------------------------------------------------------

#[test]
fn sessions_list_on_fresh_home_shows_no_sessions() {
    let home = TestHome::new();
    home.cmd()
        .args(["sessions", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No sessions yet"));
}

#[test]
fn sessions_accepts_limit_and_sort_flags_before_subcommand() {
    let home = TestHome::new();
    home.cmd()
        .args(["sessions", "--limit", "5", "--sort", "name", "list"])
        .assert()
        .success();
}

#[test]
fn sessions_list_shows_seeded_sessions() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.seed_session("sess-old", "expired-session", "2020-01-01 00:00:00", false);

    home.cmd()
        .args(["sessions", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("alpha")
                .and(predicate::str::contains("expired-session"))
                // The listing shows the opening prompt so a session is
                // recognisable without opening it.
                .and(predicate::str::contains("Hello world question"))
                .and(predicate::str::contains("Resume:")),
        );
}

#[test]
fn sessions_list_limit_caps_rows() {
    let home = TestHome::new();
    home.seed_session("sess-a", "alpha", "2099-01-01 00:00:00", false);
    home.seed_session("sess-b", "beta", "2099-01-02 00:00:00", false);
    home.seed_session("sess-c", "gamma", "2099-01-03 00:00:00", true);

    // Sorted by name and capped at 2, only alpha and beta may appear.
    home.cmd()
        .args(["sessions", "--limit", "2", "--sort", "name", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("alpha")
                .and(predicate::str::contains("beta"))
                .and(predicate::str::contains("gamma").not()),
        );
}

#[test]
fn sessions_list_sort_orders_rows() {
    let home = TestHome::new();
    // Seed out of alphabetical order; dates make beta the most recent.
    home.seed_session("sess-g", "gamma", "2099-01-01 00:00:00", false);
    home.seed_session("sess-a", "alpha", "2099-01-02 00:00:00", false);
    home.seed_session("sess-b", "beta", "2099-01-03 00:00:00", true);

    // --sort name lists alphabetically.
    let assert = home
        .cmd()
        .args(["sessions", "--sort", "name", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let alpha = stdout.find("alpha").expect("alpha listed");
    let beta = stdout.find("beta").expect("beta listed");
    let gamma = stdout.find("gamma").expect("gamma listed");
    assert!(
        alpha < beta && beta < gamma,
        "--sort name must list alphabetically, got:\n{stdout}"
    );

    // --sort date lists most recently used first.
    let assert = home
        .cmd()
        .args(["sessions", "--sort", "date", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let alpha = stdout.find("alpha").expect("alpha listed");
    let beta = stdout.find("beta").expect("beta listed");
    let gamma = stdout.find("gamma").expect("gamma listed");
    assert!(
        beta < alpha && alpha < gamma,
        "--sort date must list most recent first, got:\n{stdout}"
    );
}

#[test]
fn sessions_list_filter_matches_name_substring() {
    let home = TestHome::new();
    home.seed_session("sess-a", "project-alpha", "2099-01-01 00:00:00", false);
    home.seed_session("sess-b", "project-beta", "2099-01-02 00:00:00", false);
    home.seed_session("sess-c", "scratch", "2099-01-03 00:00:00", true);

    home.cmd()
        .args(["sessions", "--filter", "project", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("project-alpha")
                .and(predicate::str::contains("project-beta"))
                .and(predicate::str::contains("scratch").not()),
        );

    home.cmd()
        .args(["sessions", "--filter", "beta", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("project-beta")
                .and(predicate::str::contains("project-alpha").not())
                .and(predicate::str::contains("scratch").not()),
        );
}

#[test]
fn sessions_find_searches_message_content() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.seed_session("sess-b", "beta", "2099-01-02 00:00:00", false);
    home.seed_message("sess-b-m1", "sess-b", "user", "how do I configure flyway");

    // The name says nothing about the content; the search has to read the
    // messages.
    home.cmd()
        .args(["sessions", "find", "flyway"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("beta")
                .and(predicate::str::contains("configure flyway"))
                .and(predicate::str::contains("alpha").not()),
        );

    home.cmd()
        .args(["sessions", "find", "nothing-mentions-this"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No conversation mentions"));
}

#[test]
fn sessions_repair_previews_then_removes_duplicates() {
    let home = TestHome::new();
    home.seed_session("sess-a", "duped", "2099-01-01 00:00:00", true);
    // The shape older versions produced: turn 1 stored, then turn 2 re-storing
    // turn 1 alongside it.
    home.seed_message("m1", "sess-a", "user", "q1");
    home.seed_message("m2", "sess-a", "assistant", "a1");
    home.seed_message("m3", "sess-a", "user", "q1");
    home.seed_message("m4", "sess-a", "assistant", "a1");
    home.seed_message("m5", "sess-a", "user", "q2");
    home.seed_message("m6", "sess-a", "assistant", "a2");

    // A preview changes nothing.
    home.cmd()
        .args(["sessions", "repair"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing changed"));
    assert_eq!(home.count("SELECT COUNT(*) FROM messages"), 6);

    home.cmd()
        .args(["sessions", "repair", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed from their conversations"));

    // Nothing is deleted, and the conversation reads correctly again.
    assert_eq!(home.count("SELECT COUNT(*) FROM messages"), 6);
    assert_eq!(
        home.count("SELECT COUNT(*) FROM messages WHERE message_type = 'duplicate'"),
        2
    );
    home.cmd()
        .args(["sessions", "show", "duped"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Messages: 4"));

    // Re-running finds nothing left to do, and --undo puts them back.
    home.cmd()
        .args(["sessions", "repair"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No duplicated messages found"));
    home.cmd()
        .args(["sessions", "repair", "--undo"])
        .assert()
        .success();
    assert_eq!(
        home.count("SELECT COUNT(*) FROM messages WHERE message_type = 'duplicate'"),
        0
    );
}

#[test]
fn sessions_show_displays_seeded_session_details() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");

    home.cmd()
        .args(["sessions", "show", "alpha"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Session Details")
                .and(predicate::str::contains("Name: alpha"))
                .and(predicate::str::contains("ID: sess-alpha"))
                .and(predicate::str::contains("Messages: 2"))
                .and(predicate::str::contains("Hello world question"))
                .and(predicate::str::contains("Hello world answer")),
        );
}

#[test]
fn sessions_show_unknown_session_fails_gracefully() {
    let home = TestHome::new();
    home.cmd()
        .args(["sessions", "show", "no-such-session"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Session command failed")
                .and(predicate::str::contains("Session Troubleshooting")),
        );
}

#[test]
fn sessions_delete_unknown_session_fails_gracefully() {
    let home = TestHome::new();
    home.cmd()
        .args(["sessions", "delete", "no-such-session"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Session command failed"));
}

#[test]
fn sessions_delete_without_tty_fails_and_preserves_session() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");

    // Deletion requires an interactive confirmation; without a TTY the
    // command must fail rather than silently delete data.
    home.cmd()
        .args(["sessions", "delete", "alpha"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Session deleted successfully").not());

    assert_eq!(home.count("SELECT COUNT(*) FROM sessions"), 1);
    assert_eq!(home.count("SELECT COUNT(*) FROM messages"), 2);
}

// ---------------------------------------------------------------------------
// sessions: conversation branching
// ---------------------------------------------------------------------------

#[test]
fn sessions_branch_create_is_persisted() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");

    home.cmd()
        .args([
            "sessions",
            "branch",
            "alpha",
            "--name",
            "feature-x",
            "--description",
            "try an alternative approach",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully created branch 'feature-x'",
        ));

    // The branch is visible from a second invocation...
    home.cmd()
        .args(["sessions", "branches", "alpha"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feature-x").and(predicate::str::contains("(root)")));

    home.cmd()
        .args(["sessions", "tree", "alpha"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Branch Tree").and(predicate::str::contains("feature-x")));

    // ...and is really persisted in the database.
    let (session_id, status): (String, String) = home
        .db()
        .query_row(
            "SELECT session_id, status FROM conversation_branches WHERE branch_name = 'feature-x'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(session_id, "sess-alpha");
    assert_eq!(status, "active");
    // Branch messages were copied from the source session.
    assert_eq!(home.count("SELECT COUNT(*) FROM branch_messages"), 2);
}

#[test]
fn sessions_branch_against_nonexistent_session_fails_gracefully() {
    let home = TestHome::new();
    home.cmd()
        .args(["sessions", "branch", "ghost", "--name", "b1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Session command failed"));
    assert_eq!(home.count("SELECT COUNT(*) FROM conversation_branches"), 0);
}

#[test]
fn sessions_bookmark_create_and_remove_roundtrip() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    // Create a bookmark and verify it is persisted.
    home.cmd()
        .args([
            "sessions",
            "bookmark",
            "alpha",
            "feature-x",
            "--name",
            "fav",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created bookmark 'fav'"));
    let bookmark: String = home
        .db()
        .query_row(
            "SELECT value FROM branch_metadata WHERE key = 'bookmark'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(bookmark, "fav");

    // The bookmark is searchable.
    home.cmd()
        .args(["sessions", "search", "alpha", "fav"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Found 1 branches").and(predicate::str::contains("feature-x")),
        );

    // Remove it again and verify it is gone.
    home.cmd()
        .args(["sessions", "bookmark", "alpha", "feature-x", "--remove"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed bookmark"));
    assert_eq!(
        home.count("SELECT COUNT(*) FROM branch_metadata WHERE key = 'bookmark'"),
        0
    );
}

#[test]
fn sessions_switch_returns_honest_not_supported_error() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    // Switching the active branch context is not implemented; the command
    // must fail instead of pretending the switch happened.
    home.cmd()
        .args(["sessions", "switch", "alpha", "feature-x"])
        .assert()
        .failure()
        .stdout(
            // It resolves and displays the target branch first...
            predicate::str::contains("Branch Switch")
                .and(predicate::str::contains("feature-x"))
                // ...but never claims success.
                .and(predicate::str::contains("Switched").not())
                .and(predicate::str::contains("\u{2705}").not()),
        )
        // The honest inner reason must reach stderr, not just the banner.
        .stderr(
            predicate::str::contains("Session command failed")
                .and(predicate::str::contains("not implemented")),
        );
}

#[test]
fn sessions_stats_reports_seeded_branches() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    home.cmd()
        .args(["sessions", "stats", "alpha"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Branch Statistics for 'alpha'")
                .and(predicate::str::contains("Total branches:"))
                .and(predicate::str::contains("Active branches:")),
        );
}

#[test]
fn sessions_compare_requires_at_least_two_branches() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    home.cmd()
        .args(["sessions", "compare", "alpha", "feature-x"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Need at least 2 branches to compare",
        ));
}

// ---------------------------------------------------------------------------
// sessions: export / archive / cleanup
// ---------------------------------------------------------------------------

#[test]
fn sessions_export_branch_to_file_contains_session_content() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    let export_path = home.dir.path().join("branch_export.json");
    home.cmd()
        .args([
            "sessions",
            "export",
            "alpha",
            "feature-x",
            "--format",
            "json",
            "--output",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("exported to"));

    let exported = fs::read_to_string(&export_path).unwrap();
    assert!(exported.contains("feature-x"));
    assert!(exported.contains("Hello world question"));
    assert!(exported.contains("Hello world answer"));
}

#[test]
fn sessions_archive_marks_branch_archived_in_db() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    home.cmd()
        .args([
            "sessions",
            "archive",
            "alpha",
            "feature-x",
            "--reason",
            "done",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 branches archived"));

    let status: String = home
        .db()
        .query_row(
            "SELECT status FROM conversation_branches WHERE branch_name = 'feature-x'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "archived");
}

#[test]
fn sessions_archive_unknown_branch_fails_gracefully() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");

    home.cmd()
        .args(["sessions", "archive", "alpha", "no-such-branch"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"))
        .stderr(predicate::str::contains("Session command failed"));
}

#[test]
fn sessions_cleanup_reports_honest_counts() {
    let home = TestHome::new();
    // Seed a current and an expired session; cleanup operates on the named
    // session's branches.
    home.seed_default_session("sess-alpha", "alpha");
    home.seed_session("sess-old", "expired-session", "2020-01-01 00:00:00", false);
    home.cmd()
        .args(["sessions", "branch", "alpha", "--name", "feature-x"])
        .assert()
        .success();

    // Preview mode: reports counts without touching anything. The seeded
    // branch has messages, so `remove-empty` must find nothing to clean.
    home.cmd()
        .args([
            "sessions",
            "cleanup",
            "alpha",
            "--strategy",
            "remove-empty",
            "--preview",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Branch Cleanup")
                .and(predicate::str::contains("Found: 0 branches"))
                .and(predicate::str::contains("Would preserve: 1 branches")),
        );

    // Real run reports the same honest zero-cleanup outcome and the branch
    // survives.
    home.cmd()
        .args(["sessions", "cleanup", "alpha", "--strategy", "remove-empty"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 branches cleaned up"));
    assert_eq!(home.count("SELECT COUNT(*) FROM conversation_branches"), 1);
}

#[test]
fn sessions_cleanup_days_flag_controls_archive_old_cutoff() {
    let home = TestHome::new();
    home.seed_default_session("sess-alpha", "alpha");

    // Seed an archived branch whose last activity is ~2000 days in the past.
    home.db()
        .execute(
            "INSERT INTO conversation_branches
                 (id, session_id, parent_branch_id, branch_name, description,
                  created_at, last_activity, status)
             VALUES ('br-old', 'sess-alpha', NULL, 'old-branch', NULL,
                     '2020-01-01 00:00:00', '2020-01-01 00:00:00', 'archived')",
            [],
        )
        .unwrap();

    // With a cutoff far larger than the branch's age, archive-old must find
    // nothing to clean. (A hardcoded 30-day cutoff would wrongly clean it.)
    home.cmd()
        .args([
            "sessions",
            "cleanup",
            "alpha",
            "--strategy",
            "archive-old",
            "--days",
            "1000000",
            "--preview",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Found: 0 branches"));
    assert_eq!(home.count("SELECT COUNT(*) FROM conversation_branches"), 1);

    // With a 1-day cutoff the ancient archived branch must be cleaned up.
    home.cmd()
        .args([
            "sessions",
            "cleanup",
            "alpha",
            "--strategy",
            "archive-old",
            "--days",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 branches cleaned up"));
    assert_eq!(home.count("SELECT COUNT(*) FROM conversation_branches"), 0);
}

// ---------------------------------------------------------------------------
// presets
// ---------------------------------------------------------------------------

const BUILTIN_PRESETS: [&str; 5] = [
    "Code Review Assistant",
    "Documentation Generator",
    "Test Generator",
    "Debugging Assistant",
    "Refactoring Assistant",
];

/// A minimal user preset in the on-disk YAML format used by import/export.
const USER_PRESET_YAML: &str = r#"name: My E2E Preset
description: A preset used by e2e tests
category: testing
version: "1.0"
template:
  name: My E2E Preset
  description: A preset used by e2e tests
  template: "Say hello to {{target}} please"
  variables:
    target:
      type: string
      required: true
      default: null
      description: Who to greet
      pattern: null
  metadata:
    version: null
    author: null
    tags: []
    created: null
    updated: null
    usage_count: 0
config: {}
"#;

#[test]
fn preset_list_shows_all_five_builtins() {
    let home = TestHome::new();
    let mut expected = predicate::str::contains("Available Presets").boxed();
    for name in BUILTIN_PRESETS {
        expected = expected.and(predicate::str::contains(name)).boxed();
    }
    home.cmd()
        .args(["preset", "list"])
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn preset_show_renders_builtin_with_template() {
    let home = TestHome::new();
    home.cmd()
        .args(["preset", "show", "Test Generator", "--template"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Test Generator")
                .and(predicate::str::contains("Category: testing"))
                .and(predicate::str::contains("Template Content:"))
                .and(predicate::str::contains("{{test_framework}}")),
        );
}

#[test]
fn preset_search_finds_builtin_by_term() {
    let home = TestHome::new();
    home.cmd()
        .args(["preset", "search", "debug"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Found 1 result")
                .and(predicate::str::contains("Debugging Assistant")),
        );
}

#[test]
fn preset_show_unknown_name_fails_gracefully() {
    let home = TestHome::new();
    home.cmd()
        .args(["preset", "show", "No Such Preset"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Preset command failed").and(predicate::str::contains(
                "Preset Management Troubleshooting",
            )),
        );
}

#[test]
fn preset_import_then_export_roundtrip() {
    let home = TestHome::new();
    let seed_path = home.dir.path().join("seed_preset.yaml");
    fs::write(&seed_path, USER_PRESET_YAML).unwrap();

    // Import the preset.
    home.cmd()
        .args(["preset", "import", seed_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Preset 'My E2E Preset' imported"));

    // It now appears in the listing alongside the built-ins.
    home.cmd()
        .args(["preset", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("My E2E Preset"));

    // Export it back out and verify the template content survived.
    let export_path = home.dir.path().join("roundtrip.yaml");
    home.cmd()
        .args([
            "preset",
            "export",
            "My E2E Preset",
            "--file",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    let exported = fs::read_to_string(&export_path).unwrap();
    assert!(exported.contains("Say hello to {{target}} please"));
    assert!(exported.contains("My E2E Preset"));
}

#[test]
fn preset_import_missing_file_fails_gracefully() {
    let home = TestHome::new();
    home.cmd()
        .args(["preset", "import", "/nonexistent/preset.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Preset command failed"));
}

#[test]
fn preset_delete_builtin_is_rejected_but_user_preset_deletes() {
    let home = TestHome::new();

    // Built-in presets cannot be deleted.
    home.cmd()
        .args(["preset", "delete", "Code Review Assistant", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Preset command failed"));
    home.cmd()
        .args(["preset", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Code Review Assistant"));

    // User presets delete for real.
    let seed_path = home.dir.path().join("seed_preset.yaml");
    fs::write(&seed_path, USER_PRESET_YAML).unwrap();
    home.cmd()
        .args(["preset", "import", seed_path.to_str().unwrap()])
        .assert()
        .success();
    home.cmd()
        .args(["preset", "delete", "My E2E Preset", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted successfully"));
    home.cmd()
        .args(["preset", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("My E2E Preset").not());
}

#[test]
fn preset_export_of_builtin_is_not_supported() {
    let home = TestHome::new();
    let export_path = home.dir.path().join("builtin.yaml");
    // Built-ins are not stored on disk as user presets, so exporting one
    // fails with an error rather than fabricating a file.
    home.cmd()
        .args([
            "preset",
            "export",
            "Code Review Assistant",
            "--file",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Preset command failed"));
    assert!(!export_path.exists());
}

#[test]
fn preset_create_requires_interactive_terminal() {
    let home = TestHome::new();
    // Even with every flag supplied, creation ends in an interactive
    // "Save this preset?" confirmation, so it must fail without a TTY and
    // must not save anything.
    home.cmd()
        .args([
            "preset",
            "create",
            "my-noninteractive-preset",
            "--description",
            "test preset",
            "--category",
            "custom",
            "--template",
            "Plain template with no variables",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Preset command failed"));

    home.cmd()
        .args(["preset", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("my-noninteractive-preset").not());
}
