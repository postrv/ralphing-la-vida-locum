//! Stagnation detection, prediction, and persistence.
//!
//! This module provides utilities for detecting, predicting, and tracking
//! stagnation patterns in the automation loop.

pub mod persistence;
pub mod types;

pub use persistence::{
    PredictorStats, StatsPersistence, MIN_STATS_VERSION, STATS_FILENAME, STATS_VERSION,
};
pub use types::{RiskLevel, RiskScore, RiskWeights};
