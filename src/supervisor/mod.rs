//! Chief Supervisor - monitors and validates automation progress.
//!
//! Inspired by Chief Wiggum, this supervisor periodically checks on the
//! automation loop and can intervene when things go wrong. It catches
//! stale loops, unhappy paths, and provides diagnostic information.
//!
//! # Modules
//!
//! - [`predictor`] - Predictive stagnation prevention with risk assessment

pub mod predictor;

use ralph::Analytics;
use crate::r#loop::state::{LoopMode, LoopState};
use anyhow::Result;
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, warn};

/// Default supervisor check interval (iterations)
pub const DEFAULT_CHECK_INTERVAL: u32 = 5;

/// Maximum health history entries to keep
const MAX_HEALTH_HISTORY: usize = 20;

/// Supervisor verdict on loop health
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SupervisorVerdict {
    /// All good, continue execution
    Proceed,
    /// Pause and request human input
    PauseForReview { reason: String },
    /// Abort the loop immediately
    Abort { reason: String },
    /// Switch to different mode
    SwitchMode { target: LoopMode, reason: String },
    /// Reset stagnation counter and retry
    Reset { reason: String },
}

impl std::fmt::Display for SupervisorVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Proceed => write!(f, "PROCEED"),
            Self::PauseForReview { reason } => write!(f, "PAUSE: {}", reason),
            Self::Abort { reason } => write!(f, "ABORT: {}", reason),
            Self::SwitchMode { target, reason } => {
                write!(f, "SWITCH to {}: {}", target, reason)
            }
            Self::Reset { reason } => write!(f, "RESET: {}", reason),
        }
    }
}

/// Health indicators for the loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Iterations since last commit
    pub iterations_since_commit: u32,
    /// Iterations since IMPLEMENTATION_PLAN.md changed
    pub iterations_since_plan_change: u32,
    /// Number of errors in recent iterations
    pub error_count_recent: u32,
    /// Test pass rate (0.0 - 1.0)
    pub test_pass_rate: f64,
    /// Number of clippy warnings
    pub clippy_warning_count: u32,
    /// Current stagnation count
    pub stagnation_count: u32,
    /// Number of mode switches in this session
    pub mode_switches: u32,
    /// Current loop mode
    pub current_mode: LoopMode,
    /// Timestamp of this measurement
    pub measured_at: DateTime<Utc>,
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            iterations_since_commit: 0,
            iterations_since_plan_change: 0,
            error_count_recent: 0,
            test_pass_rate: 1.0,
            clippy_warning_count: 0,
            stagnation_count: 0,
            mode_switches: 0,
            current_mode: LoopMode::Build,
            measured_at: Utc::now(),
        }
    }
}

/// Diagnostic report for failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    /// Git status output
    pub git_status: String,
    /// Git diff summary
    pub git_diff_summary: String,
    /// Recent commit hashes and messages
    pub recent_commits: Vec<String>,
    /// Test output (if available)
    pub test_output: Option<String>,
    /// Clippy output (if available)
    pub clippy_output: Option<String>,
    /// Recent analytics events
    pub recent_events: Vec<serde_json::Value>,
    /// Generated timestamp
    pub generated_at: DateTime<Utc>,
}

impl DiagnosticReport {
    /// Save diagnostic report to file
    pub fn save(&self, project_dir: &Path) -> Result<PathBuf> {
        let ralph_dir = project_dir.join(".ralph");
        std::fs::create_dir_all(&ralph_dir)?;

        let filename = format!("diagnostic-{}.json", self.generated_at.timestamp());
        let path = ralph_dir.join(filename);

        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(path)
    }
}

/// Detected stagnation patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StagnationPattern {
    /// Same error repeating multiple times
    RepeatingError { error: String, count: u32 },
    /// Test count or pass rate decreasing
    TestRegression { drop_percent: u32 },
    /// Oscillating between build and debug modes
    ModeOscillation { switches: u32 },
    /// No meaningful changes being made
    NoMeaningfulChanges { iterations: u32 },
    /// Clippy warnings accumulating
    AccumulatingWarnings { count: u32 },
}

