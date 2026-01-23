//! Predictive Stagnation Prevention.
//!
//! This module provides proactive stagnation detection through risk scoring
//! and pattern analysis. Unlike the reactive supervisor, the predictor aims
//! to identify problems *before* they become stagnation events.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │ StagnationPredictor │
//! │                     │
//! │  ┌───────────────┐  │    ┌────────────────┐
//! │  │ RiskFactors   │──┼───>│ RiskScore      │
//! │  └───────────────┘  │    │ (0-100)        │
//! │                     │    └───────┬────────┘
//! │  ┌───────────────┐  │            │
//! │  │ PatternDetect │  │            v
//! │  └───────────────┘  │    ┌────────────────┐
//! │                     │    │ PreventiveAction│
//! └─────────────────────┘    └────────────────┘
//! ```
//!
//! # Risk Factors
//!
//! The predictor monitors several signals that correlate with stagnation:
//!
//! | Factor | Weight | Description |
//! |--------|--------|-------------|
//! | Commit Gap | 0.25 | Iterations since last commit |
//! | File Churn | 0.20 | Same files edited repeatedly |
//! | Error Repeat | 0.20 | Same errors occurring |
//! | Test Stagnation | 0.15 | No new tests added |
//! | Mode Oscillation | 0.10 | Frequent mode switches |
//! | Warning Growth | 0.10 | Clippy warnings increasing |
//!
//! # Example
//!
//! ```rust
//! use ralph::supervisor::predictor::{StagnationPredictor, PredictorConfig, RiskSignals};
//!
//! let predictor = StagnationPredictor::new(PredictorConfig::default());
//!
//! let signals = RiskSignals {
//!     iterations_since_commit: 8,
//!     file_touch_counts: vec![("main.rs".into(), 5), ("lib.rs".into(), 3)],
//!     error_messages: vec!["cannot find value".into(), "cannot find value".into()],
//!     test_count_history: vec![10, 10, 10, 10],
//!     mode_switches: 2,
//!     clippy_warning_history: vec![3, 5, 7],
//! };
//!
//! let score = predictor.risk_score(&signals);
//! let level = predictor.risk_level(score);
//! let action = predictor.preventive_action(&signals, score);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Risk score range: 0-100.
pub type RiskScore = f64;

/// Risk level classification based on score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Score 0-30: Normal operation, no intervention needed.
    Low,
    /// Score 30-60: Caution, monitor closely.
    Medium,
    /// Score 60-80: Elevated risk, consider intervention.
    High,
    /// Score 80-100: Critical, immediate action required.
    Critical,
}

impl RiskLevel {
    /// Returns the minimum score for this risk level.
    #[must_use]
    pub fn min_score(&self) -> f64 {
        match self {
            Self::Low => 0.0,
            Self::Medium => 30.0,
            Self::High => 60.0,
            Self::Critical => 80.0,
        }
    }

    /// Returns the maximum score for this risk level.
    #[must_use]
    pub fn max_score(&self) -> f64 {
        match self {
            Self::Low => 30.0,
            Self::Medium => 60.0,
            Self::High => 80.0,
            Self::Critical => 100.0,
        }
    }

    /// Returns true if this level requires intervention.
    #[must_use]
    pub fn requires_intervention(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }

    /// Returns true if this level is critical.
    #[must_use]
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical)
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Weights for each risk factor.
///
/// Weights should sum to 1.0 for normalized scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskWeights {
    /// Weight for iterations since last commit (default: 0.25).
    pub commit_gap: f64,
    /// Weight for repeated file edits (default: 0.20).
    pub file_churn: f64,
    /// Weight for error repetition (default: 0.20).
    pub error_repeat: f64,
    /// Weight for test count stagnation (default: 0.15).
    pub test_stagnation: f64,
    /// Weight for mode oscillation (default: 0.10).
    pub mode_oscillation: f64,
    /// Weight for clippy warning growth (default: 0.10).
    pub warning_growth: f64,
}

impl Default for RiskWeights {
    fn default() -> Self {
        Self {
            commit_gap: 0.25,
            file_churn: 0.20,
            error_repeat: 0.20,
            test_stagnation: 0.15,
            mode_oscillation: 0.10,
            warning_growth: 0.10,
        }
    }
}

impl RiskWeights {
    /// Creates new risk weights with custom values.
    #[must_use]
    pub fn new(
        commit_gap: f64,
        file_churn: f64,
        error_repeat: f64,
        test_stagnation: f64,
        mode_oscillation: f64,
        warning_growth: f64,
    ) -> Self {
        Self {
            commit_gap,
            file_churn,
            error_repeat,
            test_stagnation,
            mode_oscillation,
            warning_growth,
        }
    }

    /// Returns the sum of all weights.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.commit_gap
            + self.file_churn
            + self.error_repeat
            + self.test_stagnation
            + self.mode_oscillation
            + self.warning_growth
    }

    /// Returns normalized weights that sum to 1.0.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let total = self.total();
        if total == 0.0 {
            return Self::default();
        }
        Self {
            commit_gap: self.commit_gap / total,
            file_churn: self.file_churn / total,
            error_repeat: self.error_repeat / total,
            test_stagnation: self.test_stagnation / total,
            mode_oscillation: self.mode_oscillation / total,
            warning_growth: self.warning_growth / total,
        }
    }

    /// Validates that the weights are valid (no negative, NaN, or infinite values).
    ///
    /// # Errors
    ///
    /// Returns an error string if any weight is invalid:
    /// - Negative values
    /// - NaN values
    /// - Infinite values
    /// - All weights are zero
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::supervisor::predictor::RiskWeights;
    ///
    /// let valid = RiskWeights::new(0.25, 0.20, 0.20, 0.15, 0.10, 0.10);
    /// assert!(valid.validate().is_ok());
    ///
    /// let invalid = RiskWeights::new(-0.1, 0.20, 0.20, 0.15, 0.10, 0.10);
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        let weights = [
            ("commit_gap", self.commit_gap),
            ("file_churn", self.file_churn),
            ("error_repeat", self.error_repeat),
            ("test_stagnation", self.test_stagnation),
            ("mode_oscillation", self.mode_oscillation),
            ("warning_growth", self.warning_growth),
        ];

        for (name, value) in weights {
            if value.is_nan() {
                return Err(format!("{} weight is NaN", name));
            }
            if value.is_infinite() {
                return Err(format!("{} weight is infinite", name));
            }
            if value < 0.0 {
                return Err(format!("{} weight is negative: {}", name, value));
            }
        }

        if self.total() == 0.0 {
            return Err("All weights are zero - at least one must be positive".to_string());
        }

        Ok(())
    }
}

/// Preset weight profiles for common use cases.
///
/// These presets provide tuned weight configurations for different
/// operational contexts:
///
/// - **Balanced**: Default weights, good for most projects
/// - **Conservative**: Emphasizes early stagnation detection (commit gap, error repeat)
/// - **Aggressive**: Tolerates more exploration, focuses on actual problems (file churn)
///
/// # Example
///
/// ```rust
/// use ralph::supervisor::predictor::{WeightPreset, StagnationPredictor};
///
/// // Create a predictor with conservative weights
/// let predictor = StagnationPredictor::with_preset(WeightPreset::Conservative);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeightPreset {
    /// Balanced weights - default configuration suitable for most projects.
    ///
    /// Weights: commit_gap=0.25, file_churn=0.20, error_repeat=0.20,
    /// test_stagnation=0.15, mode_oscillation=0.10, warning_growth=0.10
    #[default]
    Balanced,

    /// Conservative weights - catches stagnation early.
    ///
    /// Emphasizes commit gap and error repetition to trigger interventions
    /// sooner. Good for projects where early warning is preferred over
    /// allowing more exploration time.
    ///
    /// Weights: commit_gap=0.35, file_churn=0.15, error_repeat=0.25,
    /// test_stagnation=0.10, mode_oscillation=0.10, warning_growth=0.05
    Conservative,

    /// Aggressive weights - tolerates more exploration.
    ///
    /// Reduces emphasis on commit gap to allow longer exploration periods.
    /// Focuses more on file churn and warning growth as indicators of
    /// actual problems rather than just time passing.
    ///
    /// Weights: commit_gap=0.15, file_churn=0.30, error_repeat=0.15,
    /// test_stagnation=0.15, mode_oscillation=0.10, warning_growth=0.15
    Aggressive,
}

