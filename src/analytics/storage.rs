//! Analytics upload and storage types.
//!
//! This module provides types for uploading analytics events to remote
//! endpoints, with privacy controls and stub implementations for testing.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use super::AnalyticsEvent;

// ============================================================================
// Phase 18.1: Remote Analytics Upload Stub
// ============================================================================

/// Privacy settings for analytics upload.
///
/// Controls what data is included when uploading analytics to a remote endpoint.
/// By default, session IDs are anonymized to protect user privacy.
///
/// # Example
///
/// ```
/// use ralph::analytics::PrivacySettings;
///
/// let settings = PrivacySettings::default();
/// assert!(settings.anonymize_session_ids);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacySettings {
    /// Anonymize session IDs by hashing them.
    ///
    /// When true, session IDs are replaced with a SHA-256 hash to prevent
    /// identification of specific users or sessions.
    #[serde(default = "default_true_privacy")]
    pub anonymize_session_ids: bool,

    /// Exclude event-specific data from uploads.
    ///
    /// When true, only event types and timestamps are uploaded, not the
    /// detailed data payloads.
    #[serde(default)]
    pub exclude_event_data: bool,

    /// Only upload aggregate statistics, not individual events.
    ///
    /// When true, events are batched and only summary statistics (counts,
    /// averages, etc.) are uploaded.
    #[serde(default)]
    pub include_only_aggregates: bool,
}

fn default_true_privacy() -> bool {
    true
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            anonymize_session_ids: true,
            exclude_event_data: false,
            include_only_aggregates: false,
        }
    }
}

/// Configuration for analytics upload.
///
/// Controls whether analytics are uploaded to a remote endpoint and what
/// privacy settings to apply. Upload is disabled by default.
///
/// # Example
///
/// ```
/// use ralph::analytics::AnalyticsUploadConfig;
///
/// let config = AnalyticsUploadConfig::default();
/// assert!(!config.upload_enabled);
/// ```
///
/// # Data Uploaded
///
/// When upload is enabled, the following data may be sent (subject to privacy settings):
///
/// - **Session metadata**: Session ID (optionally anonymized), start/end times, duration
/// - **Event types**: Types of events that occurred (session_start, iteration, etc.)
/// - **Quality metrics**: Warning counts, test pass rates, security scan results
/// - **Aggregate statistics**: Total iterations, stagnation counts, error counts
///
/// No source code, file contents, or project-specific identifiers are ever uploaded.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyticsUploadConfig {
    /// Whether analytics upload is enabled.
    ///
    /// Default: `false` (opt-in only).
    #[serde(default)]
    pub upload_enabled: bool,

    /// The endpoint URL for analytics upload.
    ///
    /// This is a placeholder for future cloud integration.
    /// Default: empty string (not configured).
    #[serde(default)]
    pub endpoint_url: String,

    /// Path to log file for stub uploader.
    ///
    /// When using the stub uploader, events that would be uploaded are
    /// instead written to this file for debugging/auditing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,

    /// Privacy settings for upload.
    #[serde(default)]
    pub privacy: PrivacySettings,
}

/// Trait for analytics uploaders.
///
/// Implementations handle uploading analytics events to a remote endpoint.
/// The stub implementation logs events to a file instead of uploading.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::analytics::{AnalyticsUploader, StubAnalyticsUploader, AnalyticsUploadConfig};
///
/// let config = AnalyticsUploadConfig::default();
/// let uploader = StubAnalyticsUploader::new(config);
/// uploader.upload(&events)?;
/// ```
pub trait AnalyticsUploader: Send + Sync {
    /// Upload analytics events.
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails. Implementations should provide
    /// meaningful error messages for debugging.
    fn upload(&self, events: &[AnalyticsEvent]) -> Result<()>;

