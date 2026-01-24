//! Go-specific quality gate implementations.
//!
//! This module provides quality gates for Go projects using standard tooling:
//! - [`GoVetGate`] - Runs `go vet` static analysis
//! - [`GolangciLintGate`] - Runs `golangci-lint` linter
//! - [`GoTestGate`] - Runs `go test` test suite
//! - [`GovulncheckGate`] - Runs `govulncheck` security scanner

use std::path::Path;
use std::process::Command;

use anyhow::Result;

use super::{GateIssue, IssueSeverity, QualityGate};

// ============================================================================
// Public Factory Function
// ============================================================================

/// Returns all standard quality gates for Go projects.
///
/// The returned gates include:
/// - GoVet (static analysis)
/// - GolangciLint (comprehensive linting)
/// - GoTest (unit/integration tests)
/// - Govulncheck (security vulnerability scanning)
#[must_use]
pub fn go_gates() -> Vec<Box<dyn QualityGate>> {
    vec![
        Box::new(GoVetGate::new()),
        Box::new(GolangciLintGate::new()),
        Box::new(GoTestGate::new()),
        Box::new(GovulncheckGate::new()),
    ]
}

// ============================================================================
// GoVet Gate
// ============================================================================

/// Quality gate that runs `go vet` static analysis.
///
/// `go vet` examines Go source code and reports suspicious constructs, such as
/// Printf calls whose arguments do not align with the format string.
pub struct GoVetGate {
    /// Additional arguments to pass to go vet.
    extra_args: Vec<String>,
}

impl GoVetGate {
    /// Create a new go vet gate with default configuration.
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

    /// Parse go vet output into gate issues.
    fn parse_output(&self, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // go vet output format: path/file.go:line:col: message
        for line in stderr.lines() {
            if let Some(issue) = Self::parse_vet_line(line) {
                issues.push(issue);
            }
        }

        issues
    }

    /// Parse a single go vet output line.
    fn parse_vet_line(line: &str) -> Option<GateIssue> {
        // Format: path/to/file.go:10:5: message
        // Or:     path/to/file.go:10: message (no column)
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 3 {
            return None;
        }

        let file = parts[0];
        // Skip lines that don't look like file paths
        if !file.ends_with(".go") {
            return None;
        }

        let row: u32 = parts[1].parse().ok()?;

        // Check if we have a column number
        let (col, message) = if parts.len() == 4 {
            if let Ok(col_num) = parts[2].parse::<u32>() {
                (Some(col_num), parts[3].trim().to_string())
            } else {
                (
                    None,
                    format!("{}: {}", parts[2], parts[3]).trim().to_string(),
                )
            }
        } else {
            (None, parts[2].trim().to_string())
        };

        let mut issue = GateIssue::new(IssueSeverity::Error, &message)
            .with_location(file, row)
            .with_code("go-vet");

        if let Some(c) = col {
            issue = issue.with_column(c);
        }

        Some(issue)
    }
}

impl Default for GoVetGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for GoVetGate {
    fn name(&self) -> &str {
        "go-vet"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["vet", "./..."];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("go")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                // go vet outputs to stderr
                let stderr = String::from_utf8_lossy(&out.stderr);

                if out.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues = self.parse_output(&stderr);
                    if issues.is_empty() && !stderr.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            format!(
                                "go vet reported issues: {}",
                                stderr.lines().next().unwrap_or("")
                            ),
                        )])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // go not available
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "go not available (install Go from https://go.dev)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();

        format!(
            r#"## go vet Issues

Found {} issues.

**How to fix:**
1. Run `go vet ./...` to see all issues
2. Fix each issue at the reported location
3. Run `go vet ./...` again to verify

**Common issues:**
- Printf format mismatches: ensure format verbs match argument types
- Unreachable code: remove dead code after return/panic
- Shadowed variables: rename to avoid confusion
- Incorrect struct tags: fix JSON/XML tag syntax
"#,
            error_count
        )
    }

    fn required_tool(&self) -> Option<&str> {
        Some("go")
    }

    fn run_scoped(
        &self,
        project_dir: &Path,
        files: Option<&[std::path::PathBuf]>,
    ) -> Result<Vec<GateIssue>> {
        match files {
            None => self.run(project_dir),
            Some([]) => Ok(Vec::new()),
            Some(file_list) => {
                // Filter to only .go files
                let go_files: Vec<&std::path::PathBuf> = file_list
                    .iter()
                    .filter(|f| f.extension().is_some_and(|ext| ext == "go"))
                    .collect();

                if go_files.is_empty() {
                    return Ok(Vec::new());
                }

                // go vet can accept specific files
                let mut args: Vec<String> = vec!["vet".to_string()];
                for file in &go_files {
                    args.push(file.to_string_lossy().to_string());
                }
                args.extend(self.extra_args.iter().cloned());

                let output = Command::new("go")
                    .args(&args)
                    .current_dir(project_dir)
                    .output();

                match output {
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        if out.status.success() {
                            Ok(Vec::new())
                        } else {
                            let issues = self.parse_output(&stderr);
                            if issues.is_empty() && !stderr.is_empty() {
                                Ok(vec![GateIssue::new(
                                    IssueSeverity::Error,
                                    format!(
                                        "go vet reported issues: {}",
                                        stderr.lines().next().unwrap_or("")
                                    ),
                                )])
                            } else {
                                Ok(issues)
                            }
                        }
                    }
                    Err(_) => Ok(vec![GateIssue::new(
                        IssueSeverity::Warning,
                        "go not available for scoped check",
                    )]),
                }
            }
        }
    }
}

