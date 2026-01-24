//! Session persistence layer for atomic file-based storage.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use tracing::warn;

use super::SessionState;
use ralph::error::{RalphError, Result};

/// Default session file name.
const SESSION_FILE: &str = "session.json";

/// Temporary file suffix for atomic writes.
const TMP_SUFFIX: &str = ".tmp";

/// Lock file suffix for concurrent access prevention.
const LOCK_SUFFIX: &str = ".lock";

/// Session persistence manager providing atomic file operations.
#[derive(Debug, Clone)]
pub struct SessionPersistence {
    /// Directory where session files are stored.
    dir: PathBuf,
}

impl SessionPersistence {
    /// Creates a new session persistence manager.
    #[must_use]
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    /// Returns the path to the session file.
    #[must_use]
    pub fn session_file_path(&self) -> PathBuf {
        self.dir.join(SESSION_FILE)
    }

    /// Returns the path to the temporary session file.
    #[must_use]
    pub fn tmp_file_path(&self) -> PathBuf {
        self.dir.join(format!("{SESSION_FILE}{TMP_SUFFIX}"))
    }

    /// Returns the path to the lock file.
    #[must_use]
    pub fn lock_file_path(&self) -> PathBuf {
        self.dir.join(format!("{SESSION_FILE}{LOCK_SUFFIX}"))
    }

    /// Saves session state atomically.
    pub fn save(&self, state: &SessionState) -> Result<()> {
        fs::create_dir_all(&self.dir)?;

        let lock_file = File::create(self.lock_file_path())?;
        FileExt::lock_exclusive(&lock_file).map_err(|e| {
            RalphError::Internal(format!("Failed to acquire session lock: {e}"))
        })?;

        let tmp_path = self.tmp_file_path();
        let json = serde_json::to_string_pretty(state)?;

        let mut tmp_file = File::create(&tmp_path)?;
        tmp_file.write_all(json.as_bytes())?;
        tmp_file.sync_all()?;

        fs::rename(&tmp_path, self.session_file_path())?;

        Ok(())
    }