impl StagnationPattern {
    /// Check if this pattern is unrecoverable
    pub fn is_unrecoverable(&self) -> bool {
        match self {
            StagnationPattern::RepeatingError { count, .. } => *count >= 3,
            StagnationPattern::ModeOscillation { switches } => *switches >= 4,
            StagnationPattern::TestRegression { drop_percent } => *drop_percent >= 50,
            _ => false,
        }
    }

    /// Get severity level (0-100)
    pub fn severity(&self) -> u32 {
        match self {
            StagnationPattern::RepeatingError { count, .. } => count * 30,
            StagnationPattern::TestRegression { drop_percent } => *drop_percent,
            StagnationPattern::ModeOscillation { switches } => switches * 20,
            StagnationPattern::NoMeaningfulChanges { iterations } => iterations * 10,
            StagnationPattern::AccumulatingWarnings { count } => count * 5,
        }
    }
}

impl std::fmt::Display for StagnationPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepeatingError { error, count } => {
                write!(f, "Same error repeated {} times: {}", count, error)
            }
            Self::TestRegression { drop_percent } => {
                write!(f, "Test pass rate dropped by {}%", drop_percent)
            }
            Self::ModeOscillation { switches } => {
                write!(f, "Mode oscillation detected ({} switches)", switches)
            }
            Self::NoMeaningfulChanges { iterations } => {
                write!(f, "No meaningful changes in {} iterations", iterations)
            }
            Self::AccumulatingWarnings { count } => {
                write!(f, "{} clippy warnings accumulating", count)
            }
        }
    }
}

/// The Chief Supervisor
pub struct Supervisor {
    project_dir: PathBuf,
    check_interval: u32,
    last_check_iteration: u32,
    health_history: VecDeque<HealthMetrics>,
    mode_switch_count: u32,
    last_error: Option<String>,
    error_repeat_count: u32,
}

