//! Semantic progress tracking with multi-dimensional signals.
//!
//! This module provides fine-grained progress detection beyond simple commit counting.
//! It collects signals from git, quality gates, and behavioral patterns to determine
//! whether meaningful progress has been made.
//!
//! # Architecture
//!
//! ```text
//! ProgressTracker
//!   ├── GitSignalCollector   - commits, lines changed, files modified
//!   ├── QualitySignalCollector - test deltas, clippy deltas
//!   └── ProgressEvaluator    - weighted scoring of signals
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::r#loop::progress::{ProgressTracker, ProgressEvaluatorConfig};
//!
//! let tracker = ProgressTracker::new(project_dir, git_ops);
//! let signals = tracker.collect_signals(&last_state)?;
//! let evaluation = tracker.evaluate(&signals);
//!
//! match evaluation.verdict {
//!     ProgressVerdict::MeaningfulProgress => { /* reset stagnation */ }
//!     ProgressVerdict::PartialProgress => { /* track but don't reset */ }
//!     ProgressVerdict::NoProgress => { /* increment stagnation */ }
//!     ProgressVerdict::Exploration => { /* allow some exploration */ }
//! }
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

// ============================================================================
// Progress Signals
// ============================================================================

/// Multi-dimensional progress signals collected from various sources.
///
/// Each field category represents a different aspect of progress:
/// - Git-level: Repository state changes
/// - File-level: What types of files changed
/// - Quality-level: Test and lint improvements
/// - Behavioral-level: Working patterns
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgressSignals {
    // ===== Git-level signals =====
    /// Number of new commits since last check.
    pub commits_added: u32,
    /// Number of lines changed (added + removed).
    pub lines_changed: u32,
    /// Files modified in git status.
    pub files_modified_count: u32,

    // ===== File-level signals =====
    /// Number of source files (*.rs) modified.
    pub source_files_modified: u32,
    /// Number of test files modified.
    pub test_files_modified: u32,
    /// Number of documentation files modified.
    pub doc_files_modified: u32,
    /// Number of configuration files modified.
    pub config_files_modified: u32,

    // ===== Quality-level signals =====
    /// Number of tests added since last check.
    pub tests_added: i32,
    /// Change in test pass count (can be negative).
    pub test_pass_delta: i32,
    /// Change in clippy warning count (negative is good).
    pub clippy_warnings_delta: i32,

    // ===== Behavioral signals =====
    /// Unique files touched this iteration.
    pub unique_file_touches: HashSet<PathBuf>,
    /// Number of times the same file was edited without committing.
    pub repeated_edit_count: u32,
    /// Breadth of exploration (different directories touched).
    pub exploration_breadth: u32,
}

impl ProgressSignals {
    /// Create a new empty signals instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any positive signals.
    #[must_use]
    pub fn has_any_positive_signal(&self) -> bool {
        self.commits_added > 0
            || self.lines_changed > 10
            || self.tests_added > 0
            || self.clippy_warnings_delta < 0
    }

    /// Get a summary string of the signals.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if self.commits_added > 0 {
            parts.push(format!("{} commits", self.commits_added));
        }
        if self.lines_changed > 0 {
            parts.push(format!("{} lines", self.lines_changed));
        }
        if self.source_files_modified > 0 {
            parts.push(format!("{} src files", self.source_files_modified));
        }
        if self.test_files_modified > 0 {
            parts.push(format!("{} test files", self.test_files_modified));
        }
        if self.tests_added != 0 {
            let sign = if self.tests_added > 0 { "+" } else { "" };
            parts.push(format!("{}{}  tests", sign, self.tests_added));
        }
        if self.clippy_warnings_delta != 0 {
            let sign = if self.clippy_warnings_delta > 0 { "+" } else { "" };
            parts.push(format!("{}{} warnings", sign, self.clippy_warnings_delta));
        }

        if parts.is_empty() {
            "no changes".to_string()
        } else {
            parts.join(", ")
        }
    }
}

