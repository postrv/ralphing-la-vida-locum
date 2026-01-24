//! Campaign API for cloud-based campaign orchestration.
//!
//! This module provides a stub for cloud-based campaign orchestration.
//! The cloud features are not yet implemented - this is a placeholder
//! for future cloud integration (Phase 18.2).
//!
//! # Campaign Concepts
//!
//! A **campaign** represents a coordinated automation run across one or more
//! projects. Campaigns track:
//!
//! - Campaign metadata (name, description, created timestamp)
//! - Target projects and their configurations
//! - Execution status and progress
//! - Results and metrics
//!
//! # Local vs Cloud
//!
//! - **Local campaigns**: Run entirely on the local machine, managed by Ralph
//!   directly. This is the current default behavior.
//! - **Cloud campaigns**: (Coming soon) Orchestrated via a cloud API, enabling
//!   distributed execution, team collaboration, and centralized reporting.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::campaign::{CampaignApi, LocalCampaignApi, CampaignConfig};
//!
//! // Local campaigns work without cloud
//! let config = CampaignConfig::default();
//! let api = LocalCampaignApi::new(config);
//! let campaign = api.create_campaign("My Campaign", None)?;
//! ```
//!
//! # Cloud Feature Roadmap
//!
//! Cloud campaign features are planned for future releases:
//!
//! - **Phase 1**: Remote campaign storage and retrieval
//! - **Phase 2**: Distributed execution across multiple machines
//! - **Phase 3**: Team collaboration and shared campaigns
//! - **Phase 4**: Centralized reporting and analytics dashboard

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the Campaign API.
///
/// Controls whether cloud features are enabled and connection settings.
/// Cloud features are disabled by default.
///
/// # Example
///
/// ```
/// use ralph::campaign::CampaignConfig;
///
/// let config = CampaignConfig::default();
/// assert!(!config.cloud_enabled);
/// ```
///
/// # Cloud Feature Status
///
/// Cloud campaign features are marked as "coming soon" and are not yet
/// functional. When cloud is enabled, operations will return appropriate
/// "not available" errors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CampaignConfig {
    /// Whether cloud campaign features are enabled.
    ///
    /// Default: `false` (local-only mode).
    ///
    /// When enabled, the API will attempt to use cloud endpoints.
    /// Currently, all cloud operations return "not available" status.
    #[serde(default)]
    pub cloud_enabled: bool,

    /// The endpoint URL for cloud campaign API.
    ///
    /// This is a placeholder for future cloud integration.
    /// Default: empty string (not configured).
    #[serde(default)]
    pub endpoint_url: String,

    /// Optional campaign ID to use.
    ///
    /// When specified, operations will use this campaign ID instead of
    /// creating a new campaign. Useful for resuming existing campaigns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,

    /// Path to log file for stub operations.
    ///
    /// When using the cloud stub, operations that would be sent to the cloud
    /// are instead logged to this file for debugging/auditing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,
}

// ============================================================================
// Campaign Data Types
// ============================================================================

/// Represents a campaign.
///
/// A campaign is a coordinated automation run with associated metadata,
/// targets, and execution status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Campaign {
    /// Unique identifier for the campaign.
    pub id: String,

    /// Human-readable name for the campaign.
    pub name: String,

    /// Optional description of the campaign's purpose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// When the campaign was created.
    pub created_at: DateTime<Utc>,

    /// When the campaign was last updated.
    pub updated_at: DateTime<Utc>,

    /// Current status of the campaign.
    pub status: CampaignStatus,

    /// Optional metadata key-value pairs.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Status of a campaign.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    /// Campaign has been created but not started.
    #[default]
    Pending,
    /// Campaign is currently running.
    Running,
    /// Campaign completed successfully.
    Completed,
    /// Campaign failed with errors.
    Failed,
    /// Campaign was cancelled by user.
    Cancelled,
}

/// Updates to apply to a campaign.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CampaignUpdate {
    /// New name for the campaign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// New description for the campaign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// New status for the campaign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<CampaignStatus>,

    /// Metadata to merge with existing metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Result of a cloud API operation.
///
/// Used to communicate the status of cloud operations, particularly
/// when cloud features are not available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudOperationResult {
    /// Whether the operation succeeded.
    pub success: bool,

    /// Human-readable message describing the result.
    pub message: String,

    /// Whether cloud features are available.
    pub cloud_available: bool,
}

