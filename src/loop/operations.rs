//! Real implementations of testable traits.
//!
//! These implementations use actual system calls for git, Claude, and file operations.
//! They implement the same traits as the mocks, enabling dependency injection.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use ralph::quality::gates::QualityGate;
use ralph::quality::{EnforcerConfig, QualityGateEnforcer};
use ralph::testing::{ClaudeProcess, FileSystem, GitOperations, QualityChecker, QualityGateResult};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::process::Command as AsyncCommand;
use tracing::debug;

/// Real git operations implementation.
///
/// Executes actual git commands against the file system.
#[derive(Debug, Clone)]
pub struct RealGitOperations {
    project_dir: PathBuf,
}

impl RealGitOperations {
    /// Create a new git operations instance for the given directory.
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }
}

impl GitOperations for RealGitOperations {
    fn get_commit_hash(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to run git rev-parse")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Ok(String::new())
        }
    }

    fn count_commits_since(&self, old_hash: &str) -> u32 {
        if old_hash.is_empty() {
            return 0;
        }

        let output = Command::new("git")
            .args(["rev-list", "--count", &format!("{old_hash}..HEAD")])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse()
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn get_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to get current branch")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            bail!("Not in a git repository")
        }
    }

    fn get_modified_files(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to get modified files")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let files: Vec<String> = stdout
                .lines()
                .filter_map(|line| {
                    // Format: "XY filename" where XY is status
                    if line.len() > 3 {
                        Some(line[3..].to_string())
                    } else {
                        None
                    }
                })
                .collect();
            Ok(files)
        } else {
            Ok(Vec::new())
        }
    }

    fn push(&self, remote: &str, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["push", remote, branch])
            .env(
                "GIT_SSH_COMMAND",
                "ssh -o BatchMode=yes -o ConnectTimeout=10",
            )
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to push to remote")?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Push failed: {}", stderr)
        }
    }
}

/// Real Claude Code process implementation.
///
/// Spawns actual Claude Code subprocesses.
#[derive(Debug, Clone)]
pub struct RealClaudeProcess {
    project_dir: PathBuf,
    /// The model to use for Claude Code iterations.
    model: String,
}

impl RealClaudeProcess {
    /// Default model used when not explicitly specified.
    pub const DEFAULT_MODEL: &'static str = "opus";

    /// Create a new Claude process instance for the given directory.
    ///
    /// Uses the default model ("opus").
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            project_dir,
            model: Self::DEFAULT_MODEL.to_string(),
        }
    }

    /// Create a new Claude process instance with a specific model.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The directory to run Claude Code in.
    /// * `model` - The model name (e.g., "opus", "sonnet", "haiku").
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::r#loop::operations::RealClaudeProcess;
    ///
    /// let claude = RealClaudeProcess::with_model(
    ///     PathBuf::from("."),
    ///     "sonnet".to_string()
    /// );
    /// ```
    #[must_use]
    pub fn with_model(project_dir: PathBuf, model: String) -> Self {
        Self { project_dir, model }
    }

    /// Get the model being used for iterations.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl ClaudeProcess for RealClaudeProcess {
    async fn run_iteration(&self, prompt: &str) -> Result<i32> {
        let args = vec![
            "-p",
            "--dangerously-skip-permissions",
            "--model",
            &self.model,
        ];

        debug!(model = %self.model(), "Running Claude Code iteration");

        let mut child = AsyncCommand::new("claude")
            .args(&args)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.flush().await?;
            drop(stdin);
        }

        let status = child.wait().await?;
        Ok(status.code().unwrap_or(1))
    }

    async fn run_agent(&self, agent: &str, prompt: &str) -> Result<String> {
        let output = AsyncCommand::new("claude")
            .args(["--dangerously-skip-permissions", "--agent", agent, prompt])
            .current_dir(&self.project_dir)
            .output()
            .await
            .context("Failed to run Claude agent")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Agent failed: {}", stderr)
        }
    }
}

/// Real file system implementation.
///
/// Performs actual file system operations.
#[derive(Debug, Clone)]
pub struct RealFileSystem {
    base_path: PathBuf,
}

impl RealFileSystem {
    /// Create a new file system instance with the given base path.
    #[must_use]
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Resolve a path relative to the base path.
    fn resolve(&self, path: &str) -> PathBuf {
        if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            self.base_path.join(path)
        }
    }
}

impl FileSystem for RealFileSystem {
    fn read_file(&self, path: &str) -> Result<String> {
        let full_path = self.resolve(path);
        std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {}", full_path.display()))
    }

    fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        let full_path = self.resolve(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, content)
            .with_context(|| format!("Failed to write file: {}", full_path.display()))
    }

    fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }

    fn create_dir(&mut self, path: &str) -> Result<()> {
        let full_path = self.resolve(path);
        std::fs::create_dir_all(&full_path)
            .with_context(|| format!("Failed to create directory: {}", full_path.display()))
    }

    fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let full_path = self.resolve(path);
        let entries = std::fs::read_dir(&full_path)
            .with_context(|| format!("Failed to list directory: {}", full_path.display()))?;

        let mut files = Vec::new();
        for entry in entries {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }
        Ok(files)
    }

    fn file_size(&self, path: &str) -> Result<u64> {
        let full_path = self.resolve(path);
        let metadata = std::fs::metadata(&full_path)
            .with_context(|| format!("Failed to get metadata: {}", full_path.display()))?;
        Ok(metadata.len())
    }
}

/// Real quality checker implementation.
///
/// Uses the comprehensive `QualityGateEnforcer` for running quality checks.
/// This integrates the quality gate enforcement system with the testable trait interface.
///
/// # Polyglot Support (Phase 7.2)
///
/// This checker supports two modes of operation:
/// 1. **Legacy mode**: Uses `QualityGateEnforcer` with `EnforcerConfig` (Rust-only)
/// 2. **Polyglot mode**: Uses injected `QualityGate` instances via `with_gates()`
///
/// The `run_gates()` method runs all injected gates when in polyglot mode.
pub struct RealQualityChecker {
    project_dir: PathBuf,
    /// Configuration for the quality gate enforcer (legacy mode).
    enforcer_config: EnforcerConfig,
    /// Injected quality gates for polyglot support.
    /// When non-empty, `run_gates()` uses these instead of the enforcer.
    gates: Arc<Vec<Box<dyn QualityGate>>>,
}

impl std::fmt::Debug for RealQualityChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealQualityChecker")
            .field("project_dir", &self.project_dir)
            .field("enforcer_config", &self.enforcer_config)
            .field("gates_count", &self.gates.len())
            .finish()
    }
}

impl Clone for RealQualityChecker {
    fn clone(&self) -> Self {
        Self {
            project_dir: self.project_dir.clone(),
            enforcer_config: self.enforcer_config.clone(),
            gates: Arc::clone(&self.gates),
        }
    }
}

impl RealQualityChecker {
    /// Create a new quality checker for the given directory.
    ///
    /// Uses the default enforcer configuration (Rust quality gates).
    /// For polyglot support, use `with_gates()` instead.
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            project_dir,
            enforcer_config: EnforcerConfig::default(),
            gates: Arc::new(Vec::new()),
        }
    }

    /// Create a quality checker with custom enforcer configuration.
    ///
    /// This is used for legacy Rust-only quality checking.
    /// For polyglot support, use `with_gates()` instead.
    #[must_use]
    pub fn with_config(project_dir: PathBuf, config: EnforcerConfig) -> Self {
        let mut checker = Self::new(project_dir);
        checker.enforcer_config = config;
        checker
    }

    /// Create a quality checker with injected quality gates.
    ///
    /// This enables polyglot quality checking by allowing gates for multiple
    /// languages to be injected. Use `run_gates()` to execute all injected gates.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory
    /// * `gates` - Quality gates to run (typically from `detect_available_gates()`)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::quality::gates::detect_available_gates;
    /// use ralph::bootstrap::Language;
    ///
    /// let languages = vec![Language::Rust, Language::Python];
    /// let gates = detect_available_gates(&project_dir, &languages);
    /// let checker = RealQualityChecker::with_gates(project_dir, gates);
    ///
    /// let result = checker.run_gates().unwrap();
    /// ```
    #[must_use]
    pub fn with_gates(project_dir: PathBuf, gates: Vec<Box<dyn QualityGate>>) -> Self {
        Self {
            project_dir,
            enforcer_config: EnforcerConfig::default(),
            gates: Arc::new(gates),
        }
    }

    /// Returns the names of all injected quality gates.
    ///
    /// Returns an empty vector if using legacy enforcer mode (no injected gates).
    #[must_use]
    pub fn gate_names(&self) -> Vec<String> {
        self.gates.iter().map(|g| g.name().to_string()).collect()
    }

    /// Run all injected quality gates and return combined results.
    ///
    /// This method iterates over all gates provided via `with_gates()` and
    /// collects their results. Gates are run in order and failures are combined.
    ///
    /// # Returns
    ///
    /// A `QualityGateResult` with:
    /// - `passed`: true if all gates passed (or no gates to run)
    /// - `failures`: combined failure messages from all gates
    /// - `warnings`: empty (not currently used by polyglot gates)
    /// - `output`: empty (individual gate outputs not combined)
    ///
    /// # Errors
    ///
    /// Returns an error if any gate fails to execute (not if checks fail).
    pub fn run_gates(&self) -> Result<QualityGateResult> {
        // If no gates are injected, return success
        if self.gates.is_empty() {
            return Ok(QualityGateResult::pass());
        }

        let mut all_failures = Vec::new();
        let mut all_passed = true;

        // Run each gate in order (deterministic)
        for gate in self.gates.iter() {
            let issues = gate.run(&self.project_dir)?;

            // Convert issues to failure messages
            for issue in &issues {
                let msg = format!("[{}] {}", gate.name(), issue.message);
                all_failures.push(msg);
            }

            // Check if gate failed (has blocking issues)
            if !issues.is_empty() && gate.is_blocking() {
                all_passed = false;
            }
        }

        Ok(QualityGateResult {
            passed: all_passed,
            warnings: Vec::new(),
            failures: all_failures,
            output: String::new(),
        })
    }

    /// Get an enforcer instance configured for this checker.
    fn enforcer(&self) -> QualityGateEnforcer {
        QualityGateEnforcer::with_config(&self.project_dir, self.enforcer_config.clone())
    }
}

