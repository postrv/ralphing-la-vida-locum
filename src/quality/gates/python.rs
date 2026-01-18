//! Python-specific quality gate implementations.
//!
//! This module provides quality gates for Python projects using standard tooling:
//! - [`RuffGate`] - Runs `ruff` linter (with flake8 fallback)
//! - [`PytestGate`] - Runs `pytest` test suite
//! - [`MypyGate`] - Runs `mypy` type checker
//! - [`BanditGate`] - Runs `bandit` security scanner

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use super::{GateIssue, IssueSeverity, QualityGate};

// ============================================================================
// Public Factory Function
// ============================================================================

/// Returns all standard quality gates for Python projects.
///
/// The returned gates include:
/// - Ruff (linting, with flake8 fallback)
/// - Pytest (unit/integration tests)
/// - Mypy (type checking)
/// - Bandit (security scanning)
#[must_use]
pub fn python_gates() -> Vec<Box<dyn QualityGate>> {
    vec![
        Box::new(RuffGate::new()),
        Box::new(PytestGate::new()),
        Box::new(MypyGate::new()),
        Box::new(BanditGate::new()),
    ]
}

// ============================================================================
// Ruff Gate
// ============================================================================

/// Quality gate that runs `ruff` linter with flake8 fallback.
///
/// Ruff is a fast Python linter written in Rust that can replace flake8,
/// isort, and other tools. If ruff is not available, falls back to flake8.
pub struct RuffGate {
    /// Additional arguments to pass to ruff.
    extra_args: Vec<String>,
}

impl RuffGate {
    /// Create a new ruff gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse ruff JSON output into gate issues.
    fn parse_ruff_json(&self, stdout: &str) -> Vec<GateIssue> {
        // Ruff JSON output format:
        // [{"code": "E501", "message": "Line too long", "filename": "foo.py", "row": 10, "column": 80}]
        let mut issues = Vec::new();

        if let Ok(parsed) = serde_json::from_str::<Vec<RuffDiagnostic>>(stdout) {
            for diag in parsed {
                let severity = Self::severity_from_code(&diag.code);
                let mut issue = GateIssue::new(severity, &diag.message)
                    .with_code(&diag.code);

                if let Some(row) = diag.location.as_ref().map(|l| l.row) {
                    issue = issue.with_location(&diag.filename, row);
                    if let Some(col) = diag.location.as_ref().and_then(|l| l.column) {
                        issue = issue.with_column(col);
                    }
                } else {
                    issue.file = Some(PathBuf::from(&diag.filename));
                }

                if let Some(fix) = &diag.fix {
                    if let Some(msg) = &fix.message {
                        issue = issue.with_suggestion(msg);
                    }
                }

                issues.push(issue);
            }
        }

        issues
    }

    /// Parse flake8 output (fallback).
    fn parse_flake8_output(&self, stdout: &str) -> Vec<GateIssue> {
        // Flake8 default output format: path:row:col: CODE message
        let mut issues = Vec::new();

        for line in stdout.lines() {
            if let Some(issue) = Self::parse_flake8_line(line) {
                issues.push(issue);
            }
        }

        issues
    }

    /// Parse a single flake8 output line.
    fn parse_flake8_line(line: &str) -> Option<GateIssue> {
        // Format: path/to/file.py:10:5: E501 line too long
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let file = parts[0];
        let row: u32 = parts[1].parse().ok()?;
        let col: u32 = parts[2].parse().ok()?;
        let rest = parts[3].trim();

        // Split code from message
        let (code, message) = if let Some(space_pos) = rest.find(' ') {
            (rest[..space_pos].to_string(), rest[space_pos + 1..].to_string())
        } else {
            (String::new(), rest.to_string())
        };

        let severity = Self::severity_from_code(&code);
        let mut issue = GateIssue::new(severity, message)
            .with_location(file, row)
            .with_column(col);

        if !code.is_empty() {
            issue = issue.with_code(code);
        }

        Some(issue)
    }

    /// Determine severity from ruff/flake8 error code.
    fn severity_from_code(code: &str) -> IssueSeverity {
        // E/F/S codes are errors (E=pycodestyle errors, F=pyflakes, S=security)
        // Everything else is a warning
        if code.starts_with('E') || code.starts_with('F') || code.starts_with('S') {
            IssueSeverity::Error
        } else {
            IssueSeverity::Warning
        }
    }