// ============================================================================
// File Categorization
// ============================================================================

/// Categories of files for progress tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    /// Source code (*.rs in src/)
    Source,
    /// Test code (*.rs in tests/ or contains #[cfg(test)])
    Test,
    /// Documentation (*.md, *.txt)
    Documentation,
    /// Configuration (*.toml, *.json, *.yaml)
    Configuration,
    /// Other files
    Other,
}

impl FileCategory {
    /// Categorize a file path.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let path_str = path.to_string_lossy();
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Check if it's a test file
        if path_str.contains("/tests/")
            || path_str.starts_with("tests/")
            || path_str.contains("_test.rs")
        {
            return Self::Test;
        }

        // Check by extension
        match extension {
            "rs" => {
                // Check for source files (handle both /src/ and src/ at start)
                if path_str.contains("/src/") || path_str.starts_with("src/") {
                    Self::Source
                } else {
                    Self::Other
                }
            }
            "md" | "txt" | "rst" => Self::Documentation,
            "toml" | "json" | "yaml" | "yml" => Self::Configuration,
            _ => Self::Other,
        }
    }
}

// ============================================================================
// Progress Verdict
// ============================================================================

/// Result of progress evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressVerdict {
    /// Meaningful progress was made - reset stagnation counter.
    MeaningfulProgress,
    /// Some progress but not enough to reset stagnation.
    PartialProgress,
    /// No progress detected - increment stagnation.
    NoProgress,
    /// Exploratory activity detected - allow some slack.
    Exploration,
    /// Quality regression detected - may need intervention.
    Regression,
}

impl ProgressVerdict {
    /// Check if this verdict should reset the stagnation counter.
    #[must_use]
    pub fn should_reset_stagnation(&self) -> bool {
        matches!(self, Self::MeaningfulProgress)
    }

    /// Check if this verdict indicates healthy activity.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(
            self,
            Self::MeaningfulProgress | Self::PartialProgress | Self::Exploration
        )
    }
}

impl std::fmt::Display for ProgressVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MeaningfulProgress => write!(f, "meaningful progress"),
            Self::PartialProgress => write!(f, "partial progress"),
            Self::NoProgress => write!(f, "no progress"),
            Self::Exploration => write!(f, "exploration"),
            Self::Regression => write!(f, "regression"),
        }
    }
}

// ============================================================================
// Progress Evaluation
// ============================================================================

/// Result of evaluating progress signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvaluation {
    /// The overall verdict.
    pub verdict: ProgressVerdict,
    /// Weighted score (0.0 to 1.0, higher = more progress).
    pub score: f64,
    /// Explanation of the evaluation.
    pub explanation: String,
    /// Individual signal contributions.
    pub contributions: Vec<SignalContribution>,
}

/// Contribution of a single signal to the overall score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalContribution {
    /// Name of the signal.
    pub signal_name: String,
    /// Raw value of the signal.
    pub raw_value: f64,
    /// Weight applied to this signal.
    pub weight: f64,
    /// Weighted contribution to score.
    pub contribution: f64,
}

/// Configuration for the progress evaluator.
#[derive(Debug, Clone)]
pub struct ProgressEvaluatorConfig {
    // Weights for different signal types (should sum to ~1.0)
    /// Weight for commit signals.
    pub commit_weight: f64,
    /// Weight for line change signals.
    pub line_change_weight: f64,
    /// Weight for test signals.
    pub test_weight: f64,
    /// Weight for quality signals.
    pub quality_weight: f64,

    // Thresholds
    /// Score threshold for meaningful progress.
    pub meaningful_threshold: f64,
    /// Score threshold for partial progress.
    pub partial_threshold: f64,
    /// Exploration allowance (files touched without commits).
    pub exploration_allowance: u32,
}

