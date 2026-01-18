//! Individual quality gate implementations.
//!
//! Each gate checks a specific aspect of code quality and returns
//! a structured result that can be used for enforcement and remediation.
//!
//! # Available Gates
//!
//! - [`ClippyGate`] - Runs `cargo clippy` with warnings as errors
//! - [`TestGate`] - Runs `cargo test` and parses results
//! - [`NoAllowGate`] - Checks for forbidden `#[allow(...)]` annotations
//! - [`SecurityGate`] - Runs security scans via narsil-mcp
//! - [`NoTodoGate`] - Checks for TODO/FIXME comments in code
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::quality::gates::{ClippyGate, Gate, GateConfig};
//!
//! let gate = ClippyGate::new("/path/to/project");
//! let result = gate.check()?;
//! if !result.passed {
//!     eprintln!("Clippy failed: {:?}", result.issues);
//! }
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::narsil::{NarsilClient, SecurityFinding, SecuritySeverity};

// ============================================================================
// Gate Result Types
// ============================================================================

/// Severity level for quality issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Informational only, doesn't block.
    Info,
    /// Warning - should be fixed but doesn't block.
    Warning,
    /// Error - blocks commits.
    Error,
    /// Critical - security issue, must be fixed immediately.
    Critical,
}

impl IssueSeverity {
    /// Check if this severity blocks commits.
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Error | Self::Critical)
    }
}

impl std::fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A single issue found by a quality gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateIssue {
    /// Severity of the issue.
    pub severity: IssueSeverity,
    /// Brief description of the issue.
    pub message: String,
    /// File path where the issue was found (if applicable).
    pub file: Option<PathBuf>,
    /// Line number (if applicable).
    pub line: Option<u32>,
    /// Column number (if applicable).
    pub column: Option<u32>,
    /// Error/warning code (e.g., "E0308", "clippy::unwrap_used").
    pub code: Option<String>,
    /// Suggested fix (if available).
    pub suggestion: Option<String>,
}

impl GateIssue {
    /// Create a new issue.
    pub fn new(severity: IssueSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            file: None,
            line: None,
            column: None,
            code: None,
            suggestion: None,
        }
    }

    /// Add a file location.
    #[must_use]
    pub fn with_location(mut self, file: impl AsRef<Path>, line: u32) -> Self {
        self.file = Some(file.as_ref().to_path_buf());
        self.line = Some(line);
        self
    }

    /// Add column information.
    #[must_use]
    pub fn with_column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    /// Add an error code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Add a suggested fix.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Format as a string for display.
    #[must_use]
    pub fn format(&self) -> String {
        let mut parts = vec![format!("[{}]", self.severity)];

        if let Some(ref code) = self.code {
            parts.push(format!("[{}]", code));
        }

        parts.push(self.message.clone());

        if let Some(ref file) = self.file {
            let loc = if let Some(line) = self.line {
                if let Some(col) = self.column {
                    format!("{}:{}:{}", file.display(), line, col)
                } else {
                    format!("{}:{}", file.display(), line)
                }
            } else {
                file.display().to_string()
            };
            parts.push(format!("at {}", loc));
        }

        parts.join(" ")
    }
}

/// Result from running a quality gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Name of the gate that was run.
    pub gate_name: String,
    /// Whether the gate passed (no blocking issues).
    pub passed: bool,
    /// Issues found by the gate.
    pub issues: Vec<GateIssue>,
    /// Raw output from the tool (for debugging).
    pub raw_output: String,
    /// Duration of the check in milliseconds.
    pub duration_ms: u64,
}

impl GateResult {
    /// Create a passing result.
    pub fn pass(gate_name: impl Into<String>) -> Self {
        Self {
            gate_name: gate_name.into(),
            passed: true,
            issues: Vec::new(),
            raw_output: String::new(),
            duration_ms: 0,
        }
    }