    /// Try running flake8 as fallback.
    fn run_flake8(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let output = Command::new("flake8")
            .args([".", "--format=default"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    Ok(self.parse_flake8_output(&stdout))
                }
            }
            Err(_) => {
                // Neither ruff nor flake8 available
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "No Python linter available (install ruff or flake8)",
                )])
            }
        }
    }
}

impl Default for RuffGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for RuffGate {
    fn name(&self) -> &str {
        "Ruff"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["check", ".", "--output-format=json"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("ruff")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues = self.parse_ruff_json(&stdout);
                    if issues.is_empty() && !stdout.is_empty() {
                        // JSON parsing failed, try line-by-line
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            format!("Ruff reported issues: {}", stdout.lines().next().unwrap_or("")),
                        )])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // Ruff not installed - try flake8 fallback
                self.run_flake8(project_dir)
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues.iter().filter(|i| i.severity == IssueSeverity::Error).count();
        let warning_count = issues.iter().filter(|i| i.severity == IssueSeverity::Warning).count();

        format!(
            r#"## Ruff Linting Issues

Found {} errors and {} warnings.

**How to fix:**
1. Run `ruff check . --fix` to auto-fix many issues
2. For remaining issues, see https://docs.astral.sh/ruff/rules/
3. Run `ruff check .` to verify all fixed

**Common fixes:**
- Unused imports: remove or use the import
- Line too long: break into multiple lines
- Missing docstrings: add module/function docstrings
"#,
            error_count, warning_count
        )
    }
}

// ============================================================================
// Pytest Gate
// ============================================================================

/// Quality gate that runs `pytest` test suite.
pub struct PytestGate {
    /// Additional arguments to pass to pytest.
    extra_args: Vec<String>,
}

impl PytestGate {
    /// Create a new pytest gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse pytest output to extract failure information.
    fn parse_output(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();
        let combined = format!("{}\n{}", stdout, stderr);

        // Look for FAILED lines: FAILED tests/test_foo.py::test_bar - AssertionError
        for line in combined.lines() {
            if line.starts_with("FAILED ") {
                let rest = line.trim_start_matches("FAILED ");
                let (test_path, message) = if let Some(dash_pos) = rest.find(" - ") {
                    (&rest[..dash_pos], &rest[dash_pos + 3..])
                } else {
                    (rest, "Test failed")
                };

                issues.push(
                    GateIssue::new(IssueSeverity::Error, format!("Test failed: {}", message))
                        .with_code("pytest_failure")
                        .with_suggestion(format!("Run `pytest {} -v` for details", test_path)),
                );

                // Try to extract file location from test path
                if let Some(double_colon) = test_path.find("::") {
                    let file = &test_path[..double_colon];
                    issues.last_mut().unwrap().file = Some(PathBuf::from(file));
                }
            }
        }

        // Also check for collection errors
        for line in combined.lines() {
            if line.contains("ERROR collecting") || line.contains("ImportError") {
                issues.push(
                    GateIssue::new(IssueSeverity::Error, line.to_string())
                        .with_code("pytest_collection_error"),
                );
            }
        }

        issues
    }
}

impl Default for PytestGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for PytestGate {
    fn name(&self) -> &str {
        "Pytest"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["--tb=short", "-q"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("pytest")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);

                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues = self.parse_output(&stdout, &stderr);
                    if issues.is_empty() {
                        // Couldn't parse specific failures
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            "Pytest failed (run `pytest -v` for details)",
                        )
                        .with_code("pytest_failure")])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // Pytest not installed
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "Pytest not available (install with `pip install pytest`)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let failed_tests: Vec<_> = issues
            .iter()
            .filter(|i| i.code.as_deref() == Some("pytest_failure"))
            .filter_map(|i| i.file.as_ref().map(|f| f.display().to_string()))
            .collect();

        format!(
            r#"## Pytest Failures

{} test(s) failed.

**Failed test files:**
{}

**How to fix:**
1. Run `pytest -v` to see detailed failure output
2. Fix each failing test
3. Run `pytest` to verify all pass
"#,
            issues.len(),
            if failed_tests.is_empty() {
                "- (unable to determine specific files)".to_string()
            } else {
                failed_tests.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
            }
        )
    }
}

