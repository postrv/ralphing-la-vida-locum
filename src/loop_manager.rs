//! Core automation loop manager - the Bjarne-style loop.
//!
//! This module handles the main automation loop that runs Claude Code
//! iterations with stagnation detection, mode switching, and analytics.

use ralph::Analytics;
use ralph::config::ProjectConfig;
use crate::supervisor::{Supervisor, SupervisorVerdict};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::ValueEnum;
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn};

/// Loop execution mode
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LoopMode {
    /// Planning phase - create implementation plan
    Plan,
    /// Build phase - implement tasks
    Build,
    /// Debug phase - focus on blockers
    Debug,
}

impl std::fmt::Display for LoopMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoopMode::Plan => write!(f, "plan"),
            LoopMode::Build => write!(f, "build"),
            LoopMode::Debug => write!(f, "debug"),
        }
    }
}

/// State of the automation loop
#[derive(Debug)]
pub struct LoopState {
    pub iteration: u32,
    pub stagnation_count: u32,
    pub last_plan_hash: String,
    pub last_commit_hash: String,
    pub cumulative_changes: u32,
    pub mode: LoopMode,
    pub session_id: String,
}

impl LoopState {
    /// Create a new loop state with the given mode
    pub fn new(mode: LoopMode) -> Self {
        Self {
            iteration: 0,
            stagnation_count: 0,
            last_plan_hash: String::new(),
            last_commit_hash: String::new(),
            cumulative_changes: 0,
            mode,
            session_id: Utc::now().timestamp().to_string(),
        }
    }
}

impl Default for LoopState {
    fn default() -> Self {
        Self::new(LoopMode::Build)
    }
}

/// Maximum retries for transient failures (LSP crashes, etc.)
const MAX_RETRIES: u32 = 3;

/// Backoff delay between retries in milliseconds
const RETRY_BACKOFF_MS: u64 = 2000;

/// Estimated tokens per byte (conservative estimate for code)
const TOKENS_PER_BYTE: f64 = 0.25;

/// Warning threshold for file size (Claude's limit is ~25k tokens)
const FILE_SIZE_WARNING_TOKENS: usize = 20_000;

/// Critical threshold - files this size will cause Claude errors
const FILE_SIZE_CRITICAL_TOKENS: usize = 25_000;

/// The main loop manager
#[derive(Debug)]
pub struct LoopManager {
    project_dir: PathBuf,
    max_iterations: u32,
    stagnation_threshold: u32,
    doc_sync_interval: u32,
    state: LoopState,
    analytics: Analytics,
    config: ProjectConfig,
    verbose: bool,
}

impl LoopManager {
    /// Create a new loop manager
    pub fn new(
        project_dir: PathBuf,
        mode: LoopMode,
        max_iterations: u32,
        stagnation_threshold: u32,
        doc_sync_interval: u32,
        config: ProjectConfig,
        verbose: bool,
    ) -> Result<Self> {
        let analytics = Analytics::new(project_dir.clone());
        let state = LoopState::new(mode);

        // Ensure required files exist
        let plan_path = project_dir.join("IMPLEMENTATION_PLAN.md");
        if !plan_path.exists() {
            bail!(
                "IMPLEMENTATION_PLAN.md not found. Run 'ralph bootstrap' first or create the file."
            );
        }

        info!(
            "LoopManager initialized: {} allowed permissions, {} safety blocks",
            config.permissions.allow.len(),
            config.permissions.deny.len()
        );

        Ok(Self {
            project_dir,
            max_iterations,
            stagnation_threshold,
            doc_sync_interval,
            state,
            analytics,
            config,
            verbose,
        })
    }

