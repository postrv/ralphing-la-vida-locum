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
    ClippyConfig, ClippyGate, Gate, GateResult, NoAllowGate, NoTodoGate, QualityGate, SecurityGate,
    TestConfig, TestGate,
};
use anyhow::Result;
use futures::future::join_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Whether to check for `#[allow]` annotations.
    pub check_no_allow: bool,
    /// Patterns to allow in `#[allow]` checks.
    pub allowed_patterns: Vec<String>,
    /// Whether to run security scans.
    pub run_security: bool,
    /// Whether to check for TODO/FIXME comments.
    pub check_todos: bool,
    /// Stop on first failure (don't run remaining gates).
    pub fail_fast: bool,
    /// Run gates in parallel (default: true).
    ///
    /// When enabled, independent gates are executed concurrently using
    /// tokio's async runtime for faster feedback on quality issues.
    pub parallel_gates: bool,
    /// Timeout for individual gate execution in milliseconds (default: 60000).
    ///
    /// Gates that exceed this timeout will be cancelled and marked as failed
    /// with a timeout error. This prevents slow gates from blocking the entire
    /// quality check process.
    pub gate_timeout_ms: u64,
    /// Run gates incrementally based on changed files (default: true).
    ///
    /// When enabled, only gates for languages with changed files will run.
    /// This optimization speeds up feedback loops by skipping gates for
    /// unchanged languages. Manifest file changes (Cargo.toml, package.json,
    /// etc.) force a full gate run.
    pub incremental_gates: bool,
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
            parallel_gates: true,    // Parallel execution enabled by default
            gate_timeout_ms: 60_000, // 60 seconds per gate
            incremental_gates: true, // Incremental execution enabled by default
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

    /// Add allowed patterns for `#[allow]` checks.
    #[must_use]
    pub fn with_allowed_patterns(mut self, patterns: Vec<String>) -> Self {
        self.allowed_patterns = patterns;
        self
    }

    /// Enable/disable parallel gate execution.
    ///
    /// When enabled (default), independent gates run concurrently for faster
    /// feedback. Disable for deterministic sequential execution or debugging.
    #[must_use]
    pub fn with_parallel_gates(mut self, enabled: bool) -> Self {
        self.parallel_gates = enabled;
        self
    }

    /// Set the timeout for individual gate execution in milliseconds.
    ///
    /// Gates exceeding this timeout are cancelled and report a timeout error.
    /// Default is 60000ms (60 seconds).
    #[must_use]
    pub fn with_gate_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.gate_timeout_ms = timeout_ms;
        self
    }

    /// Enable/disable incremental gate execution.
    ///
    /// When enabled (default), only gates for languages with changed files will run.
    /// This speeds up feedback loops by skipping gates for unchanged languages.
    /// Disable for full gate runs regardless of which files changed.
    #[must_use]
    pub fn with_incremental_gates(mut self, enabled: bool) -> Self {
        self.incremental_gates = enabled;
        self
    }
}

// ============================================================================
// Incremental Gate Execution (Phase 15.2)
// ============================================================================

use crate::bootstrap::language::Language;
use std::collections::HashSet;
use std::process::Command;

/// All known manifest file names across supported languages.
///
/// Changes to these files should trigger a full gate run since they can
/// affect dependencies, build configuration, and overall project behavior.
const MANIFEST_FILES: &[&str] = &[
    // Rust
    "Cargo.toml",
    "Cargo.lock",
    // Python
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "Pipfile",
    "Pipfile.lock",
    "poetry.lock",
    // JavaScript/TypeScript
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "tsconfig.json",
    // Go
    "go.mod",
    "go.sum",
    // Java/Kotlin
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    // Ruby
    "Gemfile",
    "Gemfile.lock",
    // PHP
    "composer.json",
    "composer.lock",
    // .NET
    ".csproj",
    ".fsproj",
    "Directory.Build.props",
    "nuget.config",
];

