//! Integration tests for the Ralph CLI

use assert_cmd::cargo;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Get a Command for the ralph binary
fn ralph() -> Command {
    Command::new(cargo::cargo_bin!("ralph"))
}

#[test]
fn test_help() {
    ralph()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Autonomous Claude Code execution"));
}

#[test]
fn test_version() {
    ralph()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn test_bootstrap_creates_structure() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success()
        .stdout(predicate::str::contains("Automation suite bootstrapped"));

    // Verify key files were created
    assert!(temp.path().join(".claude/CLAUDE.md").exists());
    assert!(temp.path().join(".claude/settings.json").exists());
    assert!(temp.path().join(".claude/mcp.json").exists());
    assert!(temp.path().join("IMPLEMENTATION_PLAN.md").exists());
    assert!(temp.path().join("PROMPT_build.md").exists());
}

#[test]
fn test_context_stats_only() {
    let temp = TempDir::new().unwrap();

    // Bootstrap first
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Run context with stats-only
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("context")
        .arg("--stats-only")
        .assert()
        .success()
        .stdout(predicate::str::contains("files_included"));
}

#[test]
fn test_context_builds_file() {
    let temp = TempDir::new().unwrap();

    // Bootstrap first
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    let output_file = temp.path().join("context.txt");

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("context")
        .arg("--output")
        .arg(&output_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Context built"));

    assert!(output_file.exists());
}

#[test]
fn test_archive_stats() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("archive")
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total_docs"));
}

#[test]
fn test_archive_list_stale() {
    let temp = TempDir::new().unwrap();

    // Create docs directory
    std::fs::create_dir_all(temp.path().join("docs")).unwrap();
    std::fs::write(temp.path().join("docs/test.md"), "test").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("archive")
        .arg("list-stale")
        .arg("--stale-days")
        .arg("0")
        .assert()
        .success();
}

#[test]
fn test_analytics_sessions_empty() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("sessions")
        .assert()
        .success()
        .stdout(predicate::str::contains("No sessions found"));
}

#[test]
fn test_analytics_aggregate_empty() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("aggregate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Total sessions: 0"));
}

#[test]
fn test_analytics_log_and_retrieve() {
    let temp = TempDir::new().unwrap();

    // Log an event
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("log")
        .arg("--session")
        .arg("test-session")
        .arg("--event")
        .arg("test_event")
        .arg("--data")
        .arg(r#"{"test": true}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("Event logged"));

    // Retrieve sessions
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("sessions")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("test-session"));
}

#[test]
fn test_hook_validate_safe_command() {
    ralph()
        .arg("hook")
        .arg("validate")
        .arg("git status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Command is safe"));
}

#[test]
fn test_hook_validate_dangerous_command() {
    ralph()
        .arg("hook")
        .arg("validate")
        .arg("rm -rf /")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Blocked"));
}

#[test]
fn test_hook_run_security_filter() {
    ralph()
        .arg("hook")
        .arg("run")
        .arg("security-filter")
        .arg("git commit -m test")
        .assert()
        .success();
}

#[test]
fn test_hook_run_security_filter_blocks() {
    ralph()
        .arg("hook")
        .arg("run")
        .arg("security-filter")
        .arg("chmod 777 /")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Blocked"));
}

#[test]
fn test_hook_scan_clean_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("clean.txt");
    std::fs::write(&file_path, "This is clean content").unwrap();

    ralph()
        .arg("hook")
        .arg("scan")
        .arg(&file_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("No secrets found"));
}

#[test]
fn test_hook_scan_file_with_secret() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("secrets.txt");
    std::fs::write(&file_path, r#"api_key = "sk-secret123""#).unwrap();

    ralph()
        .arg("hook")
        .arg("scan")
        .arg(&file_path)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("potential secrets"));
}

#[test]
fn test_config_paths() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("paths")
        .assert()
        .success()
        .stdout(predicate::str::contains("Settings:"))
        .stdout(predicate::str::contains("CLAUDE.md:"))
        .stdout(predicate::str::contains("Analytics:"));
}