// ============================================================================
// GolangciLint Gate
// ============================================================================

/// Quality gate that runs `golangci-lint` linter.
///
/// `golangci-lint` is a fast, parallel linters runner for Go that aggregates
/// multiple linters including staticcheck, errcheck, gosimple, and more.
pub struct GolangciLintGate {
    /// Additional arguments to pass to golangci-lint.
    extra_args: Vec<String>,
}

impl GolangciLintGate {
    /// Create a new golangci-lint gate with default configuration.
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

    /// Parse golangci-lint JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        if let Ok(report) = serde_json::from_str::<GolangciLintReport>(stdout) {
            if let Some(lint_issues) = report.issues {
                for lint_issue in lint_issues {
                    let severity = Self::severity_from_linter(&lint_issue.from_linter);
                    let mut issue = GateIssue::new(severity, &lint_issue.text)
                        .with_code(&lint_issue.from_linter);

                    if let Some(ref pos) = lint_issue.pos {
                        issue = issue.with_location(&pos.filename, pos.line);
                        if let Some(col) = pos.column {
                            issue = issue.with_column(col);
                        }
                    }

                    if let Some(ref replacement) = lint_issue.replacement {
                        if let Some(ref new_text) = replacement.new_lines {
                            issue = issue
                                .with_suggestion(format!("Replace with:\n{}", new_text.join("\n")));
                        }
                    }

                    issues.push(issue);
                }
            }
        }

        issues
    }

    /// Determine severity from linter name.
    fn severity_from_linter(linter: &str) -> IssueSeverity {
        // Security and critical linters
        match linter {
            "gosec" | "errcheck" | "staticcheck" => IssueSeverity::Error,
            "unused" | "ineffassign" | "deadcode" => IssueSeverity::Error,
            "govet" | "typecheck" => IssueSeverity::Error,
            _ => IssueSeverity::Warning,
        }
    }

    /// Parse plain text output (fallback).
    fn parse_text_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // golangci-lint text format: path/file.go:line:col: linter: message
        for line in stdout.lines() {
            if let Some(issue) = Self::parse_text_line(line) {
                issues.push(issue);
            }
        }

        issues
    }

    /// Parse a single golangci-lint text output line.
    fn parse_text_line(line: &str) -> Option<GateIssue> {
        // Format: path/to/file.go:10:5: linter: message
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let file = parts[0];
        if !file.ends_with(".go") {
            return None;
        }

        let row: u32 = parts[1].parse().ok()?;
        let col: u32 = parts[2].parse().ok()?;

        let rest = parts[3..].join(":");
        let rest = rest.trim();

        // Try to extract linter name
        let (linter, message) = if let Some(space_pos) = rest.find(' ') {
            let potential_linter = &rest[..space_pos];
            // Check if it looks like a linter name (single word, no spaces)
            if potential_linter
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-')
            {
                (
                    potential_linter.to_string(),
                    rest[space_pos + 1..].trim_start().to_string(),
                )
            } else {
                (String::new(), rest.to_string())
            }
        } else {
            (String::new(), rest.to_string())
        };

        let severity = if linter.is_empty() {
            IssueSeverity::Warning
        } else {
            Self::severity_from_linter(&linter)
        };

        let mut issue = GateIssue::new(severity, &message)
            .with_location(file, row)
            .with_column(col);

        if !linter.is_empty() {
            issue = issue.with_code(&linter);
        }

        Some(issue)
    }
}

