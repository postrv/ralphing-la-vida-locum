//! Checkpoint, archive, and maintenance methods for `LoopManager`.
//!
//! This module contains methods for:
//! - Creating and managing checkpoints
//! - Auto-archiving stale files
//! - Security audits
//! - Documentation sync
//! - File size monitoring

use super::{LoopManager, FILE_SIZE_CRITICAL_TOKENS, FILE_SIZE_WARNING_TOKENS, TOKENS_PER_BYTE};
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use tokio::process::Command as AsyncCommand;
use tracing::debug;

impl LoopManager {
    /// Run documentation sync check.
    pub(crate) async fn run_doc_sync(&self) -> Result<()> {
        println!("   {} Running documentation sync check...", "Info:".blue());

        // Try to run docs-sync agent if available
        let result = AsyncCommand::new("claude")
            .args([
                "--dangerously-skip-permissions",
                "--agent",
                "docs-sync",
                "Check for documentation drift",
            ])
            .current_dir(&self.project_dir)
            .output()
            .await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("drift_detected") && stdout.contains("true") {
                    println!("   {} Documentation drift detected", "Warning:".yellow());
                    self.analytics.log_event(
                        &self.state.session_id,
                        "docs_drift_detected",
                        serde_json::json!({
                            "iteration": self.state.iteration,
                        }),
                    )?;
                }
            }
            Err(e) => {
                debug!("docs-sync agent not available: {}", e);
            }
        }

        Ok(())
    }

    /// Run final security audit.
    pub(crate) async fn run_security_audit(&self) -> Result<()> {
        // Try narsil-mcp scan
        let scan_result = AsyncCommand::new("narsil-mcp")
            .arg("scan_security")
            .current_dir(&self.project_dir)
            .output()
            .await;

        match scan_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("CRITICAL") {
                    eprintln!(
                        "   {} Critical security issues found!",
                        "Warning:".red().bold()
                    );
                } else {
                    println!("   {} Security scan complete", "OK".green());
                }
            }
            Err(_) => {
                debug!("narsil-mcp not available for security scan");
            }
        }

        // Try to generate SBOM
        let sbom_result = AsyncCommand::new("narsil-mcp")
            .args(["generate_sbom", "--format", "cyclonedx"])
            .current_dir(&self.project_dir)
            .output()
            .await;

        if let Ok(output) = sbom_result {
            if output.status.success() {
                let sbom_path = self.project_dir.join("sbom.json");
                std::fs::write(&sbom_path, &output.stdout)?;
                println!("   {} SBOM generated: sbom.json", "OK".green());
            }
        }

        Ok(())
    }

    /// Check for oversized source files that might cause Claude errors.
    ///
    /// Scans the project for source files exceeding token thresholds.
    /// Returns a list of (path, estimated_tokens) tuples for problematic files.
    pub(crate) fn find_oversized_files(&self) -> Vec<(PathBuf, usize)> {
        let mut oversized = Vec::new();
        let src_dir = self.project_dir.join("src");

        if !src_dir.exists() {
            return oversized;
        }

        let code_extensions = [
            "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "cpp", "c", "h",
        ];

        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            if !code_extensions.contains(&ext) {
                continue;
            }

            if let Ok(metadata) = std::fs::metadata(path) {
                let estimated_tokens = (metadata.len() as f64 * TOKENS_PER_BYTE) as usize;

                if estimated_tokens >= FILE_SIZE_WARNING_TOKENS {
                    oversized.push((path.to_path_buf(), estimated_tokens));
                }
            }
        }

        // Sort by size descending
        oversized.sort_by(|a, b| b.1.cmp(&a.1));
        oversized
    }

    /// Check files and print warnings for oversized ones.
    pub(crate) fn check_file_sizes(&self) {
        let oversized = self.find_oversized_files();

        for (path, tokens) in oversized {
            let rel_path = path.strip_prefix(&self.project_dir).unwrap_or(&path);

            if tokens >= FILE_SIZE_CRITICAL_TOKENS {
                eprintln!(
                    "   {} {} (~{} tokens) - will cause Claude errors! Consider splitting.",
                    "CRITICAL:".red().bold(),
                    rel_path.display(),
                    tokens
                );
            } else {
                println!(
                    "   {} {} (~{} tokens) - approaching limit, consider splitting.",
                    "Warning:".yellow(),
                    rel_path.display(),
                    tokens
                );
            }
        }
    }

    /// Run automatic archiving of stale markdown files.
    pub(crate) fn run_auto_archive(&self) -> Result<()> {
        let archive_manager = crate::archive::ArchiveManager::new(
            self.project_dir.clone(),
            90, // 90 day stale threshold
        );

        // Also check for stale markdown files in project root
        let root_stale = self.find_stale_root_markdown()?;

        if !root_stale.is_empty() {
            println!(
                "   {} Found {} stale .md files in project root",
                "Archive:".cyan(),
                root_stale.len()
            );
            for path in &root_stale {
                debug!("Stale root markdown: {}", path.display());
            }
        }

        // Run the standard archive process (dry run first to report)
        let result = archive_manager.run(true)?;

        if result.docs_archived > 0 || result.decisions_archived > 0 {
            println!(
                "   {} {} docs and {} decisions eligible for archiving (run 'ralph archive run')",
                "Info:".blue(),
                result.docs_archived,
                result.decisions_archived
            );
        }

        Ok(())
    }

    /// Find stale markdown files in the project root.
    pub(crate) fn find_stale_root_markdown(&self) -> Result<Vec<PathBuf>> {
        let mut stale_files = Vec::new();
        let threshold_secs = 90 * 24 * 60 * 60; // 90 days in seconds

        // Look for markdown files directly in project root (not in subdirectories)
        for entry in std::fs::read_dir(&self.project_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "md" {
                continue;
            }

            // Skip known important files
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if [
                "README.md",
                "IMPLEMENTATION_PLAN.md",
                "CLAUDE.md",
                "CHANGELOG.md",
            ]
            .contains(&filename)
            {
                continue;
            }

            // Skip PROMPT_*.md files
            if filename.starts_with("PROMPT_") {
                continue;
            }

            // Check age
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = std::time::SystemTime::now().duration_since(modified) {
                        if duration.as_secs() > threshold_secs {
                            stale_files.push(path);
                        }
                    }
                }
            }
        }

        Ok(stale_files)
    }
}
