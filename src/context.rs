//! Context builder for generating LLM-ready project context bundles.

use ralph::config::{default_ignore_dirs, default_ignore_files, extensions};
use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use walkdir::WalkDir;

/// Context building mode
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ContextMode {
    /// Include all file types
    #[default]
    Full,
    /// Code files only
    Code,
    /// Documentation files only
    Docs,
    /// Configuration files only
    Config,
}

impl ContextMode {
    fn extensions(&self) -> Vec<&'static str> {
        match self {
            ContextMode::Full => extensions::all(),
            ContextMode::Code => extensions::CODE.to_vec(),
            ContextMode::Docs => extensions::DOCS.to_vec(),
            ContextMode::Config => extensions::CONFIG.to_vec(),
        }
    }
}

/// Statistics about the built context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    pub files_included: usize,
    pub files_skipped: usize,
    pub total_lines: usize,
    pub estimated_tokens: usize,
    pub stale_files: Vec<String>,
    pub archive_excluded: usize,
    pub generated_at: DateTime<Utc>,
    pub context_hash: String,
}

/// Builder for creating project context bundles
pub struct ContextBuilder {
    project_dir: PathBuf,
    mode: ContextMode,
    max_tokens: usize,
    include_narsil: bool,
    stale_threshold_days: u32,
    max_file_size_kb: usize,
    ignore_dirs: HashSet<String>,
    ignore_files: HashSet<String>,
}