impl WeightPreset {
    /// Returns the risk weights for this preset.
    #[must_use]
    pub fn weights(&self) -> RiskWeights {
        match self {
            Self::Balanced => RiskWeights::default(),
            Self::Conservative => RiskWeights::new(0.35, 0.15, 0.25, 0.10, 0.10, 0.05),
            Self::Aggressive => RiskWeights::new(0.15, 0.30, 0.15, 0.15, 0.10, 0.15),
        }
    }
}

impl std::fmt::Display for WeightPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Balanced => write!(f, "balanced"),
            Self::Conservative => write!(f, "conservative"),
            Self::Aggressive => write!(f, "aggressive"),
        }
    }
}

impl std::str::FromStr for WeightPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "conservative" => Ok(Self::Conservative),
            "aggressive" => Ok(Self::Aggressive),
            _ => Err(format!(
                "Unknown weight preset '{}'. Valid options: balanced, conservative, aggressive",
                s
            )),
        }
    }
}

/// Thresholds for intervention decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterventionThresholds {
    /// Score below which risk is considered low (default: 30).
    pub low_max: f64,
    /// Score below which risk is considered medium (default: 60).
    pub medium_max: f64,
    /// Score below which risk is considered high (default: 80).
    pub high_max: f64,
}

impl Default for InterventionThresholds {
    fn default() -> Self {
        Self {
            low_max: 30.0,
            medium_max: 60.0,
            high_max: 80.0,
        }
    }
}

impl InterventionThresholds {
    /// Creates new thresholds with custom values.
    #[must_use]
    pub fn new(low_max: f64, medium_max: f64, high_max: f64) -> Self {
        Self {
            low_max,
            medium_max,
            high_max,
        }
    }

    /// Classify a risk score into a risk level.
    #[must_use]
    pub fn classify(&self, score: f64) -> RiskLevel {
        if score < self.low_max {
            RiskLevel::Low
        } else if score < self.medium_max {
            RiskLevel::Medium
        } else if score < self.high_max {
            RiskLevel::High
        } else {
            RiskLevel::Critical
        }
    }
}

/// Configuration for the stagnation predictor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorConfig {
    /// Weights for risk factors.
    pub weights: RiskWeights,
    /// Thresholds for intervention levels.
    pub thresholds: InterventionThresholds,
    /// Maximum iterations without commit before full risk (default: 15).
    pub max_commit_gap: u32,
    /// Maximum file touches before full risk (default: 5).
    pub max_file_touches: u32,
    /// Maximum error repeats before full risk (default: 3).
    pub max_error_repeats: u32,
    /// Maximum stagnant iterations for test count (default: 5).
    pub max_test_stagnation: u32,
    /// Maximum mode switches before full risk (default: 4).
    pub max_mode_switches: u32,
    /// History length for tracking patterns (default: 10).
    pub history_length: usize,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            weights: RiskWeights::default(),
            thresholds: InterventionThresholds::default(),
            max_commit_gap: 15,
            max_file_touches: 5,
            max_error_repeats: 3,
            max_test_stagnation: 5,
            max_mode_switches: 4,
            history_length: 10,
        }
    }
}

impl PredictorConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new configuration with weights from a preset.
    ///
    /// # Arguments
    ///
    /// * `preset` - The weight preset to use.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::supervisor::predictor::{PredictorConfig, WeightPreset};
    ///
    /// let config = PredictorConfig::with_preset(WeightPreset::Conservative);
    /// ```
    #[must_use]
    pub fn with_preset(preset: WeightPreset) -> Self {
        Self {
            weights: preset.weights(),
            ..Self::default()
        }
    }

    /// Sets the risk weights.
    #[must_use]
    pub fn with_weights(mut self, weights: RiskWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Sets the intervention thresholds.
    #[must_use]
    pub fn with_thresholds(mut self, thresholds: InterventionThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Sets the maximum commit gap before full risk.
    #[must_use]
    pub fn with_max_commit_gap(mut self, gap: u32) -> Self {
        self.max_commit_gap = gap;
        self
    }

    /// Sets the maximum file touches before full risk.
    #[must_use]
    pub fn with_max_file_touches(mut self, touches: u32) -> Self {
        self.max_file_touches = touches;
        self
    }

    /// Sets the history length.
    #[must_use]
    pub fn with_history_length(mut self, length: usize) -> Self {
        self.history_length = length;
        self
    }
}

/// Input signals for risk assessment.
///
/// These signals are collected from the loop state and used to calculate
/// the overall stagnation risk.
#[derive(Debug, Clone, Default)]
pub struct RiskSignals {
    /// Number of iterations since the last commit.
    pub iterations_since_commit: u32,
    /// File paths and how many times each has been touched.
    pub file_touch_counts: Vec<(String, u32)>,
    /// Recent error messages (for repetition detection).
    pub error_messages: Vec<String>,
    /// Test count history (recent values, oldest first).
    pub test_count_history: Vec<u32>,
    /// Number of mode switches in this session.
    pub mode_switches: u32,
    /// Clippy warning count history (recent values, oldest first).
    pub clippy_warning_history: Vec<u32>,
}

impl RiskSignals {
    /// Creates empty risk signals.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the iterations since commit.
    #[must_use]
    pub fn with_commit_gap(mut self, gap: u32) -> Self {
        self.iterations_since_commit = gap;
        self
    }

    /// Sets the file touch counts.
    #[must_use]
    pub fn with_file_touches(mut self, touches: Vec<(String, u32)>) -> Self {
        self.file_touch_counts = touches;
        self
    }

    /// Sets the error messages.
    #[must_use]
    pub fn with_errors(mut self, errors: Vec<String>) -> Self {
        self.error_messages = errors;
        self
    }

    /// Sets the test count history.
    #[must_use]
    pub fn with_test_history(mut self, history: Vec<u32>) -> Self {
        self.test_count_history = history;
        self
    }

    /// Sets the mode switch count.
    #[must_use]
    pub fn with_mode_switches(mut self, switches: u32) -> Self {
        self.mode_switches = switches;
        self
    }

    /// Sets the clippy warning history.
    #[must_use]
    pub fn with_warning_history(mut self, history: Vec<u32>) -> Self {
        self.clippy_warning_history = history;
        self
    }
}

/// Breakdown of individual risk factor contributions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskBreakdown {
    /// Contribution from commit gap (0-100 scaled by weight).
    pub commit_gap_contribution: f64,
    /// Contribution from file churn (0-100 scaled by weight).
    pub file_churn_contribution: f64,
    /// Contribution from error repetition (0-100 scaled by weight).
    pub error_repeat_contribution: f64,
    /// Contribution from test stagnation (0-100 scaled by weight).
    pub test_stagnation_contribution: f64,
    /// Contribution from mode oscillation (0-100 scaled by weight).
    pub mode_oscillation_contribution: f64,
    /// Contribution from warning growth (0-100 scaled by weight).
    pub warning_growth_contribution: f64,
    /// Total weighted score.
    pub total: f64,
}

impl RiskBreakdown {
    /// Returns the dominant risk factor.
    #[must_use]
    pub fn dominant_factor(&self) -> &'static str {
        let factors = [
            (self.commit_gap_contribution, "commit_gap"),
            (self.file_churn_contribution, "file_churn"),
            (self.error_repeat_contribution, "error_repeat"),
            (self.test_stagnation_contribution, "test_stagnation"),
            (self.mode_oscillation_contribution, "mode_oscillation"),
            (self.warning_growth_contribution, "warning_growth"),
        ];
        factors
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, name)| *name)
            .unwrap_or("unknown")
    }

    /// Returns a human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Risk={:.0}: commit_gap={:.0}, file_churn={:.0}, error_repeat={:.0}, test_stagnation={:.0}, mode_oscillation={:.0}, warning_growth={:.0}",
            self.total,
            self.commit_gap_contribution,
            self.file_churn_contribution,
            self.error_repeat_contribution,
            self.test_stagnation_contribution,
            self.mode_oscillation_contribution,
            self.warning_growth_contribution
        )
    }
}

