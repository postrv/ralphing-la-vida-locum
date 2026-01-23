//! Audit logging for compliance and debugging.
//!
//! This module provides tamper-evident audit logging with hash chaining,
//! log rotation, and verification capabilities.
//!
//! # Features
//!
//! - **Append-only logging**: All audit events are appended to a JSONL file
//! - **Tamper-evident**: Each entry includes a SHA-256 hash of the previous entry
//! - **Log rotation**: Supports rotation by size and date
//! - **Verification**: Can verify the integrity of the entire audit log
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::audit::{AuditLogger, AuditEvent, AuditEventType};
//!
//! let logger = AuditLogger::new(project_dir)?;
//!
//! // Log a command execution
//! logger.log_command("cargo test", 0, "All tests passed")?;
//!
//! // Log a gate result
//! logger.log_gate_result("clippy", true, None)?;
//!
//! // Verify log integrity
//! let result = logger.verify()?;
//! assert!(result.is_valid);
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// The type of audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// A command was executed (e.g., cargo test, cargo clippy).
    CommandExecution,
    /// A quality gate was run with a result.
    GateResult,
    /// A git commit was made.
    Commit,
    /// A session started.
    SessionStart,
    /// A session ended.
    SessionEnd,
    /// A checkpoint was created.
    CheckpointCreated,
    /// A rollback was performed.
    Rollback,
    /// A configuration change occurred.
    ConfigChange,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::CommandExecution => "command_execution",
            Self::GateResult => "gate_result",
            Self::Commit => "commit",
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::CheckpointCreated => "checkpoint_created",
            Self::Rollback => "rollback",
            Self::ConfigChange => "config_change",
        };
        write!(f, "{}", s)
    }
}

/// An audit log entry with hash chaining for tamper evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique sequence number for this entry.
    pub sequence: u64,
    /// Timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Type of audit event.
    pub event_type: AuditEventType,
    /// Session ID associated with this event.
    pub session_id: String,
    /// User or actor that triggered the event.
    pub actor: String,
    /// Event-specific data.
    pub data: serde_json::Value,
    /// SHA-256 hash of the previous entry (hex-encoded).
    /// For the first entry, this is a hash of the genesis string.
    pub previous_hash: String,
    /// SHA-256 hash of this entry (hex-encoded).
    /// Computed from all fields except this one.
    pub hash: String,
}

impl AuditEntry {
    /// Compute the hash of this entry for verification.
    ///
    /// The hash is computed from: sequence, timestamp, event_type, session_id,
    /// actor, data, and previous_hash.
    #[must_use]
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(self.event_type.to_string().as_bytes());
        hasher.update(self.session_id.as_bytes());
        hasher.update(self.actor.as_bytes());
        hasher.update(self.data.to_string().as_bytes());
        hasher.update(self.previous_hash.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify this entry's hash is correct.
    #[must_use]
    pub fn verify_hash(&self) -> bool {
        self.hash == self.compute_hash()
    }
}

/// Configuration for audit log rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationConfig {
    /// Maximum file size in bytes before rotation (default: 10MB).
    pub max_size_bytes: u64,
    /// Maximum age in days before rotation (default: 30 days).
    pub max_age_days: u32,
    /// Maximum number of rotated files to keep (default: 10).
    pub max_files: u32,
}

impl Default for RotationConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 10 * 1024 * 1024, // 10MB
            max_age_days: 30,
            max_files: 10,
        }
    }
}

/// Result of verifying an audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the entire log is valid.
    pub is_valid: bool,
    /// Total number of entries verified.
    pub entries_verified: u64,
    /// Index of the first invalid entry (if any).
    pub first_invalid_entry: Option<u64>,
    /// Description of the verification error (if any).
    pub error_description: Option<String>,
}

impl VerificationResult {
    /// Create a successful verification result.
    #[must_use]
    pub fn valid(entries_verified: u64) -> Self {
        Self {
            is_valid: true,
            entries_verified,
            first_invalid_entry: None,
            error_description: None,
        }
    }