    /// Create a failing result with issues.
    pub fn fail(gate_name: impl Into<String>, issues: Vec<GateIssue>) -> Self {
        Self {
            gate_name: gate_name.into(),
            passed: false,
            issues,
            raw_output: String::new(),
            duration_ms: 0,
        }
    }

    /// Add raw output.
    #[must_use]
    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.raw_output = output.into();
        self
    }

    /// Add duration.
    #[must_use]
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Count issues by severity.
    #[must_use]
    pub fn count_by_severity(&self, severity: IssueSeverity) -> usize {
        self.issues.iter().filter(|i| i.severity == severity).count()
    }

    /// Get only blocking issues.
    #[must_use]
    pub fn blocking_issues(&self) -> Vec<&GateIssue> {
        self.issues.iter().filter(|i| i.severity.is_blocking()).collect()
    }

    /// Format a summary for display.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.passed {
            format!("✅ {}: PASSED", self.gate_name)
        } else {
            let counts: Vec<String> = [
                (IssueSeverity::Critical, "critical"),
                (IssueSeverity::Error, "errors"),
                (IssueSeverity::Warning, "warnings"),
            ]
            .iter()
            .filter_map(|(sev, name)| {
                let count = self.count_by_severity(*sev);
                if count > 0 {
                    Some(format!("{} {}", count, name))
                } else {
                    None
                }
            })
            .collect();

            format!("❌ {}: FAILED ({})", self.gate_name, counts.join(", "))
        }
    }
}

// ============================================================================
// Gate Trait
// ============================================================================

/// Trait for quality gates.
pub trait Gate {
    /// Get the name of this gate.
    fn name(&self) -> &str;

    /// Run the gate check.
    ///
    /// # Errors
    ///
    /// Returns an error if the gate fails to execute (not if checks fail).
    fn check(&self) -> Result<GateResult>;

    /// Check if this gate is blocking (prevents commits on failure).
    fn is_blocking(&self) -> bool {
        true
    }
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
    project_dir: PathBuf,
    config: ClippyConfig,
}

impl ClippyGate {
    /// Create a new clippy gate for the given project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            config: ClippyConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(project_dir: impl AsRef<Path>, config: ClippyConfig) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            config,
        }
    }

    /// Parse clippy output into issues.
    fn parse_output(&self, stderr: &str) -> Vec<GateIssue> {
        let mut issues = Vec::new();

        // Parse lines like:
        // warning: unused variable: `x`
        //   --> src/main.rs:10:9
        //   |
        // 10 |     let x = 5;
        //   |         ^ help: if this is intentional, prefix it with an underscore: `_x`

        let mut current_severity = None;
        let mut current_message = String::new();
        let mut current_file = None;
        let mut current_line = None;
        let mut current_code = None;

        for line in stderr.lines() {
            // Check for warning/error start
            // Handles formats:
            // - "warning: unused variable"
            // - "error: mismatched types"
            // - "error[E0308]: mismatched types"
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
                    // Parse "error[E0308]: message"
                    if let Some(bracket_end) = line.find("]: ") {
                        let code = &line[6..bracket_end]; // Skip "error["
                        let msg = &line[bracket_end + 3..]; // Skip "]: "
                        (IssueSeverity::Error, msg, Some(code.to_string()))
                    } else {
                        continue;
                    }
                } else if line.starts_with("warning[") {
                    // Parse "warning[W0001]: message"
                    if let Some(bracket_end) = line.find("]: ") {
                        let code = &line[8..bracket_end]; // Skip "warning["
                        let msg = &line[bracket_end + 3..]; // Skip "]: "
                        (IssueSeverity::Warning, msg, Some(code.to_string()))
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };

                current_severity = Some(sev);

                // Use error code from bracket format if present, otherwise extract from message
                if let Some(code) = error_code {
                    current_code = Some(code);
                    current_message = rest.to_string();
                } else if let Some(bracket_start) = rest.rfind('[') {
                    // Extract code if present (e.g., "warning: unused variable: `x` [clippy::unused]")
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
            }
            // Check for location line
            else if line.trim_start().starts_with("--> ") {
                let loc = line.trim_start().trim_start_matches("--> ");
                // Parse "src/main.rs:10:9"
                let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
                if parts.len() >= 2 {
                    if let Ok(line_num) = parts[1].parse::<u32>() {
                        current_line = Some(line_num);
                        // Reconstruct file path (everything before the line number)
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

        issues
    }
}

impl Gate for ClippyGate {
    fn name(&self) -> &str {
        "Clippy"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();

        let mut args = vec!["clippy".to_string()];
        args.extend(self.config.extra_args.clone());

        if self.config.warnings_as_errors {
            args.push("--".to_string());
            args.push("-D".to_string());
            args.push("warnings".to_string());
        }

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to run cargo clippy")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let duration_ms = start.elapsed().as_millis() as u64;

        let issues = self.parse_output(&stderr);

        // Filter out allowed lints
        let issues: Vec<_> = issues
            .into_iter()
            .filter(|issue| {
                if let Some(ref code) = issue.code {
                    !self.config.allowed_lints.iter().any(|allowed| code.contains(allowed))
                } else {
                    true
                }
            })
            .collect();

        let has_blocking = issues.iter().any(|i| i.severity.is_blocking());
        let passed = output.status.success() && !has_blocking;

        Ok(if passed {
            GateResult::pass(self.name())
                .with_output(stderr.to_string())
                .with_duration(duration_ms)
        } else {
            GateResult::fail(self.name(), issues)
                .with_output(stderr.to_string())
                .with_duration(duration_ms)
        })
    }
}

// ============================================================================
// Test Gate
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
            min_pass_rate: 1.0, // All tests must pass by default
            extra_args: Vec::new(),
            include_doc_tests: true,
        }
    }
}