impl Default for ProgressEvaluatorConfig {
    fn default() -> Self {
        Self {
            commit_weight: 0.4,
            line_change_weight: 0.2,
            test_weight: 0.25,
            quality_weight: 0.15,
            meaningful_threshold: 0.5,
            partial_threshold: 0.2,
            exploration_allowance: 5,
        }
    }
}

impl ProgressEvaluatorConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the meaningful progress threshold.
    #[must_use]
    pub fn with_meaningful_threshold(mut self, threshold: f64) -> Self {
        self.meaningful_threshold = threshold;
        self
    }

    /// Set the partial progress threshold.
    #[must_use]
    pub fn with_partial_threshold(mut self, threshold: f64) -> Self {
        self.partial_threshold = threshold;
        self
    }
}

// ============================================================================
// Progress Evaluator
// ============================================================================

/// Evaluates progress signals to determine overall progress verdict.
#[derive(Debug)]
pub struct ProgressEvaluator {
    config: ProgressEvaluatorConfig,
}

impl ProgressEvaluator {
    /// Create a new evaluator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ProgressEvaluatorConfig::default(),
        }
    }

    /// Create a new evaluator with custom configuration.
    #[must_use]
    pub fn with_config(config: ProgressEvaluatorConfig) -> Self {
        Self { config }
    }

    /// Evaluate progress signals and return a verdict.
    #[must_use]
    pub fn evaluate(&self, signals: &ProgressSignals) -> ProgressEvaluation {
        let mut contributions = Vec::new();
        let mut total_score = 0.0;

        // Commit contribution (0 or 1 based on any commits)
        let commit_score = if signals.commits_added > 0 { 1.0 } else { 0.0 };
        let commit_contribution = commit_score * self.config.commit_weight;
        contributions.push(SignalContribution {
            signal_name: "commits".to_string(),
            raw_value: signals.commits_added as f64,
            weight: self.config.commit_weight,
            contribution: commit_contribution,
        });
        total_score += commit_contribution;

        // Line change contribution (logarithmic scale, capped)
        let line_score = if signals.lines_changed > 0 {
            (signals.lines_changed as f64).ln().min(5.0) / 5.0
        } else {
            0.0
        };
        let line_contribution = line_score * self.config.line_change_weight;
        contributions.push(SignalContribution {
            signal_name: "lines_changed".to_string(),
            raw_value: signals.lines_changed as f64,
            weight: self.config.line_change_weight,
            contribution: line_contribution,
        });
        total_score += line_contribution;

        // Test contribution (positive for added tests)
        let test_score = if signals.tests_added > 0 {
            (signals.tests_added as f64 / 5.0).min(1.0)
        } else if signals.test_pass_delta > 0 {
            0.5
        } else {
            0.0
        };
        let test_contribution = test_score * self.config.test_weight;
        contributions.push(SignalContribution {
            signal_name: "tests".to_string(),
            raw_value: signals.tests_added as f64,
            weight: self.config.test_weight,
            contribution: test_contribution,
        });
        total_score += test_contribution;

        // Quality contribution (positive for reduced warnings)
        let quality_score = if signals.clippy_warnings_delta < 0 {
            (-signals.clippy_warnings_delta as f64 / 5.0).min(1.0)
        } else if signals.clippy_warnings_delta > 0 {
            -0.5 // Penalty for introducing warnings
        } else {
            0.0
        };
        let quality_contribution = quality_score * self.config.quality_weight;
        contributions.push(SignalContribution {
            signal_name: "quality".to_string(),
            raw_value: signals.clippy_warnings_delta as f64,
            weight: self.config.quality_weight,
            contribution: quality_contribution,
        });
        total_score += quality_contribution;

        // Clamp score to 0.0-1.0
        total_score = total_score.clamp(0.0, 1.0);

        // Determine verdict
        let verdict = self.determine_verdict(signals, total_score);

        // Generate explanation
        let explanation = self.generate_explanation(signals, &verdict, total_score);

        ProgressEvaluation {
            verdict,
            score: total_score,
            explanation,
            contributions,
        }
    }

    fn determine_verdict(&self, signals: &ProgressSignals, score: f64) -> ProgressVerdict {
        // Check for regression first
        if signals.clippy_warnings_delta > 5 || signals.test_pass_delta < -3 {
            return ProgressVerdict::Regression;
        }

        // Check for meaningful progress
        if score >= self.config.meaningful_threshold {
            return ProgressVerdict::MeaningfulProgress;
        }

        // Check for partial progress
        if score >= self.config.partial_threshold {
            return ProgressVerdict::PartialProgress;
        }

        // Check for exploration (many files touched but no commits)
        if signals.commits_added == 0
            && signals.unique_file_touches.len() >= self.config.exploration_allowance as usize
        {
            return ProgressVerdict::Exploration;
        }

        ProgressVerdict::NoProgress
    }

    fn generate_explanation(
        &self,
        signals: &ProgressSignals,
        verdict: &ProgressVerdict,
        score: f64,
    ) -> String {
        match verdict {
            ProgressVerdict::MeaningfulProgress => {
                format!(
                    "Meaningful progress detected (score: {:.2}): {}",
                    score,
                    signals.summary()
                )
            }
            ProgressVerdict::PartialProgress => {
                format!(
                    "Partial progress detected (score: {:.2}): {}",
                    score,
                    signals.summary()
                )
            }
            ProgressVerdict::NoProgress => {
                "No meaningful progress detected. Consider changing approach.".to_string()
            }
            ProgressVerdict::Exploration => {
                format!(
                    "Exploration detected: {} files touched without commits",
                    signals.unique_file_touches.len()
                )
            }
            ProgressVerdict::Regression => {
                let mut issues = Vec::new();
                if signals.clippy_warnings_delta > 0 {
                    issues.push(format!("+{} warnings", signals.clippy_warnings_delta));
                }
                if signals.test_pass_delta < 0 {
                    issues.push(format!("{} test failures", -signals.test_pass_delta));
                }
                format!("Quality regression detected: {}", issues.join(", "))
            }
        }
    }
}