impl CloudOperationResult {
    /// Create a "not available" result for cloud operations.
    #[must_use]
    pub fn cloud_not_available() -> Self {
        Self {
            success: false,
            message: "Cloud campaign features are coming soon. This is a stub implementation."
                .to_string(),
            cloud_available: false,
        }
    }

    /// Create a success result.
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            cloud_available: true,
        }
    }
}

// ============================================================================
// Campaign API Trait
// ============================================================================

/// Trait for campaign API implementations.
///
/// Provides CRUD operations for campaigns. Implementations can be local
/// (in-memory or file-based) or cloud-based (remote API).
///
/// # Example
///
/// ```rust,ignore
/// use ralph::campaign::{CampaignApi, LocalCampaignApi, CampaignConfig};
///
/// let config = CampaignConfig::default();
/// let api = LocalCampaignApi::new(config);
///
/// // Create a new campaign
/// let campaign = api.create_campaign("My Campaign", None)?;
///
/// // List all campaigns
/// let campaigns = api.list_campaigns()?;
///
/// // Get a specific campaign
/// if let Some(c) = api.get_campaign(&campaign.id)? {
///     println!("Found campaign: {}", c.name);
/// }
/// ```
pub trait CampaignApi: Send + Sync {
    /// Create a new campaign.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the campaign
    /// * `description` - Optional description of the campaign's purpose
    ///
    /// # Errors
    ///
    /// Returns an error if the campaign could not be created.
    fn create_campaign(&self, name: &str, description: Option<&str>) -> Result<Campaign>;

    /// Get a campaign by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The campaign's unique identifier
    ///
    /// # Returns
    ///
    /// Returns `Some(campaign)` if found, `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails (e.g., network error for cloud).
    fn get_campaign(&self, id: &str) -> Result<Option<Campaign>>;

    /// Update an existing campaign.
    ///
    /// # Arguments
    ///
    /// * `id` - The campaign's unique identifier
    /// * `updates` - The updates to apply
    ///
    /// # Errors
    ///
    /// Returns an error if the campaign is not found or update fails.
    fn update_campaign(&self, id: &str, updates: CampaignUpdate) -> Result<Campaign>;

    /// Delete a campaign.
    ///
    /// # Arguments
    ///
    /// * `id` - The campaign's unique identifier
    ///
    /// # Errors
    ///
    /// Returns an error if the campaign is not found or deletion fails.
    fn delete_campaign(&self, id: &str) -> Result<()>;

    /// List all campaigns.
    ///
    /// # Errors
    ///
    /// Returns an error if the listing fails.
    fn list_campaigns(&self) -> Result<Vec<Campaign>>;

    /// Check if cloud features are enabled.
    #[must_use]
    fn is_cloud_enabled(&self) -> bool;

    /// Get the status of cloud features.
    ///
    /// Returns information about whether cloud features are available
    /// and any relevant status messages.
    #[must_use]
    fn cloud_status(&self) -> CloudOperationResult;
}

// ============================================================================
// Local Campaign API
// ============================================================================

/// Local campaign API implementation.
///
/// Provides in-memory campaign management without requiring cloud connectivity.
/// This is the default implementation used when cloud features are disabled.
///
/// # Example
///
/// ```
/// use ralph::campaign::{LocalCampaignApi, CampaignConfig, CampaignApi};
///
/// let config = CampaignConfig::default();
/// let api = LocalCampaignApi::new(config);
///
/// // Local campaigns work without cloud
/// assert!(!api.is_cloud_enabled());
/// ```
#[derive(Debug)]
pub struct LocalCampaignApi {
    config: CampaignConfig,
    campaigns: Arc<RwLock<HashMap<String, Campaign>>>,
}

impl LocalCampaignApi {
    /// Create a new local campaign API.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the API
    #[must_use]
    pub fn new(config: CampaignConfig) -> Self {
        Self {
            config,
            campaigns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a unique campaign ID.
    #[must_use]
    fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        format!("campaign_{:016x}", timestamp)
    }
}

impl CampaignApi for LocalCampaignApi {
    fn create_campaign(&self, name: &str, description: Option<&str>) -> Result<Campaign> {
        let now = Utc::now();
        let campaign = Campaign {
            id: Self::generate_id(),
            name: name.to_string(),
            description: description.map(String::from),
            created_at: now,
            updated_at: now,
            status: CampaignStatus::Pending,
            metadata: HashMap::new(),
        };

        let mut campaigns = self
            .campaigns
            .write()
            .map_err(|e| crate::error::RalphError::Internal(format!("Lock poisoned: {}", e)))?;

        campaigns.insert(campaign.id.clone(), campaign.clone());
        Ok(campaign)
    }