    /// Create a failed verification result.
    #[must_use]
    pub fn invalid(entries_verified: u64, invalid_entry: u64, error: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            entries_verified,
            first_invalid_entry: Some(invalid_entry),
            error_description: Some(error.into()),
        }
    }
}

/// The genesis hash used for the first entry in the audit log.
const GENESIS_HASH: &str = "ralph-audit-genesis-v1";

/// Audit logger with tamper-evident hash chaining.
#[derive(Debug)]
pub struct AuditLogger {
    project_dir: PathBuf,
    rotation_config: RotationConfig,
}

impl AuditLogger {
    /// Create a new audit logger for the given project directory.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The root directory of the project
    ///
    /// # Errors
    ///
    /// Returns an error if the audit directory cannot be created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let logger = AuditLogger::new(PathBuf::from("."))?;
    /// ```
    pub fn new(project_dir: PathBuf) -> Result<Self> {
        let logger = Self {
            project_dir,
            rotation_config: RotationConfig::default(),
        };
        logger.ensure_dir()?;
        Ok(logger)
    }

    /// Create a new audit logger with custom rotation configuration.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The root directory of the project
    /// * `rotation_config` - Custom rotation configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the audit directory cannot be created.
    pub fn with_rotation(project_dir: PathBuf, rotation_config: RotationConfig) -> Result<Self> {
        let logger = Self {
            project_dir,
            rotation_config,
        };
        logger.ensure_dir()?;
        Ok(logger)
    }

    /// Get the path to the audit log file.
    fn audit_file(&self) -> PathBuf {
        self.project_dir.join(".ralph/audit.jsonl")
    }

    /// Get the audit directory path.
    fn audit_dir(&self) -> PathBuf {
        self.project_dir.join(".ralph")
    }