impl Default for ProgressEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Git Signal Collector
// ============================================================================

/// Collects progress signals from git operations.
#[derive(Debug)]
pub struct GitSignalCollector {
    project_dir: PathBuf,
}

impl GitSignalCollector {
    /// Create a new git signal collector.
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }

    /// Count commits since a given hash.
    pub fn count_commits_since(&self, hash: &str) -> Result<u32> {
        if hash.is_empty() {
            return Ok(0);
        }

        let output = Command::new("git")
            .args(["rev-list", "--count", &format!("{hash}..HEAD")])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to count commits")?;

        if output.status.success() {
            let count_str = String::from_utf8_lossy(&output.stdout);
            Ok(count_str.trim().parse().unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// Get lines changed since a given hash.
    pub fn count_lines_changed_since(&self, hash: &str) -> Result<u32> {
        if hash.is_empty() {
            return Ok(0);
        }

        let output = Command::new("git")
            .args(["diff", "--stat", hash, "HEAD"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to get diff stats")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Last line contains summary like "5 files changed, 100 insertions(+), 20 deletions(-)"
            if let Some(last_line) = stdout.lines().last() {
                let mut total = 0u32;
                for word in last_line.split_whitespace() {
                    if let Ok(num) = word.parse::<u32>() {
                        if last_line.contains("insertion") || last_line.contains("deletion") {
                            total += num;
                        }
                    }
                }
                return Ok(total);
            }
        }
        Ok(0)
    }

    /// Get list of modified files from git status.
    pub fn get_modified_files(&self) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to get git status")?;

        let mut files = Vec::new();
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                // Format: "XY filename" where XY is status
                if line.len() > 3 {
                    let path = line[3..].trim();
                    files.push(PathBuf::from(path));
                }
            }
        }
        Ok(files)
    }

    /// Categorize modified files by type.
    pub fn categorize_changes(&self) -> Result<FileCategoryCounts> {
        let files = self.get_modified_files()?;
        let mut counts = FileCategoryCounts::default();

        for file in files {
            match FileCategory::from_path(&file) {
                FileCategory::Source => counts.source += 1,
                FileCategory::Test => counts.test += 1,
                FileCategory::Documentation => counts.doc += 1,
                FileCategory::Configuration => counts.config += 1,
                FileCategory::Other => counts.other += 1,
            }
        }

        Ok(counts)
    }

    /// Collect all git-related signals.
    pub fn collect_signals(&self, last_commit_hash: &str) -> Result<GitSignals> {
        let commits_added = self.count_commits_since(last_commit_hash)?;
        let lines_changed = self.count_lines_changed_since(last_commit_hash)?;
        let modified_files = self.get_modified_files()?;
        let categories = self.categorize_changes()?;

        Ok(GitSignals {
            commits_added,
            lines_changed,
            files_modified_count: modified_files.len() as u32,
            modified_files,
            categories,
        })
    }
}