    fn get_campaign(&self, id: &str) -> Result<Option<Campaign>> {
        let campaigns = self
            .campaigns
            .read()
            .map_err(|e| crate::error::RalphError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(campaigns.get(id).cloned())
    }

    fn update_campaign(&self, id: &str, updates: CampaignUpdate) -> Result<Campaign> {
        let mut campaigns = self
            .campaigns
            .write()
            .map_err(|e| crate::error::RalphError::Internal(format!("Lock poisoned: {}", e)))?;

        let campaign = campaigns.get_mut(id).ok_or_else(|| {
            crate::error::RalphError::NotFound(format!("Campaign not found: {}", id))
        })?;

        if let Some(name) = updates.name {
            campaign.name = name;
        }
        if let Some(description) = updates.description {
            campaign.description = Some(description);
        }
        if let Some(status) = updates.status {
            campaign.status = status;
        }
        for (key, value) in updates.metadata {
            campaign.metadata.insert(key, value);
        }
        campaign.updated_at = Utc::now();

        Ok(campaign.clone())
    }

    fn delete_campaign(&self, id: &str) -> Result<()> {
        let mut campaigns = self
            .campaigns
            .write()
            .map_err(|e| crate::error::RalphError::Internal(format!("Lock poisoned: {}", e)))?;

        if campaigns.remove(id).is_none() {
            return Err(crate::error::RalphError::NotFound(format!(
                "Campaign not found: {}",
                id
            )));
        }

        Ok(())
    }

    fn list_campaigns(&self) -> Result<Vec<Campaign>> {
        let campaigns = self
            .campaigns
            .read()
            .map_err(|e| crate::error::RalphError::Internal(format!("Lock poisoned: {}", e)))?;

        Ok(campaigns.values().cloned().collect())
    }

    fn is_cloud_enabled(&self) -> bool {
        self.config.cloud_enabled
    }

    fn cloud_status(&self) -> CloudOperationResult {
        if self.config.cloud_enabled {
            CloudOperationResult::cloud_not_available()
        } else {
            CloudOperationResult {
                success: true,
                message: "Running in local mode. Cloud features are disabled.".to_string(),
                cloud_available: false,
            }
        }
    }
}

// ============================================================================
// Cloud Campaign API Stub
// ============================================================================

/// Cloud campaign API stub.
///
/// This is a stub implementation that returns "not available" for all
/// cloud operations. It is used when cloud features are enabled but
/// the cloud backend is not yet implemented.
///
/// # Coming Soon
///
/// Cloud campaign features are planned for future releases. This stub
/// serves as a placeholder and documents the intended API.
///
/// # Example
///
/// ```
/// use ralph::campaign::{CloudCampaignApi, CampaignConfig, CampaignApi};
///
/// let config = CampaignConfig {
///     cloud_enabled: true,
///     ..Default::default()
/// };
/// let api = CloudCampaignApi::new(config);
///
/// // Cloud features return "not available"
/// let status = api.cloud_status();
/// assert!(!status.cloud_available);
/// assert!(status.message.contains("coming soon"));
/// ```
#[derive(Debug)]
pub struct CloudCampaignApi {
    config: CampaignConfig,
}

impl CloudCampaignApi {
    /// Create a new cloud campaign API stub.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the API
    #[must_use]
    pub fn new(config: CampaignConfig) -> Self {
        Self { config }
    }

    /// Log a stub operation to the configured log file.
    fn log_stub_operation(&self, operation: &str, details: &str) {
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
            "stub_log": true,
            "message": format!("STUB: Cloud operation '{}' called but not available", operation),
            "operation": operation,
            "details": details,
            "endpoint": self.config.endpoint_url,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let _ = writeln!(
            file,
            "{}",
            serde_json::to_string(&log_entry).unwrap_or_default()
        );
    }
}

impl CampaignApi for CloudCampaignApi {
    fn create_campaign(&self, name: &str, description: Option<&str>) -> Result<Campaign> {
        self.log_stub_operation(
            "create_campaign",
            &format!("name={}, description={:?}", name, description),
        );

        Err(crate::error::RalphError::NotSupported(
            "Cloud campaign creation is coming soon. This is a stub implementation.".to_string(),
        ))
    }

