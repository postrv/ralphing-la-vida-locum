//! Mock implementations of testing traits.
//!
//! These mocks provide controllable test doubles for external dependencies,
//! enabling deterministic unit tests.

use super::traits::{ClaudeProcess, FileSystem, GitOperations, QualityChecker, QualityGateResult};
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// Mock implementation of git operations.
///
/// # Example
///
/// ```rust,ignore
/// let git = MockGitOperations::new()
///     .with_commit_hash("abc123")
///     .with_commits_since(3);
///
/// assert_eq!(git.count_commits_since("old"), 3);
/// ```
#[derive(Debug, Clone)]
pub struct MockGitOperations {
    commit_hash: String,
    commits_since: u32,
    branch: String,
    modified_files: Vec<String>,
    push_succeeds: bool,
    push_error: Option<String>,
}

impl Default for MockGitOperations {
    fn default() -> Self {
        Self {
            commit_hash: String::new(),
            commits_since: 0,
            branch: "main".to_string(),
            modified_files: Vec::new(),
            push_succeeds: true,
            push_error: None,
        }
    }
}

impl MockGitOperations {
    /// Create a new mock with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the commit hash to return.
    #[must_use]
    pub fn with_commit_hash(mut self, hash: &str) -> Self {
        self.commit_hash = hash.to_string();
        self
    }

    /// Set the number of commits since a given hash.
    #[must_use]
    pub fn with_commits_since(mut self, count: u32) -> Self {
        self.commits_since = count;
        self
    }

    /// Set the current branch name.
    #[must_use]
    pub fn with_branch(mut self, branch: &str) -> Self {
        self.branch = branch.to_string();
        self
    }

    /// Set the list of modified files.
    #[must_use]
    pub fn with_modified_files(mut self, files: Vec<String>) -> Self {
        self.modified_files = files;
        self
    }

    /// Configure push to fail with an error.
    #[must_use]
    pub fn with_push_error(mut self, error: &str) -> Self {
        self.push_succeeds = false;
        self.push_error = Some(error.to_string());
        self
    }
}

impl GitOperations for MockGitOperations {
    fn get_commit_hash(&self) -> Result<String> {
        Ok(self.commit_hash.clone())
    }

    fn count_commits_since(&self, _old_hash: &str) -> u32 {
        self.commits_since
    }

    fn get_branch(&self) -> Result<String> {
        Ok(self.branch.clone())
    }

    fn get_modified_files(&self) -> Result<Vec<String>> {
        Ok(self.modified_files.clone())
    }

    fn push(&self, _remote: &str, _branch: &str) -> Result<()> {
        if self.push_succeeds {
            Ok(())
        } else {
            bail!(
                "{}",
                self.push_error
                    .as_deref()
                    .unwrap_or("Push failed")
            )
        }
    }
}

/// Mock implementation of Claude Code process.
///
/// Thread-safe for use in async contexts.
///
/// # Example
///
/// ```rust,ignore
/// let claude = MockClaudeProcess::new()
///     .with_exit_code(0);
///
/// assert_eq!(claude.run_iteration("prompt").await.unwrap(), 0);
/// ```
#[derive(Debug)]
pub struct MockClaudeProcess {
    exit_code: i32,
    error: Option<String>,
    agent_output: String,
    call_count: AtomicU32,
}

impl Clone for MockClaudeProcess {
    fn clone(&self) -> Self {
        Self {
            exit_code: self.exit_code,
            error: self.error.clone(),
            agent_output: self.agent_output.clone(),
            call_count: AtomicU32::new(self.call_count.load(Ordering::SeqCst)),
        }
    }
}

impl Default for MockClaudeProcess {
    fn default() -> Self {
        Self {
            exit_code: 0,
            error: None,
            agent_output: String::new(),
            call_count: AtomicU32::new(0),
        }
    }
}