/// Statistics about prediction accuracy for analytics integration.
///
/// This struct provides a serializable summary of prediction performance
/// that can be stored in analytics events for trend analysis.
///
/// # Example
///
/// ```rust
/// use ralph::supervisor::predictor::{StagnationPredictor, PredictorConfig};
///
/// let mut predictor = StagnationPredictor::with_defaults();
/// predictor.record_prediction(50.0, true);
/// predictor.record_prediction(20.0, false);
///
/// let stats = predictor.prediction_statistics();
/// assert_eq!(stats.total_predictions, 2);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PredictionStatistics {
    /// Total number of predictions recorded.
    pub total_predictions: usize,
    /// Number of predictions at each risk level.
    pub predictions_by_level: HashMap<RiskLevel, usize>,
    /// Accuracy at each risk level (None if no predictions at that level).
    pub accuracy_by_level: HashMap<RiskLevel, Option<f64>>,
    /// Overall prediction accuracy (None if no predictions).
    pub overall_accuracy: Option<f64>,
    /// Number of correct predictions.
    pub correct_predictions: usize,
    /// Number of predictions where high risk led to stagnation.
    pub true_positives: usize,
    /// Number of predictions where low risk led to no stagnation.
    pub true_negatives: usize,
    /// Number of predictions where high risk led to no stagnation.
    pub false_positives: usize,
    /// Number of predictions where low risk led to stagnation.
    pub false_negatives: usize,
}


/// Preventive actions that can be taken to avoid stagnation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreventiveAction {
    /// No action needed, continue normally.
    None,
    /// Add guidance to the prompt about the risk.
    InjectGuidance {
        /// The guidance message to inject.
        guidance: String,
    },
    /// Focus on a single actionable item.
    FocusTask {
        /// Description of the focused task.
        task: String,
    },
    /// Suggest running tests to verify progress.
    RunTests,
    /// Suggest committing current work.
    SuggestCommit,
    /// Switch to a different mode.
    SwitchMode {
        /// Target mode to switch to.
        target: String,
    },
    /// Request human review.
    RequestReview {
        /// Reason for requesting review.
        reason: String,
    },
}

impl std::fmt::Display for PreventiveAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::InjectGuidance { guidance } => write!(f, "inject_guidance: {}", guidance),
            Self::FocusTask { task } => write!(f, "focus_task: {}", task),
            Self::RunTests => write!(f, "run_tests"),
            Self::SuggestCommit => write!(f, "suggest_commit"),
            Self::SwitchMode { target } => write!(f, "switch_mode: {}", target),
            Self::RequestReview { reason } => write!(f, "request_review: {}", reason),
        }
    }
}

/// Stagnation predictor with risk assessment and pattern detection.
#[derive(Debug, Clone)]
pub struct StagnationPredictor {
    /// Configuration for the predictor.
    config: PredictorConfig,
    /// History of recent predictions for accuracy tracking.
    prediction_history: Vec<(RiskScore, bool)>,
}

impl StagnationPredictor {
    /// Creates a new predictor with the given configuration.
    #[must_use]
    pub fn new(config: PredictorConfig) -> Self {
        Self {
            config,
            prediction_history: Vec::new(),
        }
    }

