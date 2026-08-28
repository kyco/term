#[macro_use]
mod common;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// Integration tests for Smart Context Discovery CLI functionality
//
// These tests verify that the --smart-context flag works correctly
// with various project types and configurations.

#[cfg(test)]
mod cli_integration_tests {
    use super::*;

    #[test]
    fn test_smart_context_flag_with_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create a realistic Rust project
        fs::write(
            path.join("Cargo.toml"),
            r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(
            path.join("src/main.rs"),
            r#"
fn main() {
    println!("Hello, world!");
}
"#,
        )
        .unwrap();

        fs::write(
            path.join("src/lib.rs"),
            r#"
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
"#,
        )
        .unwrap();

        // Test the CLI with --smart-context flag
        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("Explain the main function")
            .arg(path.to_str().unwrap());

        // The command should succeed (though it might fail due to missing API keys)
        // We mainly want to test that the smart context flag is recognized
        let output = cmd.output().unwrap();

        // Check that smart context processing was attempted
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should not show "unrecognized flag" error
        assert!(!stderr.contains("unrecognized"));
        assert!(!stderr.contains("unexpected argument"));

        // Should indicate smart context discovery was attempted
        // (even if it fails later due to API keys or other issues)
        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);
    }

    #[test]
    fn test_smart_context_with_preview_flag() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create a simple project
        fs::write(path.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("--preview-context")
            .arg("Test query")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not show flag parsing errors
        assert!(!stderr.contains("unrecognized"));
        assert!(!stderr.contains("unexpected"));
    }

    #[test]
    fn test_smart_context_with_max_tokens_flag() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create a project
        fs::write(path.join("package.json"), r#"{"name": "test"}"#).unwrap();
        fs::write(path.join("index.js"), "console.log('hello');").unwrap();

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("--max-context-tokens")
            .arg("2000")
            .arg("Explain this code")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not show flag parsing errors
        assert!(!stderr.contains("unrecognized"));
        assert!(!stderr.contains("unexpected"));
        assert!(!stderr.contains("invalid value"));
    }

    #[test]
    fn test_smart_context_with_session() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        fs::write(path.join("main.py"), "print('hello')").unwrap();
        fs::write(path.join("requirements.txt"), "requests==2.28.0").unwrap();

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("--session")
            .arg("test-session")
            .arg("Explain the Python code")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not show flag parsing errors
        assert!(!stderr.contains("unrecognized"));
        assert!(!stderr.contains("unexpected"));
    }

    #[test]
    fn test_smart_context_with_custom_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create project with custom config
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();

        fs::write(
            path.join(".termai.toml"),
            r#"
[context]
max_tokens = 3000
include = ["src/**/*.rs"]
exclude = ["target/**"]

[project]
type = "rust"
"#,
        )
        .unwrap();

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("Add error handling")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not show configuration errors
        assert!(!stderr.contains("Failed to parse"));
        assert!(!stderr.contains("Invalid configuration"));
    }

    #[test]
    fn test_smart_context_help() {
        let mut cmd = termai_cmd!();
        cmd.arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("--smart-context"))
            .stdout(predicate::str::contains("Enable smart context discovery"));
    }
}

#[cfg(test)]
mod configuration_tests {
    use super::*;

    #[test]
    fn test_termai_config_file_validation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Test valid configuration
        fs::write(
            path.join(".termai.toml"),
            r#"
[context]
max_tokens = 4000
include = ["*.rs", "*.js", "*.py"]
exclude = ["target/**", "node_modules/**"]
priority_patterns = ["main.*", "index.*"]
enable_cache = true

[project]
type = "rust"
entry_points = ["src/main.rs"]
"#,
        )
        .unwrap();

        // Create a simple project to test with
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("Test query")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not show TOML parsing errors
        assert!(!stderr.contains("Failed to parse .termai.toml"));
        assert!(!stderr.contains("invalid toml"));
    }

    #[test]
    fn test_invalid_config_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create invalid TOML
        fs::write(path.join(".termai.toml"), "invalid toml content [[[").unwrap();
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("Test query")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined_output = format!("{}\n{}", stdout, stderr);

        // Should show configuration error but not crash
        // The exact error message may vary, but should not be a panic
        assert!(!combined_output.contains("thread panicked"));
        assert!(!combined_output.contains("panic"));
        assert!(!combined_output.contains("unrecognized"));
        assert!(!combined_output.contains("unexpected"));

        if !output.status.success() {
            assert!(
                combined_output.contains("Validation Error")
                    || combined_output.contains("Failed to parse .termai.toml")
                    || combined_output.contains("configuration"),
                "Unexpected failure output:\nSTDOUT:\n{}\nSTDERR:\n{}",
                stdout,
                stderr
            );
        }
    }
}

#[cfg(test)]
mod project_type_detection_cli_tests {
    use super::*;

