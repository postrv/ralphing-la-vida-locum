//! Context window language prioritization for polyglot projects.
//!
//! This module provides intelligent file prioritization for context inclusion
//! based on detected languages and file change status. Files are scored and
//! sorted to maximize relevant content within token limits.
//!
//! # Priority Levels
//!
//! Files are prioritized in the following order:
//! 1. **Changed files** - Files modified in the working tree (highest priority)
//! 2. **Primary language** - Files in the project's primary language
//! 3. **Secondary languages** - Files in other detected languages
//! 4. **Config files** - Relevant configuration files (Cargo.toml, package.json, etc.)
//! 5. **Other files** - Remaining files (lowest priority)
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::prompt::context_priority::{ContextPrioritizer, ContextPriorityConfig};
//! use ralph::Language;
//! use std::path::PathBuf;
//!
//! let config = ContextPriorityConfig::default();
//! let prioritizer = ContextPrioritizer::new(config);
//!
//! let files = vec![
//!     PathBuf::from("src/main.rs"),
//!     PathBuf::from("src/lib.py"),
//!     PathBuf::from("README.md"),
//! ];
//!
//! let changed_files = vec![PathBuf::from("src/main.rs")];
//! let languages = vec![Language::Rust, Language::Python];
//!
//! let prioritized = prioritizer.prioritize_by_language(
//!     files,
//!     &languages,
//!     &changed_files,
//!     Some(Language::Rust),
//! );
//! ```

use crate::Language;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for context window prioritization.
///
/// Controls how files are scored and selected for context inclusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPriorityConfig {
    /// Score multiplier for changed files.
    /// Default: 10.0
    #[serde(default = "default_changed_score")]
    pub changed_score: f64,

    /// Score multiplier for primary language files.
    /// Default: 5.0
    #[serde(default = "default_primary_language_score")]
    pub primary_language_score: f64,

    /// Score multiplier for secondary language files.
    /// Default: 3.0
    #[serde(default = "default_secondary_language_score")]
    pub secondary_language_score: f64,

    /// Score multiplier for config files.
    /// Default: 4.0
    #[serde(default = "default_config_score")]
    pub config_score: f64,

    /// Score multiplier for test files related to changed source files.
    /// Default: 6.0
    #[serde(default = "default_related_test_score")]
    pub related_test_score: f64,

    /// Base score for all files.
    /// Default: 1.0
    #[serde(default = "default_base_score")]
    pub base_score: f64,

    /// Whether to include test files when source files change.
    /// Default: true
    #[serde(default = "default_true")]
    pub include_related_tests: bool,

    /// Whether to include config files when relevant.
    /// Default: true
    #[serde(default = "default_true")]
    pub include_config_files: bool,
}

fn default_changed_score() -> f64 {
    10.0
}

fn default_primary_language_score() -> f64 {
    5.0
}

fn default_secondary_language_score() -> f64 {
    3.0
}

fn default_config_score() -> f64 {
    4.0
}

fn default_related_test_score() -> f64 {
    6.0
}

fn default_base_score() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

impl Default for ContextPriorityConfig {
    fn default() -> Self {
        Self {
            changed_score: default_changed_score(),
            primary_language_score: default_primary_language_score(),
            secondary_language_score: default_secondary_language_score(),
            config_score: default_config_score(),
            related_test_score: default_related_test_score(),
            base_score: default_base_score(),
            include_related_tests: true,
            include_config_files: true,
        }
    }
}

impl ContextPriorityConfig {
    /// Create a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the score for changed files.
    #[must_use]
    pub fn with_changed_score(mut self, score: f64) -> Self {
        self.changed_score = score;
        self
    }

    /// Set the score for primary language files.
    #[must_use]
    pub fn with_primary_language_score(mut self, score: f64) -> Self {
        self.primary_language_score = score;
        self
    }

    /// Set the score for secondary language files.
    #[must_use]
    pub fn with_secondary_language_score(mut self, score: f64) -> Self {
        self.secondary_language_score = score;
        self
    }

    /// Set whether to include related test files.
    #[must_use]
    pub fn with_include_related_tests(mut self, include: bool) -> Self {
        self.include_related_tests = include;
        self
    }

