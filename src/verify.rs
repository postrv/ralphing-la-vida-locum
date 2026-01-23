//! CCG-Diff Verification stub for provable quality improvement verification.
//!
//! This module provides a stub for CCG-Diff verification - a system that
//! compares code changes against CCG constraints to verify quality improvements.
//! The cloud-based verification is not yet implemented - this is a placeholder
//! for future integration (Phase 18.3).
//!
//! # CCG-Diff Concepts
//!
//! **CCG-Diff** compares two CCG snapshots (before and after a change) to:
//!
//! - Detect quality improvements (reduced complexity, better structure)
//! - Identify potential regressions (new violations, increased complexity)
//! - Provide provable evidence that changes improve code quality
//!
//! # Verification Flow
//!
//! 1. Capture CCG snapshot before changes
//! 2. Apply code changes
//! 3. Capture CCG snapshot after changes
//! 4. Compare snapshots to produce verification report
//! 5. Report shows quality delta (improvement or regression)
//!
//! # narsil-mcp Integration Points
//!
//! When narsil-mcp is available, the verifier can use these tools:
//!
//! - `get_complexity` - Measure function complexity before/after
//! - `find_dead_code` - Detect removed dead code
//! - `scan_security` - Compare security findings
//! - `get_call_graph` - Analyze structural changes
//! - `check_type_errors` - Verify type safety improvements
//!
//! These hooks are prepared in this module but require narsil-mcp to function.
//! When unavailable, the mock verifier returns simulated results.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::verify::{CcgVerifier, MockCcgVerifier, VerificationConfig};
//!
//! // Use mock verifier for development
//! let config = VerificationConfig::default();
//! let verifier = MockCcgVerifier::new(config);
//!
//! // Verify changes
//! let report = verifier.verify_changes(".")?;
//! if report.overall_improvement() {
//!     println!("Quality improved: {}", report.summary);
//! }
//! ```
//!
//! # Cloud Feature Roadmap
//!
//! Cloud verification features are planned for future releases:
//!
//! - **Phase 1**: Local CCG-diff with narsil-mcp
//! - **Phase 2**: Cloud-based verification with persistent history
//! - **Phase 3**: Team-wide quality tracking and trending
//! - **Phase 4**: CI/CD integration with quality gates

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for CCG-Diff verification.
///
/// Controls verification behavior and integration settings.
///
/// # Example
///
/// ```
/// use ralph::verify::VerificationConfig;
///
/// let config = VerificationConfig::default();
/// assert!(!config.mock_mode);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Enable mock mode for development/testing.
    ///
    /// When enabled, the verifier returns simulated results without
    /// actually comparing CCG snapshots.
    #[serde(default)]
    pub mock_mode: bool,

    /// Path to log stub operations.
    ///
    /// When using the stub verifier, operations that would be performed
    /// are logged to this file for debugging/auditing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,

    /// narsil-mcp binary path (optional).
    ///
    /// If specified, uses this path for narsil-mcp integration.
    /// Otherwise, uses the system PATH.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narsil_binary: Option<PathBuf>,

    /// Minimum improvement threshold (0.0-1.0).
    ///
    /// Changes must show at least this much improvement to be
    /// considered "quality improved". Default: 0.0 (any improvement).
    #[serde(default)]
    pub min_improvement_threshold: f64,
}

// ============================================================================
// Verification Report Types
// ============================================================================

/// Quality delta for a specific metric.
///
/// Represents the change in a quality metric between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityDelta {
    /// Name of the metric.
    pub metric: String,

    /// Value before changes.
    pub before: f64,

    /// Value after changes.
    pub after: f64,

    /// Whether lower is better for this metric.
    pub lower_is_better: bool,
}

impl QualityDelta {
    /// Create a new quality delta.
    #[must_use]
    pub fn new(metric: impl Into<String>, before: f64, after: f64, lower_is_better: bool) -> Self {
        Self {
            metric: metric.into(),
            before,
            after,
            lower_is_better,
        }
    }

    /// Calculate the improvement as a percentage.
    ///
    /// Returns positive values for improvements, negative for regressions.
    #[must_use]
    pub fn improvement_percent(&self) -> f64 {
        if self.before == 0.0 {
            return if self.after == 0.0 { 0.0 } else { -100.0 };
        }

        let change_percent = ((self.before - self.after) / self.before) * 100.0;
        if self.lower_is_better {
            change_percent
        } else {
            -change_percent
        }
    }

