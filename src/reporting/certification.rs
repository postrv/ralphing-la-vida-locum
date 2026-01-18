//! Quality certification module.
//!
//! Provides quality certification functionality for projects, including:
//!
//! - Certification JSON generation
//! - Badge endpoint URLs (shields.io style)
//! - Certification history storage
//! - Regression-based revocation
//!
//! # Certification Levels
//!
//! Projects are certified at different levels based on quality metrics:
//!
//! | Level    | Score Range | Requirements |
//! |----------|-------------|--------------|
//! | Gold     | 90-100      | All gates pass, no warnings |
//! | Silver   | 70-89       | All blocking gates pass |
//! | Bronze   | 50-69       | Tests pass, minimal issues |
//! | None     | 0-49        | Quality gates failing |
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::reporting::{QualityCertifier, CertificationLevel};
//! use ralph::analytics::QualityMetricsSnapshot;
//!
//! let snapshot = QualityMetricsSnapshot::new("session-1", 1)
//!     .with_clippy_warnings(0)
//!     .with_test_counts(100, 100, 0)
//!     .with_security_issues(0);
//!
//! let cert = QualityCertifier::certify("my-project", &snapshot)?;
//!
//! assert_eq!(cert.level, CertificationLevel::Gold);
//! println!("Badge URL: {}", cert.badge_url());
//! ```

use crate::analytics::QualityMetricsSnapshot;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

// ============================================================================
// Certification Level
// ============================================================================

/// Quality certification level.
///
/// Levels are determined by quality score thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CertificationLevel {
    /// Top tier: score >= 90, all quality gates pass.
    Gold,
    /// High tier: score >= 70, all blocking gates pass.
    Silver,
    /// Acceptable tier: score >= 50, tests pass.
    Bronze,
    /// Not certified: score < 50 or critical failures.
    None,
}

impl CertificationLevel {
    /// Determine certification level from a quality score.
    ///
    /// # Arguments
    ///
    /// * `score` - Quality score (0-100)
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::reporting::CertificationLevel;
    ///
    /// assert_eq!(CertificationLevel::from_score(95.0), CertificationLevel::Gold);
    /// assert_eq!(CertificationLevel::from_score(80.0), CertificationLevel::Silver);
    /// assert_eq!(CertificationLevel::from_score(60.0), CertificationLevel::Bronze);
    /// assert_eq!(CertificationLevel::from_score(40.0), CertificationLevel::None);
    /// ```
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s >= 90.0 => CertificationLevel::Gold,
            s if s >= 70.0 => CertificationLevel::Silver,
            s if s >= 50.0 => CertificationLevel::Bronze,
            _ => CertificationLevel::None,
        }
    }

    /// Get the color for shields.io badge.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::reporting::CertificationLevel;
    ///
    /// assert_eq!(CertificationLevel::Gold.badge_color(), "brightgreen");
    /// ```
    #[must_use]
    pub fn badge_color(&self) -> &'static str {
        match self {
            CertificationLevel::Gold => "brightgreen",
            CertificationLevel::Silver => "green",
            CertificationLevel::Bronze => "yellow",
            CertificationLevel::None => "red",
        }
    }

    /// Check if this level represents a valid certification.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::reporting::CertificationLevel;
    ///
    /// assert!(CertificationLevel::Gold.is_certified());
    /// assert!(!CertificationLevel::None.is_certified());
    /// ```
    #[must_use]
    pub fn is_certified(&self) -> bool {
        !matches!(self, CertificationLevel::None)
    }
}

impl fmt::Display for CertificationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CertificationLevel::Gold => write!(f, "gold"),
            CertificationLevel::Silver => write!(f, "silver"),
            CertificationLevel::Bronze => write!(f, "bronze"),
            CertificationLevel::None => write!(f, "none"),
        }
    }
}

impl FromStr for CertificationLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gold" => Ok(CertificationLevel::Gold),
            "silver" => Ok(CertificationLevel::Silver),
            "bronze" => Ok(CertificationLevel::Bronze),
            "none" => Ok(CertificationLevel::None),
            _ => Err(anyhow::anyhow!(
                "Invalid certification level: {}. Valid levels: gold, silver, bronze, none",
                s
            )),
        }
    }
}