/// Check if a filename is a manifest file.
///
/// Manifest files are build/dependency configuration files that can affect
/// the entire project. Changes to these files should trigger a full gate run.
///
/// # Arguments
///
/// * `filename` - The filename to check (just the file name, not the full path)
///
/// # Returns
///
/// `true` if the file is a known manifest file, `false` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::enforcer::is_manifest_file;
///
/// assert!(is_manifest_file("Cargo.toml"));
/// assert!(is_manifest_file("package.json"));
/// assert!(!is_manifest_file("main.rs"));
/// ```
#[must_use]
pub fn is_manifest_file(filename: &str) -> bool {
    // Check exact matches
    if MANIFEST_FILES.contains(&filename) {
        return true;
    }

    // Check suffix matches for patterns like *.csproj
    for pattern in MANIFEST_FILES {
        if pattern.starts_with('.') && filename.ends_with(pattern) {
            return true;
        }
    }

    false
}

/// Detect changed files in the git working tree.
///
/// Runs `git diff --name-only` and `git diff --cached --name-only` to find
/// all changed files (both staged and unstaged), then determines which
/// languages have been affected.
///
/// # Arguments
///
/// * `project_dir` - Path to the git repository root
///
/// # Returns
///
/// A tuple of (changed_files, changed_languages) where:
/// - `changed_files` is a vector of file paths that have changed
/// - `changed_languages` is a set of languages that have changed files
///
/// Returns empty collections if git is not available or the directory is
/// not a git repository.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::enforcer::detect_changed_files;
/// use std::path::Path;
///
/// let (files, languages) = detect_changed_files(Path::new("."));
/// for file in &files {
///     println!("Changed: {}", file);
/// }
/// for lang in &languages {
///     println!("Language affected: {}", lang);
/// }
/// ```
#[must_use]
pub fn detect_changed_files(project_dir: &Path) -> (Vec<String>, HashSet<Language>) {
    let mut changed_files = Vec::new();
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
                if !line.is_empty() {
                    changed_files.push(line.to_string());
                    if let Some(lang) = Language::from_path(Path::new(line)) {
                        changed_languages.insert(lang);
                    }
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
                if !line.is_empty() && !changed_files.contains(&line.to_string()) {
                    changed_files.push(line.to_string());
                    if let Some(lang) = Language::from_path(Path::new(line)) {
                        changed_languages.insert(lang);
                    }
                }
            }
        }
    }

    (changed_files, changed_languages)
}

/// Check if any of the changed files is a manifest file.
///
/// # Arguments
///
/// * `changed_files` - List of changed file paths
///
/// # Returns
///
/// `true` if any file is a manifest file, `false` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::enforcer::has_manifest_change;
///
/// let files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];
/// assert!(has_manifest_change(&files)); // Cargo.toml is a manifest
///
/// let src_only = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
/// assert!(!has_manifest_change(&src_only)); // No manifest files
/// ```
#[must_use]
pub fn has_manifest_change(changed_files: &[String]) -> bool {
    changed_files.iter().any(|f| {
        // Extract just the filename from the path
        let filename = Path::new(f)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(f);
        is_manifest_file(filename)
    })
}

/// Determine if a gate for a specific language should be skipped.
///
/// This function implements the incremental gate execution logic:
/// - If incremental mode is disabled, never skip
/// - If this is the first iteration, run all gates
/// - If the language has changed files, don't skip
/// - Otherwise, skip the gate
///
/// # Arguments
///
/// * `gate_language` - The language the gate is for
/// * `changed_languages` - Set of languages with changed files
/// * `is_first_iteration` - Whether this is the first iteration
/// * `config` - Enforcer configuration
///
/// # Returns
///
/// `true` if the gate should be skipped, `false` if it should run.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::enforcer::{should_skip_gate_for_language, EnforcerConfig};
/// use ralph::bootstrap::Language;
/// use std::collections::HashSet;
///
/// let changed: HashSet<Language> = [Language::Python].into_iter().collect();
/// let config = EnforcerConfig::new().with_incremental_gates(true);
///
/// // Python gate should run (Python changed)
/// assert!(!should_skip_gate_for_language(Language::Python, &changed, false, &config));
///
/// // Rust gate should be skipped (only Python changed)
/// assert!(should_skip_gate_for_language(Language::Rust, &changed, false, &config));
/// ```
#[must_use]
pub fn should_skip_gate_for_language(
    gate_language: Language,
    changed_languages: &HashSet<Language>,
    is_first_iteration: bool,
    config: &EnforcerConfig,
) -> bool {
    // If incremental mode is disabled, never skip
    if !config.incremental_gates {
        return false;
    }

    // On first iteration, run all gates to establish baseline
    if is_first_iteration {
        return false;
    }

    // If the language has changed files, don't skip
    if changed_languages.contains(&gate_language) {
        return false;
    }

    // Skip gates for unchanged languages
    true
}