    /// Check if this delta represents an improvement.
    #[must_use]
    pub fn is_improvement(&self) -> bool {
        if self.lower_is_better {
            self.after < self.before
        } else {
            self.after > self.before
        }
    }
}

/// Severity of a verification finding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum VerificationSeverity {
    /// Informational - not a blocking issue.
    #[default]
    Info,
    /// Warning - should be addressed but not blocking.
    Warning,
    /// Error - blocks commit.
    Error,
    /// Critical - blocks all progress.
    Critical,
}

impl std::fmt::Display for VerificationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationSeverity::Info => write!(f, "INFO"),
            VerificationSeverity::Warning => write!(f, "WARNING"),
            VerificationSeverity::Error => write!(f, "ERROR"),
            VerificationSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A finding from the verification process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationFinding {
    /// Severity of the finding.
    pub severity: VerificationSeverity,

    /// Category of the finding.
    pub category: String,

    /// Human-readable message.
    pub message: String,

    /// Affected file (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,

    /// Affected line (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,

    /// Suggested fix (if available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl VerificationFinding {
    /// Create a new verification finding.
    #[must_use]
    pub fn new(
        severity: VerificationSeverity,
        category: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            category: category.into(),
            message: message.into(),
            file: None,
            line: None,
            suggestion: None,
        }
    }

    /// Set the affected file.
    #[must_use]
    pub fn with_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set the affected line.
    #[must_use]
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the suggested fix.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Result of CCG-Diff verification.
///
/// Contains the comparison results between two CCG snapshots.
///
/// # JSON Schema
///
/// The verification report follows this JSON schema:
///
/// ```json
/// {
///   "$schema": "http://json-schema.org/draft-07/schema#",
///   "title": "VerificationReport",
///   "type": "object",
///   "required": ["verified_at", "summary", "quality_improved", "deltas", "findings"],
///   "properties": {
///     "verified_at": {
///       "type": "string",
///       "format": "date-time"
///     },
///     "summary": {
///       "type": "string"
///     },
///     "quality_improved": {
///       "type": "boolean"
///     },
///     "improvement_score": {
///       "type": "number"
///     },
///     "deltas": {
///       "type": "array",
///       "items": {
///         "type": "object",
///         "required": ["metric", "before", "after", "lower_is_better"],
///         "properties": {
///           "metric": {"type": "string"},
///           "before": {"type": "number"},
///           "after": {"type": "number"},
///           "lower_is_better": {"type": "boolean"}
///         }
///       }
///     },
///     "findings": {
///       "type": "array",
///       "items": {
///         "type": "object",
///         "required": ["severity", "category", "message"],
///         "properties": {
///           "severity": {"type": "string", "enum": ["info", "warning", "error", "critical"]},
///           "category": {"type": "string"},
///           "message": {"type": "string"},
///           "file": {"type": "string"},
///           "line": {"type": "integer"},
///           "suggestion": {"type": "string"}
///         }
///       }
///     },
///     "metadata": {
///       "type": "object",
///       "additionalProperties": {"type": "string"}
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    /// When the verification was performed.
    pub verified_at: DateTime<Utc>,

    /// Human-readable summary of the verification result.
    pub summary: String,

    /// Whether overall quality improved.
    pub quality_improved: bool,

    /// Improvement score (0.0-1.0, higher is better).
    #[serde(default)]
    pub improvement_score: f64,

    /// Quality deltas for each metric.
    #[serde(default)]
    pub deltas: Vec<QualityDelta>,

    /// Findings from the verification.
    #[serde(default)]
    pub findings: Vec<VerificationFinding>,

    /// Additional metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl VerificationReport {
    /// Create a new verification report.
    #[must_use]
    pub fn new(summary: impl Into<String>, quality_improved: bool) -> Self {
        Self {
            verified_at: Utc::now(),
            summary: summary.into(),
            quality_improved,
            improvement_score: if quality_improved { 1.0 } else { 0.0 },
            deltas: Vec::new(),
            findings: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Check if overall quality improved.
    #[must_use]
    pub fn overall_improvement(&self) -> bool {
        self.quality_improved
    }

    /// Add a quality delta.
    #[must_use]
    pub fn with_delta(mut self, delta: QualityDelta) -> Self {
        self.deltas.push(delta);
        self
    }

    /// Add a finding.
    #[must_use]
    pub fn with_finding(mut self, finding: VerificationFinding) -> Self {
        self.findings.push(finding);
        self
    }

    /// Set the improvement score.
    #[must_use]
    pub fn with_improvement_score(mut self, score: f64) -> Self {
        self.improvement_score = score.clamp(0.0, 1.0);
        self
    }

    /// Add metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get findings by severity.
    #[must_use]
    pub fn findings_by_severity(&self, severity: VerificationSeverity) -> Vec<&VerificationFinding> {
        self.findings.iter().filter(|f| f.severity == severity).collect()
    }

    /// Check if there are any blocking findings.
    #[must_use]
    pub fn has_blocking_findings(&self) -> bool {
        self.findings
            .iter()
            .any(|f| matches!(f.severity, VerificationSeverity::Error | VerificationSeverity::Critical))
    }

    /// Serialize to JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::RalphError::Internal(format!("JSON serialization failed: {}", e)))
    }
}