impl Default for GolangciLintGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for GolangciLintGate {
    fn name(&self) -> &str {
        "golangci-lint"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["run", "--out-format=json"];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("golangci-lint")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                // golangci-lint returns exit code 1 if there are issues
                let issues = self.parse_json_output(&stdout);
                if issues.is_empty() && !out.status.success() {
                    // Try text parsing as fallback
                    let text_issues = self.parse_text_output(&stdout);
                    if text_issues.is_empty() && !stdout.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Error,
                            format!(
                                "golangci-lint reported issues: {}",
                                stdout.lines().next().unwrap_or("")
                            ),
                        )])
                    } else {
                        Ok(text_issues)
                    }
                } else {
                    Ok(issues)
                }
            }
            Err(_) => {
                // golangci-lint not available - try go vet as fallback
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "golangci-lint not available (install from https://golangci-lint.run)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count();

        format!(
            r#"## golangci-lint Issues

Found {} errors and {} warnings.

**How to fix:**
1. Run `golangci-lint run` to see all issues
2. Run `golangci-lint run --fix` to auto-fix some issues
3. For remaining issues, fix manually at reported locations

**Common fixes:**
- errcheck: handle returned errors or explicitly ignore with `_ =`
- ineffassign: remove or use the assigned variable
- staticcheck: follow the suggested fix in the message
- unused: remove unused code or export if needed
"#,
            error_count, warning_count
        )
    }

    fn required_tool(&self) -> Option<&str> {
        Some("golangci-lint")
    }

    fn run_scoped(
        &self,
        project_dir: &Path,
        files: Option<&[std::path::PathBuf]>,
    ) -> Result<Vec<GateIssue>> {
        match files {
            None => self.run(project_dir),
            Some([]) => Ok(Vec::new()),
            Some(file_list) => {
                // Filter to only .go files
                let go_files: Vec<&std::path::PathBuf> = file_list
                    .iter()
                    .filter(|f| f.extension().is_some_and(|ext| ext == "go"))
                    .collect();

                if go_files.is_empty() {
                    return Ok(Vec::new());
                }

                // golangci-lint can accept specific files
                let mut args = vec!["run".to_string(), "--out-format=json".to_string()];
                for file in &go_files {
                    args.push(file.to_string_lossy().to_string());
                }
                args.extend(self.extra_args.iter().cloned());

                let output = Command::new("golangci-lint")
                    .args(&args)
                    .current_dir(project_dir)
                    .output();

                match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let issues = self.parse_json_output(&stdout);
                        if issues.is_empty() && !out.status.success() {
                            let text_issues = self.parse_text_output(&stdout);
                            if text_issues.is_empty() && !stdout.is_empty() {
                                Ok(vec![GateIssue::new(
                                    IssueSeverity::Error,
                                    format!(
                                        "golangci-lint reported issues: {}",
                                        stdout.lines().next().unwrap_or("")
                                    ),
                                )])
                            } else {
                                Ok(text_issues)
                            }
                        } else {
                            Ok(issues)
                        }
                    }
                    Err(_) => Ok(vec![GateIssue::new(
                        IssueSeverity::Warning,
                        "golangci-lint not available for scoped check",
                    )]),
                }
            }
        }
    }
}

// ============================================================================
// GoTest Gate
// ============================================================================

/// Quality gate that runs `go test` test suite.
pub struct GoTestGate {
    /// Additional arguments to pass to go test.
    extra_args: Vec<String>,
}

impl GoTestGate {
    /// Create a new go test gate with default configuration.
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

    /// Parse go test JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();
        let mut current_test: Option<String> = None;

