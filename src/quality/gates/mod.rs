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
//! # Traits
//!
//! - [`Gate`] - Legacy stateful gate interface (stores project_dir)
//! - [`QualityGate`] - New stateless interface (project_dir passed to run)
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::quality::gates::{ClippyGate, Gate, GateConfig};
//!
//! // Legacy stateful usage
//! let gate = ClippyGate::new("/path/to/project");
//! let result = gate.check()?;
//! if !result.passed {
//!     eprintln!("Clippy failed: {:?}", result.issues);
//! }
//!
//! // New QualityGate usage
//! use ralph::quality::gates::{gates_for_language, QualityGate};
//! use ralph::bootstrap::Language;
//!
//! let gates = gates_for_language(Language::Rust);
//! for gate in &gates {
//!     let issues = gate.run(Path::new("/path/to/project"))?;
//!     if !issues.is_empty() {
//!         eprintln!("{}", gate.remediation(&issues));
//!     }
//! }
//! ```

pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use tracing::debug;

use std::collections::HashMap;

use crate::bootstrap::language::Language;
use crate::narsil::{NarsilClient, NarsilConfig, SecurityFinding, SecuritySeverity};

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
        self.issues
            .iter()
            .filter(|i| i.severity == severity)
            .count()
    }

    /// Get only blocking issues.
    #[must_use]
    pub fn blocking_issues(&self) -> Vec<&GateIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity.is_blocking())
            .collect()
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
// Gate Trait (Legacy - Stateful)
// ============================================================================

/// Trait for quality gates (legacy stateful interface).
///
/// Gates implementing this trait store the project directory and are
/// used by the [`QualityGateEnforcer`](super::QualityGateEnforcer).
///
/// For new code, prefer implementing [`QualityGate`] instead.
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
// QualityGate Trait (New - Stateless)
// ============================================================================

/// Trait for language-agnostic quality gates.
///
/// This trait defines a stateless interface where the project directory
/// is passed to [`run()`](QualityGate::run) rather than stored in the gate.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to support concurrent gate
/// execution in multi-threaded environments.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::{QualityGate, GateIssue, IssueSeverity};
/// use std::path::Path;
/// use anyhow::Result;
///
/// struct MyGate;
///
/// impl QualityGate for MyGate {
///     fn name(&self) -> &str { "MyGate" }
///
///     fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
///         Ok(vec![])
///     }
///
///     fn remediation(&self, issues: &[GateIssue]) -> String {
///         format!("Fix {} issues", issues.len())
///     }
/// }
/// ```
pub trait QualityGate: Send + Sync {
    /// Returns the display name of this gate.
    fn name(&self) -> &str;

    /// Runs the quality gate check on the given project.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project root directory
    ///
    /// # Returns
    ///
    /// A vector of issues found, or an empty vector if the check passes.
    ///
    /// # Errors
    ///
    /// Returns an error if the gate fails to execute (e.g., tool not found).
    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>>;

    /// Returns whether this gate blocks commits on failure.
    fn is_blocking(&self) -> bool {
        true
    }

    /// Generates remediation guidance for the given issues.
    fn remediation(&self, issues: &[GateIssue]) -> String;

    /// Returns the name of the external tool required to run this gate.
    ///
    /// Returns `None` for built-in gates that don't require external tools.
    /// Returns `Some("tool_name")` for gates that require a specific tool to be
    /// installed and accessible via PATH.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::quality::gates::QualityGate;
    ///
    /// let gate = ClippyGate::new();
    /// assert_eq!(gate.required_tool(), Some("cargo"));
    ///
    /// let builtin_gate = NoAllowGate::new();
    /// assert_eq!(builtin_gate.required_tool(), None);
    /// ```
    fn required_tool(&self) -> Option<&str> {
        None // Default: built-in gate with no external tool requirement
    }
}

// ============================================================================
// Gate Factory
// ============================================================================

/// Returns quality gates appropriate for the given language.
///
/// This factory function creates a set of gates that use the standard
/// tooling for each language. For example:
/// - Rust: ClippyGate, TestGate, NoAllowGate, SecurityGate, NoTodoGate
/// - Python: RuffGate, PytestGate, MypyGate, BanditGate (future)
///
/// For languages without specific gate implementations, returns an empty vector.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::gates_for_language;
/// use ralph::bootstrap::Language;
///
/// let rust_gates = gates_for_language(Language::Rust);
/// assert!(!rust_gates.is_empty());
/// ```
#[must_use]
pub fn gates_for_language(lang: Language) -> Vec<Box<dyn QualityGate>> {
    match lang {
        Language::Rust => rust::rust_gates(),
        Language::Python => python::python_gates(),
        Language::TypeScript | Language::JavaScript => typescript::typescript_gates(),
        Language::Go => go::go_gates(),
        _ => Vec::new(),
    }
}

// ============================================================================
// Gate Auto-Detection (Sprint 7e)
// ============================================================================

/// Checks if a tool binary is available in the system PATH.
///
/// # Arguments
///
/// * `tool_name` - Name of the tool binary to check (e.g., "cargo", "ruff")
///
/// # Returns
///
/// `true` if the tool is found in PATH, `false` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::is_tool_available;
///
/// if is_tool_available("cargo") {
///     println!("Rust toolchain is available");
/// }
/// ```
#[must_use]
pub fn is_tool_available(tool_name: &str) -> bool {
    which::which(tool_name).is_ok()
}

/// Checks if a specific gate is available for use.
///
/// A gate is considered available if:
/// 1. It has no external tool requirement (built-in gates), or
/// 2. Its required tool is installed and accessible via PATH
///
/// # Arguments
///
/// * `gate` - The quality gate to check
///
/// # Returns
///
/// `true` if the gate can be used, `false` otherwise.
#[must_use]
pub fn is_gate_available(gate: &dyn QualityGate) -> bool {
    match gate.required_tool() {
        Some(tool) => is_tool_available(tool),
        None => true, // Built-in gates are always available
    }
}

/// Detects which quality gates are available for a project.
///
/// This function combines gates for all specified languages, filters out
/// gates whose tools are not installed, and removes duplicates.
///
/// # Arguments
///
/// * `project_dir` - Path to the project directory (used for future project-specific detection)
/// * `languages` - Languages to get gates for
///
/// # Returns
///
/// A vector of available quality gates, deduplicated by name.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::detect_available_gates;
/// use ralph::bootstrap::Language;
/// use std::path::Path;
///
/// let gates = detect_available_gates(Path::new("."), &[Language::Rust, Language::Python]);
/// for gate in &gates {
///     println!("Available: {}", gate.name());
/// }
/// ```
#[must_use]
pub fn detect_available_gates(
    _project_dir: &Path,
    languages: &[Language],
) -> Vec<Box<dyn QualityGate>> {
    let mut gates: Vec<Box<dyn QualityGate>> = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for lang in languages {
        let lang_gates = gates_for_language(*lang);

        for gate in lang_gates {
            let gate_name = gate.name().to_string();

            // Skip if we already have a gate with this name (deduplication)
            if seen_names.contains(&gate_name) {
                continue;
            }

            // Check if the gate's required tool is available
            if is_gate_available(&*gate) {
                seen_names.insert(gate_name);
                gates.push(gate);
            } else {
                // Log when gate is skipped due to missing tool
                if let Some(tool) = gate.required_tool() {
                    debug!(
                        gate = %gate_name,
                        tool = %tool,
                        "Skipping gate: required tool not found in PATH"
                    );
                }
            }
        }
    }

    gates
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

        issues
    }

    /// Run clippy on a specific project directory (for QualityGate trait).
    fn run_on(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
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
        let issues = self.parse_output(&stderr);

        // Filter out allowed lints
        Ok(issues
            .into_iter()
            .filter(|issue| {
                if let Some(ref code) = issue.code {
                    !self
                        .config
                        .allowed_lints
                        .iter()
                        .any(|allowed| code.contains(allowed))
                } else {
                    true
                }
            })
            .collect())
    }
}

