//! Session statistics and predictor accuracy types.
//!
//! This module provides types for tracking session statistics
//! and predictor accuracy across analytics events.

use serde::{Deserialize, Serialize};

/// Aggregate statistics across all sessions.
///
/// Used to provide overall statistics across multiple sessions.
///
/// # Example
///
/// ```
/// use ralph::analytics::AggregateStats;
///
/// let stats = AggregateStats::default();
/// assert_eq!(stats.total_sessions, 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateStats {
    /// Total number of sessions.
    pub total_sessions: usize,
    /// Total number of iterations across all sessions.
    pub total_iterations: usize,
    /// Total number of errors across all sessions.
    pub total_errors: usize,
    /// Total number of stagnation events across all sessions.
    pub total_stagnations: usize,
    /// Total number of drift events across all sessions.
    pub total_drift_events: usize,
    /// Average predictor accuracy across sessions that have accuracy data.
    pub avg_predictor_accuracy: Option<f64>,
    /// Total number of quality gate runs across all sessions (Phase 15.1).
    pub total_gate_runs: usize,
    /// Total time spent running quality gates in milliseconds (Phase 15.1).
    pub total_gate_execution_ms: u64,
}

// ============================================================================
// Predictor Accuracy Statistics
// ============================================================================

/// Statistics about predictor accuracy for a session.
///
/// This struct captures predictor performance metrics for analytics and
/// historical tracking.
///
/// # Example
///
/// ```
/// use ralph::analytics::PredictorAccuracyStats;
///
/// let stats = PredictorAccuracyStats {
///     total_predictions: 10,
///     correct_predictions: 8,
///     overall_accuracy: Some(0.8),
///     accuracy_low: Some(0.9),
///     accuracy_medium: Some(0.75),
///     accuracy_high: Some(0.7),
///     accuracy_critical: None,
/// };
/// assert_eq!(stats.total_predictions, 10);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PredictorAccuracyStats {
    /// Total number of predictions made.
    pub total_predictions: usize,
    /// Number of correct predictions.
    pub correct_predictions: usize,
    /// Overall prediction accuracy (0.0-1.0).
    pub overall_accuracy: Option<f64>,
    /// Accuracy for low risk predictions.
    pub accuracy_low: Option<f64>,
    /// Accuracy for medium risk predictions.
    pub accuracy_medium: Option<f64>,
    /// Accuracy for high risk predictions.
    pub accuracy_high: Option<f64>,
    /// Accuracy for critical risk predictions.
    pub accuracy_critical: Option<f64>,
}

impl PredictorAccuracyStats {
    /// Returns true if any predictions were made.
    #[must_use]
    pub fn has_predictions(&self) -> bool {
        self.total_predictions > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_stats_default() {
        let stats = AggregateStats::default();
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.total_iterations, 0);
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.total_stagnations, 0);
        assert_eq!(stats.total_drift_events, 0);
        assert!(stats.avg_predictor_accuracy.is_none());
        assert_eq!(stats.total_gate_runs, 0);
        assert_eq!(stats.total_gate_execution_ms, 0);
    }

    #[test]
    fn test_predictor_accuracy_stats_default() {
        let stats = PredictorAccuracyStats::default();
        assert_eq!(stats.total_predictions, 0);
        assert_eq!(stats.correct_predictions, 0);
        assert!(stats.overall_accuracy.is_none());
        assert!(!stats.has_predictions());
    }

    #[test]
    fn test_predictor_accuracy_stats_has_predictions() {
        let stats = PredictorAccuracyStats {
            total_predictions: 10,
            correct_predictions: 8,
            overall_accuracy: Some(0.8),
            ..Default::default()
        };
        assert!(stats.has_predictions());
    }

    #[test]
    fn test_aggregate_stats_serialization() {
        let stats = AggregateStats {
            total_sessions: 5,
            total_iterations: 100,
            total_errors: 3,
            total_stagnations: 2,
            total_drift_events: 1,
            avg_predictor_accuracy: Some(0.85),
            total_gate_runs: 50,
            total_gate_execution_ms: 15000,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let restored: AggregateStats = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_sessions, 5);
        assert_eq!(restored.total_iterations, 100);
        assert_eq!(restored.total_gate_runs, 50);
    }

    #[test]
    fn test_predictor_accuracy_stats_serialization() {
        let stats = PredictorAccuracyStats {
            total_predictions: 10,
            correct_predictions: 8,
            overall_accuracy: Some(0.8),
            accuracy_low: Some(0.9),
            accuracy_medium: Some(0.75),
            accuracy_high: Some(0.7),
            accuracy_critical: None,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let restored: PredictorAccuracyStats = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_predictions, 10);
        assert_eq!(restored.overall_accuracy, Some(0.8));
        assert!(restored.accuracy_critical.is_none());
    }
}
