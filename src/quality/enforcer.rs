//! Quality gate enforcement orchestration.
//!
//! The [`QualityGateEnforcer`] runs multiple quality gates and determines
//! whether code can be committed based on gate results.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::quality::enforcer::QualityGateEnforcer;
//!
//! let enforcer = QualityGateEnforcer::standard("/path/to/project");
//! match enforcer.can_commit() {
//!     Ok(()) => println!("All gates passed, safe to commit"),
//!     Err(failures) => {
//!         for failure in &failures {
//!             println!("{}", failure.summary());
//!         }
//!     }
//! }
//! ```

use super::gates::{
    ClippyConfig, ClippyGate, Gate, GateResult, NoAllowGate, NoTodoGate, SecurityGate, TestConfig,
    TestGate,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

// ============================================================================
// Enforcer Configuration
// ============================================================================

/// Configuration for the quality gate enforcer.
#[derive(Debug, Clone)]
pub struct EnforcerConfig {
    /// Whether to run clippy gate.
    pub run_clippy: bool,
    /// Clippy configuration.
    pub clippy_config: ClippyConfig,
    /// Whether to run test gate.
    pub run_tests: bool,
    /// Test configuration.
    pub test_config: TestConfig,
    /// Whether to check for #[allow] annotations.
    pub check_no_allow: bool,
    /// Patterns to allow in #[allow] checks.
    pub allowed_patterns: Vec<String>,
    /// Whether to run security scans.
    pub run_security: bool,
    /// Whether to check for TODO/FIXME comments.
    pub check_todos: bool,
    /// Stop on first failure (don't run remaining gates).
    pub fail_fast: bool,
}

impl Default for EnforcerConfig {
    fn default() -> Self {
        Self {
            run_clippy: true,
            clippy_config: ClippyConfig::default(),
            run_tests: true,
            test_config: TestConfig::default(),
            check_no_allow: true,
            allowed_patterns: Vec::new(),
            run_security: true,
            check_todos: false, // Disabled by default (non-blocking)
            fail_fast: false,
        }
    }
}

impl EnforcerConfig {
    /// Create a new configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable/disable clippy gate.
    #[must_use]
    pub fn with_clippy(mut self, enabled: bool) -> Self {
        self.run_clippy = enabled;
        self
    }

    /// Enable/disable test gate.
    #[must_use]
    pub fn with_tests(mut self, enabled: bool) -> Self {
        self.run_tests = enabled;
        self
    }

    /// Enable/disable no-allow gate.
    #[must_use]
    pub fn with_no_allow(mut self, enabled: bool) -> Self {
        self.check_no_allow = enabled;
        self
    }

    /// Enable/disable security gate.
    #[must_use]
    pub fn with_security(mut self, enabled: bool) -> Self {
        self.run_security = enabled;
        self
    }

    /// Enable/disable todo checking.
    #[must_use]
    pub fn with_todos(mut self, enabled: bool) -> Self {
        self.check_todos = enabled;
        self
    }

    /// Enable/disable fail-fast mode.
    #[must_use]
    pub fn with_fail_fast(mut self, enabled: bool) -> Self {
        self.fail_fast = enabled;
        self
    }

    /// Add allowed patterns for #[allow] checks.
    #[must_use]
    pub fn with_allowed_patterns(mut self, patterns: Vec<String>) -> Self {
        self.allowed_patterns = patterns;
        self
    }
}

// ============================================================================
// Enforcer Result
// ============================================================================

/// Summary of all gate results.
#[derive(Debug, Clone)]
pub struct EnforcerSummary {
    /// All gate results (passed and failed).
    pub results: Vec<GateResult>,
    /// Overall pass/fail status.
    pub all_passed: bool,
    /// Total duration of all checks in milliseconds.
    pub total_duration_ms: u64,
}

impl EnforcerSummary {
    /// Get only the failing results.
    #[must_use]
    pub fn failures(&self) -> Vec<&GateResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }

    /// Get only the passing results.
    #[must_use]
    pub fn passes(&self) -> Vec<&GateResult> {
        self.results.iter().filter(|r| r.passed).collect()
    }

    /// Format a summary for display.
    #[must_use]
    pub fn format(&self) -> String {
        let mut output = String::new();

        output.push_str("## Quality Gate Summary\n\n");

        for result in &self.results {
            output.push_str(&format!("{}\n", result.summary()));
        }

        output.push_str(&format!(
            "\n**Total time**: {}ms\n",
            self.total_duration_ms
        ));

        if self.all_passed {
            output.push_str("\n✅ **All gates passed** - safe to commit\n");
        } else {
            let failure_count = self.failures().len();
            output.push_str(&format!(
                "\n❌ **{} gate(s) failed** - fix issues before committing\n",
                failure_count
            ));
        }

        output
    }
}