    /// Set whether to include config files.
    #[must_use]
    pub fn with_include_config_files(mut self, include: bool) -> Self {
        self.include_config_files = include;
        self
    }
}

// ============================================================================
// Scored File
// ============================================================================

/// A file with its computed priority score.
#[derive(Debug, Clone)]
pub struct ScoredFile {
    /// The file path.
    pub path: PathBuf,
    /// The computed priority score.
    pub score: f64,
    /// Reasons why this file received its score.
    pub reasons: Vec<String>,
}

impl ScoredFile {
    /// Create a new scored file.
    fn new(path: PathBuf, score: f64) -> Self {
        Self {
            path,
            score,
            reasons: Vec::new(),
        }
    }

    /// Add score and reason.
    fn add_score(&mut self, score: f64, reason: impl Into<String>) {
        self.score += score;
        self.reasons.push(reason.into());
    }
}

// ============================================================================
// Context Prioritizer
// ============================================================================

/// Prioritizes files for context inclusion based on language and change status.
pub struct ContextPrioritizer {
    config: ContextPriorityConfig,
}

impl ContextPrioritizer {
    /// Create a new prioritizer with the given configuration.
    #[must_use]
    pub fn new(config: ContextPriorityConfig) -> Self {
        Self { config }
    }

    /// Create a prioritizer with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ContextPriorityConfig::default())
    }

    /// Prioritize files by language and change status.
    ///
    /// Returns files sorted by priority score (highest first).
    ///
    /// # Arguments
    ///
    /// * `files` - All available files to prioritize
    /// * `languages` - Detected languages in the project
    /// * `changed_files` - Files that have been modified
    /// * `primary_language` - The primary language of the project (if detected)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::prompt::context_priority::{ContextPrioritizer, ContextPriorityConfig};
    /// use ralph::Language;
    /// use std::path::PathBuf;
    ///
    /// let prioritizer = ContextPrioritizer::with_defaults();
    /// let files = vec![PathBuf::from("src/main.rs")];
    /// let changed = vec![PathBuf::from("src/main.rs")];
    /// let prioritized = prioritizer.prioritize_by_language(
    ///     files,
    ///     &[Language::Rust],
    ///     &changed,
    ///     Some(Language::Rust),
    /// );
    /// ```
    #[must_use]
    pub fn prioritize_by_language(
        &self,
        files: Vec<PathBuf>,
        languages: &[Language],
        changed_files: &[PathBuf],
        primary_language: Option<Language>,
    ) -> Vec<PathBuf> {
        let changed_set: HashSet<_> = changed_files.iter().collect();
        let secondary_languages: HashSet<_> = languages
            .iter()
            .filter(|&&l| Some(l) != primary_language)
            .collect();

        // Score each file
        let mut scored_files: Vec<ScoredFile> = files
            .into_iter()
            .map(|path| {
                self.score_file(&path, &changed_set, primary_language, &secondary_languages)
            })
            .collect();

        // Sort by score (descending)
        scored_files.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Extract paths
        scored_files.into_iter().map(|sf| sf.path).collect()
    }

    /// Prioritize files and return with scores for debugging/analysis.
    ///
    /// Same as `prioritize_by_language` but returns `ScoredFile` with reasons.
    #[must_use]
    pub fn prioritize_with_scores(
        &self,
        files: Vec<PathBuf>,
        languages: &[Language],
        changed_files: &[PathBuf],
        primary_language: Option<Language>,
    ) -> Vec<ScoredFile> {
        let changed_set: HashSet<_> = changed_files.iter().collect();
        let secondary_languages: HashSet<_> = languages
            .iter()
            .filter(|&&l| Some(l) != primary_language)
            .collect();

        // Score each file
        let mut scored_files: Vec<ScoredFile> = files
            .into_iter()
            .map(|path| {
                self.score_file(&path, &changed_set, primary_language, &secondary_languages)
            })
            .collect();

        // Sort by score (descending)
        scored_files.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored_files
    }

    /// Score a single file based on its characteristics.
    fn score_file(
        &self,
        path: &Path,
        changed_files: &HashSet<&PathBuf>,
        primary_language: Option<Language>,
        secondary_languages: &HashSet<&Language>,
    ) -> ScoredFile {
        let path_buf = path.to_path_buf();
        let mut scored = ScoredFile::new(path_buf.clone(), self.config.base_score);
        scored.reasons.push("base score".to_string());

        // Check if file is changed (highest priority)
        if changed_files.contains(&path_buf) {
            scored.add_score(self.config.changed_score, "changed file");
        }

        // Check language of the file
        if let Some(file_lang) = Language::from_path(path) {
            if Some(file_lang) == primary_language {
                scored.add_score(self.config.primary_language_score, "primary language");
            } else if secondary_languages.contains(&file_lang) {
                scored.add_score(self.config.secondary_language_score, "secondary language");
            }
        }

        // Check if it's a config file
        if self.config.include_config_files && self.is_config_file(path) {
            scored.add_score(self.config.config_score, "config file");
        }

        // Check if it's a test file related to changed source files
        if self.config.include_related_tests && self.is_related_test(path, changed_files) {
            scored.add_score(self.config.related_test_score, "related test file");
        }

        scored
    }

    /// Check if a file is a configuration file.
    fn is_config_file(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check common config file names
        matches!(
            file_name,
            "Cargo.toml"
                | "package.json"
                | "pyproject.toml"
                | "go.mod"
                | "tsconfig.json"
                | "setup.py"
                | "requirements.txt"
                | "Pipfile"
                | "build.gradle"
                | "pom.xml"
                | "Gemfile"
                | "composer.json"
        ) || file_name.ends_with(".csproj")
            || file_name.ends_with(".sln")
    }

    /// Check if a file is a test file related to any changed source file.
    fn is_related_test(&self, path: &Path, changed_files: &HashSet<&PathBuf>) -> bool {
        // Check if this is a test file
        if !self.is_test_file(path) {
            return false;
        }

        // Get the base name without test prefix/suffix
        let test_base = self.extract_test_base_name(path);
        if test_base.is_empty() {
            return false;
        }

        // Check if any changed file matches this test's base name
        for changed in changed_files {
            if let Some(changed_stem) = changed.file_stem().and_then(|s| s.to_str()) {
                // Remove common suffixes/prefixes from source file name
                let source_base = changed_stem
                    .trim_end_matches(".rs")
                    .trim_end_matches(".py")
                    .trim_end_matches(".ts")
                    .trim_end_matches(".js")
                    .trim_end_matches(".go");

                if test_base.contains(source_base) || source_base.contains(&test_base) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a file is a test file.
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check common test directories (with or without leading slash)
        if path_str.contains("/tests/")
            || path_str.starts_with("tests/")
            || path_str.contains("/test/")
            || path_str.starts_with("test/")
            || path_str.contains("/__tests__/")
            || path_str.starts_with("__tests__/")
            || path_str.contains("/spec/")
            || path_str.starts_with("spec/")
        {
            return true;
        }

        // Check common test file patterns
        file_name.starts_with("test_")
            || file_name.ends_with("_test.rs")
            || file_name.ends_with("_test.py")
            || file_name.ends_with("_test.go")
            || file_name.ends_with("_test.ts")
            || file_name.ends_with("_test.js")
            || file_name.ends_with(".test.ts")
            || file_name.ends_with(".test.js")
            || file_name.ends_with(".spec.ts")
            || file_name.ends_with(".spec.js")
    }

    /// Extract the base name from a test file name.
    fn extract_test_base_name(&self, path: &Path) -> String {
        let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // Remove common test prefixes/suffixes
        file_name
            .trim_start_matches("test_")
            .trim_end_matches("_test")
            .trim_end_matches(".test")
            .trim_end_matches(".spec")
            .to_string()
    }
}

impl Default for ContextPrioritizer {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Convenience function to prioritize files with default configuration.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::prompt::context_priority::prioritize_by_language;
/// use ralph::Language;
/// use std::path::PathBuf;
///
/// let files = vec![PathBuf::from("src/main.rs")];
/// let changed = vec![PathBuf::from("src/main.rs")];
/// let prioritized = prioritize_by_language(
///     files,
///     &[Language::Rust],
///     &changed,
///     Some(Language::Rust),
/// );
/// ```
#[must_use]
pub fn prioritize_by_language(
    files: Vec<PathBuf>,
    languages: &[Language],
    changed_files: &[PathBuf],
    primary_language: Option<Language>,
) -> Vec<PathBuf> {
    let prioritizer = ContextPrioritizer::with_defaults();
    prioritizer.prioritize_by_language(files, languages, changed_files, primary_language)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Test: Changed files are always included in context
    // =========================================================================

    #[test]
    fn test_changed_files_always_included() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/utils.rs"),
        ];
        let changed = vec![PathBuf::from("src/main.rs")];

        let result = prioritizer.prioritize_by_language(
            files,
            &[Language::Rust],
            &changed,
            Some(Language::Rust),
        );

        // Changed file should be first
        assert_eq!(result[0], PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_multiple_changed_files_all_high_priority() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/a.rs"),
            PathBuf::from("src/b.rs"),
            PathBuf::from("src/c.rs"),
            PathBuf::from("src/unchanged.rs"),
        ];
        let changed = vec![PathBuf::from("src/b.rs"), PathBuf::from("src/c.rs")];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust],
            &changed,
            Some(Language::Rust),
        );

        // Both changed files should have higher scores than unchanged
        let b_score = scored.iter().find(|s| s.path.ends_with("b.rs")).unwrap();
        let c_score = scored.iter().find(|s| s.path.ends_with("c.rs")).unwrap();
        let unchanged_score = scored
            .iter()
            .find(|s| s.path.ends_with("unchanged.rs"))
            .unwrap();

        assert!(b_score.score > unchanged_score.score);
        assert!(c_score.score > unchanged_score.score);
    }

    // =========================================================================
    // Test: Primary language files are prioritized over secondary
    // =========================================================================

    #[test]
    fn test_primary_language_prioritized_over_secondary() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/main.py"),  // secondary
            PathBuf::from("src/main.rs"),  // primary
            PathBuf::from("src/utils.go"), // not in languages
        ];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust, Language::Python],
            &[],
            Some(Language::Rust),
        );

        let rust_score = scored.iter().find(|s| s.path.ends_with("main.rs")).unwrap();
        let python_score = scored.iter().find(|s| s.path.ends_with("main.py")).unwrap();
        let go_score = scored
            .iter()
            .find(|s| s.path.ends_with("utils.go"))
            .unwrap();

        // Rust (primary) > Python (secondary) > Go (neither)
        assert!(
            rust_score.score > python_score.score,
            "Primary language should score higher than secondary"
        );
        assert!(
            python_score.score > go_score.score,
            "Secondary language should score higher than unrecognized"
        );
    }

    #[test]
    fn test_secondary_languages_prioritized_over_other() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/script.py"), // secondary
            PathBuf::from("src/readme.md"), // other
        ];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust, Language::Python],
            &[],
            Some(Language::Rust),
        );

        let python_score = scored
            .iter()
            .find(|s| s.path.ends_with("script.py"))
            .unwrap();
        let md_score = scored
            .iter()
            .find(|s| s.path.ends_with("readme.md"))
            .unwrap();

        assert!(
            python_score.score > md_score.score,
            "Secondary language should score higher than other files"
        );
    }

    // =========================================================================
    // Test: Context respects token limits while maximizing relevant content
    // =========================================================================

    #[test]
    fn test_prioritization_returns_sorted_by_score() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("low.txt"),
            PathBuf::from("src/main.rs"), // primary language
            PathBuf::from("changed.rs"),  // will be marked as changed
            PathBuf::from("Cargo.toml"),  // config file
        ];
        let changed = vec![PathBuf::from("changed.rs")];

        let result = prioritizer.prioritize_by_language(
            files,
            &[Language::Rust],
            &changed,
            Some(Language::Rust),
        );

        // Changed file should be first, then primary language/config, then other
        assert_eq!(
            result[0],
            PathBuf::from("changed.rs"),
            "Changed file should be first"
        );

        // low.txt should be last (lowest score)
        assert_eq!(
            result[result.len() - 1],
            PathBuf::from("low.txt"),
            "Other file should be last"
        );
    }

    #[test]
    fn test_scores_are_cumulative() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/main.rs"), // primary + changed
            PathBuf::from("Cargo.toml"),  // config only
        ];
        let changed = vec![PathBuf::from("src/main.rs")];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust],
            &changed,
            Some(Language::Rust),
        );

        let main_scored = scored.iter().find(|s| s.path.ends_with("main.rs")).unwrap();

        // Should have both "changed file" and "primary language" reasons
        assert!(
            main_scored.reasons.iter().any(|r| r.contains("changed")),
            "Should include 'changed file' reason"
        );
        assert!(
            main_scored.reasons.iter().any(|r| r.contains("primary")),
            "Should include 'primary language' reason"
        );
    }

    // =========================================================================
    // Test: Test files are included when related source files change
    // =========================================================================

    #[test]
    fn test_related_test_files_included() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/parser.rs"),
            PathBuf::from("tests/parser_test.rs"),
            PathBuf::from("tests/other_test.rs"),
        ];
        let changed = vec![PathBuf::from("src/parser.rs")];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust],
            &changed,
            Some(Language::Rust),
        );

        let parser_test = scored
            .iter()
            .find(|s| s.path.ends_with("parser_test.rs"))
            .unwrap();
        let other_test = scored
            .iter()
            .find(|s| s.path.ends_with("other_test.rs"))
            .unwrap();

        assert!(
            parser_test.score > other_test.score,
            "Related test file should have higher score"
        );
        assert!(
            parser_test
                .reasons
                .iter()
                .any(|r| r.contains("related test")),
            "Should include 'related test file' reason"
        );
    }

    #[test]
    fn test_test_file_patterns_recognized() {
        let prioritizer = ContextPrioritizer::with_defaults();

        // Various test file patterns
        let test_patterns = vec![
            "test_foo.py",
            "foo_test.rs",
            "foo_test.go",
            "foo.test.ts",
            "foo.test.js",
            "foo.spec.ts",
            "tests/foo.rs",
            "__tests__/foo.js",
        ];

        for pattern in test_patterns {
            let path = PathBuf::from(pattern);
            assert!(
                prioritizer.is_test_file(&path),
                "Should recognize {} as test file",
                pattern
            );
        }
    }

    #[test]
    fn test_non_test_files_not_recognized() {
        let prioritizer = ContextPrioritizer::with_defaults();

        let non_test_patterns = vec![
            "src/main.rs",
            "src/lib.py",
            "contest.js", // contains 'test' but not a test file
            "latest.go",  // ends with 'test' in name but not a test file
        ];

        for pattern in non_test_patterns {
            let path = PathBuf::from(pattern);
            assert!(
                !prioritizer.is_test_file(&path),
                "Should not recognize {} as test file",
                pattern
            );
        }
    }

    #[test]
    fn test_related_tests_can_be_disabled() {
        let config = ContextPriorityConfig::default().with_include_related_tests(false);
        let prioritizer = ContextPrioritizer::new(config);

        let files = vec![
            PathBuf::from("src/parser.rs"),
            PathBuf::from("tests/parser_test.rs"),
        ];
        let changed = vec![PathBuf::from("src/parser.rs")];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust],
            &changed,
            Some(Language::Rust),
        );

        let parser_test = scored
            .iter()
            .find(|s| s.path.ends_with("parser_test.rs"))
            .unwrap();

        assert!(
            !parser_test
                .reasons
                .iter()
                .any(|r| r.contains("related test")),
            "Should not include 'related test file' reason when disabled"
        );
    }

    // =========================================================================
    // Test: Config files (Cargo.toml, package.json) included when relevant
    // =========================================================================

    #[test]
    fn test_config_files_prioritized() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("random.txt")];

        let scored =
            prioritizer.prioritize_with_scores(files, &[Language::Rust], &[], Some(Language::Rust));

        let cargo_score = scored
            .iter()
            .find(|s| s.path.ends_with("Cargo.toml"))
            .unwrap();
        let txt_score = scored
            .iter()
            .find(|s| s.path.ends_with("random.txt"))
            .unwrap();

        assert!(
            cargo_score.score > txt_score.score,
            "Config file should have higher score"
        );
        assert!(
            cargo_score.reasons.iter().any(|r| r.contains("config")),
            "Should include 'config file' reason"
        );
    }

    #[test]
    fn test_various_config_files_recognized() {
        let prioritizer = ContextPrioritizer::with_defaults();

        let config_files = vec![
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "tsconfig.json",
            "setup.py",
            "requirements.txt",
            "build.gradle",
            "pom.xml",
            "Gemfile",
            "composer.json",
            "MyProject.csproj",
            "Solution.sln",
        ];

        for config in config_files {
            let path = PathBuf::from(config);
            assert!(
                prioritizer.is_config_file(&path),
                "Should recognize {} as config file",
                config
            );
        }
    }

    #[test]
    fn test_config_files_can_be_disabled() {
        let config = ContextPriorityConfig::default().with_include_config_files(false);
        let prioritizer = ContextPrioritizer::new(config);

        let files = vec![PathBuf::from("Cargo.toml")];

        let scored =
            prioritizer.prioritize_with_scores(files, &[Language::Rust], &[], Some(Language::Rust));

        let cargo_score = scored
            .iter()
            .find(|s| s.path.ends_with("Cargo.toml"))
            .unwrap();

        assert!(
            !cargo_score.reasons.iter().any(|r| r.contains("config")),
            "Should not include 'config file' reason when disabled"
        );
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_config_default_values() {
        let config = ContextPriorityConfig::default();

        assert!((config.changed_score - 10.0).abs() < f64::EPSILON);
        assert!((config.primary_language_score - 5.0).abs() < f64::EPSILON);
        assert!((config.secondary_language_score - 3.0).abs() < f64::EPSILON);
        assert!((config.config_score - 4.0).abs() < f64::EPSILON);
        assert!((config.related_test_score - 6.0).abs() < f64::EPSILON);
        assert!((config.base_score - 1.0).abs() < f64::EPSILON);
        assert!(config.include_related_tests);
        assert!(config.include_config_files);
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = ContextPriorityConfig::new()
            .with_changed_score(20.0)
            .with_primary_language_score(10.0)
            .with_include_related_tests(false);

        assert!((config.changed_score - 20.0).abs() < f64::EPSILON);
        assert!((config.primary_language_score - 10.0).abs() < f64::EPSILON);
        assert!(!config.include_related_tests);
    }

    #[test]
    fn test_custom_scores_affect_ordering() {
        // Make secondary language score higher than primary
        let config = ContextPriorityConfig::new()
            .with_primary_language_score(2.0)
            .with_secondary_language_score(8.0);
        let prioritizer = ContextPrioritizer::new(config);

        let files = vec![
            PathBuf::from("src/main.rs"), // primary
            PathBuf::from("src/main.py"), // secondary
        ];

        let result = prioritizer.prioritize_by_language(
            files,
            &[Language::Rust, Language::Python],
            &[],
            Some(Language::Rust),
        );

        // Python (secondary) should now be first due to higher score
        assert_eq!(result[0], PathBuf::from("src/main.py"));
    }

    // =========================================================================
    // Convenience Function Tests
    // =========================================================================

    #[test]
    fn test_convenience_function() {
        let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
        let changed = vec![PathBuf::from("src/main.rs")];

        let result =
            prioritize_by_language(files, &[Language::Rust], &changed, Some(Language::Rust));

        assert_eq!(result[0], PathBuf::from("src/main.rs"));
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_empty_files_list() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let result = prioritizer.prioritize_by_language(
            vec![],
            &[Language::Rust],
            &[],
            Some(Language::Rust),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn test_no_languages_detected() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("README.md")];

        let result = prioritizer.prioritize_by_language(
            files,
            &[], // no languages
            &[],
            None, // no primary language
        );

        // Should still return files, just with base scores
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_no_primary_language() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/main.py")];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust, Language::Python],
            &[],
            None, // no primary - all are secondary
        );

        let rust_score = scored.iter().find(|s| s.path.ends_with("main.rs")).unwrap();
        let python_score = scored.iter().find(|s| s.path.ends_with("main.py")).unwrap();

        // Both should have secondary language scores (equal)
        assert!(
            (rust_score.score - python_score.score).abs() < f64::EPSILON,
            "Without primary, both should have equal secondary scores"
        );
    }

    #[test]
    fn test_file_not_in_any_language() {
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("data.csv"), // not a recognized language
        ];

        let scored =
            prioritizer.prioritize_with_scores(files, &[Language::Rust], &[], Some(Language::Rust));

        let csv_score = scored
            .iter()
            .find(|s| s.path.ends_with("data.csv"))
            .unwrap();

        // Should only have base score
        assert!(
            (csv_score.score - 1.0).abs() < f64::EPSILON,
            "Unrecognized file should only have base score"
        );
    }

    // =========================================================================
    // Serde Serialization Tests
    // =========================================================================

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = ContextPriorityConfig::new()
            .with_changed_score(15.0)
            .with_primary_language_score(7.0);

        let json = serde_json::to_string(&config).expect("serialization should work");
        let deserialized: ContextPriorityConfig =
            serde_json::from_str(&json).expect("deserialization should work");

        assert!((deserialized.changed_score - 15.0).abs() < f64::EPSILON);
        assert!((deserialized.primary_language_score - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_deserialization_with_defaults() {
        let json = r#"{"changed_score": 20.0}"#;
        let config: ContextPriorityConfig = serde_json::from_str(json).expect("should deserialize");

        assert!((config.changed_score - 20.0).abs() < f64::EPSILON);
        // Other fields should have defaults
        assert!((config.primary_language_score - 5.0).abs() < f64::EPSILON);
    }

    // =========================================================================
    // Tests matching Implementation Plan naming (language_prioritization)
    // =========================================================================

    #[test]
    fn test_language_prioritization_primary_over_secondary() {
        // Alias for test_primary_language_prioritized_over_secondary
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/app.py"),  // secondary
            PathBuf::from("src/main.rs"), // primary
        ];

        let result = prioritizer.prioritize_by_language(
            files,
            &[Language::Rust, Language::Python],
            &[],
            Some(Language::Rust),
        );

        assert_eq!(
            result[0],
            PathBuf::from("src/main.rs"),
            "Primary language should be prioritized"
        );
    }

    #[test]
    fn test_language_prioritization_changed_files_first() {
        // Validates changed files always come first
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/changed.py"),
            PathBuf::from("src/main.rs"),
        ];
        let changed = vec![PathBuf::from("src/changed.py")];

        let result = prioritizer.prioritize_by_language(
            files,
            &[Language::Rust, Language::Python],
            &changed,
            Some(Language::Rust),
        );

        assert_eq!(
            result[0],
            PathBuf::from("src/changed.py"),
            "Changed files should be prioritized first"
        );
    }

    #[test]
    fn test_language_prioritization_polyglot_project() {
        // Test prioritization in a polyglot project with multiple languages
        let prioritizer = ContextPrioritizer::with_defaults();
        let files = vec![
            PathBuf::from("src/main.rs"),  // primary (Rust)
            PathBuf::from("src/app.py"),   // secondary (Python)
            PathBuf::from("src/index.ts"), // secondary (TypeScript)
            PathBuf::from("README.md"),    // other
        ];

        let scored = prioritizer.prioritize_with_scores(
            files,
            &[Language::Rust, Language::Python, Language::TypeScript],
            &[],
            Some(Language::Rust),
        );

        let rust_score = scored.iter().find(|s| s.path.ends_with("main.rs")).unwrap();
        let python_score = scored.iter().find(|s| s.path.ends_with("app.py")).unwrap();
        let md_score = scored
            .iter()
            .find(|s| s.path.ends_with("README.md"))
            .unwrap();

        assert!(rust_score.score > python_score.score, "Primary > secondary");
        assert!(python_score.score > md_score.score, "Secondary > other");
    }
}