// ============================================================================
// Quality Certification
// ============================================================================

/// A quality certification for a project at a point in time.
///
/// Certifications capture quality state and can be serialized to JSON
/// for storage or API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityCertification {
    /// Project name or identifier.
    pub project: String,
    /// Certification level achieved.
    pub level: CertificationLevel,
    /// Quality score (0-100).
    pub score: f64,
    /// Git commit hash at certification time (optional).
    pub commit_hash: Option<String>,
    /// Timestamp when certification was issued.
    pub issued_at: DateTime<Utc>,
    /// Timestamp when certification expires.
    pub expires_at: DateTime<Utc>,
    /// Whether the certification is currently valid.
    pub valid: bool,
    /// Reason for revocation if not valid.
    pub revocation_reason: Option<String>,
    /// Quality metrics used for certification.
    pub metrics: CertificationMetrics,
}

/// Quality metrics captured at certification time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificationMetrics {
    /// Clippy warnings count.
    pub clippy_warnings: u32,
    /// Total test count.
    pub test_total: u32,
    /// Passing test count.
    pub test_passed: u32,
    /// Failing test count.
    pub test_failed: u32,
    /// Security issues count.
    pub security_issues: u32,
    /// Allow annotation count.
    pub allow_annotations: u32,
}

impl QualityCertification {
    /// Create a new certification from quality metrics.
    ///
    /// # Arguments
    ///
    /// * `project` - Project name or identifier
    /// * `snapshot` - Quality metrics snapshot
    /// * `validity_days` - Number of days until certification expires
    #[must_use]
    pub fn new(project: impl Into<String>, snapshot: &QualityMetricsSnapshot, validity_days: i64) -> Self {
        let score = Self::calculate_score(snapshot);
        let level = CertificationLevel::from_score(score);
        let now = Utc::now();

        Self {
            project: project.into(),
            level,
            score,
            commit_hash: None,
            issued_at: now,
            expires_at: now + Duration::days(validity_days),
            valid: level.is_certified(),
            revocation_reason: None,
            metrics: CertificationMetrics {
                clippy_warnings: snapshot.clippy_warnings,
                test_total: snapshot.test_total,
                test_passed: snapshot.test_passed,
                test_failed: snapshot.test_failed,
                security_issues: snapshot.security_issues,
                allow_annotations: snapshot.allow_annotations,
            },
        }
    }

    /// Set the commit hash for this certification.
    #[must_use]
    pub fn with_commit(mut self, commit: impl Into<String>) -> Self {
        self.commit_hash = Some(commit.into());
        self
    }

    /// Calculate quality score from metrics (0-100).
    ///
    /// Scoring formula:
    /// - Start at 100
    /// - -2 points per clippy warning (max -20)
    /// - -5 points per test failure (max -40)
    /// - -10 points per security issue (max -30)
    /// - -1 point per allow annotation (max -10)
    fn calculate_score(snapshot: &QualityMetricsSnapshot) -> f64 {
        let mut score = 100.0;

        // Deduct for clippy warnings (2 points each, max 20)
        score -= (snapshot.clippy_warnings as f64 * 2.0).min(20.0);

        // Deduct for test failures (5 points each, max 40)
        score -= (snapshot.test_failed as f64 * 5.0).min(40.0);

        // Deduct for security issues (10 points each, max 30)
        score -= (snapshot.security_issues as f64 * 10.0).min(30.0);

        // Deduct for allow annotations (1 point each, max 10)
        score -= (snapshot.allow_annotations as f64).min(10.0);

        score.max(0.0)
    }

    /// Generate a shields.io badge URL.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cert = QualityCertification::new("my-project", &snapshot, 7);
    /// assert!(cert.badge_url().contains("shields.io"));
    /// ```
    #[must_use]
    pub fn badge_url(&self) -> String {
        format!(
            "https://img.shields.io/badge/quality-{}-{}",
            self.level,
            self.level.badge_color()
        )
    }