// ============================================================================
// Quality Gate Enforcer
// ============================================================================

/// Orchestrates running multiple quality gates.
pub struct QualityGateEnforcer {
    project_dir: PathBuf,
    config: EnforcerConfig,
}

impl QualityGateEnforcer {
    /// Create a new enforcer with default configuration.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            config: EnforcerConfig::default(),
        }
    }

    /// Create a new enforcer with custom configuration.
    pub fn with_config(project_dir: impl AsRef<Path>, config: EnforcerConfig) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            config,
        }
    }

    /// Create an enforcer with standard gates for Ralph.
    ///
    /// This is the recommended configuration for Ralph's quality enforcement:
    /// - Clippy with warnings as errors
    /// - All tests must pass
    /// - No #[allow(...)] annotations
    /// - Security scan if cargo-audit is available
    pub fn standard(project_dir: impl AsRef<Path>) -> Self {
        Self::new(project_dir)
    }

    /// Create an enforcer with minimal gates (fast checks only).
    ///
    /// Useful for quick feedback during development:
    /// - Clippy only
    /// - No tests (too slow for feedback loop)
    pub fn minimal(project_dir: impl AsRef<Path>) -> Self {
        let config = EnforcerConfig::new()
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false);

        Self::with_config(project_dir, config)
    }

    /// Get the gates to run based on configuration.
    fn get_gates(&self) -> Vec<Box<dyn Gate>> {
        let mut gates: Vec<Box<dyn Gate>> = Vec::new();

        if self.config.run_clippy {
            gates.push(Box::new(ClippyGate::with_config(
                &self.project_dir,
                self.config.clippy_config.clone(),
            )));
        }

        if self.config.run_tests {
            gates.push(Box::new(TestGate::with_config(
                &self.project_dir,
                self.config.test_config.clone(),
            )));
        }

        if self.config.check_no_allow {
            let gate = NoAllowGate::new(&self.project_dir)
                .with_allowed(self.config.allowed_patterns.clone());
            gates.push(Box::new(gate));
        }

        if self.config.run_security {
            gates.push(Box::new(SecurityGate::new(&self.project_dir)));
        }

        if self.config.check_todos {
            gates.push(Box::new(NoTodoGate::new(&self.project_dir)));
        }

        gates
    }

    /// Run all configured quality gates.
    ///
    /// # Errors
    ///
    /// Returns an error if a gate fails to execute (not if checks fail).
    pub fn run_all(&self) -> Result<EnforcerSummary> {
        let gates = self.get_gates();
        let mut results = Vec::new();
        let mut total_duration_ms = 0u64;
        let mut all_passed = true;

        for gate in gates {
            let result = gate.check()?;
            total_duration_ms += result.duration_ms;

            if !result.passed && gate.is_blocking() {
                all_passed = false;

                if self.config.fail_fast {
                    results.push(result);
                    break;
                }
            }

            results.push(result);
        }

        Ok(EnforcerSummary {
            results,
            all_passed,
            total_duration_ms,
        })
    }

    /// Check if code can be committed (all blocking gates pass).
    ///
    /// # Returns
    ///
    /// - `Ok(())` if all blocking gates pass
    /// - `Err(failures)` with the list of failing gate results
    pub fn can_commit(&self) -> Result<(), Vec<GateResult>> {
        let summary = self.run_all().map_err(|e| {
            vec![GateResult::fail(
                "Enforcer",
                vec![super::gates::GateIssue::new(
                    super::gates::IssueSeverity::Error,
                    format!("Failed to run gates: {}", e),
                )],
            )]
        })?;

        if summary.all_passed {
            Ok(())
        } else {
            Err(summary.failures().into_iter().cloned().collect())
        }
    }

    /// Run only clippy gate.
    ///
    /// # Errors
    ///
    /// Returns an error if clippy fails to execute.
    pub fn run_clippy(&self) -> Result<GateResult> {
        let gate = ClippyGate::with_config(&self.project_dir, self.config.clippy_config.clone());
        gate.check()
    }

    /// Run only test gate.
    ///
    /// # Errors
    ///
    /// Returns an error if tests fail to execute.
    pub fn run_tests(&self) -> Result<GateResult> {
        let gate = TestGate::with_config(&self.project_dir, self.config.test_config.clone());
        gate.check()
    }

    /// Run only no-allow gate.
    ///
    /// # Errors
    ///
    /// Returns an error if scanning fails.
    pub fn run_no_allow(&self) -> Result<GateResult> {
        let gate =
            NoAllowGate::new(&self.project_dir).with_allowed(self.config.allowed_patterns.clone());
        gate.check()
    }

    /// Run only security gate.
    ///
    /// # Errors
    ///
    /// Returns an error if security scan fails.
    pub fn run_security(&self) -> Result<GateResult> {
        let gate = SecurityGate::new(&self.project_dir);
        gate.check()
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
    fn test_enforcer_config_builder() {
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_no_allow(true)
            .with_security(false)
            .with_fail_fast(true);

        assert!(!config.run_clippy);
        assert!(!config.run_tests);
        assert!(config.check_no_allow);
        assert!(!config.run_security);
        assert!(config.fail_fast);
    }

    #[test]
    fn test_enforcer_summary_format() {
        let results = vec![
            GateResult::pass("Clippy").with_duration(100),
            GateResult::fail(
                "Tests",
                vec![super::super::gates::GateIssue::new(
                    super::super::gates::IssueSeverity::Error,
                    "test failed",
                )],
            )
            .with_duration(200),
        ];

        let summary = EnforcerSummary {
            results,
            all_passed: false,
            total_duration_ms: 300,
        };

        let formatted = summary.format();
        assert!(formatted.contains("Quality Gate Summary"));
        assert!(formatted.contains("Clippy"));
        assert!(formatted.contains("Tests"));
        assert!(formatted.contains("300ms"));
        assert!(formatted.contains("1 gate(s) failed"));
    }

    #[test]
    fn test_enforcer_no_allow_only() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create src directory with clean code
        let src_dir = project_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            r#"
pub fn clean_function() -> i32 {
    42
}
"#,
        )
        .unwrap();

        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(true);

        let enforcer = QualityGateEnforcer::with_config(project_dir, config);
        let result = enforcer.run_no_allow().unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_enforcer_no_allow_detects_violations() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create src directory with #[allow] annotation
        let src_dir = project_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            r#"
#[allow(dead_code)]
fn unused() {}
"#,
        )
        .unwrap();

        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(true);

        let enforcer = QualityGateEnforcer::with_config(project_dir, config);
        let result = enforcer.run_no_allow().unwrap();

        assert!(!result.passed);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_enforcer_minimal_creates_correct_gates() {
        let temp_dir = TempDir::new().unwrap();
        let enforcer = QualityGateEnforcer::minimal(temp_dir.path());

        // Minimal should only have clippy enabled
        assert!(enforcer.config.run_clippy);
        assert!(!enforcer.config.run_tests);
        assert!(!enforcer.config.run_security);
        assert!(!enforcer.config.check_no_allow);
    }

    #[test]
    fn test_enforcer_standard_creates_all_gates() {
        let temp_dir = TempDir::new().unwrap();
        let enforcer = QualityGateEnforcer::standard(temp_dir.path());

        // Standard should have all main gates enabled
        assert!(enforcer.config.run_clippy);
        assert!(enforcer.config.run_tests);
        assert!(enforcer.config.run_security);
        assert!(enforcer.config.check_no_allow);
    }
}