// ============================================================================
// Parallel Gate Execution
// ============================================================================

/// Run quality gates, either in parallel or sequentially based on configuration.
///
/// This function executes a collection of quality gates on a project and returns
/// the results. When `config.parallel_gates` is true, gates run concurrently
/// using tokio's async runtime. When false, gates run sequentially.
///
/// # Arguments
///
/// * `gates` - A slice of Arc-wrapped quality gates to execute
/// * `project_dir` - Path to the project directory to check
/// * `config` - Enforcer configuration (controls parallelism and timeout)
///
/// # Returns
///
/// A vector of `Result<GateResult>` in the same order as the input gates.
/// Each result contains either the gate's result or an error if the gate
/// failed to execute (including timeout errors).
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::enforcer::{run_gates_parallel, EnforcerConfig};
/// use ralph::quality::gates::QualityGate;
/// use std::sync::Arc;
///
/// async fn run_checks() {
///     let gates: Vec<Arc<dyn QualityGate>> = vec![/* ... */];
///     let config = EnforcerConfig::new().with_parallel_gates(true);
///
///     let results = run_gates_parallel(&gates, Path::new("."), &config).await;
///
///     for result in results {
///         match result {
///             Ok(gate_result) => println!("{}", gate_result.summary()),
///             Err(e) => eprintln!("Gate execution error: {}", e),
///         }
///     }
/// }
/// ```
pub async fn run_gates_parallel(
    gates: &[Arc<dyn QualityGate>],
    project_dir: &Path,
    config: &EnforcerConfig,
) -> Vec<Result<GateResult>> {
    if gates.is_empty() {
        return Vec::new();
    }

    if config.parallel_gates {
        run_gates_concurrent(gates, project_dir, config).await
    } else {
        run_gates_sequential(gates, project_dir, config).await
    }
}

/// Execute gates concurrently using tokio::spawn and futures::join_all.
async fn run_gates_concurrent(
    gates: &[Arc<dyn QualityGate>],
    project_dir: &Path,
    config: &EnforcerConfig,
) -> Vec<Result<GateResult>> {
    let timeout = Duration::from_millis(config.gate_timeout_ms);
    let project_dir = project_dir.to_path_buf();

    // Spawn each gate as a concurrent task
    let handles: Vec<_> = gates
        .iter()
        .map(|gate| {
            let gate = Arc::clone(gate);
            let project_dir = project_dir.clone();

            tokio::spawn(
                async move { run_single_gate_with_timeout(gate, &project_dir, timeout).await },
            )
        })
        .collect();

    // Wait for all tasks to complete
    let join_results = join_all(handles).await;

    // Convert JoinHandle results to our Result type
    join_results
        .into_iter()
        .map(|join_result| {
            join_result.unwrap_or_else(|e| Err(anyhow::anyhow!("Gate task panicked: {}", e)))
        })
        .collect()
}

/// Execute gates sequentially, one at a time.
async fn run_gates_sequential(
    gates: &[Arc<dyn QualityGate>],
    project_dir: &Path,
    config: &EnforcerConfig,
) -> Vec<Result<GateResult>> {
    let timeout = Duration::from_millis(config.gate_timeout_ms);
    let mut results = Vec::with_capacity(gates.len());

    for gate in gates {
        let result = run_single_gate_with_timeout(Arc::clone(gate), project_dir, timeout).await;
        results.push(result);
    }

    results
}