        for line in stdout.lines() {
            if let Ok(event) = serde_json::from_str::<GoTestEvent>(line) {
                match event.action.as_str() {
                    "run" => {
                        current_test = event.test.clone();
                    }
                    "fail" => {
                        if let Some(ref test) = event.test {
                            let message = format!("Test failed: {}/{}", event.package, test);
                            let mut issue = GateIssue::new(IssueSeverity::Error, &message)
                                .with_code("go-test-failure");

                            // Try to extract file location from output
                            if let Some(ref output) = event.output {
                                if let Some((file, line)) = Self::extract_location(output) {
                                    issue = issue.with_location(&file, line);
                                }
                            }

                            issues.push(issue);
                        } else if current_test.is_none() {
                            // Package-level failure
                            issues.push(
                                GateIssue::new(
                                    IssueSeverity::Error,
                                    format!("Package failed: {}", event.package),
                                )
                                .with_code("go-test-package-failure"),
                            );
                        }
                    }
                    "output" => {
                        // Check for compilation errors
                        if let Some(ref output) = event.output {
                            if output.contains("cannot find package")
                                || output.contains("undefined:")
                                || output.contains("syntax error")
                            {
                                issues.push(
                                    GateIssue::new(IssueSeverity::Error, output.trim())
                                        .with_code("go-test-compile-error"),
                                );
                            }
                        }
                    }
                    _ => {}
                }

                // Clear current test on pass
                if event.action == "pass" && event.test.is_some() {
                    current_test = None;
                }
            }
        }

        issues
    }

    /// Extract file location from test output.
    fn extract_location(output: &str) -> Option<(String, u32)> {
        // Look for patterns like "file_test.go:123:"
        for word in output.split_whitespace() {
            if word.contains(".go:") {
                let parts: Vec<&str> = word.split(':').collect();
                if parts.len() >= 2 {
                    let file = parts[0];
                    if let Ok(line) = parts[1].parse::<u32>() {
                        return Some((file.to_string(), line));
                    }
                }
            }
        }
        None
    }

    /// Parse plain text output (fallback).
    fn parse_text_output(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();
        let combined = format!("{}\n{}", stdout, stderr);

        // Look for FAIL lines
        for line in combined.lines() {
            if line.starts_with("--- FAIL:") || line.starts_with("FAIL\t") {
                let message = if line.starts_with("--- FAIL:") {
                    line.trim_start_matches("--- FAIL:").trim()
                } else {
                    line.trim_start_matches("FAIL\t").trim()
                };

                issues.push(
                    GateIssue::new(IssueSeverity::Error, format!("Test failed: {}", message))
                        .with_code("go-test-failure"),
                );
            }
        }

        issues
    }
}