// ============================================================================
// CcgVerifier Trait
// ============================================================================

/// Trait for CCG-Diff verification implementations.
///
/// Provides verification operations that compare CCG snapshots before
/// and after changes to determine quality improvements.
///
/// # CCG Integration Requirements
///
/// Implementations that integrate with narsil-mcp should:
///
/// 1. Call `get_complexity` for before/after complexity comparison
/// 2. Call `scan_security` for before/after security comparison
/// 3. Call `find_dead_code` to detect removed dead code
/// 4. Call `get_call_graph` to analyze structural changes
/// 5. Aggregate results into a `VerificationReport`
///
/// When narsil-mcp is unavailable, implementations should gracefully
/// degrade to returning mock or cached results.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::verify::{CcgVerifier, MockCcgVerifier, VerificationConfig};
///
/// let config = VerificationConfig::default();
/// let verifier = MockCcgVerifier::new(config);
///
/// // Verify changes in a directory
/// let report = verifier.verify_changes(".")?;
/// println!("Quality improved: {}", report.quality_improved);
/// ```
pub trait CcgVerifier: Send + Sync {
    /// Verify changes in the given path.
    ///
    /// Compares the current state against a baseline (e.g., last commit)
    /// and produces a verification report.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the project or file to verify
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails.
    fn verify_changes(&self, path: &str) -> Result<VerificationReport>;

    /// Verify changes between two specific commits/snapshots.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the project
    /// * `before_ref` - Git ref or snapshot ID for "before" state
    /// * `after_ref` - Git ref or snapshot ID for "after" state
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails.
    fn verify_between(&self, path: &str, before_ref: &str, after_ref: &str) -> Result<VerificationReport>;

    /// Check if the verifier is using mock mode.
    #[must_use]
    fn is_mock(&self) -> bool;

    /// Check if narsil-mcp integration is available.
    #[must_use]
    fn narsil_available(&self) -> bool;

    /// Get the verifier configuration.
    #[must_use]
    fn config(&self) -> &VerificationConfig;
}

// ============================================================================
// Mock CCG Verifier
// ============================================================================

/// Mock CCG verifier for development and testing.
///
/// Returns simulated "quality improved" results without actually
/// comparing CCG snapshots. Useful for development, testing, and
/// demonstrations.
///
/// # Example
///
/// ```
/// use ralph::verify::{MockCcgVerifier, VerificationConfig, CcgVerifier};
///
/// let config = VerificationConfig { mock_mode: true, ..Default::default() };
/// let verifier = MockCcgVerifier::new(config);
///
/// // Mock verifier always returns "quality improved"
/// let report = verifier.verify_changes(".").unwrap();
/// assert!(report.quality_improved);
/// assert!(report.summary.contains("Mock"));
/// ```
#[derive(Debug)]
pub struct MockCcgVerifier {
    config: VerificationConfig,
}

impl MockCcgVerifier {
    /// Create a new mock CCG verifier.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the verifier
    #[must_use]
    pub fn new(config: VerificationConfig) -> Self {
        Self { config }
    }

