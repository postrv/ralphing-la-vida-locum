//! Change detection module for incremental execution.
//!
//! This module provides functionality to detect changed files since a commit
//! or in the working tree, enabling incremental execution of quality gates
//! and task selection on large codebases.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::changes::ChangeDetector;
//!
//! // Detect changes since a specific commit
//! let detector = ChangeDetector::new("/path/to/repo");
//! let changed = detector.changed_since("HEAD~5")?;
//!
//! // Filter by extension
//! let rust_files = detector
//!     .with_extensions(&["rs"])
//!     .changed_in_working_tree()?;
//! ```

use crate::error::{RalphError, Result};
use globset::Glob;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a scope of changed files for incremental execution.
///
/// `ChangeScope` captures which files have changed (either in the working tree
/// or since a specific commit) and provides methods for filtering and querying
/// these changes. It's designed to work with quality gates and context builders
/// to enable incremental execution on large codebases.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::changes::{ChangeDetector, ChangeScope};
///
/// // Create scope from working tree changes
/// let detector = ChangeDetector::new(".");
/// let scope = ChangeScope::from_detector(&detector)?;
///
/// // Or create from specific files
/// let scope = ChangeScope::from_files(vec![
///     PathBuf::from("src/main.rs"),
///     PathBuf::from("src/lib.rs"),
/// ]);
///
/// // Filter to only Rust files
/// let rust_scope = scope.filter_by_extensions(&["rs"]);
///
/// // Use with quality gates or context building
/// if scope.has_changes() {
///     println!("Changed {} files", scope.changed_files().len());
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct ChangeScope {
    /// Files that are in scope (changed or explicitly specified).
    files: Vec<PathBuf>,
    /// The commit reference this scope was computed from, if any.
    commit_ref: Option<String>,
}