impl Default for GoTestGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for GoTestGate {
    fn name(&self) -> &str {
        "go-test"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["test", "-json", "./..."];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("go")
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
                    let issues = self.parse_json_output(&stdout);
                    if issues.is_empty() {
                        let text_issues = self.parse_text_output(&stdout, &stderr);
                        if text_issues.is_empty() {
                            Ok(vec![GateIssue::new(
                                IssueSeverity::Error,
                                "go test failed (run `go test ./...` for details)",
                            )
                            .with_code("go-test-failure")])
                        } else {
                            Ok(text_issues)
                        }
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => {
                // go not available
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "go not available (install Go from https://go.dev)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let failed_tests: Vec<_> = issues
            .iter()
            .filter(|i| i.code.as_deref() == Some("go-test-failure"))
            .map(|i| i.message.clone())
            .collect();

        format!(
            r#"## go test Failures

{} test(s) failed.

**Failed tests:**
{}

**How to fix:**
1. Run `go test -v ./...` to see detailed failure output
2. Fix each failing test
3. Run `go test ./...` to verify all pass

**Common issues:**
- Assertion failures: check expected vs actual values
- Nil pointer: ensure proper initialization
- Timeout: check for blocking operations or increase timeout
"#,
            issues.len(),
            if failed_tests.is_empty() {
                "- (unable to determine specific tests)".to_string()
            } else {
                failed_tests
                    .iter()
                    .map(|t| format!("- {}", t))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        )
    }

    fn required_tool(&self) -> Option<&str> {
        Some("go")
    }
}

// ============================================================================
// Govulncheck Gate
// ============================================================================

/// Quality gate that runs `govulncheck` security scanner.
///
/// `govulncheck` reports known vulnerabilities in Go dependencies and
/// checks if the vulnerable code is actually called by your application.
pub struct GovulncheckGate {
    /// Additional arguments to pass to govulncheck.
    extra_args: Vec<String>,
}

impl GovulncheckGate {
    /// Create a new govulncheck gate with default configuration.
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

    /// Parse govulncheck JSON output into gate issues.
    fn parse_json_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // govulncheck JSON output is newline-delimited JSON objects
        for line in stdout.lines() {
            if let Ok(msg) = serde_json::from_str::<GovulncheckMessage>(line) {
                if let Some(finding) = msg.finding {
                    let severity = match finding.trace.first() {
                        Some(trace) if trace.function.is_some() => IssueSeverity::Critical,
                        _ => IssueSeverity::Error,
                    };

                    let mut message = format!("{}: {}", finding.osv, finding.osv);
                    if let Some(ref fixed) = finding.fixed_version {
                        message = format!("{} (fixed in {})", message, fixed);
                    }

                    let mut issue = GateIssue::new(severity, &message).with_code(&finding.osv);

                    // Add file location from trace if available
                    if let Some(trace) = finding.trace.first() {
                        if let (Some(ref file), Some(line)) =
                            (&trace.position_file, trace.position_line)
                        {
                            issue = issue.with_location(file, line);
                        }
                    }

                    if let Some(ref fixed) = finding.fixed_version {
                        issue =
                            issue.with_suggestion(format!("Upgrade to version {} or later", fixed));
                    }

                    issues.push(issue);
                }
            }
        }

        issues
    }

    /// Parse plain text output (fallback).
    fn parse_text_output(&self, stdout: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // Look for vulnerability entries
        let mut current_vuln: Option<String> = None;

        for line in stdout.lines() {
            let trimmed = line.trim();

            // Vulnerability ID line: "Vulnerability #1: GO-2023-1234"
            if trimmed.starts_with("Vulnerability #") {
                if let Some(colon_pos) = trimmed.find(':') {
                    let vuln_id = trimmed[colon_pos + 1..].trim().to_string();
                    current_vuln = Some(vuln_id);
                }
            }
            // More info line with details
            else if trimmed.starts_with("More info:") {
                if let Some(ref vuln_id) = current_vuln {
                    let url = trimmed.trim_start_matches("More info:").trim();
                    issues.push(
                        GateIssue::new(
                            IssueSeverity::Critical,
                            format!("Vulnerability found: {}", vuln_id),
                        )
                        .with_code(vuln_id)
                        .with_suggestion(format!("See: {}", url)),
                    );
                }
                current_vuln = None;
            }
        }

        issues
    }
}

impl Default for GovulncheckGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for GovulncheckGate {
    fn name(&self) -> &str {
        "govulncheck"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["-json", "./..."];
        let extra_refs: Vec<&str> = self.extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        let output = Command::new("govulncheck")
            .args(&args)
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);

                // govulncheck returns exit code 3 if vulnerabilities are found
                let issues = self.parse_json_output(&stdout);
                if issues.is_empty() && !out.status.success() {
                    let text_issues = self.parse_text_output(&stdout);
                    if text_issues.is_empty() && !stdout.is_empty() {
                        // Check if it's just "No vulnerabilities found"
                        if stdout.contains("No vulnerabilities found") {
                            Ok(Vec::new())
                        } else {
                            Ok(vec![GateIssue::new(
                                IssueSeverity::Error,
                                "govulncheck reported issues (run `govulncheck ./...` for details)",
                            )])
                        }
                    } else {
                        Ok(text_issues)
                    }
                } else {
                    Ok(issues)
                }
            }
            Err(_) => {
                // govulncheck not available
                Ok(vec![GateIssue::new(
                    IssueSeverity::Warning,
                    "govulncheck not available (install with `go install golang.org/x/vuln/cmd/govulncheck@latest`)",
                )])
            }
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let critical = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Critical)
            .count();
        let high = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();

        format!(
            r#"## govulncheck Security Issues

Found {} critical and {} high severity vulnerabilities.

**How to fix:**
1. Run `govulncheck ./...` to see all vulnerabilities
2. Update affected dependencies to fixed versions
3. Run `go mod tidy` to clean up dependencies
4. Run `govulncheck ./...` again to verify

**Important:**
- Critical: Your code directly calls vulnerable functions
- High: Vulnerable code exists in dependencies but may not be called

**Steps to update:**
1. `go get package@version` to update specific packages
2. `go get -u ./...` to update all dependencies (use with caution)
3. Check release notes for breaking changes
"#,
            critical, high
        )
    }

    fn required_tool(&self) -> Option<&str> {
        Some("govulncheck")
    }
}

