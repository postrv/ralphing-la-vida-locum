//! Predictor stats persistence layer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::stagnation::types::{RiskLevel, RiskWeights};

/// Current schema version for predictor stats.
pub const STATS_VERSION: u32 = 1;

/// Minimum supported version for backward compatibility.
pub const MIN_STATS_VERSION: u32 = 1;

/// Default filename for predictor stats.
pub const STATS_FILENAME: &str = "predictor_stats.json";

/// Predictor statistics for cross-session learning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictorStats {
    /// Schema version for forward compatibility.
    version: u32,
    /// Total number of predictions made.
    total_predictions: u64,
    /// Total number of correct predictions.
    correct_predictions: u64,
    /// Number of predictions at each risk level.
    predictions_by_level: HashMap<RiskLevel, u64>,
    /// Number of correct predictions at each risk level.
    correct_by_level: HashMap<RiskLevel, u64>,
    /// Current factor weights.
    factor_weights: Option<RiskWeights>,
    /// When these stats were last updated.
    last_updated: DateTime<Utc>,
}

impl Default for PredictorStats {
    fn default() -> Self {
        Self::new()
    }
}

impl PredictorStats {
    /// Creates new empty predictor stats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: STATS_VERSION,
            total_predictions: 0,
            correct_predictions: 0,
            predictions_by_level: HashMap::new(),
            correct_by_level: HashMap::new(),
            factor_weights: None,
            last_updated: Utc::now(),
        }
    }

    /// Returns the total number of predictions made.
    #[must_use]
    pub fn total_predictions(&self) -> u64 {
        self.total_predictions
    }

    /// Returns the total number of correct predictions.
    #[must_use]
    pub fn correct_predictions(&self) -> u64 {
        self.correct_predictions
    }

    /// Returns the overall prediction accuracy (0.0 - 1.0).
    #[must_use]
    pub fn accuracy(&self) -> Option<f64> {
        if self.total_predictions == 0 {
            return None;
        }
        Some(self.correct_predictions as f64 / self.total_predictions as f64)
    }

    /// Returns the accuracy for a specific risk level.
    #[must_use]
    pub fn accuracy_for_level(&self, level: RiskLevel) -> Option<f64> {
        let total = self.predictions_by_level.get(&level).copied().unwrap_or(0);
        if total == 0 {
            return None;
        }
        let correct = self.correct_by_level.get(&level).copied().unwrap_or(0);
        Some(correct as f64 / total as f64)
    }

    /// Returns the number of predictions at a specific risk level.
    #[must_use]
    pub fn predictions_at_level(&self, level: RiskLevel) -> u64 {
        self.predictions_by_level.get(&level).copied().unwrap_or(0)
    }

    /// Returns a reference to the factor weights, if set.
    #[must_use]
    pub fn factor_weights(&self) -> Option<&RiskWeights> {
        self.factor_weights.as_ref()
    }

    /// Returns when these stats were last updated.
    #[must_use]
    pub fn last_updated(&self) -> DateTime<Utc> {
        self.last_updated
    }

    /// Returns the schema version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Checks if the version is compatible.
    #[must_use]
    pub fn is_compatible_version(&self) -> bool {
        self.version >= MIN_STATS_VERSION && self.version <= STATS_VERSION
    }

    /// Records a prediction result.
    pub fn record_prediction(&mut self, level: RiskLevel, was_correct: bool) {
        self.total_predictions += 1;
        *self.predictions_by_level.entry(level).or_insert(0) += 1;

        if was_correct {
            self.correct_predictions += 1;
            *self.correct_by_level.entry(level).or_insert(0) += 1;
        }

        self.last_updated = Utc::now();
    }

    /// Sets the factor weights.
    pub fn set_factor_weights(&mut self, weights: RiskWeights) {
        self.factor_weights = Some(weights);
        self.last_updated = Utc::now();
    }

    /// Returns a human-readable summary of the stats.
    #[must_use]
    pub fn summary(&self) -> String {
        let accuracy_str = self
            .accuracy()
            .map(|a| format!("{:.0}%", a * 100.0))
            .unwrap_or_else(|| "N/A".to_string());

        let mut level_breakdown = Vec::new();
        for level in RiskLevel::all() {
            let count = self.predictions_at_level(level);
            if count > 0 {
                let acc = self
                    .accuracy_for_level(level)
                    .map(|a| format!("{:.0}%", a * 100.0))
                    .unwrap_or_else(|| "N/A".to_string());
                level_breakdown.push(format!("{}={} ({})", level, count, acc));
            }
        }

        format!(
            "Predictor: {} predictions, accuracy={}, breakdown: [{}]",
            self.total_predictions,
            accuracy_str,
            level_breakdown.join(", ")
        )
    }
}

/// Persistence layer for predictor stats.
#[derive(Debug, Clone)]
pub struct StatsPersistence {
    /// Path to the `.ralph` directory.
    ralph_dir: PathBuf,
}

impl StatsPersistence {
    /// Creates a new stats persistence handler.
    #[must_use]
    pub fn new<P: AsRef<Path>>(project_dir: P) -> Self {
        Self {
            ralph_dir: project_dir.as_ref().join(".ralph"),
        }
    }