    fn create_project_and_test_cli(project_files: &[(&str, &str)], expected_to_work: bool) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create project files
        for (file_path, content) in project_files {
            if file_path.contains('/') {
                let parent = std::path::Path::new(file_path).parent().unwrap();
                fs::create_dir_all(path.join(parent)).unwrap();
            }
            fs::write(path.join(file_path), content).unwrap();
        }

        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("Explain the project structure")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);

        if expected_to_work {
            // Should not show project detection errors
            assert!(!stderr.contains("Failed to detect project type"));
            assert!(!stderr.contains("Unsupported project"));
        }

        // Should never crash with unrecognized flags
        assert!(!stderr.contains("unrecognized"));
    }

    #[test]
    fn test_cli_with_rust_project() {
        create_project_and_test_cli(
            &[
                ("Cargo.toml", "[package]\nname = \"test\""),
                ("src/main.rs", "fn main() {}"),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_javascript_project() {
        create_project_and_test_cli(
            &[
                ("package.json", r#"{"name": "test"}"#),
                ("index.js", "console.log('hello');"),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_python_project() {
        create_project_and_test_cli(
            &[
                ("pyproject.toml", "[project]\nname = \"test\""),
                ("main.py", "print('hello')"),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_go_project() {
        create_project_and_test_cli(
            &[
                ("go.mod", "module test\n\ngo 1.21"),
                ("main.go", "package main\n\nfunc main() {}"),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_java_project() {
        create_project_and_test_cli(
            &[
                (
                    "pom.xml",
                    "<project><modelVersion>4.0.0</modelVersion></project>",
                ),
                (
                    "src/main/java/Main.java",
                    "public class Main { public static void main(String[] args) {} }",
                ),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_kotlin_project() {
        create_project_and_test_cli(
            &[
                (
                    "build.gradle.kts",
                    "plugins { kotlin(\"jvm\") version \"1.9.0\" }",
                ),
                ("src/main/kotlin/Main.kt", "fun main() {}"),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_git_project() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create git repository structure
        fs::create_dir_all(path.join(".git")).unwrap();
        fs::write(path.join(".gitignore"), "*.log\ntarget/").unwrap();
        fs::write(path.join("README.md"), "# Test Repo").unwrap();

        create_project_and_test_cli(
            &[
                (".gitignore", "*.log\ntarget/"),
                ("README.md", "# Test Repo"),
            ],
            true,
        );
    }

    #[test]
    fn test_cli_with_unknown_project_type() {
        create_project_and_test_cli(
            &[
                ("random.txt", "random content"),
                ("data.csv", "col1,col2\n1,2"),
            ],
            false,
        ); // May not work as well, but shouldn't crash
    }
}

#[cfg(test)]
mod chunking_integration_tests {
    use super::*;

    #[test]
    fn test_chunking_with_large_project() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Create a large Rust project
        fs::write(
            path.join("Cargo.toml"),
            "[package]\nname = \"large-project\"",
        )
        .unwrap();
        fs::create_dir_all(path.join("src")).unwrap();

        // Create many source files
        for i in 0..50 {
            let content = format!(
                r#"
// Module {}
pub fn function_{}() -> i32 {{
    // This is a very long function with lots of documentation
    // and implementation details that will consume many tokens
    // when analyzed by the smart context discovery system.
    // We want to test that the system can handle large projects
    // by intelligently chunking the content across multiple requests.
    let mut result = {};
    for j in 0..100 {{
        result += j * 2;
        if result > 1000 {{
            result = result % 1000;
        }}
    }}
    result
}}

pub struct DataStructure{} {{
    pub field1: String,
    pub field2: i32,
    pub field3: Vec<String>,
}}

impl DataStructure{} {{
    pub fn new() -> Self {{
        Self {{
            field1: String::from("default"),
            field2: 42,
            field3: vec![],
        }}
    }}
    
    pub fn process_data(&mut self) -> Result<(), String> {{
        // More implementation details here
        self.field2 *= 2;
        self.field3.push(format!("processed_{{}}", self.field2));
        Ok(())
    }}
}}
"#,
                i, i, i, i, i
            );
            fs::write(path.join(format!("src/module_{}.rs", i)), content).unwrap();
        }

        fs::write(
            path.join("src/main.rs"),
            r#"
// Main entry point for large project
use std::collections::HashMap;

fn main() {
    println!("Large project starting");
    let mut data = HashMap::new();
    
    // Initialize various modules
    for i in 0..50 {
        data.insert(i, format!("module_{}", i));
    }
    
    // Process all modules
    for (id, name) in &data {
        println!("Processing module {} with name {}", id, name);
    }
    
    println!("All modules processed successfully");
}
"#,
        )
        .unwrap();

        // Test with low token limit to force chunking
        let mut cmd = termai_cmd!();
        cmd.arg("--smart-context")
            .arg("--max-context-tokens")
            .arg("1000") // Very low limit to force chunking
            .arg("Analyze the project structure and suggest improvements")
            .arg(path.to_str().unwrap());

        let output = cmd.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should handle chunking gracefully
        assert!(!stderr.contains("token limit exceeded"));
        assert!(!stderr.contains("panic"));
        assert!(!stderr.contains("thread panicked"));

        println!("Large project test - STDOUT: {}", stdout);
        println!("Large project test - STDERR: {}", stderr);
    }
}