// ============================================================================
// JSON Parsing Structures
// ============================================================================

/// golangci-lint JSON report structure.
#[derive(Debug, serde::Deserialize)]
struct GolangciLintReport {
    #[serde(rename = "Issues")]
    issues: Option<Vec<GolangciLintIssue>>,
}

/// golangci-lint issue structure.
#[derive(Debug, serde::Deserialize)]
struct GolangciLintIssue {
    #[serde(rename = "FromLinter")]
    from_linter: String,
    #[serde(rename = "Text")]
    text: String,
    #[serde(rename = "Pos")]
    pos: Option<GolangciLintPos>,
    #[serde(rename = "Replacement")]
    replacement: Option<GolangciLintReplacement>,
}

/// golangci-lint position structure.
#[derive(Debug, serde::Deserialize)]
struct GolangciLintPos {
    #[serde(rename = "Filename")]
    filename: String,
    #[serde(rename = "Line")]
    line: u32,
    #[serde(rename = "Column")]
    column: Option<u32>,
}

/// golangci-lint replacement suggestion.
#[derive(Debug, serde::Deserialize)]
struct GolangciLintReplacement {
    #[serde(rename = "NewLines")]
    new_lines: Option<Vec<String>>,
}

/// go test JSON event structure.
#[derive(Debug, serde::Deserialize)]
struct GoTestEvent {
    #[serde(rename = "Action")]
    action: String,
    #[serde(rename = "Package")]
    package: String,
    #[serde(rename = "Test")]
    test: Option<String>,
    #[serde(rename = "Output")]
    output: Option<String>,
}

/// govulncheck JSON message structure.
#[derive(Debug, serde::Deserialize)]
struct GovulncheckMessage {
    finding: Option<GovulncheckFinding>,
}

/// govulncheck finding structure.
#[derive(Debug, serde::Deserialize)]
struct GovulncheckFinding {
    osv: String,
    #[serde(default)]
    fixed_version: Option<String>,
    #[serde(default)]
    trace: Vec<GovulncheckTrace>,
}

/// govulncheck trace structure.
#[derive(Debug, serde::Deserialize)]
struct GovulncheckTrace {
    function: Option<String>,
    #[serde(rename = "position")]
    position_file: Option<String>,
    #[serde(rename = "line")]
    position_line: Option<u32>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // GoVetGate tests
    // =========================================================================

    #[test]
    fn test_go_vet_gate_name() {
        let gate = GoVetGate::new();
        assert_eq!(gate.name(), "go-vet");
    }

    #[test]
    fn test_go_vet_gate_is_blocking() {
        let gate = GoVetGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_go_vet_gate_default() {
        let gate = GoVetGate::default();
        assert_eq!(gate.name(), "go-vet");
    }

    #[test]
    fn test_go_vet_gate_remediation() {
        let gate = GoVetGate::new();
        let issues = vec![GateIssue::new(IssueSeverity::Error, "error")];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("go vet"));
        assert!(remediation.contains("1 issues"));
    }