#[test]
fn test_config_validate_no_config() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("settings.json not found"));
}

#[test]
fn test_config_validate_with_config() {
    let temp = TempDir::new().unwrap();

    // Bootstrap to create config
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

#[test]
fn test_config_show() {
    let temp = TempDir::new().unwrap();

    // Bootstrap to create config
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("Respect gitignore"));
}

#[test]
fn test_config_show_json() {
    let temp = TempDir::new().unwrap();

    // Bootstrap to create config
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("show")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("respectGitignore")); // Uses serde rename
}

#[test]
fn test_analyze_generates_artifacts() {
    let temp = TempDir::new().unwrap();
    let output_dir = temp.path().join("analysis");

    // Bootstrap first
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analyze")
        .arg("--output-dir")
        .arg(&output_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Analyzing project"));

    // Check that analysis artifacts were created
    assert!(output_dir.exists());
    let files: Vec<_> = std::fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(files.len() >= 2); // At least context and docs files
}

#[test]
fn test_loop_requires_plan() {
    let temp = TempDir::new().unwrap();

    // Try to run loop without IMPLEMENTATION_PLAN.md
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--max-iterations")
        .arg("1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("IMPLEMENTATION_PLAN.md not found"));
}

#[test]
fn test_verbose_flag() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--verbose")
        .arg("--project")
        .arg(temp.path())
        .arg("archive")
        .arg("stats")
        .assert()
        .success();
}

#[test]
fn test_nonexistent_project() {
    ralph()
        .arg("--project")
        .arg("/nonexistent/path/that/does/not/exist")
        .arg("bootstrap")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_hook_validate_respects_config_allow_list() {
    let temp = TempDir::new().unwrap();

    // Create config with restrictive allow list (only git commands)
    std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
    std::fs::write(
        temp.path().join(".claude/settings.json"),
        r#"{"permissions": {"allow": ["Bash(git *)"], "deny": []}}"#,
    )
    .unwrap();

    // git status should be allowed
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("hook")
        .arg("validate")
        .arg("git status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Command is safe"));

    // npm install should be blocked by the config (not in allow list)
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("hook")
        .arg("validate")
        .arg("npm install")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("denied by project permissions"));
}

#[test]
fn test_hook_validate_respects_config_deny_list() {
    let temp = TempDir::new().unwrap();

    // Create config that allows everything except npm
    std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
    std::fs::write(
        temp.path().join(".claude/settings.json"),
        r#"{"permissions": {"allow": ["Bash(*)"], "deny": ["Bash(npm *)"]}}"#,
    )
    .unwrap();

    // git status should be allowed
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("hook")
        .arg("validate")
        .arg("git status")
        .assert()
        .success();

    // npm install should be blocked by deny list
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("hook")
        .arg("validate")
        .arg("npm install")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("denied by project permissions"));
}

// ============================================================
// Bootstrap Language Override Tests (Sprint 9a)
// ============================================================

#[test]
fn test_bootstrap_detect_only() {
    let temp = TempDir::new().unwrap();

    // Create a Python project
    std::fs::write(temp.path().join("main.py"), "print('hello')").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--detect-only")
        .assert()
        .success()
        .stdout(predicate::str::contains("Detected languages"))
        .stdout(predicate::str::contains("Python"));

    // Verify NO files were created (detect-only should not bootstrap)
    assert!(!temp.path().join(".claude/CLAUDE.md").exists());
    assert!(!temp.path().join("IMPLEMENTATION_PLAN.md").exists());
}

#[test]
fn test_bootstrap_detect_only_empty_project() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--detect-only")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No programming languages detected",
        ));

    // Verify NO files were created
    assert!(!temp.path().join(".claude").exists());
}

#[test]
fn test_bootstrap_with_language_override() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--language")
        .arg("rust")
        .assert()
        .success()
        .stdout(predicate::str::contains("Automation suite bootstrapped"))
        .stdout(predicate::str::contains("Override languages"))
        .stdout(predicate::str::contains("Rust"));

    // Verify files were created
    assert!(temp.path().join(".claude/CLAUDE.md").exists());
}

