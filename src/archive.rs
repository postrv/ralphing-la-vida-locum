//! Archive manager for handling stale documentation and code artifacts.
//!
//! This module manages the archival of stale documentation, keeping
//! historical reference available without polluting the active context.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// Statistics about the archive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub total_docs: usize,
    pub total_decisions: usize,
    pub total_code: usize,
    pub total_size_bytes: u64,
    pub oldest_archive: Option<DateTime<Utc>>,
    pub newest_archive: Option<DateTime<Utc>>,
}

/// Result of an archive operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub docs_archived: usize,
    pub decisions_archived: usize,
    pub code_archived: usize,
    pub files_processed: Vec<ArchivedFile>,
}

/// Information about an archived file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedFile {
    pub original_path: String,
    pub archive_path: String,
    pub reason: String,
    pub age_days: u32,
}

/// Manager for documentation archives
pub struct ArchiveManager {
    project_dir: PathBuf,
    stale_threshold_days: u32,
}

impl ArchiveManager {
    /// Create a new archive manager
    pub fn new(project_dir: PathBuf, stale_threshold_days: u32) -> Self {
        Self {
            project_dir,
            stale_threshold_days,
        }
    }

    /// Get archive directory
    fn archive_dir(&self) -> PathBuf {
        self.project_dir.join(".archive")
    }

    /// Get docs directory
    fn docs_dir(&self) -> PathBuf {
        self.project_dir.join("docs")
    }