    /// Returns the path to the stats file.
    #[must_use]
    pub fn stats_path(&self) -> PathBuf {
        self.ralph_dir.join(STATS_FILENAME)
    }

    /// Saves predictor stats to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, stats: &PredictorStats) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.ralph_dir)?;

        let temp_path = self.ralph_dir.join(format!("{}.tmp", STATS_FILENAME));
        let stats_path = self.stats_path();

        let json = serde_json::to_string_pretty(stats)?;
        std::fs::write(&temp_path, json)?;
        std::fs::rename(&temp_path, &stats_path)?;

        Ok(())
    }

    /// Loads predictor stats from disk.
    ///
    /// # Errors
    ///
    /// Returns an error only for unexpected I/O failures.
    pub fn load(&self) -> anyhow::Result<Option<PredictorStats>> {
        let stats_path = self.stats_path();

        if !stats_path.exists() {
            return Ok(None);
        }

        let content = match std::fs::read_to_string(&stats_path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read predictor stats file: {}", e);
                return Ok(None);
            }
        };

        let stats: PredictorStats = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                warn!("Predictor stats file is corrupted, starting fresh: {}", e);
                let _ = std::fs::remove_file(&stats_path);
                return Ok(None);
            }
        };

        if !stats.is_compatible_version() {
            warn!(
                "Predictor stats version {} is incompatible, starting fresh",
                stats.version
            );
            let _ = std::fs::remove_file(&stats_path);
            return Ok(None);
        }

        Ok(Some(stats))
    }

    /// Loads predictor stats or returns default if not found.
    ///
    /// # Errors
    ///
    /// Returns an error only for unexpected I/O failures.
    pub fn load_or_default(&self) -> anyhow::Result<PredictorStats> {
        Ok(self.load()?.unwrap_or_default())
    }

    /// Checks if stats file exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.stats_path().exists()
    }

    /// Deletes the stats file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be deleted.
    pub fn delete(&self) -> anyhow::Result<()> {
        let path = self.stats_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_predictor_stats_serialization() {
        let mut stats = PredictorStats::new();
        stats.record_prediction(RiskLevel::High, true);
        stats.record_prediction(RiskLevel::Low, false);

        let json = serde_json::to_string_pretty(&stats).expect("serialization");
        let restored: PredictorStats = serde_json::from_str(&json).expect("deserialization");

        assert_eq!(restored.total_predictions(), stats.total_predictions());
        assert_eq!(restored.correct_predictions(), stats.correct_predictions());
    }

    #[test]
    fn test_predictor_stats_persistence_roundtrip() {
        let temp = TempDir::new().unwrap();
        let persistence = StatsPersistence::new(temp.path());

        let mut original = PredictorStats::new();
        original.record_prediction(RiskLevel::High, true);
        original.record_prediction(RiskLevel::Low, true);

        persistence.save(&original).expect("save");
        assert!(persistence.exists());

        let loaded = persistence.load().expect("load").expect("stats");
        assert_eq!(loaded.total_predictions(), original.total_predictions());
    }

    #[test]
    fn test_persistence_load_returns_none_when_missing() {
        let temp = TempDir::new().unwrap();
        let persistence = StatsPersistence::new(temp.path());
        assert!(persistence.load().expect("load").is_none());
    }

    #[test]
    fn test_predictor_stats_new_is_empty() {
        let stats = PredictorStats::new();
        assert_eq!(stats.total_predictions(), 0);
        assert!(stats.accuracy().is_none());
    }

    #[test]
    fn test_predictor_stats_record_prediction() {
        let mut stats = PredictorStats::new();
        stats.record_prediction(RiskLevel::High, true);
        stats.record_prediction(RiskLevel::High, false);

        assert_eq!(stats.total_predictions(), 2);
        assert_eq!(stats.correct_predictions(), 1);
        assert_eq!(stats.predictions_at_level(RiskLevel::High), 2);
    }

    #[test]
    fn test_predictor_stats_accuracy() {
        let mut stats = PredictorStats::new();
        stats.record_prediction(RiskLevel::High, true);
        stats.record_prediction(RiskLevel::Low, false);

        assert!((stats.accuracy().unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_predictor_stats_summary() {
        let mut stats = PredictorStats::new();
        stats.record_prediction(RiskLevel::High, true);

        let summary = stats.summary();
        assert!(summary.contains("1 predictions"));
        assert!(summary.contains("100%"));
    }

    #[test]
    fn test_persistence_save_creates_ralph_dir() {
        let temp = TempDir::new().unwrap();
        let persistence = StatsPersistence::new(temp.path());

        assert!(!temp.path().join(".ralph").exists());

        let stats = PredictorStats::new();
        persistence.save(&stats).expect("save");

        assert!(temp.path().join(".ralph").exists());
    }

    #[test]
    fn test_persistence_delete() {
        let temp = TempDir::new().unwrap();
        let persistence = StatsPersistence::new(temp.path());

        let stats = PredictorStats::new();
        persistence.save(&stats).expect("save");
        assert!(persistence.exists());

        persistence.delete().expect("delete");
        assert!(!persistence.exists());
    }

    #[test]
    fn test_stats_path() {
        let persistence = StatsPersistence::new("/some/project");
        assert_eq!(
            persistence.stats_path(),
            PathBuf::from("/some/project/.ralph/predictor_stats.json")
        );
    }
}
