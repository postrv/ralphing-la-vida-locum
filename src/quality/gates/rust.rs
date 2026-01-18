//! Rust-specific quality gate implementations.
//!
//! This module provides quality gates for Rust projects using standard tooling:
//! - [`ClippyGate`] - Runs `cargo clippy` with warnings as errors
//! - [`CargoTestGate`] - Runs `cargo test` and parses results
//! - [`NoAllowGate`] - Checks for forbidden `#[allow(...)]` annotations
//! - [`SecurityGate`] - Runs security scans via narsil-mcp and cargo-audit
//! - [`NoTodoGate`] - Checks for TODO/FIXME comments

use std::sync::OnceLock;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use super::{GateIssue, IssueSeverity, QualityGate};
use crate::narsil::{NarsilClient, NarsilConfig, SecurityFinding, SecuritySeverity};

// ============================================================================
// Public Factory Function
// ============================================================================

/// Returns all standard quality gates for Rust projects.
///
/// The returned gates include:
/// - Clippy (linting)
/// - Tests (unit/integration tests)
/// - NoAllow (forbidden annotations)
/// - Security (cargo-audit + narsil-mcp)
/// - NoTodo (TODO/FIXME comments)
#[must_use]
pub fn rust_gates() -> Vec<Box<dyn QualityGate>> {
    vec![
        Box::new(ClippyGate::new()),
        Box::new(CargoTestGate::new()),
        Box::new(NoAllowGate::new()),
        Box::new(SecurityGate::new()),
        Box::new(NoTodoGate::new()),
    ]
}

// ============================================================================
// Clippy Gate
// ============================================================================

/// Configuration for the clippy gate.
#[derive(Debug, Clone)]
pub struct ClippyConfig {
    /// Treat warnings as errors.
    pub warnings_as_errors: bool,
    /// Additional clippy arguments.
    pub extra_args: Vec<String>,
    /// Lints to allow (won't report these).
    pub allowed_lints: Vec<String>,
}

impl Default for ClippyConfig {
    fn default() -> Self {
        Self {
            warnings_as_errors: true,
            extra_args: vec!["--all-targets".to_string()],
            allowed_lints: Vec::new(),
        }
    }
}

/// Quality gate that runs `cargo clippy`.
pub struct ClippyGate {
    config: ClippyConfig,
}

impl ClippyGate {
    /// Create a new clippy gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ClippyConfig::default(),
        }
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(config: ClippyConfig) -> Self {
        Self { config }
    }

    /// Parse clippy output into issues.
    fn parse_output(&self, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        let mut current_severity = None;
        let mut current_message = String::new();
        let mut current_file = None;
        let mut current_line = None;
        let mut current_code = None;

        for line in stderr.lines() {
            let is_warning = line.starts_with("warning: ") || line.starts_with("warning[");
            let is_error = line.starts_with("error: ") || line.starts_with("error[");

            if is_warning || is_error {
                // Save previous issue if any
                if let Some(sev) = current_severity {
                    if !current_message.is_empty() {
                        let mut issue = GateIssue::new(sev, current_message.clone());
                        if let Some(ref file) = current_file {
                            if let Some(line_num) = current_line {
                                issue = issue.with_location(file, line_num);
                            }
                        }
                        if let Some(ref code) = current_code {
                            issue = issue.with_code(code);
                        }
                        issues.push(issue);
                    }
                }

                // Parse new issue
                let (sev, rest, error_code) = if let Some(stripped) = line.strip_prefix("warning: ")
                {
                    (IssueSeverity::Warning, stripped, None)
                } else if let Some(stripped) = line.strip_prefix("error: ") {
                    (IssueSeverity::Error, stripped, None)
                } else if line.starts_with("error[") {
                    if let Some(bracket_end) = line.find("]: ") {
                        let code = &line[6..bracket_end];
                        let msg = &line[bracket_end + 3..];
                        (IssueSeverity::Error, msg, Some(code.to_string()))
                    } else {
                        continue;
                    }
                } else if line.starts_with("warning[") {
                    if let Some(bracket_end) = line.find("]: ") {
                        let code = &line[8..bracket_end];
                        let msg = &line[bracket_end + 3..];
                        (IssueSeverity::Warning, msg, Some(code.to_string()))
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };

                current_severity = Some(sev);

                if let Some(code) = error_code {
                    current_code = Some(code);
                    current_message = rest.to_string();
                } else if let Some(bracket_start) = rest.rfind('[') {
                    if let Some(bracket_end) = rest.rfind(']') {
                        current_code = Some(rest[bracket_start + 1..bracket_end].to_string());
                        current_message = rest[..bracket_start].trim().to_string();
                    } else {
                        current_message = rest.to_string();
                        current_code = None;
                    }
                } else {
                    current_message = rest.to_string();
                    current_code = None;
                }

                current_file = None;
                current_line = None;
            } else if line.trim_start().starts_with("--> ") {
                let loc = line.trim_start().trim_start_matches("--> ");
                let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
                if parts.len() >= 2 {
                    if let Ok(line_num) = parts[1].parse::<u32>() {
                        current_line = Some(line_num);
                        if parts.len() >= 3 {
                            current_file = Some(parts[2].to_string());
                        }
                    }
                }
            }
        }

        // Don't forget the last issue
        if let Some(sev) = current_severity {
            if !current_message.is_empty() {
                let mut issue = GateIssue::new(sev, current_message);
                if let Some(ref file) = current_file {
                    if let Some(line_num) = current_line {
                        issue = issue.with_location(file, line_num);
                    }
                }
                if let Some(ref code) = current_code {
                    issue = issue.with_code(code);
                }
                issues.push(issue);
            }
        }

        // Filter out allowed lints
        issues
            .into_iter()
            .filter(|issue| {
                if let Some(ref code) = issue.code {
                    !self.config.allowed_lints.iter().any(|allowed| code.contains(allowed))
                } else {
                    true
                }
            })
            .collect()
    }
}

