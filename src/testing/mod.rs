//! Testing infrastructure for Ralph.
//!
//! This module provides traits, mocks, fixtures, and assertions for testing
//! the automation loop and its components without real external dependencies.
//!
//! # Architecture
//!
//! The testing infrastructure is organized into:
//! - **Traits**: Abstractions for external dependencies (git, subprocess, file system)
//! - **Mocks**: Test doubles that implement the traits with controllable behavior
//! - **Fixtures**: Pre-built test data and project structures (test-only)
//! - **Assertions**: Custom assertions for domain-specific testing
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::testing::{MockGitOperations, MockClaudeProcess, TestFixture};
//!
//! let git = MockGitOperations::new()
//!     .with_commit_hash("abc123")
//!     .with_commits_since(3);
//!
//! let claude = MockClaudeProcess::new()
//!     .with_exit_code(0);
//!
//! let fixture = TestFixture::minimal_project();
//! ```

pub mod assertions;
#[cfg(test)]
pub mod fixtures;
pub mod mocks;
pub mod traits;

// Re-export commonly used types
pub use assertions::*;
#[cfg(test)]
pub use fixtures::*;
pub use mocks::*;
pub use traits::*;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Mock Git Operations Tests
    // =========================================================================

    #[test]
    fn test_mock_git_operations_default() {
        let git = MockGitOperations::default();
        assert!(git.get_commit_hash().unwrap().is_empty());
        assert_eq!(git.count_commits_since(""), 0);
    }

    #[test]
    fn test_mock_git_operations_with_commit_hash() {
        let git = MockGitOperations::new().with_commit_hash("abc123def456");
        assert_eq!(git.get_commit_hash().unwrap(), "abc123def456");
    }

    #[test]
    fn test_mock_git_operations_with_commits_since() {
        let git = MockGitOperations::new().with_commits_since(5);
        assert_eq!(git.count_commits_since("old_hash"), 5);
    }

    // =========================================================================
    // Mock Claude Process Tests
    // =========================================================================

    #[tokio::test]
    async fn test_mock_claude_process_default() {
        let claude = MockClaudeProcess::default();
        // Default should succeed
        let result = claude.run_iteration("test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mock_claude_process_with_exit_code() {
        let claude = MockClaudeProcess::new().with_exit_code(1);
        let result = claude.run_iteration("test prompt").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_mock_claude_process_with_error() {
        let claude = MockClaudeProcess::new().with_error("Process crashed");
        let result = claude.run_iteration("test prompt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Process crashed"));
    }

    // =========================================================================
    // Mock File System Tests
    // =========================================================================

    #[test]
    fn test_mock_file_system_read_write() {
        let mut fs = MockFileSystem::new();
        fs.write_file("test.txt", "hello world").unwrap();
        let content = fs.read_file("test.txt").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_mock_file_system_file_not_found() {
        let fs = MockFileSystem::new();
        let result = fs.read_file("nonexistent.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_file_system_exists() {
        let mut fs = MockFileSystem::new();
        assert!(!fs.exists("test.txt"));
        fs.write_file("test.txt", "content").unwrap();
        assert!(fs.exists("test.txt"));
    }

    // =========================================================================
    // Mock Quality Checker Tests
    // =========================================================================

    #[test]
    fn test_mock_quality_checker_all_pass() {
        let checker = MockQualityChecker::new().all_passing();
        assert!(checker.run_clippy().unwrap().passed);
        assert!(checker.run_tests().unwrap().passed);
    }

    #[test]
    fn test_mock_quality_checker_clippy_fails() {
        let checker = MockQualityChecker::new()
            .with_clippy_warnings(vec!["warning: unused variable `x`".to_string()]);
        let result = checker.run_clippy().unwrap();
        assert!(!result.passed);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_mock_quality_checker_tests_fail() {
        let checker = MockQualityChecker::new()
            .with_test_failures(vec!["test_something_important".to_string()]);
        let result = checker.run_tests().unwrap();
        assert!(!result.passed);
        assert_eq!(result.failures.len(), 1);
    }

    // =========================================================================
    // Test Fixture Tests (only available in test builds)
    // =========================================================================

    #[test]
    fn test_fixture_minimal_project() {
        let fixture = TestFixture::minimal_project();
        assert!(fixture.has_implementation_plan());
        assert!(fixture.path().exists());
    }

    #[test]
    fn test_fixture_with_prompt_files() {
        let fixture = TestFixture::with_prompt_files();
        assert!(fixture.has_prompt_file("build"));
        assert!(fixture.has_prompt_file("debug"));
        assert!(fixture.has_prompt_file("plan"));
    }

    #[test]
    fn test_fixture_with_git_repo() {
        let fixture = TestFixture::with_git_repo();
        assert!(fixture.is_git_repo());
    }
}
