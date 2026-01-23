//! CLI integration tests for checkpoint diff functionality (Phase 11.3).
//!
//! These tests verify the `ralph checkpoint diff` command works correctly
//! with various scenarios and output formats.

use assert_cmd::cargo;
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;

/// Get a Command for the ralph binary
fn ralph() -> Command {
    Command::new(cargo::cargo_bin!("ralph"))
}

/// Helper to create a checkpoint JSON file in the checkpoint directory.
fn create_checkpoint_file(
    checkpoint_dir: &std::path::Path,
    id: &str,
    test_total: u32,
    test_passed: u32,
    test_failed: u32,
    clippy_warnings: u32,
    files_modified: &[&str],
) {
    let checkpoint = json!({
        "id": id,
        "created_at": "2024-01-15T10:00:00Z",
        "description": format!("Checkpoint {}", id),
        "git_hash": "abc123",
        "git_branch": "main",
        "metrics": {
            "clippy_warnings": clippy_warnings,
            "test_total": test_total,
            "test_passed": test_passed,
            "test_failed": test_failed,
            "security_issues": 0,
            "allow_annotations": 0,
            "todo_comments": 0,
            "lines_of_code": null,
            "test_coverage": null
        },
        "metrics_by_language": {},
        "task_tracker_state": null,
        "iteration": 1,
        "verified": false,
        "tags": [],
        "files_modified": files_modified
    });

    let path = checkpoint_dir.join(format!("{}.json", id));
    std::fs::write(path, serde_json::to_string_pretty(&checkpoint).unwrap()).unwrap();
}

// ============================================================
// Phase 11.3: Checkpoint Diff Visualization Tests
// ============================================================

#[test]
fn test_cli_checkpoint_diff_shows_test_count_changes() {
    // Test: diff shows test count changes between checkpoints
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    // Create two checkpoints with different test counts
    create_checkpoint_file(
        &checkpoint_dir,
        "cp_from",
        10, // test_total
        8,  // test_passed
        2,  // test_failed
        5,  // clippy_warnings
        &[],
    );
    create_checkpoint_file(
        &checkpoint_dir,
        "cp_to",
        15, // test_total increased
        14, // test_passed increased
        1,  // test_failed decreased
        3,  // clippy_warnings decreased
        &[],
    );

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_from")
        .arg("cp_to")
        .assert()
        .success()
        .stdout(predicate::str::contains("Tests total"))
        .stdout(predicate::str::contains("+5")); // test_total increased by 5
}

#[test]
fn test_cli_checkpoint_diff_shows_lint_warning_changes() {
    // Test: diff shows lint warning changes between checkpoints
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    // Create checkpoints with different warning counts
    create_checkpoint_file(&checkpoint_dir, "cp_before", 10, 10, 0, 20, &[]); // 20 warnings
    create_checkpoint_file(&checkpoint_dir, "cp_after", 10, 10, 0, 5, &[]); // 5 warnings (improved)

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_before")
        .arg("cp_after")
        .assert()
        .success()
        .stdout(predicate::str::contains("Clippy warnings"))
        .stdout(predicate::str::contains("-15")); // warnings decreased by 15
}

#[test]
fn test_cli_checkpoint_diff_shows_files_modified_between_checkpoints() {
    // Test: diff shows files modified between checkpoints
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    // Create checkpoints with different file lists
    create_checkpoint_file(&checkpoint_dir, "cp_start", 10, 10, 0, 0, &["src/main.rs"]);
    create_checkpoint_file(
        &checkpoint_dir,
        "cp_end",
        12,
        12,
        0,
        0,
        &["src/main.rs", "src/lib.rs", "tests/test.rs"],
    );

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_start")
        .arg("cp_end")
        .assert()
        .success()
        .stdout(predicate::str::contains("Files"));
}

#[test]
fn test_cli_checkpoint_diff_json_output_is_machine_readable() {
    // Test: diff output is machine-readable JSON when --json flag is used
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    create_checkpoint_file(&checkpoint_dir, "cp_a", 10, 10, 0, 5, &["file1.rs"]);
    create_checkpoint_file(&checkpoint_dir, "cp_b", 15, 14, 1, 3, &["file1.rs", "file2.rs"]);

    let output = ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_a")
        .arg("cp_b")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).expect("Output should be valid UTF-8");

    // Verify it's valid JSON
    let json: serde_json::Value =
        serde_json::from_str(&output_str).expect("Output should be valid JSON");

    // Verify required fields exist
    assert!(json.get("from_id").is_some(), "Should have from_id field");
    assert!(json.get("to_id").is_some(), "Should have to_id field");
    assert!(
        json.get("test_total_delta").is_some(),
        "Should have test_total_delta field"
    );
    assert!(
        json.get("clippy_warnings_delta").is_some(),
        "Should have clippy_warnings_delta field"
    );
}

#[test]
fn test_cli_checkpoint_diff_can_compare_arbitrary_checkpoint_ids() {
    // Test: diff can compare arbitrary checkpoint IDs (not just sequential)
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    // Create three checkpoints
    create_checkpoint_file(&checkpoint_dir, "cp_001", 5, 5, 0, 10, &[]);
    create_checkpoint_file(&checkpoint_dir, "cp_002", 10, 10, 0, 8, &[]);
    create_checkpoint_file(&checkpoint_dir, "cp_003", 15, 15, 0, 2, &[]);

    // Compare first and last (skipping middle)
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_001")
        .arg("cp_003")
        .assert()
        .success()
        .stdout(predicate::str::contains("cp_001"))
        .stdout(predicate::str::contains("cp_003"));

    // Compare in reverse order
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_003")
        .arg("cp_001")
        .assert()
        .success()
        .stdout(predicate::str::contains("cp_003"))
        .stdout(predicate::str::contains("cp_001"));
}

#[test]
fn test_cli_checkpoint_diff_not_found() {
    // Test: diff returns error when checkpoint not found
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    create_checkpoint_file(&checkpoint_dir, "cp_exists", 10, 10, 0, 0, &[]);

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_exists")
        .arg("cp_nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_cli_checkpoint_diff_shows_improvement_regression() {
    // Test: diff clearly shows improvements vs regressions
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    // Create checkpoints where quality improved
    create_checkpoint_file(&checkpoint_dir, "cp_worse", 10, 5, 5, 20, &[]); // 5 failures, 20 warnings
    create_checkpoint_file(&checkpoint_dir, "cp_better", 20, 20, 0, 2, &[]); // 0 failures, 2 warnings

    let output = ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("cp_worse")
        .arg("cp_better")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).expect("Valid UTF-8");

    // Should show improvements (positive indicators for tests, negative for warnings)
    assert!(
        output_str.contains('+') || output_str.contains('-'),
        "Should show delta indicators"
    );
}

#[test]
fn test_cli_checkpoint_diff_empty_directory() {
    // Test: diff handles empty checkpoint directory gracefully
    let temp = TempDir::new().unwrap();
    let checkpoint_dir = temp.path().join(".ralph/checkpoints");
    std::fs::create_dir_all(&checkpoint_dir).unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("checkpoint")
        .arg("diff")
        .arg("any_id")
        .arg("other_id")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