impl Default for ClippyGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for ClippyGate {
    fn name(&self) -> &str {
        "Clippy"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["clippy".to_string()];
        args.extend(self.config.extra_args.clone());

        if self.config.warnings_as_errors {
            args.push("--".to_string());
            args.push("-D".to_string());
            args.push("warnings".to_string());
        }

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(project_dir)
            .output()
            .context("Failed to run cargo clippy")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(self.parse_output(&stderr))
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let error_count = issues.iter().filter(|i| i.severity == IssueSeverity::Error).count();
        let warning_count = issues.iter().filter(|i| i.severity == IssueSeverity::Warning).count();

        format!(
            r#"## Clippy Linting Issues

Found {} errors and {} warnings.

**How to fix:**
1. Run `cargo clippy --all-targets` to see all issues
2. Fix each issue at the reported location
3. Run `cargo clippy --all-targets -- -D warnings` to verify all fixed

**Common fixes:**
- Unused variables: prefix with `_` or remove
- Unused imports: remove the import
- Type mismatches: ensure types match expected signatures
"#,
            error_count, warning_count
        )
    }
}

// ============================================================================
// Cargo Test Gate
// ============================================================================

/// Configuration for the test gate.
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Minimum pass rate required (0.0 - 1.0).
    pub min_pass_rate: f64,
    /// Additional test arguments.
    pub extra_args: Vec<String>,
    /// Run doc tests as well.
    pub include_doc_tests: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            min_pass_rate: 1.0,
            extra_args: Vec::new(),
            include_doc_tests: true,
        }
    }
}

/// Quality gate that runs `cargo test`.
pub struct CargoTestGate {
    config: TestConfig,
}

impl CargoTestGate {
    /// Create a new test gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TestConfig::default(),
        }
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(config: TestConfig) -> Self {
        Self { config }
    }

    /// Parse test output to extract failure information.
    fn parse_output(&self, stdout: &str, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();
        let combined = format!("{}\n{}", stdout, stderr);

        for line in combined.lines() {
            // Parse individual failure: "---- test_name stdout ----"
            if line.starts_with("---- ") && line.ends_with(" ----") {
                let test_name = line
                    .trim_start_matches("---- ")
                    .trim_end_matches(" ----")
                    .trim_end_matches(" stdout");

                issues.push(
                    GateIssue::new(IssueSeverity::Error, format!("Test failed: {}", test_name))
                        .with_code("test_failure"),
                );
            }
        }

        issues
    }
}