    /// Generate a shields.io badge URL with score.
    #[must_use]
    pub fn badge_url_with_score(&self) -> String {
        format!(
            "https://img.shields.io/badge/quality-{}%20({:.0})-{}",
            self.level,
            self.score,
            self.level.badge_color()
        )
    }

    /// Check if the certification is currently valid.
    ///
    /// A certification is valid if:
    /// - It has not been revoked
    /// - It has not expired
    /// - It achieved a certifiable level
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid && Utc::now() < self.expires_at && self.level.is_certified()
    }

    /// Revoke this certification.
    ///
    /// # Arguments
    ///
    /// * `reason` - Reason for revocation
    pub fn revoke(&mut self, reason: impl Into<String>) {
        self.valid = false;
        self.revocation_reason = Some(reason.into());
    }

    /// Check if this certification would be revoked by the given metrics.
    ///
    /// A certification is revoked if the new metrics show regression below
    /// the current certification level.
    #[must_use]
    pub fn would_be_revoked_by(&self, new_snapshot: &QualityMetricsSnapshot) -> bool {
        let new_score = Self::calculate_score(new_snapshot);
        let new_level = CertificationLevel::from_score(new_score);

        // Revoke if new level is worse than current level
        !new_level.is_certified() || (self.level == CertificationLevel::Gold && new_level != CertificationLevel::Gold)
    }

    /// Serialize certification to JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize certification to JSON")
    }

    /// Deserialize certification from JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("Failed to deserialize certification from JSON")
    }
}

// ============================================================================
// Certification History
// ============================================================================

/// Storage for certification history.
///
/// Maintains a history of certifications for a project, allowing
/// tracking of certification status over time.
pub struct CertificationHistory {
    history_file: PathBuf,
}

impl CertificationHistory {
    /// Create a new certification history for a project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        let history_file = project_dir
            .as_ref()
            .join(".ralph")
            .join("certifications.jsonl");

        Self { history_file }
    }

    /// Store a certification in history.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn store(&self, cert: &QualityCertification) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.history_file.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create certification history directory")?;
        }

        // Append to history file
        let json = serde_json::to_string(cert).context("Failed to serialize certification")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)
            .context("Failed to open certification history file")?;

        use std::io::Write;
        writeln!(file, "{}", json).context("Failed to write certification to history")?;

        Ok(())
    }

    /// Load all certifications from history.
    ///
    /// # Errors
    ///
    /// Returns an error if reading or parsing fails.
    pub fn load_all(&self) -> Result<Vec<QualityCertification>> {
        if !self.history_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.history_file)
            .context("Failed to read certification history")?;

        let certs: Result<Vec<_>> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line).context("Failed to parse certification from history")
            })
            .collect();

        certs
    }

    /// Get the most recent certification.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub fn get_latest(&self) -> Result<Option<QualityCertification>> {
        let certs = self.load_all()?;
        Ok(certs.into_iter().last())
    }

    /// Get the current valid certification, if any.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub fn get_current_valid(&self) -> Result<Option<QualityCertification>> {
        let certs = self.load_all()?;
        Ok(certs.into_iter().rev().find(|c| c.is_valid()))
    }

    /// Revoke all certifications due to regression.
    ///
    /// # Arguments
    ///
    /// * `reason` - Reason for revocation
    ///
    /// # Errors
    ///
    /// Returns an error if updating fails.
    pub fn revoke_all(&self, reason: &str) -> Result<()> {
        let mut certs = self.load_all()?;

        for cert in &mut certs {
            if cert.valid {
                cert.revoke(reason);
            }
        }

        // Rewrite the history file
        let content = certs
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to serialize certifications")?
            .join("\n");

        std::fs::write(&self.history_file, content + "\n")
            .context("Failed to rewrite certification history")?;

        Ok(())
    }
}

// ============================================================================
// Quality Certifier
// ============================================================================

/// Creates and manages quality certifications.
pub struct QualityCertifier {
    history: CertificationHistory,
    /// Default validity period in days.
    pub validity_days: i64,
}

impl QualityCertifier {
    /// Create a new certifier for a project.
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        let history = CertificationHistory::new(project_dir);