impl ChangeScope {
    /// Create a new empty change scope.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let scope = ChangeScope::new();
    /// assert!(!scope.has_changes());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope from a list of file paths.
    ///
    /// # Arguments
    ///
    /// * `files` - Files to include in the scope
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let scope = ChangeScope::from_files(vec![
    ///     PathBuf::from("src/main.rs"),
    ///     PathBuf::from("src/lib.rs"),
    /// ]);
    /// ```
    #[must_use]
    pub fn from_files(files: Vec<PathBuf>) -> Self {
        Self {
            files,
            commit_ref: None,
        }
    }

    /// Create a scope from a `ChangeDetector` by detecting working tree changes.
    ///
    /// # Arguments
    ///
    /// * `detector` - The change detector to use
    ///
    /// # Errors
    ///
    /// Returns an error if git operations fail.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".");
    /// let scope = ChangeScope::from_detector(&detector)?;
    /// ```
    pub fn from_detector(detector: &ChangeDetector) -> Result<Self> {
        let files = detector.changed_in_working_tree()?;
        Ok(Self {
            files,
            commit_ref: None,
        })
    }

    /// Create a scope from changes since a specific commit.
    ///
    /// # Arguments
    ///
    /// * `detector` - The change detector to use
    /// * `commit` - Git commit reference (e.g., "HEAD~5", "abc123")
    ///
    /// # Errors
    ///
    /// Returns an error if the commit reference is invalid or git operations fail.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".");
    /// let scope = ChangeScope::from_detector_since(&detector, "HEAD~3")?;
    /// ```
    pub fn from_detector_since(detector: &ChangeDetector, commit: &str) -> Result<Self> {
        let files = detector.changed_since(commit)?;
        Ok(Self {
            files,
            commit_ref: Some(commit.to_string()),
        })
    }

    /// Set the commit reference for this scope.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let scope = ChangeScope::new().with_commit_ref("HEAD~5");
    /// ```
    #[must_use]
    pub fn with_commit_ref(mut self, commit: impl Into<String>) -> Self {
        self.commit_ref = Some(commit.into());
        self
    }

    /// Get the list of changed files.
    #[must_use]
    pub fn changed_files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Get the commit reference, if this scope was computed from a commit.
    #[must_use]
    pub fn commit_ref(&self) -> Option<&str> {
        self.commit_ref.as_deref()
    }

    /// Check if there are any changes in this scope.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.files.is_empty()
    }

    /// Check if a specific file is in this scope.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to check
    #[must_use]
    pub fn is_file_in_scope(&self, path: &Path) -> bool {
        self.files.iter().any(|f| f == path)
    }

    /// Filter the scope to only include files with specific extensions.
    ///
    /// Extensions should be provided without the leading dot (e.g., "rs" not ".rs").
    ///
    /// # Arguments
    ///
    /// * `extensions` - Extensions to filter by
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let rust_scope = scope.filter_by_extensions(&["rs"]);
    /// ```
    #[must_use]
    pub fn filter_by_extensions(&self, extensions: &[&str]) -> Self {
        let filtered: Vec<PathBuf> = self
            .files
            .iter()
            .filter(|path| {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy();
                    extensions.iter().any(|e| e.eq_ignore_ascii_case(&ext_str))
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        Self {
            files: filtered,
            commit_ref: self.commit_ref.clone(),
        }
    }

    /// Merge this scope with another, combining their changed files.
    ///
    /// Duplicate files are deduplicated.
    ///
    /// # Arguments
    ///
    /// * `other` - The scope to merge with
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let merged = scope1.merge(&scope2);
    /// ```
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut merged = Vec::new();

        for file in &self.files {
            if seen.insert(file.clone()) {
                merged.push(file.clone());
            }
        }

        for file in &other.files {
            if seen.insert(file.clone()) {
                merged.push(file.clone());
            }
        }

        Self {
            files: merged,
            // Keep our commit ref if we have one, otherwise use other's
            commit_ref: self.commit_ref.clone().or_else(|| other.commit_ref.clone()),
        }
    }
}

/// Detects changed files in a git repository.
///
/// `ChangeDetector` provides methods to identify files that have been modified,
/// added, deleted, or renamed since a specific commit or in the working tree.
///
/// # Example
///
/// ```rust,ignore
/// let detector = ChangeDetector::new(".");
///
/// // Get all files changed since HEAD~3
/// let changes = detector.changed_since("HEAD~3")?;
///
/// // Get files with uncommitted changes
/// let working_changes = detector.changed_in_working_tree()?;
///
/// // Filter to only Rust files
/// let rust_changes = detector
///     .with_extensions(&["rs"])
///     .changed_since("HEAD~1")?;
/// ```
#[derive(Debug, Clone)]
pub struct ChangeDetector {
    /// Path to the repository root
    repo_path: PathBuf,
    /// File extensions to filter by (empty means all files)
    extensions: Vec<String>,
    /// Glob patterns to filter by (empty means no glob filtering)
    glob_patterns: Vec<String>,
}

impl ChangeDetector {
    /// Create a new `ChangeDetector` for the given repository path.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository root
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".");
    /// let detector = ChangeDetector::new("/home/user/my-project");
    /// ```
    #[must_use]
    pub fn new<P: AsRef<Path>>(repo_path: P) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
            extensions: Vec::new(),
            glob_patterns: Vec::new(),
        }
    }

    /// Filter results to only include files with the specified extensions.
    ///
    /// Extensions should be provided without the leading dot (e.g., "rs" not ".rs").
    ///
    /// # Arguments
    ///
    /// * `extensions` - Slice of file extensions to include
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".")
    ///     .with_extensions(&["rs", "toml"]);
    /// ```
    #[must_use]
    pub fn with_extensions(mut self, extensions: &[&str]) -> Self {
        self.extensions = extensions.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Filter results using glob patterns.
    ///
    /// # Arguments
    ///
    /// * `patterns` - Slice of glob patterns to match against file paths
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".")
    ///     .with_glob_patterns(&["src/**/*.rs", "tests/**/*.rs"]);
    /// ```
    #[must_use]
    pub fn with_glob_patterns(mut self, patterns: &[&str]) -> Self {
        self.glob_patterns = patterns.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Detect files changed since a specific commit.
    ///
    /// Returns a list of files that have been modified, added, deleted, or
    /// renamed since the specified commit reference.
    ///
    /// # Arguments
    ///
    /// * `commit` - Git commit reference (e.g., "HEAD~5", "abc123", "main")
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is not a git repository
    /// - The commit reference is invalid
    /// - Git command fails to execute
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".");
    /// let changes = detector.changed_since("HEAD~3")?;
    /// for file in changes {
    ///     println!("Changed: {}", file.display());
    /// }
    /// ```
    pub fn changed_since(&self, commit: &str) -> Result<Vec<PathBuf>> {
        // Use git diff --name-status to get changed files
        // --diff-filter=ACDMR includes Added, Copied, Deleted, Modified, Renamed
        let output = Command::new("git")
            .args([
                "diff",
                "--name-status",
                "--diff-filter=ACDMR",
                "-M", // Detect renames
                commit,
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| RalphError::git("diff", e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RalphError::git("diff", stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_name_status_output(&stdout)
    }

    /// Detect files with uncommitted changes in the working tree.
    ///
    /// Returns a list of files that have staged or unstaged modifications,
    /// including new untracked files.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is not a git repository
    /// - Git command fails to execute
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = ChangeDetector::new(".");
    /// let changes = detector.changed_in_working_tree()?;
    /// println!("Uncommitted changes: {} files", changes.len());
    /// ```
    pub fn changed_in_working_tree(&self) -> Result<Vec<PathBuf>> {
        // Get staged changes (diff --cached)
        let staged_output = Command::new("git")
            .args([
                "diff",
                "--name-status",
                "--diff-filter=ACDMR",
                "-M",
                "--cached",
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| RalphError::git("diff --cached", e.to_string()))?;

        if !staged_output.status.success() {
            let stderr = String::from_utf8_lossy(&staged_output.stderr);
            return Err(RalphError::git("diff --cached", stderr.to_string()));
        }

        // Get unstaged changes (diff)
        let unstaged_output = Command::new("git")
            .args(["diff", "--name-status", "--diff-filter=ACDMR", "-M"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| RalphError::git("diff", e.to_string()))?;

        if !unstaged_output.status.success() {
            let stderr = String::from_utf8_lossy(&unstaged_output.stderr);
            return Err(RalphError::git("diff", stderr.to_string()));
        }

        // Get untracked files
        let untracked_output = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| RalphError::git("ls-files", e.to_string()))?;

        if !untracked_output.status.success() {
            let stderr = String::from_utf8_lossy(&untracked_output.stderr);
            return Err(RalphError::git("ls-files", stderr.to_string()));
        }

        let staged = String::from_utf8_lossy(&staged_output.stdout);
        let unstaged = String::from_utf8_lossy(&unstaged_output.stdout);
        let untracked = String::from_utf8_lossy(&untracked_output.stdout);

        let mut files = self.parse_name_status_output(&staged)?;
        let unstaged_files = self.parse_name_status_output(&unstaged)?;
        let untracked_files = self.parse_simple_file_list(&untracked)?;

        // Merge and deduplicate
        for file in unstaged_files {
            if !files.contains(&file) {
                files.push(file);
            }
        }
        for file in untracked_files {
            if !files.contains(&file) {
                files.push(file);
            }
        }

        Ok(files)
    }

    /// Parse git diff --name-status output into a list of file paths.
    ///
    /// Handles status codes: A (added), C (copied), D (deleted), M (modified), R (renamed)
    /// For renames (R), extracts the new file path.
    fn parse_name_status_output(&self, output: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Format: STATUS<tab>path or STATUS<tab>old_path<tab>new_path (for renames/copies)
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let status = parts[0];
            let file_path = if status.starts_with('R') || status.starts_with('C') {
                // For renames/copies, use the new path (second path)
                if parts.len() >= 3 {
                    parts[2]
                } else {
                    continue;
                }
            } else if parts.len() >= 2 {
                parts[1]
            } else {
                continue;
            };

            let path = PathBuf::from(file_path);

            // Apply extension filter
            if !self.extensions.is_empty() && !self.matches_extension(&path) {
                continue;
            }

            // Apply glob filter
            if !self.glob_patterns.is_empty() && !self.matches_glob(&path) {
                continue;
            }

            files.push(path);
        }

        Ok(files)
    }

    /// Parse a simple newline-separated list of files.
    fn parse_simple_file_list(&self, output: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let path = PathBuf::from(line);

            // Apply extension filter
            if !self.extensions.is_empty() && !self.matches_extension(&path) {
                continue;
            }

            // Apply glob filter
            if !self.glob_patterns.is_empty() && !self.matches_glob(&path) {
                continue;
            }

            files.push(path);
        }

        Ok(files)
    }

    /// Check if a path matches any of the configured extensions.
    fn matches_extension(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy();
            self.extensions.iter().any(|e| e == ext_str.as_ref())
        } else {
            false
        }
    }

    /// Check if a path matches any of the configured glob patterns.
    fn matches_glob(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        for pattern in &self.glob_patterns {
            if let Ok(glob) = Glob::new(pattern) {
                let matcher = glob.compile_matcher();
                if matcher.is_match(path_str.as_ref()) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestFixture;

    // =========================================================================
    // Basic Construction Tests
    // =========================================================================

    #[test]
    fn test_change_detector_new() {
        let detector = ChangeDetector::new(".");
        assert_eq!(detector.repo_path, PathBuf::from("."));
        assert!(detector.extensions.is_empty());
        assert!(detector.glob_patterns.is_empty());
    }

    #[test]
    fn test_change_detector_with_extensions() {
        let detector = ChangeDetector::new(".").with_extensions(&["rs", "toml"]);
        assert_eq!(detector.extensions, vec!["rs", "toml"]);
    }

    #[test]
    fn test_change_detector_with_glob_patterns() {
        let detector = ChangeDetector::new(".").with_glob_patterns(&["src/**/*.rs", "tests/*.rs"]);
        assert_eq!(detector.glob_patterns, vec!["src/**/*.rs", "tests/*.rs"]);
    }

    #[test]
    fn test_change_detector_builder_chaining() {
        let detector = ChangeDetector::new(".")
            .with_extensions(&["rs"])
            .with_glob_patterns(&["src/**/*"]);
        assert_eq!(detector.extensions, vec!["rs"]);
        assert_eq!(detector.glob_patterns, vec!["src/**/*"]);
    }

    // =========================================================================
    // Extension Filtering Tests
    // =========================================================================

    #[test]
    fn test_matches_extension_with_rs() {
        let detector = ChangeDetector::new(".").with_extensions(&["rs"]);
        assert!(detector.matches_extension(Path::new("src/main.rs")));
        assert!(!detector.matches_extension(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_matches_extension_multiple() {
        let detector = ChangeDetector::new(".").with_extensions(&["rs", "toml"]);
        assert!(detector.matches_extension(Path::new("src/main.rs")));
        assert!(detector.matches_extension(Path::new("Cargo.toml")));
        assert!(!detector.matches_extension(Path::new("README.md")));
    }

    #[test]
    fn test_matches_extension_no_extension() {
        let detector = ChangeDetector::new(".").with_extensions(&["rs"]);
        assert!(!detector.matches_extension(Path::new("Makefile")));
    }

    // =========================================================================
    // Glob Pattern Tests
    // =========================================================================

    #[test]
    fn test_matches_glob_simple() {
        let detector = ChangeDetector::new(".").with_glob_patterns(&["*.rs"]);
        assert!(detector.matches_glob(Path::new("main.rs")));
        assert!(!detector.matches_glob(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_matches_glob_recursive() {
        let detector = ChangeDetector::new(".").with_glob_patterns(&["src/**/*.rs"]);
        assert!(detector.matches_glob(Path::new("src/main.rs")));
        assert!(detector.matches_glob(Path::new("src/module/nested.rs")));
        assert!(!detector.matches_glob(Path::new("tests/test.rs")));
    }

    // =========================================================================
    // Parse Output Tests
    // =========================================================================

    #[test]
    fn test_parse_name_status_modified() {
        let detector = ChangeDetector::new(".");
        let output = "M\tsrc/main.rs\nM\tsrc/lib.rs\n";
        let files = detector.parse_name_status_output(output).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&PathBuf::from("src/main.rs")));
        assert!(files.contains(&PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn test_parse_name_status_added() {
        let detector = ChangeDetector::new(".");
        let output = "A\tnew_file.rs\n";
        let files = detector.parse_name_status_output(output).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], PathBuf::from("new_file.rs"));
    }

    #[test]
    fn test_parse_name_status_renamed() {
        let detector = ChangeDetector::new(".");
        // Rename format: R<score>\told_path\tnew_path
        let output = "R100\told_name.rs\tnew_name.rs\n";
        let files = detector.parse_name_status_output(output).unwrap();
        assert_eq!(files.len(), 1);
        // Should return the NEW path, not the old one
        assert_eq!(files[0], PathBuf::from("new_name.rs"));
    }

    #[test]
    fn test_parse_name_status_with_extension_filter() {
        let detector = ChangeDetector::new(".").with_extensions(&["rs"]);
        let output = "M\tsrc/main.rs\nM\tCargo.toml\n";
        let files = detector.parse_name_status_output(output).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_parse_name_status_empty() {
        let detector = ChangeDetector::new(".");
        let output = "";
        let files = detector.parse_name_status_output(output).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_simple_file_list() {
        let detector = ChangeDetector::new(".");
        let output = "file1.rs\nfile2.rs\nfile3.txt\n";
        let files = detector.parse_simple_file_list(output).unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_parse_simple_file_list_with_filter() {
        let detector = ChangeDetector::new(".").with_extensions(&["rs"]);
        let output = "file1.rs\nfile2.rs\nfile3.txt\n";
        let files = detector.parse_simple_file_list(output).unwrap();
        assert_eq!(files.len(), 2);
        assert!(!files.contains(&PathBuf::from("file3.txt")));
    }

    // =========================================================================
    // Integration Tests (require git repo)
    // =========================================================================

    #[test]
    fn test_change_detector_finds_modified_files() {
        let fixture = TestFixture::with_git_repo();

        // Modify an existing file
        fixture
            .write_file("PROMPT_build.md", "# Modified content")
            .unwrap();

        let detector = ChangeDetector::new(fixture.path());
        let changes = detector.changed_in_working_tree().unwrap();

        assert!(
            changes.contains(&PathBuf::from("PROMPT_build.md")),
            "Should detect modified file. Found: {:?}",
            changes
        );
    }

    #[test]
    fn test_change_detector_finds_added_files() {
        let fixture = TestFixture::with_git_repo();

        // Add a new file
        fixture.write_file("new_file.rs", "fn main() {}").unwrap();

        let detector = ChangeDetector::new(fixture.path());
        let changes = detector.changed_in_working_tree().unwrap();

        assert!(
            changes.contains(&PathBuf::from("new_file.rs")),
            "Should detect added file. Found: {:?}",
            changes
        );
    }

    #[test]
    fn test_change_detector_handles_renames() {
        let fixture = TestFixture::with_git_repo();

        // Create and commit a file
        fixture.write_file("old_name.rs", "fn foo() {}").unwrap();
        fixture.make_commit("Add old_name.rs");

        let old_hash = fixture.get_commit_hash();

        // Rename the file using git mv
        std::process::Command::new("git")
            .args(["mv", "old_name.rs", "new_name.rs"])
            .current_dir(fixture.path())
            .output()
            .expect("Failed to git mv");

        fixture.make_commit("Rename file");

        let detector = ChangeDetector::new(fixture.path());
        let changes = detector.changed_since(&old_hash).unwrap();

        // Should include the new name, not the old name
        assert!(
            changes.contains(&PathBuf::from("new_name.rs")),
            "Should detect renamed file with new name. Found: {:?}",
            changes
        );
    }

    #[test]
    fn test_change_detector_filters_by_extension() {
        let fixture = TestFixture::with_git_repo();

        // Add files with different extensions
        fixture.write_file("code.rs", "fn main() {}").unwrap();
        fixture
            .write_file("config.toml", "key = \"value\"")
            .unwrap();
        fixture.write_file("readme.md", "# Title").unwrap();

        let detector = ChangeDetector::new(fixture.path()).with_extensions(&["rs"]);
        let changes = detector.changed_in_working_tree().unwrap();

        assert!(
            changes.contains(&PathBuf::from("code.rs")),
            "Should include .rs files"
        );
        assert!(
            !changes.contains(&PathBuf::from("config.toml")),
            "Should exclude .toml files"
        );
        assert!(
            !changes.contains(&PathBuf::from("readme.md")),
            "Should exclude .md files"
        );
    }

    #[test]
    fn test_change_detector_changed_since_commit() {
        let fixture = TestFixture::with_git_repo();

        let old_hash = fixture.get_commit_hash();

        // Make changes after the commit
        fixture
            .write_file("new_after_commit.rs", "fn new() {}")
            .unwrap();
        fixture.make_commit("Add new file");

        let detector = ChangeDetector::new(fixture.path());
        let changes = detector.changed_since(&old_hash).unwrap();

        assert!(
            changes.contains(&PathBuf::from("new_after_commit.rs")),
            "Should detect file added after commit. Found: {:?}",
            changes
        );
    }

    #[test]
    fn test_change_detector_not_a_git_repo() {
        let temp = tempfile::TempDir::new().unwrap();

        let detector = ChangeDetector::new(temp.path());
        let result = detector.changed_in_working_tree();

        assert!(result.is_err(), "Should error on non-git directory");
    }

    #[test]
    fn test_change_detector_no_changes() {
        let fixture = TestFixture::with_git_repo();

        // No modifications after initial commit
        let detector = ChangeDetector::new(fixture.path());
        let changes = detector.changed_in_working_tree().unwrap();

        assert!(
            changes.is_empty(),
            "Should have no changes. Found: {:?}",
            changes
        );
    }

    #[test]
    fn test_change_detector_staged_changes() {
        let fixture = TestFixture::with_git_repo();

        // Add a new file and stage it
        fixture
            .write_file("staged_file.rs", "fn staged() {}")
            .unwrap();

        std::process::Command::new("git")
            .args(["add", "staged_file.rs"])
            .current_dir(fixture.path())
            .output()
            .expect("Failed to stage file");

        let detector = ChangeDetector::new(fixture.path());
        let changes = detector.changed_in_working_tree().unwrap();

        assert!(
            changes.contains(&PathBuf::from("staged_file.rs")),
            "Should detect staged file. Found: {:?}",
            changes
        );
    }

    #[test]
    fn test_change_detector_filters_by_multiple_extensions() {
        let fixture = TestFixture::with_git_repo();

        fixture.write_file("code.rs", "fn main() {}").unwrap();
        fixture
            .write_file("config.toml", "key = \"value\"")
            .unwrap();
        fixture.write_file("readme.md", "# Title").unwrap();

        let detector = ChangeDetector::new(fixture.path()).with_extensions(&["rs", "toml"]);
        let changes = detector.changed_in_working_tree().unwrap();

        assert!(changes.contains(&PathBuf::from("code.rs")));
        assert!(changes.contains(&PathBuf::from("config.toml")));
        assert!(!changes.contains(&PathBuf::from("readme.md")));
    }

    // =========================================================================
    // ChangeScope Tests (Sprint 26.3)
    // =========================================================================

    #[test]
    fn test_change_scope_new_empty() {
        let scope = ChangeScope::new();
        assert!(scope.changed_files().is_empty());
        assert!(scope.commit_ref().is_none());
        assert!(!scope.has_changes());
    }

    #[test]
    fn test_change_scope_with_files() {
        let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
        let scope = ChangeScope::from_files(files.clone());

        assert_eq!(scope.changed_files().len(), 2);
        assert!(scope.has_changes());
        assert!(scope.commit_ref().is_none());
    }

    #[test]
    fn test_change_scope_with_commit_ref() {
        let scope = ChangeScope::new().with_commit_ref("HEAD~5");
        assert_eq!(scope.commit_ref(), Some("HEAD~5"));
    }

    #[test]
    fn test_change_scope_from_detector() {
        let fixture = TestFixture::with_git_repo();

        // Add a new file
        fixture.write_file("new_file.rs", "fn new() {}").unwrap();

        let detector = ChangeDetector::new(fixture.path());
        let scope = ChangeScope::from_detector(&detector).unwrap();

        assert!(scope.has_changes());
        assert!(
            scope
                .changed_files()
                .contains(&PathBuf::from("new_file.rs")),
            "Scope should contain the changed file"
        );
    }

    #[test]
    fn test_change_scope_from_detector_with_commit() {
        let fixture = TestFixture::with_git_repo();
        let old_hash = fixture.get_commit_hash();

        // Make changes after the commit
        fixture
            .write_file("after_commit.rs", "fn after() {}")
            .unwrap();
        fixture.make_commit("Add after_commit.rs");

        let detector = ChangeDetector::new(fixture.path());
        let scope = ChangeScope::from_detector_since(&detector, &old_hash).unwrap();

        assert!(scope.has_changes());
        assert_eq!(scope.commit_ref(), Some(old_hash.as_str()));
        assert!(scope
            .changed_files()
            .contains(&PathBuf::from("after_commit.rs")));
    }

    #[test]
    fn test_change_scope_filter_by_extension() {
        let files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("Cargo.toml"),
            PathBuf::from("README.md"),
        ];
        let scope = ChangeScope::from_files(files);
        let rust_only = scope.filter_by_extensions(&["rs"]);

        assert_eq!(rust_only.changed_files().len(), 1);
        assert!(rust_only
            .changed_files()
            .contains(&PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_change_scope_extract_function_names() {
        // Changed files should allow extracting function names for CCG lookup
        let files = vec![PathBuf::from("src/handler.rs")];
        let scope = ChangeScope::from_files(files);

        // This would be used by the intelligence builder to query call graphs
        let files_for_query: Vec<&str> = scope
            .changed_files()
            .iter()
            .filter_map(|p| p.to_str())
            .collect();

        assert_eq!(files_for_query, vec!["src/handler.rs"]);
    }

    #[test]
    fn test_change_scope_merge() {
        let scope1 = ChangeScope::from_files(vec![PathBuf::from("a.rs")]);
        let scope2 = ChangeScope::from_files(vec![PathBuf::from("b.rs")]);
        let merged = scope1.merge(&scope2);

        assert_eq!(merged.changed_files().len(), 2);
        assert!(merged.changed_files().contains(&PathBuf::from("a.rs")));
        assert!(merged.changed_files().contains(&PathBuf::from("b.rs")));
    }

    #[test]
    fn test_change_scope_merge_deduplicates() {
        let scope1 = ChangeScope::from_files(vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]);
        let scope2 = ChangeScope::from_files(vec![PathBuf::from("b.rs"), PathBuf::from("c.rs")]);
        let merged = scope1.merge(&scope2);

        assert_eq!(merged.changed_files().len(), 3);
    }

    #[test]
    fn test_change_scope_is_file_in_scope() {
        let scope = ChangeScope::from_files(vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/lib.rs"),
        ]);

        assert!(scope.is_file_in_scope(&PathBuf::from("src/main.rs")));
        assert!(!scope.is_file_in_scope(&PathBuf::from("src/other.rs")));
    }
}