impl Default for CargoTestGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for CargoTestGate {
    fn name(&self) -> &str {
        "Tests"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut args = vec!["test".to_string()];
        args.extend(self.config.extra_args.clone());

        if !self.config.include_doc_tests {
            args.push("--lib".to_string());
            args.push("--tests".to_string());
        }

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(project_dir)
            .output()
            .context("Failed to run cargo test")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(Vec::new())
        } else {
            Ok(self.parse_output(&stdout, &stderr))
        }
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let failed_tests: Vec<_> = issues
            .iter()
            .filter_map(|i| {
                if i.message.starts_with("Test failed: ") {
                    Some(i.message.trim_start_matches("Test failed: "))
                } else {
                    None
                }
            })
            .collect();

        format!(
            r#"## Test Failures

{} test(s) failed.

**Failed tests:**
{}

**How to fix:**
1. Run `cargo test` to see detailed failure output
2. Fix each failing test
3. Run `cargo test` to verify all pass
"#,
            failed_tests.len(),
            failed_tests.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
        )
    }
}

// ============================================================================
// No Allow Gate
// ============================================================================

/// Quality gate that checks for forbidden `#[allow(...)]` annotations.
pub struct NoAllowGate {
    /// Patterns that are allowed.
    allowed_patterns: Vec<String>,
}

impl NoAllowGate {
    /// Create a new no-allow gate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_patterns: Vec::new(),
        }
    }

    /// Add allowed patterns.
    #[must_use]
    pub fn with_allowed(mut self, patterns: Vec<String>) -> Self {
        self.allowed_patterns = patterns;
        self
    }

    /// Scan a file for #[allow(...)] annotations.
    fn scan_file(&self, path: &Path) -> Result<Vec<GateIssue>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let mut issues = Vec::new();
        let mut in_raw_string = false;
        let mut raw_string_hashes = 0;

        for (line_num, line) in content.lines().enumerate() {
            // Track raw string literal state
            if !in_raw_string {
                if let Some(pos) = line.find("r#") {
                    let after_r = &line[pos + 1..];
                    let hash_count = after_r.chars().take_while(|&c| c == '#').count();
                    if after_r.len() > hash_count && after_r.chars().nth(hash_count) == Some('"') {
                        let closing_delim = format!("\"{}", "#".repeat(hash_count));
                        let content_start = pos + 2 + hash_count;
                        if content_start < line.len() {
                            let rest_of_line = &line[content_start..];
                            if !rest_of_line.contains(&closing_delim) {
                                in_raw_string = true;
                                raw_string_hashes = hash_count;
                            }
                        } else {
                            in_raw_string = true;
                            raw_string_hashes = hash_count;
                        }
                    }
                }
            } else {
                let closing_delim = format!("\"{}", "#".repeat(raw_string_hashes));
                if line.contains(&closing_delim) {
                    in_raw_string = false;
                    raw_string_hashes = 0;
                }
                continue;
            }

            if in_raw_string {
                continue;
            }

            let trimmed = line.trim();

            if (trimmed.starts_with("#[allow(") || trimmed.starts_with("#![allow("))
                && trimmed.contains(')')
            {
                let start = trimmed.find('(').unwrap() + 1;
                let end = trimmed.rfind(')').unwrap();
                let lint = &trimmed[start..end];

                if self.allowed_patterns.iter().any(|p| lint.contains(p)) {
                    continue;
                }

                issues.push(
                    GateIssue::new(
                        IssueSeverity::Error,
                        format!("Forbidden #[allow({})] annotation", lint),
                    )
                    .with_location(path, (line_num + 1) as u32)
                    .with_code("no_allow")
                    .with_suggestion(format!(
                        "Remove the #[allow({})] and fix the underlying issue",
                        lint
                    )),
                );
            }
        }

        Ok(issues)
    }

    /// Find all Rust source files in the project.
    fn find_rust_files(&self, project_dir: &Path) -> Result<Vec<PathBuf>> {
        let src_dir = project_dir.join("src");
        if !src_dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        self.walk_dir(&src_dir, &mut files)?;
        Ok(files)
    }

    fn walk_dir(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.walk_dir(&path, files)?;
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        Ok(())
    }
}

impl Default for NoAllowGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for NoAllowGate {
    fn name(&self) -> &str {
        "NoAllow"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let rust_files = self.find_rust_files(project_dir)?;
        let mut all_issues = Vec::new();

        for file in rust_files {
            let issues = self.scan_file(&file)?;
            all_issues.extend(issues);
        }

        Ok(all_issues)
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        format!(
            r#"## Forbidden #[allow(...)] Annotations

Found {} forbidden annotations.

**Why this matters:**
- `#[allow(...)]` annotations hide problems instead of fixing them
- They make code harder to maintain
- They can mask security issues

**How to fix:**
1. Remove each `#[allow(...)]` annotation
2. Fix the underlying issue that caused the warning
3. If truly necessary, document why in a comment
"#,
            issues.len()
        )
    }
}