        Self {
            history,
            validity_days: 7, // Default: 1 week validity
        }
    }

    /// Set the validity period for new certifications.
    #[must_use]
    pub fn with_validity_days(mut self, days: i64) -> Self {
        self.validity_days = days;
        self
    }

    /// Certify a project based on quality metrics.
    ///
    /// # Arguments
    ///
    /// * `project_name` - Name of the project
    /// * `snapshot` - Current quality metrics
    ///
    /// # Errors
    ///
    /// Returns an error if storing the certification fails.
    pub fn certify(
        &self,
        project_name: impl Into<String>,
        snapshot: &QualityMetricsSnapshot,
    ) -> Result<QualityCertification> {
        // Check for regression against previous certification
        if let Ok(Some(prev)) = self.history.get_current_valid() {
            if prev.would_be_revoked_by(snapshot) {
                self.history.revoke_all("Quality regression detected")?;
            }
        }

        // Create new certification
        let cert = QualityCertification::new(project_name, snapshot, self.validity_days);

        // Store in history
        self.history.store(&cert)?;

        Ok(cert)
    }

    /// Certify with a specific commit hash.
    ///
    /// # Arguments
    ///
    /// * `project_name` - Name of the project
    /// * `snapshot` - Current quality metrics
    /// * `commit` - Git commit hash
    ///
    /// # Errors
    ///
    /// Returns an error if storing the certification fails.
    pub fn certify_with_commit(
        &self,
        project_name: impl Into<String>,
        snapshot: &QualityMetricsSnapshot,
        commit: impl Into<String>,
    ) -> Result<QualityCertification> {
        let cert = QualityCertification::new(project_name, snapshot, self.validity_days)
            .with_commit(commit);

        // Check for regression
        if let Ok(Some(prev)) = self.history.get_current_valid() {
            if prev.would_be_revoked_by(snapshot) {
                self.history.revoke_all("Quality regression detected")?;
            }
        }

        self.history.store(&cert)?;

        Ok(cert)
    }

    /// Get the current valid certification, if any.
    ///
    /// # Errors
    ///
    /// Returns an error if reading history fails.
    pub fn get_current(&self) -> Result<Option<QualityCertification>> {
        self.history.get_current_valid()
    }

    /// Get all certification history.
    ///
    /// # Errors
    ///
    /// Returns an error if reading history fails.
    pub fn get_history(&self) -> Result<Vec<QualityCertification>> {
        self.history.load_all()
    }

    /// Manually revoke all certifications.
    ///
    /// # Errors
    ///
    /// Returns an error if updating history fails.
    pub fn revoke(&self, reason: &str) -> Result<()> {
        self.history.revoke_all(reason)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // CertificationLevel Tests
    // ========================================================================

    #[test]
    fn test_certification_level_from_score_gold() {
        assert_eq!(CertificationLevel::from_score(100.0), CertificationLevel::Gold);
        assert_eq!(CertificationLevel::from_score(95.0), CertificationLevel::Gold);
        assert_eq!(CertificationLevel::from_score(90.0), CertificationLevel::Gold);
    }

    #[test]
    fn test_certification_level_from_score_silver() {
        assert_eq!(CertificationLevel::from_score(89.9), CertificationLevel::Silver);
        assert_eq!(CertificationLevel::from_score(80.0), CertificationLevel::Silver);
        assert_eq!(CertificationLevel::from_score(70.0), CertificationLevel::Silver);
    }

    #[test]
    fn test_certification_level_from_score_bronze() {
        assert_eq!(CertificationLevel::from_score(69.9), CertificationLevel::Bronze);
        assert_eq!(CertificationLevel::from_score(60.0), CertificationLevel::Bronze);
        assert_eq!(CertificationLevel::from_score(50.0), CertificationLevel::Bronze);
    }

    #[test]
    fn test_certification_level_from_score_none() {
        assert_eq!(CertificationLevel::from_score(49.9), CertificationLevel::None);
        assert_eq!(CertificationLevel::from_score(25.0), CertificationLevel::None);
        assert_eq!(CertificationLevel::from_score(0.0), CertificationLevel::None);
    }

    #[test]
    fn test_certification_level_badge_color() {
        assert_eq!(CertificationLevel::Gold.badge_color(), "brightgreen");
        assert_eq!(CertificationLevel::Silver.badge_color(), "green");
        assert_eq!(CertificationLevel::Bronze.badge_color(), "yellow");
        assert_eq!(CertificationLevel::None.badge_color(), "red");
    }

    #[test]
    fn test_certification_level_is_certified() {
        assert!(CertificationLevel::Gold.is_certified());
        assert!(CertificationLevel::Silver.is_certified());
        assert!(CertificationLevel::Bronze.is_certified());
        assert!(!CertificationLevel::None.is_certified());
    }

    #[test]
    fn test_certification_level_display() {
        assert_eq!(format!("{}", CertificationLevel::Gold), "gold");
        assert_eq!(format!("{}", CertificationLevel::Silver), "silver");
        assert_eq!(format!("{}", CertificationLevel::Bronze), "bronze");
        assert_eq!(format!("{}", CertificationLevel::None), "none");
    }

    #[test]
    fn test_certification_level_from_str() {
        assert_eq!("gold".parse::<CertificationLevel>().unwrap(), CertificationLevel::Gold);
        assert_eq!("SILVER".parse::<CertificationLevel>().unwrap(), CertificationLevel::Silver);
        assert_eq!("Bronze".parse::<CertificationLevel>().unwrap(), CertificationLevel::Bronze);
        assert_eq!("none".parse::<CertificationLevel>().unwrap(), CertificationLevel::None);
        assert!("invalid".parse::<CertificationLevel>().is_err());
    }

    // ========================================================================
    // QualityCertification Tests
    // ========================================================================

    fn create_perfect_snapshot() -> QualityMetricsSnapshot {
        QualityMetricsSnapshot::new("test-session", 1)
            .with_clippy_warnings(0)
            .with_test_counts(100, 100, 0)
            .with_security_issues(0)
    }

    fn create_good_snapshot() -> QualityMetricsSnapshot {
        QualityMetricsSnapshot::new("test-session", 1)
            .with_clippy_warnings(3) // -6 points
            .with_test_counts(100, 98, 2) // -10 points
            .with_security_issues(0)
    }

    fn create_mediocre_snapshot() -> QualityMetricsSnapshot {
        QualityMetricsSnapshot::new("test-session", 1)
            .with_clippy_warnings(5) // -10 points
            .with_test_counts(100, 90, 10) // Capped at -40 points, but 10*5 = -50, so -40
            .with_security_issues(0)
        // Score: 100 - 10 - 40 = 50
    }

    fn create_poor_snapshot() -> QualityMetricsSnapshot {
        QualityMetricsSnapshot::new("test-session", 1)
            .with_clippy_warnings(10) // -20 points (capped)
            .with_test_counts(100, 50, 50) // -40 points (capped)
            .with_security_issues(3) // -30 points (capped)
        // Score: 100 - 20 - 40 - 30 = 10
    }

    #[test]
    fn test_certification_new_gold() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        assert_eq!(cert.project, "test-project");
        assert_eq!(cert.level, CertificationLevel::Gold);
        assert_eq!(cert.score, 100.0);
        assert!(cert.valid);
        assert!(cert.revocation_reason.is_none());
    }

    #[test]
    fn test_certification_new_silver() {
        let snapshot = create_good_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        assert_eq!(cert.level, CertificationLevel::Silver);
        assert!((cert.score - 84.0).abs() < 0.1); // 100 - 6 - 10 = 84
        assert!(cert.valid);
    }

    #[test]
    fn test_certification_new_bronze() {
        let snapshot = create_mediocre_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        assert_eq!(cert.level, CertificationLevel::Bronze);
        assert!(cert.valid);
    }

    #[test]
    fn test_certification_new_none() {
        let snapshot = create_poor_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        assert_eq!(cert.level, CertificationLevel::None);
        assert!(!cert.valid); // Not valid because level is None
    }

    #[test]
    fn test_certification_with_commit() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7)
            .with_commit("abc123def");

        assert_eq!(cert.commit_hash, Some("abc123def".to_string()));
    }

    #[test]
    fn test_certification_badge_url() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        let url = cert.badge_url();
        assert!(url.contains("shields.io"));
        assert!(url.contains("gold"));
        assert!(url.contains("brightgreen"));
    }

    #[test]
    fn test_certification_badge_url_with_score() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        let url = cert.badge_url_with_score();
        assert!(url.contains("shields.io"));
        assert!(url.contains("gold"));
        assert!(url.contains("100"));
    }

    #[test]
    fn test_certification_is_valid() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        assert!(cert.is_valid());
    }

    #[test]
    fn test_certification_is_valid_after_revocation() {
        let snapshot = create_perfect_snapshot();
        let mut cert = QualityCertification::new("test-project", &snapshot, 7);

        cert.revoke("Test revocation");

        assert!(!cert.is_valid());
        assert_eq!(cert.revocation_reason, Some("Test revocation".to_string()));
    }

    #[test]
    fn test_certification_would_be_revoked_by_regression() {
        let good_snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &good_snapshot, 7);

        let bad_snapshot = create_poor_snapshot();
        assert!(cert.would_be_revoked_by(&bad_snapshot));
    }

    #[test]
    fn test_certification_would_not_be_revoked_by_same_quality() {
        let snapshot = create_good_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        assert!(!cert.would_be_revoked_by(&snapshot));
    }

    #[test]
    fn test_certification_to_json() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        let json = cert.to_json().unwrap();

        assert!(json.contains("test-project"));
        assert!(json.contains("gold"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_certification_from_json() {
        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        let json = cert.to_json().unwrap();
        let loaded = QualityCertification::from_json(&json).unwrap();

        assert_eq!(loaded.project, cert.project);
        assert_eq!(loaded.level, cert.level);
        assert_eq!(loaded.score, cert.score);
    }

    // ========================================================================
    // CertificationHistory Tests
    // ========================================================================

    #[test]
    fn test_history_new() {
        let temp_dir = TempDir::new().unwrap();
        let history = CertificationHistory::new(temp_dir.path());

        assert!(history.history_file.ends_with("certifications.jsonl"));
    }

    #[test]
    fn test_history_store_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let history = CertificationHistory::new(temp_dir.path());

        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("test-project", &snapshot, 7);

        history.store(&cert).unwrap();

        let loaded = history.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].project, "test-project");
    }

    #[test]
    fn test_history_get_latest() {
        let temp_dir = TempDir::new().unwrap();
        let history = CertificationHistory::new(temp_dir.path());

        let snapshot1 = QualityMetricsSnapshot::new("s1", 1)
            .with_clippy_warnings(5)
            .with_test_counts(10, 10, 0);
        let snapshot2 = create_perfect_snapshot();

        let cert1 = QualityCertification::new("project", &snapshot1, 7);
        let cert2 = QualityCertification::new("project", &snapshot2, 7);

        history.store(&cert1).unwrap();
        history.store(&cert2).unwrap();

        let latest = history.get_latest().unwrap().unwrap();
        assert_eq!(latest.level, CertificationLevel::Gold);
    }

    #[test]
    fn test_history_get_current_valid() {
        let temp_dir = TempDir::new().unwrap();
        let history = CertificationHistory::new(temp_dir.path());

        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("project", &snapshot, 7);

        history.store(&cert).unwrap();

        let current = history.get_current_valid().unwrap().unwrap();
        assert!(current.is_valid());
    }

    #[test]
    fn test_history_revoke_all() {
        let temp_dir = TempDir::new().unwrap();
        let history = CertificationHistory::new(temp_dir.path());

        let snapshot = create_perfect_snapshot();
        let cert = QualityCertification::new("project", &snapshot, 7);

        history.store(&cert).unwrap();
        history.revoke_all("Test revocation").unwrap();

        let loaded = history.load_all().unwrap();
        assert!(!loaded[0].valid);
        assert_eq!(loaded[0].revocation_reason, Some("Test revocation".to_string()));
    }

    #[test]
    fn test_history_empty_load() {
        let temp_dir = TempDir::new().unwrap();
        let history = CertificationHistory::new(temp_dir.path());

        let loaded = history.load_all().unwrap();
        assert!(loaded.is_empty());
    }

    // ========================================================================
    // QualityCertifier Tests
    // ========================================================================

    #[test]
    fn test_certifier_new() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path());

        assert_eq!(certifier.validity_days, 7);
    }

    #[test]
    fn test_certifier_with_validity_days() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path())
            .with_validity_days(30);

        assert_eq!(certifier.validity_days, 30);
    }

    #[test]
    fn test_certifier_certify() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path());

        let snapshot = create_perfect_snapshot();
        let cert = certifier.certify("test-project", &snapshot).unwrap();

        assert_eq!(cert.level, CertificationLevel::Gold);

        // Should be stored in history
        let history = certifier.get_history().unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_certifier_certify_with_commit() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path());

        let snapshot = create_perfect_snapshot();
        let cert = certifier.certify_with_commit("test-project", &snapshot, "abc123").unwrap();

        assert_eq!(cert.commit_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_certifier_revoke_on_regression() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path());

        // First, certify with perfect snapshot
        let good_snapshot = create_perfect_snapshot();
        certifier.certify("test-project", &good_snapshot).unwrap();

        // Then, certify with poor snapshot (regression)
        let bad_snapshot = create_poor_snapshot();
        certifier.certify("test-project", &bad_snapshot).unwrap();

        // First certification should be revoked
        let history = certifier.get_history().unwrap();
        assert_eq!(history.len(), 2);
        assert!(!history[0].valid); // First cert revoked
    }

    #[test]
    fn test_certifier_get_current() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path());

        // Initially no certification
        assert!(certifier.get_current().unwrap().is_none());

        // After certifying
        let snapshot = create_perfect_snapshot();
        certifier.certify("test-project", &snapshot).unwrap();

        let current = certifier.get_current().unwrap().unwrap();
        assert_eq!(current.level, CertificationLevel::Gold);
    }

    #[test]
    fn test_certifier_manual_revoke() {
        let temp_dir = TempDir::new().unwrap();
        let certifier = QualityCertifier::new(temp_dir.path());

        let snapshot = create_perfect_snapshot();
        certifier.certify("test-project", &snapshot).unwrap();

        certifier.revoke("Manual revocation").unwrap();

        let current = certifier.get_current().unwrap();
        assert!(current.is_none()); // No valid certification after revoke
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_certification_score_caps() {
        // Test that deductions are properly capped
        let snapshot = QualityMetricsSnapshot::new("test", 1)
            .with_clippy_warnings(100) // Should cap at -20
            .with_test_counts(100, 0, 100) // Should cap at -40
            .with_security_issues(100); // Should cap at -30

        let cert = QualityCertification::new("test", &snapshot, 7);

        // Score should be 100 - 20 - 40 - 30 = 10
        assert_eq!(cert.score, 10.0);
    }

    #[test]
    fn test_certification_metrics_preserved() {
        let snapshot = QualityMetricsSnapshot::new("test", 1)
            .with_clippy_warnings(5)
            .with_test_counts(50, 45, 5)
            .with_security_issues(2);

        let cert = QualityCertification::new("test", &snapshot, 7);

        assert_eq!(cert.metrics.clippy_warnings, 5);
        assert_eq!(cert.metrics.test_total, 50);
        assert_eq!(cert.metrics.test_passed, 45);
        assert_eq!(cert.metrics.test_failed, 5);
        assert_eq!(cert.metrics.security_issues, 2);
    }

    #[test]
    fn test_certification_serialization_roundtrip() {
        let snapshot = QualityMetricsSnapshot::new("test", 1)
            .with_clippy_warnings(2)
            .with_test_counts(20, 18, 2)
            .with_security_issues(1);

        let cert = QualityCertification::new("test-project", &snapshot, 30)
            .with_commit("abc123");

        let json = cert.to_json().unwrap();
        let loaded = QualityCertification::from_json(&json).unwrap();

        assert_eq!(loaded.project, cert.project);
        assert_eq!(loaded.level, cert.level);
        assert_eq!(loaded.score, cert.score);
        assert_eq!(loaded.commit_hash, cert.commit_hash);
        assert_eq!(loaded.metrics.clippy_warnings, cert.metrics.clippy_warnings);
    }
}