    /// Log a mock operation to the configured log file.
    fn log_mock_operation(&self, operation: &str, details: &str) {
        use std::fs::OpenOptions;
        use std::io::Write;

        let Some(log_file) = &self.config.log_file else {
            return;
        };

        // Create parent directories if needed
        if let Some(parent) = log_file.parent() {
            if !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_file) else {
            return;
        };

        let log_entry = serde_json::json!({
            "mock_log": true,
            "message": format!("MOCK: CCG verification '{}' simulated", operation),
            "operation": operation,
            "details": details,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let _ = writeln!(file, "{}", serde_json::to_string(&log_entry).unwrap_or_default());
    }

    /// Generate mock quality deltas.
    fn mock_deltas(&self) -> Vec<QualityDelta> {
        vec![
            QualityDelta::new("cyclomatic_complexity", 15.0, 12.0, true),
            QualityDelta::new("cognitive_complexity", 22.0, 18.0, true),
            QualityDelta::new("security_findings", 3.0, 1.0, true),
            QualityDelta::new("dead_code_blocks", 5.0, 2.0, true),
            QualityDelta::new("test_coverage", 0.75, 0.82, false),
        ]
    }

    /// Generate mock findings.
    fn mock_findings(&self) -> Vec<VerificationFinding> {
        vec![
            VerificationFinding::new(
                VerificationSeverity::Info,
                "complexity",
                "Cyclomatic complexity reduced by 20%",
            ),
            VerificationFinding::new(
                VerificationSeverity::Info,
                "security",
                "2 security findings resolved",
            ),
            VerificationFinding::new(
                VerificationSeverity::Info,
                "dead_code",
                "3 dead code blocks removed",
            ),
        ]
    }
}

impl CcgVerifier for MockCcgVerifier {
    fn verify_changes(&self, path: &str) -> Result<VerificationReport> {
        self.log_mock_operation("verify_changes", &format!("path={}", path));

        let report = VerificationReport::new(
            "Mock verification: Quality improved (simulated result)",
            true,
        )
        .with_improvement_score(0.85)
        .with_metadata("mock", "true")
        .with_metadata("path", path);

        // Add mock deltas
        let mut report = report;
        for delta in self.mock_deltas() {
            report = report.with_delta(delta);
        }

        // Add mock findings
        for finding in self.mock_findings() {
            report = report.with_finding(finding);
        }

        Ok(report)
    }

    fn verify_between(&self, path: &str, before_ref: &str, after_ref: &str) -> Result<VerificationReport> {
        self.log_mock_operation(
            "verify_between",
            &format!("path={}, before={}, after={}", path, before_ref, after_ref),
        );

        let report = VerificationReport::new(
            format!(
                "Mock verification: Quality improved from {} to {} (simulated)",
                before_ref, after_ref
            ),
            true,
        )
        .with_improvement_score(0.85)
        .with_metadata("mock", "true")
        .with_metadata("path", path)
        .with_metadata("before_ref", before_ref)
        .with_metadata("after_ref", after_ref);

        // Add mock deltas
        let mut report = report;
        for delta in self.mock_deltas() {
            report = report.with_delta(delta);
        }

        // Add mock findings
        for finding in self.mock_findings() {
            report = report.with_finding(finding);
        }

        Ok(report)
    }

    fn is_mock(&self) -> bool {
        true
    }

    fn narsil_available(&self) -> bool {
        false
    }

    fn config(&self) -> &VerificationConfig {
        &self.config
    }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Create a CCG verifier based on configuration.
///
/// Currently returns a `MockCcgVerifier` as the real implementation
/// is not yet available. When narsil-mcp integration is complete,
/// this will return an appropriate implementation based on availability.
///
/// # Example
///
/// ```
/// use ralph::verify::{create_verifier, VerificationConfig, CcgVerifier};
///
/// let config = VerificationConfig::default();
/// let verifier = create_verifier(config);
///
/// // Currently always mock until narsil-mcp integration is complete
/// assert!(verifier.is_mock());
/// ```
#[must_use]
pub fn create_verifier(config: VerificationConfig) -> Box<dyn CcgVerifier> {
    // TODO: When narsil-mcp integration is complete, check for availability
    // and return a real verifier when possible.
    //
    // For now, always return mock verifier.
    Box::new(MockCcgVerifier::new(config))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // CCG-Diff Trait Tests (Phase 18.3)
    // ========================================================================

    #[test]
    fn test_ccg_verifier_trait_is_defined() {
        // Verify the trait can be used as a trait object
        let config = VerificationConfig::default();
        let verifier = MockCcgVerifier::new(config);
        let _boxed: Box<dyn CcgVerifier> = Box::new(verifier);
    }

    #[test]
    fn test_mock_verifier_returns_quality_improved() {
        let config = VerificationConfig {
            mock_mode: true,
            ..Default::default()
        };
        let verifier = MockCcgVerifier::new(config);

        let report = verifier.verify_changes(".").unwrap();

        // Mock verifier should always return "quality improved"
        assert!(report.quality_improved);
        assert!(report.overall_improvement());
        assert!(report.summary.to_lowercase().contains("mock"));
    }

    #[test]
    fn test_mock_verifier_returns_quality_deltas() {
        let config = VerificationConfig::default();
        let verifier = MockCcgVerifier::new(config);

        let report = verifier.verify_changes(".").unwrap();

        // Should have quality deltas
        assert!(!report.deltas.is_empty());

        // Check for expected metrics
        let metric_names: Vec<_> = report.deltas.iter().map(|d| d.metric.as_str()).collect();
        assert!(metric_names.contains(&"cyclomatic_complexity"));
        assert!(metric_names.contains(&"security_findings"));
    }

    #[test]
    fn test_verification_config_defaults() {
        let config = VerificationConfig::default();

        assert!(!config.mock_mode);
        assert!(config.log_file.is_none());
        assert!(config.narsil_binary.is_none());
        assert_eq!(config.min_improvement_threshold, 0.0);
    }

    #[test]
    fn test_verification_report_json_schema() {
        let report = VerificationReport::new("Test verification", true)
            .with_improvement_score(0.75)
            .with_delta(QualityDelta::new("complexity", 10.0, 8.0, true))
            .with_finding(VerificationFinding::new(
                VerificationSeverity::Info,
                "test",
                "Test finding",
            ))
            .with_metadata("key", "value");

        // Serialize to JSON
        let json = report.to_json().unwrap();

        // Deserialize back
        let restored: VerificationReport = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.quality_improved, report.quality_improved);
        assert_eq!(restored.summary, report.summary);
        assert_eq!(restored.deltas.len(), 1);
        assert_eq!(restored.findings.len(), 1);
        assert_eq!(restored.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_narsil_hooks_are_prepared() {
        // This test documents the narsil-mcp hooks that will be used
        // when integration is complete. The mock verifier's deltas
        // correspond to these hooks.

        let config = VerificationConfig::default();
        let verifier = MockCcgVerifier::new(config);
        let report = verifier.verify_changes(".").unwrap();

        // Hooks are documented by the metrics in the report:
        // - cyclomatic_complexity -> get_complexity
        // - security_findings -> scan_security
        // - dead_code_blocks -> find_dead_code
        // - test_coverage -> (future hook)

        let metrics: Vec<_> = report.deltas.iter().map(|d| d.metric.as_str()).collect();

        // These metrics correspond to narsil-mcp tools
        assert!(metrics.contains(&"cyclomatic_complexity")); // get_complexity
        assert!(metrics.contains(&"security_findings")); // scan_security
        assert!(metrics.contains(&"dead_code_blocks")); // find_dead_code
    }

    #[test]
    fn test_mock_verifier_logs_to_file() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("verify_stub.jsonl");

        let config = VerificationConfig {
            mock_mode: true,
            log_file: Some(log_path.clone()),
            ..Default::default()
        };

        let verifier = MockCcgVerifier::new(config);

        // Perform verification (will log)
        let _ = verifier.verify_changes(".");

        // Verify the log file contains what was attempted
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("verify_changes"));
        assert!(log_content.contains("MOCK"));
    }

    #[test]
    fn test_quality_delta_improvement_calculation() {
        // Lower is better: complexity reduced
        let delta = QualityDelta::new("complexity", 20.0, 15.0, true);
        assert!(delta.is_improvement());
        assert!(delta.improvement_percent() > 0.0);

        // Lower is better: complexity increased (regression)
        let delta = QualityDelta::new("complexity", 15.0, 20.0, true);
        assert!(!delta.is_improvement());
        assert!(delta.improvement_percent() < 0.0);

        // Higher is better: coverage increased
        let delta = QualityDelta::new("coverage", 0.70, 0.85, false);
        assert!(delta.is_improvement());

        // Higher is better: coverage decreased (regression)
        let delta = QualityDelta::new("coverage", 0.85, 0.70, false);
        assert!(!delta.is_improvement());
    }

    #[test]
    fn test_verification_finding_builder() {
        let finding = VerificationFinding::new(
            VerificationSeverity::Warning,
            "complexity",
            "Function too complex",
        )
        .with_file("src/main.rs")
        .with_line(42)
        .with_suggestion("Consider extracting helper functions");

        assert_eq!(finding.severity, VerificationSeverity::Warning);
        assert_eq!(finding.category, "complexity");
        assert_eq!(finding.file, Some(PathBuf::from("src/main.rs")));
        assert_eq!(finding.line, Some(42));
        assert!(finding.suggestion.is_some());
    }

    #[test]
    fn test_verification_severity_ordering() {
        assert!(VerificationSeverity::Info < VerificationSeverity::Warning);
        assert!(VerificationSeverity::Warning < VerificationSeverity::Error);
        assert!(VerificationSeverity::Error < VerificationSeverity::Critical);
    }

    #[test]
    fn test_report_has_blocking_findings() {
        let mut report = VerificationReport::new("Test", true);

        // No findings = not blocking
        assert!(!report.has_blocking_findings());

        // Info finding = not blocking
        report = report.with_finding(VerificationFinding::new(
            VerificationSeverity::Info,
            "test",
            "Info",
        ));
        assert!(!report.has_blocking_findings());

        // Error finding = blocking
        report = report.with_finding(VerificationFinding::new(
            VerificationSeverity::Error,
            "test",
            "Error",
        ));
        assert!(report.has_blocking_findings());
    }

    #[test]
    fn test_create_verifier_factory() {
        let config = VerificationConfig::default();
        let verifier = create_verifier(config);

        // Currently always returns mock
        assert!(verifier.is_mock());
        assert!(!verifier.narsil_available());
    }

    #[test]
    fn test_verify_between_with_refs() {
        let config = VerificationConfig::default();
        let verifier = MockCcgVerifier::new(config);

        let report = verifier.verify_between(".", "HEAD~1", "HEAD").unwrap();

        assert!(report.quality_improved);
        assert!(report.metadata.contains_key("before_ref"));
        assert!(report.metadata.contains_key("after_ref"));
    }

    #[test]
    fn test_verification_config_serialization() {
        let config = VerificationConfig {
            mock_mode: true,
            log_file: Some(PathBuf::from("/tmp/verify.log")),
            narsil_binary: Some(PathBuf::from("/usr/bin/narsil-mcp")),
            min_improvement_threshold: 0.1,
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: VerificationConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.mock_mode, config.mock_mode);
        assert_eq!(restored.log_file, config.log_file);
        assert_eq!(restored.narsil_binary, config.narsil_binary);
        assert!((restored.min_improvement_threshold - config.min_improvement_threshold).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mock_verifier_is_mock() {
        let config = VerificationConfig::default();
        let verifier = MockCcgVerifier::new(config);

        assert!(verifier.is_mock());
    }

    #[test]
    fn test_improvement_score_clamped() {
        let report = VerificationReport::new("Test", true)
            .with_improvement_score(1.5); // Above max

        assert_eq!(report.improvement_score, 1.0);

        let report = VerificationReport::new("Test", true)
            .with_improvement_score(-0.5); // Below min

        assert_eq!(report.improvement_score, 0.0);
    }

    #[test]
    fn test_findings_by_severity() {
        let report = VerificationReport::new("Test", true)
            .with_finding(VerificationFinding::new(VerificationSeverity::Info, "a", "info"))
            .with_finding(VerificationFinding::new(VerificationSeverity::Warning, "b", "warn"))
            .with_finding(VerificationFinding::new(VerificationSeverity::Info, "c", "info2"));

        let info_findings = report.findings_by_severity(VerificationSeverity::Info);
        assert_eq!(info_findings.len(), 2);

        let warning_findings = report.findings_by_severity(VerificationSeverity::Warning);
        assert_eq!(warning_findings.len(), 1);

        let error_findings = report.findings_by_severity(VerificationSeverity::Error);
        assert_eq!(error_findings.len(), 0);
    }
}