// ============================================================================
// Security Gate
// ============================================================================

/// Quality gate that runs security scans via narsil-mcp and cargo-audit.
pub struct SecurityGate {
    /// Minimum severity to report.
    severity_threshold: IssueSeverity,
    /// Cached narsil scan results.
    narsil_cache: OnceLock<Vec<GateIssue>>,
}

impl SecurityGate {
    /// Create a new security gate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            severity_threshold: IssueSeverity::Warning,
            narsil_cache: OnceLock::new(),
        }
    }

    /// Set the minimum severity threshold.
    #[must_use]
    pub fn with_threshold(mut self, threshold: IssueSeverity) -> Self {
        self.severity_threshold = threshold;
        self
    }

    /// Convert a narsil SecuritySeverity to a gate IssueSeverity.
    fn convert_severity(severity: SecuritySeverity) -> IssueSeverity {
        match severity {
            SecuritySeverity::Critical => IssueSeverity::Critical,
            SecuritySeverity::High => IssueSeverity::Error,
            SecuritySeverity::Medium => IssueSeverity::Warning,
            SecuritySeverity::Low => IssueSeverity::Info,
            SecuritySeverity::Info => IssueSeverity::Info,
        }
    }

    /// Convert a narsil SecurityFinding to a gate GateIssue.
    fn convert_finding(finding: &SecurityFinding) -> GateIssue {
        let mut issue = GateIssue::new(Self::convert_severity(finding.severity), &finding.message);

        issue.file = Some(finding.file.clone());

        if let Some(line) = finding.line {
            issue.line = Some(line);
        }

        if let Some(ref rule_id) = finding.rule_id {
            issue.code = Some(rule_id.clone());
        }

        if let Some(ref suggestion) = finding.suggestion {
            issue.suggestion = Some(suggestion.clone());
        }

        issue
    }

    /// Run narsil-mcp security scan if available.
    fn run_narsil_scan(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        if let Some(cached) = self.narsil_cache.get() {
            return Ok(cached.clone());
        }

        let config = NarsilConfig::new(project_dir);
        let client = match NarsilClient::new(config) {
            Ok(c) => c,
            Err(_) => {
                let _ = self.narsil_cache.set(Vec::new());
                return Ok(Vec::new());
            }
        };

        if !client.is_available() {
            let _ = self.narsil_cache.set(Vec::new());
            return Ok(Vec::new());
        }

        match client.scan_security() {
            Ok(findings) => {
                let issues: Vec<GateIssue> = findings.iter().map(Self::convert_finding).collect();
                let _ = self.narsil_cache.set(issues.clone());
                Ok(issues)
            }
            Err(e) => {
                if e.is_recoverable() {
                    let _ = self.narsil_cache.set(Vec::new());
                    Ok(Vec::new())
                } else {
                    Err(anyhow::anyhow!("narsil-mcp scan failed: {}", e))
                }
            }
        }
    }

    /// Try to run cargo-audit for dependency vulnerabilities.
    fn run_cargo_audit(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let output = Command::new("cargo")
            .args(["audit", "--json"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if output.status.success() {
                    Ok(Vec::new())
                } else {
                    let issues: Vec<GateIssue> = stdout
                        .lines()
                        .filter(|line| line.contains("vulnerability"))
                        .map(|line| GateIssue::new(IssueSeverity::Critical, line.to_string()))
                        .collect();

                    if issues.is_empty() {
                        Ok(vec![GateIssue::new(
                            IssueSeverity::Warning,
                            "cargo-audit reported issues (run manually for details)",
                        )])
                    } else {
                        Ok(issues)
                    }
                }
            }
            Err(_) => Ok(Vec::new()), // cargo-audit not installed
        }
    }
}