    #[test]
    fn test_go_vet_parse_output() {
        let gate = GoVetGate::new();

        let stderr = r#"./main.go:10:5: Printf format %d has arg of wrong type string
./util.go:20: unreachable code"#;

        let issues = gate.parse_output(stderr);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("Printf format"));
        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(5));

        assert_eq!(issues[1].severity, IssueSeverity::Error);
        assert!(issues[1].message.contains("unreachable"));
        assert_eq!(issues[1].line, Some(20));
    }

    #[test]
    fn test_go_vet_parse_line_with_column() {
        let issue =
            GoVetGate::parse_vet_line("pkg/handler.go:42:10: shadow: declaration of \"err\"")
                .unwrap();
        assert_eq!(issue.severity, IssueSeverity::Error);
        assert_eq!(issue.line, Some(42));
        assert_eq!(issue.column, Some(10));
        assert!(issue.message.contains("shadow"));
    }

    #[test]
    fn test_go_vet_parse_line_without_column() {
        let issue = GoVetGate::parse_vet_line("main.go:15: unreachable code").unwrap();
        assert_eq!(issue.severity, IssueSeverity::Error);
        assert_eq!(issue.line, Some(15));
        assert!(issue.column.is_none());
    }

    #[test]
    fn test_go_vet_parse_line_non_go_file() {
        let result = GoVetGate::parse_vet_line("README.md:10: some issue");
        assert!(result.is_none());
    }

    // =========================================================================
    // GolangciLintGate tests
    // =========================================================================

    #[test]
    fn test_golangci_lint_gate_name() {
        let gate = GolangciLintGate::new();
        assert_eq!(gate.name(), "golangci-lint");
    }

    #[test]
    fn test_golangci_lint_gate_is_blocking() {
        let gate = GolangciLintGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_golangci_lint_gate_default() {
        let gate = GolangciLintGate::default();
        assert_eq!(gate.name(), "golangci-lint");
    }

    #[test]
    fn test_golangci_lint_gate_remediation() {
        let gate = GolangciLintGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "error"),
            GateIssue::new(IssueSeverity::Warning, "warning"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("golangci-lint"));
        assert!(remediation.contains("1 errors"));
        assert!(remediation.contains("1 warnings"));
    }

    #[test]
    fn test_golangci_lint_parse_json_output() {
        let gate = GolangciLintGate::new();

        let json = r#"{
            "Issues": [
                {
                    "FromLinter": "errcheck",
                    "Text": "Error return value not checked",
                    "Pos": {
                        "Filename": "main.go",
                        "Line": 15,
                        "Column": 10
                    },
                    "Replacement": null
                },
                {
                    "FromLinter": "gofmt",
                    "Text": "File is not gofmt-ed",
                    "Pos": {
                        "Filename": "util.go",
                        "Line": 1,
                        "Column": null
                    },
                    "Replacement": {
                        "NewLines": ["package util", "", "import \"fmt\""]
                    }
                }
            ]
        }"#;

        let issues = gate.parse_json_output(json);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("Error return value"));
        assert_eq!(issues[0].code, Some("errcheck".to_string()));
        assert_eq!(issues[0].line, Some(15));
        assert_eq!(issues[0].column, Some(10));

        assert_eq!(issues[1].severity, IssueSeverity::Warning);
        assert!(issues[1].message.contains("gofmt"));
        assert!(issues[1].suggestion.is_some());
    }

    #[test]
    fn test_golangci_lint_severity_from_linter() {
        assert_eq!(
            GolangciLintGate::severity_from_linter("errcheck"),
            IssueSeverity::Error
        );
        assert_eq!(
            GolangciLintGate::severity_from_linter("staticcheck"),
            IssueSeverity::Error
        );
        assert_eq!(
            GolangciLintGate::severity_from_linter("gosec"),
            IssueSeverity::Error
        );
        assert_eq!(
            GolangciLintGate::severity_from_linter("gofmt"),
            IssueSeverity::Warning
        );
        assert_eq!(
            GolangciLintGate::severity_from_linter("golint"),
            IssueSeverity::Warning
        );
    }

    #[test]
    fn test_golangci_lint_parse_text_output() {
        let gate = GolangciLintGate::new();

        let output = "main.go:10:5: errcheck Error return value not checked\nutil.go:20:1: gofmt file is not formatted";

        let issues = gate.parse_text_output(output);
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(5));
    }

    // =========================================================================
    // GoTestGate tests
    // =========================================================================

    #[test]
    fn test_go_test_gate_name() {
        let gate = GoTestGate::new();
        assert_eq!(gate.name(), "go-test");
    }

    #[test]
    fn test_go_test_gate_is_blocking() {
        let gate = GoTestGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_go_test_gate_default() {
        let gate = GoTestGate::default();
        assert_eq!(gate.name(), "go-test");
    }

    #[test]
    fn test_go_test_gate_remediation() {
        let gate = GoTestGate::new();
        let issues = vec![GateIssue::new(IssueSeverity::Error, "Test failed: TestFoo")
            .with_code("go-test-failure")];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("go test"));
        assert!(remediation.contains("1 test(s) failed"));
    }

    #[test]
    fn test_go_test_parse_json_output() {
        let gate = GoTestGate::new();

        let stdout = r#"{"Action":"run","Package":"example.com/pkg","Test":"TestAdd"}
{"Action":"output","Package":"example.com/pkg","Test":"TestAdd","Output":"    main_test.go:15: expected 5, got 4\n"}
{"Action":"fail","Package":"example.com/pkg","Test":"TestAdd"}"#;

        let issues = gate.parse_json_output(stdout);
        assert_eq!(issues.len(), 1);

        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("TestAdd"));
        assert_eq!(issues[0].code, Some("go-test-failure".to_string()));
    }

    #[test]
    fn test_go_test_parse_text_output() {
        let gate = GoTestGate::new();

        let stdout = r#"--- FAIL: TestAdd (0.00s)
    main_test.go:15: expected 5, got 4
FAIL	example.com/pkg	0.005s"#;

        let issues = gate.parse_text_output(stdout, "");
        assert!(!issues.is_empty());
        assert!(issues[0].message.contains("TestAdd"));
    }

    #[test]
    fn test_go_test_extract_location() {
        let location = GoTestGate::extract_location("    main_test.go:15: expected 5, got 4");
        assert!(location.is_some());
        let (file, line) = location.unwrap();
        assert_eq!(file, "main_test.go");
        assert_eq!(line, 15);
    }

    // =========================================================================
    // GovulncheckGate tests
    // =========================================================================

    #[test]
    fn test_govulncheck_gate_name() {
        let gate = GovulncheckGate::new();
        assert_eq!(gate.name(), "govulncheck");
    }

    #[test]
    fn test_govulncheck_gate_is_blocking() {
        let gate = GovulncheckGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_govulncheck_gate_default() {
        let gate = GovulncheckGate::default();
        assert_eq!(gate.name(), "govulncheck");
    }

    #[test]
    fn test_govulncheck_gate_remediation() {
        let gate = GovulncheckGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Critical, "critical vuln"),
            GateIssue::new(IssueSeverity::Error, "high vuln"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("govulncheck"));
        assert!(remediation.contains("1 critical"));
        assert!(remediation.contains("1 high"));
    }

    #[test]
    fn test_govulncheck_parse_json_output() {
        let gate = GovulncheckGate::new();

        let stdout = r#"{"finding":{"osv":"GO-2023-1234","fixed_version":"v1.2.3","trace":[{"function":"vulnerable.Func","position":"main.go","line":42}]}}"#;

        let issues = gate.parse_json_output(stdout);
        assert_eq!(issues.len(), 1);

        assert_eq!(issues[0].severity, IssueSeverity::Critical);
        assert!(issues[0].message.contains("GO-2023-1234"));
        assert_eq!(issues[0].code, Some("GO-2023-1234".to_string()));
        assert!(issues[0].suggestion.as_ref().unwrap().contains("v1.2.3"));
    }

    #[test]
    fn test_govulncheck_parse_text_output() {
        let gate = GovulncheckGate::new();

        let stdout = r#"Vulnerability #1: GO-2023-1234
    Affected: example.com/vulnerable@v1.0.0
    More info: https://pkg.go.dev/vuln/GO-2023-1234"#;

        let issues = gate.parse_text_output(stdout);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("GO-2023-1234"));
    }

    // =========================================================================
    // go_gates() factory tests
    // =========================================================================

    #[test]
    fn test_go_gates_returns_all_gates() {
        let gates = go_gates();
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"go-vet"));
        assert!(names.contains(&"golangci-lint"));
        assert!(names.contains(&"go-test"));
        assert!(names.contains(&"govulncheck"));
    }

    #[test]
    fn test_go_gates_are_send_sync() {
        let gates = go_gates();
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_go_gates_have_remediation() {
        let gates = go_gates();
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

    // =========================================================================
    // Scoped Quality Gate tests (Sprint 26.2)
    // =========================================================================

    #[test]
    fn test_go_vet_gate_scoped_with_empty_returns_no_issues() {
        let gate = GoVetGate::new();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let issues = gate.run_scoped(temp_dir.path(), Some(&[])).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_golangci_lint_gate_scoped_with_empty_returns_no_issues() {
        let gate = GolangciLintGate::new();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let issues = gate.run_scoped(temp_dir.path(), Some(&[])).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_go_gates_support_run_scoped_api() {
        // Verify all Go gates support the run_scoped API
        let gates = go_gates();
        let temp_dir = tempfile::TempDir::new().unwrap();

        for gate in &gates {
            // Should not panic when called with empty files
            let _ = gate.run_scoped(temp_dir.path(), Some(&[]));
        }
    }
}