impl QualityChecker for RealQualityChecker {
    fn run_clippy(&self) -> Result<QualityGateResult> {
        let enforcer = self.enforcer();
        let result = enforcer.run_clippy()?;

        // Convert GateResult to QualityGateResult
        let warnings: Vec<String> = result
            .issues
            .iter()
            .map(|issue| {
                let mut msg = issue.message.clone();
                if let Some(ref file) = issue.file {
                    msg = format!("{}: {}", file.display(), msg);
                }
                msg
            })
            .collect();

        Ok(QualityGateResult {
            passed: result.passed,
            warnings,
            failures: Vec::new(),
            output: result.raw_output,
        })
    }

    fn run_tests(&self) -> Result<QualityGateResult> {
        let enforcer = self.enforcer();
        let result = enforcer.run_tests()?;

        // Convert GateResult to QualityGateResult
        let failures: Vec<String> = result
            .issues
            .iter()
            .map(|issue| {
                let mut msg = issue.message.clone();
                if let Some(ref file) = issue.file {
                    msg = format!("{}: {}", file.display(), msg);
                }
                msg
            })
            .collect();

        Ok(QualityGateResult {
            passed: result.passed,
            warnings: Vec::new(),
            failures,
            output: result.raw_output,
        })
    }

    fn run_security_scan(&self) -> Result<QualityGateResult> {
        let enforcer = self.enforcer();
        let result = enforcer.run_security()?;

        // Convert GateResult to QualityGateResult
        let failures: Vec<String> = result
            .issues
            .iter()
            .map(|issue| {
                let mut msg = format!("[{:?}] {}", issue.severity, issue.message);
                if let Some(ref code) = issue.code {
                    msg = format!("[{}] {}", code, msg);
                }
                msg
            })
            .collect();

        Ok(QualityGateResult {
            passed: result.passed,
            warnings: Vec::new(),
            failures,
            output: result.raw_output,
        })
    }

    fn check_no_allow_annotations(&self) -> Result<QualityGateResult> {
        let enforcer = self.enforcer();
        let result = enforcer.run_no_allow()?;

        // Convert GateResult to QualityGateResult
        let warnings: Vec<String> = result
            .issues
            .iter()
            .map(|issue| {
                let mut msg = issue.message.clone();
                if let Some(ref file) = issue.file {
                    if let Some(line) = issue.line {
                        msg = format!("{}:{}: {}", file.display(), line, msg);
                    } else {
                        msg = format!("{}: {}", file.display(), msg);
                    }
                }
                msg
            })
            .collect();

        Ok(QualityGateResult {
            passed: result.passed,
            warnings,
            failures: Vec::new(),
            output: result.raw_output,
        })
    }

    fn run_gates(&self) -> Result<QualityGateResult> {
        // Delegate to the inherent method
        RealQualityChecker::run_gates(self)
    }

