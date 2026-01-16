//! Test fixtures for creating reproducible test environments.
//!
//! Provides pre-built project structures and test data for consistent testing.

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A test fixture representing a temporary project directory.
///
/// Automatically cleans up when dropped.
///
/// # Example
///
/// ```rust,ignore
/// let fixture = TestFixture::minimal_project();
/// assert!(fixture.has_implementation_plan());
/// // Directory is cleaned up when fixture goes out of scope
/// ```
pub struct TestFixture {
    temp_dir: TempDir,
    is_git_repo: bool,
}

impl TestFixture {
    /// Create a minimal project with just an IMPLEMENTATION_PLAN.md.
    ///
    /// # Panics
    ///
    /// Panics if temporary directory creation fails.
    #[must_use]
    pub fn minimal_project() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        std::fs::write(
            temp_dir.path().join("IMPLEMENTATION_PLAN.md"),
            Self::minimal_plan_content(),
        )
        .expect("Failed to write IMPLEMENTATION_PLAN.md");

        Self {
            temp_dir,
            is_git_repo: false,
        }
    }

    /// Create a project with prompt files for all modes.
    ///
    /// # Panics
    ///
    /// Panics if file creation fails.
    #[must_use]
    pub fn with_prompt_files() -> Self {
        let fixture = Self::minimal_project();

        std::fs::write(
            fixture.temp_dir.path().join("PROMPT_build.md"),
            Self::build_prompt_content(),
        )
        .expect("Failed to write PROMPT_build.md");

        std::fs::write(
            fixture.temp_dir.path().join("PROMPT_debug.md"),
            Self::debug_prompt_content(),
        )
        .expect("Failed to write PROMPT_debug.md");

        std::fs::write(
            fixture.temp_dir.path().join("PROMPT_plan.md"),
            Self::plan_prompt_content(),
        )
        .expect("Failed to write PROMPT_plan.md");

        fixture
    }

    /// Create a project with a git repository initialized.
    ///
    /// # Panics
    ///
    /// Panics if git initialization fails.
    #[must_use]
    pub fn with_git_repo() -> Self {
        let mut fixture = Self::with_prompt_files();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(fixture.temp_dir.path())
            .output()
            .expect("Failed to initialize git repo");

        // Configure git user for commits
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(fixture.temp_dir.path())
            .output()
            .expect("Failed to configure git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(fixture.temp_dir.path())
            .output()
            .expect("Failed to configure git name");

        // Add and commit initial files
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(fixture.temp_dir.path())
            .output()
            .expect("Failed to git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(fixture.temp_dir.path())
            .output()
            .expect("Failed to git commit");

        fixture.is_git_repo = true;
        fixture
    }

    /// Create a project with a src directory and Rust files.
    ///
    /// # Panics
    ///
    /// Panics if file creation fails.
    #[must_use]
    pub fn with_rust_project() -> Self {
        let fixture = Self::with_git_repo();

        let src_dir = fixture.temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("Failed to create src directory");

        std::fs::write(src_dir.join("main.rs"), Self::main_rs_content())
            .expect("Failed to write main.rs");

        std::fs::write(src_dir.join("lib.rs"), Self::lib_rs_content())
            .expect("Failed to write lib.rs");

        std::fs::write(
            fixture.temp_dir.path().join("Cargo.toml"),
            Self::cargo_toml_content(),
        )
        .expect("Failed to write Cargo.toml");

        fixture
    }

    /// Get the path to the fixture directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Get the path as a PathBuf (owned).
    #[must_use]
    pub fn path_buf(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    /// Check if IMPLEMENTATION_PLAN.md exists.
    #[must_use]
    pub fn has_implementation_plan(&self) -> bool {
        self.temp_dir.path().join("IMPLEMENTATION_PLAN.md").exists()
    }

    /// Check if a prompt file exists for the given mode.
    #[must_use]
    pub fn has_prompt_file(&self, mode: &str) -> bool {
        self.temp_dir
            .path()
            .join(format!("PROMPT_{}.md", mode))
            .exists()
    }

    /// Check if this is a git repository.
    #[must_use]
    pub fn is_git_repo(&self) -> bool {
        self.is_git_repo && self.temp_dir.path().join(".git").exists()
    }

    /// Write a file to the fixture directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn write_file(&self, relative_path: &str, content: &str) -> std::io::Result<()> {
        let path = self.temp_dir.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)
    }

    /// Read a file from the fixture directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn read_file(&self, relative_path: &str) -> std::io::Result<String> {
        std::fs::read_to_string(self.temp_dir.path().join(relative_path))
    }

    /// Make a commit in the git repo.
    ///
    /// # Panics
    ///
    /// Panics if not a git repo or commit fails.
    pub fn make_commit(&self, message: &str) {
        assert!(self.is_git_repo, "Not a git repository");

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(self.temp_dir.path())
            .output()
            .expect("Failed to git add");

        std::process::Command::new("git")
            .args(["commit", "-m", message, "--allow-empty"])
            .current_dir(self.temp_dir.path())
            .output()
            .expect("Failed to git commit");
    }

    /// Get the current git commit hash.
    ///
    /// # Panics
    ///
    /// Panics if not a git repo or command fails.
    #[must_use]
    pub fn get_commit_hash(&self) -> String {
        assert!(self.is_git_repo, "Not a git repository");

        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(self.temp_dir.path())
            .output()
            .expect("Failed to get commit hash");

        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    // =========================================================================
    // Content Templates
    // =========================================================================

    fn minimal_plan_content() -> &'static str {
        r#"# Implementation Plan

## Tasks

### Phase 1: Foundation
- [ ] Task 1
- [ ] Task 2

### Phase 2: Implementation
- [ ] Task 3
- [ ] Task 4

## Completed
(none yet)
"#
    }

    fn build_prompt_content() -> &'static str {
        r#"# Build Phase