    /// Get archive statistics
    pub fn get_stats(&self) -> Result<ArchiveStats> {
        let archive_dir = self.archive_dir();

        let mut stats = ArchiveStats {
            total_docs: 0,
            total_decisions: 0,
            total_code: 0,
            total_size_bytes: 0,
            oldest_archive: None,
            newest_archive: None,
        };

        if !archive_dir.exists() {
            return Ok(stats);
        }

        for entry in WalkDir::new(&archive_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let rel_path = path.strip_prefix(&archive_dir).unwrap_or(path);

            // Count by category
            if rel_path.starts_with("docs") {
                stats.total_docs += 1;
            } else if rel_path.starts_with("decisions") {
                stats.total_decisions += 1;
            } else if rel_path.starts_with("code") {
                stats.total_code += 1;
            }

            // Track size
            if let Ok(metadata) = fs::metadata(path) {
                stats.total_size_bytes += metadata.len();

                // Track dates
                if let Ok(modified) = metadata.modified() {
                    let datetime: DateTime<Utc> = modified.into();

                    match stats.oldest_archive {
                        None => stats.oldest_archive = Some(datetime),
                        Some(oldest) if datetime < oldest => stats.oldest_archive = Some(datetime),
                        _ => {}
                    }

                    match stats.newest_archive {
                        None => stats.newest_archive = Some(datetime),
                        Some(newest) if datetime > newest => stats.newest_archive = Some(datetime),
                        _ => {}
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Run the archive manager
    pub fn run(&self, dry_run: bool) -> Result<ArchiveResult> {
        let mut result = ArchiveResult {
            docs_archived: 0,
            decisions_archived: 0,
            code_archived: 0,
            files_processed: Vec::new(),
        };

        // Ensure archive directories exist
        if !dry_run {
            fs::create_dir_all(self.archive_dir().join("docs"))?;
            fs::create_dir_all(self.archive_dir().join("decisions"))?;
            fs::create_dir_all(self.archive_dir().join("code"))?;
        }

        // Archive stale documentation
        self.archive_stale_docs(&mut result, dry_run)?;

        // Archive deprecated decision records
        self.archive_deprecated_decisions(&mut result, dry_run)?;

        Ok(result)
    }

    /// Archive stale documentation files
    fn archive_stale_docs(&self, result: &mut ArchiveResult, dry_run: bool) -> Result<()> {
        let docs_dir = self.docs_dir();

        if !docs_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(&docs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map(|e| e == "md").unwrap_or(false))
        {
            let path = entry.path();
            let age_days = self.file_age_days(path);

            if age_days > self.stale_threshold_days {
                let rel_path = path.strip_prefix(&docs_dir).unwrap_or(path);
                let archive_path = self.archive_dir().join("docs").join(rel_path);

                if !dry_run {
                    self.archive_file(path, &archive_path, "stale")?;
                }

                result.docs_archived += 1;
                result.files_processed.push(ArchivedFile {
                    original_path: path.display().to_string(),
                    archive_path: archive_path.display().to_string(),
                    reason: format!("Stale (no updates in {} days)", age_days),
                    age_days,
                });
            }
        }

        Ok(())
    }

    /// Archive deprecated decision records
    fn archive_deprecated_decisions(&self, result: &mut ArchiveResult, dry_run: bool) -> Result<()> {
        let decisions_dir = self.docs_dir().join("decisions");

        if !decisions_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(&decisions_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map(|e| e == "md").unwrap_or(false))
        {
            let path = entry.path();

            // Check if the ADR is deprecated or superseded
            if let Ok(content) = fs::read_to_string(path) {
                let is_deprecated = content.contains("Status: Deprecated")
                    || content.contains("Status: Superseded")
                    || content.contains("Status.*Deprecated")
                    || content.contains("Status.*Superseded");

                if is_deprecated {
                    let filename = path.file_name().unwrap_or_default();
                    let archive_path = self.archive_dir().join("decisions").join(filename);

                    if !dry_run {
                        self.archive_file(path, &archive_path, "deprecated")?;
                    }

                    result.decisions_archived += 1;
                    result.files_processed.push(ArchivedFile {
                        original_path: path.display().to_string(),
                        archive_path: archive_path.display().to_string(),
                        reason: "Deprecated or superseded ADR".to_string(),
                        age_days: self.file_age_days(path),
                    });
                }
            }
        }

        Ok(())
    }

    /// Archive a file with metadata header
    fn archive_file(&self, source: &Path, dest: &Path, reason: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read original content
        let content = fs::read_to_string(source)
            .with_context(|| format!("Failed to read source file: {}", source.display()))?;

        // Create archived content with metadata header
        let archived_content = format!(
            "<!-- ARCHIVED: {} -->\n<!-- REASON: {} -->\n<!-- ORIGINAL: {} -->\n\n{}",
            Utc::now().to_rfc3339(),
            reason,
            source.display(),
            content
        );

        // Write to archive
        fs::write(dest, archived_content)
            .with_context(|| format!("Failed to write archive file: {}", dest.display()))?;

        // Remove original
        fs::remove_file(source)
            .with_context(|| format!("Failed to remove original file: {}", source.display()))?;

        Ok(())
    }

    /// Get file age in days
    fn file_age_days(&self, path: &Path) -> u32 {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|mtime| {
                let now = SystemTime::now();
                let duration = now.duration_since(mtime).unwrap_or_default();
                (duration.as_secs() / 86400) as u32
            })
            .unwrap_or(0)
    }

    /// Find all stale files (without archiving)
    pub fn find_stale_files(&self) -> Result<Vec<ArchivedFile>> {
        let mut stale_files = Vec::new();
        let docs_dir = self.docs_dir();

        if !docs_dir.exists() {
            return Ok(stale_files);
        }

        for entry in WalkDir::new(&docs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map(|e| e == "md").unwrap_or(false))
        {
            let path = entry.path();
            let age_days = self.file_age_days(path);

            if age_days > self.stale_threshold_days {
                stale_files.push(ArchivedFile {
                    original_path: path.display().to_string(),
                    archive_path: String::new(),
                    reason: format!("Stale (no updates in {} days)", age_days),
                    age_days,
                });
            }
        }

        Ok(stale_files)
    }

    /// Restore a file from archive
    pub fn restore(&self, archive_path: &Path) -> Result<PathBuf> {
        let content = fs::read_to_string(archive_path)
            .with_context(|| format!("Failed to read archive: {}", archive_path.display()))?;

        // Extract original path from header
        let original_path = content
            .lines()
            .find(|line| line.starts_with("<!-- ORIGINAL:"))
            .and_then(|line| {
                line.strip_prefix("<!-- ORIGINAL:")
                    .and_then(|s| s.strip_suffix("-->"))
                    .map(|s| s.trim().to_string())
            })
            .ok_or_else(|| anyhow::anyhow!("Could not find original path in archive metadata"))?;

        // Remove metadata headers
        let restored_content: String = content
            .lines()
            .skip_while(|line| line.starts_with("<!--") || line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        let dest = PathBuf::from(&original_path);

        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write restored content
        fs::write(&dest, restored_content)?;

        // Remove archive file
        fs::remove_file(archive_path)?;

        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_archive_stats_empty() {
        let temp = TempDir::new().unwrap();
        let manager = ArchiveManager::new(temp.path().to_path_buf(), 90);
        let stats = manager.get_stats().unwrap();

        assert_eq!(stats.total_docs, 0);
        assert_eq!(stats.total_decisions, 0);
    }

    #[test]
    fn test_archive_stats_with_files() {
        let temp = TempDir::new().unwrap();

        // Create archive structure with files
        std::fs::create_dir_all(temp.path().join(".archive/docs")).unwrap();
        std::fs::create_dir_all(temp.path().join(".archive/decisions")).unwrap();
        std::fs::write(temp.path().join(".archive/docs/old.md"), "archived doc").unwrap();
        std::fs::write(temp.path().join(".archive/decisions/adr-001.md"), "archived adr").unwrap();

        let manager = ArchiveManager::new(temp.path().to_path_buf(), 90);
        let stats = manager.get_stats().unwrap();

        assert_eq!(stats.total_docs, 1);
        assert_eq!(stats.total_decisions, 1);
    }

    #[test]
    fn test_find_stale_files_none() {
        let temp = TempDir::new().unwrap();

        // Create fresh docs
        std::fs::create_dir_all(temp.path().join("docs")).unwrap();
        std::fs::write(temp.path().join("docs/fresh.md"), "fresh content").unwrap();

        let manager = ArchiveManager::new(temp.path().to_path_buf(), 90);
        let stale = manager.find_stale_files().unwrap();

        assert!(stale.is_empty());
    }

    #[test]
    fn test_archive_run_dry_run_fresh_files() {
        let temp = TempDir::new().unwrap();

        // Create docs directory with fresh file
        std::fs::create_dir_all(temp.path().join("docs")).unwrap();
        std::fs::write(temp.path().join("docs/test.md"), "test content").unwrap();

        // Fresh files (age 0) won't be archived even with threshold 0 because 0 > 0 is false
        // This is correct behavior - use threshold that's achievable
        let manager = ArchiveManager::new(temp.path().to_path_buf(), 0);
        let result = manager.run(true).unwrap();

        // Fresh file should not be archived (age 0, threshold 0, but 0 > 0 is false)
        assert!(temp.path().join("docs/test.md").exists());
        assert_eq!(result.docs_archived, 0);
    }

    #[test]
    fn test_archive_creates_directories() {
        let temp = TempDir::new().unwrap();

        let manager = ArchiveManager::new(temp.path().to_path_buf(), 90);
        manager.run(false).unwrap();

        // Archive directories should be created even if no files are archived
        assert!(temp.path().join(".archive/docs").exists());
        assert!(temp.path().join(".archive/decisions").exists());
        assert!(temp.path().join(".archive/code").exists());
    }

    #[test]
    fn test_archive_deprecated_adr() {
        let temp = TempDir::new().unwrap();

        // Create decisions directory with deprecated ADR
        // Note: The format must be "Status: Deprecated" on the same line
        std::fs::create_dir_all(temp.path().join("docs/decisions")).unwrap();
        std::fs::write(
            temp.path().join("docs/decisions/adr-001.md"),
            "# ADR-001\n\n## Status: Deprecated\n\n## Context\nOld decision"
        ).unwrap();

        let manager = ArchiveManager::new(temp.path().to_path_buf(), 365); // High threshold
        let result = manager.run(false).unwrap();

        // Deprecated ADRs are archived regardless of age
        assert_eq!(result.decisions_archived, 1);
        assert!(!temp.path().join("docs/decisions/adr-001.md").exists());
        assert!(temp.path().join(".archive/decisions/adr-001.md").exists());
    }

    #[test]
    fn test_restore_from_archive() {
        let temp = TempDir::new().unwrap();

        // Manually create an archived file with proper header
        std::fs::create_dir_all(temp.path().join(".archive/docs")).unwrap();
        let archive_path = temp.path().join(".archive/docs/restored.md");
        let original_path = temp.path().join("docs/restored.md");

        std::fs::create_dir_all(temp.path().join("docs")).unwrap();
        std::fs::write(
            &archive_path,
            format!(
                "<!-- ARCHIVED: 2024-01-01T00:00:00Z -->\n<!-- REASON: test -->\n<!-- ORIGINAL: {} -->\n\nRestored content",
                original_path.display()
            )
        ).unwrap();

        let manager = ArchiveManager::new(temp.path().to_path_buf(), 90);
        let restored = manager.restore(&archive_path).unwrap();

        assert!(restored.exists());
        assert!(!archive_path.exists());
        let content = std::fs::read_to_string(&restored).unwrap();
        assert!(content.contains("Restored content"));
    }

    #[test]
    fn test_archived_file_struct() {
        let file = ArchivedFile {
            original_path: "/docs/test.md".to_string(),
            archive_path: "/.archive/docs/test.md".to_string(),
            reason: "Stale".to_string(),
            age_days: 100,
        };

        assert_eq!(file.age_days, 100);
        assert!(file.reason.contains("Stale"));
    }
}