    /// Creates a new predictor with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(PredictorConfig::default())
    }

    /// Creates a new predictor with a weight preset.
    ///
    /// # Arguments
    ///
    /// * `preset` - The weight preset to use.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::supervisor::predictor::{StagnationPredictor, WeightPreset};
    ///
    /// let predictor = StagnationPredictor::with_preset(WeightPreset::Conservative);
    /// ```
    #[must_use]
    pub fn with_preset(preset: WeightPreset) -> Self {
        Self::new(PredictorConfig::with_preset(preset))
    }

    /// Returns a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &PredictorConfig {
        &self.config
    }

    /// Calculate the overall risk score from signals.
    ///
    /// Returns a score from 0-100 where higher values indicate greater risk.
    ///
    /// # Arguments
    ///
    /// * `signals` - The current risk signals to evaluate.
    ///
    /// # Returns
    ///
    /// A risk score from 0.0 to 100.0.
    #[must_use]
    pub fn risk_score(&self, signals: &RiskSignals) -> RiskScore {
        self.risk_breakdown(signals).total
    }

    /// Calculate detailed risk breakdown with individual factor contributions.
    ///
    /// # Arguments
    ///
    /// * `signals` - The current risk signals to evaluate.
    ///
    /// # Returns
    ///
    /// A `RiskBreakdown` with individual factor contributions.
    #[must_use]
    pub fn risk_breakdown(&self, signals: &RiskSignals) -> RiskBreakdown {
        let weights = self.config.weights.normalized();

        // Calculate individual factor scores (0-100 each)
        let commit_gap_raw = self.score_commit_gap(signals.iterations_since_commit);
        let file_churn_raw = self.score_file_churn(&signals.file_touch_counts);
        let error_repeat_raw = self.score_error_repetition(&signals.error_messages);
        let test_stagnation_raw = self.score_test_stagnation(&signals.test_count_history);
        let mode_oscillation_raw = self.score_mode_oscillation(signals.mode_switches);
        let warning_growth_raw = self.score_warning_growth(&signals.clippy_warning_history);

        // Apply weights
        let commit_gap_contribution = commit_gap_raw * weights.commit_gap * 100.0;
        let file_churn_contribution = file_churn_raw * weights.file_churn * 100.0;
        let error_repeat_contribution = error_repeat_raw * weights.error_repeat * 100.0;
        let test_stagnation_contribution = test_stagnation_raw * weights.test_stagnation * 100.0;
        let mode_oscillation_contribution = mode_oscillation_raw * weights.mode_oscillation * 100.0;
        let warning_growth_contribution = warning_growth_raw * weights.warning_growth * 100.0;

        let total = commit_gap_contribution
            + file_churn_contribution
            + error_repeat_contribution
            + test_stagnation_contribution
            + mode_oscillation_contribution
            + warning_growth_contribution;

        RiskBreakdown {
            commit_gap_contribution,
            file_churn_contribution,
            error_repeat_contribution,
            test_stagnation_contribution,
            mode_oscillation_contribution,
            warning_growth_contribution,
            total: total.min(100.0),
        }
    }

    /// Classify a risk score into a risk level.
    #[must_use]
    pub fn risk_level(&self, score: RiskScore) -> RiskLevel {
        self.config.thresholds.classify(score)
    }

    /// Evaluate signals and return the risk level directly.
    #[must_use]
    pub fn evaluate(&self, signals: &RiskSignals) -> RiskLevel {
        let score = self.risk_score(signals);
        self.risk_level(score)
    }

    // =========================================================================
    // Phase 6.2: Pattern Detection Methods
    // =========================================================================

    /// Detect repeated file touches pattern.
    ///
    /// Returns 0.0 if no files are repeatedly touched, up to 1.0 if many
    /// files are being touched repeatedly without progress.
    #[must_use]
    pub fn repeated_file_touches(&self, touches: &[(String, u32)]) -> f64 {
        self.score_file_churn(touches)
    }

    /// Detect test count stagnation.
    ///
    /// Returns the number of iterations the test count has remained unchanged.
    ///
    /// # Arguments
    ///
    /// * `history` - Test count history (oldest first).
    ///
    /// # Returns
    ///
    /// Number of iterations with unchanged test count.
    #[must_use]
    pub fn test_count_stagnant_for(&self, history: &[u32]) -> u32 {
        if history.len() < 2 {
            return 0;
        }

        let last = *history.last().unwrap_or(&0);
        let mut stagnant_count = 0u32;

        for count in history.iter().rev().skip(1) {
            if *count == last {
                stagnant_count += 1;
            } else {
                break;
            }
        }

        stagnant_count
    }

    /// Calculate error repetition rate.
    ///
    /// Returns 0.0 if all errors are unique, up to 1.0 if all errors are identical.
    ///
    /// # Arguments
    ///
    /// * `errors` - Recent error messages.
    ///
    /// # Returns
    ///
    /// Repetition rate from 0.0 to 1.0.
    #[must_use]
    pub fn error_repetition_rate(&self, errors: &[String]) -> f64 {
        self.score_error_repetition(errors)
    }

    /// Detect recent mode switches.
    ///
    /// Returns true if there have been mode switches in the recent history.
    #[must_use]
    pub fn recent_mode_switch(&self, mode_switches: u32, threshold: u32) -> bool {
        mode_switches >= threshold
    }

    // =========================================================================
    // Phase 6.3: Preventive Actions
    // =========================================================================

    /// Determine the appropriate preventive action based on signals and score.
    ///
    /// # Arguments
    ///
    /// * `signals` - The current risk signals.
    /// * `score` - The calculated risk score.
    ///
    /// # Returns
    ///
    /// The recommended preventive action.
    #[must_use]
    pub fn preventive_action(&self, signals: &RiskSignals, score: RiskScore) -> PreventiveAction {
        let level = self.risk_level(score);
        let breakdown = self.risk_breakdown(signals);

        match level {
            RiskLevel::Low => PreventiveAction::None,
            RiskLevel::Medium => self.medium_risk_action(&breakdown, signals),
            RiskLevel::High => self.high_risk_action(&breakdown, signals),
            RiskLevel::Critical => self.critical_risk_action(&breakdown, signals),
        }
    }

    /// Generate unstick guidance based on the dominant risk factor.
    ///
    /// # Arguments
    ///
    /// * `breakdown` - The risk breakdown showing factor contributions.
    ///
    /// # Returns
    ///
    /// Guidance text to help unstick the loop.
    #[must_use]
    pub fn generate_unstick_guidance(&self, breakdown: &RiskBreakdown) -> String {
        let dominant = breakdown.dominant_factor();

        match dominant {
            "commit_gap" => "Consider committing your current progress, even if incomplete. \
                Small, incremental commits are better than large, delayed ones. \
                If there are blocking issues, document them and commit what works."
                .to_string(),
            "file_churn" => "You're editing the same files repeatedly. This often indicates \
                an unclear goal or approach. Step back and clarify: what exactly \
                needs to change? Write a brief plan before making more edits."
                .to_string(),
            "error_repeat" => "The same error keeps occurring. Don't keep trying the same fix. \
                Instead: 1) Read the full error carefully, 2) Search for similar \
                issues, 3) Try a completely different approach."
                .to_string(),
            "test_stagnation" => {
                "No new tests have been added recently. Tests help verify progress \
                and catch regressions. Write a test for the current functionality \
                before moving on."
                    .to_string()
            }
            "mode_oscillation" => "Frequent mode switches suggest uncertainty about the approach. \
                Pick one mode and commit to it for at least 5 iterations. If in \
                build mode, focus on implementation. If in debug mode, focus on \
                fixing the specific issue."
                .to_string(),
            "warning_growth" => "Clippy warnings are accumulating. Address them now rather than \
                later. Run 'cargo clippy --fix' for auto-fixable issues, then \
                manually address the rest."
                .to_string(),
            _ => "Progress has stalled. Consider: 1) What's the smallest possible \
                next step? 2) Is there a test you can write? 3) Should you commit \
                what you have so far?"
                .to_string(),
        }
    }

    /// Identify a single actionable item from the current signals.
    ///
    /// # Arguments
    ///
    /// * `signals` - The current risk signals.
    /// * `breakdown` - The risk breakdown.
    ///
    /// # Returns
    ///
    /// A single, focused task description.
    #[must_use]
    pub fn identify_single_actionable_item(
        &self,
        signals: &RiskSignals,
        breakdown: &RiskBreakdown,
    ) -> String {
        let dominant = breakdown.dominant_factor();

        match dominant {
            "commit_gap" => "Make a commit with your current changes".to_string(),
            "file_churn" => {
                if let Some((file, count)) = signals.file_touch_counts.iter().max_by_key(|(_, c)| c)
                {
                    format!(
                        "Stop editing {} (touched {} times) and verify it works",
                        file, count
                    )
                } else {
                    "Verify your changes work before editing more files".to_string()
                }
            }
            "error_repeat" => {
                if let Some(error) = signals.error_messages.first() {
                    let short_error = if error.len() > 50 {
                        format!("{}...", &error[..50])
                    } else {
                        error.clone()
                    };
                    format!("Fix this error with a different approach: {}", short_error)
                } else {
                    "Try a different approach to fix the recurring error".to_string()
                }
            }
            "test_stagnation" => "Write one test for your current functionality".to_string(),
            "mode_oscillation" => "Stay in current mode for 5 more iterations".to_string(),
            "warning_growth" => "Fix the highest-priority clippy warning".to_string(),
            _ => "Complete the smallest possible unit of work".to_string(),
        }
    }

    /// Record a prediction for accuracy tracking.
    ///
    /// # Arguments
    ///
    /// * `score` - The predicted risk score.
    /// * `actually_stagnated` - Whether stagnation actually occurred.
    pub fn record_prediction(&mut self, score: RiskScore, actually_stagnated: bool) {
        self.prediction_history.push((score, actually_stagnated));

        // Keep history bounded
        if self.prediction_history.len() > self.config.history_length * 10 {
            self.prediction_history.drain(0..self.config.history_length);
        }
    }

    /// Calculate prediction accuracy.
    ///
    /// # Returns
    ///
    /// Accuracy as a value from 0.0 to 1.0, or None if no predictions recorded.
    #[must_use]
    pub fn prediction_accuracy(&self) -> Option<f64> {
        if self.prediction_history.is_empty() {
            return None;
        }

        let high_threshold = self.config.thresholds.medium_max;
        let correct = self
            .prediction_history
            .iter()
            .filter(|(score, stagnated)| {
                let predicted_stagnation = *score >= high_threshold;
                predicted_stagnation == *stagnated
            })
            .count();

        Some(correct as f64 / self.prediction_history.len() as f64)
    }

    /// Returns a summary of the predictor state.
    #[must_use]
    pub fn summary(&self) -> String {
        let accuracy = self
            .prediction_accuracy()
            .map(|a| format!("{:.0}%", a * 100.0))
            .unwrap_or_else(|| "N/A".to_string());

        format!(
            "Predictor: {} predictions, accuracy={}",
            self.prediction_history.len(),
            accuracy
        )
    }

    // =========================================================================
    // Phase 10.2: Prediction Accuracy Tracking
    // =========================================================================

    /// Calculate prediction accuracy broken down by risk level.
    ///
    /// For each risk level, calculates the accuracy of predictions where:
    /// - Low/Medium risk: correct if no stagnation occurred
    /// - High/Critical risk: correct if stagnation occurred
    ///
    /// # Returns
    ///
    /// A `HashMap` with accuracy for each risk level. `None` value indicates
    /// no predictions were made at that level.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::supervisor::predictor::{StagnationPredictor, RiskLevel};
    ///
    /// let mut predictor = StagnationPredictor::with_defaults();
    /// predictor.record_prediction(70.0, true);  // High risk, stagnated (correct)
    /// predictor.record_prediction(20.0, false); // Low risk, no stagnation (correct)
    ///
    /// let breakdown = predictor.prediction_accuracy_by_level();
    /// assert_eq!(breakdown.get(&RiskLevel::High).unwrap(), &Some(1.0));
    /// assert_eq!(breakdown.get(&RiskLevel::Low).unwrap(), &Some(1.0));
    /// ```
    #[must_use]
    pub fn prediction_accuracy_by_level(&self) -> HashMap<RiskLevel, Option<f64>> {
        let mut result = HashMap::new();
        result.insert(RiskLevel::Low, None);
        result.insert(RiskLevel::Medium, None);
        result.insert(RiskLevel::High, None);
        result.insert(RiskLevel::Critical, None);

        if self.prediction_history.is_empty() {
            return result;
        }

        // Group predictions by risk level
        let mut by_level: HashMap<RiskLevel, Vec<(RiskScore, bool)>> = HashMap::new();
        for &(score, stagnated) in &self.prediction_history {
            let level = self.config.thresholds.classify(score);
            by_level.entry(level).or_default().push((score, stagnated));
        }

        // Calculate accuracy for each level
        for (level, predictions) in by_level {
            if predictions.is_empty() {
                continue;
            }

            let correct = predictions
                .iter()
                .filter(|(score, stagnated)| {
                    let predicted_high_risk = *score >= self.config.thresholds.medium_max;
                    predicted_high_risk == *stagnated
                })
                .count();

            result.insert(level, Some(correct as f64 / predictions.len() as f64));
        }

        result
    }

    /// Get comprehensive prediction statistics for analytics.
    ///
    /// Returns a serializable `PredictionStatistics` struct containing:
    /// - Total predictions and breakdown by level
    /// - Accuracy metrics (overall and by level)
    /// - Confusion matrix values (true/false positives/negatives)
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::supervisor::predictor::StagnationPredictor;
    ///
    /// let mut predictor = StagnationPredictor::with_defaults();
    /// predictor.record_prediction(50.0, false);
    /// predictor.record_prediction(70.0, true);
    ///
    /// let stats = predictor.prediction_statistics();
    /// assert_eq!(stats.total_predictions, 2);
    /// ```
    #[must_use]
    pub fn prediction_statistics(&self) -> PredictionStatistics {
        if self.prediction_history.is_empty() {
            return PredictionStatistics::default();
        }

        let mut stats = PredictionStatistics {
            total_predictions: self.prediction_history.len(),
            predictions_by_level: HashMap::new(),
            accuracy_by_level: self.prediction_accuracy_by_level(),
            overall_accuracy: self.prediction_accuracy(),
            correct_predictions: 0,
            true_positives: 0,
            true_negatives: 0,
            false_positives: 0,
            false_negatives: 0,
        };

        let high_threshold = self.config.thresholds.medium_max;

        for &(score, stagnated) in &self.prediction_history {
            // Count by level
            let level = self.config.thresholds.classify(score);
            *stats.predictions_by_level.entry(level).or_insert(0) += 1;

            // Calculate confusion matrix
            let predicted_high_risk = score >= high_threshold;
            match (predicted_high_risk, stagnated) {
                (true, true) => {
                    stats.true_positives += 1;
                    stats.correct_predictions += 1;
                }
                (false, false) => {
                    stats.true_negatives += 1;
                    stats.correct_predictions += 1;
                }
                (true, false) => stats.false_positives += 1,
                (false, true) => stats.false_negatives += 1,
            }
        }

        stats
    }

    /// Returns a reference to the prediction history.
    ///
    /// Each entry is a tuple of (risk_score, actually_stagnated).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::supervisor::predictor::StagnationPredictor;
    ///
    /// let mut predictor = StagnationPredictor::with_defaults();
    /// predictor.record_prediction(50.0, true);
    ///
    /// let history = predictor.prediction_history();
    /// assert_eq!(history.len(), 1);
    /// assert_eq!(history[0], (50.0, true));
    /// ```
    #[must_use]
    pub fn prediction_history(&self) -> &[(RiskScore, bool)] {
        &self.prediction_history
    }

    // =========================================================================
    // Private: Individual Factor Scoring (0.0 to 1.0)
    // =========================================================================

    fn score_commit_gap(&self, iterations: u32) -> f64 {
        let max = self.config.max_commit_gap as f64;
        (iterations as f64 / max).min(1.0)
    }

    fn score_file_churn(&self, touches: &[(String, u32)]) -> f64 {
        if touches.is_empty() {
            return 0.0;
        }

        let max_touches = self.config.max_file_touches as f64;
        let high_churn_files = touches
            .iter()
            .filter(|(_, count)| *count >= self.config.max_file_touches)
            .count();

        let avg_churn = touches.iter().map(|(_, c)| *c as f64).sum::<f64>() / touches.len() as f64;

        // Combine: high churn file count + average churn
        let churn_ratio = (high_churn_files as f64 / touches.len() as f64) * 0.5
            + (avg_churn / max_touches).min(1.0) * 0.5;

        churn_ratio.min(1.0)
    }

    fn score_error_repetition(&self, errors: &[String]) -> f64 {
        if errors.len() < 2 {
            return 0.0;
        }

        // Count error frequencies
        let mut counts: HashMap<&str, u32> = HashMap::new();
        for error in errors {
            // Normalize: take first 100 chars to group similar errors
            let key = if error.len() > 100 {
                &error[..100]
            } else {
                error.as_str()
            };
            *counts.entry(key).or_insert(0) += 1;
        }

        // Find max repetition
        let max_repeat = *counts.values().max().unwrap_or(&0);
        let max_allowed = self.config.max_error_repeats as f64;

        (max_repeat as f64 / max_allowed).min(1.0)
    }

    fn score_test_stagnation(&self, history: &[u32]) -> f64 {
        let stagnant_count = self.test_count_stagnant_for(history);
        let max_stagnation = self.config.max_test_stagnation as f64;

        (stagnant_count as f64 / max_stagnation).min(1.0)
    }

    fn score_mode_oscillation(&self, switches: u32) -> f64 {
        let max_switches = self.config.max_mode_switches as f64;
        (switches as f64 / max_switches).min(1.0)
    }

    fn score_warning_growth(&self, history: &[u32]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        // Calculate trend: positive = warnings increasing
        let first = history.first().copied().unwrap_or(0) as f64;
        let last = history.last().copied().unwrap_or(0) as f64;

        if first == 0.0 {
            // Started with no warnings, any increase is concerning
            return (last / 10.0).min(1.0);
        }

        let growth_rate = (last - first) / first;

        // Clamp to 0-1: 0% growth = 0.0, 100%+ growth = 1.0
        growth_rate.clamp(0.0, 1.0)
    }

    // =========================================================================
    // Private: Action Selection
    // =========================================================================

    fn medium_risk_action(
        &self,
        breakdown: &RiskBreakdown,
        _signals: &RiskSignals,
    ) -> PreventiveAction {
        // At medium risk, inject guidance based on dominant factor
        let guidance = self.generate_unstick_guidance(breakdown);
        PreventiveAction::InjectGuidance { guidance }
    }

    fn high_risk_action(
        &self,
        breakdown: &RiskBreakdown,
        signals: &RiskSignals,
    ) -> PreventiveAction {
        let dominant = breakdown.dominant_factor();

        match dominant {
            "commit_gap" => PreventiveAction::SuggestCommit,
            "test_stagnation" => PreventiveAction::RunTests,
            "mode_oscillation" => PreventiveAction::SwitchMode {
                target: "debug".to_string(),
            },
            _ => {
                let task = self.identify_single_actionable_item(signals, breakdown);
                PreventiveAction::FocusTask { task }
            }
        }
    }

    fn critical_risk_action(
        &self,
        breakdown: &RiskBreakdown,
        _signals: &RiskSignals,
    ) -> PreventiveAction {
        PreventiveAction::RequestReview {
            reason: format!(
                "Critical stagnation risk (score={:.0}, dominant_factor={}). \
                Multiple intervention attempts have not resolved the issue.",
                breakdown.total,
                breakdown.dominant_factor()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Phase 6.1: Risk Model Tests
    // =========================================================================

    #[test]
    fn test_risk_level_classification() {
        let thresholds = InterventionThresholds::default();

        assert_eq!(thresholds.classify(0.0), RiskLevel::Low);
        assert_eq!(thresholds.classify(29.9), RiskLevel::Low);
        assert_eq!(thresholds.classify(30.0), RiskLevel::Medium);
        assert_eq!(thresholds.classify(59.9), RiskLevel::Medium);
        assert_eq!(thresholds.classify(60.0), RiskLevel::High);
        assert_eq!(thresholds.classify(79.9), RiskLevel::High);
        assert_eq!(thresholds.classify(80.0), RiskLevel::Critical);
        assert_eq!(thresholds.classify(100.0), RiskLevel::Critical);
    }

    #[test]
    fn test_risk_level_properties() {
        assert!(!RiskLevel::Low.requires_intervention());
        assert!(!RiskLevel::Medium.requires_intervention());
        assert!(RiskLevel::High.requires_intervention());
        assert!(RiskLevel::Critical.requires_intervention());

        assert!(!RiskLevel::Low.is_critical());
        assert!(!RiskLevel::High.is_critical());
        assert!(RiskLevel::Critical.is_critical());
    }

    #[test]
    fn test_risk_weights_normalization() {
        let weights = RiskWeights::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
        let normalized = weights.normalized();

        let total = normalized.commit_gap
            + normalized.file_churn
            + normalized.error_repeat
            + normalized.test_stagnation
            + normalized.mode_oscillation
            + normalized.warning_growth;

        assert!((total - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_risk_weights_default_sums_to_one() {
        let weights = RiskWeights::default();
        let total = weights.total();

        assert!((total - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_predictor_low_risk_signals() {
        let predictor = StagnationPredictor::with_defaults();

        let signals = RiskSignals::new()
            .with_commit_gap(2)
            .with_file_touches(vec![("main.rs".into(), 1)])
            .with_errors(vec!["error 1".into(), "error 2".into()])
            .with_test_history(vec![10, 11, 12])
            .with_mode_switches(0)
            .with_warning_history(vec![0, 0, 0]);

        let score = predictor.risk_score(&signals);
        let level = predictor.risk_level(score);

        assert!(score < 30.0, "Score {} should be low", score);
        assert_eq!(level, RiskLevel::Low);
    }

    #[test]
    fn test_predictor_high_risk_signals() {
        let predictor = StagnationPredictor::with_defaults();

        let signals = RiskSignals::new()
            .with_commit_gap(12)
            .with_file_touches(vec![("main.rs".into(), 6), ("lib.rs".into(), 5)])
            .with_errors(vec![
                "same error".into(),
                "same error".into(),
                "same error".into(),
            ])
            .with_test_history(vec![10, 10, 10, 10, 10])
            .with_mode_switches(3)
            .with_warning_history(vec![5, 8, 12]);

        let score = predictor.risk_score(&signals);
        let level = predictor.risk_level(score);

        assert!(score >= 60.0, "Score {} should be high", score);
        assert!(level.requires_intervention());
    }

    #[test]
    fn test_risk_breakdown_dominant_factor() {
        let predictor = StagnationPredictor::with_defaults();

        // High commit gap, low everything else
        let signals = RiskSignals::new()
            .with_commit_gap(15)
            .with_file_touches(vec![])
            .with_errors(vec![])
            .with_test_history(vec![10, 11, 12])
            .with_mode_switches(0)
            .with_warning_history(vec![]);

        let breakdown = predictor.risk_breakdown(&signals);
        assert_eq!(breakdown.dominant_factor(), "commit_gap");
    }

    #[test]
    fn test_risk_breakdown_summary() {
        let predictor = StagnationPredictor::with_defaults();
        let signals = RiskSignals::new().with_commit_gap(10);

        let breakdown = predictor.risk_breakdown(&signals);
        let summary = breakdown.summary();

        assert!(summary.contains("Risk="));
        assert!(summary.contains("commit_gap="));
    }

    #[test]
    fn test_predictor_config_builder() {
        let config = PredictorConfig::new()
            .with_max_commit_gap(20)
            .with_max_file_touches(10)
            .with_history_length(20);

        assert_eq!(config.max_commit_gap, 20);
        assert_eq!(config.max_file_touches, 10);
        assert_eq!(config.history_length, 20);
    }

    // =========================================================================
    // Phase 6.2: Pattern Detection Tests
    // =========================================================================

    #[test]
    fn test_repeated_file_touches() {
        let predictor = StagnationPredictor::with_defaults();

        let low_churn = vec![("a.rs".into(), 1), ("b.rs".into(), 2)];
        assert!(predictor.repeated_file_touches(&low_churn) < 0.5);

        let high_churn = vec![("a.rs".into(), 10), ("b.rs".into(), 8)];
        assert!(predictor.repeated_file_touches(&high_churn) > 0.5);
    }

    #[test]
    fn test_test_count_stagnant_for() {
        let predictor = StagnationPredictor::with_defaults();

        // Increasing: no stagnation
        let increasing = vec![10, 11, 12, 13];
        assert_eq!(predictor.test_count_stagnant_for(&increasing), 0);

        // Stagnant: 3 iterations at same count
        let stagnant = vec![10, 12, 15, 15, 15];
        assert_eq!(predictor.test_count_stagnant_for(&stagnant), 2);

        // All same: all stagnant
        let all_same = vec![10, 10, 10, 10, 10];
        assert_eq!(predictor.test_count_stagnant_for(&all_same), 4);
    }

    #[test]
    fn test_error_repetition_rate() {
        let predictor = StagnationPredictor::with_defaults();

        // Unique errors: low rate
        let unique = vec!["error 1".into(), "error 2".into(), "error 3".into()];
        assert!(predictor.error_repetition_rate(&unique) < 0.5);

        // Repeated errors: high rate
        let repeated = vec![
            "same error".into(),
            "same error".into(),
            "same error".into(),
        ];
        assert_eq!(predictor.error_repetition_rate(&repeated), 1.0);
    }

    #[test]
    fn test_recent_mode_switch() {
        let predictor = StagnationPredictor::with_defaults();

        assert!(!predictor.recent_mode_switch(0, 2));
        assert!(!predictor.recent_mode_switch(1, 2));
        assert!(predictor.recent_mode_switch(2, 2));
        assert!(predictor.recent_mode_switch(5, 2));
    }

    // =========================================================================
    // Phase 6.3: Preventive Action Tests
    // =========================================================================

    #[test]
    fn test_preventive_action_low_risk() {
        let predictor = StagnationPredictor::with_defaults();
        let signals = RiskSignals::new().with_commit_gap(1);
        let score = predictor.risk_score(&signals);
        let action = predictor.preventive_action(&signals, score);

        assert_eq!(action, PreventiveAction::None);
    }

    #[test]
    fn test_preventive_action_medium_risk() {
        let predictor = StagnationPredictor::with_defaults();
        let signals = RiskSignals::new()
            .with_commit_gap(8)
            .with_file_touches(vec![("a.rs".into(), 3)])
            .with_errors(vec!["err".into(), "err".into()]);

        let _score = predictor.risk_score(&signals);
        // Ensure we're in medium range (use explicit value for testing)
        let adjusted_score = 40.0;
        let action = predictor.preventive_action(&signals, adjusted_score);

        match action {
            PreventiveAction::InjectGuidance { .. } => (),
            other => panic!("Expected InjectGuidance, got {:?}", other),
        }
    }

    #[test]
    fn test_preventive_action_high_risk_commit() {
        let predictor = StagnationPredictor::with_defaults();
        // Signals dominated by commit gap
        let signals = RiskSignals::new().with_commit_gap(15);

        let breakdown = predictor.risk_breakdown(&signals);
        let action = predictor.high_risk_action(&breakdown, &signals);

        assert_eq!(action, PreventiveAction::SuggestCommit);
    }

    #[test]
    fn test_preventive_action_critical() {
        let predictor = StagnationPredictor::with_defaults();
        let signals = RiskSignals::new().with_commit_gap(15);
        let action = predictor.preventive_action(&signals, 85.0);

        match action {
            PreventiveAction::RequestReview { reason } => {
                assert!(reason.contains("Critical"));
            }
            other => panic!("Expected RequestReview, got {:?}", other),
        }
    }

    #[test]
    fn test_generate_unstick_guidance() {
        let predictor = StagnationPredictor::with_defaults();

        let breakdown = RiskBreakdown {
            commit_gap_contribution: 50.0,
            file_churn_contribution: 10.0,
            error_repeat_contribution: 10.0,
            test_stagnation_contribution: 10.0,
            mode_oscillation_contribution: 5.0,
            warning_growth_contribution: 5.0,
            total: 90.0,
        };

        let guidance = predictor.generate_unstick_guidance(&breakdown);
        assert!(guidance.contains("commit"));
    }

    #[test]
    fn test_identify_single_actionable_item() {
        let predictor = StagnationPredictor::with_defaults();

        let signals = RiskSignals::new().with_file_touches(vec![("main.rs".into(), 10)]);

        let breakdown = RiskBreakdown {
            commit_gap_contribution: 10.0,
            file_churn_contribution: 50.0,
            error_repeat_contribution: 10.0,
            test_stagnation_contribution: 10.0,
            mode_oscillation_contribution: 5.0,
            warning_growth_contribution: 5.0,
            total: 90.0,
        };

        let item = predictor.identify_single_actionable_item(&signals, &breakdown);
        assert!(item.contains("main.rs"));
        assert!(item.contains("10 times"));
    }

    #[test]
    fn test_prediction_accuracy_tracking() {
        let mut predictor = StagnationPredictor::with_defaults();

        // No predictions yet
        assert!(predictor.prediction_accuracy().is_none());

        // Record some predictions
        predictor.record_prediction(70.0, true); // High risk, stagnated - correct
        predictor.record_prediction(20.0, false); // Low risk, no stagnation - correct
        predictor.record_prediction(80.0, false); // High risk, no stagnation - wrong

        let accuracy = predictor.prediction_accuracy().unwrap();
        // 2 out of 3 correct
        assert!((accuracy - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_predictor_summary() {
        let mut predictor = StagnationPredictor::with_defaults();
        predictor.record_prediction(50.0, true);
        predictor.record_prediction(50.0, false);

        let summary = predictor.summary();
        assert!(summary.contains("2 predictions"));
        assert!(summary.contains("accuracy="));
    }

    #[test]
    fn test_preventive_action_display() {
        let action = PreventiveAction::SuggestCommit;
        assert_eq!(action.to_string(), "suggest_commit");

        let action = PreventiveAction::InjectGuidance {
            guidance: "test".into(),
        };
        assert!(action.to_string().contains("inject_guidance"));

        let action = PreventiveAction::FocusTask {
            task: "do something".into(),
        };
        assert!(action.to_string().contains("focus_task"));
    }

    #[test]
    fn test_risk_signals_builder() {
        let signals = RiskSignals::new()
            .with_commit_gap(5)
            .with_file_touches(vec![("a.rs".into(), 2)])
            .with_errors(vec!["err".into()])
            .with_test_history(vec![10, 11])
            .with_mode_switches(1)
            .with_warning_history(vec![0, 1]);

        assert_eq!(signals.iterations_since_commit, 5);
        assert_eq!(signals.file_touch_counts.len(), 1);
        assert_eq!(signals.error_messages.len(), 1);
        assert_eq!(signals.test_count_history.len(), 2);
        assert_eq!(signals.mode_switches, 1);
        assert_eq!(signals.clippy_warning_history.len(), 2);
    }

    #[test]
    fn test_risk_level_display() {
        assert_eq!(RiskLevel::Low.to_string(), "low");
        assert_eq!(RiskLevel::Medium.to_string(), "medium");
        assert_eq!(RiskLevel::High.to_string(), "high");
        assert_eq!(RiskLevel::Critical.to_string(), "critical");
    }

    #[test]
    fn test_risk_level_score_ranges() {
        assert_eq!(RiskLevel::Low.min_score(), 0.0);
        assert_eq!(RiskLevel::Low.max_score(), 30.0);
        assert_eq!(RiskLevel::Medium.min_score(), 30.0);
        assert_eq!(RiskLevel::Medium.max_score(), 60.0);
        assert_eq!(RiskLevel::High.min_score(), 60.0);
        assert_eq!(RiskLevel::High.max_score(), 80.0);
        assert_eq!(RiskLevel::Critical.min_score(), 80.0);
        assert_eq!(RiskLevel::Critical.max_score(), 100.0);
    }

    #[test]
    fn test_empty_signals_low_risk() {
        let predictor = StagnationPredictor::with_defaults();
        let signals = RiskSignals::new();

        let score = predictor.risk_score(&signals);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_warning_growth_scoring() {
        let predictor = StagnationPredictor::with_defaults();

        // No growth
        let no_growth = vec![5, 5, 5];
        assert_eq!(predictor.score_warning_growth(&no_growth), 0.0);

        // 100% growth (5 -> 10)
        let double_growth = vec![5, 7, 10];
        assert!((predictor.score_warning_growth(&double_growth) - 1.0).abs() < 0.01);

        // 50% growth (10 -> 15)
        let half_growth = vec![10, 12, 15];
        assert!((predictor.score_warning_growth(&half_growth) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_max_score_capped() {
        let predictor = StagnationPredictor::with_defaults();

        // Extreme signals that would exceed 100
        let signals = RiskSignals::new()
            .with_commit_gap(100)
            .with_file_touches(vec![("a.rs".into(), 100)])
            .with_errors(vec!["err".into(); 100])
            .with_test_history(vec![10; 20])
            .with_mode_switches(100)
            .with_warning_history(vec![1, 100]);

        let score = predictor.risk_score(&signals);
        assert!(score <= 100.0, "Score {} should be capped at 100", score);
    }

    // =========================================================================
    // Phase 10.2: Predictor Accuracy Tracking Tests
    // =========================================================================

    #[test]
    fn test_prediction_accuracy_by_risk_level_empty() {
        let predictor = StagnationPredictor::with_defaults();

        let breakdown = predictor.prediction_accuracy_by_level();

        assert!(breakdown.get(&RiskLevel::Low).unwrap().is_none());
        assert!(breakdown.get(&RiskLevel::Medium).unwrap().is_none());
        assert!(breakdown.get(&RiskLevel::High).unwrap().is_none());
        assert!(breakdown.get(&RiskLevel::Critical).unwrap().is_none());
    }

    #[test]
    fn test_prediction_accuracy_by_risk_level_with_data() {
        let mut predictor = StagnationPredictor::with_defaults();

        // Low risk predictions (score < 30): 2 correct, 1 wrong
        predictor.record_prediction(10.0, false); // Correct: low risk, no stagnation
        predictor.record_prediction(20.0, false); // Correct: low risk, no stagnation
        predictor.record_prediction(25.0, true); // Wrong: low risk, but stagnated

        // Medium risk predictions (30-60): 1 correct, 1 wrong
        predictor.record_prediction(40.0, false); // Correct: medium risk, no stagnation
        predictor.record_prediction(50.0, true); // Wrong: medium risk, stagnated

        // High risk predictions (60-80): 2 correct
        predictor.record_prediction(65.0, true); // Correct: high risk, stagnated
        predictor.record_prediction(75.0, true); // Correct: high risk, stagnated

        // Critical risk predictions (80+): 1 correct, 1 wrong
        predictor.record_prediction(85.0, true); // Correct: critical, stagnated
        predictor.record_prediction(90.0, false); // Wrong: critical, no stagnation

        let breakdown = predictor.prediction_accuracy_by_level();

        // Low: 2/3 correct = 66.67%
        let low_acc = breakdown.get(&RiskLevel::Low).unwrap().unwrap();
        assert!((low_acc - 0.6667).abs() < 0.01, "Low accuracy was {}", low_acc);

        // Medium: 1/2 correct = 50%
        let medium_acc = breakdown.get(&RiskLevel::Medium).unwrap().unwrap();
        assert!((medium_acc - 0.5).abs() < 0.01, "Medium accuracy was {}", medium_acc);

        // High: 2/2 correct = 100%
        let high_acc = breakdown.get(&RiskLevel::High).unwrap().unwrap();
        assert!((high_acc - 1.0).abs() < 0.01, "High accuracy was {}", high_acc);

        // Critical: 1/2 correct = 50%
        let critical_acc = breakdown.get(&RiskLevel::Critical).unwrap().unwrap();
        assert!((critical_acc - 0.5).abs() < 0.01, "Critical accuracy was {}", critical_acc);
    }

    #[test]
    fn test_prediction_statistics() {
        let mut predictor = StagnationPredictor::with_defaults();

        predictor.record_prediction(20.0, false);
        predictor.record_prediction(45.0, true);
        predictor.record_prediction(70.0, true);
        predictor.record_prediction(85.0, true);

        let stats = predictor.prediction_statistics();

        assert_eq!(stats.total_predictions, 4);
        assert_eq!(stats.predictions_by_level.get(&RiskLevel::Low).copied().unwrap_or(0), 1);
        assert_eq!(stats.predictions_by_level.get(&RiskLevel::Medium).copied().unwrap_or(0), 1);
        assert_eq!(stats.predictions_by_level.get(&RiskLevel::High).copied().unwrap_or(0), 1);
        assert_eq!(stats.predictions_by_level.get(&RiskLevel::Critical).copied().unwrap_or(0), 1);
        assert!(stats.overall_accuracy.is_some());
    }

    #[test]
    fn test_prediction_statistics_serialization() {
        let mut predictor = StagnationPredictor::with_defaults();

        predictor.record_prediction(50.0, true);
        predictor.record_prediction(70.0, false);

        let stats = predictor.prediction_statistics();

        // Should be serializable for analytics
        let json = serde_json::to_string(&stats).expect("Stats should be serializable");
        assert!(json.contains("total_predictions"));
        assert!(json.contains("overall_accuracy"));
    }

    #[test]
    fn test_prediction_history_retrieval() {
        let mut predictor = StagnationPredictor::with_defaults();

        predictor.record_prediction(30.0, false);
        predictor.record_prediction(60.0, true);
        predictor.record_prediction(80.0, true);

        let history = predictor.prediction_history();

        assert_eq!(history.len(), 3);
        assert_eq!(history[0], (30.0, false));
        assert_eq!(history[1], (60.0, true));
        assert_eq!(history[2], (80.0, true));
    }

    // =========================================================================
    // Phase 10.3: Dynamic Risk Weight Tuning Tests
    // =========================================================================

    #[test]
    fn test_weight_preset_balanced_is_default() {
        let balanced = WeightPreset::Balanced.weights();
        let default = RiskWeights::default();

        assert!((balanced.commit_gap - default.commit_gap).abs() < 0.001);
        assert!((balanced.file_churn - default.file_churn).abs() < 0.001);
        assert!((balanced.error_repeat - default.error_repeat).abs() < 0.001);
        assert!((balanced.test_stagnation - default.test_stagnation).abs() < 0.001);
        assert!((balanced.mode_oscillation - default.mode_oscillation).abs() < 0.001);
        assert!((balanced.warning_growth - default.warning_growth).abs() < 0.001);
    }

    #[test]
    fn test_weight_preset_conservative() {
        // Conservative: emphasize commit gap and error repeat (catch stagnation early)
        let conservative = WeightPreset::Conservative.weights();

        // Weights should sum to 1.0
        assert!((conservative.total() - 1.0).abs() < 0.001);

        // Conservative emphasizes commit_gap and error_repeat
        assert!(
            conservative.commit_gap >= 0.30,
            "Conservative should emphasize commit_gap"
        );
        assert!(
            conservative.error_repeat >= 0.25,
            "Conservative should emphasize error_repeat"
        );
    }

    #[test]
    fn test_weight_preset_aggressive() {
        // Aggressive: emphasize file churn and warning growth (tolerate more commit gap)
        let aggressive = WeightPreset::Aggressive.weights();

        // Weights should sum to 1.0
        assert!((aggressive.total() - 1.0).abs() < 0.001);

        // Aggressive reduces commit_gap emphasis
        assert!(
            aggressive.commit_gap <= 0.20,
            "Aggressive should de-emphasize commit_gap"
        );
        // Aggressive emphasizes file churn (focus on actual problems)
        assert!(
            aggressive.file_churn >= 0.25,
            "Aggressive should emphasize file_churn"
        );
    }

    #[test]
    fn test_weight_presets_all_normalize() {
        for preset in [
            WeightPreset::Balanced,
            WeightPreset::Conservative,
            WeightPreset::Aggressive,
        ] {
            let weights = preset.weights();
            assert!(
                (weights.total() - 1.0).abs() < 0.001,
                "{:?} weights should sum to 1.0, got {}",
                preset,
                weights.total()
            );
        }
    }

    #[test]
    fn test_weight_preset_from_str() {
        assert_eq!(
            "balanced".parse::<WeightPreset>().unwrap(),
            WeightPreset::Balanced
        );
        assert_eq!(
            "conservative".parse::<WeightPreset>().unwrap(),
            WeightPreset::Conservative
        );
        assert_eq!(
            "aggressive".parse::<WeightPreset>().unwrap(),
            WeightPreset::Aggressive
        );
        assert!("invalid".parse::<WeightPreset>().is_err());
    }

    #[test]
    fn test_weight_preset_display() {
        assert_eq!(WeightPreset::Balanced.to_string(), "balanced");
        assert_eq!(WeightPreset::Conservative.to_string(), "conservative");
        assert_eq!(WeightPreset::Aggressive.to_string(), "aggressive");
    }

    #[test]
    fn test_custom_weights_affect_risk_score() {
        // Use custom weights that heavily emphasize commit_gap
        let custom_weights = RiskWeights::new(0.90, 0.02, 0.02, 0.02, 0.02, 0.02);
        let config = PredictorConfig::new().with_weights(custom_weights);
        let predictor = StagnationPredictor::new(config);

        // Same signals should produce different scores with different weights
        let signals = RiskSignals::new()
            .with_commit_gap(15) // Max commit gap
            .with_file_touches(vec![("a.rs".into(), 1)]) // Low churn
            .with_errors(vec!["unique1".into(), "unique2".into()]) // No repeats
            .with_test_history(vec![10, 11, 12]) // Growing tests
            .with_mode_switches(0)
            .with_warning_history(vec![0, 0]);

        let score = predictor.risk_score(&signals);

        // With 90% weight on commit_gap which is maxed, score should be very high
        assert!(
            score >= 80.0,
            "Custom weights heavily on commit_gap with max gap should produce high score, got {}",
            score
        );
    }

    #[test]
    fn test_predictor_with_preset() {
        let predictor = StagnationPredictor::with_preset(WeightPreset::Conservative);

        let conservative_weights = WeightPreset::Conservative.weights();
        let config_weights = predictor.config().weights.clone();

        assert!(
            (config_weights.commit_gap - conservative_weights.commit_gap).abs() < 0.001,
            "Predictor should use preset weights"
        );
    }

    #[test]
    fn test_risk_weights_validation_valid() {
        // Valid weights
        let weights = RiskWeights::new(0.25, 0.20, 0.20, 0.15, 0.10, 0.10);
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn test_risk_weights_validation_negative() {
        // Negative weights are invalid
        let weights = RiskWeights::new(-0.1, 0.30, 0.30, 0.20, 0.15, 0.15);
        let result = weights.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("negative"));
    }

    #[test]
    fn test_risk_weights_validation_all_zero() {
        // All zero weights are invalid
        let weights = RiskWeights::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let result = weights.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("zero"));
    }

    #[test]
    fn test_risk_weights_validation_nan() {
        // NaN weights are invalid
        let weights = RiskWeights::new(f64::NAN, 0.20, 0.20, 0.15, 0.10, 0.10);
        let result = weights.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_weights_validation_infinity() {
        // Infinite weights are invalid
        let weights = RiskWeights::new(f64::INFINITY, 0.20, 0.20, 0.15, 0.10, 0.10);
        let result = weights.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_weight_presets_serialization() {
        let preset = WeightPreset::Conservative;
        let json = serde_json::to_string(&preset).expect("Should serialize");
        assert!(json.contains("conservative"));

        let deserialized: WeightPreset =
            serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized, preset);
    }

    #[test]
    fn test_predictor_config_with_preset() {
        let config = PredictorConfig::with_preset(WeightPreset::Aggressive);

        let aggressive_weights = WeightPreset::Aggressive.weights();
        assert!(
            (config.weights.commit_gap - aggressive_weights.commit_gap).abs() < 0.001,
            "Config should use preset weights"
        );
    }
}