impl Default for SecurityGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for SecurityGate {
    fn name(&self) -> &str {
        "Security"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut all_issues = Vec::new();

        // Run cargo-audit for dependency vulnerabilities
        let audit_issues = self.run_cargo_audit(project_dir)?;
        all_issues.extend(audit_issues);

        // Run narsil-mcp scan for code vulnerabilities
        let narsil_issues = self.run_narsil_scan(project_dir)?;
        all_issues.extend(narsil_issues);

        // Filter by severity threshold
        Ok(all_issues
            .into_iter()
            .filter(|i| i.severity >= self.severity_threshold)
            .collect())
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        let critical = issues.iter().filter(|i| i.severity == IssueSeverity::Critical).count();
        let high = issues.iter().filter(|i| i.severity == IssueSeverity::Error).count();

        format!(
            r#"## Security Vulnerabilities

Found {} critical and {} high severity issues.

**How to fix:**
1. Run `cargo audit` to see dependency vulnerabilities
2. Update affected dependencies to patched versions
3. Run narsil-mcp `scan_security` for code vulnerabilities
4. Address each finding at the reported location

**Priority:**
- Fix CRITICAL issues immediately
- Fix HIGH issues before committing
"#,
            critical, high
        )
    }
}

// ============================================================================
// No TODO Gate
// ============================================================================

/// Quality gate that checks for TODO/FIXME comments.
pub struct NoTodoGate {
    /// Patterns to search for.
    patterns: Vec<String>,
}

impl NoTodoGate {
    /// Create a new no-todo gate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: vec!["TODO:".to_string(), "FIXME:".to_string()],
        }
    }

    /// Add additional patterns to search for.
    #[must_use]
    pub fn with_patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns.extend(patterns);
        self
    }

    /// Scan a file for TODO/FIXME comments.
    fn scan_file(&self, path: &Path) -> Result<Vec<GateIssue>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let mut issues = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for pattern in &self.patterns {
                if line.contains(pattern) {
                    issues.push(
                        GateIssue::new(
                            IssueSeverity::Warning,
                            format!("Found {} comment", pattern.trim_end_matches(':')),
                        )
                        .with_location(path, (line_num + 1) as u32)
                        .with_code("todo_comment")
                        .with_suggestion(
                            "Implement the TODO or move to IMPLEMENTATION_PLAN.md".to_string(),
                        ),
                    );
                }
            }
        }

        Ok(issues)
    }

    /// Find all Rust source files in the project.
    fn find_rust_files(&self, project_dir: &Path) -> Result<Vec<PathBuf>> {
        let src_dir = project_dir.join("src");
        if !src_dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        self.walk_dir(&src_dir, &mut files)?;
        Ok(files)
    }

    fn walk_dir(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.walk_dir(&path, files)?;
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        Ok(())
    }
}

impl Default for NoTodoGate {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGate for NoTodoGate {
    fn name(&self) -> &str {
        "NoTodo"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let rust_files = self.find_rust_files(project_dir)?;
        let mut all_issues = Vec::new();

        for file in rust_files {
            let issues = self.scan_file(&file)?;
            all_issues.extend(issues);
        }

        Ok(all_issues)
    }

    fn is_blocking(&self) -> bool {
        // TODO/FIXME comments are warnings, not blocking
        false
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        format!(
            r#"## TODO/FIXME Comments

Found {} TODO/FIXME comments.

**How to fix:**
1. Implement each TODO item now
2. Or move to IMPLEMENTATION_PLAN.md for tracking
3. Remove the comment when done

**Note:** This gate is non-blocking (warnings only).
"#,
            issues.len()
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // ClippyGate tests
    // =========================================================================

    #[test]
    fn test_clippy_gate_name() {
        let gate = ClippyGate::new();
        assert_eq!(gate.name(), "Clippy");
    }

    #[test]
    fn test_clippy_gate_is_blocking() {
        let gate = ClippyGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_clippy_gate_remediation() {
        let gate = ClippyGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "error"),
            GateIssue::new(IssueSeverity::Warning, "warning"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Clippy"));
        assert!(remediation.contains("1 errors"));
        assert!(remediation.contains("1 warnings"));
    }

    #[test]
    fn test_clippy_output_parsing() {
        let gate = ClippyGate::new();

        let stderr = r#"
warning: unused variable: `x`
  --> src/main.rs:10:9

error[E0308]: mismatched types
  --> src/lib.rs:20:5
"#;

        let issues = gate.parse_output(stderr);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].severity, IssueSeverity::Warning);
        assert_eq!(issues[1].severity, IssueSeverity::Error);
    }

    // =========================================================================
    // CargoTestGate tests
    // =========================================================================

    #[test]
    fn test_cargo_test_gate_name() {
        let gate = CargoTestGate::new();
        assert_eq!(gate.name(), "Tests");
    }

    #[test]
    fn test_cargo_test_gate_is_blocking() {
        let gate = CargoTestGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_cargo_test_gate_remediation() {
        let gate = CargoTestGate::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "Test failed: test_foo"),
            GateIssue::new(IssueSeverity::Error, "Test failed: test_bar"),
        ];
        let remediation = gate.remediation(&issues);
        assert!(remediation.contains("Test Failures"));
        assert!(remediation.contains("2 test(s) failed"));
    }