#[test]
fn test_bootstrap_with_multiple_languages() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--language")
        .arg("rust")
        .arg("--language")
        .arg("python")
        .arg("--language")
        .arg("typescript")
        .assert()
        .success()
        .stdout(predicate::str::contains("Override languages"))
        .stdout(predicate::str::contains("Rust"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("TypeScript"));
}

#[test]
fn test_bootstrap_with_invalid_language() {
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--language")
        .arg("notareallanguage")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown language"));
}

#[test]
fn test_bootstrap_language_shorthand() {
    let temp = TempDir::new().unwrap();

    // Test common shorthand aliases
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--language")
        .arg("ts")  // shorthand for TypeScript
        .arg("--language")
        .arg("py")  // shorthand for Python
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("Python"));
}

// ============================================================
// CLI Integration Tests for Polyglot Support (Sprint 7, Phase 7.5)
// ============================================================

#[test]
fn test_detect_shows_all_detected_languages_with_confidence() {
    // Test: `ralph detect` shows all detected languages with confidence
    // Expected behavior: A dedicated `detect` command that lists languages with confidence scores
    let temp = TempDir::new().unwrap();

    // Create a Python project
    std::fs::write(temp.path().join("main.py"), "print('hello')").unwrap();
    std::fs::write(temp.path().join("utils.py"), "def foo(): pass").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("Detected languages"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("confidence"));
}

#[test]
fn test_detect_shows_gate_availability() {
    // Test: `ralph detect --show-gates` shows which gates are available for each language
    // Expected behavior: After showing languages, show which gates are available
    let temp = TempDir::new().unwrap();

    // Create a Rust project
    std::fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/lib.rs"), "// test").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .arg("--show-gates")
        .assert()
        .success()
        .stdout(predicate::str::contains("Rust"))
        .stdout(predicate::str::contains("Available gates"));
}

#[test]
fn test_detect_empty_project() {
    // Test: `ralph detect` handles empty projects gracefully
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No programming languages detected",
        ));
}

#[test]
fn test_detect_polyglot_project() {
    // Test: `ralph detect` shows multiple languages in polyglot projects
    let temp = TempDir::new().unwrap();

    // Create a polyglot project (Rust + Python)
    std::fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/lib.rs"), "// test").unwrap();
    std::fs::write(temp.path().join("script.py"), "print('hello')").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("Rust"))
        .stdout(predicate::str::contains("Python"));
}

#[test]
fn test_bootstrap_reports_detected_languages_and_gates() {
    // Test: `ralph bootstrap` reports detected languages and selected gates during setup
    // Expected behavior: Bootstrap output includes which languages were detected and which gates will be used
    let temp = TempDir::new().unwrap();

    // Create a Rust project
    std::fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/lib.rs"), "// test").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success()
        .stdout(predicate::str::contains("Detected languages"))
        .stdout(predicate::str::contains("Rust"))
        .stdout(predicate::str::contains("Selected gates"));
}

// ============================================================
// Verify Command Tests (Sprint 19, Phase 19.4)
// ============================================================

#[test]
fn test_verify_command_with_mock_flag() {
    // Test: `ralph verify --mock` runs verification with MockCcgVerifier
    // Expected behavior: Returns mock verification report showing quality improved
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("verify")
        .arg("--mock")
        .assert()
        .success()
        .stdout(predicate::str::contains("Verification Report"))
        .stdout(predicate::str::contains("Quality improved"));
}

#[test]
fn test_verify_command_outputs_json() {
    // Test: `ralph verify --mock --json` outputs verification report in JSON format
    // Expected behavior: JSON output with quality_improved, deltas, findings fields
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("verify")
        .arg("--mock")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("quality_improved"))
        .stdout(predicate::str::contains("deltas"))
        .stdout(predicate::str::contains("findings"));
}

#[test]
fn test_verify_command_outputs_markdown() {
    // Test: `ralph verify --mock --markdown` outputs verification report in Markdown format
    // Expected behavior: Markdown output with headers and formatted content
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("verify")
        .arg("--mock")
        .arg("--markdown")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Verification Report"))
        .stdout(predicate::str::contains("## Quality Deltas"));
}