// ============================================================================
// Mypy Gate
// ============================================================================

/// Quality gate that runs `mypy` type checker.
pub struct MypyGate {
    /// Additional arguments to pass to mypy.
    extra_args: Vec<String>,
}

impl MypyGate {
    /// Create a new mypy gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse mypy output into gate issues.
    fn parse_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // Mypy output format: path.py:10: error: Description [error-code]
        for line in stdout.lines() {
            if let Some(issue) = Self::parse_mypy_line(line) {
                issues.push(issue);
            }
        }

        issues
    }

    /// Parse a single mypy output line.
    fn parse_mypy_line(line: &str) -> Option<GateIssue> {
        // Format: path/to/file.py:10: error: Message [error-code]
        // Or:     path/to/file.py:10:5: error: Message [error-code]
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() < 3 {
            return None;
        }

        let file = parts[0];
        let row: u32 = parts[1].parse().ok()?;

        // Check if there's a column number
        let rest = parts[2].trim();
        let (col, message_part) = if let Some((col_str, remaining)) = rest.split_once(':') {
            if let Ok(col_num) = col_str.parse::<u32>() {
                (Some(col_num), remaining.trim())
            } else {
                (None, rest)
            }
        } else {
            (None, rest)
        };

        // Parse severity and message
        let (severity, message, code) = if let Some(err_pos) = message_part.find("error:") {
            let msg = message_part[err_pos + 6..].trim();
            let (msg_text, code) = Self::extract_error_code(msg);
            (IssueSeverity::Error, msg_text, code)
        } else if let Some(warn_pos) = message_part.find("warning:") {
            let msg = message_part[warn_pos + 8..].trim();
            let (msg_text, code) = Self::extract_error_code(msg);
            (IssueSeverity::Warning, msg_text, code)
        } else if let Some(note_pos) = message_part.find("note:") {
            let msg = message_part[note_pos + 5..].trim();
            (IssueSeverity::Info, msg.to_string(), None)
        } else {
            return None;
        };

        let mut issue = GateIssue::new(severity, message).with_location(file, row);

        if let Some(c) = col {
            issue = issue.with_column(c);
        }

        if let Some(c) = code {
            issue = issue.with_code(c);
        }

        Some(issue)
    }

    /// Extract error code from mypy message (e.g., "Message [error-code]").
    fn extract_error_code(msg: &str) -> (String, Option<String>) {
        if let (Some(bracket_start), Some(bracket_end)) = (msg.rfind('['), msg.rfind(']')) {
            if bracket_end > bracket_start {
                let code = msg[bracket_start + 1..bracket_end].to_string();
                let message = msg[..bracket_start].trim().to_string();
                return (message, Some(code));
            }
        }
        (msg.to_string(), None)
    }
}

impl Default for MypyGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for MypyGate {
    fn name(&self) -> &str {
        "Mypy"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["."];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("mypy")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues = self.parse_output(&stdout);
                    if issues.is_empty() && !stdout.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            "Mypy reported type errors (run `mypy .` for details)",
                        )])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // Mypy not installed - this is non-blocking as type checking is optional
                Ok(Vec::new())
            }
        }
    }

    fn is_blocking(&self) -> bool {
        // Type checking is important but not always blocking
        // Many Python projects don't have full type coverage
        true
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues.iter().filter(|i| i.severity == IssueSeverity::Error).count();

        format!(
            r#"## Mypy Type Errors

Found {} type errors.

**How to fix:**
1. Run `mypy .` to see all type errors
2. Add type annotations to fix inference issues
3. Use `# type: ignore` sparingly for false positives
4. Consider using `--ignore-missing-imports` for third-party libraries

**Common fixes:**
- Missing return type: add `-> ReturnType` to function signature
- Incompatible types: ensure argument types match expected types
- Missing annotations: add type hints to function parameters
"#,
            error_count
        )
    }
}

// ============================================================================
// Bandit Gate
// ============================================================================

