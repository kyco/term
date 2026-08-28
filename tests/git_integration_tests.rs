/// Comprehensive integration tests for Git functionality
/// Tests all Git commands and their interactions with the repository
#[macro_use]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[cfg(test)]
mod git_integration_tests {
    use super::*;

    /// Test helper to create a temporary git repository
    fn setup_test_repo() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .assert()
            .success();

        // Set up git config for tests
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .assert()
            .success();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .assert()
            .success();

        // Create initial commit
        fs::write(
            repo_path.join("README.md"),
            "# Test Repository\n\nThis is a test repository for TermAI Git integration tests.\n",
        )?;

        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(repo_path)
            .assert()
            .success();

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .assert()
            .success();

        Ok(temp_dir)
    }

    /// Test helper to create a Rust project structure
    fn setup_rust_project(repo_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create Cargo.toml
        fs::write(
            repo_path.join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.0"
"#,
        )?;

        // Create src directory and main.rs
        fs::create_dir_all(repo_path.join("src"))?;
        fs::write(
            repo_path.join("src/main.rs"),
            r#"fn main() {
    println!("Hello, world!");
}
"#,
        )?;

        // Create lib.rs with some functions
        fs::write(
            repo_path.join("src/lib.rs"),
            r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
        )?;

        Ok(())
    }

    /// Test helper to create multiple commits
    fn create_test_commits(repo_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // First commit - add feature
        fs::write(
            repo_path.join("src/auth.rs"),
            r#"pub fn authenticate(username: &str, password: &str) -> bool {
    // TODO: Implement OAuth2
    username == "test" && password == "password"
}
"#,
        )?;

        Command::new("git")
            .args(["add", "src/auth.rs"])
            .current_dir(repo_path)
            .assert()
            .success();

        Command::new("git")
            .args(["commit", "-m", "feat: add authentication module"])
            .current_dir(repo_path)
            .assert()
            .success();

        // Second commit - fix bug
        fs::write(
            repo_path.join("src/auth.rs"),
            r#"pub fn authenticate(username: &str, password: &str) -> bool {
    // Fixed: Added proper validation
    !username.is_empty() && !password.is_empty() && 
    username == "test" && password == "password"
}
"#,
        )?;

        Command::new("git")
            .args(["add", "src/auth.rs"])
            .current_dir(repo_path)
            .assert()
            .success();

        Command::new("git")
            .args(["commit", "-m", "fix: add input validation to auth"])
            .current_dir(repo_path)
            .assert()
            .success();

        // Third commit - add tests
        fs::write(
            repo_path.join("src/auth.rs"),
            r#"pub fn authenticate(username: &str, password: &str) -> bool {
    // Fixed: Added proper validation
    !username.is_empty() && !password.is_empty() && 
    username == "test" && password == "password"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_credentials() {
        assert!(authenticate("test", "password"));
    }

    #[test]
    fn test_invalid_credentials() {
        assert!(!authenticate("wrong", "wrong"));
    }

    #[test]
    fn test_empty_credentials() {
        assert!(!authenticate("", ""));
    }
}
"#,
        )?;

        Command::new("git")
            .args(["add", "src/auth.rs"])
            .current_dir(repo_path)
            .assert()
            .success();

        Command::new("git")
            .args(["commit", "-m", "test: add comprehensive auth tests"])
            .current_dir(repo_path)
            .assert()
            .success();

        Ok(())
    }

    #[test]
    fn test_tag_list_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Create some test tags
        Command::new("git")
            .args(["tag", "v0.1.0"])
            .current_dir(repo_path)
            .assert()
            .success();

        Command::new("git")
            .args(["tag", "-a", "v0.2.0", "-m", "Version 0.2.0 release"])
            .current_dir(repo_path)
            .assert()
            .success();

        // Test termai tag list command
        let mut cmd = termai_cmd!();
        cmd.arg("tag")
            .arg("list")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "🏷️  TermAI Git Tag & Release Management",
            ))
            .stdout(predicate::str::contains("📋 Git Tags"))
            // Real tags created in the fixture must be listed
            .stdout(predicate::str::contains("v0.1.0"))
            .stdout(predicate::str::contains("v0.2.0"))
            // Annotated tag message must come from the real tag object
            .stdout(predicate::str::contains("Version 0.2.0 release"));
    }

    #[test]
    fn test_tag_suggest_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Test termai tag suggest command
        let mut cmd = termai_cmd!();
        cmd.arg("tag")
            .arg("suggest")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("🎯 Tag Suggestion"))
            .stdout(predicate::str::contains("🔍 Analyzing recent changes"))
            .stdout(predicate::str::contains("📊 Change Analysis"))
            .stdout(predicate::str::contains("🎯 Recommendation"))
            // Analysis must be derived from real commits in the fixture
            .stdout(predicate::str::contains("add authentication module"));
    }

    #[test]
    fn test_branch_summary_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Test termai branch-summary command
        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "🔍 Analyzing Git repository and branch",
            ))
            .stdout(predicate::str::contains("📊 Branch Analysis"))
            .stdout(predicate::str::contains("ℹ️  Branch Information"))
            .stdout(predicate::str::contains("🔄 Branch Comparison"));
    }

    #[test]
    fn test_branch_naming_suggestions() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");

        // Test termai branch-summary --suggest-name
        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .arg("--suggest-name")
            .arg("--context")
            .arg("OAuth integration")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("🌿 AI Branch Naming Assistant"))
            .stdout(predicate::str::contains("🔍 Analyzing repository context"))
            .stdout(predicate::str::contains("📊 Context Analysis"))
            .stdout(predicate::str::contains("💡 AI Branch Name Suggestions"))
            .stdout(predicate::str::contains("feature/oauth-integration"));
    }

    #[test]
    fn test_rebase_status_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Test termai rebase status command
        let mut cmd = termai_cmd!();
        cmd.arg("rebase")
            .arg("status")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "🔄 TermAI Interactive Rebase Assistant",
            ))
            .stdout(predicate::str::contains("📊 Rebase Status"))
            .stdout(predicate::str::contains("ℹ️ No rebase in progress"));
    }

    #[test]
    fn test_rebase_plan_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Test termai rebase plan command
        let mut cmd = termai_cmd!();
        cmd.arg("rebase")
            .arg("plan")
            .arg("--count")
            .arg("3")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("📋 Rebase Plan Generation"))
            .stdout(predicate::str::contains("🎯 Rebase Target Analysis"))
            .stdout(predicate::str::contains("📝 Rebase Recommendations"))
            .stdout(predicate::str::contains("📋 Suggested Rebase Plan"))
            // Plan must list real commits from the fixture, not mock data
            .stdout(predicate::str::contains("add input validation to auth"));
    }

    #[test]
    fn test_rebase_analyze_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Test termai rebase analyze command
        let mut cmd = termai_cmd!();
        cmd.arg("rebase")
            .arg("analyze")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("🔬 Commit Analysis for Rebase"))
            .stdout(predicate::str::contains("📊 Commit Statistics"))
            .stdout(predicate::str::contains("📈 Commit Type Distribution"))
            .stdout(predicate::str::contains("📝 Commit Details"));
    }

    #[test]
    fn test_conflicts_detect_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");

        // Test termai conflicts detect command (should show no conflicts)
        let mut cmd = termai_cmd!();
        cmd.arg("conflicts")
            .arg("detect")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "⚔️ TermAI Conflict Resolution Assistant",
            ))
            .stdout(predicate::str::contains("🔍 Detecting Merge Conflicts"))
            .stdout(predicate::str::contains("conflicts detected"));
    }

    #[test]
    fn test_conflicts_guide_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");

        // Test termai conflicts guide command
        let mut cmd = termai_cmd!();
        cmd.arg("conflicts")
            .arg("guide")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("📚 Conflict Resolution Guide"))
            .stdout(predicate::str::contains(
                "🔍 Understanding Conflict Markers",
            ))
            .stdout(predicate::str::contains("🛠️  Resolution Strategies"))
            .stdout(predicate::str::contains("🔧 Recommended Tools"))
            .stdout(predicate::str::contains("⚠️  Common Pitfalls"));
    }

    #[test]
    fn test_conflicts_status_command() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");

        // Test termai conflicts status command
        let mut cmd = termai_cmd!();
        cmd.arg("conflicts")
            .arg("status")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("📊 Conflict Status"))
            // Clean fixture repo: status must honestly report no operation
            .stdout(predicate::str::contains("No merge operation in progress"));
    }

    #[test]
    fn test_stash_list_empty() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        let mut cmd = termai_cmd!();
        cmd.arg("stash")
            .arg("list")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("📋 Git Stashes"))
            .stdout(predicate::str::contains("No stashes found"));
    }

    #[test]
    fn test_stash_push_list_and_pop() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        let original =
            fs::read_to_string(repo_path.join("README.md")).expect("Failed to read README");

        // Modify a tracked file so there is something to stash
        fs::write(repo_path.join("README.md"), "# Modified for stash test\n")
            .expect("Failed to modify README");

        // Push a real stash
        let mut cmd = termai_cmd!();
        cmd.arg("stash")
            .arg("push")
            .arg("--message")
            .arg("WIP: stash e2e test")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Stash created"));

        // The working tree must actually be restored to the committed state
        let after_push =
            fs::read_to_string(repo_path.join("README.md")).expect("Failed to read README");
        assert_eq!(after_push, original, "stash push must restore working tree");

        // The real stash must show up in the list
        let mut cmd = termai_cmd!();
        cmd.arg("stash")
            .arg("list")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("stash@{0}"))
            .stdout(predicate::str::contains("WIP: stash e2e test"));

        // Pop must actually re-apply the changes and remove the stash
        let mut cmd = termai_cmd!();
        cmd.arg("stash")
            .arg("pop")
            .current_dir(repo_path)
            .assert()
            .success();

        let after_pop =
            fs::read_to_string(repo_path.join("README.md")).expect("Failed to read README");
        assert_eq!(
            after_pop, "# Modified for stash test\n",
            "stash pop must restore stashed changes"
        );

        let mut cmd = termai_cmd!();
        cmd.arg("stash")
            .arg("list")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("No stashes found"));
    }

    #[test]
    fn test_git_integration_workflow() {
        // Comprehensive workflow test that combines multiple features
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // 1. Test branch analysis
        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Branch Analysis"));

        // 2. Test branch naming suggestions
        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .arg("--suggest-name")
            .arg("--context")
            .arg("API improvements")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("AI Branch Name Suggestions"));

        // 3. Test rebase planning
        let mut cmd = termai_cmd!();
        cmd.arg("rebase")
            .arg("plan")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Rebase Plan Generation"));

        // 4. Test tag suggestion
        let mut cmd = termai_cmd!();
        cmd.arg("tag")
            .arg("suggest")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Tag Suggestion"));

        // 5. Test conflict detection
        let mut cmd = termai_cmd!();
        cmd.arg("conflicts")
            .arg("detect")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Detecting Merge Conflicts"));
    }

    #[test]
    fn test_error_handling_outside_git_repo() {
        // Test that commands handle non-git directories gracefully
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Test tag command outside git repo
        let mut cmd = termai_cmd!();
        cmd.arg("tag")
            .arg("list")
            .current_dir(temp_dir.path())
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("No Git repository found")
                    .or(predicate::str::contains("failed")),
            );

        // Test branch command outside git repo
        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .current_dir(temp_dir.path())
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("No Git repository found")
                    .or(predicate::str::contains("failed")),
            );

        // Test rebase command outside git repo
        let mut cmd = termai_cmd!();
        cmd.arg("rebase")
            .arg("status")
            .current_dir(temp_dir.path())
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("No Git repository found")
                    .or(predicate::str::contains("failed")),
            );

        // Test conflicts command outside git repo
        let mut cmd = termai_cmd!();
        cmd.arg("conflicts")
            .arg("detect")
            .current_dir(temp_dir.path())
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("No Git repository found")
                    .or(predicate::str::contains("failed")),
            );
    }

    #[test]
    fn test_repository_type_detection() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        // Test Rust project detection
        setup_rust_project(repo_path).expect("Failed to setup Rust project");

        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .arg("--suggest-name")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Repository type: Rust Project"))
            .stdout(predicate::str::contains("perf/optimization"))
            .stdout(predicate::str::contains("feature/new-module"));

        // Test Node.js project detection
        fs::write(
            repo_path.join("package.json"),
            r#"{
  "name": "test-project",
  "version": "1.0.0",
  "description": "Test Node.js project"
}"#,
        )
        .expect("Failed to create package.json");

        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .arg("--suggest-name")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("Repository type: Node.js Project"));
    }

    #[test]
    fn test_commit_type_analysis() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");
        create_test_commits(repo_path).expect("Failed to create test commits");

        // Test rebase analyze command to check commit type detection
        let mut cmd = termai_cmd!();
        cmd.arg("rebase")
            .arg("analyze")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("feat"))
            .stdout(predicate::str::contains("fix"))
            .stdout(predicate::str::contains("test"))
            .stdout(predicate::str::contains("Commit Type Distribution"));
    }

    #[test]
    fn test_ai_suggestions_quality() {
        let temp_repo = setup_test_repo().expect("Failed to setup test repo");
        let repo_path = temp_repo.path();

        setup_rust_project(repo_path).expect("Failed to setup Rust project");

        // Create auth-related files for context analysis
        fs::create_dir_all(repo_path.join("src/auth")).expect("Failed to create auth dir");
        fs::write(
            repo_path.join("src/auth/mod.rs"),
            r#"pub mod oauth;
pub mod session;
"#,
        )
        .expect("Failed to create auth mod.rs");

        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .assert()
            .success();

        // Test branch naming with auth context
        let mut cmd = termai_cmd!();
        cmd.arg("branch-summary")
            .arg("--suggest-name")
            .arg("--context")
            .arg("authentication improvements")
            .current_dir(repo_path)
            .assert()
            .success()
            .stdout(predicate::str::contains("authentication"))
            .stdout(predicate::str::contains("auth"))
            .stdout(predicate::str::contains("File-based Analysis"));
    }

    #[test]
    fn test_command_help_integration() {
        // Test that all Git commands have proper help integration
        let commands = vec![
            vec!["tag", "--help"],
            vec!["rebase", "--help"],
            vec!["conflicts", "--help"],
            vec!["branch-summary", "--help"],
        ];

        for cmd_args in commands {
            let mut cmd = termai_cmd!();
            for arg in cmd_args {
                cmd.arg(arg);
            }

            cmd.assert()
                .success()
                .stdout(predicate::str::contains("Usage:"));
        }
    }
}