#[test]
fn test_verify_command_writes_output_to_file() {
    // Test: `ralph verify --mock --output <file>` writes report to specified file
    // Expected behavior: Report is written to the output file
    let temp = TempDir::new().unwrap();
    let output_file = temp.path().join("verification_report.json");

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("verify")
        .arg("--mock")
        .arg("--json")
        .arg("--output")
        .arg(&output_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Report written to"));

    // Verify file was created and contains valid JSON
    assert!(output_file.exists());
    let content = std::fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("quality_improved"));
}

#[test]
fn test_verify_command_help_text() {
    // Test: `ralph verify --help` shows verification purpose in help text
    // Expected behavior: Help describes CCG-Diff verification purpose
    ralph()
        .arg("verify")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Verify"))
        .stdout(predicate::str::contains("quality"))
        .stdout(predicate::str::contains("--mock"));
}

#[test]
fn test_verify_command_requires_mock_flag() {
    // Test: `ralph verify` without --mock fails (until real verifier is implemented)
    // Expected behavior: Error message explaining --mock is required
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("verify")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--mock"));
}

#[test]
fn test_show_gates_flag() {
    // Test: `ralph --show-gates` lists available gates for project
    // Expected behavior: A flag that just shows gates without running any commands
    let temp = TempDir::new().unwrap();

    // Create a Rust project
    std::fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/lib.rs"), "// test").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .arg("--show-gates")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available gates"))
        .stdout(predicate::str::contains("Clippy"));
}

// ============================================================
// Analytics Dashboard Tests (Sprint 25, Phase 25.4)
// ============================================================

#[test]
fn test_analytics_dashboard_generates_html() {
    // Test: `ralph analytics dashboard` generates valid HTML dashboard
    // Expected behavior: Generates HTML output with dashboard content
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("dashboard")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dashboard written to"));
}

#[test]
fn test_analytics_dashboard_creates_output_file() {
    // Test: `ralph analytics dashboard --output <path>` creates the file
    // Expected behavior: HTML file is created at specified path
    let temp = TempDir::new().unwrap();
    let output_file = temp.path().join("my-dashboard.html");

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("dashboard")
        .arg("--output")
        .arg(&output_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dashboard written to"));

    // Verify file was created and contains valid HTML
    assert!(output_file.exists());
    let content = std::fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("<!DOCTYPE html>"));
    assert!(content.contains("Ralph Analytics Dashboard"));
}

#[test]
fn test_analytics_dashboard_default_output_path() {
    // Test: `ralph analytics dashboard` uses default output path
    // Expected behavior: Creates .ralph/dashboard.html by default
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("dashboard")
        .assert()
        .success()
        .stdout(predicate::str::contains(".ralph/dashboard.html"));

    // Verify default file was created
    assert!(temp.path().join(".ralph/dashboard.html").exists());
}

#[test]
fn test_analytics_dashboard_with_sessions_filter() {
    // Test: `ralph analytics dashboard --sessions 5` filters to last N sessions
    // Expected behavior: Dashboard is generated with session filter applied
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("dashboard")
        .arg("--sessions")
        .arg("5")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dashboard written to"));
}

#[test]
fn test_analytics_dashboard_json_output() {
    // Test: `ralph analytics dashboard --json` outputs raw dashboard data as JSON
    // Expected behavior: JSON output instead of HTML file
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("analytics")
        .arg("dashboard")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""))
        .stdout(predicate::str::contains("\"sessions\""));
}

#[test]
fn test_analytics_dashboard_help() {
    // Test: `ralph analytics dashboard --help` shows usage information
    // Expected behavior: Help text describes the dashboard command options
    ralph()
        .arg("analytics")
        .arg("dashboard")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("dashboard"))
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--sessions"));
}

// ============================================================
// Incremental Execution CLI Tests (Sprint 26, Phase 26.5)
// ============================================================