/// Quality gate that runs `bandit` security scanner.
pub struct BanditGate {
    /// Minimum severity to report.
    severity_threshold: BanditSeverity,
    /// Additional arguments to pass to bandit.
    extra_args: Vec<String>,
}

/// Bandit severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BanditSeverity {
    /// Low severity issues.
    Low,
    /// Medium severity issues.
    Medium,
    /// High severity issues.
    High,
}

impl BanditGate {
    /// Create a new bandit gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            severity_threshold: BanditSeverity::Medium,
            extra_args: Vec::new(),
        }
    }

    /// Set the minimum severity threshold.
    #[must_use]
    pub fn with_threshold(mut self, threshold: BanditSeverity) -> Self {
        self.severity_threshold = threshold;
        self
    }

    /// Create with additional arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Parse bandit JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        if let Ok(report) = serde_json::from_str::<BanditReport>(stdout) {
            for result in report.results {
                let severity = match result.issue_severity.to_uppercase().as_str() {
                    "HIGH" => IssueSeverity::Critical,
                    "MEDIUM" => IssueSeverity::Error,
                    "LOW" => IssueSeverity::Warning,
                    _ => IssueSeverity::Info,
                };

                // Filter by threshold
                let bandit_sev = match result.issue_severity.to_uppercase().as_str() {
                    "HIGH" => BanditSeverity::High,
                    "MEDIUM" => BanditSeverity::Medium,
                    _ => BanditSeverity::Low,
                };

                if bandit_sev < self.severity_threshold {
                    continue;
                }

                let mut issue = GateIssue::new(severity, &result.issue_text)
                    .with_location(&result.filename, result.line_number)
                    .with_code(&result.test_id);

                if let Some(more_info) = &result.more_info {
                    issue = issue.with_suggestion(format!("See: {}", more_info));
                }

                issues.push(issue);
            }
        }

        issues
    }
}

impl Default for BanditGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for BanditGate {
    fn name(&self) -> &str {
        "Bandit"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["-r", ".", "-f", "json"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("bandit")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                // Bandit returns exit code 1 if it finds issues
                let issues = self.parse_json_output(&stdout);
                Ok(issues)
            }
            Err(_) => {
                // Bandit not installed - return warning
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "Bandit not available (install with `pip install bandit`)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let critical = issues.iter().filter(|i| i.severity == IssueSeverity::Critical).count();
        let high = issues.iter().filter(|i| i.severity == IssueSeverity::Error).count();

        format!(
            r#"## Bandit Security Issues

Found {} critical and {} high severity security issues.

**How to fix:**
1. Run `bandit -r . -f txt` to see detailed findings
2. Address each vulnerability at the reported location
3. Use `# nosec` comments sparingly for false positives

**Common issues:**
- Hardcoded passwords: use environment variables or secrets management
- SQL injection: use parameterized queries
- Shell injection: avoid shell=True, use subprocess with list args
- Insecure hashing: use hashlib with secure algorithms (SHA-256+)
"#,
            critical, high
        )
    }
}

// ============================================================================
// JSON Parsing Structures
// ============================================================================

/// Ruff JSON diagnostic structure.
#[derive(Debug, serde::Deserialize)]
struct RuffDiagnostic {
    code: String,
    message: String,
    filename: String,
    location: Option<RuffLocation>,
    fix: Option<RuffFix>,
}

/// Ruff location within a file.
#[derive(Debug, serde::Deserialize)]
struct RuffLocation {
    row: u32,
    column: Option<u32>,
}

/// Ruff fix suggestion.
#[derive(Debug, serde::Deserialize)]
struct RuffFix {
    message: Option<String>,
}

/// Bandit JSON report structure.
#[derive(Debug, serde::Deserialize)]
struct BanditReport {
    results: Vec<BanditResult>,
}

/// Bandit individual result.
#[derive(Debug, serde::Deserialize)]
struct BanditResult {
    filename: String,
    test_id: String,
    issue_text: String,
    issue_severity: String,
    line_number: u32,
    more_info: Option<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // RuffGate tests
    // =========================================================================

    #[test]
    fn test_ruff_gate_name() {
        let gate = RuffGate::new();
        assert_eq!(gate.name(), "Ruff");
    }