    /// Loads session state from file.
    pub fn load(&self) -> Result<Option<SessionState>> {
        let session_path = self.session_file_path();

        if !session_path.exists() {
            return Ok(None);
        }

        let lock_path = self.lock_file_path();
        if lock_path.exists() {
            let lock_file = File::open(&lock_path)?;
            FileExt::lock_shared(&lock_file).map_err(|e| {
                RalphError::Internal(format!("Failed to acquire session lock: {e}"))
            })?;
        }

        let mut file = match File::open(&session_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let state: SessionState = match serde_json::from_str(&contents) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "Corrupted session file at {}: {}. Deleting and starting fresh.",
                    session_path.display(),
                    e
                );
                let _ = fs::remove_file(&session_path);
                return Ok(None);
            }
        };

        if !state.is_version_compatible() {
            warn!(
                "Incompatible session version {} (supported: {}). Starting fresh.",
                state.version(),
                super::SESSION_STATE_VERSION
            );
            let _ = fs::remove_file(&session_path);
            return Ok(None);
        }

        Ok(Some(state))
    }

    /// Deletes the session file if it exists.
    pub fn delete(&self) -> Result<()> {
        let session_path = self.session_file_path();
        if session_path.exists() {
            fs::remove_file(&session_path)?;
        }
        Ok(())
    }

    /// Checks if a session file exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.session_file_path().exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#loop::state::{LoopMode, LoopState};
    use crate::r#loop::task_tracker::{TaskTracker, TaskTrackerConfig};
    use tempfile::TempDir;

    fn test_persistence() -> (SessionPersistence, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let persistence = SessionPersistence::new(temp_dir.path().join(".ralph"));
        (persistence, temp_dir)
    }

    #[test]
    fn test_persistence_save_creates_file() {
        let (persistence, _temp_dir) = test_persistence();
        let state = SessionState::new();

        assert!(!persistence.exists());
        persistence.save(&state).expect("save should succeed");
        assert!(persistence.exists());
        assert!(persistence.session_file_path().exists());
    }

    #[test]
    fn test_persistence_load_returns_none_when_missing() {
        let (persistence, _temp_dir) = test_persistence();
        let result = persistence.load().expect("load should not error");
        assert!(result.is_none());
    }

    #[test]
    fn test_persistence_save_and_load_roundtrip() {
        let (persistence, _temp_dir) = test_persistence();

        let mut state = SessionState::new();
        state.set_loop_state(LoopState::new(LoopMode::Build));
        state.set_task_tracker(TaskTracker::new(TaskTrackerConfig::default()));
        state.supervisor.mode_switch_count = 5;
        state.predictor.prediction_history.push((75.0, true));

        persistence.save(&state).expect("save should succeed");

        let loaded = persistence.load().expect("load should succeed");
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert!(loaded.loop_state.is_some());
        assert_eq!(loaded.loop_state.as_ref().unwrap().mode, LoopMode::Build);
        assert!(loaded.task_tracker.is_some());
        assert_eq!(loaded.supervisor.mode_switch_count, 5);
        assert_eq!(loaded.predictor.prediction_history.len(), 1);
    }

    #[test]
    fn test_persistence_atomic_write_no_tmp_file_after_save() {
        let (persistence, _temp_dir) = test_persistence();
        let state = SessionState::new();

        persistence.save(&state).expect("save should succeed");
        assert!(!persistence.tmp_file_path().exists());
        assert!(persistence.session_file_path().exists());
    }

    #[test]
    fn test_persistence_corrupted_file_returns_none_and_logs() {
        let (persistence, _temp_dir) = test_persistence();

        fs::create_dir_all(&persistence.dir).expect("create dir");
        fs::write(persistence.session_file_path(), "not valid json {{{")
            .expect("write corrupted file");

        let result = persistence.load().expect("load should not error");
        assert!(result.is_none());
        assert!(!persistence.session_file_path().exists());
    }

    #[test]
    fn test_persistence_incompatible_version_returns_none() {
        let (persistence, _temp_dir) = test_persistence();

        let incompatible_json = r#"{
            "metadata": {
                "version": 999,
                "created_at": "2024-01-01T00:00:00Z",
                "saved_at": "2024-01-01T00:00:00Z",
                "pid": 12345,
                "session_id": "test"
            },
            "loop_state": null,
            "task_tracker": null,
            "supervisor": {
                "mode_switch_count": 0,
                "last_error": null,
                "error_repeat_count": 0,
                "last_check_iteration": 0,
                "health_history": [],
                "last_verdict": null
            },
            "predictor": {
                "prediction_history": [],
                "config": null
            }
        }"#;

        fs::create_dir_all(&persistence.dir).expect("create dir");
        fs::write(persistence.session_file_path(), incompatible_json)
            .expect("write incompatible file");

        let result = persistence.load().expect("load should not error");
        assert!(result.is_none());
        assert!(!persistence.session_file_path().exists());
    }

    #[test]
    fn test_persistence_creates_directory_if_missing() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let nested_path = temp_dir.path().join("deep").join("nested").join(".ralph");
        let persistence = SessionPersistence::new(&nested_path);

        assert!(!nested_path.exists());

        let state = SessionState::new();
        persistence.save(&state).expect("save should succeed");

        assert!(nested_path.exists());
        assert!(persistence.session_file_path().exists());
    }

    #[test]
    fn test_persistence_delete_removes_file() {
        let (persistence, _temp_dir) = test_persistence();
        let state = SessionState::new();

        persistence.save(&state).expect("save should succeed");
        assert!(persistence.exists());

        persistence.delete().expect("delete should succeed");
        assert!(!persistence.exists());
    }

    #[test]
    fn test_persistence_delete_succeeds_when_missing() {
        let (persistence, _temp_dir) = test_persistence();
        assert!(!persistence.exists());
        persistence.delete().expect("delete should succeed");
    }

    #[test]
    fn test_persistence_file_locking_prevents_concurrent_access() {
        let (persistence, _temp_dir) = test_persistence();
        let state = SessionState::new();

        persistence.save(&state).expect("first save should succeed");

        let lock_path = persistence.lock_file_path();
        let lock_file = File::open(&lock_path).expect("open lock file");
        FileExt::lock_exclusive(&lock_file).expect("acquire lock");

        let persistence2 = SessionPersistence::new(&persistence.dir);

        FileExt::unlock(&lock_file).expect("release lock");
        persistence2.save(&state).expect("save after unlock should succeed");
    }

    #[test]
    fn test_persistence_overwrites_existing_file() {
        let (persistence, _temp_dir) = test_persistence();

        let mut state1 = SessionState::new();
        state1.supervisor.mode_switch_count = 1;
        persistence.save(&state1).expect("first save");

        let loaded1 = persistence.load().expect("load").unwrap();
        assert_eq!(loaded1.supervisor.mode_switch_count, 1);

        let mut state2 = SessionState::new();
        state2.supervisor.mode_switch_count = 99;
        persistence.save(&state2).expect("second save");

        let loaded2 = persistence.load().expect("load").unwrap();
        assert_eq!(loaded2.supervisor.mode_switch_count, 99);
    }

    #[test]
    fn test_persistence_preserves_all_session_fields() {
        let (persistence, _temp_dir) = test_persistence();

        let mut state = SessionState::new();
        state.set_loop_state(LoopState::new(LoopMode::Debug));
        state.set_task_tracker(TaskTracker::new(TaskTrackerConfig::default()));
        state.supervisor.mode_switch_count = 3;
        state.supervisor.last_error = Some("test error".to_string());
        state.supervisor.error_repeat_count = 2;
        state.predictor.prediction_history.push((50.0, true));
        state.predictor.prediction_history.push((80.0, false));

        persistence.save(&state).expect("save");
        let loaded = persistence.load().expect("load").unwrap();

        assert!(loaded.loop_state.is_some());
        assert_eq!(loaded.loop_state.as_ref().unwrap().mode, LoopMode::Debug);
        assert!(loaded.task_tracker.is_some());
        assert_eq!(loaded.supervisor.mode_switch_count, 3);
        assert_eq!(loaded.supervisor.last_error, Some("test error".to_string()));
        assert_eq!(loaded.supervisor.error_repeat_count, 2);
        assert_eq!(loaded.predictor.prediction_history.len(), 2);
    }

    #[test]
    fn test_session_file_path() {
        let persistence = SessionPersistence::new("/test/path/.ralph");
        assert_eq!(
            persistence.session_file_path(),
            PathBuf::from("/test/path/.ralph/session.json")
        );
    }

    #[test]
    fn test_persistence_handles_empty_state() {
        let (persistence, _temp_dir) = test_persistence();
        let state = SessionState::new();

        assert!(state.is_empty());

        persistence.save(&state).expect("save");

        let loaded = persistence.load().expect("load").unwrap();
        assert!(loaded.is_empty());
    }
}