/// Run a single gate with a timeout.
///
/// The gate execution happens in a blocking thread (via spawn_blocking) since
/// quality gates may perform I/O-heavy operations like running cargo commands.
async fn run_single_gate_with_timeout(
    gate: Arc<dyn QualityGate>,
    project_dir: &Path,
    timeout: Duration,
) -> Result<GateResult> {
    let project_dir = project_dir.to_path_buf();
    let gate_name = gate.name().to_string();
    let start = Instant::now();

    // Run the gate in a blocking thread since gates may do I/O
    let gate_future = tokio::task::spawn_blocking(move || gate.run(&project_dir));

    // Apply timeout
    let result = tokio::time::timeout(timeout, gate_future).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(issues_result)) => {
            // spawn_blocking completed successfully
            match issues_result {
                Ok(issues) => {
                    let passed = issues.iter().all(|i| !i.severity.is_blocking());
                    Ok(GateResult {
                        gate_name,
                        passed,
                        issues,
                        raw_output: String::new(),
                        duration_ms,
                    })
                }
                Err(e) => Err(e),
            }
        }
        Ok(Err(e)) => {
            // spawn_blocking panicked
            Err(anyhow::anyhow!("Gate '{}' panicked: {}", gate_name, e))
        }
        Err(_elapsed) => {
            // Timeout occurred - this is an execution failure
            Err(anyhow::anyhow!(
                "Gate '{}' timed out after {}ms",
                gate_name,
                timeout.as_millis()
            ))
        }
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

        output.push_str(&format!("\n**Total time**: {}ms\n", self.total_duration_ms));

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

    #[test]
    fn test_can_commit_passes_with_clean_code() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create a minimal clean Rust project
        let src_dir = project_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            r#"
/// Clean function that passes all gates.
pub fn clean_function() -> i32 {
    42
}
"#,
        )
        .unwrap();

        // Create Cargo.toml
        std::fs::write(
            project_dir.join("Cargo.toml"),
            r#"
[package]
name = "test_project"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // Use minimal config (only clippy, no tests/security)
        let config = EnforcerConfig::new()
            .with_clippy(false) // Skip clippy for speed
            .with_tests(false)
            .with_security(false)
            .with_no_allow(true);

        let enforcer = QualityGateEnforcer::with_config(project_dir, config);
        let result = enforcer.can_commit();

        assert!(result.is_ok(), "can_commit should pass with clean code");
    }

    #[test]
    fn test_can_commit_fails_with_allow_annotation() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create code with forbidden #[allow] annotation
        let src_dir = project_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            r#"
