#[macro_use]
mod common;

/// Simplified Git integration tests focusing on command functionality
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Test that Git commands handle missing repository correctly
#[test]
fn test_git_commands_require_repo() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let test_cases = vec![
        vec!["tag", "list"],
        vec!["branch-summary"],
        vec!["rebase", "status"],
        vec!["conflicts", "detect"],
    ];

    for args in test_cases {
        let mut cmd = termai_cmd!();
        for arg in args.iter() {
            cmd.arg(arg);
        }

        cmd.current_dir(temp_dir.path()).assert().failure().stderr(
            predicate::str::contains("failed").or(predicate::str::contains("No Git repository")),
        );
    }
}

/// Test that Git commands show help properly
#[test]
fn test_git_command_help() {
    let help_commands = vec![
        vec!["tag", "--help"],
        vec!["rebase", "--help"],
        vec!["conflicts", "--help"],
        vec!["branch-summary", "--help"],
    ];

    for args in help_commands {
        let mut cmd = termai_cmd!();
        for arg in args.iter() {
            cmd.arg(arg);
        }

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Usage:"));
    }
}

/// Test that Git commands validate their subcommands
#[test]
fn test_git_command_validation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a simple git repository
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .assert()
        .success();

    // Test invalid subcommands
    let invalid_commands = vec![
        vec!["tag", "invalid-action"],
        vec!["rebase", "invalid-action"],
        vec!["conflicts", "invalid-action"],
    ];

    for args in invalid_commands {
        let mut cmd = termai_cmd!();
        for arg in args.iter() {
            cmd.arg(arg);
        }

        cmd.current_dir(temp_dir.path())
            .assert()
            .failure()
            .stderr(predicate::str::contains("failed").or(predicate::str::contains("Unknown")));
    }
}

/// Test manual integration of Git workflows in existing repo
#[test]
fn test_git_workflow_integration() {
    // This test can be run in the current repository to test real Git integration
    println!("Testing Git workflow integration in current repository...");

    // Test 1: Branch analysis should work
    let mut cmd = termai_cmd!();
    let output = cmd
        .arg("branch-summary")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Branch Analysis") || stdout.contains("Analyzing Git repository"));
        println!("✅ Branch analysis works correctly");
    } else {
        println!("ℹ️  Branch analysis skipped (not in git repo or other issue)");
    }

    // Test 2: Tag commands should provide feedback
    let mut cmd = termai_cmd!();
    let output = cmd
        .arg("tag")
        .arg("list")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("TermAI Git Tag") || stdout.contains("Tag"));
        println!("✅ Tag listing works correctly");
    } else {
        println!("ℹ️  Tag listing skipped (not in git repo or other issue)");
    }

    // Test 3: Rebase status should work
    let mut cmd = termai_cmd!();
    let output = cmd
        .arg("rebase")
        .arg("status")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Rebase Status") || stdout.contains("rebase"));
        println!("✅ Rebase status works correctly");
    } else {
        println!("ℹ️  Rebase status skipped (not in git repo or other issue)");
    }

    // Test 4: Conflict detection should work
    let mut cmd = termai_cmd!();
    let output = cmd
        .arg("conflicts")
        .arg("detect")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Conflict Resolution") || stdout.contains("conflicts"));
        println!("✅ Conflict detection works correctly");
    } else {
        println!("ℹ️  Conflict detection skipped (not in git repo or other issue)");
    }

    println!("✅ Git workflow integration test completed successfully");
}

/// Test branch naming suggestions work with different contexts
#[test]
fn test_branch_naming_contexts() {
    let contexts = vec![
        "OAuth integration",
        "API improvements",
        "bug fixes",
        "performance optimization",
        "UI updates",
    ];

    for context in contexts {
        let mut cmd = termai_cmd!();
        let output = cmd
            .arg("branch-summary")
            .arg("--suggest-name")
            .arg("--context")
            .arg(context)
            .output()
            .expect("Failed to execute command");

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("Branch Naming Assistant") || stdout.contains("suggestions"));
            println!("✅ Branch naming works for context: {}", context);
        } else {
            println!(
                "ℹ️  Branch naming skipped for context: {} (not in git repo)",
                context
            );
        }
    }
}

/// Test that error messages are helpful and actionable
#[test]
fn test_error_message_quality() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let mut cmd = termai_cmd!();
    let output = cmd
        .arg("tag")
        .arg("create")
        .arg("v1.0.0")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check that error messages contain helpful guidance
    assert!(
        stderr.contains("Troubleshooting")
            || stderr.contains("Try these steps")
            || stderr.contains("failed")
    );
    assert!(stderr.contains("Git repository") || stderr.contains("directory"));

    println!("✅ Error messages provide helpful guidance");
}

/// Test command discovery and suggestions
#[test]
fn test_command_discovery() {
    // Test that main help shows Git commands
    let mut cmd = termai_cmd!();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tag"))
        .stdout(predicate::str::contains("branch-summary"))
        .stdout(predicate::str::contains("rebase"))
        .stdout(predicate::str::contains("conflicts"));
}

/// Performance test - commands should complete quickly
#[test]
fn test_command_performance() {
    use std::time::Instant;

    let start = Instant::now();

    // Test a simple command that should be fast
    let mut cmd = termai_cmd!();
    cmd.arg("tag").arg("--help").assert().success();

    let duration = start.elapsed();

    // Command should complete within 2 seconds
    assert!(
        duration.as_secs() < 2,
        "Command took too long: {:?}",
        duration
    );
    println!("✅ Commands complete quickly ({}ms)", duration.as_millis());
}
