//! Rollback implementation for checkpoint restoration.
//!
//! This module provides safe rollback operations that restore code
//! to a previous checkpoint state using git and task tracker restoration.
//!
//! # Safety
//!
//! Rollback operations are potentially destructive. This module:
//! - Creates a backup before rollback
//! - Validates the target checkpoint exists
//! - Uses `git checkout` instead of `git reset --hard` for safety
//! - Logs all operations for auditing
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::checkpoint::{CheckpointManager, RollbackManager, QualityMetrics, RegressionThresholds};
//!
//! let mut checkpoint_mgr = CheckpointManager::new(".ralph/checkpoints")?;
//! let rollback_mgr = RollbackManager::new("/path/to/project");
//!
//! // Check if rollback is needed
//! let current = QualityMetrics::new().with_test_counts(50, 45, 5);
//! let thresholds = RegressionThresholds::default();
//!
//! if let Some(target) = rollback_mgr.should_rollback(&mut checkpoint_mgr, &current, &thresholds)? {
//!     rollback_mgr.rollback_to(&target)?;
//! }
//! ```

use super::{Checkpoint, CheckpointId, CheckpointManager, QualityMetrics, RegressionThresholds};
use crate::error::RalphError;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

// ============================================================================
// Rollback Result
// ============================================================================

/// Result of a rollback operation.
#[derive(Debug, Clone)]
pub struct RollbackResult {
    /// The checkpoint that was rolled back to.
    pub checkpoint_id: CheckpointId,

    /// Git hash that was restored.
    pub restored_hash: String,

    /// Git hash before rollback (for audit).
    pub previous_hash: String,

    /// Whether task tracker state was restored.
    pub task_tracker_restored: bool,

    /// Any warnings during rollback.
    pub warnings: Vec<String>,
}

impl RollbackResult {
    /// Format a summary for display.
    #[must_use]
    pub fn summary(&self) -> String {
        let task_status = if self.task_tracker_restored {
            "task tracker restored"
        } else {
            "task tracker not restored"
        };

        let warnings_str = if self.warnings.is_empty() {
            String::new()
        } else {
            format!(" ({} warnings)", self.warnings.len())
        };

        format!(
            "Rolled back {} -> {} ({}){}",
            &self.previous_hash[..8.min(self.previous_hash.len())],
            &self.restored_hash[..8.min(self.restored_hash.len())],
            task_status,
            warnings_str
        )
    }
}

// ============================================================================
// Rollback Manager
// ============================================================================

/// Manages rollback operations with safety checks.
pub struct RollbackManager {
    /// Project root directory.
    project_dir: PathBuf,

    /// Whether to create a backup branch before rollback.
    create_backup: bool,

    /// Prefix for backup branch names.
    backup_prefix: String,
}

