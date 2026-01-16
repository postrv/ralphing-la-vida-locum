//! Checkpoint management and storage.
//!
//! The [`CheckpointManager`] handles creating, listing, retrieving, and
//! pruning checkpoints with persistence to disk.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::checkpoint::{CheckpointManager, QualityMetrics};
//!
//! let mut manager = CheckpointManager::new(".ralph/checkpoints")?;
//!
//! // Create a checkpoint
//! let metrics = QualityMetrics::new().with_test_counts(50, 50, 0);
//! let checkpoint = manager.create_checkpoint("All tests passing", "abc123", "main", metrics, 5)?;
//!
//! // List checkpoints
//! for cp in manager.list_checkpoints() {
//!     println!("{}", cp.summary());
//! }
//!
//! // Prune old checkpoints, keeping most recent 10
//! manager.prune(10)?;
//! ```

use super::{Checkpoint, CheckpointId, QualityMetrics};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

// ============================================================================
// Checkpoint Manager Configuration
// ============================================================================

/// Configuration for checkpoint manager behavior.
#[derive(Debug, Clone)]
pub struct CheckpointManagerConfig {
    /// Maximum number of checkpoints to retain.
    pub max_checkpoints: usize,

    /// Whether to auto-prune when creating new checkpoints.
    pub auto_prune: bool,

    /// Minimum interval between checkpoints (in iterations).
    pub min_interval_iterations: u32,

    /// Whether to keep all verified checkpoints regardless of pruning.
    pub preserve_verified: bool,
}

impl Default for CheckpointManagerConfig {
    fn default() -> Self {
        Self {
            max_checkpoints: 20,
            auto_prune: true,
            min_interval_iterations: 3,
            preserve_verified: true,
        }
    }
}

impl CheckpointManagerConfig {
    /// Create a new configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of checkpoints.
    #[must_use]
    pub fn with_max_checkpoints(mut self, max: usize) -> Self {
        self.max_checkpoints = max;
        self
    }

    /// Enable/disable auto-pruning.
    #[must_use]
    pub fn with_auto_prune(mut self, enabled: bool) -> Self {
        self.auto_prune = enabled;
        self
    }

    /// Set minimum iteration interval between checkpoints.
    #[must_use]
    pub fn with_min_interval(mut self, iterations: u32) -> Self {
        self.min_interval_iterations = iterations;
        self
    }
}

// ============================================================================
// Checkpoint Manager
// ============================================================================

/// Manages checkpoint storage and retrieval.
///
/// Checkpoints are stored as individual JSON files in a directory,
/// with an index file for fast listing.
pub struct CheckpointManager {
    /// Directory where checkpoints are stored.
    storage_dir: PathBuf,

    /// Configuration for manager behavior.
    config: CheckpointManagerConfig,

    /// In-memory cache of checkpoints (loaded on demand).
    checkpoints: Vec<Checkpoint>,

    /// Whether the cache has been loaded.
    cache_loaded: bool,
}