impl Gate for ClippyGate {
    fn name(&self) -> &str {
        "Clippy"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();
        let issues = self.run_on(&self.project_dir)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let has_blocking = issues.iter().any(|i| i.severity.is_blocking());
        let passed = !has_blocking;

        Ok(if passed {
            GateResult::pass(self.name()).with_duration(duration_ms)
        } else {
            GateResult::fail(self.name(), issues).with_duration(duration_ms)
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
            min_pass_rate: 1.0,
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

        let combined = format!("{}\n{}", stdout, stderr);

        for line in combined.lines() {
            if line.starts_with("test result:") {
                if let Some(p) = line.find(" passed") {
                    let before = &line[..p];
                    if let Some(start) = before.rfind(|c: char| !c.is_ascii_digit()) {
                        if let Ok(n) = before[start + 1..].trim().parse::<u32>() {
                            passed = n;
                        }
                    }
                }
                if let Some(f) = line.find(" failed") {
                    let before = &line[..f];
                    if let Some(start) = before.rfind(|c: char| !c.is_ascii_digit()) {
                        if let Ok(n) = before[start + 1..].trim().parse::<u32>() {
                            failed = n;
                        }
                    }
                }
            } else if line.starts_with("---- ") && line.ends_with(" ----") {
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
            1.0
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
    /// Patterns that are allowed.
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

    /// Run scan on a specific project directory (for QualityGate trait).
    fn run_on(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let rust_files = self.find_rust_files(project_dir)?;
        let mut all_issues = Vec::new();

        for file in rust_files {
            let issues = self.scan_file(&file)?;
            all_issues.extend(issues);
        }

        Ok(all_issues)
    }
}

impl Gate for NoAllowGate {
    fn name(&self) -> &str {
        "NoAllow"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();
        let all_issues = self.run_on(&self.project_dir)?;
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
pub struct SecurityGate {
    project_dir: PathBuf,
    /// Minimum severity to report.
    severity_threshold: IssueSeverity,
    /// Optional narsil-mcp client for code security scanning.
    narsil_client: Option<NarsilClient>,
    /// Cached narsil scan results.
    narsil_cache: OnceLock<Vec<GateIssue>>,
}

impl SecurityGate {
    /// Create a new security gate for the given project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            severity_threshold: IssueSeverity::Warning,
            narsil_client: None,
            narsil_cache: OnceLock::new(),
        }
    }

    /// Set the minimum severity threshold.
    #[must_use]
    pub fn with_threshold(mut self, threshold: IssueSeverity) -> Self {
        self.severity_threshold = threshold;
        self
    }

    /// Set the narsil-mcp client for code security scanning.
    #[must_use]
    pub fn with_narsil_client(mut self, client: NarsilClient) -> Self {
        self.narsil_client = Some(client);
        self
    }

    /// Convert a narsil SecuritySeverity to a gate IssueSeverity.
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

    /// Run narsil-mcp security scan if client is available.
    fn run_narsil_scan(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        if let Some(cached) = self.narsil_cache.get() {
            return Ok(cached.clone());
        }

        // Use existing client or try to create one
        if let Some(ref client) = self.narsil_client {
            return self.scan_with_client(client);
        }

        // Try to create a new client
        let config = NarsilConfig::new(project_dir);
        match NarsilClient::new(config) {
            Ok(client) => self.scan_with_client(&client),
            Err(_) => {
                let _ = self.narsil_cache.set(Vec::new());
                Ok(Vec::new())
            }
        }
    }

    /// Run security scan with the given client.
    fn scan_with_client(&self, client: &NarsilClient) -> Result<Vec<GateIssue>> {
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
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Run security scan on a specific project directory (for QualityGate trait).
    fn run_on(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let mut all_issues = Vec::new();

        let audit_issues = self.run_cargo_audit(project_dir)?;
        all_issues.extend(audit_issues);

        let narsil_issues = self.run_narsil_scan(project_dir)?;
        all_issues.extend(narsil_issues);

        Ok(all_issues
            .into_iter()
            .filter(|i| i.severity >= self.severity_threshold)
            .collect())
    }
}

impl Gate for SecurityGate {
    fn name(&self) -> &str {
        "Security"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();
        let issues = self.run_on(&self.project_dir)?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let has_blocking = issues.iter().any(|i| i.severity.is_blocking());

        Ok(if !has_blocking {
            GateResult::pass(self.name()).with_duration(duration_ms)
        } else {
            GateResult::fail(self.name(), issues).with_duration(duration_ms)
        })
    }

    fn is_blocking(&self) -> bool {
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

    /// Run scan on a specific project directory (for QualityGate trait).
    fn run_on(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        let rust_files = self.find_rust_files(project_dir)?;
        let mut all_issues = Vec::new();

        for file in rust_files {
            let issues = self.scan_file(&file)?;
            all_issues.extend(issues);
        }

        Ok(all_issues)
    }
}

impl Gate for NoTodoGate {
    fn name(&self) -> &str {
        "NoTodo"
    }

    fn check(&self) -> Result<GateResult> {
        let start = std::time::Instant::now();
        let all_issues = self.run_on(&self.project_dir)?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let passed = !all_issues.iter().any(|i| i.severity.is_blocking());

        Ok(if passed {
            let mut result = GateResult::pass(self.name()).with_duration(duration_ms);
            result.issues = all_issues;
            result
        } else {
            GateResult::fail(self.name(), all_issues).with_duration(duration_ms)
        })
    }

    fn is_blocking(&self) -> bool {
        false
    }
}

// ============================================================================
// Gate Weight Configuration (Sprint 9, Phase 9.1)
// ============================================================================

use std::collections::HashSet;

/// Configuration for weighted gate scoring.
///
/// This configuration controls how gate results are weighted based on
/// whether files of that language were changed in the current working tree.
///
/// # Default Weights
///
/// - Changed files: 1.0 (full weight)
/// - Unchanged files: 0.3 (reduced weight)
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::GateWeightConfig;
///
/// let config = GateWeightConfig::default();
/// assert_eq!(config.changed_weight, 1.0);
/// assert_eq!(config.unchanged_weight, 0.3);
///
/// // Custom weights
/// let custom = GateWeightConfig {
///     changed_weight: 1.0,
///     unchanged_weight: 0.5,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateWeightConfig {
    /// Weight applied to gates for languages with changed files.
    /// Default: 1.0
    #[serde(default = "default_changed_weight")]
    pub changed_weight: f64,

    /// Weight applied to gates for languages without changed files.
    /// Default: 0.3
    #[serde(default = "default_unchanged_weight")]
    pub unchanged_weight: f64,
}

fn default_changed_weight() -> f64 {
    1.0
}

fn default_unchanged_weight() -> f64 {
    0.3
}

impl Default for GateWeightConfig {
    fn default() -> Self {
        Self {
            changed_weight: default_changed_weight(),
            unchanged_weight: default_unchanged_weight(),
        }
    }
}

/// Detects which languages have changed files in the git working tree.
///
/// This function runs `git diff --name-only` and `git diff --cached --name-only`
/// to find all changed files (both staged and unstaged), then maps their
/// extensions to programming languages.
///
/// # Arguments
///
/// * `project_dir` - Path to the git repository root
///
/// # Returns
///
/// A set of languages that have changed files. Returns an empty set if
/// git is not available or the directory is not a git repository.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::detect_changed_languages;
/// use std::path::Path;
///
/// let changed = detect_changed_languages(Path::new("."));
/// for lang in &changed {
///     println!("Changed: {}", lang);
/// }
/// ```
#[must_use]
pub fn detect_changed_languages(project_dir: &Path) -> HashSet<Language> {
    let mut changed_languages = HashSet::new();

    // Get unstaged changes
    if let Ok(output) = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(project_dir)
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(lang) = Language::from_path(Path::new(line)) {
                    changed_languages.insert(lang);
                }
            }
        }
    }

    // Get staged changes
    if let Ok(output) = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(project_dir)
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(lang) = Language::from_path(Path::new(line)) {
                    changed_languages.insert(lang);
                }
            }
        }
    }