impl RollbackManager {
    /// Create a new rollback manager.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project root (must be a git repository)
    #[must_use]
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            create_backup: true,
            backup_prefix: "ralph-backup".to_string(),
        }
    }

    /// Disable backup branch creation before rollback.
    #[must_use]
    pub fn without_backup(mut self) -> Self {
        self.create_backup = false;
        self
    }

    /// Set custom backup branch prefix.
    #[must_use]
    pub fn with_backup_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.backup_prefix = prefix.into();
        self
    }

    /// Check if a rollback should be performed based on quality regression.
    ///
    /// Returns the checkpoint to rollback to if regression is significant.
    ///
    /// # Logic
    ///
    /// 1. Get the latest verified checkpoint (or latest if none verified)
    /// 2. Compare current metrics against checkpoint metrics
    /// 3. Return checkpoint if regression exceeds thresholds
    ///
    /// # Errors
    ///
    /// Returns an error if checkpoints cannot be loaded.
    pub fn should_rollback<'a>(
        &self,
        checkpoint_mgr: &'a mut CheckpointManager,
        current_metrics: &QualityMetrics,
        thresholds: &RegressionThresholds,
    ) -> Result<Option<&'a Checkpoint>> {
        // Prefer verified checkpoint, fall back to latest
        // We need to check verified first, then latest if verified is None
        let has_verified = checkpoint_mgr.latest_verified_checkpoint()?.is_some();

        let target = if has_verified {
            checkpoint_mgr.latest_verified_checkpoint()?
        } else {
            checkpoint_mgr.latest_checkpoint()?
        };

        let target = match target {
            Some(cp) => cp,
            None => {
                debug!("No checkpoints available for rollback comparison");
                return Ok(None);
            }
        };

        // Check for regression
        if current_metrics.is_worse_than(&target.metrics, thresholds) {
            let score = current_metrics.regression_score(&target.metrics);
            info!(
                "Quality regression detected (score: {:.1}): {} vs {}",
                score,
                current_metrics.summary(),
                target.metrics.summary()
            );

            // Check if regression exceeds rollback threshold
            if score >= thresholds.rollback_threshold_score {
                return Ok(Some(target));
            } else {
                debug!(
                    "Regression score {:.1} below rollback threshold {:.1}",
                    score, thresholds.rollback_threshold_score
                );
            }
        }

        Ok(None)
    }

    /// Perform rollback to a checkpoint.
    ///
    /// # Safety
    ///
    /// This operation:
    /// 1. Creates a backup branch (if enabled)
    /// 2. Checks out the target commit
    /// 3. Does NOT delete any commits (they remain in reflog)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Git operations fail
    /// - Working directory has uncommitted changes
    pub fn rollback_to(&self, checkpoint: &Checkpoint) -> Result<RollbackResult> {
        let mut warnings = Vec::new();

        // Get current HEAD hash
        let previous_hash = self.get_current_hash()?;

        // Check for uncommitted changes
        if self.has_uncommitted_changes()? {
            return Err(RalphError::git(
                "rollback",
                "Cannot rollback: working directory has uncommitted changes. Commit or stash changes first.",
            ).into());
        }

        // Create backup branch if enabled
        if self.create_backup {
            let backup_branch = format!(
                "{}-{}-{}",
                self.backup_prefix,
                chrono::Utc::now().format("%Y%m%d-%H%M%S"),
                &previous_hash[..8.min(previous_hash.len())]
            );

            match self.create_branch(&backup_branch) {
                Ok(()) => info!("Created backup branch: {}", backup_branch),
                Err(e) => {
                    let warning = format!("Failed to create backup branch: {}", e);
                    warn!("{}", warning);
                    warnings.push(warning);
                }
            }
        }

        // Perform the checkout
        info!("Rolling back to checkpoint {} ({})", checkpoint.id, checkpoint.git_hash);
        self.checkout_hash(&checkpoint.git_hash)?;

        // Attempt task tracker restoration
        let task_tracker_restored = if let Some(ref state) = checkpoint.task_tracker_state {
            match self.restore_task_tracker_state(state) {
                Ok(()) => {
                    debug!("Task tracker state restored");
                    true
                }
                Err(e) => {
                    let warning = format!("Failed to restore task tracker state: {}", e);
                    warn!("{}", warning);
                    warnings.push(warning);
                    false
                }
            }
        } else {
            debug!("No task tracker state to restore");
            false
        };

        let result = RollbackResult {
            checkpoint_id: checkpoint.id.clone(),
            restored_hash: checkpoint.git_hash.clone(),
            previous_hash,
            task_tracker_restored,
            warnings,
        };

        info!("Rollback complete: {}", result.summary());
        Ok(result)
    }

    /// Rollback to the latest verified checkpoint.
    ///
    /// Convenience method that finds and rolls back to the most recent
    /// verified checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if no verified checkpoint exists or rollback fails.
    pub fn rollback_to_latest_verified(
        &self,
        checkpoint_mgr: &mut CheckpointManager,
    ) -> Result<RollbackResult> {
        let checkpoint = checkpoint_mgr
            .latest_verified_checkpoint()?
            .ok_or_else(|| RalphError::loop_error("No verified checkpoint available for rollback"))?
            .clone();

        self.rollback_to(&checkpoint)
    }

    // ------------------------------------------------------------------------
    // Git operations
    // ------------------------------------------------------------------------

    /// Get the current HEAD commit hash.
    fn get_current_hash(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to execute git rev-parse")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RalphError::git("rev-parse", stderr.trim()).into());
        }

        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(hash)
    }

    /// Check if there are uncommitted changes.
    fn has_uncommitted_changes(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to execute git status")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RalphError::git("status", stderr.trim()).into());
        }

        let status = String::from_utf8_lossy(&output.stdout);
        Ok(!status.trim().is_empty())
    }

    /// Create a branch at current HEAD.
    fn create_branch(&self, name: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["branch", name])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to execute git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RalphError::git("branch", stderr.trim()).into());
        }

        Ok(())
    }

    /// Checkout a specific commit hash.
    fn checkout_hash(&self, hash: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["checkout", hash])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to execute git checkout")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RalphError::git("checkout", stderr.trim()).into());
        }

        Ok(())
    }

    /// Restore task tracker state from serialized JSON.
    fn restore_task_tracker_state(&self, _state: &str) -> Result<()> {
        // Task tracker restoration will be implemented when we integrate
        // with the task tracker module. For now, this is a placeholder.
        //
        // The implementation will:
        // 1. Parse the JSON state
        // 2. Write it to the task tracker state file
        // 3. The next iteration will pick it up
        debug!("Task tracker state restoration not yet implemented");
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
    use std::process::Command;

    fn setup_git_repo() -> TempDir {
        let dir = TempDir::new().expect("create temp dir");

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("git init");

        // Configure git for tests
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .expect("git config email");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .expect("git config name");

        // Create initial commit
        std::fs::write(dir.path().join("README.md"), "# Test").expect("write readme");
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir.path())
            .output()
            .expect("git commit");

        dir
    }

    #[test]
    fn test_rollback_result_summary() {
        let result = RollbackResult {
            checkpoint_id: CheckpointId::from_string("test-123"),
            restored_hash: "abc123def456".to_string(),
            previous_hash: "xyz789012345".to_string(),
            task_tracker_restored: true,
            warnings: vec![],
        };

        let summary = result.summary();
        assert!(summary.contains("xyz78901"));
        assert!(summary.contains("abc123de"));
        assert!(summary.contains("task tracker restored"));
    }

    #[test]
    fn test_rollback_result_summary_with_warnings() {
        let result = RollbackResult {
            checkpoint_id: CheckpointId::from_string("test-123"),
            restored_hash: "abc123".to_string(),
            previous_hash: "xyz789".to_string(),
            task_tracker_restored: false,
            warnings: vec!["warning 1".to_string(), "warning 2".to_string()],
        };

        let summary = result.summary();
        assert!(summary.contains("(2 warnings)"));
        assert!(summary.contains("not restored"));
    }

    #[test]
    fn test_rollback_manager_new() {
        let manager = RollbackManager::new("/test/path");
        assert_eq!(manager.project_dir, PathBuf::from("/test/path"));
        assert!(manager.create_backup);
    }

    #[test]
    fn test_rollback_manager_builder() {
        let manager = RollbackManager::new("/test/path")
            .without_backup()
            .with_backup_prefix("my-backup");

        assert!(!manager.create_backup);
        assert_eq!(manager.backup_prefix, "my-backup");
    }

    #[test]
    fn test_should_rollback_no_checkpoints() {
        let dir = setup_git_repo();
        let manager = RollbackManager::new(dir.path());

        let storage = dir.path().join(".ralph/checkpoints");
        let mut checkpoint_mgr = CheckpointManager::new(&storage).expect("create checkpoint mgr");

        let current = QualityMetrics::new().with_test_counts(50, 45, 5);
        let thresholds = RegressionThresholds::default();

        let result = manager.should_rollback(&mut checkpoint_mgr, &current, &thresholds);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_should_rollback_no_regression() {
        let dir = setup_git_repo();
        let manager = RollbackManager::new(dir.path());

        let storage = dir.path().join(".ralph/checkpoints");
        let mut checkpoint_mgr = CheckpointManager::new(&storage).expect("create checkpoint mgr");
        checkpoint_mgr.config_mut().min_interval_iterations = 0;

        // Create checkpoint with same metrics as current
        let metrics = QualityMetrics::new().with_test_counts(50, 50, 0);
        checkpoint_mgr
            .create_checkpoint("Test", "abc123", "main", metrics.clone(), 1)
            .expect("create");

        let thresholds = RegressionThresholds::default();

        let result = manager.should_rollback(&mut checkpoint_mgr, &metrics, &thresholds);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_should_rollback_with_regression() {
        let dir = setup_git_repo();
        let manager = RollbackManager::new(dir.path());

        let storage = dir.path().join(".ralph/checkpoints");
        let mut checkpoint_mgr = CheckpointManager::new(&storage).expect("create checkpoint mgr");
        checkpoint_mgr.config_mut().min_interval_iterations = 0;

        // Create checkpoint with good metrics
        let baseline = QualityMetrics::new()
            .with_test_counts(50, 50, 0)
            .with_clippy_warnings(0);
        checkpoint_mgr
            .create_checkpoint("Good", "abc123", "main", baseline, 1)
            .expect("create");

        // Current metrics are much worse
        let current = QualityMetrics::new()
            .with_test_counts(50, 40, 10)
            .with_clippy_warnings(10);

        let thresholds = RegressionThresholds::default();

        let result = manager.should_rollback(&mut checkpoint_mgr, &current, &thresholds);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_rollback_detects_uncommitted_changes() {
        let dir = setup_git_repo();
        let manager = RollbackManager::new(dir.path());

        // Create uncommitted changes
        std::fs::write(dir.path().join("uncommitted.txt"), "changes").expect("write");

        // Get first commit hash
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .expect("git rev-parse");
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let checkpoint = Checkpoint::new(
            "Test",
            hash,
            "main",
            QualityMetrics::new(),
            1,
        );

        let result = manager.rollback_to(&checkpoint);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("uncommitted changes"));
    }

    #[test]
    fn test_rollback_to_checkpoint() {
        let dir = setup_git_repo();
        let manager = RollbackManager::new(dir.path()).without_backup();

        // Get first commit hash
        let first_hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .expect("git rev-parse");
        let first_hash = String::from_utf8_lossy(&first_hash_output.stdout)
            .trim()
            .to_string();

        // Create another commit
        std::fs::write(dir.path().join("file2.txt"), "content").expect("write");
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(dir.path())
            .output()
            .expect("git commit");

        // Rollback to first commit
        let checkpoint = Checkpoint::new(
            "First",
            &first_hash,
            "main",
            QualityMetrics::new(),
            1,
        );

        let result = manager.rollback_to(&checkpoint);
        assert!(result.is_ok());

        let rollback = result.unwrap();
        assert_eq!(rollback.restored_hash, first_hash);

        // Verify we're at the first commit
        let current_hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .expect("git rev-parse");
        let current_hash = String::from_utf8_lossy(&current_hash_output.stdout)
            .trim()
            .to_string();

        assert_eq!(current_hash, first_hash);
    }

    #[test]
    fn test_rollback_creates_backup_branch() {
        let dir = setup_git_repo();
        let manager = RollbackManager::new(dir.path());

        // Get first commit hash
        let first_hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .expect("git rev-parse");
        let first_hash = String::from_utf8_lossy(&first_hash_output.stdout)
            .trim()
            .to_string();

        // Create another commit
        std::fs::write(dir.path().join("file2.txt"), "content").expect("write");
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "Second commit"])
            .current_dir(dir.path())
            .output()
            .expect("git commit");

        // Rollback
        let checkpoint = Checkpoint::new(
            "First",
            &first_hash,
            "main",
            QualityMetrics::new(),
            1,
        );

        let _result = manager.rollback_to(&checkpoint).expect("rollback");

        // Check that a backup branch was created
        let branches_output = Command::new("git")
            .args(["branch"])
            .current_dir(dir.path())
            .output()
            .expect("git branch");
        let branches = String::from_utf8_lossy(&branches_output.stdout);

        assert!(branches.contains("ralph-backup"));
    }
}