    fn get_campaign(&self, id: &str) -> Result<Option<Campaign>> {
        self.log_stub_operation("get_campaign", &format!("id={}", id));

        Err(crate::error::RalphError::NotSupported(
            "Cloud campaign retrieval is coming soon. This is a stub implementation.".to_string(),
        ))
    }

    fn update_campaign(&self, id: &str, _updates: CampaignUpdate) -> Result<Campaign> {
        self.log_stub_operation("update_campaign", &format!("id={}", id));

        Err(crate::error::RalphError::NotSupported(
            "Cloud campaign updates are coming soon. This is a stub implementation.".to_string(),
        ))
    }

    fn delete_campaign(&self, id: &str) -> Result<()> {
        self.log_stub_operation("delete_campaign", &format!("id={}", id));

        Err(crate::error::RalphError::NotSupported(
            "Cloud campaign deletion is coming soon. This is a stub implementation.".to_string(),
        ))
    }

    fn list_campaigns(&self) -> Result<Vec<Campaign>> {
        self.log_stub_operation("list_campaigns", "");

        Err(crate::error::RalphError::NotSupported(
            "Cloud campaign listing is coming soon. This is a stub implementation.".to_string(),
        ))
    }

    fn is_cloud_enabled(&self) -> bool {
        self.config.cloud_enabled
    }

    fn cloud_status(&self) -> CloudOperationResult {
        CloudOperationResult::cloud_not_available()
    }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Create a campaign API based on configuration.
///
/// Returns a `LocalCampaignApi` if cloud is disabled, or a `CloudCampaignApi`
/// stub if cloud is enabled.
///
/// # Example
///
/// ```
/// use ralph::campaign::{create_campaign_api, CampaignConfig, CampaignApi};
///
/// // Default config uses local API
/// let config = CampaignConfig::default();
/// let api = create_campaign_api(config);
/// assert!(!api.is_cloud_enabled());
/// ```
#[must_use]
pub fn create_campaign_api(config: CampaignConfig) -> Box<dyn CampaignApi> {
    if config.cloud_enabled {
        Box::new(CloudCampaignApi::new(config))
    } else {
        Box::new(LocalCampaignApi::new(config))
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
    // Campaign API Trait Tests (Phase 18.2)
    // ========================================================================

    #[test]
    fn test_campaign_api_trait_is_defined() {
        // Verify the trait can be used as a trait object
        let config = CampaignConfig::default();
        let api = LocalCampaignApi::new(config);
        let _boxed: Box<dyn CampaignApi> = Box::new(api);
    }

    #[test]
    fn test_cloud_stub_returns_not_available_for_create() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = CloudCampaignApi::new(config);

        let result = api.create_campaign("Test Campaign", None);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("coming soon") || err_msg.contains("stub"));
    }

    #[test]
    fn test_cloud_stub_returns_not_available_for_get() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = CloudCampaignApi::new(config);

        let result = api.get_campaign("some-id");
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("coming soon") || err_msg.contains("stub"));
    }

    #[test]
    fn test_cloud_stub_returns_not_available_for_update() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = CloudCampaignApi::new(config);