/// Quality gate that runs `cargo test`.
pub struct TestGate {
    project_dir: PathBuf,
    config: TestConfig,
}

impl TestGate {
    /// Create a new test gate for the given project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            config: TestConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(project_dir: impl AsRef<Path>, config: TestConfig) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            config,
        }
    }

    /// Parse test output to extract failure information.
    fn parse_output(&self, stdout: &str, stderr: &str) -> (u32, u32, Vec<GateIssue>) {
        let mut passed = 0u32;
        let mut failed = 0u32;
        let mut issues = Vec::new();

        // Combine output for parsing
        let combined = format!("{}\n{}", stdout, stderr);

        for line in combined.lines() {
            // Parse summary line: "test result: FAILED. 10 passed; 2 failed; 0 ignored"
            if line.starts_with("test result:") {
                // Extract passed count
                if let Some(p) = line.find(" passed") {
                    let before = &line[..p];
                    if let Some(start) = before.rfind(|c: char| !c.is_ascii_digit()) {
                        if let Ok(n) = before[start + 1..].trim().parse::<u32>() {
                            passed = n;
                        }
                    }
                }
                // Extract failed count
                if let Some(f) = line.find(" failed") {
                    let before = &line[..f];
                    if let Some(start) = before.rfind(|c: char| !c.is_ascii_digit()) {
                        if let Ok(n) = before[start + 1..].trim().parse::<u32>() {
                            failed = n;
                        }
                    }
                }
            }
            // Parse individual failure: "---- test_name stdout ----"
            else if line.starts_with("---- ") && line.ends_with(" ----") {
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

        (passed, failed, issues)
    }
}

impl Gate for TestGate {
    fn name(&self) -> &str {
        "Tests"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();

        let mut args = vec!["test".to_string()];
        args.extend(self.config.extra_args.clone());

        if !self.config.include_doc_tests {
            args.push("--lib".to_string());
            args.push("--tests".to_string());
        }

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to run cargo test")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let duration_ms = start.elapsed().as_millis() as u64;

        let (passed_count, failed_count, issues) = self.parse_output(&stdout, &stderr);

        let total = passed_count + failed_count;
        let pass_rate = if total > 0 {
            passed_count as f64 / total as f64
        } else {
            1.0 // No tests = pass
        };

        let passed = output.status.success() && pass_rate >= self.config.min_pass_rate;

        let combined_output = format!("{}\n{}", stdout, stderr);

        Ok(if passed {
            GateResult::pass(self.name())
                .with_output(combined_output)
                .with_duration(duration_ms)
        } else {
            GateResult::fail(self.name(), issues)
                .with_output(combined_output)
                .with_duration(duration_ms)
        })
    }
}

// ============================================================================
// No Allow Gate
// ============================================================================

/// Quality gate that checks for forbidden `#[allow(...)]` annotations.
pub struct NoAllowGate {
    project_dir: PathBuf,
    /// Patterns that are allowed (e.g., allow(dead_code) in test modules).
    allowed_patterns: Vec<String>,
}

impl NoAllowGate {
    /// Create a new no-allow gate for the given project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
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
            // Track raw string literal state to avoid false positives from test data
            // Raw strings look like r#"..."# or r##"..."## etc.
            if !in_raw_string {
                // Check for raw string start: r#" or r##" etc.
                if let Some(pos) = line.find("r#") {
                    // Count consecutive # characters after 'r'
                    let after_r = &line[pos + 1..];
                    let hash_count = after_r.chars().take_while(|&c| c == '#').count();
                    // Check if followed by opening quote
                    if after_r.len() > hash_count && after_r.chars().nth(hash_count) == Some('"') {
                        // Check if the closing delimiter is on the same line
                        let closing_delim = format!("\"{}", "#".repeat(hash_count));
                        let content_start = pos + 2 + hash_count; // r + # count + "
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
                // Check for raw string end: "# or "## etc.
                let closing_delim = format!("\"{}", "#".repeat(raw_string_hashes));
                if line.contains(&closing_delim) {
                    in_raw_string = false;
                    raw_string_hashes = 0;
                }
                continue; // Skip scanning inside raw strings
            }

            // Skip if we just entered a raw string on this line
            if in_raw_string {
                continue;
            }

            let trimmed = line.trim();

            // Check for #[allow(...)] or #![allow(...)]
            if (trimmed.starts_with("#[allow(") || trimmed.starts_with("#![allow("))
                && trimmed.contains(')')
            {
                // Extract the lint name
                let start = trimmed.find('(').unwrap() + 1;
                let end = trimmed.rfind(')').unwrap();
                let lint = &trimmed[start..end];

                // Check if this pattern is allowed
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
    fn find_rust_files(&self) -> Result<Vec<PathBuf>> {
        let src_dir = self.project_dir.join("src");
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

impl Gate for NoAllowGate {
    fn name(&self) -> &str {
        "NoAllow"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();

        let rust_files = self.find_rust_files()?;
        let mut all_issues = Vec::new();

        for file in rust_files {
            let issues = self.scan_file(&file)?;
            all_issues.extend(issues);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let passed = all_issues.is_empty();

        Ok(if passed {
            GateResult::pass(self.name()).with_duration(duration_ms)
        } else {
            GateResult::fail(self.name(), all_issues).with_duration(duration_ms)
        })
    }
}

// ============================================================================
// Security Gate
// ============================================================================

/// Quality gate that runs security scans via narsil-mcp and cargo-audit.
///
/// Combines results from multiple security scanning sources:
/// - narsil-mcp `scan_security` tool (code analysis)
/// - cargo-audit (dependency vulnerabilities)
///
/// Results are cached within the gate instance to avoid re-running
/// expensive scans if `check()` is called multiple times.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::SecurityGate;
/// use ralph::narsil::{NarsilClient, NarsilConfig};
///
/// let client = NarsilClient::new(NarsilConfig::new("."))?;
/// let gate = SecurityGate::new(".")
///     .with_narsil_client(client)
///     .with_threshold(IssueSeverity::Warning);
///
/// let result = gate.check()?;
/// ```
pub struct SecurityGate {
    project_dir: PathBuf,
    /// Minimum severity to report.
    severity_threshold: IssueSeverity,
    /// Optional narsil-mcp client for code security scanning.
    narsil_client: Option<NarsilClient>,
    /// Cached narsil scan results (to avoid re-running within iteration).
    narsil_cache: OnceCell<Vec<GateIssue>>,
}

impl SecurityGate {
    /// Create a new security gate for the given project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            severity_threshold: IssueSeverity::Warning,
            narsil_client: None,
            narsil_cache: OnceCell::new(),
        }
    }

    /// Set the minimum severity threshold.
    #[must_use]
    pub fn with_threshold(mut self, threshold: IssueSeverity) -> Self {
        self.severity_threshold = threshold;
        self
    }

    /// Set the narsil-mcp client for code security scanning.
    ///
    /// When provided, the gate will use narsil-mcp's `scan_security` tool
    /// to find code vulnerabilities in addition to cargo-audit for
    /// dependency vulnerabilities.
    #[must_use]
    pub fn with_narsil_client(mut self, client: NarsilClient) -> Self {
        self.narsil_client = Some(client);
        self
    }

    /// Convert a narsil SecuritySeverity to a gate IssueSeverity.
    ///
    /// Mapping:
    /// - Critical -> Critical (blocking)
    /// - High -> Error (blocking)
    /// - Medium -> Warning (non-blocking)
    /// - Low -> Info (non-blocking)
    /// - Info -> Info (non-blocking)
    pub fn convert_severity(severity: SecuritySeverity) -> IssueSeverity {
        match severity {
            SecuritySeverity::Critical => IssueSeverity::Critical,
            SecuritySeverity::High => IssueSeverity::Error,
            SecuritySeverity::Medium => IssueSeverity::Warning,
            SecuritySeverity::Low => IssueSeverity::Info,
            SecuritySeverity::Info => IssueSeverity::Info,
        }
    }

    /// Convert a narsil SecurityFinding to a gate GateIssue.
    pub fn convert_finding(finding: &SecurityFinding) -> GateIssue {
        let mut issue = GateIssue::new(
            Self::convert_severity(finding.severity),
            &finding.message,
        );

        // Set file location
        issue.file = Some(finding.file.clone());

        // Set line number if available
        if let Some(line) = finding.line {
            issue.line = Some(line);
        }

        // Set rule ID as code if available
        if let Some(ref rule_id) = finding.rule_id {
            issue.code = Some(rule_id.clone());
        }

        // Set suggestion if available
        if let Some(ref suggestion) = finding.suggestion {
            issue.suggestion = Some(suggestion.clone());
        }

        issue
    }

    /// Run narsil-mcp security scan if client is available.
    ///
    /// Results are cached after the first call to avoid re-running
    /// the expensive scan multiple times within the same iteration.
    fn run_narsil_scan(&self) -> Result<Vec<GateIssue>> {
        // Return cached results if available
        if let Some(cached) = self.narsil_cache.get() {
            return Ok(cached.clone());
        }

        let Some(ref client) = self.narsil_client else {
            // No client, cache empty result
            let _ = self.narsil_cache.set(Vec::new());
            return Ok(Vec::new());
        };

        // Gracefully handle unavailable narsil-mcp
        if !client.is_available() {
            let _ = self.narsil_cache.set(Vec::new());
            return Ok(Vec::new());
        }

        match client.scan_security() {
            Ok(findings) => {
                let issues: Vec<GateIssue> = findings
                    .iter()
                    .map(Self::convert_finding)
                    .collect();
                // Cache the results
                let _ = self.narsil_cache.set(issues.clone());
                Ok(issues)
            }
            Err(e) => {
                // Log the error but don't fail the gate
                // Narsil errors are recoverable
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
    fn run_cargo_audit(&self) -> Result<Vec<GateIssue>> {
        let output = Command::new("cargo")
            .args(["audit", "--json"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse JSON output if available, otherwise return empty
                // For now, just check exit code
                if output.status.success() {
                    Ok(Vec::new())
                } else {
                    // Parse basic vulnerabilities from output
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
            Err(_) => {
                // cargo-audit not installed, skip
                Ok(Vec::new())
            }
        }
    }
}

impl Gate for SecurityGate {
    fn name(&self) -> &str {
        "Security"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();

        // Collect issues from both sources
        let mut all_issues = Vec::new();

        // Run cargo-audit for dependency vulnerabilities
        let audit_issues = self.run_cargo_audit()?;
        all_issues.extend(audit_issues);

        // Run narsil-mcp scan for code vulnerabilities
        let narsil_issues = self.run_narsil_scan()?;
        all_issues.extend(narsil_issues);

        // Filter by severity threshold
        let issues: Vec<_> = all_issues
            .into_iter()
            .filter(|i| i.severity >= self.severity_threshold)
            .collect();

        let duration_ms = start.elapsed().as_millis() as u64;
        let has_blocking = issues.iter().any(|i| i.severity.is_blocking());

        Ok(if !has_blocking {
            GateResult::pass(self.name()).with_duration(duration_ms)
        } else {
            GateResult::fail(self.name(), issues).with_duration(duration_ms)
        })
    }

    fn is_blocking(&self) -> bool {
        // Security issues are blocking by default
        true
    }
}

// ============================================================================
// No TODO Gate
// ============================================================================

/// Quality gate that checks for TODO/FIXME comments.
pub struct NoTodoGate {
    project_dir: PathBuf,
    /// Patterns to search for.
    patterns: Vec<String>,
}

impl NoTodoGate {
    /// Create a new no-todo gate for the given project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
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
    fn find_rust_files(&self) -> Result<Vec<PathBuf>> {
        let src_dir = self.project_dir.join("src");
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

impl Gate for NoTodoGate {
    fn name(&self) -> &str {
        "NoTodo"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();

        let rust_files = self.find_rust_files()?;
        let mut all_issues = Vec::new();

        for file in rust_files {
            let issues = self.scan_file(&file)?;
            all_issues.extend(issues);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // TODO gate is non-blocking by default (warnings only)
        let passed = !all_issues.iter().any(|i| i.severity.is_blocking());

        Ok(if passed {
            let mut result = GateResult::pass(self.name()).with_duration(duration_ms);
            result.issues = all_issues; // Include warnings in passing result
            result
        } else {
            GateResult::fail(self.name(), all_issues).with_duration(duration_ms)
        })
    }

    fn is_blocking(&self) -> bool {
        // TODO/FIXME comments are warnings, not blocking
        false
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_issue_severity_ordering() {
        assert!(IssueSeverity::Info < IssueSeverity::Warning);
        assert!(IssueSeverity::Warning < IssueSeverity::Error);
        assert!(IssueSeverity::Error < IssueSeverity::Critical);
    }

    #[test]
    fn test_issue_severity_blocking() {
        assert!(!IssueSeverity::Info.is_blocking());
        assert!(!IssueSeverity::Warning.is_blocking());
        assert!(IssueSeverity::Error.is_blocking());
        assert!(IssueSeverity::Critical.is_blocking());
    }

    #[test]
    fn test_gate_issue_builder() {
        let issue = GateIssue::new(IssueSeverity::Error, "test error")
            .with_location("src/main.rs", 42)
            .with_column(10)
            .with_code("E0308")
            .with_suggestion("fix it");

        assert_eq!(issue.severity, IssueSeverity::Error);
        assert_eq!(issue.message, "test error");
        assert_eq!(issue.file, Some(PathBuf::from("src/main.rs")));
        assert_eq!(issue.line, Some(42));
        assert_eq!(issue.column, Some(10));
        assert_eq!(issue.code, Some("E0308".to_string()));
        assert_eq!(issue.suggestion, Some("fix it".to_string()));
    }

    #[test]
    fn test_gate_issue_format() {
        let issue = GateIssue::new(IssueSeverity::Warning, "unused variable")
            .with_location("src/lib.rs", 10)
            .with_code("unused_variables");

        let formatted = issue.format();
        assert!(formatted.contains("[WARNING]"));
        assert!(formatted.contains("[unused_variables]"));
        assert!(formatted.contains("unused variable"));
        assert!(formatted.contains("src/lib.rs:10"));
    }

    #[test]
    fn test_gate_result_pass() {
        let result = GateResult::pass("TestGate");
        assert!(result.passed);
        assert!(result.issues.is_empty());
        assert_eq!(result.gate_name, "TestGate");
    }

    #[test]
    fn test_gate_result_fail() {
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "error 1"),
            GateIssue::new(IssueSeverity::Warning, "warning 1"),
        ];
        let result = GateResult::fail("TestGate", issues);

        assert!(!result.passed);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.count_by_severity(IssueSeverity::Error), 1);
        assert_eq!(result.count_by_severity(IssueSeverity::Warning), 1);
    }

    #[test]
    fn test_gate_result_summary() {
        let result = GateResult::pass("Clippy");
        assert!(result.summary().contains("✅"));
        assert!(result.summary().contains("PASSED"));

        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "e1"),
            GateIssue::new(IssueSeverity::Error, "e2"),
            GateIssue::new(IssueSeverity::Warning, "w1"),
        ];
        let result = GateResult::fail("Clippy", issues);
        assert!(result.summary().contains("❌"));
        assert!(result.summary().contains("FAILED"));
        assert!(result.summary().contains("2 errors"));
        assert!(result.summary().contains("1 warnings"));
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

        let gate = NoAllowGate::new(temp_dir.path());
        let result = gate.check().unwrap();

        assert!(!result.passed);
        assert_eq!(result.issues.len(), 2);
        assert!(result.issues.iter().any(|i| i.message.contains("dead_code")));
        assert!(result.issues.iter().any(|i| i.message.contains("unused_variables")));
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

        let gate = NoAllowGate::new(temp_dir.path()).with_allowed(vec!["dead_code".to_string()]);
        let result = gate.check().unwrap();

        assert!(result.passed);
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

        let gate = NoTodoGate::new(temp_dir.path());
        let result = gate.check().unwrap();

        // NoTodo is non-blocking, so it passes but has issues
        assert!(result.passed);
        assert_eq!(result.issues.len(), 2);
    }

    #[test]
    fn test_clippy_output_parsing() {
        let gate = ClippyGate::new("/tmp");

        let stderr = r#"
warning: unused variable: `x`
  --> src/main.rs:10:9
   |
10 |     let x = 5;
   |         ^ help: if this is intentional, prefix it with an underscore: `_x`
   |
   = note: `#[warn(unused_variables)]` on by default

error[E0308]: mismatched types
  --> src/lib.rs:20:5
   |
20 |     "hello"
   |     ^^^^^^^ expected `i32`, found `&str`
"#;

        let issues = gate.parse_output(stderr);

        assert_eq!(issues.len(), 2);

        // Check warning
        let warning = &issues[0];
        assert_eq!(warning.severity, IssueSeverity::Warning);
        assert!(warning.message.contains("unused variable"));

        // Check error
        let error = &issues[1];
        assert_eq!(error.severity, IssueSeverity::Error);
        assert!(error.message.contains("mismatched types"));
    }

    // =========================================================================
    // SecurityGate narsil-mcp Integration Tests
    // =========================================================================

    #[test]
    fn test_security_severity_to_issue_severity() {
        use crate::narsil::SecuritySeverity;

        // Critical narsil severity -> Critical issue severity
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Critical),
            IssueSeverity::Critical
        );

        // High narsil severity -> Error issue severity (blocking)
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::High),
            IssueSeverity::Error
        );

        // Medium narsil severity -> Warning issue severity
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Medium),
            IssueSeverity::Warning
        );

        // Low narsil severity -> Info issue severity
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Low),
            IssueSeverity::Info
        );

        // Info narsil severity -> Info issue severity
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Info),
            IssueSeverity::Info
        );
    }

    #[test]
    fn test_security_finding_to_gate_issue() {
        use crate::narsil::{SecurityFinding, SecuritySeverity};

        let finding = SecurityFinding::new(
            SecuritySeverity::High,
            "SQL injection vulnerability",
            "src/db.rs",
        )
        .with_line(42)
        .with_rule_id("CWE-89")
        .with_suggestion("Use parameterized queries");

        let issue = SecurityGate::convert_finding(&finding);

        assert_eq!(issue.severity, IssueSeverity::Error);
        assert_eq!(issue.message, "SQL injection vulnerability");
        assert_eq!(issue.file, Some(PathBuf::from("src/db.rs")));
        assert_eq!(issue.line, Some(42));
        assert_eq!(issue.code, Some("CWE-89".to_string()));
        assert_eq!(issue.suggestion, Some("Use parameterized queries".to_string()));
    }

    #[test]
    fn test_security_finding_without_optional_fields() {
        use crate::narsil::{SecurityFinding, SecuritySeverity};

        let finding = SecurityFinding::new(
            SecuritySeverity::Medium,
            "Potential vulnerability",
            "src/api.rs",
        );

        let issue = SecurityGate::convert_finding(&finding);

        assert_eq!(issue.severity, IssueSeverity::Warning);
        assert_eq!(issue.message, "Potential vulnerability");
        assert_eq!(issue.file, Some(PathBuf::from("src/api.rs")));
        assert!(issue.line.is_none());
        assert!(issue.code.is_none());
        assert!(issue.suggestion.is_none());
    }

    #[test]
    fn test_security_gate_with_narsil_client() {
        use crate::narsil::{NarsilClient, NarsilConfig};

        let temp_dir = TempDir::new().unwrap();
        let config = NarsilConfig::new(temp_dir.path())
            .with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let gate = SecurityGate::new(temp_dir.path()).with_narsil_client(client);

        // When narsil is unavailable, should still work (graceful degradation)
        let result = gate.check().unwrap();
        // Should pass since no vulnerabilities found
        assert!(result.passed);
    }

    #[test]
    fn test_security_gate_combines_audit_and_narsil() {
        // Test that both cargo-audit and narsil-mcp results are combined
        use crate::narsil::{NarsilClient, NarsilConfig};

        let temp_dir = TempDir::new().unwrap();

        // Create a Cargo.toml to make cargo-audit work
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        let config = NarsilConfig::new(temp_dir.path())
            .with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let gate = SecurityGate::new(temp_dir.path()).with_narsil_client(client);
        let result = gate.check().unwrap();

        // Should complete without panicking
        assert!(result.gate_name == "Security");
    }

    #[test]
    fn test_security_gate_caches_results() {
        // Test that calling check() multiple times returns consistent results
        // (caching is an implementation detail, but behavior should be consistent)
        use crate::narsil::{NarsilClient, NarsilConfig};

        let temp_dir = TempDir::new().unwrap();

        let config = NarsilConfig::new(temp_dir.path())
            .with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let gate = SecurityGate::new(temp_dir.path()).with_narsil_client(client);

        // Call check() multiple times
        let result1 = gate.check().unwrap();
        let result2 = gate.check().unwrap();
        let result3 = gate.check().unwrap();

        // Results should be consistent
        assert_eq!(result1.passed, result2.passed);
        assert_eq!(result2.passed, result3.passed);
        assert_eq!(result1.issues.len(), result2.issues.len());
        assert_eq!(result2.issues.len(), result3.issues.len());
    }
}