impl MockClaudeProcess {
    /// Create a new mock with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the exit code to return.
    #[must_use]
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }

    /// Configure the mock to return an error.
    #[must_use]
    pub fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.to_string());
        self
    }

    /// Set the output for agent runs.
    #[must_use]
    pub fn with_agent_output(mut self, output: &str) -> Self {
        self.agent_output = output.to_string();
        self
    }

    /// Get the number of times run_iteration was called.
    pub fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ClaudeProcess for MockClaudeProcess {
    async fn run_iteration(&self, _prompt: &str) -> Result<i32> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if let Some(ref error) = self.error {
            bail!("{}", error)
        } else {
            Ok(self.exit_code)
        }
    }

    async fn run_agent(&self, _agent: &str, _prompt: &str) -> Result<String> {
        if let Some(ref error) = self.error {
            bail!("{}", error)
        } else {
            Ok(self.agent_output.clone())
        }
    }
}

/// Mock implementation of file system operations.
///
/// Uses an in-memory HashMap to simulate file storage.
///
/// # Example
///
/// ```rust,ignore
/// let mut fs = MockFileSystem::new();
/// fs.write_file("test.txt", "content").unwrap();
/// assert_eq!(fs.read_file("test.txt").unwrap(), "content");
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockFileSystem {
    files: HashMap<String, String>,
    directories: Vec<String>,
}

impl MockFileSystem {
    /// Create a new empty mock file system.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populate with files.
    #[must_use]
    pub fn with_files(mut self, files: HashMap<String, String>) -> Self {
        self.files = files;
        self
    }

    /// Add a single file.
    #[must_use]
    pub fn with_file(mut self, path: &str, content: &str) -> Self {
        self.files.insert(path.to_string(), content.to_string());
        self
    }
}