        let result = api.update_campaign("some-id", CampaignUpdate::default());
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("coming soon") || err_msg.contains("stub"));
    }

    #[test]
    fn test_cloud_stub_returns_not_available_for_delete() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = CloudCampaignApi::new(config);

        let result = api.delete_campaign("some-id");
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("coming soon") || err_msg.contains("stub"));
    }

    #[test]
    fn test_cloud_stub_returns_not_available_for_list() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = CloudCampaignApi::new(config);

        let result = api.list_campaigns();
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("coming soon") || err_msg.contains("stub"));
    }

    #[test]
    fn test_campaign_id_can_be_specified_in_config() {
        let config = CampaignConfig {
            campaign_id: Some("my-campaign-123".to_string()),
            ..Default::default()
        };

        assert_eq!(config.campaign_id, Some("my-campaign-123".to_string()));
    }

    #[test]
    fn test_local_campaigns_work_without_cloud() {
        let config = CampaignConfig::default();
        let api = LocalCampaignApi::new(config);

        // Should not require cloud
        assert!(!api.is_cloud_enabled());

        // Should be able to create campaigns locally
        let campaign = api
            .create_campaign("Local Campaign", Some("A local test"))
            .unwrap();
        assert_eq!(campaign.name, "Local Campaign");
        assert_eq!(campaign.description, Some("A local test".to_string()));
        assert_eq!(campaign.status, CampaignStatus::Pending);

        // Should be able to retrieve the campaign
        let retrieved = api.get_campaign(&campaign.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Local Campaign");

        // Should be able to list campaigns
        let all = api.list_campaigns().unwrap();
        assert_eq!(all.len(), 1);

        // Should be able to update
        let updated = api
            .update_campaign(
                &campaign.id,
                CampaignUpdate {
                    name: Some("Updated Name".to_string()),
                    status: Some(CampaignStatus::Running),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.status, CampaignStatus::Running);

        // Should be able to delete
        api.delete_campaign(&campaign.id).unwrap();
        let after_delete = api.get_campaign(&campaign.id).unwrap();
        assert!(after_delete.is_none());
    }

    #[test]
    fn test_cloud_features_marked_as_coming_soon() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = CloudCampaignApi::new(config);

        let status = api.cloud_status();
        assert!(!status.cloud_available);
        assert!(status.message.contains("coming soon"));
    }

    #[test]
    fn test_campaign_config_cloud_disabled_by_default() {
        let config = CampaignConfig::default();
        assert!(!config.cloud_enabled);
    }

    #[test]
    fn test_campaign_config_can_enable_cloud() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        assert!(config.cloud_enabled);
    }

    #[test]
    fn test_campaign_config_serialization() {
        let config = CampaignConfig {
            cloud_enabled: true,
            endpoint_url: "https://api.example.com/campaigns".to_string(),
            campaign_id: Some("campaign-123".to_string()),
            log_file: Some(PathBuf::from("/tmp/campaign_stub.log")),
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: CampaignConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.cloud_enabled, config.cloud_enabled);
        assert_eq!(restored.endpoint_url, config.endpoint_url);
        assert_eq!(restored.campaign_id, config.campaign_id);
        assert_eq!(restored.log_file, config.log_file);
    }

    #[test]
    fn test_cloud_stub_logs_to_file() {
        let temp = TempDir::new().unwrap();
        let log_path = temp.path().join("campaign_stub.jsonl");

        let config = CampaignConfig {
            cloud_enabled: true,
            endpoint_url: "https://api.example.com/campaigns".to_string(),
            log_file: Some(log_path.clone()),
            ..Default::default()
        };

        let api = CloudCampaignApi::new(config);

        // Try to create a campaign (will fail, but should log)
        let _ = api.create_campaign("Test Campaign", None);

        // Verify the log file contains what would have been attempted
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("create_campaign"));
        assert!(log_content.contains("STUB"));
        assert!(log_content.contains("not available"));
    }

    #[test]
    fn test_create_campaign_api_returns_local_when_cloud_disabled() {
        let config = CampaignConfig::default();
        let api = create_campaign_api(config);

        assert!(!api.is_cloud_enabled());

        // Should work as local API
        let campaign = api.create_campaign("Test", None);
        assert!(campaign.is_ok());
    }

    #[test]
    fn test_create_campaign_api_returns_cloud_stub_when_cloud_enabled() {
        let config = CampaignConfig {
            cloud_enabled: true,
            ..Default::default()
        };
        let api = create_campaign_api(config);

        assert!(api.is_cloud_enabled());

        // Should return stub error
        let campaign = api.create_campaign("Test", None);
        assert!(campaign.is_err());
    }

    #[test]
    fn test_local_api_update_nonexistent_fails() {
        let config = CampaignConfig::default();
        let api = LocalCampaignApi::new(config);

        let result = api.update_campaign("nonexistent", CampaignUpdate::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_local_api_delete_nonexistent_fails() {
        let config = CampaignConfig::default();
        let api = LocalCampaignApi::new(config);

        let result = api.delete_campaign("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_campaign_status_default() {
        let status = CampaignStatus::default();
        assert_eq!(status, CampaignStatus::Pending);
    }

    #[test]
    fn test_campaign_update_metadata() {
        let config = CampaignConfig::default();
        let api = LocalCampaignApi::new(config);

        let campaign = api.create_campaign("Test", None).unwrap();

        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());

        let updated = api
            .update_campaign(
                &campaign.id,
                CampaignUpdate {
                    metadata,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(updated.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_cloud_operation_result_constructors() {
        let not_available = CloudOperationResult::cloud_not_available();
        assert!(!not_available.success);
        assert!(!not_available.cloud_available);
        assert!(not_available.message.contains("coming soon"));

        let success = CloudOperationResult::success("Operation completed");
        assert!(success.success);
        assert!(success.cloud_available);
        assert_eq!(success.message, "Operation completed");
    }
}
