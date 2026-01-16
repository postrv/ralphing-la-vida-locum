//! Anti-pattern detection for prompt generation.
//!
//! This module detects common anti-patterns in the automation loop
//! and generates warnings to help improve productivity.
//!
//! # Example
//!
//! ```
//! use ralph::prompt::antipatterns::{AntiPatternDetector, IterationSummary};
//!
//! let mut detector = AntiPatternDetector::new();
//! let summary = IterationSummary::new(1)
//!     .with_files_modified(vec!["src/lib.rs".to_string()])
//!     .without_commit();
//!
//! detector.add_iteration(summary);
//! let patterns = detector.detect();
//! // No patterns after one iteration without commit
//! ```

use crate::prompt::context::{AntiPattern, AntiPatternSeverity, AntiPatternType};
use std::collections::{HashMap, HashSet};

/// Summary of a single iteration for pattern detection.
///
/// # Example
///
/// ```
/// use ralph::prompt::antipatterns::IterationSummary;
///
/// let summary = IterationSummary::new(1)
///     .with_files_modified(vec!["src/lib.rs".to_string()])
///     .with_commit()
///     .with_tests_run();
///
/// assert!(summary.committed);
/// assert!(summary.tests_run);
/// ```
#[derive(Debug, Clone)]
pub struct IterationSummary {
    /// Iteration number (1-indexed).
    pub iteration: u32,
    /// Files modified in this iteration.
    pub files_modified: Vec<String>,
    /// Whether a commit was made.
    pub committed: bool,
    /// Whether tests were run.
    pub tests_run: bool,
    /// Whether clippy was run.
    pub clippy_run: bool,
    /// Current task ID, if any.
    pub current_task: Option<String>,
    /// Errors encountered.
    pub errors: Vec<String>,
    /// Exit code of Claude process.
    pub exit_code: i32,
}

impl IterationSummary {
    /// Create a new iteration summary.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::antipatterns::IterationSummary;
    ///
    /// let summary = IterationSummary::new(1);
    /// assert_eq!(summary.iteration, 1);
    /// assert!(!summary.committed);
    /// ```
    #[must_use]
    pub fn new(iteration: u32) -> Self {
        Self {
            iteration,
            files_modified: Vec::new(),
            committed: false,
            tests_run: false,
            clippy_run: false,
            current_task: None,
            errors: Vec::new(),
            exit_code: 0,
        }
    }

    /// Set files modified.
    #[must_use]
    pub fn with_files_modified(mut self, files: Vec<String>) -> Self {
        self.files_modified = files;
        self
    }

    /// Mark that a commit was made.
    #[must_use]
    pub fn with_commit(mut self) -> Self {
        self.committed = true;
        self
    }

    /// Mark that no commit was made.
    #[must_use]
    pub fn without_commit(mut self) -> Self {
        self.committed = false;
        self
    }

    /// Mark that tests were run.
    #[must_use]
    pub fn with_tests_run(mut self) -> Self {
        self.tests_run = true;
        self
    }

    /// Mark that clippy was run.
    #[must_use]
    pub fn with_clippy_run(mut self) -> Self {
        self.clippy_run = true;
        self
    }

    /// Set the current task.
    #[must_use]
    pub fn with_task(mut self, task: impl Into<String>) -> Self {
        self.current_task = Some(task.into());
        self
    }

    /// Add errors.
    #[must_use]
    pub fn with_errors(mut self, errors: Vec<String>) -> Self {
        self.errors = errors;
        self
    }

    /// Set exit code.
    #[must_use]
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }
}

/// Configuration for anti-pattern detection thresholds.
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// Minimum iterations without commit before warning.
    pub edit_without_commit_threshold: u32,
    /// Minimum iterations without tests before warning.
    pub tests_not_run_threshold: u32,
    /// Minimum iterations without clippy before warning.
    pub clippy_not_run_threshold: u32,
    /// Minimum task switches before oscillation warning.
    pub task_oscillation_threshold: u32,
    /// Minimum error repetitions before warning.
    pub error_repetition_threshold: u32,
    /// Minimum file modifications to trigger churn detection.
    pub file_churn_threshold: u32,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            edit_without_commit_threshold: 3,
            tests_not_run_threshold: 5,
            clippy_not_run_threshold: 5,
            task_oscillation_threshold: 4,
            error_repetition_threshold: 2,
            file_churn_threshold: 5,
        }
    }
}