impl Supervisor {
    /// Create a new supervisor
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            project_dir,
            check_interval: DEFAULT_CHECK_INTERVAL,
            last_check_iteration: 0,
            health_history: VecDeque::with_capacity(MAX_HEALTH_HISTORY),
            mode_switch_count: 0,
            last_error: None,
            error_repeat_count: 0,
        }
    }

    /// Set check interval
    pub fn with_interval(mut self, interval: u32) -> Self {
        self.check_interval = interval;
        self
    }

    /// Check if supervisor should run this iteration
    pub fn should_check(&self, iteration: u32) -> bool {
        iteration >= self.last_check_iteration + self.check_interval
    }

    /// Record an error occurrence
    pub fn record_error(&mut self, error: &str) {
        if self.last_error.as_deref() == Some(error) {
            self.error_repeat_count += 1;
        } else {
            self.last_error = Some(error.to_string());
            self.error_repeat_count = 1;
        }
    }

    /// Record a mode switch
    pub fn record_mode_switch(&mut self) {
        self.mode_switch_count += 1;
    }

    /// Run supervisor check and return verdict
    pub fn check(&mut self, state: &LoopState, iteration: u32) -> Result<SupervisorVerdict> {
        self.last_check_iteration = iteration;

        // Gather current metrics
        let metrics = self.gather_metrics(state)?;

        // Store in history
        if self.health_history.len() >= MAX_HEALTH_HISTORY {
            self.health_history.pop_front();
        }
        self.health_history.push_back(metrics.clone());

        // Analyze and decide
        self.analyze_health(&metrics, state)
    }

    /// Run supervisor check with verbose status output
    pub fn check_verbose(
        &mut self,
        state: &LoopState,
        iteration: u32,
    ) -> Result<SupervisorVerdict> {
        self.last_check_iteration = iteration;

        // Gather current metrics
        let metrics = self.gather_metrics(state)?;

        // Print status
        self.print_status(&metrics);

        // Store in history
        if self.health_history.len() >= MAX_HEALTH_HISTORY {
            self.health_history.pop_front();
        }
        self.health_history.push_back(metrics.clone());

        // Analyze and decide
        self.analyze_health(&metrics, state)
    }

    /// Get the check interval
    pub fn check_interval(&self) -> u32 {
        self.check_interval
    }

    /// Get the current mode switch count
    pub fn mode_switch_count(&self) -> u32 {
        self.mode_switch_count
    }

    /// Get the last check iteration
    pub fn last_check_iteration(&self) -> u32 {
        self.last_check_iteration
    }

    /// Gather current health metrics
    fn gather_metrics(&self, state: &LoopState) -> Result<HealthMetrics> {
        let test_pass_rate = self.get_test_pass_rate();
        let clippy_warning_count = self.get_clippy_warning_count();
        let iterations_since_commit = self.get_iterations_since_commit(state)?;

        Ok(HealthMetrics {
            iterations_since_commit,
            iterations_since_plan_change: state.stagnation_count, // Approximation
            error_count_recent: self.error_repeat_count,
            test_pass_rate,
            clippy_warning_count,
            stagnation_count: state.stagnation_count,
            mode_switches: self.mode_switch_count,
            current_mode: state.mode,
            measured_at: Utc::now(),
        })
    }

    /// Get test pass rate by running cargo test
    fn get_test_pass_rate(&self) -> f64 {
        let output = Command::new("cargo")
            .args(["test", "--", "--format=terse"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{}{}", stdout, stderr);

                // Parse test results: "X passed; Y failed; Z ignored"
                let passed = self.extract_count(&combined, "passed");
                let failed = self.extract_count(&combined, "failed");

                if passed + failed > 0 {
                    passed as f64 / (passed + failed) as f64
                } else {
                    1.0 // No tests = assume passing
                }
            }
            Err(_) => 1.0, // Can't run tests = assume passing
        }
    }

    /// Get clippy warning count
    fn get_clippy_warning_count(&self) -> u32 {
        let output = Command::new("cargo")
            .args(["clippy", "--all-targets", "--message-format=short"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                stderr.matches("warning:").count() as u32
            }
            Err(_) => 0,
        }
    }

    /// Extract count from test output (e.g., "5 passed")
    fn extract_count(&self, text: &str, label: &str) -> u32 {
        // Look for patterns like "5 passed" or "5 passed;" (handle punctuation)
        let words: Vec<&str> = text.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            // Strip trailing punctuation for comparison
            let clean_word = word.trim_end_matches(|c: char| c.is_ascii_punctuation());
            if clean_word == label && i > 0 {
                if let Ok(n) = words[i - 1].parse::<u32>() {
                    return n;
                }
            }
        }
        0
    }

    /// Get iterations since last commit
    fn get_iterations_since_commit(&self, state: &LoopState) -> Result<u32> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.project_dir)
            .output()?;

        if output.status.success() {
            let current_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if current_hash == state.last_commit_hash {
                // No new commits since state was last updated
                Ok(state.stagnation_count)
            } else {
                Ok(0)
            }
        } else {
            Ok(state.stagnation_count)
        }
    }

    /// Analyze health metrics and decide verdict
    fn analyze_health(
        &self,
        metrics: &HealthMetrics,
        state: &LoopState,
    ) -> Result<SupervisorVerdict> {
        // Check for patterns
        if let Some(pattern) = self.detect_stagnation_pattern(metrics) {
            warn!("Stagnation pattern detected: {}", pattern);

            if pattern.is_unrecoverable() {
                return Ok(SupervisorVerdict::Abort {
                    reason: format!("Unrecoverable stagnation: {}", pattern),
                });
            }

            if pattern.severity() >= 60 {
                return Ok(SupervisorVerdict::PauseForReview {
                    reason: format!("Significant issue: {}", pattern),
                });
            }
        }

        // Critical: Test suite severely degraded
        if metrics.test_pass_rate < 0.5 && metrics.iterations_since_commit > 10 {
            return Ok(SupervisorVerdict::Abort {
                reason: format!(
                    "Test suite severely degraded ({}% pass rate) with no commits in {} iterations",
                    (metrics.test_pass_rate * 100.0) as u32,
                    metrics.iterations_since_commit
                ),
            });
        }

        // High: Many clippy warnings
        if metrics.clippy_warning_count > 20 {
            return Ok(SupervisorVerdict::PauseForReview {
                reason: format!(
                    "{} clippy warnings accumulated - needs cleanup",
                    metrics.clippy_warning_count
                ),
            });
        }

        // Medium: Long time without commits
        if metrics.iterations_since_commit > 15 && state.mode == LoopMode::Build {
            return Ok(SupervisorVerdict::SwitchMode {
                target: LoopMode::Debug,
                reason: format!(
                    "No commits in {} iterations in build mode",
                    metrics.iterations_since_commit
                ),
            });
        }

        // Low: Repeating error
        if self.error_repeat_count >= 2 {
            return Ok(SupervisorVerdict::Reset {
                reason: format!(
                    "Same error repeated {} times - resetting to try fresh approach",
                    self.error_repeat_count
                ),
            });
        }

        debug!("Supervisor check passed - health OK");
        Ok(SupervisorVerdict::Proceed)
    }

    /// Detect repeating failure patterns
    fn detect_stagnation_pattern(&self, current: &HealthMetrics) -> Option<StagnationPattern> {
        // Need at least 3 data points
        if self.health_history.len() < 3 {
            return None;
        }

        // Check for repeating errors
        if self.error_repeat_count >= 3 {
            if let Some(ref error) = self.last_error {
                return Some(StagnationPattern::RepeatingError {
                    error: error.clone(),
                    count: self.error_repeat_count,
                });
            }
        }

        // Check for mode oscillation
        if self.mode_switch_count >= 4 {
            // Check if recent history shows back-and-forth
            let recent_modes: Vec<_> = self
                .health_history
                .iter()
                .rev()
                .take(6)
                .map(|m| m.current_mode)
                .collect();

            let oscillating = recent_modes
                .windows(2)
                .filter(|w| w[0] != w[1])
                .count()
                >= 3;

            if oscillating {
                return Some(StagnationPattern::ModeOscillation {
                    switches: self.mode_switch_count,
                });
            }
        }

        // Check for test regression
        if let Some(first) = self.health_history.front() {
            let drop = ((first.test_pass_rate - current.test_pass_rate) * 100.0) as u32;
            if drop >= 20 {
                return Some(StagnationPattern::TestRegression {
                    drop_percent: drop,
                });
            }
        }

        // Check for accumulating warnings
        if current.clippy_warning_count > 10 {
            return Some(StagnationPattern::AccumulatingWarnings {
                count: current.clippy_warning_count,
            });
        }

        // Check for no meaningful changes
        let all_stagnant = self
            .health_history
            .iter()
            .rev()
            .take(5)
            .all(|m| m.iterations_since_commit > 0);

        if all_stagnant && self.health_history.len() >= 5 {
            return Some(StagnationPattern::NoMeaningfulChanges {
                iterations: self.health_history.len() as u32,
            });
        }

        None
    }

    /// Generate diagnostic report
    pub fn generate_diagnostics(&self, analytics: &Analytics) -> Result<DiagnosticReport> {
        // Get git status
        let git_status = Command::new("git")
            .args(["status", "--short"])
            .current_dir(&self.project_dir)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        // Get git diff summary
        let git_diff_summary = Command::new("git")
            .args(["diff", "--stat", "HEAD"])
            .current_dir(&self.project_dir)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        // Get recent commits
        let recent_commits = Command::new("git")
            .args(["log", "--oneline", "-10"])
            .current_dir(&self.project_dir)
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Get test output
        let test_output = Command::new("cargo")
            .args(["test"])
            .current_dir(&self.project_dir)
            .output()
            .ok()
            .map(|o| {
                format!(
                    "stdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                )
            });

        // Get clippy output
        let clippy_output = Command::new("cargo")
            .args(["clippy", "--all-targets"])
            .current_dir(&self.project_dir)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stderr).to_string());

        // Get recent analytics events
        let recent_events = analytics
            .read_events()
            .unwrap_or_default()
            .into_iter()
            .rev()
            .take(20)
            .map(|e| serde_json::to_value(e).unwrap_or_default())
            .collect();

        Ok(DiagnosticReport {
            git_status,
            git_diff_summary,
            recent_commits,
            test_output,
            clippy_output,
            recent_events,
            generated_at: Utc::now(),
        })
    }

    /// Print supervisor status
    pub fn print_status(&self, metrics: &HealthMetrics) {
        println!(
            "\n{} Supervisor Health Check",
            "Chief".bright_blue()
        );
        println!("{}", "-".repeat(50));
        println!(
            "   Test Pass Rate:    {}%",
            (metrics.test_pass_rate * 100.0) as u32
        );
        println!("   Clippy Warnings:   {}", metrics.clippy_warning_count);
        println!(
            "   Since Last Commit: {} iterations",
            metrics.iterations_since_commit
        );
        println!("   Mode Switches:     {}", metrics.mode_switches);
        println!("   Stagnation Count:  {}", metrics.stagnation_count);
        println!("{}", "-".repeat(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_supervisor_creation() {
        let temp = TempDir::new().unwrap();
        let supervisor = Supervisor::new(temp.path().to_path_buf());

        assert_eq!(supervisor.check_interval(), DEFAULT_CHECK_INTERVAL);
        assert_eq!(supervisor.mode_switch_count(), 0);
    }

    #[test]
    fn test_supervisor_with_interval() {
        let temp = TempDir::new().unwrap();
        let supervisor = Supervisor::new(temp.path().to_path_buf())
            .with_interval(10);

        assert_eq!(supervisor.check_interval(), 10);
    }

    #[test]
    fn test_should_check() {
        let temp = TempDir::new().unwrap();
        let supervisor = Supervisor::new(temp.path().to_path_buf());

        assert!(supervisor.should_check(5));
        assert!(supervisor.should_check(10));
        assert!(!supervisor.should_check(3));
    }

    #[test]
    fn test_record_error() {
        let temp = TempDir::new().unwrap();
        let mut supervisor = Supervisor::new(temp.path().to_path_buf());

        supervisor.record_error("test error");
        assert_eq!(supervisor.error_repeat_count, 1);

        supervisor.record_error("test error");
        assert_eq!(supervisor.error_repeat_count, 2);

        supervisor.record_error("different error");
        assert_eq!(supervisor.error_repeat_count, 1);
    }

    #[test]
    fn test_record_mode_switch() {
        let temp = TempDir::new().unwrap();
        let mut supervisor = Supervisor::new(temp.path().to_path_buf());

        assert_eq!(supervisor.mode_switch_count(), 0);
        supervisor.record_mode_switch();
        assert_eq!(supervisor.mode_switch_count(), 1);
        supervisor.record_mode_switch();
        assert_eq!(supervisor.mode_switch_count(), 2);
    }

    #[test]
    fn test_stagnation_pattern_severity() {
        let pattern = StagnationPattern::RepeatingError {
            error: "test".into(),
            count: 3,
        };
        assert_eq!(pattern.severity(), 90);
        assert!(pattern.is_unrecoverable());

        let pattern = StagnationPattern::AccumulatingWarnings { count: 5 };
        assert_eq!(pattern.severity(), 25);
        assert!(!pattern.is_unrecoverable());
    }

    #[test]
    fn test_stagnation_pattern_is_unrecoverable() {
        // Repeating error >= 3 is unrecoverable
        assert!(StagnationPattern::RepeatingError {
            error: "test".into(),
            count: 3
        }
        .is_unrecoverable());
        assert!(!StagnationPattern::RepeatingError {
            error: "test".into(),
            count: 2
        }
        .is_unrecoverable());

        // Mode oscillation >= 4 is unrecoverable
        assert!(StagnationPattern::ModeOscillation { switches: 4 }.is_unrecoverable());
        assert!(!StagnationPattern::ModeOscillation { switches: 3 }.is_unrecoverable());

        // Test regression >= 50% is unrecoverable
        assert!(StagnationPattern::TestRegression { drop_percent: 50 }.is_unrecoverable());
        assert!(!StagnationPattern::TestRegression { drop_percent: 49 }.is_unrecoverable());

        // Other patterns are never unrecoverable
        assert!(!StagnationPattern::NoMeaningfulChanges { iterations: 100 }.is_unrecoverable());
        assert!(!StagnationPattern::AccumulatingWarnings { count: 100 }.is_unrecoverable());
    }

    #[test]
    fn test_stagnation_pattern_display() {
        let pattern = StagnationPattern::RepeatingError {
            error: "compilation failed".into(),
            count: 5,
        };
        let display = pattern.to_string();
        assert!(display.contains("5 times"));
        assert!(display.contains("compilation failed"));

        let pattern = StagnationPattern::TestRegression { drop_percent: 30 };
        assert!(pattern.to_string().contains("30%"));
    }

    #[test]
    fn test_supervisor_verdict_display() {
        let verdict = SupervisorVerdict::Proceed;
        assert_eq!(verdict.to_string(), "PROCEED");

        let verdict = SupervisorVerdict::Abort {
            reason: "test failure".into(),
        };
        assert!(verdict.to_string().contains("ABORT"));
        assert!(verdict.to_string().contains("test failure"));

        let verdict = SupervisorVerdict::SwitchMode {
            target: LoopMode::Debug,
            reason: "stagnation".into(),
        };
        let display = verdict.to_string();
        assert!(display.contains("SWITCH"));
        assert!(display.contains("debug"));
        assert!(display.contains("stagnation"));
    }

    #[test]
    fn test_health_metrics_default() {
        let metrics = HealthMetrics::default();
        assert_eq!(metrics.test_pass_rate, 1.0);
        assert_eq!(metrics.clippy_warning_count, 0);
        assert_eq!(metrics.stagnation_count, 0);
        assert_eq!(metrics.mode_switches, 0);
        assert_eq!(metrics.current_mode, LoopMode::Build);
    }

    #[test]
    fn test_extract_count() {
        let temp = TempDir::new().unwrap();
        let supervisor = Supervisor::new(temp.path().to_path_buf());

        let text = "5 passed";
        let count = supervisor.extract_count(text, "passed");
        assert_eq!(count, 5);

        let text = "10 passed; 2 failed; 0 ignored";
        assert_eq!(supervisor.extract_count(text, "passed"), 10);
        assert_eq!(supervisor.extract_count(text, "failed"), 2);
        assert_eq!(supervisor.extract_count(text, "ignored"), 0);

        // No match
        let text = "no numbers here";
        assert_eq!(supervisor.extract_count(text, "passed"), 0);
    }

    #[test]
    fn test_diagnostic_report_save() {
        let temp = TempDir::new().unwrap();
        let report = DiagnosticReport {
            git_status: "M file.rs".into(),
            git_diff_summary: "1 file changed".into(),
            recent_commits: vec!["abc123 test commit".into()],
            test_output: Some("all tests passed".into()),
            clippy_output: None,
            recent_events: vec![],
            generated_at: Utc::now(),
        };

        let path = report.save(temp.path()).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("diagnostic-"));

        // Verify the file is valid JSON
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: DiagnosticReport = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.git_status, "M file.rs");
    }

    #[test]
    fn test_supervisor_verdict_equality() {
        assert_eq!(SupervisorVerdict::Proceed, SupervisorVerdict::Proceed);
        assert_ne!(
            SupervisorVerdict::Proceed,
            SupervisorVerdict::Abort {
                reason: "test".into()
            }
        );

        assert_eq!(
            SupervisorVerdict::Abort {
                reason: "test".into()
            },
            SupervisorVerdict::Abort {
                reason: "test".into()
            }
        );
    }
}