Select and implement the next task from IMPLEMENTATION_PLAN.md.

## Requirements
- Write tests first (TDD)
- Run clippy before committing
- Run tests before committing
"#
    }

    fn debug_prompt_content() -> &'static str {
        r#"# Debug Phase

Focus on resolving blockers and fixing issues.

## Steps
1. Identify the blocking issue
2. Create minimal reproduction
3. Fix the root cause
4. Verify with tests
"#
    }

    fn plan_prompt_content() -> &'static str {
        r#"# Plan Phase

Create or update the implementation plan.

## Guidelines
- Break down into small, testable tasks
- Identify dependencies
- Estimate complexity
"#
    }

    fn main_rs_content() -> &'static str {
        r#"fn main() {
    println!("Hello from test fixture");
}
"#
    }

    fn lib_rs_content() -> &'static str {
        r#"//! Test fixture library

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#
    }

    fn cargo_toml_content() -> &'static str {
        r#"[package]
name = "test-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_project_creates_plan() {
        let fixture = TestFixture::minimal_project();
        assert!(fixture.has_implementation_plan());
        assert!(!fixture.is_git_repo());
    }

    #[test]
    fn test_with_prompt_files_creates_all_prompts() {
        let fixture = TestFixture::with_prompt_files();
        assert!(fixture.has_prompt_file("build"));
        assert!(fixture.has_prompt_file("debug"));
        assert!(fixture.has_prompt_file("plan"));
    }

    #[test]
    fn test_with_git_repo_initializes_git() {
        let fixture = TestFixture::with_git_repo();
        assert!(fixture.is_git_repo());
        assert!(fixture.path().join(".git").exists());
    }

    #[test]
    fn test_write_and_read_file() {
        let fixture = TestFixture::minimal_project();
        fixture.write_file("test.txt", "hello world").unwrap();
        let content = fixture.read_file("test.txt").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_write_file_creates_directories() {
        let fixture = TestFixture::minimal_project();
        fixture
            .write_file("src/nested/file.rs", "// nested")
            .unwrap();
        assert!(fixture.path().join("src/nested/file.rs").exists());
    }

    #[test]
    fn test_make_commit() {
        let fixture = TestFixture::with_git_repo();
        let initial_hash = fixture.get_commit_hash();

        fixture.write_file("new_file.txt", "content").unwrap();
        fixture.make_commit("Add new file");

        let new_hash = fixture.get_commit_hash();
        assert_ne!(initial_hash, new_hash);
    }

    #[test]
    fn test_path_methods() {
        let fixture = TestFixture::minimal_project();
        assert!(fixture.path().exists());
        assert_eq!(fixture.path(), fixture.path_buf().as_path());
    }

    #[test]
    fn test_with_rust_project() {
        let fixture = TestFixture::with_rust_project();
        assert!(fixture.path().join("src/main.rs").exists());
        assert!(fixture.path().join("src/lib.rs").exists());
        assert!(fixture.path().join("Cargo.toml").exists());
    }
}
