//! Persistence and serialization for task tracking.
//!
//! This module provides save/load functionality for the task tracker,
//! including custom serialization for HashMap types.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Task, TaskId, TaskTracker, TaskTrackerConfig};

// ============================================================================
// Custom Serialization
// ============================================================================

/// Custom serialization for HashMap<TaskId, Task> as Vec<Task>.
///
/// This module provides serialization that converts the HashMap to a Vec
/// for cleaner JSON output, and deserializes back into a HashMap.
pub(crate) mod tasks_serde {
    use super::*;

    pub fn serialize<S>(
        tasks: &HashMap<TaskId, Task>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tasks_vec: Vec<&Task> = tasks.values().collect();
        tasks_vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> std::result::Result<HashMap<TaskId, Task>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tasks_vec: Vec<Task> = Vec::deserialize(deserializer)?;
        Ok(tasks_vec.into_iter().map(|t| (t.id.clone(), t)).collect())
    }
}

// ============================================================================
// TaskTracker Persistence Implementation
// ============================================================================

impl TaskTracker {
    /// Save the tracker state to a JSON file.
    ///
    /// Creates parent directories if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    /// use std::path::Path;
    ///
    /// let tracker = TaskTracker::new(TaskTrackerConfig::default());
    /// tracker.save(Path::new(".ralph/task_tracker.json")).unwrap();
    /// ```
    pub fn save(&self, path: &Path) -> Result<()> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
        }

        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize task tracker")?;

        std::fs::write(path, json)
            .with_context(|| format!("Failed to write task tracker to: {}", path.display()))?;

        Ok(())
    }

    /// Load tracker state from a JSON file.
    ///
    /// Returns a new tracker if the file doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    /// * `config` - Configuration to use if file doesn't exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ralph::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    /// use std::path::Path;
    ///
    /// let tracker = TaskTracker::load(
    ///     Path::new(".ralph/task_tracker.json"),
    ///     TaskTrackerConfig::default(),
    /// ).unwrap();
    /// ```
    pub fn load(path: &Path, config: TaskTrackerConfig) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new(config));
        }

        let json = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read task tracker from: {}", path.display()))?;

        let tracker: Self = serde_json::from_str(&json).with_context(|| {
            format!(
                "Failed to deserialize task tracker from: {}",
                path.display()
            )
        })?;

        Ok(tracker)
    }

    /// Load tracker state, or create new if file doesn't exist or is corrupted.
    ///
    /// This is more lenient than `load()` - it will create a fresh tracker
    /// if the file is corrupted instead of returning an error.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    /// * `config` - Configuration to use for new tracker
    pub fn load_or_new(path: &Path, config: TaskTrackerConfig) -> Self {
        match Self::load(path, config.clone()) {
            Ok(tracker) => tracker,
            Err(_) => Self::new(config),
        }
    }

    /// Auto-save the tracker if auto_save is enabled.
    ///
    /// Call this after mutations if auto-save behavior is desired.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to save to
    pub fn auto_save(&self, path: &Path) -> Result<()> {
        if self.config.auto_save {
            self.save(path)?;
        }
        Ok(())
    }

    /// Get the default persistence path for a project.
    ///
    /// Returns `.ralph/task_tracker.json` relative to the project root.
    #[must_use]
    pub fn default_path(project_dir: &Path) -> std::path::PathBuf {
        project_dir.join(".ralph").join("task_tracker.json")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#loop::task_tracker::{BlockReason, TaskState};

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("tracker.json");

        // Create and populate a tracker
        let mut original = TaskTracker::default();
        original.parse_plan("### 1. Task one\n- [ ] Item").unwrap();

        let task_id = TaskId::parse("### 1. Task one").unwrap();
        original.start_task(&task_id).unwrap();
        original.record_progress(5, 100).unwrap();

        // Save it
        original.save(&path).unwrap();
        assert!(path.exists());

        // Load it back
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        // Verify state preserved
        assert_eq!(loaded.tasks.len(), original.tasks.len());
        let loaded_task = loaded.get_task(&task_id).unwrap();
        assert_eq!(loaded_task.state, TaskState::InProgress);
        assert_eq!(loaded_task.metrics.files_modified, 5);
        assert_eq!(loaded_task.metrics.lines_changed, 100);
    }

    #[test]
    fn test_save_creates_directories() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("nested")
            .join("dirs")
            .join("tracker.json");

        let tracker = TaskTracker::default();
        tracker.save(&path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_load_nonexistent_returns_new() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("nonexistent.json");

        let config = TaskTrackerConfig::default().with_stagnation_threshold(10);
        let tracker = TaskTracker::load(&path, config).unwrap();

        assert!(tracker.tasks.is_empty());
        assert_eq!(tracker.config.stagnation_threshold, 10);
    }

    #[test]
    fn test_load_corrupted_returns_error() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("corrupted.json");

        // Write invalid JSON
        std::fs::write(&path, "not valid json {{{").unwrap();

        let result = TaskTracker::load(&path, TaskTrackerConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_or_new_with_corrupted() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("corrupted.json");

        // Write invalid JSON
        std::fs::write(&path, "not valid json {{{").unwrap();

        let config = TaskTrackerConfig::default().with_stagnation_threshold(7);
        let tracker = TaskTracker::load_or_new(&path, config);

        // Should return a new tracker, not error
        assert!(tracker.tasks.is_empty());
        assert_eq!(tracker.config.stagnation_threshold, 7);
    }

    #[test]
    fn test_load_or_new_with_valid() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("valid.json");

        // Create and save a tracker
        let mut original = TaskTracker::default();
        original.parse_plan("### 1. Test").unwrap();
        original.save(&path).unwrap();

        // Load it back with different config
        let config = TaskTrackerConfig::default().with_stagnation_threshold(99);
        let tracker = TaskTracker::load_or_new(&path, config);

        // Should load the existing tracker, not create new
        assert_eq!(tracker.tasks.len(), 1);
    }

    #[test]
    fn test_auto_save_when_enabled() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("auto.json");

        let config = TaskTrackerConfig::default().with_auto_save(true);
        let tracker = TaskTracker::new(config);

        tracker.auto_save(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_auto_save_when_disabled() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("auto.json");

        let config = TaskTrackerConfig::default().with_auto_save(false);
        let tracker = TaskTracker::new(config);

        tracker.auto_save(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_default_path() {
        let project_dir = std::path::Path::new("/home/user/project");
        let path = TaskTracker::default_path(project_dir);

        assert_eq!(
            path.to_string_lossy(),
            "/home/user/project/.ralph/task_tracker.json"
        );
    }

    #[test]
    fn test_save_preserves_all_state() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("full.json");

        // Create a tracker with complex state
        let config = TaskTrackerConfig::default()
            .with_stagnation_threshold(5)
            .with_max_quality_failures(3);
        let mut tracker = TaskTracker::new(config);

        tracker
            .parse_plan(
                r#"
### 1. Phase 1.1: Setup
- [x] Create directories
- [ ] Configure tools

### 2. Phase 1.2: Build
- [ ] Write code

### 3. Phase 2.1: Test
- [ ] Write tests
"#,
            )
            .unwrap();

        // Add state to multiple tasks
        let task1_id = TaskId::parse("### 1. Phase 1.1: Setup").unwrap();
        let task2_id = TaskId::parse("### 2. Phase 1.2: Build").unwrap();
        let task3_id = TaskId::parse("### 3. Phase 2.1: Test").unwrap();

        tracker.start_task(&task1_id).unwrap();
        tracker.record_progress(2, 50).unwrap();
        tracker.submit_for_review(&task1_id).unwrap();
        tracker.complete_task(&task1_id).unwrap();

        tracker.start_task(&task2_id).unwrap();
        tracker
            .block_task(
                &task2_id,
                BlockReason::ExternalDependency {
                    description: "Waiting for API".to_string(),
                },
            )
            .unwrap();

        // Save and reload
        tracker.save(&path).unwrap();
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        // Verify task 1 state
        let t1 = loaded.get_task(&task1_id).unwrap();
        assert_eq!(t1.state, TaskState::Complete);
        assert_eq!(t1.metrics.files_modified, 2);
        assert!(t1.transitions.len() >= 3);

        // Verify task 2 state
        let t2 = loaded.get_task(&task2_id).unwrap();
        assert_eq!(t2.state, TaskState::Blocked);
        assert!(matches!(
            t2.block_reason,
            Some(BlockReason::ExternalDependency { .. })
        ));

        // Verify task 3 state (untouched)
        let t3 = loaded.get_task(&task3_id).unwrap();
        assert_eq!(t3.state, TaskState::NotStarted);

        // Verify config was saved
        assert_eq!(loaded.config.stagnation_threshold, 5);
        assert_eq!(loaded.config.max_quality_failures, 3);
    }

    #[test]
    fn test_save_preserves_timestamps() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("timestamps.json");

        let tracker = TaskTracker::default();
        let original_created = tracker.created_at;

        tracker.save(&path).unwrap();
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        assert_eq!(loaded.created_at, original_created);
    }

    #[test]
    fn test_persistence_roundtrip_with_block_reasons() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("blocks.json");

        let mut tracker = TaskTracker::default();
        tracker
            .parse_plan("### 1. Task\n### 2. Task\n### 3. Task")
            .unwrap();

        // Create different block reasons
        let t1 = TaskId::parse("### 1. Task").unwrap();
        let t2 = TaskId::parse("### 2. Task").unwrap();
        let t3 = TaskId::parse("### 3. Task").unwrap();

        tracker.start_task(&t1).unwrap();
        tracker
            .block_task(
                &t1,
                BlockReason::MaxAttempts {
                    attempts: 5,
                    max: 5,
                },
            )
            .unwrap();

        tracker.start_task(&t2).unwrap();
        tracker
            .block_task(
                &t2,
                BlockReason::QualityGateFailure {
                    gate: "clippy".to_string(),
                    failures: 3,
                },
            )
            .unwrap();

        tracker.start_task(&t3).unwrap();
        tracker
            .block_task(&t3, BlockReason::DependsOnTask { task_number: 1 })
            .unwrap();

        // Save and reload
        tracker.save(&path).unwrap();
        let loaded = TaskTracker::load(&path, TaskTrackerConfig::default()).unwrap();

        // Verify block reasons
        let l1 = loaded.get_task(&t1).unwrap();
        assert!(matches!(
            l1.block_reason,
            Some(BlockReason::MaxAttempts {
                attempts: 5,
                max: 5
            })
        ));

        let l2 = loaded.get_task(&t2).unwrap();
        if let Some(BlockReason::QualityGateFailure { gate, failures }) = &l2.block_reason {
            assert_eq!(gate, "clippy");
            assert_eq!(*failures, 3);
        } else {
            panic!("Expected QualityGateFailure block reason");
        }

        let l3 = loaded.get_task(&t3).unwrap();
        assert!(matches!(
            l3.block_reason,
            Some(BlockReason::DependsOnTask { task_number: 1 })
        ));
    }
}