    // =========================================================================
    // NoAllowGate tests
    // =========================================================================

    #[test]
    fn test_no_allow_gate_name() {
        let gate = NoAllowGate::new();
        assert_eq!(gate.name(), "NoAllow");
    }

    #[test]
    fn test_no_allow_gate_detects_allow() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("lib.rs");
        std::fs::write(
            &file_path,
            r#"
#[allow(dead_code)]
fn unused_function() {}

#![allow(unused_variables)]
fn main() {
    let x = 5;
}
"#,
        )
        .unwrap();

        let gate = NoAllowGate::new();
        let issues = gate.run(temp_dir.path()).unwrap();

        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|i| i.message.contains("dead_code")));
        assert!(issues.iter().any(|i| i.message.contains("unused_variables")));
    }

    #[test]
    fn test_no_allow_gate_with_allowed_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("lib.rs");
        std::fs::write(
            &file_path,
            r#"
#[allow(dead_code)]
fn unused_function() {}
"#,
        )
        .unwrap();

        let gate = NoAllowGate::new().with_allowed(vec!["dead_code".to_string()]);
        let issues = gate.run(temp_dir.path()).unwrap();

        assert!(issues.is_empty());
    }

    #[test]
    fn test_no_allow_gate_skips_raw_strings() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("lib.rs");
        std::fs::write(
            &file_path,
            r##"
fn clean_function() {}

#[test]
fn test_example() {
    let test_content = r#"
#[allow(dead_code)]
fn unused_function() {}
"#;
    assert!(!test_content.is_empty());
}
"##,
        )
        .unwrap();

        let gate = NoAllowGate::new();
        let issues = gate.run(temp_dir.path()).unwrap();

        assert!(issues.is_empty(), "Gate should skip #[allow] inside raw strings");
    }

    // =========================================================================
    // SecurityGate tests
    // =========================================================================

    #[test]
    fn test_security_gate_name() {
        let gate = SecurityGate::new();
        assert_eq!(gate.name(), "Security");
    }

    #[test]
    fn test_security_gate_is_blocking() {
        let gate = SecurityGate::new();
        assert!(gate.is_blocking());
    }

    #[test]
    fn test_security_gate_severity_conversion() {
        assert_eq!(SecurityGate::convert_severity(SecuritySeverity::Critical), IssueSeverity::Critical);
        assert_eq!(SecurityGate::convert_severity(SecuritySeverity::High), IssueSeverity::Error);
        assert_eq!(SecurityGate::convert_severity(SecuritySeverity::Medium), IssueSeverity::Warning);
        assert_eq!(SecurityGate::convert_severity(SecuritySeverity::Low), IssueSeverity::Info);
    }

    // =========================================================================
    // NoTodoGate tests
    // =========================================================================

    #[test]
    fn test_no_todo_gate_name() {
        let gate = NoTodoGate::new();
        assert_eq!(gate.name(), "NoTodo");
    }

    #[test]
    fn test_no_todo_gate_is_not_blocking() {
        let gate = NoTodoGate::new();
        assert!(!gate.is_blocking());
    }

    #[test]
    fn test_no_todo_gate_detects_todos() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("lib.rs");
        std::fs::write(
            &file_path,
            r#"
fn main() {
    // TODO: implement this
    // FIXME: broken
}
"#,
        )
        .unwrap();

        let gate = NoTodoGate::new();
        let issues = gate.run(temp_dir.path()).unwrap();

        assert_eq!(issues.len(), 2);
    }

    // =========================================================================
    // rust_gates() factory tests
    // =========================================================================

    #[test]
    fn test_rust_gates_returns_all_gates() {
        let gates = rust_gates();
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"Clippy"));
        assert!(names.contains(&"Tests"));
        assert!(names.contains(&"NoAllow"));
        assert!(names.contains(&"Security"));
        assert!(names.contains(&"NoTodo"));
    }

    #[test]
    fn test_rust_gates_are_send_sync() {
        let gates = rust_gates();
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }
}