    /// Run the main automation loop
    pub async fn run(&mut self) -> Result<()> {
        self.print_banner();

        // Log session start
        self.analytics.log_event(
            &self.state.session_id,
            "session_start",
            serde_json::json!({
                "mode": self.state.mode.to_string(),
                "max_iterations": self.max_iterations,
                "config_loaded": true,
            }),
        )?;

        // Get initial plan and commit hashes
        self.state.last_plan_hash = self.get_plan_hash()?;
        self.state.last_commit_hash = self.get_commit_hash().unwrap_or_default();

        // Create supervisor for health monitoring (checks every 5 iterations by default)
        let mut supervisor = Supervisor::new(self.project_dir.clone())
            .with_interval(5);

        while self.state.iteration < self.max_iterations {
            self.state.iteration += 1;

            self.print_iteration_header();

            // Check for progress using multiple indicators (commits AND plan changes)
            if self.has_made_progress() {
                // Progress detected - reset stagnation and update tracking
                self.state.stagnation_count = 0;
                self.state.last_plan_hash = self.get_plan_hash()?;
                self.state.last_commit_hash = self.get_commit_hash().unwrap_or_default();

                // If we were in debug mode due to stagnation, return to build mode
                if self.state.mode == LoopMode::Debug {
                    info!("Progress resumed, returning to build mode");
                    println!(
                        "   {} Progress detected, returning to build mode",
                        "Info:".green().bold()
                    );
                    self.state.mode = LoopMode::Build;
                }
            } else {
                // No progress - increment stagnation counter
                self.state.stagnation_count += 1;

                if self.state.stagnation_count >= self.stagnation_threshold {
                    warn!(
                        "Stagnation detected ({} iterations without commits or plan changes)",
                        self.state.stagnation_count
                    );
                    self.state.mode = LoopMode::Debug;
                    println!(
                        "   {} Switching to debug mode (no commits or plan changes)",
                        "Warning:".yellow().bold()
                    );

                    // Log stagnation event
                    self.analytics.log_event(
                        &self.state.session_id,
                        "stagnation",
                        serde_json::json!({
                            "iteration": self.state.iteration,
                            "count": self.state.stagnation_count,
                            "last_commit": self.state.last_commit_hash,
                        }),
                    )?;
                }
            }

            // Run Claude Code iteration with retry logic for transient failures
            let mut retry_count = 0;
            let mut should_break = false;

            loop {
                let result = self.run_claude_iteration().await;

                match result {
                    Ok(exit_code) => {
                        if exit_code == 0 {
                            // Success - continue to next iteration
                            break;
                        } else if exit_code == 1 && retry_count < MAX_RETRIES {
                            // Check if this might be a transient LSP failure
                            retry_count += 1;
                            warn!(
                                "Iteration failed (attempt {}/{}), cleaning up LSP and retrying...",
                                retry_count, MAX_RETRIES
                            );
                            eprintln!(
                                "   {} Transient failure detected, retrying ({}/{})...",
                                "Retry:".yellow().bold(),
                                retry_count,
                                MAX_RETRIES
                            );

                            // Clean up LSP processes and wait
                            Self::cleanup_lsp();
                            tokio::time::sleep(std::time::Duration::from_millis(
                                RETRY_BACKOFF_MS * u64::from(retry_count),
                            ))
                            .await;
                        } else {
                            // Exhausted retries or non-recoverable error
                            eprintln!(
                                "   {} Fatal error from Claude Code (after {} retries)",
                                "Error:".red().bold(),
                                retry_count
                            );
                            should_break = true;
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("   {} {}", "Error:".red().bold(), e);
                        // Record error for supervisor pattern detection
                        supervisor.record_error(&e.to_string());
                        // Log error but continue
                        self.analytics.log_event(
                            &self.state.session_id,
                            "iteration_error",
                            serde_json::json!({
                                "iteration": self.state.iteration,
                                "error": e.to_string(),
                                "retry_count": retry_count,
                            }),
                        )?;
                        break;
                    }
                }
            }

            if should_break {
                break;
            }

            // Track cumulative changes
            if let Ok(changes) = self.get_recent_changes() {
                self.state.cumulative_changes += changes;

                // Trigger project analysis if significant changes
                if self.state.cumulative_changes > 500 {
                    info!(
                        "Significant changes detected ({} lines) - consider running project analysis",
                        self.state.cumulative_changes
                    );
                    self.state.cumulative_changes = 0;
                }
            }

            // Doc sync check (using is_multiple_of as suggested by clippy)
            if self.doc_sync_interval > 0
                && self.state.iteration.checked_rem(self.doc_sync_interval) == Some(0)
                && self.state.mode == LoopMode::Build
            {
                self.run_doc_sync().await?;
            }

            // File size check (every 5 iterations in build mode)
            if self.state.iteration.checked_rem(5) == Some(0) && self.state.mode == LoopMode::Build
            {
                self.check_file_sizes();
            }

            // Auto-archive check (every 10 iterations)
            if self.state.iteration.checked_rem(10) == Some(0) {
                if let Err(e) = self.run_auto_archive() {
                    debug!("Auto-archive check failed: {}", e);
                }
            }

            // Supervisor health check
            if supervisor.should_check(self.state.iteration) {
                let result = if self.verbose {
                    supervisor.check_verbose(&self.state, self.state.iteration)
                } else {
                    supervisor.check(&self.state, self.state.iteration)
                };
                match result {
                    Ok(verdict) => match verdict {
                        SupervisorVerdict::Proceed => {
                            debug!(
                                "Supervisor: health OK (interval={}, mode_switches={}, last_check={})",
                                supervisor.check_interval(),
                                supervisor.mode_switch_count(),
                                supervisor.last_check_iteration()
                            );
                        }
                        SupervisorVerdict::SwitchMode { target, reason } => {
                            info!("Supervisor recommends mode switch: {}", reason);
                            println!(
                                "   {} Supervisor: switching to {} mode ({})",
                                "Supervisor:".bright_blue().bold(),
                                target,
                                reason
                            );
                            self.state.mode = target;
                            supervisor.record_mode_switch();
                        }
                        SupervisorVerdict::Reset { reason } => {
                            info!("Supervisor recommends reset: {}", reason);
                            println!(
                                "   {} Supervisor: resetting stagnation ({})",
                                "Supervisor:".bright_blue().bold(),
                                reason
                            );
                            self.state.stagnation_count = 0;
                        }
                        SupervisorVerdict::PauseForReview { reason } => {
                            warn!("Supervisor requests pause: {}", reason);
                            println!(
                                "   {} Supervisor: pausing for review ({})",
                                "Warning:".yellow().bold(),
                                reason
                            );
                            self.analytics.log_event(
                                &self.state.session_id,
                                "supervisor_pause",
                                serde_json::json!({ "reason": reason }),
                            )?;
                            break;
                        }
                        SupervisorVerdict::Abort { reason } => {
                            warn!("Supervisor abort: {}", reason);
                            println!(
                                "   {} Supervisor: aborting ({})",
                                "Error:".red().bold(),
                                reason
                            );
                            // Generate and save diagnostics before aborting
                            if let Ok(report) = supervisor.generate_diagnostics(&self.analytics) {
                                if let Ok(path) = report.save(&self.project_dir) {
                                    println!(
                                        "   {} Diagnostics saved to: {}",
                                        "Info:".blue(),
                                        path.display()
                                    );
                                }
                            }
                            self.analytics.log_event(
                                &self.state.session_id,
                                "supervisor_abort",
                                serde_json::json!({ "reason": reason }),
                            )?;
                            bail!("Supervisor abort: {}", reason);
                        }
                    },
                    Err(e) => {
                        debug!("Supervisor check failed: {}", e);
                    }
                }
            }

            // Push to remote
            self.try_push().await;

            // Check for completion
            if self.is_complete()? {
                println!(
                    "\n   {} All tasks complete!",
                    "Success:".green().bold()
                );
                break;
            }

            // Log iteration
            self.analytics.log_event(
                &self.state.session_id,
                "iteration",
                serde_json::json!({
                    "iteration": self.state.iteration,
                    "stagnation": self.state.stagnation_count,
                    "mode": self.state.mode.to_string(),
                }),
            )?;
        }

        // Run final security audit
        println!("\n{} Running final security audit...", "Info:".blue());
        self.run_security_audit().await?;

        // Log session end
        self.analytics.log_event(
            &self.state.session_id,
            "session_end",
            serde_json::json!({
                "iterations": self.state.iteration,
                "final_mode": self.state.mode.to_string(),
            }),
        )?;

        println!(
            "\n{} Session complete. Analytics: .ralph/analytics.jsonl",
            "Done:".green().bold()
        );

        Ok(())
    }

    /// Print the startup banner
    fn print_banner(&self) {
        println!("{}", "═".repeat(60).bright_blue());
        println!(
            "{}",
            "     RALPH - Claude Code Automation Suite".bright_blue().bold()
        );
        println!("{}", "═".repeat(60).bright_blue());
        println!();
        println!("   Project: {}", self.project_dir.display());
        println!("   Mode: {}", self.state.mode);
        println!("   Max iterations: {}", self.max_iterations);
        println!("   Stagnation threshold: {}", self.stagnation_threshold);
        if !self.config.permissions.allow.is_empty() {
            println!(
                "   Permissions: {} allowed, {} safety blocks",
                self.config.permissions.allow.len(),
                self.config.permissions.deny.len()
            );
        }

        if self.verbose {
            println!();
            println!("{}", "   Allowed operations:".cyan());
            for perm in &self.config.permissions.allow {
                println!("     ✓ {}", perm.green());
            }
            if !self.config.permissions.deny.is_empty() {
                println!("{}", "   Safety blocks (denied):".cyan());
                for perm in &self.config.permissions.deny {
                    println!("     ✗ {}", perm.red());
                }
            }
        }
        println!();
    }

    /// Print iteration header
    fn print_iteration_header(&self) {
        println!(
            "\n{} Iteration {}/{} (stagnation: {}/{})",
            "===".bright_blue(),
            self.state.iteration,
            self.max_iterations,
            self.state.stagnation_count,
            self.stagnation_threshold
        );
    }

    /// Get MD5 hash of IMPLEMENTATION_PLAN.md
    fn get_plan_hash(&self) -> Result<String> {
        let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
        let content = std::fs::read_to_string(&plan_path)
            .context("Failed to read IMPLEMENTATION_PLAN.md")?;
        Ok(format!("{:x}", md5::compute(content.as_bytes())))
    }

    /// Get the current git commit hash (HEAD)
    fn get_commit_hash(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.project_dir)
            .output()
            .context("Failed to run git rev-parse")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Ok(String::new())
        }
    }

    /// Count commits between two hashes
    fn count_commits_since(&self, old_hash: &str) -> u32 {
        if old_hash.is_empty() {
            return 0;
        }

        let output = Command::new("git")
            .args(["rev-list", "--count", &format!("{old_hash}..HEAD")])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse()
                    .unwrap_or(0)
            }
            _ => 0,
        }
    }

    /// Check if there's been any progress (commits or plan changes)
    fn has_made_progress(&self) -> bool {
        // Check for new commits
        let commit_count = self.count_commits_since(&self.state.last_commit_hash);
        if commit_count > 0 {
            debug!("Progress detected: {} new commit(s)", commit_count);
            return true;
        }

        // Check for plan file changes
        if let Ok(current_hash) = self.get_plan_hash() {
            if current_hash != self.state.last_plan_hash {
                debug!("Progress detected: IMPLEMENTATION_PLAN.md changed");
                return true;
            }
        }

        false
    }

    /// Get the prompt file for current mode
    fn get_prompt_path(&self) -> PathBuf {
        let filename = format!("PROMPT_{}.md", self.state.mode);
        self.project_dir.join(filename)
    }

    /// Run a single Claude Code iteration
    async fn run_claude_iteration(&self) -> Result<i32> {
        let prompt_path = self.get_prompt_path();

        if !prompt_path.exists() {
            bail!("Prompt file not found: {}", prompt_path.display());
        }

        let prompt = std::fs::read_to_string(&prompt_path)?;

        debug!("Running Claude Code with prompt from {}", prompt_path.display());

        // Build command arguments
        let args = vec!["-p", "--dangerously-skip-permissions", "--model", "opus"];

        // If we have a CLAUDE.md, reference it
        let claude_md = ProjectConfig::claude_md_path(&self.project_dir);
        if claude_md.exists() {
            debug!("Using CLAUDE.md from {}", claude_md.display());
        }

        // Run claude with the prompt piped to stdin
        let mut child = AsyncCommand::new("claude")
            .args(&args)
            .current_dir(&self.project_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        // Write prompt to stdin, flush, and close to signal EOF
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.flush().await?;
            drop(stdin); // Explicitly close stdin to signal EOF
        }

        let status = child.wait().await?;
        Ok(status.code().unwrap_or(1))
    }

    /// Run documentation sync check
    async fn run_doc_sync(&self) -> Result<()> {
        println!("   {} Running documentation sync check...", "Info:".blue());

        // Try to run docs-sync agent if available
        let result = AsyncCommand::new("claude")
            .args(["--dangerously-skip-permissions", "--agent", "docs-sync", "Check for documentation drift"])
            .current_dir(&self.project_dir)
            .output()
            .await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("drift_detected") && stdout.contains("true") {
                    println!(
                        "   {} Documentation drift detected",
                        "Warning:".yellow()
                    );
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

    /// Try to push to remote (uses BatchMode to avoid SSH passphrase hang)
    async fn try_push(&self) {
        // Get current branch
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.project_dir)
            .output();

        if let Ok(output) = branch_output {
            if output.status.success() {
                let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

                // Use GIT_SSH_COMMAND with BatchMode to fail fast on passphrase prompt
                // instead of hanging indefinitely
                let push_result = AsyncCommand::new("git")
                    .args(["push", "origin", &branch])
                    .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=10")
                    .current_dir(&self.project_dir)
                    .output()
                    .await;

                match push_result {
                    Ok(output) if output.status.success() => {
                        debug!("Pushed to origin/{}", branch);
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("Host key verification failed")
                            || stderr.contains("Permission denied")
                        {
                            debug!(
                                "Push failed due to SSH auth - consider using HTTPS remote or ssh-agent"
                            );
                        } else {
                            debug!("Push failed: {}", stderr);
                        }
                    }
                    Err(e) => {
                        debug!("Push error: {}", e);
                    }
                }
            }
        }
    }

    /// Check if all tasks are complete
    fn is_complete(&self) -> Result<bool> {
        let plan_path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
        let content = std::fs::read_to_string(&plan_path)?;
        Ok(content.contains("ALL_TASKS_COMPLETE"))
    }

    /// Get number of lines changed in recent commit
    fn get_recent_changes(&self) -> Result<u32> {
        let output = Command::new("git")
            .args(["diff", "--stat", "HEAD~1"])
            .current_dir(&self.project_dir)
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse last line for changes count
            if let Some(last_line) = stdout.lines().last() {
                // Extract numbers from "X files changed, Y insertions(+), Z deletions(-)"
                let numbers: Vec<u32> = last_line
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if numbers.len() >= 2 {
                    return Ok(numbers[1] + numbers.get(2).unwrap_or(&0));
                }
            }
        }

        Ok(0)
    }

    /// Run final security audit
    async fn run_security_audit(&self) -> Result<()> {
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

    /// Get the current project configuration
    ///
    /// Returns a reference to the project configuration for inspection
    /// or passing to other components.
    #[cfg(test)]
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    /// Get the current loop state
    ///
    /// Returns a reference to the current state for inspection,
    /// including iteration count, mode, and checkpoint history.
    #[cfg(test)]
    pub fn state(&self) -> &LoopState {
        &self.state
    }

    /// Clean up stale LSP processes (rust-analyzer, etc.)
    ///
    /// This helps prevent LSP crashes from accumulating and causing failures
    /// in long-running automation sessions.
    fn cleanup_lsp() {
        // Kill any stale rust-analyzer processes
        let _ = Command::new("pkill")
            .args(["-f", "rust-analyzer"])
            .output();

        // Give processes time to terminate
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    /// Check for oversized source files that might cause Claude errors
    ///
    /// Scans the project for source files exceeding token thresholds.
    /// Returns a list of (path, estimated_tokens) tuples for problematic files.
    fn find_oversized_files(&self) -> Vec<(PathBuf, usize)> {
        let mut oversized = Vec::new();
        let src_dir = self.project_dir.join("src");

        if !src_dir.exists() {
            return oversized;
        }

        let code_extensions = ["rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "cpp", "c", "h"];

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

    /// Check files and print warnings for oversized ones
    fn check_file_sizes(&self) {
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

    /// Run automatic archiving of stale markdown files
    fn run_auto_archive(&self) -> Result<()> {
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

    /// Find stale markdown files in the project root
    fn find_stale_root_markdown(&self) -> Result<Vec<PathBuf>> {
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
            if ["README.md", "IMPLEMENTATION_PLAN.md", "CLAUDE.md", "CHANGELOG.md"]
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_loop_state_new() {
        let state = LoopState::new(LoopMode::Plan);
        assert_eq!(state.mode, LoopMode::Plan);
        assert_eq!(state.iteration, 0);
        assert_eq!(state.stagnation_count, 0);
    }

    #[test]
    fn test_loop_mode_display() {
        assert_eq!(LoopMode::Plan.to_string(), "plan");
        assert_eq!(LoopMode::Build.to_string(), "build");
        assert_eq!(LoopMode::Debug.to_string(), "debug");
    }

    #[test]
    fn test_loop_manager_requires_plan() {
        let temp = TempDir::new().unwrap();
        let result = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("IMPLEMENTATION_PLAN.md"));
    }

    #[test]
    fn test_loop_manager_creates_with_plan() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let result = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_loop_manager_config_accessor() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let config = ProjectConfig {
            permissions: ralph::config::PermissionsConfig {
                allow: vec!["Bash(git *)".to_string()],
                deny: vec![],
            },
            ..Default::default()
        };

        let manager = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            config,
            false,
        ).unwrap();

        // Test config accessor
        assert_eq!(manager.config().permissions.allow.len(), 1);
        assert!(manager.config().permissions.allow.contains(&"Bash(git *)".to_string()));
    }

    #[test]
    fn test_loop_manager_state_accessor() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let manager = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Plan,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        ).unwrap();

        // Test state accessor
        assert_eq!(manager.state().mode, LoopMode::Plan);
        assert_eq!(manager.state().iteration, 0);
        assert_eq!(manager.state().stagnation_count, 0);
    }

    #[test]
    fn test_loop_state_default() {
        let state = LoopState::default();
        assert_eq!(state.mode, LoopMode::Build);
        assert_eq!(state.iteration, 0);
    }

    #[test]
    fn test_find_oversized_files_empty_project() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();

        let manager = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        )
        .unwrap();

        // No src directory - should return empty
        let oversized = manager.find_oversized_files();
        assert!(oversized.is_empty());
    }

    #[test]
    fn test_find_oversized_files_small_files() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();

        // Write a small Rust file (well under threshold)
        std::fs::write(
            temp.path().join("src/main.rs"),
            "fn main() { println!(\"Hello\"); }",
        )
        .unwrap();

        let manager = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        )
        .unwrap();

        let oversized = manager.find_oversized_files();
        assert!(oversized.is_empty());
    }

    #[test]
    fn test_find_oversized_files_detects_large() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();

        // Write a large Rust file (~100k bytes = ~25k tokens at 0.25 ratio)
        let large_content = "fn test() { }\n".repeat(8000);
        std::fs::write(temp.path().join("src/large.rs"), &large_content).unwrap();

        let manager = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        )
        .unwrap();

        let oversized = manager.find_oversized_files();
        assert!(!oversized.is_empty());
        assert!(oversized[0].0.ends_with("large.rs"));
    }

    #[test]
    fn test_find_stale_root_markdown_no_stale() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("IMPLEMENTATION_PLAN.md"), "# Plan").unwrap();
        std::fs::write(temp.path().join("README.md"), "# Readme").unwrap();

        let manager = LoopManager::new(
            temp.path().to_path_buf(),
            LoopMode::Build,
            10,
            3,
            5,
            ProjectConfig::default(),
            false,
        )
        .unwrap();

        // Fresh files should not be stale
        let stale = manager.find_stale_root_markdown().unwrap();
        assert!(stale.is_empty());
    }

    #[test]
    fn test_token_estimation() {
        // Test that token estimation produces reasonable values
        // A 1000 byte file should produce ~250 tokens at 0.25 tokens/byte
        let file_size_bytes: u64 = 1000;
        let estimated_tokens = (file_size_bytes as f64 * TOKENS_PER_BYTE) as usize;
        assert!(estimated_tokens > 200);
        assert!(estimated_tokens < 300);

        // A file at the critical threshold should be ~100KB
        let critical_bytes = (FILE_SIZE_CRITICAL_TOKENS as f64 / TOKENS_PER_BYTE) as u64;
        assert!(critical_bytes > 90_000);
        assert!(critical_bytes < 110_000);
    }
}