    changed_languages
}

// ============================================================================
// PolyglotGateResult (Sprint 7, Phase 7.4)
// ============================================================================

/// Aggregated gate results across multiple languages.
///
/// This type collects results from quality gates across all detected languages
/// in a polyglot project, providing methods to determine commit eligibility
/// and generate summaries for remediation.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::gates::{PolyglotGateResult, GateResult};
/// use ralph::bootstrap::Language;
///
/// let mut result = PolyglotGateResult::new();
/// result.add_result(Language::Rust, GateResult::pass("Clippy"));
/// result.add_result(Language::Python, GateResult::pass("Ruff"));
///
/// if result.can_commit() {
///     println!("All gates passed - safe to commit");
/// } else {
///     println!("{}", result.remediation_prompt());
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct PolyglotGateResult {
    /// Gate results organized by language.
    by_language: HashMap<Language, Vec<GateResult>>,
}

impl PolyglotGateResult {
    /// Create a new empty polyglot gate result.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_language: HashMap::new(),
        }
    }

    /// Add a gate result for a specific language.
    pub fn add_result(&mut self, language: Language, result: GateResult) {
        self.by_language.entry(language).or_default().push(result);
    }

    /// Returns all gate results organized by language.
    #[must_use]
    pub fn by_language(&self) -> &HashMap<Language, Vec<GateResult>> {
        &self.by_language
    }

    /// Check if all blocking gates passed.
    ///
    /// Returns `true` if there are no blocking failures (errors or critical issues),
    /// `false` otherwise. An empty result (no gates run) returns `true`.
    #[must_use]
    pub fn can_commit(&self) -> bool {
        self.blocking_failures().is_empty()
    }

    /// Returns gate results that failed with blocking issues.
    ///
    /// A result is considered a blocking failure if it contains any issues
    /// with severity `Error` or `Critical`.
    #[must_use]
    pub fn blocking_failures(&self) -> Vec<&GateResult> {
        self.by_language
            .values()
            .flatten()
            .filter(|result| {
                !result.passed && result.issues.iter().any(|i| i.severity.is_blocking())
            })
            .collect()
    }

    /// Returns gate results that have non-blocking warnings.
    ///
    /// A result is considered a warning if it has issues but none are blocking,
    /// or if the gate passed but reported non-blocking issues.
    #[must_use]
    pub fn warnings(&self) -> Vec<&GateResult> {
        self.by_language
            .values()
            .flatten()
            .filter(|result| {
                // Has warnings but no blocking issues
                !result.issues.is_empty() && !result.issues.iter().any(|i| i.severity.is_blocking())
            })
            .collect()
    }

    /// Generate a summary showing per-language gate counts.
    ///
    /// The summary includes the number of passed and failed gates for each
    /// language, along with issue counts by severity.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.by_language.is_empty() {
            return "No gates executed".to_string();
        }

        let mut lines = Vec::new();
        let mut total_passed = 0;
        let mut total_failed = 0;

        // Sort languages for deterministic output
        let mut languages: Vec<_> = self.by_language.keys().collect();
        languages.sort_by_key(|l| format!("{}", l));

        for lang in languages {
            let results = &self.by_language[lang];
            let passed = results.iter().filter(|r| r.passed).count();
            let failed = results.len() - passed;
            total_passed += passed;
            total_failed += failed;

            let status = if failed > 0 { "❌" } else { "✅" };
            lines.push(format!(
                "{} {}: {}/{} gates passed",
                status,
                lang,
                passed,
                results.len()
            ));

            // Add details for failed gates
            for result in results.iter().filter(|r| !r.passed) {
                let error_count = result.count_by_severity(IssueSeverity::Error);
                let critical_count = result.count_by_severity(IssueSeverity::Critical);
                let warning_count = result.count_by_severity(IssueSeverity::Warning);

                let mut counts = Vec::new();
                if critical_count > 0 {
                    counts.push(format!("{} critical", critical_count));
                }
                if error_count > 0 {
                    counts.push(format!("{} errors", error_count));
                }
                if warning_count > 0 {
                    counts.push(format!("{} warnings", warning_count));
                }

                if !counts.is_empty() {
                    lines.push(format!("   └─ {}: {}", result.gate_name, counts.join(", ")));
                }
            }
        }

        // Add overall summary
        lines.push(String::new());
        if total_failed == 0 {
            lines.push(format!("✅ All {} gates passed", total_passed));
        } else {
            lines.push(format!(
                "❌ {}/{} gates failed",
                total_failed,
                total_passed + total_failed
            ));
        }

        lines.join("\n")
    }

    /// Generate a remediation prompt for Claude feedback.
    ///
    /// This creates a structured prompt that describes the failures and
    /// provides guidance for fixing them.
    #[must_use]
    pub fn remediation_prompt(&self) -> String {
        let blocking = self.blocking_failures();
        let warnings = self.warnings();

        if blocking.is_empty() && warnings.is_empty() {
            return "All quality gates passed. Safe to commit.".to_string();
        }

        let mut prompt = String::new();

        if !blocking.is_empty() {
            prompt.push_str("## Blocking Issues (Must Fix Before Commit)\n\n");

            for result in &blocking {
                prompt.push_str(&format!("### {} Gate Failed\n\n", result.gate_name));

                for issue in result.blocking_issues() {
                    prompt.push_str(&format!("- **[{}]** {}\n", issue.severity, issue.message));

                    if let Some(ref file) = issue.file {
                        if let Some(line) = issue.line {
                            prompt.push_str(&format!(
                                "  - Location: {}:{}\n",
                                file.display(),
                                line
                            ));
                        } else {
                            prompt.push_str(&format!("  - Location: {}\n", file.display()));
                        }
                    }

                    if let Some(ref suggestion) = issue.suggestion {
                        prompt.push_str(&format!("  - Fix: {}\n", suggestion));
                    }
                }

                prompt.push('\n');
            }
        }

        if !warnings.is_empty() {
            prompt.push_str("## Warnings (Non-blocking)\n\n");

            for result in &warnings {
                prompt.push_str(&format!("### {} Gate Warnings\n\n", result.gate_name));

                for issue in &result.issues {
                    prompt.push_str(&format!("- **[{}]** {}\n", issue.severity, issue.message));

                    if let Some(ref file) = issue.file {
                        if let Some(line) = issue.line {
                            prompt.push_str(&format!(
                                "  - Location: {}:{}\n",
                                file.display(),
                                line
                            ));
                        } else {
                            prompt.push_str(&format!("  - Location: {}\n", file.display()));
                        }
                    }
                }

                prompt.push('\n');
            }
        }

        prompt
    }

    // =========================================================================
    // Weighted Scoring Methods (Sprint 9, Phase 9.1)
    // =========================================================================

    /// Compute weights for each language based on changed files.
    ///
    /// Languages with changed files get the `changed_weight`, while languages
    /// without changes get the `unchanged_weight`.
    ///
    /// # Arguments
    ///
    /// * `changed_languages` - Set of languages that have changed files
    /// * `config` - Weight configuration
    ///
    /// # Returns
    ///
    /// A map of language to weight value.
    #[must_use]
    pub fn compute_weights(
        &self,
        changed_languages: &HashSet<Language>,
        config: &GateWeightConfig,
    ) -> HashMap<Language, f64> {
        self.by_language
            .keys()
            .map(|lang| {
                let weight = if changed_languages.contains(lang) {
                    config.changed_weight
                } else {
                    config.unchanged_weight
                };
                (*lang, weight)
            })
            .collect()
    }

    /// Compute a weighted score for the gate results.
    ///
    /// The score is computed as the weighted sum of passing gates divided by
    /// the total weighted sum. Returns 1.0 if no gates were run.
    ///
    /// # Arguments
    ///
    /// * `changed_languages` - Set of languages that have changed files
    /// * `config` - Weight configuration
    ///
    /// # Returns
    ///
    /// A score between 0.0 and 1.0, where 1.0 means all gates passed.
    #[must_use]
    pub fn weighted_score(
        &self,
        changed_languages: &HashSet<Language>,
        config: &GateWeightConfig,
    ) -> f64 {
        let weights = self.compute_weights(changed_languages, config);

        let mut weighted_passed = 0.0;
        let mut weighted_total = 0.0;

        for (lang, results) in &self.by_language {
            let weight = weights.get(lang).copied().unwrap_or(config.unchanged_weight);

            for result in results {
                weighted_total += weight;
                if result.passed {
                    weighted_passed += weight;
                }
            }
        }

        if weighted_total == 0.0 {
            1.0 // No gates means perfect score
        } else {
            weighted_passed / weighted_total
        }
    }

    /// Check if commit is allowed using weighted scoring.
    ///
    /// Returns `true` if:
    /// 1. There are no blocking failures (errors or critical issues), AND
    /// 2. The weighted score is acceptable (non-blocking issues in low-weight
    ///    languages don't prevent commit)
    ///
    /// Blocking failures (Error or Critical severity) always block commit
    /// regardless of the language's weight.
    ///
    /// # Arguments
    ///
    /// * `changed_languages` - Set of languages that have changed files
    /// * `config` - Weight configuration
    ///
    /// # Returns
    ///
    /// `true` if commit is allowed, `false` otherwise.
    #[must_use]
    pub fn can_commit_weighted(
        &self,
        changed_languages: &HashSet<Language>,
        config: &GateWeightConfig,
    ) -> bool {
        // Blocking failures always block, regardless of weight
        if !self.blocking_failures().is_empty() {
            return false;
        }

        // If no blocking failures, check weighted score
        // Currently, we allow commit if there are no blocking issues
        // The weighted score is informational for non-blocking issues
        let _score = self.weighted_score(changed_languages, config);

        true
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
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("dead_code")));
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("unused_variables")));
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
    fn test_no_allow_gate_skips_raw_string_literals() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("lib.rs");
        std::fs::write(
            &file_path,
            r##"
fn clean_function() {
    // This file tests raw string handling
}

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

        let gate = NoAllowGate::new(temp_dir.path());
        let result = gate.check().unwrap();

        assert!(
            result.passed,
            "Gate should pass - #[allow] inside raw strings should be skipped"
        );
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_no_allow_gate_handles_multiline_raw_strings() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let file_path = src_dir.join("lib.rs");
        std::fs::write(
            &file_path,
            r###"
fn main() {}

const TEST_CODE: &str = r#"
// Test fixture with allow annotation
#[allow(unused_variables)]
fn test_fn() {
    let x = 5;
}
"#;
"###,
        )
        .unwrap();

        let gate = NoAllowGate::new(temp_dir.path());
        let result = gate.check().unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_no_allow_gate_passes_on_this_project() {
        // Self-check: Verify the NoAllowGate passes on this project.
        // All #[allow] patterns should be inside raw string literals.
        let project_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let gate = NoAllowGate::new(project_dir);
        let result = gate.check().unwrap();

        if !result.passed {
            for issue in &result.issues {
                eprintln!("Issue: {}", issue.message);
                if let (Some(file), Some(line)) = (&issue.file, issue.line) {
                    eprintln!("  at {}:{}", file.display(), line);
                }
            }
        }

        assert!(
            result.passed,
            "NoAllowGate should pass on this project - all #[allow] should be in raw strings"
        );
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

        let warning = &issues[0];
        assert_eq!(warning.severity, IssueSeverity::Warning);
        assert!(warning.message.contains("unused variable"));

        let error = &issues[1];
        assert_eq!(error.severity, IssueSeverity::Error);
        assert!(error.message.contains("mismatched types"));
    }

    // =========================================================================
    // SecurityGate narsil-mcp Integration Tests
    // =========================================================================

    #[test]
    fn test_security_severity_to_issue_severity() {
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Critical),
            IssueSeverity::Critical
        );
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::High),
            IssueSeverity::Error
        );
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Medium),
            IssueSeverity::Warning
        );
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Low),
            IssueSeverity::Info
        );
        assert_eq!(
            SecurityGate::convert_severity(SecuritySeverity::Info),
            IssueSeverity::Info
        );
    }

    #[test]
    fn test_security_finding_to_gate_issue() {
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
        assert_eq!(
            issue.suggestion,
            Some("Use parameterized queries".to_string())
        );
    }

    #[test]
    fn test_security_finding_without_optional_fields() {
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
        let temp_dir = TempDir::new().unwrap();
        let config = NarsilConfig::new(temp_dir.path()).with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let gate = SecurityGate::new(temp_dir.path()).with_narsil_client(client);

        let result = gate.check().unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_security_gate_combines_audit_and_narsil() {
        let temp_dir = TempDir::new().unwrap();

        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        let config = NarsilConfig::new(temp_dir.path()).with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let gate = SecurityGate::new(temp_dir.path()).with_narsil_client(client);
        let result = gate.check().unwrap();

        assert_eq!(result.gate_name, "Security");
    }

    #[test]
    fn test_security_gate_caches_results() {
        let temp_dir = TempDir::new().unwrap();

        let config = NarsilConfig::new(temp_dir.path()).with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let gate = SecurityGate::new(temp_dir.path()).with_narsil_client(client);

        let result1 = gate.check().unwrap();
        let result2 = gate.check().unwrap();
        let result3 = gate.check().unwrap();

        assert_eq!(result1.passed, result2.passed);
        assert_eq!(result2.passed, result3.passed);
        assert_eq!(result1.issues.len(), result2.issues.len());
        assert_eq!(result2.issues.len(), result3.issues.len());
    }

    // =========================================================================
    // QualityGate trait tests
    // =========================================================================

    #[test]
    fn test_gates_for_language_rust_not_empty() {
        let gates = gates_for_language(Language::Rust);
        assert!(!gates.is_empty(), "Rust should have quality gates");
    }

    #[test]
    fn test_gates_for_language_rust_has_expected_gates() {
        let gates = gates_for_language(Language::Rust);
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"Clippy"), "Should have Clippy gate");
        assert!(names.contains(&"Tests"), "Should have Tests gate");
        assert!(names.contains(&"NoAllow"), "Should have NoAllow gate");
        assert!(names.contains(&"Security"), "Should have Security gate");
        assert!(names.contains(&"NoTodo"), "Should have NoTodo gate");
    }

    #[test]
    fn test_gates_for_language_unsupported_returns_empty() {
        let gates = gates_for_language(Language::Sql);
        assert!(
            gates.is_empty(),
            "Unsupported languages should return empty"
        );
    }

    #[test]
    fn test_quality_gates_are_send_sync() {
        let gates = gates_for_language(Language::Rust);
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_quality_gates_have_remediation() {
        let gates = gates_for_language(Language::Rust);
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
    // Python QualityGate tests
    // =========================================================================

    #[test]
    fn test_gates_for_language_python_not_empty() {
        let gates = gates_for_language(Language::Python);
        assert!(!gates.is_empty(), "Python should have quality gates");
    }

    #[test]
    fn test_gates_for_language_python_has_expected_gates() {
        let gates = gates_for_language(Language::Python);
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"Ruff"), "Should have Ruff gate");
        assert!(names.contains(&"Pytest"), "Should have Pytest gate");
        assert!(names.contains(&"Mypy"), "Should have Mypy gate");
        assert!(names.contains(&"Bandit"), "Should have Bandit gate");
    }

    #[test]
    fn test_python_quality_gates_are_send_sync() {
        let gates = gates_for_language(Language::Python);
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_python_quality_gates_have_remediation() {
        let gates = gates_for_language(Language::Python);
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
    // TypeScript/JavaScript QualityGate tests
    // =========================================================================

    #[test]
    fn test_gates_for_language_typescript_not_empty() {
        let gates = gates_for_language(Language::TypeScript);
        assert!(!gates.is_empty(), "TypeScript should have quality gates");
    }

    #[test]
    fn test_gates_for_language_javascript_not_empty() {
        let gates = gates_for_language(Language::JavaScript);
        assert!(!gates.is_empty(), "JavaScript should have quality gates");
    }

    #[test]
    fn test_gates_for_language_typescript_has_expected_gates() {
        let gates = gates_for_language(Language::TypeScript);
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"ESLint"), "Should have ESLint gate");
        assert!(names.contains(&"Jest"), "Should have Jest gate");
        assert!(names.contains(&"TypeScript"), "Should have TypeScript gate");
        assert!(names.contains(&"npm-audit"), "Should have npm-audit gate");
    }

    #[test]
    fn test_gates_for_language_javascript_has_expected_gates() {
        let gates = gates_for_language(Language::JavaScript);
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"ESLint"), "Should have ESLint gate");
        assert!(names.contains(&"Jest"), "Should have Jest gate");
        assert!(names.contains(&"npm-audit"), "Should have npm-audit gate");
    }

    #[test]
    fn test_typescript_quality_gates_are_send_sync() {
        let gates = gates_for_language(Language::TypeScript);
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_typescript_quality_gates_have_remediation() {
        let gates = gates_for_language(Language::TypeScript);
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
    // Go QualityGate tests
    // =========================================================================

    #[test]
    fn test_gates_for_language_go_not_empty() {
        let gates = gates_for_language(Language::Go);
        assert!(!gates.is_empty(), "Go should have quality gates");
    }

    #[test]
    fn test_gates_for_language_go_has_expected_gates() {
        let gates = gates_for_language(Language::Go);
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        assert!(names.contains(&"go-vet"), "Should have go-vet gate");
        assert!(
            names.contains(&"golangci-lint"),
            "Should have golangci-lint gate"
        );
        assert!(names.contains(&"go-test"), "Should have go-test gate");
        assert!(
            names.contains(&"govulncheck"),
            "Should have govulncheck gate"
        );
    }

    #[test]
    fn test_go_quality_gates_are_send_sync() {
        let gates = gates_for_language(Language::Go);
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        for gate in &gates {
            assert_send_sync(gate);
        }
    }

    #[test]
    fn test_go_quality_gates_have_remediation() {
        let gates = gates_for_language(Language::Go);
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
    // Gate Auto-Detection tests (Sprint 7e)
    // =========================================================================

    #[test]
    fn test_is_tool_available_returns_true_for_cargo() {
        // cargo should always be available on a system running Rust tests
        assert!(is_tool_available("cargo"), "cargo should be available");
    }

    #[test]
    fn test_is_tool_available_returns_false_for_nonexistent() {
        assert!(
            !is_tool_available("nonexistent_tool_that_definitely_does_not_exist_xyz123"),
            "nonexistent tool should not be available"
        );
    }

    #[test]
    fn test_detect_available_gates_returns_gates_for_rust() {
        let temp_dir = TempDir::new().unwrap();
        // Create a minimal Cargo.toml to indicate a Rust project
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        let gates = detect_available_gates(temp_dir.path(), &[Language::Rust]);

        // At minimum, Rust should have Clippy and Test gates available
        // since cargo is available (we're running tests!)
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();
        assert!(
            names.contains(&"Clippy") || names.contains(&"Tests"),
            "Rust project should have at least one gate available. Got: {:?}",
            names
        );
    }

    #[test]
    fn test_detect_available_gates_returns_empty_for_unavailable_tools() {
        let temp_dir = TempDir::new().unwrap();

        // Python gates should not be available if ruff/pytest/etc aren't installed
        // We test with an empty project dir - no manifests or tools configured
        let gates = detect_available_gates(temp_dir.path(), &[Language::Python]);

        // Either returns empty or only returns gates for tools that happen to be installed
        // This test verifies the function doesn't panic and returns a reasonable result
        for gate in &gates {
            // Each returned gate should have a valid name
            assert!(!gate.name().is_empty(), "Gate should have a name");
        }
    }

    #[test]
    fn test_detect_available_gates_combines_multiple_languages() {
        let temp_dir = TempDir::new().unwrap();

        // Create markers for both Rust and Python projects
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test"
version = "0.1.0"
"#,
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("pyproject.toml"),
            "[project]\nname = \"test\"",
        )
        .unwrap();

        let gates = detect_available_gates(temp_dir.path(), &[Language::Rust, Language::Python]);

        // Should have gates from both languages (at least Rust gates since cargo is available)
        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        // Verify we get Rust gates
        let has_rust_gate = names.iter().any(|n| *n == "Clippy" || *n == "Tests");
        assert!(
            has_rust_gate,
            "Polyglot project should have Rust gates. Got: {:?}",
            names
        );
    }

    #[test]
    fn test_detect_available_gates_deduplicates() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        // Request same language twice
        let gates = detect_available_gates(temp_dir.path(), &[Language::Rust, Language::Rust]);

        let names: Vec<_> = gates.iter().map(|g| g.name()).collect();

        // Count occurrences of each gate name
        for name in &names {
            let count = names.iter().filter(|n| *n == name).count();
            assert_eq!(count, 1, "Gate '{}' should appear exactly once", name);
        }
    }

    #[test]
    fn test_detect_available_gates_handles_empty_languages() {
        let temp_dir = TempDir::new().unwrap();

        let gates = detect_available_gates(temp_dir.path(), &[]);

        // Empty languages should return empty gates (or possibly just security gate)
        // This verifies the function handles edge cases gracefully
        assert!(
            gates.len() <= 1,
            "Empty languages should return minimal gates. Got: {} gates",
            gates.len()
        );
    }

    // =========================================================================
    // Gate Availability Tests - required_tool() Trait Method
    // =========================================================================

    #[test]
    fn test_python_ruff_gate_requires_ruff() {
        let gate = super::python::RuffGate::new();
        assert_eq!(gate.required_tool(), Some("ruff"));
    }

    #[test]
    fn test_python_pytest_gate_requires_pytest() {
        let gate = super::python::PytestGate::new();
        assert_eq!(gate.required_tool(), Some("pytest"));
    }

    #[test]
    fn test_python_mypy_gate_requires_mypy() {
        let gate = super::python::MypyGate::new();
        assert_eq!(gate.required_tool(), Some("mypy"));
    }

    #[test]
    fn test_python_bandit_gate_requires_bandit() {
        let gate = super::python::BanditGate::new();
        assert_eq!(gate.required_tool(), Some("bandit"));
    }

    #[test]
    fn test_typescript_eslint_gate_requires_npx() {
        let gate = super::typescript::EslintGate::new();
        assert_eq!(gate.required_tool(), Some("npx"));
    }

    #[test]
    fn test_typescript_jest_gate_requires_npx() {
        let gate = super::typescript::JestGate::new();
        assert_eq!(gate.required_tool(), Some("npx"));
    }

    #[test]
    fn test_typescript_tsc_gate_requires_npx() {
        let gate = super::typescript::TscGate::new();
        assert_eq!(gate.required_tool(), Some("npx"));
    }

    #[test]
    fn test_typescript_npm_audit_gate_requires_npm() {
        let gate = super::typescript::NpmAuditGate::new();
        assert_eq!(gate.required_tool(), Some("npm"));
    }

    #[test]
    fn test_go_vet_gate_requires_go() {
        let gate = super::go::GoVetGate::new();
        assert_eq!(gate.required_tool(), Some("go"));
    }

    #[test]
    fn test_go_golangci_lint_gate_requires_golangci_lint() {
        let gate = super::go::GolangciLintGate::new();
        assert_eq!(gate.required_tool(), Some("golangci-lint"));
    }

    #[test]
    fn test_go_test_gate_requires_go() {
        let gate = super::go::GoTestGate::new();
        assert_eq!(gate.required_tool(), Some("go"));
    }

    #[test]
    fn test_go_govulncheck_gate_requires_govulncheck() {
        let gate = super::go::GovulncheckGate::new();
        assert_eq!(gate.required_tool(), Some("govulncheck"));
    }

    #[test]
    fn test_rust_clippy_gate_requires_cargo() {
        let gate = super::rust::ClippyGate::new();
        assert_eq!(gate.required_tool(), Some("cargo"));
    }

    #[test]
    fn test_rust_tests_gate_requires_cargo() {
        let gate = super::rust::CargoTestGate::new();
        assert_eq!(gate.required_tool(), Some("cargo"));
    }

    #[test]
    fn test_no_allow_gate_has_no_required_tool() {
        let gate = super::rust::NoAllowGate::new();
        assert_eq!(gate.required_tool(), None);
    }

    #[test]
    fn test_no_todo_gate_has_no_required_tool() {
        let gate = super::rust::NoTodoGate::new();
        assert_eq!(gate.required_tool(), None);
    }

    // =========================================================================
    // PolyglotGateResult tests (Sprint 7, Phase 7.4)
    // =========================================================================

    #[test]
    fn test_polyglot_gate_result_can_commit_returns_true_when_all_gates_pass() {
        let mut result = super::PolyglotGateResult::new();

        // Add passing results for multiple languages
        result.add_result(Language::Rust, GateResult::pass("Clippy"));
        result.add_result(Language::Rust, GateResult::pass("Tests"));
        result.add_result(Language::Python, GateResult::pass("Ruff"));
        result.add_result(Language::Python, GateResult::pass("Pytest"));

        assert!(
            result.can_commit(),
            "can_commit() should return true when all gates pass"
        );
    }

    #[test]
    fn test_polyglot_gate_result_can_commit_returns_false_when_blocking_gate_fails() {
        let mut result = super::PolyglotGateResult::new();

        // Add passing results
        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        // Add a failing result with blocking error
        let issues = vec![GateIssue::new(IssueSeverity::Error, "compilation error")];
        result.add_result(Language::Rust, GateResult::fail("Tests", issues));

        assert!(
            !result.can_commit(),
            "can_commit() should return false when any blocking gate fails"
        );
    }

    #[test]
    fn test_polyglot_gate_result_can_commit_returns_true_with_warnings_only() {
        let mut result = super::PolyglotGateResult::new();

        // Add a result with only warnings (non-blocking)
        let issues = vec![GateIssue::new(IssueSeverity::Warning, "unused variable")];
        let mut gate_result = GateResult::pass("NoTodo");
        gate_result.issues = issues;
        result.add_result(Language::Rust, gate_result);

        assert!(
            result.can_commit(),
            "can_commit() should return true when only warnings exist"
        );
    }

    #[test]
    fn test_polyglot_gate_result_can_commit_returns_true_when_empty() {
        let result = super::PolyglotGateResult::new();

        assert!(
            result.can_commit(),
            "can_commit() should return true when no gates run (empty is success)"
        );
    }

    #[test]
    fn test_polyglot_gate_result_can_commit_returns_false_on_critical() {
        let mut result = super::PolyglotGateResult::new();

        // Add critical issue
        let issues = vec![GateIssue::new(
            IssueSeverity::Critical,
            "security vulnerability",
        )];
        result.add_result(Language::Rust, GateResult::fail("Security", issues));

        assert!(
            !result.can_commit(),
            "can_commit() should return false when critical issue exists"
        );
    }

    #[test]
    fn test_polyglot_gate_result_summary_shows_per_language_breakdown() {
        let mut result = super::PolyglotGateResult::new();

        result.add_result(Language::Rust, GateResult::pass("Clippy"));
        result.add_result(Language::Rust, GateResult::pass("Tests"));
        result.add_result(Language::Python, GateResult::pass("Ruff"));

        let summary = result.summary();

        // Should mention each language
        assert!(
            summary.contains("Rust"),
            "Summary should mention Rust. Got: {}",
            summary
        );
        assert!(
            summary.contains("Python"),
            "Summary should mention Python. Got: {}",
            summary
        );

        // Should show pass counts
        assert!(
            summary.contains("2/2"),
            "Summary should show 2/2 for Rust. Got: {}",
            summary
        );
        assert!(
            summary.contains("1/1"),
            "Summary should show 1/1 for Python. Got: {}",
            summary
        );
    }

    #[test]
    fn test_polyglot_gate_result_summary_shows_failure_details() {
        let mut result = super::PolyglotGateResult::new();

        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "test failure 1"),
            GateIssue::new(IssueSeverity::Error, "test failure 2"),
            GateIssue::new(IssueSeverity::Warning, "warning 1"),
        ];
        result.add_result(Language::Rust, GateResult::fail("Tests", issues));

        let summary = result.summary();

        // Should show failure indicator
        assert!(
            summary.contains("❌"),
            "Summary should show failure indicator. Got: {}",
            summary
        );
        assert!(
            summary.contains("Tests"),
            "Summary should mention failed gate name. Got: {}",
            summary
        );
        assert!(
            summary.contains("2 errors"),
            "Summary should show error count. Got: {}",
            summary
        );
    }

    #[test]
    fn test_polyglot_gate_result_summary_returns_no_gates_when_empty() {
        let result = super::PolyglotGateResult::new();
        let summary = result.summary();

        assert!(
            summary.contains("No gates executed"),
            "Empty result should say no gates executed. Got: {}",
            summary
        );
    }

    #[test]
    fn test_polyglot_gate_result_blocking_failures_returns_only_blocking() {
        let mut result = super::PolyglotGateResult::new();

        // Add passing gate
        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        // Add warning-only gate (non-blocking)
        let warning_issues = vec![GateIssue::new(IssueSeverity::Warning, "unused var")];
        let mut warning_result = GateResult::pass("NoTodo");
        warning_result.issues = warning_issues;
        result.add_result(Language::Rust, warning_result);

        // Add blocking failure
        let error_issues = vec![GateIssue::new(IssueSeverity::Error, "compile error")];
        result.add_result(Language::Rust, GateResult::fail("Tests", error_issues));

        let blocking = result.blocking_failures();

        assert_eq!(
            blocking.len(),
            1,
            "Should have exactly 1 blocking failure. Got: {}",
            blocking.len()
        );
        assert_eq!(
            blocking[0].gate_name, "Tests",
            "Blocking failure should be Tests gate"
        );
    }

    #[test]
    fn test_polyglot_gate_result_warnings_returns_non_blocking_issues() {
        let mut result = super::PolyglotGateResult::new();

        // Add passing gate (no issues)
        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        // Add warning-only gate
        let warning_issues = vec![
            GateIssue::new(IssueSeverity::Warning, "unused var"),
            GateIssue::new(IssueSeverity::Info, "consider refactoring"),
        ];
        let mut warning_result = GateResult::pass("NoTodo");
        warning_result.issues = warning_issues;
        result.add_result(Language::Rust, warning_result);

        // Add blocking failure (should NOT appear in warnings)
        let error_issues = vec![GateIssue::new(IssueSeverity::Error, "compile error")];
        result.add_result(Language::Rust, GateResult::fail("Tests", error_issues));

        let warnings = result.warnings();

        assert_eq!(
            warnings.len(),
            1,
            "Should have exactly 1 warning result. Got: {}",
            warnings.len()
        );
        assert_eq!(
            warnings[0].gate_name, "NoTodo",
            "Warning should be from NoTodo gate"
        );
    }

    #[test]
    fn test_polyglot_gate_result_warnings_returns_empty_when_no_warnings() {
        let mut result = super::PolyglotGateResult::new();

        // Only passing gates with no issues
        result.add_result(Language::Rust, GateResult::pass("Clippy"));
        result.add_result(Language::Rust, GateResult::pass("Tests"));

        let warnings = result.warnings();

        assert!(
            warnings.is_empty(),
            "Should have no warnings when all gates pass cleanly"
        );
    }

    #[test]
    fn test_polyglot_gate_result_remediation_prompt_all_pass() {
        let mut result = super::PolyglotGateResult::new();

        result.add_result(Language::Rust, GateResult::pass("Clippy"));
        result.add_result(Language::Python, GateResult::pass("Ruff"));

        let prompt = result.remediation_prompt();

        assert!(
            prompt.contains("All quality gates passed"),
            "Should indicate all gates passed. Got: {}",
            prompt
        );
    }

    #[test]
    fn test_polyglot_gate_result_remediation_prompt_shows_blocking_issues() {
        let mut result = super::PolyglotGateResult::new();

        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "undefined variable 'x'")
                .with_location("src/main.rs", 42)
                .with_suggestion("Define variable 'x' before use"),
        ];
        result.add_result(Language::Rust, GateResult::fail("Clippy", issues));

        let prompt = result.remediation_prompt();

        assert!(
            prompt.contains("Blocking Issues"),
            "Should have blocking issues section. Got: {}",
            prompt
        );
        assert!(
            prompt.contains("Clippy Gate Failed"),
            "Should mention Clippy gate. Got: {}",
            prompt
        );
        assert!(
            prompt.contains("undefined variable"),
            "Should include issue message. Got: {}",
            prompt
        );
        assert!(
            prompt.contains("src/main.rs:42"),
            "Should include location. Got: {}",
            prompt
        );
        assert!(
            prompt.contains("Define variable"),
            "Should include suggestion. Got: {}",
            prompt
        );
    }

    #[test]
    fn test_polyglot_gate_result_remediation_prompt_shows_warnings() {
        let mut result = super::PolyglotGateResult::new();

        let warning_issues = vec![GateIssue::new(IssueSeverity::Warning, "unused import")];
        let mut warning_result = GateResult::pass("Clippy");
        warning_result.issues = warning_issues;
        result.add_result(Language::Rust, warning_result);

        let prompt = result.remediation_prompt();

        assert!(
            prompt.contains("Warnings"),
            "Should have warnings section. Got: {}",
            prompt
        );
        assert!(
            prompt.contains("unused import"),
            "Should include warning message. Got: {}",
            prompt
        );
    }

    #[test]
    fn test_polyglot_gate_result_by_language_accessor() {
        let mut result = super::PolyglotGateResult::new();

        result.add_result(Language::Rust, GateResult::pass("Clippy"));
        result.add_result(Language::Python, GateResult::pass("Ruff"));
        result.add_result(Language::Rust, GateResult::pass("Tests"));

        let by_lang = result.by_language();

        assert_eq!(by_lang.len(), 2, "Should have 2 languages");
        assert_eq!(
            by_lang.get(&Language::Rust).map(|v| v.len()),
            Some(2),
            "Rust should have 2 results"
        );
        assert_eq!(
            by_lang.get(&Language::Python).map(|v| v.len()),
            Some(1),
            "Python should have 1 result"
        );
    }

    #[test]
    fn test_polyglot_gate_result_default() {
        let result = super::PolyglotGateResult::default();

        assert!(result.can_commit(), "Default result should allow commit");
        assert!(
            result.by_language().is_empty(),
            "Default result should be empty"
        );
    }

    // =========================================================================
    // Weighted Gate Scoring tests (Sprint 9, Phase 9.1)
    // =========================================================================

    #[test]
    fn test_weighted_scoring_changed_files_weighted_higher() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let mut changed_languages = HashSet::new();
        changed_languages.insert(Language::Python);

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        // Add passing gates for Python (changed) and Rust (unchanged)
        result.add_result(Language::Python, GateResult::pass("Ruff"));
        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        // Compute weights based on changed languages
        let weights = result.compute_weights(&changed_languages, &config);

        // Python should have full weight (1.0)
        assert!(
            (weights.get(&Language::Python).copied().unwrap_or(0.0) - 1.0).abs() < f64::EPSILON,
            "Changed language should have weight 1.0"
        );

        // Rust should have reduced weight (0.3)
        assert!(
            (weights.get(&Language::Rust).copied().unwrap_or(0.0) - 0.3).abs() < f64::EPSILON,
            "Unchanged language should have weight 0.3"
        );
    }

    #[test]
    fn test_weighted_scoring_unchanged_gates_contribute_less() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let mut changed_languages = HashSet::new();
        changed_languages.insert(Language::Python);

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        // Add failing gate for unchanged language (Rust)
        let issues = vec![GateIssue::new(IssueSeverity::Warning, "unused variable")];
        let mut gate_result = GateResult::fail("Clippy", issues);
        gate_result.passed = false;
        result.add_result(Language::Rust, gate_result);

        // Add passing gate for changed language (Python)
        result.add_result(Language::Python, GateResult::pass("Ruff"));

        // Weighted score should still allow commit because the failed gate
        // is in an unchanged language with reduced weight
        let score = result.weighted_score(&changed_languages, &config);

        // With Python (1.0 weight) passing and Rust (0.3 weight) failing,
        // the weighted pass rate should be > threshold
        assert!(
            score > 0.5,
            "Weighted score should be high when changed language passes"
        );
    }

    #[test]
    fn test_weighted_scoring_blocking_failures_always_block() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let mut changed_languages = HashSet::new();
        changed_languages.insert(Language::Python);

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        // Add blocking error in unchanged language (Rust)
        let issues = vec![GateIssue::new(IssueSeverity::Error, "compilation error")];
        result.add_result(Language::Rust, GateResult::fail("Clippy", issues));

        // Add passing gate for changed language (Python)
        result.add_result(Language::Python, GateResult::pass("Ruff"));

        // Despite the weighted score, blocking failures should still block
        assert!(
            !result.can_commit_weighted(&changed_languages, &config),
            "Blocking failures should always block regardless of weight"
        );
    }

    #[test]
    fn test_weighted_scoring_critical_always_blocks() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let mut changed_languages = HashSet::new();
        changed_languages.insert(Language::Python);

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        // Add critical issue in unchanged language
        let issues = vec![GateIssue::new(
            IssueSeverity::Critical,
            "security vulnerability",
        )];
        result.add_result(Language::Rust, GateResult::fail("Security", issues));

        // Add passing gates for changed language
        result.add_result(Language::Python, GateResult::pass("Ruff"));
        result.add_result(Language::Python, GateResult::pass("Bandit"));

        // Critical issues should always block, even in unchanged languages
        assert!(
            !result.can_commit_weighted(&changed_languages, &config),
            "Critical issues should always block regardless of weight"
        );
    }

    #[test]
    fn test_weighted_scoring_config_default_values() {
        use super::GateWeightConfig;

        let config = GateWeightConfig::default();

        assert!(
            (config.changed_weight - 1.0).abs() < f64::EPSILON,
            "Default changed weight should be 1.0"
        );
        assert!(
            (config.unchanged_weight - 0.3).abs() < f64::EPSILON,
            "Default unchanged weight should be 0.3"
        );
    }

    #[test]
    fn test_weighted_scoring_configurable_weights() {
        use super::GateWeightConfig;

        let config = GateWeightConfig {
            changed_weight: 1.0,
            unchanged_weight: 0.5, // Higher than default
        };

        assert!(
            (config.unchanged_weight - 0.5).abs() < f64::EPSILON,
            "Custom unchanged weight should be configurable"
        );
    }

    #[test]
    fn test_weighted_scoring_all_languages_changed() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let mut changed_languages = HashSet::new();
        changed_languages.insert(Language::Python);
        changed_languages.insert(Language::Rust);

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        result.add_result(Language::Python, GateResult::pass("Ruff"));
        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        let weights = result.compute_weights(&changed_languages, &config);

        // Both should have full weight
        assert!(
            (weights.get(&Language::Python).copied().unwrap_or(0.0) - 1.0).abs() < f64::EPSILON,
            "Python should have full weight when changed"
        );
        assert!(
            (weights.get(&Language::Rust).copied().unwrap_or(0.0) - 1.0).abs() < f64::EPSILON,
            "Rust should have full weight when changed"
        );
    }

    #[test]
    fn test_weighted_scoring_no_languages_changed() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let changed_languages: HashSet<Language> = HashSet::new();

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        result.add_result(Language::Python, GateResult::pass("Ruff"));
        result.add_result(Language::Rust, GateResult::pass("Clippy"));

        let weights = result.compute_weights(&changed_languages, &config);

        // All should have reduced weight
        assert!(
            (weights.get(&Language::Python).copied().unwrap_or(0.0) - 0.3).abs() < f64::EPSILON,
            "Python should have reduced weight when not changed"
        );
        assert!(
            (weights.get(&Language::Rust).copied().unwrap_or(0.0) - 0.3).abs() < f64::EPSILON,
            "Rust should have reduced weight when not changed"
        );
    }

    #[test]
    fn test_weighted_scoring_empty_result_can_commit() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let changed_languages: HashSet<Language> = HashSet::new();
        let config = GateWeightConfig::default();
        let result = super::PolyglotGateResult::new();

        assert!(
            result.can_commit_weighted(&changed_languages, &config),
            "Empty result should allow commit with weighted scoring"
        );
    }

    #[test]
    fn test_weighted_scoring_warnings_in_changed_language_still_allow_commit() {
        use super::GateWeightConfig;
        use std::collections::HashSet;

        let mut changed_languages = HashSet::new();
        changed_languages.insert(Language::Python);

        let config = GateWeightConfig::default();
        let mut result = super::PolyglotGateResult::new();

        // Add warnings (non-blocking) in changed language
        let issues = vec![GateIssue::new(IssueSeverity::Warning, "missing docstring")];
        let mut gate_result = GateResult::pass("Ruff");
        gate_result.issues = issues;
        result.add_result(Language::Python, gate_result);

        // Warnings should not block commit
        assert!(
            result.can_commit_weighted(&changed_languages, &config),
            "Warnings should not block commit with weighted scoring"
        );
    }
}