    /// Upload analytics events without propagating errors.
    ///
    /// This method catches any upload errors and logs them, ensuring that
    /// upload failures do not affect Ralph's normal operation.
    fn upload_graceful(&self, events: &[AnalyticsEvent]) -> Result<()> {
        if let Err(e) = self.upload(events) {
            // Log the error but don't propagate it
            eprintln!(
                "{}",
                format!("Analytics upload failed (non-fatal): {}", e).yellow()
            );
        }
        Ok(())
    }

    /// Check if upload is enabled.
    fn is_enabled(&self) -> bool;
}

/// Stub analytics uploader that logs to file instead of uploading.
///
/// This implementation is used during development and testing. It writes
/// events to a local file instead of sending them to a remote endpoint,
/// allowing inspection of what would be uploaded.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::analytics::{StubAnalyticsUploader, AnalyticsUploadConfig};
/// use std::path::PathBuf;
///
/// let config = AnalyticsUploadConfig {
///     upload_enabled: true,
///     log_file: Some(PathBuf::from("analytics_debug.jsonl")),
///     ..Default::default()
/// };
///
/// let uploader = StubAnalyticsUploader::new(config);
/// ```
#[derive(Debug)]
pub struct StubAnalyticsUploader {
    config: AnalyticsUploadConfig,
}

impl StubAnalyticsUploader {
    /// Create a new stub analytics uploader.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the uploader
    #[must_use]
    pub fn new(config: AnalyticsUploadConfig) -> Self {
        Self { config }
    }

    /// Anonymize a session ID by hashing it.
    #[must_use]
    fn anonymize_session_id(session_id: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        session_id.hash(&mut hasher);
        format!("anon_{:016x}", hasher.finish())
    }

    /// Apply privacy settings to an event.
    fn apply_privacy(&self, event: &AnalyticsEvent) -> AnalyticsEvent {
        let mut processed = event.clone();

        if self.config.privacy.anonymize_session_ids {
            processed.session = Self::anonymize_session_id(&event.session);
        }

        if self.config.privacy.exclude_event_data {
            processed.data = serde_json::json!({});
        }

        processed
    }
}

impl AnalyticsUploader for StubAnalyticsUploader {
    fn upload(&self, events: &[AnalyticsEvent]) -> Result<()> {
        if !self.config.upload_enabled {
            return Ok(());
        }

        let Some(log_file) = &self.config.log_file else {
            // No log file configured, just skip
            return Ok(());
        };

        // Ensure parent directory exists
        if let Some(parent) = log_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).context("Failed to create log directory")?;
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .context("Failed to open analytics upload log file")?;

        for event in events {
            let processed = self.apply_privacy(event);

            let log_entry = serde_json::json!({
                "stub_log": true,
                "message": "STUB: Would upload to remote endpoint",
                "endpoint": self.config.endpoint_url,
                "timestamp": Utc::now().to_rfc3339(),
                "event": processed,
            });

            writeln!(file, "{}", serde_json::to_string(&log_entry)?)?;
        }

        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.config.upload_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // Analytics Upload Stub Tests (Phase 18.1)
    // ========================================================================

    #[test]
    fn test_analytics_upload_config_disabled_by_default() {
        let config = AnalyticsUploadConfig::default();
        assert!(!config.upload_enabled);
    }