impl ContextBuilder {
    /// Create a new context builder for a project
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            project_dir,
            mode: ContextMode::Full,
            max_tokens: 100_000,
            include_narsil: true,
            stale_threshold_days: 90,
            max_file_size_kb: 500,
            ignore_dirs: default_ignore_dirs().iter().map(|s: &&str| s.to_string()).collect(),
            ignore_files: default_ignore_files().iter().map(|s: &&str| s.to_string()).collect(),
        }
    }

    /// Set the context mode
    pub fn mode(mut self, mode: ContextMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set maximum tokens
    pub fn max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Whether to include narsil-mcp summaries
    pub fn include_narsil(mut self, include: bool) -> Self {
        self.include_narsil = include;
        self
    }

    /// Set stale threshold in days
    pub fn stale_threshold_days(mut self, days: u32) -> Self {
        self.stale_threshold_days = days;
        self
    }

    /// Build statistics without writing output
    pub fn build_stats(&self) -> Result<ContextStats> {
        let mut stats = ContextStats {
            files_included: 0,
            files_skipped: 0,
            total_lines: 0,
            estimated_tokens: 0,
            stale_files: Vec::new(),
            archive_excluded: 0,
            generated_at: Utc::now(),
            context_hash: String::new(),
        };

        let extensions = self.mode.extensions();

        for entry in self.walk_files() {
            let path = entry.path();

            if !self.should_include_file(path, &extensions) {
                stats.files_skipped += 1;
                continue;
            }

            if let Ok(metadata) = fs::metadata(path) {
                if metadata.len() > (self.max_file_size_kb * 1024) as u64 {
                    stats.files_skipped += 1;
                    continue;
                }

                if let Ok(content) = fs::read_to_string(path) {
                    let tokens = estimate_tokens(&content);

                    if stats.estimated_tokens + tokens > self.max_tokens {
                        stats.files_skipped += 1;
                        continue;
                    }

                    stats.files_included += 1;
                    stats.total_lines += content.lines().count();
                    stats.estimated_tokens += tokens;

                    if self.is_file_stale(path) {
                        if let Some(rel_path) = self.relative_path(path) {
                            stats.stale_files.push(rel_path);
                        }
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Build context and write to output file
    pub fn build(&self, output: &Path) -> Result<ContextStats> {
        let file = File::create(output)?;
        let mut writer = BufWriter::new(file);
        let mut content_for_hash = String::new();

        let mut stats = ContextStats {
            files_included: 0,
            files_skipped: 0,
            total_lines: 0,
            estimated_tokens: 0,
            stale_files: Vec::new(),
            archive_excluded: 0,
            generated_at: Utc::now(),
            context_hash: String::new(),
        };

        // Write header
        let header = format!(
            "<!-- Context Bundle: {} -->\n<!-- Generated: {} -->\n<!-- Mode: {:?} -->\n\n",
            self.project_dir
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default(),
            Utc::now().to_rfc3339(),
            self.mode
        );
        writer.write_all(header.as_bytes())?;
        content_for_hash.push_str(&header);

        // Add narsil-mcp summaries if available
        if self.include_narsil {
            let intelligence = self.get_project_intelligence();
            if !intelligence.is_empty() {
                writer.write_all(b"<project_intelligence>\n")?;
                writer.write_all(intelligence.as_bytes())?;
                writer.write_all(b"</project_intelligence>\n\n")?;
                content_for_hash.push_str(&intelligence);
            }
        }

        // Process files
        writer.write_all(b"<files>\n\n")?;

        let extensions = self.mode.extensions();
        let mut files: Vec<_> = self
            .walk_files()
            .filter(|e| self.should_include_file(e.path(), &extensions))
            .collect();

        // Sort: docs first, then code, then config
        files.sort_by(|a, b| {
            let a_priority = self.file_priority(a.path());
            let b_priority = self.file_priority(b.path());
            a_priority.cmp(&b_priority).then_with(|| a.path().cmp(b.path()))
        });

        for entry in files {
            let path = entry.path();

            if let Ok(metadata) = fs::metadata(path) {
                if metadata.len() > (self.max_file_size_kb * 1024) as u64 {
                    stats.files_skipped += 1;
                    continue;
                }
            }

            if let Ok(content) = fs::read_to_string(path) {
                let tokens = estimate_tokens(&content);

                if stats.estimated_tokens + tokens > self.max_tokens {
                    stats.files_skipped += 1;
                    continue;
                }

                let rel_path = self.relative_path(path).unwrap_or_else(|| path.display().to_string());
                let is_stale = self.is_file_stale(path);
                let age_days = self.file_age_days(path);

                // Write file tag
                let mut tag = format!(
                    "<file name=\"{}\" path=\"{}\"",
                    path.file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default(),
                    rel_path
                );

                if is_stale {
                    tag.push_str(&format!(" stale=\"true\" age_days=\"{}\"", age_days));
                    stats.stale_files.push(rel_path.clone());
                }
                tag.push_str(">\n");

                writer.write_all(tag.as_bytes())?;
                writer.write_all(content.as_bytes())?;
                writer.write_all(b"\n</file>\n\n")?;

                content_for_hash.push_str(&tag);
                content_for_hash.push_str(&content);

                stats.files_included += 1;
                stats.total_lines += content.lines().count();
                stats.estimated_tokens += tokens;
            }
        }

        writer.write_all(b"</files>\n")?;
        writer.flush()?;

        // Compute hash
        stats.context_hash = compute_hash(&content_for_hash);

        Ok(stats)
    }

    /// Walk files in project directory
    fn walk_files(&self) -> impl Iterator<Item = walkdir::DirEntry> + '_ {
        WalkDir::new(&self.project_dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();

                // Skip hidden directories and ignored directories
                if e.file_type().is_dir() {
                    return !name.starts_with('.') && !self.ignore_dirs.contains(name.as_ref());
                }

                true
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
    }

    /// Check if file should be included
    fn should_include_file(&self, path: &Path, extensions: &[&str]) -> bool {
        let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();

        // Skip hidden files
        if name.starts_with('.') {
            return false;
        }

        // Skip ignored files
        if self.ignore_files.contains(name.as_ref()) {
            return false;
        }

        // Check extension
        let ext = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        extensions.iter().any(|e| ext.eq_ignore_ascii_case(e))
    }

    /// Get relative path from project directory
    fn relative_path(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.project_dir)
            .ok()
            .map(|p| p.display().to_string())
    }

    /// Check if file is stale
    fn is_file_stale(&self, path: &Path) -> bool {
        self.file_age_days(path) > self.stale_threshold_days as i64
    }

    /// Get file age in days
    fn file_age_days(&self, path: &Path) -> i64 {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|mtime| {
                let now = SystemTime::now();
                let duration = now.duration_since(mtime).unwrap_or_default();
                (duration.as_secs() / 86400) as i64
            })
            .unwrap_or(0)
    }

    /// Get file priority for sorting (lower = earlier)
    fn file_priority(&self, path: &Path) -> u8 {
        let ext = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        if extensions::DOCS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
            0
        } else if extensions::CODE.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
            1
        } else {
            2
        }
    }

    /// Get project intelligence from narsil-mcp
    fn get_project_intelligence(&self) -> String {
        let mut output = String::new();

        // Try to get project structure
        if let Some(structure) = self.run_narsil("get_project_structure") {
            output.push_str("<structure>\n");
            output.push_str(&structure);
            output.push_str("</structure>\n\n");
        }

        // Try to get key symbols
        if let Some(symbols) = self.run_narsil("find_symbols") {
            output.push_str("<key_symbols>\n");
            // Truncate if too long
            let truncated: String = symbols.chars().take(5000).collect();
            output.push_str(&truncated);
            output.push_str("</key_symbols>\n\n");
        }

        // Try to get security summary for full/code modes
        if matches!(self.mode, ContextMode::Full | ContextMode::Code) {
            if let Some(security) = self.run_narsil("get_security_summary") {
                output.push_str("<security_summary>\n");
                output.push_str(&security);
                output.push_str("</security_summary>\n\n");
            }
        }

        output
    }

    /// Run a narsil-mcp command
    fn run_narsil(&self, command: &str) -> Option<String> {
        let result = Command::new("narsil-mcp")
            .arg("--repos")
            .arg(&self.project_dir)
            .arg(command)
            .output()
            .ok()?;

        if result.status.success() {
            Some(String::from_utf8_lossy(&result.stdout).into_owned())
        } else {
            None
        }
    }
}

/// Estimate tokens (roughly 4 chars per token for code)
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Compute MD5 hash of content
fn compute_hash(content: &str) -> String {
    format!("{:x}", md5::compute(content.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello world"), 2);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_context_mode_extensions() {
        assert!(!ContextMode::Full.extensions().is_empty());
        assert!(ContextMode::Code.extensions().contains(&".rs"));
        assert!(ContextMode::Docs.extensions().contains(&".md"));
        assert!(ContextMode::Config.extensions().contains(&".toml"));
    }
}