    /// Ensure the audit directory exists.
    fn ensure_dir(&self) -> Result<()> {
        let dir = self.audit_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir).context("Failed to create audit directory")?;
        }
        Ok(())
    }

    /// Get the current sequence number (next entry's sequence).
    fn get_next_sequence(&self) -> Result<u64> {
        let entries = self.read_entries()?;
        Ok(entries.last().map(|e| e.sequence + 1).unwrap_or(0))
    }

    /// Get the hash of the last entry (or genesis hash if empty).
    fn get_previous_hash(&self) -> Result<String> {
        let entries = self.read_entries()?;
        Ok(entries
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(compute_genesis_hash))
    }

    /// Log an audit event.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The type of event
    /// * `session_id` - The session ID
    /// * `actor` - The actor (user or system)
    /// * `data` - Event-specific data
    ///
    /// # Errors
    ///
    /// Returns an error if the entry cannot be written.
    pub fn log_event(
        &self,
        event_type: AuditEventType,
        session_id: &str,
        actor: &str,
        data: serde_json::Value,
    ) -> Result<AuditEntry> {
        self.maybe_rotate()?;

        let sequence = self.get_next_sequence()?;
        let previous_hash = self.get_previous_hash()?;
        let timestamp = Utc::now();

        let mut entry = AuditEntry {
            sequence,
            timestamp,
            event_type,
            session_id: session_id.to_string(),
            actor: actor.to_string(),
            data,
            previous_hash,
            hash: String::new(), // Computed below
        };

        entry.hash = entry.compute_hash();

        self.write_entry(&entry)?;
        Ok(entry)
    }

    /// Log a command execution.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    /// * `command` - The command that was executed
    /// * `exit_code` - The exit code of the command
    /// * `output` - Optional output from the command
    ///
    /// # Errors
    ///
    /// Returns an error if the entry cannot be written.
    pub fn log_command(
        &self,
        session_id: &str,
        command: &str,
        exit_code: i32,
        output: Option<&str>,
    ) -> Result<AuditEntry> {
        self.log_event(
            AuditEventType::CommandExecution,
            session_id,
            "system",
            serde_json::json!({
                "command": command,
                "exit_code": exit_code,
                "output": output
            }),
        )
    }

    /// Log a quality gate result.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    /// * `gate_name` - The name of the gate
    /// * `passed` - Whether the gate passed
    /// * `details` - Optional details about the result
    ///
    /// # Errors
    ///
    /// Returns an error if the entry cannot be written.
    pub fn log_gate_result(
        &self,
        session_id: &str,
        gate_name: &str,
        passed: bool,
        details: Option<&str>,
    ) -> Result<AuditEntry> {
        self.log_event(
            AuditEventType::GateResult,
            session_id,
            "system",
            serde_json::json!({
                "gate_name": gate_name,
                "passed": passed,
                "details": details
            }),
        )
    }

    /// Log a git commit.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    /// * `commit_hash` - The commit hash
    /// * `message` - The commit message
    /// * `author` - The commit author
    ///
    /// # Errors
    ///
    /// Returns an error if the entry cannot be written.
    pub fn log_commit(
        &self,
        session_id: &str,
        commit_hash: &str,
        message: &str,
        author: &str,
    ) -> Result<AuditEntry> {
        self.log_event(
            AuditEventType::Commit,
            session_id,
            author,
            serde_json::json!({
                "commit_hash": commit_hash,
                "message": message
            }),
        )
    }

    /// Write an entry to the audit log.
    fn write_entry(&self, entry: &AuditEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.audit_file())
            .context("Failed to open audit file")?;

        let json = serde_json::to_string(entry).context("Failed to serialize audit entry")?;
        writeln!(file, "{}", json).context("Failed to write audit entry")?;

        Ok(())
    }

    /// Read all entries from the audit log.
    ///
    /// # Errors
    ///
    /// Returns an error if the audit file cannot be read or parsed.
    pub fn read_entries(&self) -> Result<Vec<AuditEntry>> {
        let file_path = self.audit_file();

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&file_path).context("Failed to open audit file")?;
        let reader = BufReader::new(file);

        let mut entries = Vec::new();
        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result.context("Failed to read line from audit file")?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: AuditEntry = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse audit entry at line {}", line_num + 1))?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Verify the integrity of the audit log.
    ///
    /// Checks that:
    /// 1. Each entry's hash is correct
    /// 2. Each entry's previous_hash matches the previous entry's hash
    /// 3. The first entry's previous_hash matches the genesis hash
    /// 4. Sequence numbers are consecutive
    ///
    /// # Errors
    ///
    /// Returns an error if the audit log cannot be read.
    pub fn verify(&self) -> Result<VerificationResult> {
        let entries = self.read_entries()?;

        if entries.is_empty() {
            return Ok(VerificationResult::valid(0));
        }

        let genesis = compute_genesis_hash();

        for (i, entry) in entries.iter().enumerate() {
            // Check sequence number
            if entry.sequence != i as u64 {
                return Ok(VerificationResult::invalid(
                    i as u64,
                    entry.sequence,
                    format!(
                        "Sequence mismatch: expected {}, got {}",
                        i, entry.sequence
                    ),
                ));
            }

            // Verify entry hash
            if !entry.verify_hash() {
                return Ok(VerificationResult::invalid(
                    i as u64,
                    entry.sequence,
                    "Entry hash verification failed",
                ));
            }

            // Verify chain hash
            let expected_previous = if i == 0 {
                &genesis
            } else {
                &entries[i - 1].hash
            };

            if entry.previous_hash != *expected_previous {
                return Ok(VerificationResult::invalid(
                    i as u64,
                    entry.sequence,
                    "Chain hash mismatch: previous_hash doesn't match",
                ));
            }
        }

        Ok(VerificationResult::valid(entries.len() as u64))
    }

    /// Check if rotation is needed and perform it if so.
    fn maybe_rotate(&self) -> Result<()> {
        let file_path = self.audit_file();
        if !file_path.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(&file_path)?;
        let should_rotate = metadata.len() >= self.rotation_config.max_size_bytes;

        if should_rotate {
            self.rotate()?;
        }

        Ok(())
    }

    /// Perform log rotation.
    ///
    /// Renames the current log file with a timestamp and removes old files
    /// if the maximum number of rotated files is exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error if rotation fails.
    pub fn rotate(&self) -> Result<()> {
        let file_path = self.audit_file();
        if !file_path.exists() {
            return Ok(());
        }

        // Generate rotated file name with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let rotated_name = format!("audit_{}.jsonl", timestamp);
        let rotated_path = self.audit_dir().join(rotated_name);

        // Rename current file
        fs::rename(&file_path, &rotated_path).context("Failed to rotate audit file")?;

        // Clean up old rotated files
        self.cleanup_old_files()?;

        Ok(())
    }

    /// Remove old rotated files exceeding the maximum count.
    fn cleanup_old_files(&self) -> Result<()> {
        let audit_dir = self.audit_dir();
        let mut rotated_files: Vec<PathBuf> = fs::read_dir(&audit_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();

        // Sort by name (timestamp) descending
        rotated_files.sort_by(|a, b| b.cmp(a));

        // Remove files exceeding max_files
        for path in rotated_files
            .iter()
            .skip(self.rotation_config.max_files as usize)
        {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    /// Clear all audit logs (for testing only).
    ///
    /// # Errors
    ///
    /// Returns an error if files cannot be removed.
    pub fn clear(&self) -> Result<()> {
        let file_path = self.audit_file();
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }

        // Also remove rotated files
        let audit_dir = self.audit_dir();
        for entry in fs::read_dir(&audit_dir)? {
            let path = entry?.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
                .unwrap_or(false)
            {
                fs::remove_file(&path)?;
            }
        }

        Ok(())
    }

    /// Get entries filtered by event type.
    ///
    /// # Errors
    ///
    /// Returns an error if the audit log cannot be read.
    pub fn get_entries_by_type(&self, event_type: AuditEventType) -> Result<Vec<AuditEntry>> {
        let entries = self.read_entries()?;
        Ok(entries
            .into_iter()
            .filter(|e| e.event_type == event_type)
            .collect())
    }

    /// Get entries for a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the audit log cannot be read.
    pub fn get_entries_by_session(&self, session_id: &str) -> Result<Vec<AuditEntry>> {
        let entries = self.read_entries()?;
        Ok(entries
            .into_iter()
            .filter(|e| e.session_id == session_id)
            .collect())
    }

    /// Get the count of entries in the audit log.
    ///
    /// # Errors
    ///
    /// Returns an error if the audit log cannot be read.
    pub fn entry_count(&self) -> Result<u64> {
        let entries = self.read_entries()?;
        Ok(entries.len() as u64)
    }
}

/// Compute the genesis hash used for the first entry.
#[must_use]
fn compute_genesis_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(GENESIS_HASH.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // Test: Audit log records all command executions
    // ========================================================================

    #[test]
    fn test_audit_log_records_command_execution() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        // Log a command execution
        let entry = logger
            .log_command("session-1", "cargo test", 0, Some("All tests passed"))
            .unwrap();

        assert_eq!(entry.event_type, AuditEventType::CommandExecution);
        assert_eq!(entry.session_id, "session-1");

        let data = &entry.data;
        assert_eq!(data["command"], "cargo test");
        assert_eq!(data["exit_code"], 0);
        assert_eq!(data["output"], "All tests passed");
    }

    #[test]
    fn test_audit_log_records_multiple_commands() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cargo build", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo clippy", 1, Some("warnings found"))
            .unwrap();

        let entries = logger
            .get_entries_by_type(AuditEventType::CommandExecution)
            .unwrap();
        assert_eq!(entries.len(), 3);
    }

    // ========================================================================
    // Test: Audit log records all gate results
    // ========================================================================

    #[test]
    fn test_audit_log_records_gate_result_passed() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_gate_result("session-1", "clippy", true, None)
            .unwrap();

        assert_eq!(entry.event_type, AuditEventType::GateResult);
        assert_eq!(entry.data["gate_name"], "clippy");
        assert_eq!(entry.data["passed"], true);
    }

    #[test]
    fn test_audit_log_records_gate_result_failed() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_gate_result("session-1", "tests", false, Some("3 tests failed"))
            .unwrap();

        assert_eq!(entry.event_type, AuditEventType::GateResult);
        assert_eq!(entry.data["gate_name"], "tests");
        assert_eq!(entry.data["passed"], false);
        assert_eq!(entry.data["details"], "3 tests failed");
    }

    #[test]
    fn test_audit_log_records_multiple_gate_results() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_gate_result("session-1", "clippy", true, None)
            .unwrap();
        logger
            .log_gate_result("session-1", "tests", true, None)
            .unwrap();
        logger
            .log_gate_result("session-1", "security", false, Some("vulnerability found"))
            .unwrap();

        let entries = logger
            .get_entries_by_type(AuditEventType::GateResult)
            .unwrap();
        assert_eq!(entries.len(), 3);
    }

    // ========================================================================
    // Test: Audit log records all commits
    // ========================================================================

    #[test]
    fn test_audit_log_records_commit() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_commit(
                "session-1",
                "abc123def456",
                "feat: add audit logging",
                "developer@example.com",
            )
            .unwrap();

        assert_eq!(entry.event_type, AuditEventType::Commit);
        assert_eq!(entry.actor, "developer@example.com");
        assert_eq!(entry.data["commit_hash"], "abc123def456");
        assert_eq!(entry.data["message"], "feat: add audit logging");
    }

    #[test]
    fn test_audit_log_records_multiple_commits() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_commit("session-1", "abc123", "feat: first commit", "dev@example.com")
            .unwrap();
        logger
            .log_commit("session-1", "def456", "fix: bug fix", "dev@example.com")
            .unwrap();
        logger
            .log_commit(
                "session-2",
                "ghi789",
                "docs: update readme",
                "other@example.com",
            )
            .unwrap();

        let entries = logger.get_entries_by_type(AuditEventType::Commit).unwrap();
        assert_eq!(entries.len(), 3);
    }

    // ========================================================================
    // Test: Audit log is tamper-evident (hashed entries)
    // ========================================================================

    #[test]
    fn test_audit_entry_has_hash() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();

        // Hash should be non-empty
        assert!(!entry.hash.is_empty());
        // Hash should be 64 hex characters (SHA-256)
        assert_eq!(entry.hash.len(), 64);
        // Hash should be valid hex
        assert!(hex::decode(&entry.hash).is_ok());
    }

    #[test]
    fn test_audit_entry_hash_is_verifiable() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();

        // The entry should verify its own hash
        assert!(entry.verify_hash());
    }

    #[test]
    fn test_audit_entries_form_hash_chain() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cargo build", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo clippy", 0, None)
            .unwrap();

        let entries = logger.read_entries().unwrap();
        assert_eq!(entries.len(), 3);

        // First entry should reference genesis hash
        let genesis = compute_genesis_hash();
        assert_eq!(entries[0].previous_hash, genesis);

        // Each subsequent entry should reference the previous entry's hash
        assert_eq!(entries[1].previous_hash, entries[0].hash);
        assert_eq!(entries[2].previous_hash, entries[1].hash);
    }

    #[test]
    fn test_audit_log_verification_passes_for_valid_log() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cargo build", 0, None)
            .unwrap();
        logger
            .log_gate_result("session-1", "clippy", true, None)
            .unwrap();
        logger
            .log_commit("session-1", "abc123", "test commit", "dev@example.com")
            .unwrap();

        let result = logger.verify().unwrap();
        assert!(result.is_valid);
        assert_eq!(result.entries_verified, 3);
        assert!(result.first_invalid_entry.is_none());
        assert!(result.error_description.is_none());
    }

    #[test]
    fn test_audit_log_verification_fails_for_tampered_hash() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cargo build", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();

        // Manually tamper with the file
        let file_path = temp_dir.path().join(".ralph/audit.jsonl");
        let content = fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Parse and modify the second entry's hash
        let mut entry: AuditEntry = serde_json::from_str(lines[1]).unwrap();
        entry.hash = "tampered_hash_000000000000000000000000000000000000000".to_string();
        let tampered_line = serde_json::to_string(&entry).unwrap();

        let new_content = format!("{}\n{}\n", lines[0], tampered_line);
        fs::write(&file_path, new_content).unwrap();

        let result = logger.verify().unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.first_invalid_entry, Some(1));
        assert!(result.error_description.is_some());
    }

    #[test]
    fn test_audit_log_verification_fails_for_broken_chain() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cargo build", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();

        // Manually tamper with the chain
        let file_path = temp_dir.path().join(".ralph/audit.jsonl");
        let content = fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Parse and modify the second entry's previous_hash
        let mut entry: AuditEntry = serde_json::from_str(lines[1]).unwrap();
        entry.previous_hash = "wrong_previous_hash_00000000000000000000000000000000".to_string();
        // Recompute hash with wrong previous_hash
        entry.hash = entry.compute_hash();
        let tampered_line = serde_json::to_string(&entry).unwrap();

        let new_content = format!("{}\n{}\n", lines[0], tampered_line);
        fs::write(&file_path, new_content).unwrap();

        let result = logger.verify().unwrap();
        assert!(!result.is_valid);
        assert!(result.error_description.unwrap().contains("Chain hash"));
    }

    #[test]
    fn test_audit_log_verification_empty_log_is_valid() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let result = logger.verify().unwrap();
        assert!(result.is_valid);
        assert_eq!(result.entries_verified, 0);
    }

    // ========================================================================
    // Test: Audit log rotation works
    // ========================================================================

    #[test]
    fn test_audit_log_rotation_by_size() {
        let temp_dir = TempDir::new().unwrap();
        let config = RotationConfig {
            max_size_bytes: 500, // Very small for testing
            max_age_days: 30,
            max_files: 3,
        };
        let logger = AuditLogger::with_rotation(temp_dir.path().to_path_buf(), config).unwrap();

        // Write enough entries to exceed the size limit
        for i in 0..20 {
            logger
                .log_command(
                    "session-1",
                    &format!("command_{}", i),
                    0,
                    Some("some output that takes up space"),
                )
                .unwrap();
        }

        // Check that rotated files exist
        let audit_dir = temp_dir.path().join(".ralph");
        let rotated_files: Vec<_> = fs::read_dir(&audit_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();

        // Should have some rotated files
        assert!(!rotated_files.is_empty());
    }

    #[test]
    fn test_audit_log_rotation_cleanup_old_files() {
        let temp_dir = TempDir::new().unwrap();
        let config = RotationConfig {
            max_size_bytes: 100, // Very small
            max_age_days: 30,
            max_files: 2, // Only keep 2 rotated files
        };
        let logger = AuditLogger::with_rotation(temp_dir.path().to_path_buf(), config).unwrap();

        // Write many entries to trigger multiple rotations
        for i in 0..100 {
            logger
                .log_command(
                    "session-1",
                    &format!("command_{}", i),
                    0,
                    Some("padding text to fill up the log file quickly"),
                )
                .unwrap();
        }

        // Check rotated file count
        let audit_dir = temp_dir.path().join(".ralph");
        let rotated_count = fs::read_dir(&audit_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .count();

        // Should not exceed max_files
        assert!(rotated_count <= 2);
    }

    #[test]
    fn test_audit_log_manual_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        // Write some entries
        logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cargo build", 0, None)
            .unwrap();

        // Manually rotate
        logger.rotate().unwrap();

        // Main audit file should not exist or be empty
        let main_file = temp_dir.path().join(".ralph/audit.jsonl");
        let main_exists = main_file.exists();

        // A rotated file should exist
        let audit_dir = temp_dir.path().join(".ralph");
        let rotated_exists = fs::read_dir(&audit_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            });

        assert!(!main_exists || fs::read_to_string(&main_file).unwrap().is_empty());
        assert!(rotated_exists);
    }

    // ========================================================================
    // Additional tests for coverage
    // ========================================================================

    #[test]
    fn test_audit_event_type_display() {
        assert_eq!(AuditEventType::CommandExecution.to_string(), "command_execution");
        assert_eq!(AuditEventType::GateResult.to_string(), "gate_result");
        assert_eq!(AuditEventType::Commit.to_string(), "commit");
        assert_eq!(AuditEventType::SessionStart.to_string(), "session_start");
        assert_eq!(AuditEventType::SessionEnd.to_string(), "session_end");
        assert_eq!(AuditEventType::CheckpointCreated.to_string(), "checkpoint_created");
        assert_eq!(AuditEventType::Rollback.to_string(), "rollback");
        assert_eq!(AuditEventType::ConfigChange.to_string(), "config_change");
    }

    #[test]
    fn test_audit_entry_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_command("session-1", "cargo test", 0, None)
            .unwrap();

        // Serialize and deserialize
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.sequence, deserialized.sequence);
        assert_eq!(entry.event_type, deserialized.event_type);
        assert_eq!(entry.session_id, deserialized.session_id);
        assert_eq!(entry.hash, deserialized.hash);
    }

    #[test]
    fn test_get_entries_by_session() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cmd1", 0, None)
            .unwrap();
        logger
            .log_command("session-2", "cmd2", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cmd3", 0, None)
            .unwrap();

        let session1_entries = logger.get_entries_by_session("session-1").unwrap();
        assert_eq!(session1_entries.len(), 2);

        let session2_entries = logger.get_entries_by_session("session-2").unwrap();
        assert_eq!(session2_entries.len(), 1);
    }

    #[test]
    fn test_entry_count() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(logger.entry_count().unwrap(), 0);

        logger
            .log_command("session-1", "cmd1", 0, None)
            .unwrap();
        assert_eq!(logger.entry_count().unwrap(), 1);

        logger
            .log_command("session-1", "cmd2", 0, None)
            .unwrap();
        assert_eq!(logger.entry_count().unwrap(), 2);
    }

    #[test]
    fn test_clear_audit_log() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        logger
            .log_command("session-1", "cmd1", 0, None)
            .unwrap();
        logger
            .log_command("session-1", "cmd2", 0, None)
            .unwrap();

        assert_eq!(logger.entry_count().unwrap(), 2);

        logger.clear().unwrap();

        assert_eq!(logger.entry_count().unwrap(), 0);
    }

    #[test]
    fn test_verification_result_constructors() {
        let valid = VerificationResult::valid(10);
        assert!(valid.is_valid);
        assert_eq!(valid.entries_verified, 10);

        let invalid = VerificationResult::invalid(5, 5, "test error");
        assert!(!invalid.is_valid);
        assert_eq!(invalid.entries_verified, 5);
        assert_eq!(invalid.first_invalid_entry, Some(5));
        assert_eq!(invalid.error_description, Some("test error".to_string()));
    }

    #[test]
    fn test_rotation_config_default() {
        let config = RotationConfig::default();
        assert_eq!(config.max_size_bytes, 10 * 1024 * 1024);
        assert_eq!(config.max_age_days, 30);
        assert_eq!(config.max_files, 10);
    }

    #[test]
    fn test_generic_log_event() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = logger
            .log_event(
                AuditEventType::ConfigChange,
                "session-1",
                "admin",
                serde_json::json!({
                    "setting": "max_iterations",
                    "old_value": 50,
                    "new_value": 100
                }),
            )
            .unwrap();

        assert_eq!(entry.event_type, AuditEventType::ConfigChange);
        assert_eq!(entry.actor, "admin");
        assert_eq!(entry.data["setting"], "max_iterations");
    }

    #[test]
    fn test_sequence_numbers_are_consecutive() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();

        for i in 0..5 {
            let entry = logger
                .log_command("session-1", &format!("cmd{}", i), 0, None)
                .unwrap();
            assert_eq!(entry.sequence, i as u64);
        }
    }
}