/// Counts of files by category.
#[derive(Debug, Clone, Default)]
pub struct FileCategoryCounts {
    /// Source files count.
    pub source: u32,
    /// Test files count.
    pub test: u32,
    /// Documentation files count.
    pub doc: u32,
    /// Configuration files count.
    pub config: u32,
    /// Other files count.
    pub other: u32,
}

/// Git-related progress signals.
#[derive(Debug, Clone)]
pub struct GitSignals {
    /// Commits added since last check.
    pub commits_added: u32,
    /// Lines changed since last check.
    pub lines_changed: u32,
    /// Number of modified files.
    pub files_modified_count: u32,
    /// List of modified file paths.
    pub modified_files: Vec<PathBuf>,
    /// File categories.
    pub categories: FileCategoryCounts,
}

// ============================================================================
// Quality Signal Collector
// ============================================================================

/// Collects progress signals from quality checks.
#[derive(Debug)]
pub struct QualitySignalCollector {
    project_dir: PathBuf,
}

impl QualitySignalCollector {
    /// Create a new quality signal collector.
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }

    /// Count the number of tests in the project.
    pub fn count_tests(&self) -> Result<u32> {
        // List tests without running them
        let output = Command::new("cargo")
            .args(["test", "--", "--list"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to list tests")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Count lines that end with ": test"
            let count = stdout
                .lines()
                .filter(|line| line.ends_with(": test"))
                .count();
            Ok(count as u32)
        } else {
            Ok(0)
        }
    }

    /// Count clippy warnings.
    pub fn count_clippy_warnings(&self) -> Result<u32> {
        let output = Command::new("cargo")
            .args(["clippy", "--all-targets", "--message-format=json"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to run clippy")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let warning_count = stdout.matches("\"level\":\"warning\"").count();
        Ok(warning_count as u32)
    }

    /// Collect quality signals.
    pub fn collect_signals(
        &self,
        last_test_count: u32,
        last_warning_count: u32,
    ) -> Result<QualitySignals> {
        let current_test_count = self.count_tests().unwrap_or(last_test_count);
        let current_warning_count = self.count_clippy_warnings().unwrap_or(last_warning_count);

        Ok(QualitySignals {
            test_count: current_test_count,
            tests_added: current_test_count as i32 - last_test_count as i32,
            warning_count: current_warning_count,
            warnings_delta: current_warning_count as i32 - last_warning_count as i32,
        })
    }
}

/// Quality-related progress signals.
#[derive(Debug, Clone)]
pub struct QualitySignals {
    /// Current test count.
    pub test_count: u32,
    /// Tests added since last check.
    pub tests_added: i32,
    /// Current warning count.
    pub warning_count: u32,
    /// Warning delta (negative is good).
    pub warnings_delta: i32,
}

impl QualitySignals {
    /// Get a summary string of the quality state.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        parts.push(format!("{} tests", self.test_count));

        if self.tests_added != 0 {
            let sign = if self.tests_added > 0 { "+" } else { "" };
            parts.push(format!("({}{})", sign, self.tests_added));
        }

        parts.push(format!("{} warnings", self.warning_count));

        if self.warnings_delta != 0 {
            let sign = if self.warnings_delta > 0 { "+" } else { "" };
            parts.push(format!("({}{})", sign, self.warnings_delta));
        }

        parts.join(" ")
    }
}