impl FileSystem for MockFileSystem {
    fn read_file(&self, path: &str) -> Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", path))
    }

    fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        self.files.insert(path.to_string(), content.to_string());
        Ok(())
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path) || self.directories.contains(&path.to_string())
    }

    fn create_dir(&mut self, path: &str) -> Result<()> {
        if !self.directories.contains(&path.to_string()) {
            self.directories.push(path.to_string());
        }
        Ok(())
    }

    fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let prefix = if path.ends_with('/') {
            path.to_string()
        } else {
            format!("{}/", path)
        };

        let files: Vec<String> = self
            .files
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| {
                // Extract just the filename, not subdirectories
                let rest = &k[prefix.len()..];
                if let Some(slash_pos) = rest.find('/') {
                    rest[..slash_pos].to_string()
                } else {
                    rest.to_string()
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if files.is_empty() && !self.directories.contains(&path.to_string()) {
            bail!("Directory not found: {}", path)
        }

        Ok(files)
    }

    fn file_size(&self, path: &str) -> Result<u64> {
        self.files
            .get(path)
            .map(|content| content.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", path))
    }
}

/// Mock implementation of quality checker.
///
/// # Example
///
/// ```rust,ignore
/// let checker = MockQualityChecker::new()
///     .all_passing();
///
/// assert!(checker.run_clippy().unwrap().passed);
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockQualityChecker {
    clippy_warnings: Vec<String>,
    test_failures: Vec<String>,
    security_findings: Vec<String>,
    allow_annotations: Vec<String>,
}

impl MockQualityChecker {
    /// Create a new mock quality checker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure all gates to pass.
    #[must_use]
    pub fn all_passing(self) -> Self {
        // Default state is already passing (empty vectors)
        self
    }

    /// Set clippy warnings.
    #[must_use]
    pub fn with_clippy_warnings(mut self, warnings: Vec<String>) -> Self {
        self.clippy_warnings = warnings;
        self
    }

    /// Set test failures.
    #[must_use]
    pub fn with_test_failures(mut self, failures: Vec<String>) -> Self {
        self.test_failures = failures;
        self
    }

    /// Set security findings.
    #[must_use]
    pub fn with_security_findings(mut self, findings: Vec<String>) -> Self {
        self.security_findings = findings;
        self
    }

    /// Set allow annotation locations.
    #[must_use]
    pub fn with_allow_annotations(mut self, locations: Vec<String>) -> Self {
        self.allow_annotations = locations;
        self
    }
}

impl QualityChecker for MockQualityChecker {
    fn run_clippy(&self) -> Result<QualityGateResult> {
        if self.clippy_warnings.is_empty() {
            Ok(QualityGateResult::pass())
        } else {
            Ok(QualityGateResult::fail_with_warnings(
                self.clippy_warnings.clone(),
            ))
        }
    }

    fn run_tests(&self) -> Result<QualityGateResult> {
        if self.test_failures.is_empty() {
            Ok(QualityGateResult::pass())
        } else {
            Ok(QualityGateResult::fail_with_failures(
                self.test_failures.clone(),
            ))
        }
    }

    fn run_security_scan(&self) -> Result<QualityGateResult> {
        if self.security_findings.is_empty() {
            Ok(QualityGateResult::pass())
        } else {
            Ok(QualityGateResult::fail_with_failures(
                self.security_findings.clone(),
            ))
        }
    }

    fn check_no_allow_annotations(&self) -> Result<QualityGateResult> {
        if self.allow_annotations.is_empty() {
            Ok(QualityGateResult::pass())
        } else {
            Ok(QualityGateResult::fail_with_warnings(
                self.allow_annotations.clone(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_git_default_values() {
        let git = MockGitOperations::default();
        assert!(git.commit_hash.is_empty());
        assert_eq!(git.commits_since, 0);
        assert_eq!(git.branch, "main");
    }

    #[test]
    fn test_mock_git_builder_pattern() {
        let git = MockGitOperations::new()
            .with_commit_hash("abc123")
            .with_commits_since(5)
            .with_branch("feature")
            .with_modified_files(vec!["src/main.rs".to_string()]);

        assert_eq!(git.get_commit_hash().unwrap(), "abc123");
        assert_eq!(git.count_commits_since("old"), 5);
        assert_eq!(git.get_branch().unwrap(), "feature");
        assert_eq!(git.get_modified_files().unwrap().len(), 1);
    }

    #[test]
    fn test_mock_git_push_success() {
        let git = MockGitOperations::new();
        assert!(git.push("origin", "main").is_ok());
    }

    #[test]
    fn test_mock_git_push_failure() {
        let git = MockGitOperations::new().with_push_error("Authentication failed");
        let result = git.push("origin", "main");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Authentication"));
    }

    #[tokio::test]
    async fn test_mock_claude_call_count() {
        let claude = MockClaudeProcess::new();
        assert_eq!(claude.call_count(), 0);

        claude.run_iteration("test").await.unwrap();
        assert_eq!(claude.call_count(), 1);

        claude.run_iteration("test2").await.unwrap();
        assert_eq!(claude.call_count(), 2);
    }

    #[tokio::test]
    async fn test_mock_claude_agent_output() {
        let claude = MockClaudeProcess::new().with_agent_output("agent result");
        let output = claude.run_agent("test-agent", "prompt").await.unwrap();
        assert_eq!(output, "agent result");
    }

    #[test]
    fn test_mock_fs_with_files_builder() {
        let mut files = HashMap::new();
        files.insert("a.txt".to_string(), "content a".to_string());
        files.insert("b.txt".to_string(), "content b".to_string());

        let fs = MockFileSystem::new().with_files(files);
        assert_eq!(fs.read_file("a.txt").unwrap(), "content a");
        assert_eq!(fs.read_file("b.txt").unwrap(), "content b");
    }

    #[test]
    fn test_mock_fs_file_size() {
        let fs = MockFileSystem::new().with_file("test.txt", "hello");
        assert_eq!(fs.file_size("test.txt").unwrap(), 5);
    }

    #[test]
    fn test_mock_fs_create_and_list_dir() {
        let mut fs = MockFileSystem::new();
        fs.create_dir("src").unwrap();
        fs.write_file("src/main.rs", "fn main() {}").unwrap();
        fs.write_file("src/lib.rs", "// lib").unwrap();

        let files = fs.list_dir("src").unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"main.rs".to_string()));
        assert!(files.contains(&"lib.rs".to_string()));
    }

    #[test]
    fn test_mock_quality_security_scan() {
        let checker = MockQualityChecker::new().with_security_findings(vec![
            "SQL injection in db.rs:42".to_string(),
        ]);
        let result = checker.run_security_scan().unwrap();
        assert!(!result.passed);
        assert_eq!(result.failures.len(), 1);
    }

    #[test]
    fn test_mock_quality_allow_annotations() {
        let checker = MockQualityChecker::new().with_allow_annotations(vec![
            "src/lib.rs:10: #[allow(dead_code)]".to_string(),
        ]);
        let result = checker.check_no_allow_annotations().unwrap();
        assert!(!result.passed);
        assert_eq!(result.warnings.len(), 1);
    }
}