/// Anti-pattern detector that analyzes iteration history.
///
/// # Example
///
/// ```
/// use ralph::prompt::antipatterns::{AntiPatternDetector, IterationSummary};
///
/// let mut detector = AntiPatternDetector::new();
///
/// // Simulate 4 iterations editing without committing
/// for i in 1..=4 {
///     detector.add_iteration(
///         IterationSummary::new(i)
///             .with_files_modified(vec!["src/lib.rs".to_string()])
///     );
/// }
///
/// let patterns = detector.detect();
/// assert!(!patterns.is_empty());
/// ```
#[derive(Debug)]
pub struct AntiPatternDetector {
    /// Configuration for detection thresholds.
    config: DetectorConfig,
    /// History of iteration summaries.
    iterations: Vec<IterationSummary>,
    /// Persistence counts for detected patterns.
    persistence: HashMap<AntiPatternType, u32>,
}

impl AntiPatternDetector {
    /// Create a new detector with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DetectorConfig::default())
    }

    /// Create a detector with custom configuration.
    #[must_use]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            config,
            iterations: Vec::new(),
            persistence: HashMap::new(),
        }
    }

    /// Add an iteration summary to the history.
    pub fn add_iteration(&mut self, summary: IterationSummary) {
        self.iterations.push(summary);
    }

    /// Clear the iteration history.
    pub fn clear(&mut self) {
        self.iterations.clear();
    }

    /// Get the number of iterations in history.
    #[must_use]
    pub fn iteration_count(&self) -> usize {
        self.iterations.len()
    }

    /// Detect all anti-patterns in the current iteration history.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::antipatterns::{AntiPatternDetector, IterationSummary};
    ///
    /// let mut detector = AntiPatternDetector::new();
    /// detector.add_iteration(IterationSummary::new(1).with_files_modified(vec!["a.rs".into()]));
    /// detector.add_iteration(IterationSummary::new(2).with_files_modified(vec!["b.rs".into()]));
    ///
    /// let patterns = detector.detect();
    /// // May or may not have patterns depending on configuration
    /// ```
    #[must_use]
    pub fn detect(&mut self) -> Vec<AntiPattern> {
        let mut patterns = Vec::new();

        if self.iterations.is_empty() {
            return patterns;
        }

        // Detect each pattern type
        if let Some(pattern) = self.detect_edit_without_commit() {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_tests_not_run() {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_clippy_not_run() {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_task_oscillation() {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_repeating_errors() {
            patterns.push(pattern);
        }

        if let Some(pattern) = self.detect_file_churn() {
            patterns.push(pattern);
        }

        patterns
    }

    /// Detect editing without committing pattern.
    fn detect_edit_without_commit(&mut self) -> Option<AntiPattern> {
        let recent = self.recent_iterations(self.config.edit_without_commit_threshold);

        // Count consecutive iterations without commit that have file modifications
        let mut consecutive_no_commit = 0;
        let mut files_modified: HashSet<String> = HashSet::new();

        for summary in recent.iter().rev() {
            if summary.committed {
                break;
            }
            if !summary.files_modified.is_empty() {
                consecutive_no_commit += 1;
                for file in &summary.files_modified {
                    files_modified.insert(file.clone());
                }
            }
        }

        if consecutive_no_commit >= self.config.edit_without_commit_threshold {
            let persistence = self.increment_persistence(AntiPatternType::EditWithoutCommit);
            let severity = if consecutive_no_commit >= 5 {
                AntiPatternSeverity::High
            } else {
                AntiPatternSeverity::Medium
            };

            Some(
                AntiPattern::new(
                    AntiPatternType::EditWithoutCommit,
                    format!(
                        "{} consecutive iterations have modified files without committing",
                        consecutive_no_commit
                    ),
                )
                .with_evidence(
                    files_modified
                        .iter()
                        .take(5)
                        .cloned()
                        .collect(),
                )
                .with_severity(severity)
                .with_remediation("Make incremental commits to save progress and enable rollback")
                .with_persistence(persistence),
            )
        } else {
            self.reset_persistence(AntiPatternType::EditWithoutCommit);
            None
        }
    }

    /// Detect tests not being run.
    fn detect_tests_not_run(&mut self) -> Option<AntiPattern> {
        let recent = self.recent_iterations(self.config.tests_not_run_threshold);

        let tests_run_count = recent.iter().filter(|s| s.tests_run).count();
        let recent_len = recent.len();
        // Drop the borrow on self.iterations
        drop(recent);

        if recent_len >= self.config.tests_not_run_threshold as usize && tests_run_count == 0 {
            let persistence = self.increment_persistence(AntiPatternType::TestsNotRun);
            let severity = if persistence >= 3 {
                AntiPatternSeverity::High
            } else {
                AntiPatternSeverity::Medium
            };

            Some(
                AntiPattern::new(
                    AntiPatternType::TestsNotRun,
                    format!(
                        "Tests have not been run in the last {} iterations",
                        recent_len
                    ),
                )
                .with_severity(severity)
                .with_remediation("Run `cargo test` to verify changes don't break existing functionality")
                .with_persistence(persistence),
            )
        } else {
            self.reset_persistence(AntiPatternType::TestsNotRun);
            None
        }
    }

    /// Detect clippy not being run.
    fn detect_clippy_not_run(&mut self) -> Option<AntiPattern> {
        let recent = self.recent_iterations(self.config.clippy_not_run_threshold);

        let clippy_run_count = recent.iter().filter(|s| s.clippy_run).count();
        let recent_len = recent.len();
        // Drop the borrow on self.iterations
        drop(recent);

        if recent_len >= self.config.clippy_not_run_threshold as usize && clippy_run_count == 0 {
            let persistence = self.increment_persistence(AntiPatternType::ClippyNotRun);

            Some(
                AntiPattern::new(
                    AntiPatternType::ClippyNotRun,
                    format!(
                        "Clippy has not been run in the last {} iterations",
                        recent_len
                    ),
                )
                .with_severity(AntiPatternSeverity::Low)
                .with_remediation("Run `cargo clippy --all-targets -- -D warnings` to catch lints")
                .with_persistence(persistence),
            )
        } else {
            self.reset_persistence(AntiPatternType::ClippyNotRun);
            None
        }
    }

    /// Detect task oscillation.
    fn detect_task_oscillation(&mut self) -> Option<AntiPattern> {
        let recent = self.recent_iterations(self.config.task_oscillation_threshold + 2);

        if recent.len() < self.config.task_oscillation_threshold as usize {
            return None;
        }

        // Extract task sequence (owned strings to avoid borrow issues)
        let tasks: Vec<String> = recent
            .iter()
            .filter_map(|s| s.current_task.clone())
            .collect();

        // Drop the borrow on self.iterations
        drop(recent);

        if tasks.len() < self.config.task_oscillation_threshold as usize {
            return None;
        }

        // Count task switches
        let mut switches = 0;
        let mut seen_tasks: HashSet<String> = HashSet::new();

        for window in tasks.windows(2) {
            if window[0] != window[1] {
                switches += 1;
                seen_tasks.insert(window[0].clone());
                seen_tasks.insert(window[1].clone());
            }
        }

        let seen_tasks_len = seen_tasks.len();

        // Detect oscillation: many switches between few tasks
        if switches >= self.config.task_oscillation_threshold as usize && seen_tasks_len <= 3 {
            let persistence = self.increment_persistence(AntiPatternType::TaskOscillation);

            Some(
                AntiPattern::new(
                    AntiPatternType::TaskOscillation,
                    format!(
                        "Switched tasks {} times between {} tasks without completing any",
                        switches, seen_tasks_len
                    ),
                )
                .with_evidence(
                    seen_tasks
                        .iter()
                        .map(|s| format!("Task: {}", s))
                        .collect(),
                )
                .with_severity(AntiPatternSeverity::High)
                .with_remediation("Focus on completing one task before starting another")
                .with_persistence(persistence),
            )
        } else {
            self.reset_persistence(AntiPatternType::TaskOscillation);
            None
        }
    }

    /// Detect repeating errors.
    fn detect_repeating_errors(&mut self) -> Option<AntiPattern> {
        let recent = self.recent_iterations(5);

        // Count error occurrences (use owned strings)
        let mut error_counts: HashMap<String, usize> = HashMap::new();
        for summary in &recent {
            for error in &summary.errors {
                *error_counts.entry(error.clone()).or_insert(0) += 1;
            }
        }
        // Drop the borrow on self.iterations
        drop(recent);

        // Find errors that repeat (owned data)
        let repeating: Vec<(String, usize)> = error_counts
            .into_iter()
            .filter(|(_, count)| *count >= self.config.error_repetition_threshold as usize)
            .collect();

        if !repeating.is_empty() {
            let repeating_len = repeating.len();
            let is_high_severity = repeating_len >= 3 || repeating.iter().any(|(_, c)| *c >= 4);
            let evidence: Vec<String> = repeating
                .iter()
                .map(|(error, count)| format!("{} (×{})", error, count))
                .collect();

            let persistence = self.increment_persistence(AntiPatternType::RepeatingErrors);
            let severity = if is_high_severity {
                AntiPatternSeverity::High
            } else {
                AntiPatternSeverity::Medium
            };

            Some(
                AntiPattern::new(
                    AntiPatternType::RepeatingErrors,
                    format!("{} error(s) have occurred multiple times", repeating_len),
                )
                .with_evidence(evidence)
                .with_severity(severity)
                .with_remediation("Try a different approach - the current strategy isn't resolving these errors")
                .with_persistence(persistence),
            )
        } else {
            self.reset_persistence(AntiPatternType::RepeatingErrors);
            None
        }
    }

    /// Detect file churn (repeatedly modifying the same files).
    fn detect_file_churn(&mut self) -> Option<AntiPattern> {
        let recent = self.recent_iterations(self.config.file_churn_threshold);
        let churn_min = (self.config.file_churn_threshold as usize / 2).max(3);

        // Count file modification occurrences (owned strings)
        let mut file_counts: HashMap<String, usize> = HashMap::new();
        for summary in &recent {
            for file in &summary.files_modified {
                *file_counts.entry(file.clone()).or_insert(0) += 1;
            }
        }
        // Drop the borrow on self.iterations
        drop(recent);

        // Find files modified in most iterations (owned data)
        let churning: Vec<(String, usize)> = file_counts
            .into_iter()
            .filter(|(_, count)| *count >= churn_min)
            .collect();

        let churning_len = churning.len();
        if !churning.is_empty() && churning_len <= 3 {
            let evidence: Vec<String> = churning
                .iter()
                .map(|(file, count)| format!("{} (modified {} times)", file, count))
                .collect();

            let persistence = self.increment_persistence(AntiPatternType::FileChurn);

            Some(
                AntiPattern::new(
                    AntiPatternType::FileChurn,
                    format!(
                        "{} file(s) have been modified repeatedly without progress",
                        churning_len
                    ),
                )
                .with_evidence(evidence)
                .with_severity(AntiPatternSeverity::Medium)
                .with_remediation("Consider if the approach is correct - repeated edits to the same file may indicate a design issue")
                .with_persistence(persistence),
            )
        } else {
            self.reset_persistence(AntiPatternType::FileChurn);
            None
        }
    }

    /// Get the most recent N iterations.
    fn recent_iterations(&self, n: u32) -> Vec<&IterationSummary> {
        let start = self.iterations.len().saturating_sub(n as usize);
        self.iterations[start..].iter().collect()
    }

    /// Increment persistence counter for a pattern type.
    fn increment_persistence(&mut self, pattern_type: AntiPatternType) -> u32 {
        let count = self.persistence.entry(pattern_type).or_insert(0);
        *count += 1;
        *count
    }

    /// Reset persistence counter for a pattern type.
    fn reset_persistence(&mut self, pattern_type: AntiPatternType) {
        self.persistence.remove(&pattern_type);
    }
}

impl Default for AntiPatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyzes task tracker state for anti-patterns.
///
/// This function can be used when you have access to task tracker state.
///
/// # Example
///
/// ```
/// use ralph::prompt::antipatterns::detect_quality_gate_ignoring;
/// use ralph::prompt::context::{QualityGateStatus, GateResult};
///
/// let status = QualityGateStatus::new()
///     .with_tests(GateResult::fail(vec!["test failed".to_string()]))
///     .with_timestamp();
///
/// let consecutive_failures = 3;
/// let pattern = detect_quality_gate_ignoring(&status, consecutive_failures);
/// assert!(pattern.is_some());
/// ```
#[must_use]
pub fn detect_quality_gate_ignoring(
    status: &crate::prompt::context::QualityGateStatus,
    consecutive_failures: u32,
) -> Option<AntiPattern> {
    if consecutive_failures >= 3 && status.has_failures() {
        let failing = status.failing_gates();
        Some(
            AntiPattern::new(
                AntiPatternType::IgnoringQualityGates,
                format!(
                    "Quality gates have been failing for {} consecutive iterations: {}",
                    consecutive_failures,
                    failing.join(", ")
                ),
            )
            .with_evidence(
                failing
                    .iter()
                    .map(|g| format!("{} is failing", g))
                    .collect(),
            )
            .with_severity(AntiPatternSeverity::High)
            .with_remediation("Fix the failing quality gates before continuing with other work"),
        )
    } else {
        None
    }
}

/// Analyzes scope for scope creep.
///
/// # Example
///
/// ```
/// use ralph::prompt::antipatterns::detect_scope_creep;
///
/// let files = vec![
///     "src/module_a/mod.rs".to_string(),
///     "src/module_b/mod.rs".to_string(),
///     "src/module_c/mod.rs".to_string(),
///     "tests/test_a.rs".to_string(),
/// ];
///
/// let pattern = detect_scope_creep(&files, 3);
/// // May detect scope creep if many unrelated files are modified
/// ```
#[must_use]
pub fn detect_scope_creep(files_modified: &[String], threshold: usize) -> Option<AntiPattern> {
    if files_modified.len() < threshold {
        return None;
    }

    // Extract directory paths
    let directories: HashSet<_> = files_modified
        .iter()
        .filter_map(|f| {
            let parts: Vec<_> = f.split('/').collect();
            if parts.len() > 1 {
                Some(parts[..parts.len() - 1].join("/"))
            } else {
                None
            }
        })
        .collect();

    // Scope creep: many different directories modified
    if directories.len() >= threshold {
        Some(
            AntiPattern::new(
                AntiPatternType::ScopeCreep,
                format!(
                    "Modified {} files across {} different directories",
                    files_modified.len(),
                    directories.len()
                ),
            )
            .with_evidence(directories.iter().take(5).cloned().collect())
            .with_severity(AntiPatternSeverity::Medium)
            .with_remediation("Consider focusing on fewer areas at once to reduce complexity"),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::context::{GateResult, QualityGateStatus};

    // IterationSummary tests

    #[test]
    fn test_iteration_summary_new() {
        let summary = IterationSummary::new(1);
        assert_eq!(summary.iteration, 1);
        assert!(!summary.committed);
        assert!(!summary.tests_run);
        assert!(!summary.clippy_run);
        assert!(summary.files_modified.is_empty());
    }

    #[test]
    fn test_iteration_summary_builders() {
        let summary = IterationSummary::new(2)
            .with_files_modified(vec!["src/lib.rs".to_string()])
            .with_commit()
            .with_tests_run()
            .with_clippy_run()
            .with_task("1.2")
            .with_errors(vec!["E0308".to_string()])
            .with_exit_code(1);

        assert_eq!(summary.iteration, 2);
        assert!(summary.committed);
        assert!(summary.tests_run);
        assert!(summary.clippy_run);
        assert_eq!(summary.current_task, Some("1.2".to_string()));
        assert_eq!(summary.errors, vec!["E0308"]);
        assert_eq!(summary.exit_code, 1);
    }

    // DetectorConfig tests

    #[test]
    fn test_detector_config_default() {
        let config = DetectorConfig::default();
        assert_eq!(config.edit_without_commit_threshold, 3);
        assert_eq!(config.tests_not_run_threshold, 5);
        assert_eq!(config.clippy_not_run_threshold, 5);
    }

    // AntiPatternDetector tests

    #[test]
    fn test_detector_new() {
        let detector = AntiPatternDetector::new();
        assert_eq!(detector.iteration_count(), 0);
    }

    #[test]
    fn test_detector_add_iteration() {
        let mut detector = AntiPatternDetector::new();
        detector.add_iteration(IterationSummary::new(1));
        detector.add_iteration(IterationSummary::new(2));
        assert_eq!(detector.iteration_count(), 2);
    }

    #[test]
    fn test_detector_clear() {
        let mut detector = AntiPatternDetector::new();
        detector.add_iteration(IterationSummary::new(1));
        detector.clear();
        assert_eq!(detector.iteration_count(), 0);
    }

    // Edit without commit detection

    #[test]
    fn test_detect_edit_without_commit_threshold_not_met() {
        let mut detector = AntiPatternDetector::new();

        // Only 2 iterations without commit (threshold is 3)
        for i in 1..=2 {
            detector.add_iteration(
                IterationSummary::new(i).with_files_modified(vec!["a.rs".to_string()]),
            );
        }

        let patterns = detector.detect();
        assert!(!patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::EditWithoutCommit));
    }

    #[test]
    fn test_detect_edit_without_commit_threshold_met() {
        let mut detector = AntiPatternDetector::new();

        // 3 iterations without commit
        for i in 1..=3 {
            detector.add_iteration(
                IterationSummary::new(i).with_files_modified(vec![format!("{}.rs", i)]),
            );
        }

        let patterns = detector.detect();
        let pattern = patterns
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::EditWithoutCommit);

        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert!(p.description.contains("3"));
    }

    #[test]
    fn test_detect_edit_without_commit_reset_on_commit() {
        let mut detector = AntiPatternDetector::new();

        // 2 iterations without commit
        detector.add_iteration(
            IterationSummary::new(1).with_files_modified(vec!["a.rs".to_string()]),
        );
        detector.add_iteration(
            IterationSummary::new(2).with_files_modified(vec!["b.rs".to_string()]),
        );

        // Then a commit
        detector.add_iteration(
            IterationSummary::new(3)
                .with_files_modified(vec!["c.rs".to_string()])
                .with_commit(),
        );

        // 1 more without commit
        detector.add_iteration(
            IterationSummary::new(4).with_files_modified(vec!["d.rs".to_string()]),
        );

        let patterns = detector.detect();
        assert!(!patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::EditWithoutCommit));
    }

    // Tests not run detection

    #[test]
    fn test_detect_tests_not_run() {
        let config = DetectorConfig {
            tests_not_run_threshold: 3,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        // 3 iterations without tests
        for i in 1..=3 {
            detector.add_iteration(IterationSummary::new(i));
        }

        let patterns = detector.detect();
        let pattern = patterns
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::TestsNotRun);

        assert!(pattern.is_some());
    }

    #[test]
    fn test_detect_tests_run_resets() {
        let config = DetectorConfig {
            tests_not_run_threshold: 3,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        // 2 without tests, then 1 with
        detector.add_iteration(IterationSummary::new(1));
        detector.add_iteration(IterationSummary::new(2));
        detector.add_iteration(IterationSummary::new(3).with_tests_run());

        let patterns = detector.detect();
        assert!(!patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::TestsNotRun));
    }

    // Clippy not run detection

    #[test]
    fn test_detect_clippy_not_run() {
        let config = DetectorConfig {
            clippy_not_run_threshold: 3,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        for i in 1..=3 {
            detector.add_iteration(IterationSummary::new(i));
        }

        let patterns = detector.detect();
        let pattern = patterns
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::ClippyNotRun);

        assert!(pattern.is_some());
        assert_eq!(pattern.unwrap().severity, AntiPatternSeverity::Low);
    }

    // Task oscillation detection

    #[test]
    fn test_detect_task_oscillation() {
        let config = DetectorConfig {
            task_oscillation_threshold: 3,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        // Oscillate between two tasks
        detector.add_iteration(IterationSummary::new(1).with_task("task-1"));
        detector.add_iteration(IterationSummary::new(2).with_task("task-2"));
        detector.add_iteration(IterationSummary::new(3).with_task("task-1"));
        detector.add_iteration(IterationSummary::new(4).with_task("task-2"));
        detector.add_iteration(IterationSummary::new(5).with_task("task-1"));

        let patterns = detector.detect();
        let pattern = patterns
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::TaskOscillation);

        assert!(pattern.is_some());
        assert_eq!(pattern.unwrap().severity, AntiPatternSeverity::High);
    }

    #[test]
    fn test_no_task_oscillation_when_progressing() {
        let mut detector = AntiPatternDetector::new();

        // Different tasks but not oscillating
        detector.add_iteration(IterationSummary::new(1).with_task("task-1"));
        detector.add_iteration(IterationSummary::new(2).with_task("task-1"));
        detector.add_iteration(IterationSummary::new(3).with_task("task-2"));
        detector.add_iteration(IterationSummary::new(4).with_task("task-2"));
        detector.add_iteration(IterationSummary::new(5).with_task("task-3"));

        let patterns = detector.detect();
        assert!(!patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::TaskOscillation));
    }

    // Repeating errors detection

    #[test]
    fn test_detect_repeating_errors() {
        let config = DetectorConfig {
            error_repetition_threshold: 2,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        detector.add_iteration(IterationSummary::new(1).with_errors(vec!["E0308".to_string()]));
        detector.add_iteration(IterationSummary::new(2).with_errors(vec!["E0308".to_string()]));

        let patterns = detector.detect();
        let pattern = patterns
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::RepeatingErrors);

        assert!(pattern.is_some());
        assert!(pattern.unwrap().evidence.iter().any(|e| e.contains("E0308")));
    }

    #[test]
    fn test_no_repeating_errors_when_unique() {
        let mut detector = AntiPatternDetector::new();

        detector.add_iteration(IterationSummary::new(1).with_errors(vec!["E0308".to_string()]));
        detector.add_iteration(IterationSummary::new(2).with_errors(vec!["E0433".to_string()]));
        detector.add_iteration(IterationSummary::new(3).with_errors(vec!["E0599".to_string()]));

        let patterns = detector.detect();
        assert!(!patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::RepeatingErrors));
    }

    // File churn detection

    #[test]
    fn test_detect_file_churn() {
        let config = DetectorConfig {
            file_churn_threshold: 4,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        // Same file modified in 3 out of 4 iterations
        detector.add_iteration(
            IterationSummary::new(1).with_files_modified(vec!["src/lib.rs".to_string()]),
        );
        detector.add_iteration(
            IterationSummary::new(2).with_files_modified(vec!["src/lib.rs".to_string()]),
        );
        detector.add_iteration(
            IterationSummary::new(3).with_files_modified(vec!["src/lib.rs".to_string()]),
        );
        detector.add_iteration(IterationSummary::new(4).with_files_modified(vec![]));

        let patterns = detector.detect();
        let pattern = patterns
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::FileChurn);

        assert!(pattern.is_some());
    }

    // Persistence tracking

    #[test]
    fn test_persistence_increments() {
        let mut detector = AntiPatternDetector::new();

        // Create pattern condition
        for i in 1..=3 {
            detector.add_iteration(
                IterationSummary::new(i).with_files_modified(vec!["a.rs".to_string()]),
            );
        }

        let patterns1 = detector.detect();
        let pattern1 = patterns1
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::EditWithoutCommit)
            .unwrap();
        assert_eq!(pattern1.persistence_count, 1);

        // Add another iteration, persistence should increment
        detector.add_iteration(
            IterationSummary::new(4).with_files_modified(vec!["b.rs".to_string()]),
        );

        let patterns2 = detector.detect();
        let pattern2 = patterns2
            .iter()
            .find(|p| p.pattern_type == AntiPatternType::EditWithoutCommit)
            .unwrap();
        assert_eq!(pattern2.persistence_count, 2);
    }

    // Quality gate ignoring detection

    #[test]
    fn test_detect_quality_gate_ignoring() {
        let status = QualityGateStatus::new()
            .with_tests(GateResult::fail(vec!["test failed".to_string()]))
            .with_clippy(GateResult::pass())
            .with_no_allow(GateResult::pass())
            .with_security(GateResult::pass())
            .with_docs(GateResult::pass())
            .with_timestamp();

        let pattern = detect_quality_gate_ignoring(&status, 3);
        assert!(pattern.is_some());
        assert_eq!(
            pattern.unwrap().pattern_type,
            AntiPatternType::IgnoringQualityGates
        );
    }

    #[test]
    fn test_no_quality_gate_ignoring_when_passing() {
        let status = QualityGateStatus::all_passing();
        let pattern = detect_quality_gate_ignoring(&status, 5);
        assert!(pattern.is_none());
    }

    #[test]
    fn test_no_quality_gate_ignoring_below_threshold() {
        let status = QualityGateStatus::new()
            .with_tests(GateResult::fail(vec!["test failed".to_string()]))
            .with_clippy(GateResult::pass())
            .with_no_allow(GateResult::pass())
            .with_security(GateResult::pass())
            .with_docs(GateResult::pass())
            .with_timestamp();

        let pattern = detect_quality_gate_ignoring(&status, 2);
        assert!(pattern.is_none());
    }

    // Scope creep detection

    #[test]
    fn test_detect_scope_creep() {
        let files = vec![
            "src/module_a/mod.rs".to_string(),
            "src/module_b/mod.rs".to_string(),
            "src/module_c/mod.rs".to_string(),
            "src/module_d/mod.rs".to_string(),
        ];

        let pattern = detect_scope_creep(&files, 3);
        assert!(pattern.is_some());
        assert_eq!(pattern.unwrap().pattern_type, AntiPatternType::ScopeCreep);
    }

    #[test]
    fn test_no_scope_creep_focused_changes() {
        let files = vec![
            "src/module_a/mod.rs".to_string(),
            "src/module_a/types.rs".to_string(),
            "src/module_a/tests.rs".to_string(),
        ];

        let pattern = detect_scope_creep(&files, 3);
        assert!(pattern.is_none());
    }

    // Integration test

    #[test]
    fn test_multiple_patterns_detected() {
        let config = DetectorConfig {
            edit_without_commit_threshold: 2,
            tests_not_run_threshold: 2,
            error_repetition_threshold: 2,
            ..Default::default()
        };
        let mut detector = AntiPatternDetector::with_config(config);

        // Create conditions for multiple patterns
        detector.add_iteration(
            IterationSummary::new(1)
                .with_files_modified(vec!["a.rs".to_string()])
                .with_errors(vec!["E0308".to_string()]),
        );
        detector.add_iteration(
            IterationSummary::new(2)
                .with_files_modified(vec!["b.rs".to_string()])
                .with_errors(vec!["E0308".to_string()]),
        );

        let patterns = detector.detect();

        // Should detect multiple patterns
        assert!(patterns.len() >= 2);
        assert!(patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::EditWithoutCommit));
        assert!(patterns
            .iter()
            .any(|p| p.pattern_type == AntiPatternType::RepeatingErrors));
    }
}