// ============================================================================
// Progress Tracker (Facade)
// ============================================================================

/// High-level facade for progress tracking.
///
/// Combines git signals, quality signals, and behavioral analysis
/// to provide a comprehensive progress evaluation.
#[derive(Debug)]
pub struct ProgressTracker {
    git_collector: GitSignalCollector,
    quality_collector: QualitySignalCollector,
    evaluator: ProgressEvaluator,
    /// Last known test count for delta calculation.
    last_test_count: u32,
    /// Last known warning count for delta calculation.
    last_warning_count: u32,
}

impl ProgressTracker {
    /// Create a new progress tracker.
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            git_collector: GitSignalCollector::new(project_dir.clone()),
            quality_collector: QualitySignalCollector::new(project_dir),
            evaluator: ProgressEvaluator::new(),
            last_test_count: 0,
            last_warning_count: 0,
        }
    }

    /// Create with custom evaluator config.
    #[must_use]
    pub fn with_evaluator_config(project_dir: PathBuf, config: ProgressEvaluatorConfig) -> Self {
        Self {
            git_collector: GitSignalCollector::new(project_dir.clone()),
            quality_collector: QualitySignalCollector::new(project_dir),
            evaluator: ProgressEvaluator::with_config(config),
            last_test_count: 0,
            last_warning_count: 0,
        }
    }

    /// Collect all progress signals.
    pub fn collect_signals(&self, last_commit_hash: &str) -> Result<ProgressSignals> {
        let git_signals = self.git_collector.collect_signals(last_commit_hash)?;
        let quality_signals = self
            .quality_collector
            .collect_signals(self.last_test_count, self.last_warning_count)?;

        Ok(ProgressSignals {
            commits_added: git_signals.commits_added,
            lines_changed: git_signals.lines_changed,
            files_modified_count: git_signals.files_modified_count,
            source_files_modified: git_signals.categories.source,
            test_files_modified: git_signals.categories.test,
            doc_files_modified: git_signals.categories.doc,
            config_files_modified: git_signals.categories.config,
            tests_added: quality_signals.tests_added,
            test_pass_delta: 0, // Would need test execution to track
            clippy_warnings_delta: quality_signals.warnings_delta,
            unique_file_touches: git_signals
                .modified_files
                .into_iter()
                .collect(),
            repeated_edit_count: 0, // Would need iteration history
            exploration_breadth: self.calculate_exploration_breadth(&git_signals.categories),
        })
    }

    /// Evaluate progress and return verdict.
    pub fn evaluate(&self, last_commit_hash: &str) -> Result<ProgressEvaluation> {
        let signals = self.collect_signals(last_commit_hash)?;
        Ok(self.evaluator.evaluate(&signals))
    }

    /// Evaluate given signals.
    #[must_use]
    pub fn evaluate_signals(&self, signals: &ProgressSignals) -> ProgressEvaluation {
        self.evaluator.evaluate(signals)
    }

    /// Update baseline counts for delta calculation.
    pub fn update_baselines(&mut self, test_count: u32, warning_count: u32) {
        self.last_test_count = test_count;
        self.last_warning_count = warning_count;
    }

    /// Collect quality signals and log their summary.
    ///
    /// Returns the quality signals for further processing. Also logs the summary
    /// for debugging and monitoring purposes.
    pub fn collect_quality_signals(&self) -> Result<QualitySignals> {
        let signals = self
            .quality_collector
            .collect_signals(self.last_test_count, self.last_warning_count)?;
        tracing::debug!("Quality signals: {}", signals.summary());
        Ok(signals)
    }

    /// Create a tracker with strict thresholds.
    ///
    /// Uses higher thresholds for meaningful progress, suitable for
    /// environments where stricter validation is needed.
    #[must_use]
    pub fn with_strict_thresholds(project_dir: PathBuf) -> Self {
        let config = ProgressEvaluatorConfig::new()
            .with_meaningful_threshold(0.6)
            .with_partial_threshold(0.3);
        Self::with_evaluator_config(project_dir, config)
    }

    fn calculate_exploration_breadth(&self, categories: &FileCategoryCounts) -> u32 {
        let mut breadth = 0;
        if categories.source > 0 {
            breadth += 1;
        }
        if categories.test > 0 {
            breadth += 1;
        }
        if categories.doc > 0 {
            breadth += 1;
        }
        if categories.config > 0 {
            breadth += 1;
        }
        breadth
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_signals_default() {
        let signals = ProgressSignals::new();
        assert_eq!(signals.commits_added, 0);
        assert_eq!(signals.lines_changed, 0);
        assert!(!signals.has_any_positive_signal());
    }

    #[test]
    fn test_progress_signals_summary_empty() {
        let signals = ProgressSignals::new();
        assert_eq!(signals.summary(), "no changes");
    }

    #[test]
    fn test_progress_signals_summary_with_data() {
        let mut signals = ProgressSignals::new();
        signals.commits_added = 2;
        signals.lines_changed = 50;
        signals.source_files_modified = 3;

        let summary = signals.summary();
        assert!(summary.contains("2 commits"));
        assert!(summary.contains("50 lines"));
        assert!(summary.contains("3 src files"));
    }

    #[test]
    fn test_progress_signals_has_positive_signal() {
        let mut signals = ProgressSignals::new();
        assert!(!signals.has_any_positive_signal());

        signals.commits_added = 1;
        assert!(signals.has_any_positive_signal());

        signals.commits_added = 0;
        signals.lines_changed = 20;
        assert!(signals.has_any_positive_signal());

        signals.lines_changed = 0;
        signals.tests_added = 1;
        assert!(signals.has_any_positive_signal());

        signals.tests_added = 0;
        signals.clippy_warnings_delta = -3;
        assert!(signals.has_any_positive_signal());
    }

    #[test]
    fn test_file_category_from_path() {
        assert_eq!(
            FileCategory::from_path(Path::new("src/lib.rs")),
            FileCategory::Source
        );
        assert_eq!(
            FileCategory::from_path(Path::new("tests/integration.rs")),
            FileCategory::Test
        );
        assert_eq!(
            FileCategory::from_path(Path::new("README.md")),
            FileCategory::Documentation
        );
        assert_eq!(
            FileCategory::from_path(Path::new("Cargo.toml")),
            FileCategory::Configuration
        );
        assert_eq!(
            FileCategory::from_path(Path::new("some_file")),
            FileCategory::Other
        );
    }

    #[test]
    fn test_progress_verdict_should_reset_stagnation() {
        assert!(ProgressVerdict::MeaningfulProgress.should_reset_stagnation());
        assert!(!ProgressVerdict::PartialProgress.should_reset_stagnation());
        assert!(!ProgressVerdict::NoProgress.should_reset_stagnation());
        assert!(!ProgressVerdict::Exploration.should_reset_stagnation());
        assert!(!ProgressVerdict::Regression.should_reset_stagnation());
    }

    #[test]
    fn test_progress_verdict_is_healthy() {
        assert!(ProgressVerdict::MeaningfulProgress.is_healthy());
        assert!(ProgressVerdict::PartialProgress.is_healthy());
        assert!(ProgressVerdict::Exploration.is_healthy());
        assert!(!ProgressVerdict::NoProgress.is_healthy());
        assert!(!ProgressVerdict::Regression.is_healthy());
    }

    #[test]
    fn test_progress_evaluator_no_signals() {
        let evaluator = ProgressEvaluator::new();
        let signals = ProgressSignals::new();
        let evaluation = evaluator.evaluate(&signals);

        assert_eq!(evaluation.verdict, ProgressVerdict::NoProgress);
        assert_eq!(evaluation.score, 0.0);
    }

    #[test]
    fn test_progress_evaluator_meaningful_progress() {
        let evaluator = ProgressEvaluator::new();
        let mut signals = ProgressSignals::new();
        signals.commits_added = 2;
        signals.lines_changed = 100;
        signals.tests_added = 5;

        let evaluation = evaluator.evaluate(&signals);
        assert_eq!(evaluation.verdict, ProgressVerdict::MeaningfulProgress);
        assert!(evaluation.score >= 0.5);
    }

    #[test]
    fn test_progress_evaluator_regression() {
        let evaluator = ProgressEvaluator::new();
        let mut signals = ProgressSignals::new();
        signals.clippy_warnings_delta = 10;

        let evaluation = evaluator.evaluate(&signals);
        assert_eq!(evaluation.verdict, ProgressVerdict::Regression);
    }

    #[test]
    fn test_progress_evaluator_exploration() {
        let evaluator = ProgressEvaluator::new();
        let mut signals = ProgressSignals::new();
        signals.commits_added = 0;
        for i in 0..10 {
            signals
                .unique_file_touches
                .insert(PathBuf::from(format!("file{}.rs", i)));
        }

        let evaluation = evaluator.evaluate(&signals);
        assert_eq!(evaluation.verdict, ProgressVerdict::Exploration);
    }

    #[test]
    fn test_progress_evaluator_partial_progress() {
        let evaluator = ProgressEvaluator::new();
        let mut signals = ProgressSignals::new();
        // Need significant line changes to reach partial progress threshold (0.2)
        // With line_change_weight of 0.2, we need line_score of 1.0
        // which requires ln(lines_changed) >= 5.0, so lines_changed >= 149
        signals.lines_changed = 150; // Some activity but no commits

        let evaluation = evaluator.evaluate(&signals);
        assert_eq!(evaluation.verdict, ProgressVerdict::PartialProgress);
    }

    #[test]
    fn test_evaluator_config_custom_thresholds() {
        let config = ProgressEvaluatorConfig::new()
            .with_meaningful_threshold(0.8)
            .with_partial_threshold(0.4);

        let evaluator = ProgressEvaluator::with_config(config);
        let mut signals = ProgressSignals::new();
        signals.commits_added = 1;

        let evaluation = evaluator.evaluate(&signals);
        // With higher threshold, a single commit might not be "meaningful"
        assert!(evaluation.score < 0.8);
    }

    #[test]
    fn test_progress_evaluation_contributions() {
        let evaluator = ProgressEvaluator::new();
        let mut signals = ProgressSignals::new();
        signals.commits_added = 1;
        signals.tests_added = 2;

        let evaluation = evaluator.evaluate(&signals);
        assert!(!evaluation.contributions.is_empty());

        // Should have contributions for commits, lines, tests, quality
        let names: Vec<_> = evaluation
            .contributions
            .iter()
            .map(|c| c.signal_name.as_str())
            .collect();
        assert!(names.contains(&"commits"));
        assert!(names.contains(&"tests"));
    }

    #[test]
    fn test_file_category_counts_default() {
        let counts = FileCategoryCounts::default();
        assert_eq!(counts.source, 0);
        assert_eq!(counts.test, 0);
        assert_eq!(counts.doc, 0);
        assert_eq!(counts.config, 0);
        assert_eq!(counts.other, 0);
    }
}
