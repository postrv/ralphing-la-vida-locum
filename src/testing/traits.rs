//! Trait definitions for testable abstractions.
//!
//! These traits abstract external dependencies to enable unit testing
//! without real git repositories, subprocesses, or file systems.

use anyhow::Result;
use async_trait::async_trait;

/// Abstraction for git operations.
///
/// Enables testing loop manager logic without real git repositories.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::testing::GitOperations;
///
/// fn check_progress(git: &impl GitOperations) -> bool {
///     git.count_commits_since("old_hash") > 0
/// }
/// ```
pub trait GitOperations {
    /// Get the current HEAD commit hash.
    ///
    /// # Errors
    ///
    /// Returns an error if git is not available or not in a repository.
    fn get_commit_hash(&self) -> Result<String>;

    /// Count commits since a given hash.
    ///
    /// Returns 0 if the hash is empty or invalid.
    fn count_commits_since(&self, old_hash: &str) -> u32;

    /// Get the current branch name.
    ///
    /// # Errors
    ///
    /// Returns an error if not in a git repository.
    fn get_branch(&self) -> Result<String>;

    /// Get list of modified files in the working tree.
    fn get_modified_files(&self) -> Result<Vec<String>>;

    /// Push to remote repository.
    ///
    /// # Errors
    ///
    /// Returns an error if push fails (auth, network, etc.).
    fn push(&self, remote: &str, branch: &str) -> Result<()>;
}

/// Abstraction for Claude Code subprocess execution.
///
/// Enables testing iteration logic without spawning real processes.
/// This trait is async to support non-blocking process execution.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::testing::ClaudeProcess;
///
/// async fn run_with_retry(claude: &impl ClaudeProcess, prompt: &str) -> Result<i32> {
///     for _ in 0..3 {
///         match claude.run_iteration(prompt).await {
///             Ok(0) => return Ok(0),
///             Ok(code) => continue,
///             Err(_) => continue,
///         }
///     }
///     Ok(1)
/// }
/// ```
#[async_trait]
pub trait ClaudeProcess: Send + Sync {
    /// Run a single Claude Code iteration with the given prompt.
    ///
    /// # Returns
    ///
    /// The exit code from the Claude process (0 = success).
    ///
    /// # Errors
    ///
    /// Returns an error if the process fails to spawn or crashes.
    async fn run_iteration(&self, prompt: &str) -> Result<i32>;

    /// Run Claude Code with a specific agent.
    ///
    /// # Errors
    ///
    /// Returns an error if the agent is not available or process fails.
    async fn run_agent(&self, agent: &str, prompt: &str) -> Result<String>;
}

/// Abstraction for file system operations.
///
/// Enables testing file-dependent logic with in-memory files.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::testing::FileSystem;
///
/// fn read_plan(fs: &impl FileSystem, project_dir: &Path) -> Result<String> {
///     fs.read_file(&project_dir.join("IMPLEMENTATION_PLAN.md"))
/// }
/// ```
pub trait FileSystem {
    /// Read file contents as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the file doesn't exist or can't be read.
    fn read_file(&self, path: &str) -> Result<String>;

    /// Write content to a file.
    ///
    /// Creates parent directories if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    fn write_file(&mut self, path: &str, content: &str) -> Result<()>;

    /// Check if a file or directory exists.
    fn exists(&self, path: &str) -> bool;

    /// Create a directory (and parents if needed).
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails.
    fn create_dir(&mut self, path: &str) -> Result<()>;

    /// List files in a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory doesn't exist.
    fn list_dir(&self, path: &str) -> Result<Vec<String>>;

    /// Get file metadata (size in bytes).
    ///
    /// # Errors
    ///
    /// Returns an error if the file doesn't exist.
    fn file_size(&self, path: &str) -> Result<u64>;
}

/// Result from a quality gate check.
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    /// Whether the gate passed.
    pub passed: bool,
    /// Warning messages (for clippy, etc.).
    pub warnings: Vec<String>,
    /// Failure messages (for tests, etc.).
    pub failures: Vec<String>,
    /// Raw output from the tool.
    pub output: String,
}

impl QualityGateResult {
    /// Create a passing result.
    #[must_use]
    pub fn pass() -> Self {
        Self {
            passed: true,
            warnings: Vec::new(),
            failures: Vec::new(),
            output: String::new(),
        }
    }

    /// Create a failing result with warnings.
    #[must_use]
    pub fn fail_with_warnings(warnings: Vec<String>) -> Self {
        Self {
            passed: false,
            warnings,
            failures: Vec::new(),
            output: String::new(),
        }
    }

    /// Create a failing result with test failures.
    #[must_use]
    pub fn fail_with_failures(failures: Vec<String>) -> Self {
        Self {
            passed: false,
            warnings: Vec::new(),
            failures,
            output: String::new(),
        }
    }
}

/// Abstraction for quality checking tools (clippy, tests, etc.).
///
/// Enables testing quality gate logic without running real tools.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::testing::QualityChecker;
///
/// fn can_commit(checker: &impl QualityChecker) -> bool {
///     checker.run_clippy().map(|r| r.passed).unwrap_or(false)
///         && checker.run_tests().map(|r| r.passed).unwrap_or(false)
/// }
/// ```
pub trait QualityChecker {
    /// Run clippy and return results.
    ///
    /// # Errors
    ///
    /// Returns an error if clippy fails to run.
    fn run_clippy(&self) -> Result<QualityGateResult>;

    /// Run tests and return results.
    ///
    /// # Errors
    ///
    /// Returns an error if cargo test fails to run.
    fn run_tests(&self) -> Result<QualityGateResult>;

    /// Run security scan and return results.
    ///
    /// # Errors
    ///
    /// Returns an error if security scanner is unavailable.
    fn run_security_scan(&self) -> Result<QualityGateResult>;

    /// Check for #[allow(...)] annotations.
    ///
    /// # Errors
    ///
    /// Returns an error if file scanning fails.
    fn check_no_allow_annotations(&self) -> Result<QualityGateResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_gate_result_pass() {
        let result = QualityGateResult::pass();
        assert!(result.passed);
        assert!(result.warnings.is_empty());
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_quality_gate_result_fail_with_warnings() {
        let result = QualityGateResult::fail_with_warnings(vec!["warning 1".to_string()]);
        assert!(!result.passed);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_quality_gate_result_fail_with_failures() {
        let result = QualityGateResult::fail_with_failures(vec!["test_failed".to_string()]);
        assert!(!result.passed);
        assert!(result.warnings.is_empty());
        assert_eq!(result.failures.len(), 1);
    }
}