    #[test]
    fn test_ruff_gate_is_blocking() {
        let gate = RuffGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_ruff_gate_default() {
        let gate = RuffGate::default();
        assert_eq!(gate.name(), "Ruff");
    }

    #[test]
    fn test_ruff_gate_remediation() {
        let gate = RuffGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "error"),
            GateIssue::new(IssueSeverity::Warning, "warning"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Ruff"));
        assert!(remediation.contains("1 errors"));
        assert!(remediation.contains("1 warnings"));
    }

    #[test]
    fn test_ruff_parse_json_output() {
        let gate = RuffGate::new();

        let json = r#"[
            {
                "code": "E501",
                "message": "Line too long (100 > 88 characters)",
                "filename": "src/main.py",
                "location": {"row": 10, "column": 89},
                "fix": {"message": "Split line"}
            }
        ]"#;

        let issues = gate.parse_ruff_json(json);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("Line too long"));
        assert_eq!(issues[0].code, Some("E501".to_string()));
        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(89));
    }

    #[test]
    fn test_ruff_parse_flake8_output() {
        let gate = RuffGate::new();

        let output = "src/main.py:10:5: E501 line too long (100 > 88 characters)\n\
                      src/utils.py:20:1: W503 line break before binary operator";

        let issues = gate.parse_flake8_output(output);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(5));
        assert_eq!(issues[0].code, Some("E501".to_string()));

        assert_eq!(issues[1].severity, IssueSeverity::Warning);
        assert_eq!(issues[1].line, Some(20));
    }

    #[test]
    fn test_ruff_severity_from_code() {
        assert_eq!(RuffGate::severity_from_code("E501"), IssueSeverity::Error);
        assert_eq!(RuffGate::severity_from_code("W503"), IssueSeverity::Warning);
        assert_eq!(RuffGate::severity_from_code("F401"), IssueSeverity::Error);
        assert_eq!(RuffGate::severity_from_code("S101"), IssueSeverity::Error);
        assert_eq!(RuffGate::severity_from_code("B001"), IssueSeverity::Warning);
        assert_eq!(RuffGate::severity_from_code("C901"), IssueSeverity::Warning);
    }

    // =========================================================================
    // PytestGate tests
    // =========================================================================

    #[test]
    fn test_pytest_gate_name() {
        let gate = PytestGate::new();
        assert_eq!(gate.name(), "Pytest");
    }

    #[test]
    fn test_pytest_gate_is_blocking() {
        let gate = PytestGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_pytest_gate_default() {
        let gate = PytestGate::default();
        assert_eq!(gate.name(), "Pytest");
    }

    #[test]
    fn test_pytest_parse_failures() {
        let gate = PytestGate::new();

        let stdout = r#"
FAILED tests/test_foo.py::test_bar - AssertionError: expected 1, got 2
FAILED tests/test_baz.py::test_qux - ValueError: invalid input
"#;

        let issues = gate.parse_output(stdout, "");
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("AssertionError"));
        assert_eq!(issues[0].file, Some(PathBuf::from("tests/test_foo.py")));

        assert_eq!(issues[1].severity, IssueSeverity::Error);
        assert!(issues[1].message.contains("ValueError"));
    }

    #[test]
    fn test_pytest_gate_remediation() {
        let gate = PytestGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "Test failed")
                .with_code("pytest_failure"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Pytest"));
        assert!(remediation.contains("1 test(s) failed"));
    }

    // =========================================================================
    // MypyGate tests
    // =========================================================================

    #[test]
    fn test_mypy_gate_name() {
        let gate = MypyGate::new();
        assert_eq!(gate.name(), "Mypy");
    }

    #[test]
    fn test_mypy_gate_is_blocking() {
        let gate = MypyGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_mypy_gate_default() {
        let gate = MypyGate::default();
        assert_eq!(gate.name(), "Mypy");
    }

    #[test]
    fn test_mypy_parse_output() {
        let gate = MypyGate::new();

        let output = r#"src/main.py:10: error: Incompatible types [arg-type]
src/utils.py:20:5: error: Missing return type [no-untyped-def]
src/foo.py:30: warning: Unused variable [unused-ignore]"#;

        let issues = gate.parse_output(output);
        assert_eq!(issues.len(), 3);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("Incompatible types"));
        assert_eq!(issues[0].code, Some("arg-type".to_string()));
        assert_eq!(issues[0].line, Some(10));

        assert_eq!(issues[1].severity, IssueSeverity::Error);
        assert_eq!(issues[1].column, Some(5));

        assert_eq!(issues[2].severity, IssueSeverity::Warning);
    }

    #[test]
    fn test_mypy_extract_error_code() {
        let (msg, code) = MypyGate::extract_error_code("Incompatible types [arg-type]");
        assert_eq!(msg, "Incompatible types");
        assert_eq!(code, Some("arg-type".to_string()));

        let (msg2, code2) = MypyGate::extract_error_code("No error code here");
        assert_eq!(msg2, "No error code here");
        assert_eq!(code2, None);
    }

    #[test]
    fn test_mypy_gate_remediation() {
        let gate = MypyGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "type error"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Mypy"));
        assert!(remediation.contains("1 type errors"));
    }

    // =========================================================================
    // BanditGate tests
    // =========================================================================

    #[test]
    fn test_bandit_gate_name() {
        let gate = BanditGate::new();
        assert_eq!(gate.name(), "Bandit");
    }

    #[test]
    fn test_bandit_gate_is_blocking() {
        let gate = BanditGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_bandit_gate_default() {
        let gate = BanditGate::default();
        assert_eq!(gate.name(), "Bandit");
    }

    #[test]
    fn test_bandit_parse_json_output() {
        let gate = BanditGate::new();

        let json = r#"{
            "results": [
                {
                    "filename": "src/main.py",
                    "test_id": "B105",
                    "issue_text": "Possible hardcoded password",
                    "issue_severity": "HIGH",
                    "line_number": 15,
                    "more_info": "https://bandit.readthedocs.io/en/latest/plugins/b105.html"
                },
                {
                    "filename": "src/db.py",
                    "test_id": "B608",
                    "issue_text": "Possible SQL injection",
                    "issue_severity": "MEDIUM",
                    "line_number": 42,
                    "more_info": null
                }
            ]
        }"#;

        let issues = gate.parse_json_output(json);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Critical);
        assert!(issues[0].message.contains("hardcoded password"));
        assert_eq!(issues[0].code, Some("B105".to_string()));
        assert_eq!(issues[0].line, Some(15));

        assert_eq!(issues[1].severity, IssueSeverity::Error);
        assert!(issues[1].message.contains("SQL injection"));
    }

    #[test]
    fn test_bandit_severity_threshold() {
        let gate = BanditGate::new().with_threshold(BanditSeverity::High);

        let json = r#"{
            "results": [
                {
                    "filename": "src/main.py",
                    "test_id": "B105",
                    "issue_text": "High severity",
                    "issue_severity": "HIGH",
                    "line_number": 15,
                    "more_info": null
                },
                {
                    "filename": "src/db.py",
                    "test_id": "B608",
                    "issue_text": "Medium severity",
                    "issue_severity": "MEDIUM",
                    "line_number": 42,
                    "more_info": null
                }
            ]
        }"#;

        let issues = gate.parse_json_output(json);
        // Only HIGH severity should be included
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("High severity"));
    }

    #[test]
    fn test_bandit_gate_remediation() {
        let gate = BanditGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Critical, "critical issue"),
            GateIssue::new(IssueSeverity::Error, "high issue"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Bandit"));
        assert!(remediation.contains("1 critical"));
        assert!(remediation.contains("1 high"));
    }

    // =========================================================================
    // python_gates() factory tests
    // =========================================================================

    #[test]
    fn test_python_gates_returns_all_gates() {
        let gates = python_gates();
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"Ruff"));
        assert!(names.contains(&"Pytest"));
        assert!(names.contains(&"Mypy"));
        assert!(names.contains(&"Bandit"));
    }

    #[test]
    fn test_python_gates_are_send_sync() {
        let gates = python_gates();
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_python_gates_have_remediation() {
        let gates = python_gates();
        for gate in &gates {
            let issues = vec![GateIssue::new(IssueSeverity::Error, "test error")];
            let remediation = gate.remediation(&issues);
            assert!(
                !remediation.is_empty(),
                "Gate {} should provide remediation",
                gate.name()
            );
        }
    }
}