#[test]
fn test_loop_help_shows_files_flag() {
    // Test: `ralph loop --help` shows --files flag
    // Expected behavior: Help text includes --files option with glob description
    ralph()
        .arg("loop")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--files"))
        .stdout(predicate::str::contains("glob"));
}

#[test]
fn test_loop_help_shows_changed_flag() {
    // Test: `ralph loop --help` shows --changed flag (distinct from --changed-since)
    // Expected behavior: Help text includes --changed option as shorthand for --changed-since HEAD~1
    // The regex ensures we match "--changed" as a standalone flag (followed by newline), not --changed-since
    ralph()
        .arg("loop")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"--changed\n").unwrap())
        .stdout(predicate::str::contains(
            "Shorthand for --changed-since HEAD~1",
        ));
}

#[test]
fn test_loop_files_and_changed_since_mutually_exclusive() {
    // Test: `ralph loop --files <glob> --changed-since <commit>` errors
    // Expected behavior: Error message explaining the flags are mutually exclusive
    let temp = TempDir::new().unwrap();

    // Create minimal project with IMPLEMENTATION_PLAN.md
    std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

    // Initialize git repo so --changed-since can work
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to init git");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to config git");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to config git");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .expect("Failed to add");
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to commit");

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--files")
        .arg("*.rs")
        .arg("--changed-since")
        .arg("HEAD~1")
        .arg("--max-iterations")
        .arg("1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_loop_files_and_changed_mutually_exclusive() {
    // Test: `ralph loop --files <glob> --changed` errors
    // Expected behavior: Error message explaining the flags are mutually exclusive
    let temp = TempDir::new().unwrap();

    // Create minimal project with IMPLEMENTATION_PLAN.md
    std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--files")
        .arg("*.rs")
        .arg("--changed")
        .arg("--max-iterations")
        .arg("1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_loop_changed_since_and_changed_mutually_exclusive() {
    // Test: `ralph loop --changed-since <commit> --changed` errors
    // Expected behavior: Error message explaining the flags are mutually exclusive
    let temp = TempDir::new().unwrap();

    // Create minimal project with IMPLEMENTATION_PLAN.md
    std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

    // Initialize git repo so --changed-since can work
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to init git");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to config git");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to config git");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .expect("Failed to add");
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to commit");

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--changed-since")
        .arg("HEAD~1")
        .arg("--changed")
        .arg("--max-iterations")
        .arg("1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_loop_files_flag_logs_scope() {
    // Test: `ralph loop --files <glob>` logs the scope information
    // Expected behavior: Log message showing files matched by glob pattern
    let temp = TempDir::new().unwrap();

    // Create minimal project
    std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
    std::fs::write(temp.path().join("test.rs"), "fn main() {}").unwrap();
    std::fs::write(temp.path().join("lib.rs"), "// lib").unwrap();

    // Bootstrap the project so config exists
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Check that the flag is recognized and the scope message appears
    // Note: tracing output goes to stdout in test configuration
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--files")
        .arg("*.rs")
        .arg("--max-iterations")
        .arg("0")
        .assert()
        .success()
        .stdout(predicate::str::contains("incremental mode"))
        .stdout(predicate::str::contains("files matched"));
}

#[test]
fn test_loop_changed_flag_logs_scope() {
    // Test: `ralph loop --changed` logs the scope information
    // Expected behavior: Log message showing files changed since HEAD~1
    let temp = TempDir::new().unwrap();

    // Create minimal project
    std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to init git");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to config git");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to config git");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .expect("Failed to add");
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to commit");

    // Bootstrap the project
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Add another commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .expect("Failed to add");
    std::process::Command::new("git")
        .args(["commit", "-m", "bootstrap"])
        .current_dir(temp.path())
        .output()
        .expect("Failed to commit");

    // Check that the flag is recognized and the scope message appears
    // Note: tracing output goes to stdout in test configuration
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--changed")
        .arg("--max-iterations")
        .arg("0")
        .assert()
        .success()
        .stdout(predicate::str::contains("incremental mode"))
        .stdout(predicate::str::contains("HEAD~1"));
}