#[allow(dead_code)]
fn unused_function() {}
"#,
        )
        .unwrap();

        // Use no_allow check only
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(true);

        let enforcer = QualityGateEnforcer::with_config(project_dir, config);
        let result = enforcer.can_commit();

        assert!(
            result.is_err(),
            "can_commit should fail with #[allow] annotation"
        );
        let failures = result.unwrap_err();
        assert!(!failures.is_empty(), "Should have at least one failure");
        assert!(
            failures.iter().any(|f| f.gate_name == "NoAllow"),
            "Failure should be from NoAllow gate"
        );
    }

    // ========================================================================
    // Phase 15.1: Parallel Gate Execution Tests
    // ========================================================================

    #[test]
    fn test_enforcer_config_parallel_gates_default() {
        // Parallel gate execution should be enabled by default
        let config = EnforcerConfig::default();
        assert!(
            config.parallel_gates,
            "parallel_gates should be enabled by default"
        );
    }

    #[test]
    fn test_enforcer_config_parallel_gates_builder() {
        // Should be able to disable parallel gates via builder
        let config = EnforcerConfig::new().with_parallel_gates(false);
        assert!(
            !config.parallel_gates,
            "should be able to disable parallel_gates"
        );
    }

    #[test]
    fn test_enforcer_config_gate_timeout() {
        // Should have a configurable per-gate timeout
        let config = EnforcerConfig::new().with_gate_timeout_ms(5000);
        assert_eq!(config.gate_timeout_ms, 5000);
    }

    #[test]
    fn test_enforcer_config_gate_timeout_default() {
        // Default gate timeout should be 60 seconds
        let config = EnforcerConfig::default();
        assert_eq!(
            config.gate_timeout_ms, 60_000,
            "default gate timeout should be 60 seconds"
        );
    }

    #[tokio::test]
    async fn test_parallel_gates_run_concurrently() {
        // Gates should run in parallel when parallel_gates is enabled.
        // This test uses mock gates that sleep to verify concurrency.
        use super::super::gates::{GateIssue, QualityGate};
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        // Track how many gates are running concurrently
        static CONCURRENT_COUNT: AtomicU32 = AtomicU32::new(0);
        static MAX_CONCURRENT: AtomicU32 = AtomicU32::new(0);

        struct SlowGate {
            name: String,
            sleep_ms: u64,
        }

        impl QualityGate for SlowGate {
            fn name(&self) -> &str {
                &self.name
            }

            fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
                let current = CONCURRENT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                MAX_CONCURRENT.fetch_max(current, Ordering::SeqCst);

                std::thread::sleep(Duration::from_millis(self.sleep_ms));

                CONCURRENT_COUNT.fetch_sub(1, Ordering::SeqCst);
                Ok(vec![])
            }

            fn remediation(&self, _issues: &[GateIssue]) -> String {
                String::new()
            }
        }

        // Reset atomics
        CONCURRENT_COUNT.store(0, Ordering::SeqCst);
        MAX_CONCURRENT.store(0, Ordering::SeqCst);

        let gates: Vec<Arc<dyn QualityGate>> = vec![
            Arc::new(SlowGate {
                name: "Gate1".to_string(),
                sleep_ms: 100,
            }),
            Arc::new(SlowGate {
                name: "Gate2".to_string(),
                sleep_ms: 100,
            }),
            Arc::new(SlowGate {
                name: "Gate3".to_string(),
                sleep_ms: 100,
            }),
        ];

        let temp_dir = TempDir::new().unwrap();
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(true);

        let start = Instant::now();
        let results = run_gates_parallel(&gates, temp_dir.path(), &config).await;
        let elapsed = start.elapsed();

        // All gates should have passed
        assert!(results.iter().all(|r| r.is_ok()), "All gates should pass");
        assert_eq!(results.len(), 3, "Should have 3 results");

        // If running in parallel, max concurrent should be > 1
        let max_conc = MAX_CONCURRENT.load(Ordering::SeqCst);
        assert!(
            max_conc > 1,
            "Gates should run concurrently (max concurrent was {})",
            max_conc
        );

        // Total time should be ~100ms (parallel) not ~300ms (sequential)
        assert!(
            elapsed.as_millis() < 250,
            "Parallel execution should be faster than sequential (took {}ms)",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_parallel_gates_results_collected_correctly() {
        // Results from all gates should be collected even when running in parallel
        use super::super::gates::{GateIssue, IssueSeverity, QualityGate};
        use std::sync::Arc;

        struct NamedGate {
            name: String,
            should_fail: bool,
        }

        impl QualityGate for NamedGate {
            fn name(&self) -> &str {
                &self.name
            }

            fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
                if self.should_fail {
                    Ok(vec![GateIssue::new(
                        IssueSeverity::Error,
                        format!("{} failed", self.name),
                    )])
                } else {
                    Ok(vec![])
                }
            }

            fn remediation(&self, _issues: &[GateIssue]) -> String {
                String::new()
            }
        }

        let gates: Vec<Arc<dyn QualityGate>> = vec![
            Arc::new(NamedGate {
                name: "PassGate".to_string(),
                should_fail: false,
            }),
            Arc::new(NamedGate {
                name: "FailGate".to_string(),
                should_fail: true,
            }),
            Arc::new(NamedGate {
                name: "AnotherPassGate".to_string(),
                should_fail: false,
            }),
        ];

        let temp_dir = TempDir::new().unwrap();
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(true);

        let results = run_gates_parallel(&gates, temp_dir.path(), &config).await;

        // Should have all 3 results
        assert_eq!(results.len(), 3, "Should collect all gate results");

        // Convert to GateResults and verify
        let gate_results: Vec<GateResult> = results.into_iter().filter_map(|r| r.ok()).collect();
        assert_eq!(gate_results.len(), 3, "All gates should complete");

        // Verify we have the right mix of pass/fail
        let passed_count = gate_results.iter().filter(|r| r.passed).count();
        let failed_count = gate_results.iter().filter(|r| !r.passed).count();
        assert_eq!(passed_count, 2, "Should have 2 passing gates");
        assert_eq!(failed_count, 1, "Should have 1 failing gate");
    }

    #[tokio::test]
    async fn test_parallel_gates_respects_timeout() {
        // Gates should be cancelled if they exceed the timeout
        use super::super::gates::{GateIssue, QualityGate};
        use std::sync::Arc;
        use std::time::Duration;

        struct SlowGate {
            name: String,
            sleep_ms: u64,
        }

        impl QualityGate for SlowGate {
            fn name(&self) -> &str {
                &self.name
            }

            fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
                std::thread::sleep(Duration::from_millis(self.sleep_ms));
                Ok(vec![])
            }

            fn remediation(&self, _issues: &[GateIssue]) -> String {
                String::new()
            }
        }

        let gates: Vec<Arc<dyn QualityGate>> = vec![
            Arc::new(SlowGate {
                name: "FastGate".to_string(),
                sleep_ms: 10,
            }),
            Arc::new(SlowGate {
                name: "SlowGate".to_string(),
                sleep_ms: 5000, // This should timeout
            }),
        ];

        let temp_dir = TempDir::new().unwrap();
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(true)
            .with_gate_timeout_ms(100); // 100ms timeout

        let start = std::time::Instant::now();
        let results = run_gates_parallel(&gates, temp_dir.path(), &config).await;
        let elapsed = start.elapsed();

        // Should complete in reasonable time (not 5 seconds)
        assert!(
            elapsed.as_millis() < 1000,
            "Should timeout slow gates (took {}ms)",
            elapsed.as_millis()
        );

        // Should have 2 results (one pass, one timeout error)
        assert_eq!(results.len(), 2, "Should have results for both gates");

        // One should be error (timeout)
        let error_count = results.iter().filter(|r| r.is_err()).count();
        assert!(error_count >= 1, "Slow gate should timeout");
    }

    #[tokio::test]
    async fn test_parallel_gates_failure_doesnt_cancel_others() {
        // A failing gate should not prevent other gates from completing
        use super::super::gates::{GateIssue, QualityGate};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        static SLOW_GATE_COMPLETED: AtomicBool = AtomicBool::new(false);

        struct FailingGate;
        struct SlowGate;

        impl QualityGate for FailingGate {
            fn name(&self) -> &str {
                "FailingGate"
            }

            fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
                // Return an error (gate fails to execute, not just finding issues)
                anyhow::bail!("Gate execution failed");
            }

            fn remediation(&self, _issues: &[GateIssue]) -> String {
                String::new()
            }
        }

        impl QualityGate for SlowGate {
            fn name(&self) -> &str {
                "SlowGate"
            }

            fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
                std::thread::sleep(Duration::from_millis(50));
                SLOW_GATE_COMPLETED.store(true, Ordering::SeqCst);
                Ok(vec![])
            }

            fn remediation(&self, _issues: &[GateIssue]) -> String {
                String::new()
            }
        }

        SLOW_GATE_COMPLETED.store(false, Ordering::SeqCst);

        let gates: Vec<Arc<dyn QualityGate>> = vec![Arc::new(FailingGate), Arc::new(SlowGate)];

        let temp_dir = TempDir::new().unwrap();
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(true);

        let results = run_gates_parallel(&gates, temp_dir.path(), &config).await;

        // Both gates should have results
        assert_eq!(results.len(), 2, "Should have results for both gates");

        // The slow gate should have completed despite the other gate failing
        assert!(
            SLOW_GATE_COMPLETED.load(Ordering::SeqCst),
            "Slow gate should complete even when another gate fails"
        );
    }

    #[tokio::test]
    async fn test_sequential_execution_when_parallel_disabled() {
        // When parallel_gates is disabled, gates should run sequentially
        use super::super::gates::{GateIssue, QualityGate};
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        static CONCURRENT_COUNT: AtomicU32 = AtomicU32::new(0);
        static MAX_CONCURRENT: AtomicU32 = AtomicU32::new(0);

        struct SlowGate {
            name: String,
            sleep_ms: u64,
        }

        impl QualityGate for SlowGate {
            fn name(&self) -> &str {
                &self.name
            }

            fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
                let current = CONCURRENT_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                MAX_CONCURRENT.fetch_max(current, Ordering::SeqCst);

                std::thread::sleep(Duration::from_millis(self.sleep_ms));

                CONCURRENT_COUNT.fetch_sub(1, Ordering::SeqCst);
                Ok(vec![])
            }

            fn remediation(&self, _issues: &[GateIssue]) -> String {
                String::new()
            }
        }

        // Reset atomics
        CONCURRENT_COUNT.store(0, Ordering::SeqCst);
        MAX_CONCURRENT.store(0, Ordering::SeqCst);

        let gates: Vec<Arc<dyn QualityGate>> = vec![
            Arc::new(SlowGate {
                name: "Gate1".to_string(),
                sleep_ms: 50,
            }),
            Arc::new(SlowGate {
                name: "Gate2".to_string(),
                sleep_ms: 50,
            }),
        ];

        let temp_dir = TempDir::new().unwrap();
        let config = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(false); // Disabled!

        let start = std::time::Instant::now();
        let results = run_gates_parallel(&gates, temp_dir.path(), &config).await;
        let elapsed = start.elapsed();

        // All gates should have passed
        assert!(results.iter().all(|r| r.is_ok()), "All gates should pass");
        assert_eq!(results.len(), 2, "Should have 2 results");

        // When running sequentially, max concurrent should be 1
        let max_conc = MAX_CONCURRENT.load(Ordering::SeqCst);
        assert_eq!(
            max_conc, 1,
            "Gates should run sequentially (max concurrent was {})",
            max_conc
        );

        // Total time should be ~100ms (sequential)
        assert!(
            elapsed.as_millis() >= 90,
            "Sequential execution should take at least 100ms (took {}ms)",
            elapsed.as_millis()
        );
    }

    // ========================================================================
    // Phase 15.2: Incremental Gate Execution Tests
    // ========================================================================

    #[test]
    fn test_enforcer_config_incremental_gates_default() {
        // Incremental gate execution should be enabled by default
        let config = EnforcerConfig::default();
        assert!(
            config.incremental_gates,
            "incremental_gates should be enabled by default"
        );
    }

    #[test]
    fn test_enforcer_config_incremental_gates_builder() {
        // Should be able to disable incremental gates via builder
        let config = EnforcerConfig::new().with_incremental_gates(false);
        assert!(
            !config.incremental_gates,
            "should be able to disable incremental_gates"
        );
    }

    #[test]
    fn test_is_manifest_file_rust() {
        use super::is_manifest_file;
        assert!(is_manifest_file("Cargo.toml"), "Cargo.toml is a manifest");
        assert!(is_manifest_file("Cargo.lock"), "Cargo.lock is a manifest");
    }

    #[test]
    fn test_is_manifest_file_python() {
        use super::is_manifest_file;
        assert!(
            is_manifest_file("pyproject.toml"),
            "pyproject.toml is a manifest"
        );
        assert!(
            is_manifest_file("requirements.txt"),
            "requirements.txt is a manifest"
        );
        assert!(is_manifest_file("setup.py"), "setup.py is a manifest");
    }

    #[test]
    fn test_is_manifest_file_typescript() {
        use super::is_manifest_file;
        assert!(
            is_manifest_file("package.json"),
            "package.json is a manifest"
        );
        assert!(
            is_manifest_file("tsconfig.json"),
            "tsconfig.json is a manifest"
        );
    }

    #[test]
    fn test_is_manifest_file_go() {
        use super::is_manifest_file;
        assert!(is_manifest_file("go.mod"), "go.mod is a manifest");
        assert!(is_manifest_file("go.sum"), "go.sum is a manifest");
    }

    #[test]
    fn test_is_manifest_file_regular_file() {
        use super::is_manifest_file;
        assert!(!is_manifest_file("main.rs"), "main.rs is not a manifest");
        assert!(!is_manifest_file("app.py"), "app.py is not a manifest");
        assert!(!is_manifest_file("index.ts"), "index.ts is not a manifest");
        assert!(!is_manifest_file("main.go"), "main.go is not a manifest");
        assert!(
            !is_manifest_file("README.md"),
            "README.md is not a manifest"
        );
    }

    #[test]
    fn test_detect_changed_files_and_languages() {
        // This tests the git diff detection function
        // In a real git repo with changes, this should detect the changed files
        use super::detect_changed_files;

        let temp_dir = TempDir::new().unwrap();
        // In a non-git directory, should return empty
        let (files, _) = detect_changed_files(temp_dir.path());
        assert!(
            files.is_empty(),
            "Non-git directory should have no changed files"
        );
    }

    #[test]
    fn test_has_manifest_change() {
        use super::has_manifest_change;

        let with_manifest = vec![
            "src/main.rs".to_string(),
            "Cargo.toml".to_string(),
            "src/lib.rs".to_string(),
        ];
        assert!(
            has_manifest_change(&with_manifest),
            "Should detect Cargo.toml as manifest"
        );

        let without_manifest = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/test.rs".to_string(),
        ];
        assert!(
            !has_manifest_change(&without_manifest),
            "Should not detect manifest in source files"
        );
    }

    #[test]
    fn test_should_skip_gate_for_language_incremental_disabled() {
        use super::should_skip_gate_for_language;
        use crate::bootstrap::language::Language;
        use std::collections::HashSet;

        // When incremental is disabled, never skip
        let changed: HashSet<Language> = [Language::Python].into_iter().collect();
        let config = EnforcerConfig::new().with_incremental_gates(false);

        assert!(
            !should_skip_gate_for_language(Language::Rust, &changed, false, &config),
            "Should not skip Rust gate when incremental is disabled"
        );
    }

    #[test]
    fn test_should_skip_gate_for_language_first_iteration() {
        use super::should_skip_gate_for_language;
        use crate::bootstrap::language::Language;
        use std::collections::HashSet;

        // On first iteration, never skip (run all gates)
        let changed: HashSet<Language> = [Language::Python].into_iter().collect();
        let config = EnforcerConfig::new().with_incremental_gates(true);

        assert!(
            !should_skip_gate_for_language(Language::Rust, &changed, true, &config),
            "Should not skip any gate on first iteration"
        );
    }

    #[test]
    fn test_should_skip_gate_for_language_not_changed() {
        use super::should_skip_gate_for_language;
        use crate::bootstrap::language::Language;
        use std::collections::HashSet;

        // When only Python files changed, skip Rust gates
        let changed: HashSet<Language> = [Language::Python].into_iter().collect();
        let config = EnforcerConfig::new().with_incremental_gates(true);

        assert!(
            should_skip_gate_for_language(Language::Rust, &changed, false, &config),
            "Should skip Rust gate when only Python changed"
        );
    }

    #[test]
    fn test_should_skip_gate_for_language_changed() {
        use super::should_skip_gate_for_language;
        use crate::bootstrap::language::Language;
        use std::collections::HashSet;

        // When Python files changed, don't skip Python gates
        let changed: HashSet<Language> = [Language::Python].into_iter().collect();
        let config = EnforcerConfig::new().with_incremental_gates(true);

        assert!(
            !should_skip_gate_for_language(Language::Python, &changed, false, &config),
            "Should not skip Python gate when Python changed"
        );
    }

    #[test]
    fn test_only_python_gates_run_when_only_py_changed() {
        use super::should_skip_gate_for_language;
        use crate::bootstrap::language::Language;
        use std::collections::HashSet;

        let changed: HashSet<Language> = [Language::Python].into_iter().collect();
        let config = EnforcerConfig::new().with_incremental_gates(true);

        // Python gate should run
        assert!(
            !should_skip_gate_for_language(Language::Python, &changed, false, &config),
            "Python gate should run when .py changed"
        );

        // Other gates should be skipped
        assert!(
            should_skip_gate_for_language(Language::Rust, &changed, false, &config),
            "Rust gate should skip when only .py changed"
        );
        assert!(
            should_skip_gate_for_language(Language::TypeScript, &changed, false, &config),
            "TypeScript gate should skip when only .py changed"
        );
        assert!(
            should_skip_gate_for_language(Language::Go, &changed, false, &config),
            "Go gate should skip when only .py changed"
        );
    }

    #[test]
    fn test_only_typescript_gates_run_when_only_ts_changed() {
        use super::should_skip_gate_for_language;
        use crate::bootstrap::language::Language;
        use std::collections::HashSet;

        let changed: HashSet<Language> = [Language::TypeScript].into_iter().collect();
        let config = EnforcerConfig::new().with_incremental_gates(true);

        // TypeScript gate should run
        assert!(
            !should_skip_gate_for_language(Language::TypeScript, &changed, false, &config),
            "TypeScript gate should run when .ts changed"
        );

        // Other gates should be skipped
        assert!(
            should_skip_gate_for_language(Language::Rust, &changed, false, &config),
            "Rust gate should skip when only .ts changed"
        );
        assert!(
            should_skip_gate_for_language(Language::Python, &changed, false, &config),
            "Python gate should skip when only .ts changed"
        );
        assert!(
            should_skip_gate_for_language(Language::Go, &changed, false, &config),
            "Go gate should skip when only .ts changed"
        );
    }
}