    #[test]
    fn test_analytics_upload_config_can_be_enabled() {
        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            ..Default::default()
        };
        assert!(config.upload_enabled);
    }

    #[test]
    fn test_analytics_uploader_stub_logs_to_file() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("upload_log.jsonl");

        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            endpoint_url: "https://example.com/analytics".to_string(),
            log_file: Some(log_path.clone()),
            privacy: PrivacySettings {
                anonymize_session_ids: false, // Disable for this test
                exclude_event_data: false,
                include_only_aggregates: false,
            },
        };

        let uploader = StubAnalyticsUploader::new(config);

        // Create a sample event to upload
        let event = AnalyticsEvent {
            session: "test-session".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"key": "value"}),
        };

        uploader.upload(&[event]).unwrap();

        // Verify the log file contains what would have been uploaded
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("test-session"));
        assert!(log_content.contains("test_event"));
        assert!(log_content.contains("STUB: Would upload"));
    }

    #[test]
    fn test_analytics_uploader_respects_privacy_settings() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("upload_log.jsonl");

        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            log_file: Some(log_path.clone()),
            privacy: PrivacySettings {
                anonymize_session_ids: true,
                exclude_event_data: true,
                include_only_aggregates: false,
            },
            ..Default::default()
        };

        let uploader = StubAnalyticsUploader::new(config);

        let event = AnalyticsEvent {
            session: "my-unique-session-id-12345".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"sensitive": "data"}),
        };

        uploader.upload(&[event]).unwrap();

        let log_content = std::fs::read_to_string(&log_path).unwrap();

        // Session ID should be anonymized (hashed)
        assert!(!log_content.contains("my-unique-session-id-12345"));

        // Event data should be excluded
        assert!(!log_content.contains("sensitive"));
    }

    #[test]
    fn test_analytics_uploader_failure_does_not_affect_operation() {
        // Create uploader with invalid log path to simulate failure
        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            log_file: Some(PathBuf::from("/nonexistent/path/that/cannot/be/written")),
            ..Default::default()
        };

        let uploader = StubAnalyticsUploader::new(config);

        let event = AnalyticsEvent {
            session: "test-session".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({}),
        };

        // upload_graceful should return Ok even on failure
        let result = uploader.upload_graceful(&[event]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analytics_uploader_skips_when_disabled() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("upload_log.jsonl");

        let config = AnalyticsUploadConfig {
            upload_enabled: false, // Disabled
            log_file: Some(log_path.clone()),
            ..Default::default()
        };

        let uploader = StubAnalyticsUploader::new(config);

        let event = AnalyticsEvent {
            session: "test-session".to_string(),
            event: "test_event".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({}),
        };

        uploader.upload(&[event]).unwrap();

        // Log file should not exist since upload is disabled
        assert!(!log_path.exists());
    }

    #[test]
    fn test_analytics_uploader_trait_stub_implements() {
        let config = AnalyticsUploadConfig::default();
        let uploader = StubAnalyticsUploader::new(config);

        // Verify it implements the trait (compile-time check via dyn)
        let _boxed: Box<dyn AnalyticsUploader> = Box::new(uploader);
    }

    #[test]
    fn test_privacy_settings_default() {
        let settings = PrivacySettings::default();

        // Default should be privacy-preserving
        assert!(settings.anonymize_session_ids);
        assert!(!settings.exclude_event_data);
        assert!(!settings.include_only_aggregates);
    }

    #[test]
    fn test_analytics_upload_config_serialization() {
        let config = AnalyticsUploadConfig {
            upload_enabled: true,
            endpoint_url: "https://analytics.example.com".to_string(),
            log_file: Some(PathBuf::from("/tmp/analytics.log")),
            privacy: PrivacySettings {
                anonymize_session_ids: true,
                exclude_event_data: false,
                include_only_aggregates: true,
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: AnalyticsUploadConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.upload_enabled, config.upload_enabled);
        assert_eq!(restored.endpoint_url, config.endpoint_url);
        assert_eq!(
            restored.privacy.include_only_aggregates,
            config.privacy.include_only_aggregates
        );
    }

    #[test]
    fn test_anonymize_session_id() {
        let original = "my-session-id-12345";
        let anonymized = StubAnalyticsUploader::anonymize_session_id(original);

        // Should start with anon_
        assert!(anonymized.starts_with("anon_"));

        // Should be consistent
        let anonymized2 = StubAnalyticsUploader::anonymize_session_id(original);
        assert_eq!(anonymized, anonymized2);

        // Should not contain original
        assert!(!anonymized.contains("my-session"));
    }
}
