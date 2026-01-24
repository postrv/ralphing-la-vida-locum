//! Core types for stagnation prediction and risk assessment.

use serde::{Deserialize, Serialize};

/// Risk score range: 0-100.
pub type RiskScore = f64;

/// Risk level classification based on score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Score 0-30: Normal operation.
    Low,
    /// Score 30-60: Caution.
    Medium,
    /// Score 60-80: Elevated risk.
    High,
    /// Score 80-100: Critical.
    Critical,
}

impl RiskLevel {
    /// Creates a risk level from a score.
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        if score >= 80.0 {
            Self::Critical
        } else if score >= 60.0 {
            Self::High
        } else if score >= 30.0 {
            Self::Medium
        } else {
            Self::Low
        }
    }

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

    /// Returns all risk levels in order.
    #[must_use]
    pub fn all() -> [RiskLevel; 4] {
        [Self::Low, Self::Medium, Self::High, Self::Critical]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskWeights {
    /// Weight for iterations since last commit.
    pub commit_gap: f64,
    /// Weight for repeated file edits.
    pub file_churn: f64,
    /// Weight for error repetition.
    pub error_repeat: f64,
    /// Weight for test count stagnation.
    pub test_stagnation: f64,
    /// Weight for mode oscillation.
    pub mode_oscillation: f64,
    /// Weight for clippy warning growth.
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

    /// Validates that the weights are valid.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(0.0), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(15.0), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(30.0), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(60.0), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(80.0), RiskLevel::Critical);
    }

    #[test]
    fn test_risk_level_requires_intervention() {
        assert!(!RiskLevel::Low.requires_intervention());
        assert!(!RiskLevel::Medium.requires_intervention());
        assert!(RiskLevel::High.requires_intervention());
        assert!(RiskLevel::Critical.requires_intervention());
    }

    #[test]
    fn test_risk_weights_default() {
        let weights = RiskWeights::default();
        assert!((weights.total() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_risk_weights_validate() {
        let valid = RiskWeights::default();
        assert!(valid.validate().is_ok());

        let invalid = RiskWeights::new(-0.1, 0.2, 0.2, 0.15, 0.1, 0.1);
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_risk_level_serialization() {
        let level = RiskLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        let restored: RiskLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, RiskLevel::High);
    }
}