    fn gate_names(&self) -> Vec<String> {
        // Delegate to the inherent method
        RealQualityChecker::gate_names(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph::quality::gates::{GateIssue, IssueSeverity, QualityGate};
    use std::path::Path;
    use tempfile::TempDir;

    // =========================================================================
    // Mock QualityGate for testing (Phase 7.2)
    // =========================================================================

    /// A mock quality gate for testing gate injection.
    struct MockGate {
        name: String,
        issues: Vec<GateIssue>,
        blocking: bool,
    }

    impl MockGate {
        /// Create a passing gate with no issues.
        fn passing(name: &str) -> Self {
            Self {
                name: name.to_string(),
                issues: Vec::new(),
                blocking: true,
            }
        }

        /// Create a failing gate with specified issues.
        fn failing(name: &str, messages: Vec<&str>) -> Self {
            let issues = messages
                .iter()
                .map(|msg| GateIssue::new(IssueSeverity::Error, *msg))
                .collect();
            Self {
                name: name.to_string(),
                issues,
                blocking: true,
            }
        }
    }

    impl QualityGate for MockGate {
        fn name(&self) -> &str {
            &self.name
        }

        fn run(&self, _project_dir: &Path) -> Result<Vec<GateIssue>> {
            Ok(self.issues.clone())
        }

        fn is_blocking(&self) -> bool {
            self.blocking
        }

        fn remediation(&self, issues: &[GateIssue]) -> String {
            issues
                .iter()
                .map(|i| i.message.clone())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    // =========================================================================
    // RealQualityChecker with_gates() tests (Phase 7.2)
    // =========================================================================

    #[test]
    fn test_with_gates_stores_provided_gates() {
        let temp = TempDir::new().unwrap();
        let gates: Vec<Box<dyn QualityGate>> = vec![
            Box::new(MockGate::passing("Gate1")),
            Box::new(MockGate::passing("Gate2")),
        ];

        let checker = RealQualityChecker::with_gates(temp.path().to_path_buf(), gates);

        // Verify gates were stored
        assert_eq!(checker.gate_names().len(), 2);
        assert!(checker.gate_names().contains(&"Gate1".to_string()));
        assert!(checker.gate_names().contains(&"Gate2".to_string()));
    }

    #[test]
    fn test_run_gates_executes_all_injected_gates() {
        let temp = TempDir::new().unwrap();
        let gates: Vec<Box<dyn QualityGate>> = vec![
            Box::new(MockGate::passing("Gate1")),
            Box::new(MockGate::passing("Gate2")),
            Box::new(MockGate::passing("Gate3")),
        ];

        let checker = RealQualityChecker::with_gates(temp.path().to_path_buf(), gates);
        let result = checker.run_gates().unwrap();

        // All gates should pass
        assert!(result.passed);
        assert!(result.warnings.is_empty());
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_run_gates_returns_combined_results_from_multiple_languages() {
        let temp = TempDir::new().unwrap();
        let gates: Vec<Box<dyn QualityGate>> = vec![
            Box::new(MockGate::failing("Clippy", vec!["rust warning 1"])),
            Box::new(MockGate::failing(
                "Ruff",
                vec!["python warning 1", "python warning 2"],
            )),
        ];

        let checker = RealQualityChecker::with_gates(temp.path().to_path_buf(), gates);
        let result = checker.run_gates().unwrap();

        // Should fail and have combined failures
        assert!(!result.passed);
        assert_eq!(result.failures.len(), 3);
        assert!(result.failures.iter().any(|f| f.contains("rust warning")));
        assert!(result.failures.iter().any(|f| f.contains("python warning")));
    }

    #[test]
    fn test_run_gates_empty_gates_list_returns_success() {
        let temp = TempDir::new().unwrap();
        let gates: Vec<Box<dyn QualityGate>> = vec![];

        let checker = RealQualityChecker::with_gates(temp.path().to_path_buf(), gates);
        let result = checker.run_gates().unwrap();

        // No gates to fail means success
        assert!(result.passed);
        assert!(result.warnings.is_empty());
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_run_gates_execution_order_is_deterministic() {
        let temp = TempDir::new().unwrap();

        // Run multiple times with same gate order
        for _ in 0..3 {
            let gates: Vec<Box<dyn QualityGate>> = vec![
                Box::new(MockGate::failing("Gate1", vec!["error1"])),
                Box::new(MockGate::failing("Gate2", vec!["error2"])),
                Box::new(MockGate::failing("Gate3", vec!["error3"])),
            ];

            let checker = RealQualityChecker::with_gates(temp.path().to_path_buf(), gates);
            let result = checker.run_gates().unwrap();

            // Failures should be in deterministic order
            assert_eq!(result.failures.len(), 3);
            assert!(result.failures[0].contains("error1"));
            assert!(result.failures[1].contains("error2"));
            assert!(result.failures[2].contains("error3"));
        }
    }

    #[test]
    fn test_new_provides_backward_compatibility() {
        let temp = TempDir::new().unwrap();
        let checker = RealQualityChecker::new(temp.path().to_path_buf());

        // new() should use default enforcer config (backward compatible)
        assert!(checker.enforcer_config.run_clippy);
        assert!(checker.enforcer_config.run_tests);
        // The gates field should be empty when using legacy enforcer
        assert!(checker.gate_names().is_empty());
    }

    // =========================================================================
    // Original tests
    // =========================================================================

    #[test]
    fn test_real_file_system_read_write() {
        let temp = TempDir::new().unwrap();
        let mut fs = RealFileSystem::new(temp.path().to_path_buf());

        fs.write_file("test.txt", "hello world").unwrap();
        let content = fs.read_file("test.txt").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_real_file_system_exists() {
        let temp = TempDir::new().unwrap();
        let mut fs = RealFileSystem::new(temp.path().to_path_buf());

        assert!(!fs.exists("test.txt"));
        fs.write_file("test.txt", "content").unwrap();
        assert!(fs.exists("test.txt"));
    }

    #[test]
    fn test_real_file_system_create_dir() {
        let temp = TempDir::new().unwrap();
        let mut fs = RealFileSystem::new(temp.path().to_path_buf());

        fs.create_dir("nested/dir").unwrap();
        assert!(temp.path().join("nested/dir").exists());
    }

    #[test]
    fn test_real_file_system_list_dir() {
        let temp = TempDir::new().unwrap();
        let mut fs = RealFileSystem::new(temp.path().to_path_buf());

        fs.write_file("a.txt", "a").unwrap();
        fs.write_file("b.txt", "b").unwrap();

        let files = fs.list_dir(".").unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_real_file_system_file_size() {
        let temp = TempDir::new().unwrap();
        let mut fs = RealFileSystem::new(temp.path().to_path_buf());

        fs.write_file("test.txt", "12345").unwrap();
        assert_eq!(fs.file_size("test.txt").unwrap(), 5);
    }

    #[test]
    fn test_real_git_operations_in_non_repo() {
        let temp = TempDir::new().unwrap();
        let git = RealGitOperations::new(temp.path().to_path_buf());

        // Should return empty string for non-repo
        let hash = git.get_commit_hash().unwrap();
        assert!(hash.is_empty());
    }

    #[test]
    fn test_real_claude_process_construction() {
        let temp = TempDir::new().unwrap();
        let claude = RealClaudeProcess::new(temp.path().to_path_buf());
        // Verify it's constructed (we don't actually run Claude in tests)
        assert_eq!(claude.project_dir, temp.path());
    }

    #[test]
    fn test_real_claude_process_default_model() {
        let temp = TempDir::new().unwrap();
        let claude = RealClaudeProcess::new(temp.path().to_path_buf());
        // Default model should be "opus"
        assert_eq!(claude.model(), "opus");
    }

    #[test]
    fn test_real_claude_process_with_model() {
        let temp = TempDir::new().unwrap();
        let claude = RealClaudeProcess::with_model(temp.path().to_path_buf(), "sonnet".to_string());
        // Custom model should be used
        assert_eq!(claude.model(), "sonnet");
        assert_eq!(claude.project_dir, temp.path());
    }

    #[test]
    fn test_real_claude_process_with_model_haiku() {
        let temp = TempDir::new().unwrap();
        let claude = RealClaudeProcess::with_model(temp.path().to_path_buf(), "haiku".to_string());
        assert_eq!(claude.model(), "haiku");
    }

    #[test]
    fn test_real_quality_checker_construction() {
        let temp = TempDir::new().unwrap();
        let checker = RealQualityChecker::new(temp.path().to_path_buf());
        // Verify it's constructed with default config
        assert_eq!(checker.project_dir, temp.path());
        assert!(checker.enforcer_config.run_clippy);
        assert!(checker.enforcer_config.run_tests);
    }

    #[test]
    fn test_real_quality_checker_with_config() {
        let temp = TempDir::new().unwrap();
        let config = EnforcerConfig::new()
            .with_clippy(true)
            .with_tests(false)
            .with_security(false);
        let checker = RealQualityChecker::with_config(temp.path().to_path_buf(), config);
        // Verify custom config is used
        assert_eq!(checker.project_dir, temp.path());
        assert!(checker.enforcer_config.run_clippy);
        assert!(!checker.enforcer_config.run_tests);
        assert!(!checker.enforcer_config.run_security);
    }
}