impl CheckpointManager {
    /// Create a new checkpoint manager with storage at the given path.
    ///
    /// Creates the storage directory if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage directory cannot be created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::checkpoint::CheckpointManager;
    ///
    /// let manager = CheckpointManager::new(".ralph/checkpoints")?;
    /// ```
    pub fn new(storage_dir: impl AsRef<Path>) -> Result<Self> {
        let storage_dir = storage_dir.as_ref().to_path_buf();

        // Create storage directory if it doesn't exist
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)
                .with_context(|| format!("Failed to create checkpoint directory: {}", storage_dir.display()))?;
            debug!("Created checkpoint directory: {}", storage_dir.display());
        }

        Ok(Self {
            storage_dir,
            config: CheckpointManagerConfig::default(),
            checkpoints: Vec::new(),
            cache_loaded: false,
        })
    }

    /// Create a checkpoint manager with custom configuration.
    pub fn with_config(storage_dir: impl AsRef<Path>, config: CheckpointManagerConfig) -> Result<Self> {
        let mut manager = Self::new(storage_dir)?;
        manager.config = config;
        Ok(manager)
    }

    /// Get the storage directory path.
    #[must_use]
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &CheckpointManagerConfig {
        &self.config
    }

    /// Get mutable reference to configuration.
    #[must_use]
    pub fn config_mut(&mut self) -> &mut CheckpointManagerConfig {
        &mut self.config
    }

    /// Create a new checkpoint and persist it.
    ///
    /// # Arguments
    ///
    /// * `description` - Human-readable description
    /// * `git_hash` - Current git commit hash
    /// * `git_branch` - Current git branch
    /// * `metrics` - Quality metrics at this point
    /// * `iteration` - Current iteration number
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be saved.
    pub fn create_checkpoint(
        &mut self,
        description: impl Into<String>,
        git_hash: impl Into<String>,
        git_branch: impl Into<String>,
        metrics: QualityMetrics,
        iteration: u32,
    ) -> Result<Checkpoint> {
        self.ensure_cache_loaded()?;

        // Check minimum interval
        if let Some(last) = self.checkpoints.last() {
            if iteration.saturating_sub(last.iteration) < self.config.min_interval_iterations {
                debug!(
                    "Skipping checkpoint: only {} iterations since last (min: {})",
                    iteration - last.iteration,
                    self.config.min_interval_iterations
                );
                // Return the last checkpoint instead
                return Ok(last.clone());
            }
        }

        let checkpoint = Checkpoint::new(description, git_hash, git_branch, metrics, iteration);

        // Save to disk
        self.save_checkpoint(&checkpoint)?;

        // Add to cache
        self.checkpoints.push(checkpoint.clone());

        // Auto-prune if enabled
        if self.config.auto_prune && self.checkpoints.len() > self.config.max_checkpoints {
            self.prune(self.config.max_checkpoints)?;
        }

        info!("Created checkpoint: {}", checkpoint.summary());
        Ok(checkpoint)
    }

    /// Get a checkpoint by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache cannot be loaded.
    pub fn get_checkpoint(&mut self, id: &CheckpointId) -> Result<Option<&Checkpoint>> {
        self.ensure_cache_loaded()?;
        Ok(self.checkpoints.iter().find(|cp| &cp.id == id))
    }

    /// List all checkpoints, sorted by creation time (oldest first).
    ///
    /// # Errors
    ///
    /// Returns an error if the cache cannot be loaded.
    pub fn list_checkpoints(&mut self) -> Result<&[Checkpoint]> {
        self.ensure_cache_loaded()?;
        Ok(&self.checkpoints)
    }

    /// Get the most recent checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache cannot be loaded.
    pub fn latest_checkpoint(&mut self) -> Result<Option<&Checkpoint>> {
        self.ensure_cache_loaded()?;
        Ok(self.checkpoints.last())
    }

    /// Get the most recent verified checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache cannot be loaded.
    pub fn latest_verified_checkpoint(&mut self) -> Result<Option<&Checkpoint>> {
        self.ensure_cache_loaded()?;
        Ok(self.checkpoints.iter().rev().find(|cp| cp.verified))
    }

    /// Find checkpoints with a specific tag.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache cannot be loaded.
    pub fn checkpoints_with_tag(&mut self, tag: &str) -> Result<Vec<&Checkpoint>> {
        self.ensure_cache_loaded()?;
        Ok(self.checkpoints.iter().filter(|cp| cp.has_tag(tag)).collect())
    }

    /// Prune checkpoints to keep only the most recent N.
    ///
    /// If `preserve_verified` is enabled in config, verified checkpoints
    /// are never pruned.
    ///
    /// # Errors
    ///
    /// Returns an error if checkpoints cannot be deleted.
    pub fn prune(&mut self, keep: usize) -> Result<usize> {
        self.ensure_cache_loaded()?;

        if self.checkpoints.len() <= keep {
            return Ok(0);
        }

        let mut to_remove = Vec::new();
        let mut kept = 0;

        // Iterate from newest to oldest
        for (idx, checkpoint) in self.checkpoints.iter().enumerate().rev() {
            if kept >= keep {
                // Check if we should preserve this verified checkpoint
                if self.config.preserve_verified && checkpoint.verified {
                    debug!("Preserving verified checkpoint: {}", checkpoint.id);
                    continue;
                }
                to_remove.push(idx);
            } else {
                kept += 1;
            }
        }

        // Remove from disk and cache (in reverse order to maintain indices)
        to_remove.sort_unstable();
        for &idx in to_remove.iter().rev() {
            let checkpoint = &self.checkpoints[idx];
            let path = self.checkpoint_path(&checkpoint.id);
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed to delete checkpoint: {}", path.display()))?;
            }
            self.checkpoints.remove(idx);
        }

        let removed = to_remove.len();
        if removed > 0 {
            info!("Pruned {} checkpoints, {} remaining", removed, self.checkpoints.len());
        }

        Ok(removed)
    }

    /// Delete a specific checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be deleted.
    pub fn delete_checkpoint(&mut self, id: &CheckpointId) -> Result<bool> {
        self.ensure_cache_loaded()?;

        let idx = self.checkpoints.iter().position(|cp| &cp.id == id);

        if let Some(idx) = idx {
            let path = self.checkpoint_path(id);
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed to delete checkpoint: {}", path.display()))?;
            }
            self.checkpoints.remove(idx);
            info!("Deleted checkpoint: {}", id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Mark a checkpoint as verified.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be found or saved.
    pub fn verify_checkpoint(&mut self, id: &CheckpointId) -> Result<bool> {
        self.ensure_cache_loaded()?;

        // Find the index of the checkpoint to avoid borrow conflicts
        let idx = self.checkpoints.iter().position(|cp| &cp.id == id);

        if let Some(idx) = idx {
            self.checkpoints[idx].verified = true;
            // Clone the checkpoint for saving to avoid borrow conflict
            let checkpoint_to_save = self.checkpoints[idx].clone();
            self.save_checkpoint(&checkpoint_to_save)?;
            info!("Verified checkpoint: {}", id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get total number of checkpoints.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache cannot be loaded.
    pub fn count(&mut self) -> Result<usize> {
        self.ensure_cache_loaded()?;
        Ok(self.checkpoints.len())
    }

    // ------------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------------

    /// Generate the file path for a checkpoint.
    fn checkpoint_path(&self, id: &CheckpointId) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }

    /// Save a checkpoint to disk.
    fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let path = self.checkpoint_path(&checkpoint.id);
        let json = serde_json::to_string_pretty(checkpoint)
            .context("Failed to serialize checkpoint")?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write checkpoint: {}", path.display()))?;
        debug!("Saved checkpoint to: {}", path.display());
        Ok(())
    }

    /// Load all checkpoints from disk into cache.
    fn load_checkpoints(&mut self) -> Result<()> {
        self.checkpoints.clear();

        let entries = fs::read_dir(&self.storage_dir)
            .with_context(|| format!("Failed to read checkpoint directory: {}", self.storage_dir.display()))?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                match self.load_checkpoint_file(&path) {
                    Ok(checkpoint) => {
                        self.checkpoints.push(checkpoint);
                    }
                    Err(e) => {
                        warn!("Failed to load checkpoint {}: {}", path.display(), e);
                        // Continue loading other checkpoints
                    }
                }
            }
        }

        // Sort by creation time (oldest first)
        self.checkpoints.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        debug!("Loaded {} checkpoints from disk", self.checkpoints.len());
        self.cache_loaded = true;
        Ok(())
    }

    /// Load a single checkpoint from a file.
    fn load_checkpoint_file(&self, path: &Path) -> Result<Checkpoint> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read checkpoint file: {}", path.display()))?;
        let checkpoint: Checkpoint = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse checkpoint file: {}", path.display()))?;
        Ok(checkpoint)
    }

    /// Ensure the cache is loaded.
    fn ensure_cache_loaded(&mut self) -> Result<()> {
        if !self.cache_loaded {
            self.load_checkpoints()?;
        }
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_manager() -> (TempDir, CheckpointManager) {
        let dir = TempDir::new().expect("create temp dir");
        let manager = CheckpointManager::new(dir.path().join("checkpoints"))
            .expect("create manager");
        (dir, manager)
    }

    #[test]
    fn test_checkpoint_manager_new_creates_directory() {
        let dir = TempDir::new().expect("create temp dir");
        let storage = dir.path().join("checkpoints");

        assert!(!storage.exists());
        let _manager = CheckpointManager::new(&storage).expect("create manager");
        assert!(storage.exists());
    }

    #[test]
    fn test_checkpoint_manager_create_and_list() {
        let (_dir, mut manager) = temp_manager();

        // Adjust config to allow creation at every iteration
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new().with_test_counts(10, 10, 0);
        let cp = manager
            .create_checkpoint("First", "abc123", "main", metrics.clone(), 1)
            .expect("create checkpoint");

        assert_eq!(cp.description, "First");

        let list = manager.list_checkpoints().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, cp.id);
    }

    #[test]
    fn test_checkpoint_manager_get_by_id() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new();
        let cp = manager
            .create_checkpoint("Test", "abc", "main", metrics, 1)
            .expect("create");

        let found = manager.get_checkpoint(&cp.id).expect("get");
        assert!(found.is_some());
        assert_eq!(found.unwrap().description, "Test");

        let not_found = manager.get_checkpoint(&CheckpointId::from_string("nonexistent")).expect("get");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_checkpoint_manager_latest() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new();

        manager
            .create_checkpoint("First", "abc", "main", metrics.clone(), 1)
            .expect("create");
        manager
            .create_checkpoint("Second", "def", "main", metrics.clone(), 2)
            .expect("create");
        manager
            .create_checkpoint("Third", "ghi", "main", metrics, 3)
            .expect("create");

        let latest = manager.latest_checkpoint().expect("latest");
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().description, "Third");
    }

    #[test]
    fn test_checkpoint_manager_prune() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;
        manager.config.auto_prune = false;

        let metrics = QualityMetrics::new();

        for i in 1..=5 {
            manager
                .create_checkpoint(format!("CP {}", i), format!("hash{}", i), "main", metrics.clone(), i)
                .expect("create");
        }

        assert_eq!(manager.count().expect("count"), 5);

        let removed = manager.prune(3).expect("prune");
        assert_eq!(removed, 2);
        assert_eq!(manager.count().expect("count"), 3);

        // Verify oldest were removed
        let list = manager.list_checkpoints().expect("list");
        assert_eq!(list[0].description, "CP 3");
        assert_eq!(list[2].description, "CP 5");
    }

    #[test]
    fn test_checkpoint_manager_prune_preserves_verified() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;
        manager.config.auto_prune = false;
        manager.config.preserve_verified = true;

        let metrics = QualityMetrics::new();

        let cp1 = manager
            .create_checkpoint("CP 1", "hash1", "main", metrics.clone(), 1)
            .expect("create");
        manager
            .create_checkpoint("CP 2", "hash2", "main", metrics.clone(), 2)
            .expect("create");
        manager
            .create_checkpoint("CP 3", "hash3", "main", metrics, 3)
            .expect("create");

        // Verify the first checkpoint
        manager.verify_checkpoint(&cp1.id).expect("verify");

        // Prune to keep only 1
        manager.prune(1).expect("prune");

        // Should have 2: the verified one and the most recent
        assert_eq!(manager.count().expect("count"), 2);

        let list = manager.list_checkpoints().expect("list");
        assert!(list.iter().any(|cp| cp.id == cp1.id && cp.verified));
    }

    #[test]
    fn test_checkpoint_manager_delete() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new();
        let cp = manager
            .create_checkpoint("Test", "abc", "main", metrics, 1)
            .expect("create");

        assert_eq!(manager.count().expect("count"), 1);

        let deleted = manager.delete_checkpoint(&cp.id).expect("delete");
        assert!(deleted);

        assert_eq!(manager.count().expect("count"), 0);

        // Delete non-existent returns false
        let deleted_again = manager.delete_checkpoint(&cp.id).expect("delete");
        assert!(!deleted_again);
    }

    #[test]
    fn test_checkpoint_manager_verify() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new();
        let cp = manager
            .create_checkpoint("Test", "abc", "main", metrics, 1)
            .expect("create");

        assert!(!manager.get_checkpoint(&cp.id).expect("get").unwrap().verified);

        manager.verify_checkpoint(&cp.id).expect("verify");

        assert!(manager.get_checkpoint(&cp.id).expect("get").unwrap().verified);
    }

    #[test]
    fn test_checkpoint_manager_latest_verified() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new();

        let cp1 = manager
            .create_checkpoint("First", "abc", "main", metrics.clone(), 1)
            .expect("create");
        manager
            .create_checkpoint("Second", "def", "main", metrics.clone(), 2)
            .expect("create");
        manager
            .create_checkpoint("Third", "ghi", "main", metrics, 3)
            .expect("create");

        // No verified checkpoints yet
        assert!(manager.latest_verified_checkpoint().expect("get").is_none());

        // Verify the first one
        manager.verify_checkpoint(&cp1.id).expect("verify");

        let latest_verified = manager.latest_verified_checkpoint().expect("get");
        assert!(latest_verified.is_some());
        assert_eq!(latest_verified.unwrap().id, cp1.id);
    }

    #[test]
    fn test_checkpoint_manager_min_interval() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 5;

        let metrics = QualityMetrics::new();

        let cp1 = manager
            .create_checkpoint("First", "abc", "main", metrics.clone(), 1)
            .expect("create");

        // Try to create at iteration 3 (should return cp1 due to interval)
        let cp2 = manager
            .create_checkpoint("Second", "def", "main", metrics.clone(), 3)
            .expect("create");

        assert_eq!(cp2.id, cp1.id); // Should be the same

        // Create at iteration 10 (should succeed)
        let cp3 = manager
            .create_checkpoint("Third", "ghi", "main", metrics, 10)
            .expect("create");

        assert_ne!(cp3.id, cp1.id);
    }

    #[test]
    fn test_checkpoint_manager_persistence() {
        let dir = TempDir::new().expect("create temp dir");
        let storage = dir.path().join("checkpoints");

        let cp_id;
        {
            let mut manager = CheckpointManager::new(&storage).expect("create manager");
            manager.config.min_interval_iterations = 0;

            let metrics = QualityMetrics::new().with_test_counts(42, 42, 0);
            let cp = manager
                .create_checkpoint("Persisted", "xyz789", "main", metrics, 5)
                .expect("create");
            cp_id = cp.id.clone();
        }

        // Create new manager and verify checkpoint was persisted
        {
            let mut manager = CheckpointManager::new(&storage).expect("create manager");
            let loaded = manager.get_checkpoint(&cp_id).expect("get");

            assert!(loaded.is_some());
            let checkpoint = loaded.unwrap();
            assert_eq!(checkpoint.description, "Persisted");
            assert_eq!(checkpoint.git_hash, "xyz789");
            assert_eq!(checkpoint.metrics.test_total, 42);
        }
    }

    #[test]
    fn test_checkpoint_manager_tags() {
        let (_dir, mut manager) = temp_manager();
        manager.config.min_interval_iterations = 0;

        let metrics = QualityMetrics::new();

        // Need to manually add tags since create_checkpoint doesn't support them directly
        let mut cp1 = Checkpoint::new("Release", "abc", "main", metrics.clone(), 1);
        cp1 = cp1.with_tag("release").with_tag("v1.0");
        manager.checkpoints.push(cp1.clone());
        manager.save_checkpoint(&cp1).expect("save");
        manager.cache_loaded = true;

        let _cp2 = manager
            .create_checkpoint("Dev", "def", "main", metrics, 2)
            .expect("create");

        let releases = manager.checkpoints_with_tag("release").expect("filter");
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].description, "Release");

        let v1 = manager.checkpoints_with_tag("v1.0").expect("filter");
        assert_eq!(v1.len(), 1);

        let none = manager.checkpoints_with_tag("nonexistent").expect("filter");
        assert!(none.is_empty());
    }

    #[test]
    fn test_checkpoint_manager_config_builder() {
        let config = CheckpointManagerConfig::new()
            .with_max_checkpoints(50)
            .with_auto_prune(false)
            .with_min_interval(10);

        assert_eq!(config.max_checkpoints, 50);
        assert!(!config.auto_prune);
        assert_eq!(config.min_interval_iterations, 10);
    }
}
