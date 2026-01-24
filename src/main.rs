//! Ralph - Claude Code Automation Suite
//!
//! A Rust-based automation suite for running Claude Code autonomously
//! with bombproof reliability, type-checking, and memory guarantees.

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

mod archive;
mod context;
mod hooks;
mod r#loop;
mod session;
mod supervisor;

use crate::archive::ArchiveManager;
use crate::context::ContextBuilder;
use crate::hooks::HookType;
use crate::r#loop::{LoopManager, LoopManagerConfig, LoopMode};
use ralph::bootstrap::language_detector::LanguageDetector;
use ralph::bootstrap::Bootstrap;
use ralph::quality::gates::{detect_available_gates, gates_for_language, is_gate_available};
use ralph::quality::{EnforcerConfig, PluginLoader};
use ralph::Analytics;
use ralph::ProjectConfig;

#[derive(Parser)]
#[command(name = "ralph")]
#[command(author = "Claude Code Automation Suite")]
#[command(version = "0.1.0")]
#[command(about = "Autonomous Claude Code execution with bombproof reliability", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Project directory (defaults to current directory)
    #[arg(short, long, global = true, default_value = ".")]
    project: PathBuf,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the main automation loop
    Loop {
        /// Loop mode: plan, build, or debug
        #[arg(value_enum, default_value = "build")]
        mode: LoopMode,

        /// Maximum iterations
        #[arg(short, long, default_value = "50")]
        max_iterations: u32,

        /// Stagnation threshold before switching to debug mode
        #[arg(short, long, default_value = "5")]
        stagnation_threshold: u32,

        /// Sync docs every N iterations (0 to disable)
        #[arg(long, default_value = "5")]
        doc_sync_interval: u32,

        /// Skip running tests in quality checks (faster iteration)
        #[arg(long)]
        skip_tests: bool,

        /// Skip security scans in quality checks
        #[arg(long)]
        skip_security: bool,

        /// Stagnation predictor weight profile: balanced, conservative, or aggressive
        #[arg(long, value_name = "PROFILE")]
        predictor_profile: Option<String>,

        /// LLM model to use: claude, openai, gemini, or ollama
        /// (Note: only claude is currently implemented)
        #[arg(long, value_name = "MODEL")]
        model: Option<String>,

        /// Run quality gates in parallel for faster feedback (default: true).
        /// Use --no-parallel-gates to disable.
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        parallel_gates: bool,

        /// Timeout for individual gate execution in milliseconds (default: 60000)
        #[arg(long, default_value = "60000", value_name = "MS")]
        gate_timeout: u64,

        /// Run gates incrementally based on changed files (default: true).
        /// Only gates for languages with changed files will run.
        /// Use --no-incremental-gates to disable.
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        incremental_gates: bool,

        /// Resume from a previous session if available.
        /// Defaults to true. Use --no-resume or --fresh to start fresh.
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        resume: bool,

        /// Start fresh, ignoring any existing session state.
        /// Alias for --no-resume.
        #[arg(long)]
        fresh: bool,

        /// Disable session persistence on shutdown.
        /// Useful for testing or when session recovery is not needed.
        #[arg(long)]
        no_persist: bool,

        /// Run in incremental mode, processing only files changed since the given commit.
        /// Example: --changed-since HEAD~1 or --changed-since abc1234
        #[arg(long, value_name = "COMMIT")]
        changed_since: Option<String>,
    },

    /// Build context for LLM analysis
    Context {
        /// Output file path
        #[arg(short, long, default_value = "context.txt")]
        output: PathBuf,

        /// Context mode: full, code, docs, or config
        #[arg(short, long, default_value = "full")]
        mode: context::ContextMode,

        /// Maximum estimated tokens
        #[arg(long, default_value = "100000")]
        max_tokens: usize,

        /// Skip narsil-mcp integration
        #[arg(long)]
        no_narsil: bool,

        /// Output JSON stats only
        #[arg(long)]
        stats_only: bool,

        /// Days after which files are marked stale
        #[arg(long, default_value = "90")]
        stale_days: u32,
    },

    /// Manage documentation archives
    Archive {
        #[command(subcommand)]
        action: ArchiveAction,
    },

    /// Bootstrap automation suite in a project
    Bootstrap {
        /// Force overwrite existing files
        #[arg(short, long)]
        force: bool,

        /// Skip git hook installation
        #[arg(long)]
        no_git_hooks: bool,

        /// Override detected languages. Can be specified multiple times for polyglot projects.
        /// Accepts language names (rust, python, typescript) or common aliases (rs, py, ts).
        #[arg(short, long = "language", value_name = "LANG")]
        languages: Vec<String>,

        /// Only detect and display project languages without bootstrapping
        #[arg(long)]
        detect_only: bool,
    },

    /// Detect programming languages and available quality gates in a project
    Detect {
        /// Show available quality gates for detected languages
        #[arg(long)]
        show_gates: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Analyze project (generate full analysis artifacts)
    Analyze {
        /// Output directory for analysis artifacts
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },

    /// Show analytics and session history
    Analytics {
        #[command(subcommand)]
        action: AnalyticsAction,
    },

    /// Security hooks and validation
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },

    /// Show or validate project configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage checkpoints
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// Manage quality gate plugins
    Plugins {
        #[command(subcommand)]
        action: PluginsAction,
    },

    /// Manage audit logs
    Audit {
        #[command(subcommand)]
        action: AuditAction,
    },

    /// Verify code quality changes using CCG-Diff verification
    ///
    /// Compares code quality before and after changes to verify
    /// improvements. Currently requires --mock flag as the real
    /// narsil-mcp integration is not yet complete.
    Verify {
        /// Use mock verifier for development/testing (required until narsil-mcp integration)
        #[arg(long)]
        mock: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Output as Markdown
        #[arg(long)]
        markdown: bool,

        /// Write report to file
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },

    /// Show project status including predictor stats and session state
    ///
    /// Displays the current state of the automation including:
    /// - Predictor statistics (accuracy, predictions by risk level)
    /// - Current session state (if any)
    /// - Recent commits
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CheckpointAction {
    /// Compare two checkpoints and show the diff
    Diff {
        /// ID of the baseline checkpoint (from)
        from: String,

        /// ID of the checkpoint to compare (to)
        to: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List all checkpoints
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed information about a checkpoint
    Show {
        /// Checkpoint ID
        id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ArchiveAction {
    /// Run archive process (move stale docs to archive)
    Run {
        /// Stale threshold in days
        #[arg(short, long, default_value = "90")]
        stale_days: u32,

        /// Dry run - don't actually move files
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Show archive statistics
    Stats {
        /// Stale threshold in days (for stale file detection)
        #[arg(short, long, default_value = "90")]
        stale_days: u32,
    },

    /// List stale files that would be archived
    ListStale {
        /// Stale threshold in days
        #[arg(short, long, default_value = "90")]
        stale_days: u32,
    },

    /// Restore a file from archive
    Restore {
        /// Path to the archived file to restore
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum AnalyticsAction {
    /// Show recent sessions
    Sessions {
        /// Show last N sessions
        #[arg(short, long, default_value = "5")]
        last: usize,

        /// Show detailed events
        #[arg(short, long)]
        detailed: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show aggregate statistics across all sessions
    Aggregate {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show LLM cost tracking (cumulative and per-session)
    Costs {
        /// Show per-session breakdown
        #[arg(short, long)]
        sessions: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Clear all analytics data
    Clear {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Log a custom event (for scripting)
    Log {
        /// Session ID
        #[arg(short, long)]
        session: String,

        /// Event name
        #[arg(short, long)]
        event: String,

        /// Event data as JSON
        #[arg(short, long, default_value = "{}")]
        data: String,
    },

    /// Generate HTML dashboard from analytics data
    Dashboard {
        /// Output file path (default: .ralph/dashboard.html)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Filter to last N sessions
        #[arg(short, long)]
        sessions: Option<usize>,

        /// Output raw data as JSON instead of HTML
        #[arg(long)]
        json: bool,

        /// Open dashboard in default browser after generating
        #[arg(long)]
        open: bool,
    },
}

#[derive(Subcommand)]
enum HookAction {
    /// Run a specific hook type
    Run {
        /// Hook type to run
        #[arg(value_enum)]
        hook_type: HookType,

        /// Input for the hook (JSON or command string)
        input: Option<String>,
    },

    /// Validate a command against security rules
    Validate {
        /// The command to validate
        command: String,
    },

    /// Scan a file for secrets
    Scan {
        /// File path to scan
        path: PathBuf,
    },

    /// Scan all modified files for secrets
    ScanModified,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Validate configuration files
    Validate {
        /// Show detailed validation output including inheritance chain
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show configuration file paths
    Paths,
}

#[derive(Subcommand)]
enum PluginsAction {
    /// List all discovered plugins
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed information about a specific plugin
    Info {
        /// Plugin name
        name: String,
    },
}

#[derive(Subcommand)]
enum AuditAction {
    /// Display recent audit entries
    Show {
        /// Maximum number of entries to display
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Show entries since this datetime (RFC 3339 format, e.g., 2024-01-01T00:00:00Z)
        #[arg(long, value_name = "DATETIME")]
        since: Option<String>,

        /// Filter by event type (command_execution, gate_result, commit, session_start, session_end, checkpoint_created, rollback, config_change)
        #[arg(short = 't', long, value_name = "TYPE")]
        event_type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Verify integrity of the audit log hash chain
    Verify {
        /// Repair corrupted log by truncating at first invalid entry (creates backup)
        #[arg(long)]
        repair: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        "ralph=debug,info"
    } else {
        "ralph=info,warn"
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Resolve project path
    let project_path = cli.project.canonicalize().unwrap_or(cli.project.clone());

    if !project_path.exists() {
        eprintln!(
            "{} Project directory does not exist: {}",
            "Error:".red().bold(),
            project_path.display()
        );
        std::process::exit(1);
    }

    match cli.command {
        Commands::Loop {
            mode,
            max_iterations,
            stagnation_threshold,
            doc_sync_interval,
            skip_tests,
            skip_security,
            predictor_profile,
            model,
            parallel_gates,
            gate_timeout,
            incremental_gates,
            resume,
            fresh,
            no_persist,
            changed_since,
        } => {
            // Session persistence (Phase 21.4)
            let ralph_dir = project_path.join(".ralph");
            let session_persistence = session::persistence::SessionPersistence::new(&ralph_dir);

            // Determine if we should resume: --resume=true AND --fresh=false
            // --fresh overrides --resume (they're essentially opposites)
            let should_resume = resume && !fresh;

            // Handle --fresh flag by deleting existing session state
            if fresh && session_persistence.exists() {
                if let Err(e) = session_persistence.delete() {
                    tracing::warn!("Failed to delete existing session: {}", e);
                } else {
                    tracing::info!("Deleted existing session state (--fresh mode)");
                }
            }

            // Clean up any stale temporary files from interrupted saves
            let tmp_path = session_persistence.tmp_file_path();
            if tmp_path.exists() {
                let _ = std::fs::remove_file(&tmp_path);
            }

            // Log session file paths for debugging
            tracing::debug!(
                "Session: main={}, tmp={}, resume={}",
                session_persistence.session_file_path().display(),
                session_persistence.tmp_file_path().display(),
                should_resume
            );

            // Load project configuration
            let mut config = ProjectConfig::load(&project_path).unwrap_or_default();

            // Override predictor weights with CLI profile if specified
            if let Some(ref profile) = predictor_profile {
                // Validate the profile name
                match profile.to_lowercase().as_str() {
                    "balanced" | "conservative" | "aggressive" => {
                        config.predictor_weights.preset = Some(profile.to_lowercase());
                    }
                    _ => {
                        eprintln!(
                            "{} Invalid predictor profile '{}'. Valid options: balanced, conservative, aggressive",
                            "Error:".red().bold(),
                            profile
                        );
                        std::process::exit(1);
                    }
                }
            }

            // Override LLM model with CLI flag if specified
            if let Some(ref model_name) = model {
                config.llm.model = model_name.to_lowercase();
                // Validate the model
                if let Err(e) = config.llm.validate() {
                    eprintln!("{} {}", "Error:".red().bold(), e);
                    std::process::exit(1);
                }
            }

            let mut loop_config = LoopManagerConfig::new(project_path.clone(), config)
                .with_mode(mode)
                .with_max_iterations(max_iterations)
                .with_stagnation_threshold(stagnation_threshold)
                .with_doc_sync_interval(doc_sync_interval)
                .with_verbose(cli.verbose)
                .with_session_persistence(session_persistence.clone())
                .with_resume(should_resume);

            // Handle --changed-since flag for incremental execution (Phase 26.4)
            if let Some(ref commit) = changed_since {
                let detector = ralph::changes::ChangeDetector::new(&project_path);
                match ralph::changes::ChangeScope::from_detector_since(&detector, commit) {
                    Ok(scope) => {
                        let file_count = scope.changed_files().len();
                        tracing::info!(
                            "Running in incremental mode: {} files changed since {}",
                            file_count,
                            commit
                        );
                        loop_config = loop_config.with_change_scope(scope);
                    }
                    Err(e) => {
                        eprintln!(
                            "{} Failed to detect changes since '{}': {}",
                            "Error:".red().bold(),
                            commit,
                            e
                        );
                        std::process::exit(1);
                    }
                }
            }

            // Configure signal handler for graceful shutdown (Phase 21.3)
            // Create a placeholder session state - the actual state will be saved by LoopManager
            let placeholder_session = session::SessionState::new();
            let signal_config = session::signals::SignalHandlerConfig {
                persist: !no_persist,
            };
            let signal_handler = session::signals::SignalHandler::new(signal_config)
                .with_persistence(session_persistence.clone(), placeholder_session);

            if no_persist {
                tracing::info!("Session persistence disabled (--no-persist flag)");
            } else {
                tracing::debug!(
                    "Signal handler configured: persist_enabled={}, has_persistence={}",
                    signal_handler.persist_enabled(),
                    signal_handler.has_persistence()
                );
            }

            // Configure quality gates
            // Always create config to apply parallel gates and incremental settings
            let quality_config = EnforcerConfig::new()
                .with_clippy(true)
                .with_tests(!skip_tests)
                .with_security(!skip_security)
                .with_no_allow(true)
                .with_parallel_gates(parallel_gates)
                .with_gate_timeout_ms(gate_timeout)
                .with_incremental_gates(incremental_gates);
            loop_config = loop_config.with_quality_config(quality_config);

            let mut manager = LoopManager::new(loop_config)?;

            // Run the loop with signal handling for graceful shutdown (Phase 21.3/21.4)
            // Use tokio::select! to race between the loop and shutdown signals
            tokio::select! {
                result = manager.run() => {
                    // Loop completed normally
                    result?;
                    // Save final session state on normal completion using LoopManager's shutdown
                    if let Err(e) = manager.shutdown() {
                        tracing::warn!("Failed to save session on completion: {}", e);
                    }
                    // Also call signal handler's shutdown for compatibility
                    let final_result = signal_handler.shutdown().await;
                    tracing::debug!("Loop completed, shutdown result: {:?}", final_result);
                }
                result = signal_handler.wait_for_shutdown() => {
                    // Signal received - save state using LoopManager's shutdown
                    if let Err(e) = manager.shutdown() {
                        tracing::warn!("Failed to save session on signal: {}", e);
                    }
                    match result {
                        Ok(shutdown_result) => {
                            tracing::info!("Graceful shutdown completed: {:?}", shutdown_result);
                        }
                        Err(e) => {
                            tracing::error!("Error during graceful shutdown: {}", e);
                        }
                    }
                }
            }
        }

        Commands::Context {
            output,
            mode,
            max_tokens,
            no_narsil,
            stats_only,
            stale_days,
        } => {
            let builder = ContextBuilder::new(project_path)
                .mode(mode)
                .max_tokens(max_tokens)
                .include_narsil(!no_narsil)
                .stale_threshold_days(stale_days);

            if stats_only {
                let stats = builder.build_stats()?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                let stats = builder.build(&output)?;
                println!(
                    "\n{} Context built: {}",
                    "OK".green().bold(),
                    output.display()
                );
                println!(
                    "   Files: {} included, {} skipped",
                    stats.files_included, stats.files_skipped
                );
                println!("   Lines: {}", stats.total_lines);
                println!("   Tokens (est): {}", stats.estimated_tokens);

                if !stats.stale_files.is_empty() {
                    println!(
                        "   {} Stale files ({}): {}",
                        "Warning:".yellow(),
                        stats.stale_files.len(),
                        stats
                            .stale_files
                            .iter()
                            .take(5)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
        }

        Commands::Archive { action } => match action {
            ArchiveAction::Run {
                stale_days,
                dry_run,
            } => {
                let manager = ArchiveManager::new(project_path, stale_days);
                let result = manager.run(dry_run)?;

                println!(
                    "\n{} Archive Manager (threshold: {} days)",
                    "OK".green().bold(),
                    stale_days
                );
                println!("   Stale docs archived: {}", result.docs_archived);
                println!("   Deprecated ADRs archived: {}", result.decisions_archived);

                if !result.files_processed.is_empty() {
                    println!("\n   Files processed:");
                    for file in &result.files_processed {
                        println!("     - {} ({})", file.original_path, file.reason);
                    }
                }

                if dry_run {
                    println!("\n   {} Dry run - no changes made", "Info:".blue());
                }
            }

            ArchiveAction::Stats { stale_days } => {
                let manager = ArchiveManager::new(project_path, stale_days);
                let stats = manager.get_stats()?;
                println!("{}", serde_json::to_string_pretty(&stats)?);
            }

            ArchiveAction::ListStale { stale_days } => {
                let manager = ArchiveManager::new(project_path, stale_days);
                let stale_files = manager.find_stale_files()?;

                if stale_files.is_empty() {
                    println!(
                        "{} No stale files found (threshold: {} days)",
                        "OK".green(),
                        stale_days
                    );
                } else {
                    println!(
                        "{} Found {} stale files (threshold: {} days):\n",
                        "Warning:".yellow().bold(),
                        stale_files.len(),
                        stale_days
                    );
                    for file in &stale_files {
                        println!("  {} ({} days old)", file.original_path, file.age_days);
                    }
                }
            }

            ArchiveAction::Restore { path } => {
                let manager = ArchiveManager::new(project_path, 90);
                let restored_path = manager.restore(&path)?;
                println!(
                    "{} Restored: {}",
                    "OK".green().bold(),
                    restored_path.display()
                );
            }
        },

        Commands::Bootstrap {
            force,
            no_git_hooks,
            languages,
            detect_only,
        } => {
            // Parse language overrides if provided
            let parsed_languages: Vec<ralph::Language> = if languages.is_empty() {
                Vec::new()
            } else {
                let mut result = Vec::new();
                for lang_str in &languages {
                    match lang_str.parse::<ralph::Language>() {
                        Ok(lang) => result.push(lang),
                        Err(e) => {
                            eprintln!("{} {}: {}", "Error:".red().bold(), e, lang_str);
                            std::process::exit(1);
                        }
                    }
                }
                result
            };

            // Create bootstrap instance with optional language override
            let bootstrap = if parsed_languages.is_empty() {
                Bootstrap::new(project_path)
            } else {
                Bootstrap::new(project_path).with_languages(parsed_languages.clone())
            };

            // Handle detect-only mode
            if detect_only {
                // Display language override if provided
                if !parsed_languages.is_empty() {
                    println!("\n{} Override languages:", "Languages:".cyan());
                    for lang in &parsed_languages {
                        println!("   → {}", lang.to_string().bold());
                    }
                    println!();
                } else {
                    // Display detected languages
                    let detected = bootstrap.detect_languages();
                    if detected.is_empty() {
                        println!("\n{} No programming languages detected", "Note:".yellow());
                        println!("   This is an empty or unrecognized project type");
                    } else {
                        println!("\n{} Detected languages:", "Languages:".cyan());
                        for lang in &detected {
                            let marker = if lang.primary { "→" } else { " " };
                            let primary_tag = if lang.primary {
                                " (primary)".green().to_string()
                            } else {
                                String::new()
                            };
                            println!(
                                "   {} {}: {:.0}% confidence ({} files){}",
                                marker,
                                lang.language.to_string().bold(),
                                lang.confidence * 100.0,
                                lang.file_count,
                                primary_tag
                            );
                        }
                    }
                }
                return Ok(());
            }

            // Display override languages if provided
            if !parsed_languages.is_empty() {
                println!("\n{} Override languages:", "Languages:".cyan());
                for lang in &parsed_languages {
                    println!("   → {}", lang.to_string().bold());
                }
                println!();
            }

            bootstrap.run(force, !no_git_hooks)?;

            println!("\n{} Automation suite bootstrapped!", "OK".green().bold());
            println!("\nQuick start:");
            println!("  1. Edit IMPLEMENTATION_PLAN.md with your tasks");
            println!("  2. Run: ralph loop plan --max-iterations 5");
            println!("  3. Run: ralph loop build --max-iterations 50");
        }

        Commands::Detect { show_gates, json } => {
            // Detect languages in the project
            let detector = LanguageDetector::new(&project_path);
            let detected = detector.detect();

            if json {
                // JSON output mode
                let output = serde_json::json!({
                    "languages": detected.iter().map(|d| {
                        let mut lang_info = serde_json::json!({
                            "language": d.language.to_string(),
                            "confidence": d.confidence,
                            "file_count": d.file_count,
                            "primary": d.primary
                        });

                        if show_gates {
                            let lang_gates = gates_for_language(d.language);
                            let available: Vec<_> = lang_gates.iter()
                                .filter(|g| is_gate_available(g.as_ref()))
                                .map(|g| serde_json::json!({
                                    "name": g.name(),
                                    "blocking": g.is_blocking(),
                                    "available": true
                                }))
                                .collect();
                            let unavailable: Vec<_> = lang_gates.iter()
                                .filter(|g| !is_gate_available(g.as_ref()))
                                .map(|g| serde_json::json!({
                                    "name": g.name(),
                                    "blocking": g.is_blocking(),
                                    "available": false,
                                    "required_tool": g.required_tool()
                                }))
                                .collect();
                            lang_info["gates"] = serde_json::json!({
                                "available": available,
                                "unavailable": unavailable
                            });
                        }

                        lang_info
                    }).collect::<Vec<_>>()
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                // Human-readable output
                if detected.is_empty() {
                    println!("\n{} No programming languages detected", "Note:".yellow());
                    println!("   This is an empty or unrecognized project type");
                } else {
                    println!("\n{} Detected languages:", "Languages:".cyan().bold());
                    for lang in &detected {
                        let marker = if lang.primary { "→" } else { " " };
                        let primary_tag = if lang.primary {
                            " (primary)".green().to_string()
                        } else {
                            String::new()
                        };
                        println!(
                            "   {} {}: {:.0}% confidence ({} files){}",
                            marker,
                            lang.language.to_string().bold(),
                            lang.confidence * 100.0,
                            lang.file_count,
                            primary_tag
                        );
                    }

                    // Show gates if requested
                    if show_gates {
                        println!("\n{} Available gates:", "Gates:".cyan().bold());

                        // Group gates by language
                        for lang in &detected {
                            let lang_gates = gates_for_language(lang.language);
                            if lang_gates.is_empty() {
                                continue;
                            }

                            println!("\n   {}:", lang.language.to_string().bold());
                            for gate in &lang_gates {
                                let available = is_gate_available(gate.as_ref());
                                let status = if available {
                                    "✓".green().to_string()
                                } else {
                                    "✗".red().to_string()
                                };
                                let blocking_tag = if gate.is_blocking() {
                                    " (blocking)"
                                } else {
                                    ""
                                };
                                let tool_info = if !available {
                                    if let Some(tool) = gate.required_tool() {
                                        format!(" - requires: {}", tool.yellow())
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    String::new()
                                };
                                println!(
                                    "     {} {}{}{}",
                                    status,
                                    gate.name(),
                                    blocking_tag,
                                    tool_info
                                );
                            }
                        }
                    } else {
                        // Show summary of available gates
                        let languages: Vec<_> = detected
                            .iter()
                            .filter(|d| {
                                d.confidence >= LanguageDetector::DEFAULT_POLYGLOT_THRESHOLD
                            })
                            .map(|d| d.language)
                            .collect();

                        let available_gates = detect_available_gates(&project_path, &languages);
                        if !available_gates.is_empty() {
                            println!(
                                "\n{} {} gates available",
                                "Gates:".cyan().bold(),
                                available_gates.len()
                            );
                            println!("   Use {} for details", "--show-gates".cyan());
                        }
                    }
                }
            }
        }

        Commands::Analyze { output_dir } => {
            let output = output_dir.unwrap_or_else(|| project_path.join(".ralph/analysis"));
            std::fs::create_dir_all(&output)?;

            println!("{} Analyzing project...", "Info:".blue());

            // Build full context
            let context_output = output.join(format!(
                "context-{}.txt",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ));
            let builder = ContextBuilder::new(project_path.clone());
            let stats = builder.build(&context_output)?;

            println!(
                "   {} Context: {} ({} files, ~{} tokens)",
                "OK".green(),
                context_output.display(),
                stats.files_included,
                stats.estimated_tokens
            );

            // Build docs context
            let docs_output = output.join(format!(
                "docs-{}.txt",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ));
            let docs_builder =
                ContextBuilder::new(project_path.clone()).mode(context::ContextMode::Docs);
            let _ = docs_builder.build(&docs_output)?;

            println!(
                "   {} Docs context: {}",
                "OK".green(),
                docs_output.display()
            );

            // Generate analysis prompt
            let prompt_output = output.join(format!(
                "prompt-{}.md",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ));
            std::fs::write(&prompt_output, include_str!("templates/analysis_prompt.md"))?;

            println!(
                "   {} Analysis prompt: {}",
                "OK".green(),
                prompt_output.display()
            );

            println!(
                "\n{} Upload {} to Claude/Grok/Gemini for analysis",
                "Next:".cyan().bold(),
                context_output.display()
            );
        }

        Commands::Analytics { action } => {
            let analytics = Analytics::new(project_path.clone());

            match action {
                AnalyticsAction::Sessions {
                    last,
                    detailed,
                    json,
                } => {
                    let sessions = analytics.get_recent_sessions(last)?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&sessions)?);
                    } else {
                        analytics.print_summary(&sessions, detailed);
                    }
                }

                AnalyticsAction::Aggregate { json } => {
                    let stats = analytics.get_aggregate_stats()?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&stats)?);
                    } else {
                        println!("\n{} Aggregate Statistics", "Analytics:".cyan().bold());
                        println!("{}", "─".repeat(40));
                        println!("   Total sessions: {}", stats.total_sessions);
                        println!("   Total iterations: {}", stats.total_iterations);
                        println!("   Total errors: {}", stats.total_errors);
                        println!("   Total stagnations: {}", stats.total_stagnations);
                        println!("   Docs drift events: {}", stats.total_drift_events);
                    }
                }

                AnalyticsAction::Costs { sessions, json } => {
                    use ralph::analytics::CostTracker;

                    let tracker = CostTracker::new(&project_path)?;

                    if json {
                        // Output structured JSON
                        let output = serde_json::json!({
                            "total_cost_usd": tracker.total_cost(),
                            "total_tokens": tracker.total_tokens(),
                            "providers": tracker.all_providers(),
                            "recent_sessions": if sessions { Some(tracker.recent_sessions()) } else { None }
                        });
                        println!("{}", serde_json::to_string_pretty(&output)?);
                    } else {
                        // Human-readable output
                        println!("\n{} LLM Cost Summary", "Analytics:".cyan().bold());
                        println!("{}", "─".repeat(40));
                        println!(
                            "   Total cost: {}",
                            format!("${:.4}", tracker.total_cost()).green()
                        );
                        println!(
                            "   Total tokens: {} ({} requests)",
                            tracker.total_tokens(),
                            tracker
                                .all_providers()
                                .values()
                                .map(|p| p.request_count)
                                .sum::<u64>()
                        );

                        if !tracker.all_providers().is_empty() {
                            println!("\n   {} By Provider:", "─".repeat(20));
                            let mut providers: Vec<_> = tracker.all_providers().iter().collect();
                            providers.sort_by(|a, b| {
                                b.1.total_cost_usd.partial_cmp(&a.1.total_cost_usd).unwrap()
                            });

                            for (name, cost) in providers {
                                println!(
                                    "   {}: ${:.4} ({} tokens, {} requests)",
                                    name.cyan(),
                                    cost.total_cost_usd,
                                    cost.total_tokens(),
                                    cost.request_count
                                );
                            }
                        }

                        if sessions && !tracker.recent_sessions().is_empty() {
                            println!("\n   {} Recent Sessions:", "─".repeat(20));
                            for session in tracker.recent_sessions().iter().rev().take(10) {
                                println!(
                                    "   {} ${:.4}",
                                    session.session_id.dimmed(),
                                    session.total_cost_usd
                                );
                            }
                        }
                    }
                }

                AnalyticsAction::Clear { force } => {
                    if !force {
                        eprintln!(
                            "{} This will delete all analytics data. Use --force to confirm.",
                            "Warning:".yellow().bold()
                        );
                        std::process::exit(1);
                    }

                    analytics.clear()?;
                    println!("{} Analytics data cleared", "OK".green().bold());
                }

                AnalyticsAction::Log {
                    session,
                    event,
                    data,
                } => {
                    let data: serde_json::Value = serde_json::from_str(&data)
                        .unwrap_or_else(|_| serde_json::json!({"raw": data}));

                    analytics.log_event(&session, &event, data)?;
                    println!("{} Event logged", "OK".green());
                }

                AnalyticsAction::Dashboard {
                    output,
                    sessions,
                    json,
                    open,
                } => {
                    use ralph::analytics::dashboard::{
                        DashboardData, DashboardTemplate, TimeRange,
                    };

                    // Determine time range filter
                    let time_range = match sessions {
                        Some(n) => TimeRange::LastNSessions(n),
                        None => TimeRange::All,
                    };

                    // Aggregate dashboard data
                    let dashboard = DashboardData::from_analytics(&analytics, time_range)?;

                    if json {
                        // Output raw JSON data
                        println!("{}", serde_json::to_string_pretty(&dashboard)?);
                    } else {
                        // Generate HTML and write to file
                        let template = DashboardTemplate::new(&dashboard);
                        let html = template.render();

                        // Determine output path (default: .ralph/dashboard.html)
                        let output_path =
                            output.unwrap_or_else(|| project_path.join(".ralph/dashboard.html"));

                        // Create parent directory if it doesn't exist
                        if let Some(parent) = output_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        // Write HTML file
                        std::fs::write(&output_path, &html)?;

                        println!(
                            "{} Dashboard written to: {}",
                            "OK".green().bold(),
                            output_path.display()
                        );

                        // Open in browser if requested
                        if open {
                            let _ = open_in_browser(&output_path);
                        }
                    }
                }
            }
        }

        Commands::Hook { action } => {
            match action {
                HookAction::Run { hook_type, input } => {
                    let result = hooks::run_hook(hook_type, input.as_deref())?;

                    // Print warnings if any
                    for warning in &result.warnings {
                        println!("{} {}", "Warning:".yellow(), warning);
                    }

                    if result.blocked {
                        eprintln!(
                            "{} Hook blocked: {}",
                            "Blocked:".red().bold(),
                            result.message.unwrap_or_default()
                        );
                        std::process::exit(2);
                    } else if let Some(msg) = result.message {
                        println!("{}", msg);
                    }
                }

                HookAction::Validate { command } => {
                    // Load project config to check allow/deny lists
                    let config = ProjectConfig::load(&project_path).unwrap_or_default();
                    let result = hooks::validate_command_with_config(&command, &config)?;

                    for warning in &result.warnings {
                        println!("{} {}", "Warning:".yellow(), warning);
                    }

                    if result.blocked {
                        eprintln!(
                            "{} Command blocked: {}",
                            "Blocked:".red().bold(),
                            result.message.unwrap_or_default()
                        );
                        std::process::exit(2);
                    } else {
                        println!("{} Command is safe", "OK".green().bold());
                    }
                }

                HookAction::Scan { path } => {
                    let findings = hooks::scan_file_for_secrets(&path)?;

                    if findings.is_empty() {
                        println!(
                            "{} No secrets found in {}",
                            "OK".green().bold(),
                            path.display()
                        );
                    } else {
                        eprintln!(
                            "{} Found {} potential secrets in {}:",
                            "Warning:".yellow().bold(),
                            findings.len(),
                            path.display()
                        );
                        for finding in &findings {
                            eprintln!("  - {}", finding);
                        }
                        std::process::exit(1);
                    }
                }

                HookAction::ScanModified => {
                    let result = hooks::run_hook(HookType::PostEditScan, None)?;

                    for warning in &result.warnings {
                        println!("{} {}", "Warning:".yellow(), warning);
                    }

                    if result.warnings.is_empty() {
                        println!("{} No secrets found in modified files", "OK".green().bold());
                    } else {
                        std::process::exit(1);
                    }
                }
            }
        }

        Commands::Config { action } => {
            match action {
                ConfigAction::Show { json } => {
                    let config = ProjectConfig::load(&project_path)?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&config)?);
                    } else {
                        println!("\n{} Project Configuration", "Config:".cyan().bold());
                        println!("{}", "─".repeat(40));
                        println!("   Respect gitignore: {}", config.respect_gitignore);
                        println!("   Allowed permissions: {}", config.permissions.allow.len());
                        println!("   Denied permissions: {}", config.permissions.deny.len());
                        println!("   PreToolUse hooks: {}", config.hooks.pre_tool_use.len());
                        println!("   PostToolUse hooks: {}", config.hooks.post_tool_use.len());
                        println!("   Stop hooks: {}", config.hooks.stop.len());
                        println!(
                            "   SessionStart hooks: {}",
                            config.hooks.session_start.len()
                        );
                    }
                }

                ConfigAction::Validate { verbose, json } => {
                    let validator = ralph::config::ConfigValidator::new(&project_path);
                    let report = validator.validate()?;

                    if json {
                        // JSON output mode
                        let output = serde_json::json!({
                            "valid": report.is_valid(),
                            "errors": report.errors,
                            "warnings": report.warnings,
                            "files_checked": report.files_checked.iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>(),
                            "inheritance_chain": report.inheritance_chain.sources.iter()
                                .map(|s| serde_json::json!({
                                    "level": format!("{}", s.level),
                                    "path": s.path.display().to_string(),
                                    "loaded": s.loaded
                                }))
                                .collect::<Vec<_>>(),
                            "exit_code": report.exit_code()
                        });
                        println!("{}", serde_json::to_string_pretty(&output)?);
                    } else if verbose {
                        // Verbose human-readable output
                        println!("{}", report.verbose_report());
                    } else {
                        // Standard human-readable output
                        println!("\n{} Configuration Validation", "Config:".cyan().bold());
                        println!("{}", "─".repeat(40));

                        // Show errors
                        for error in &report.errors {
                            eprintln!("   {} {}", "Error:".red(), error);
                        }

                        // Show warnings
                        for warning in &report.warnings {
                            println!("   {} {}", "Warning:".yellow(), warning);
                        }

                        // Show files checked
                        println!();
                        println!("   Files checked: {}", report.files_checked.len());

                        // Final status
                        println!();
                        if report.is_valid() {
                            println!("   {} {}", "OK".green().bold(), report.summary());
                        } else {
                            eprintln!("   {} {}", "Failed:".red().bold(), report.summary());
                        }
                    }

                    if !report.is_valid() {
                        std::process::exit(report.exit_code());
                    }
                }

                ConfigAction::Paths => {
                    println!("\n{} Configuration Paths", "Config:".cyan().bold());
                    println!("{}", "─".repeat(40));
                    println!(
                        "   Settings: {}",
                        ProjectConfig::settings_path(&project_path).display()
                    );
                    println!(
                        "   CLAUDE.md: {}",
                        ProjectConfig::claude_md_path(&project_path).display()
                    );
                    println!(
                        "   Analytics: {}",
                        ProjectConfig::analytics_dir(&project_path).display()
                    );
                    println!(
                        "   Archive: {}",
                        ProjectConfig::archive_dir(&project_path).display()
                    );
                    println!(
                        "   Analysis: {}",
                        ProjectConfig::analysis_dir(&project_path).display()
                    );
                }
            }
        }

        Commands::Checkpoint { action } => {
            let checkpoint_dir = project_path.join(".ralph/checkpoints");

            match action {
                CheckpointAction::Diff { from, to, json } => {
                    let mut manager = ralph::checkpoint::CheckpointManager::new(&checkpoint_dir)?;
                    let diff = manager.diff(&from, &to)?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&diff)?);
                    } else {
                        println!("{}", diff.detailed_report());
                    }
                }

                CheckpointAction::List { json } => {
                    let mut manager = ralph::checkpoint::CheckpointManager::new(&checkpoint_dir)?;
                    let checkpoints = manager.list_checkpoints()?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&checkpoints)?);
                    } else {
                        println!(
                            "\n{} Checkpoints ({} total)",
                            "Checkpoints:".cyan().bold(),
                            checkpoints.len()
                        );
                        println!("{}", "─".repeat(60));

                        if checkpoints.is_empty() {
                            println!("   No checkpoints found");
                        } else {
                            for cp in checkpoints {
                                let verified = if cp.verified { " ✓" } else { "" };
                                println!(
                                    "   {} [{}] {}{}",
                                    cp.id,
                                    cp.created_at.format("%Y-%m-%d %H:%M"),
                                    cp.description,
                                    verified.green()
                                );
                                println!(
                                    "      Tests: {} ({} passed, {} failed)",
                                    cp.metrics.test_total,
                                    cp.metrics.test_passed,
                                    cp.metrics.test_failed
                                );
                                println!(
                                    "      Warnings: {}, Security: {}",
                                    cp.metrics.clippy_warnings, cp.metrics.security_issues
                                );
                            }
                        }
                    }
                }

                CheckpointAction::Show { id, json } => {
                    let mut manager = ralph::checkpoint::CheckpointManager::new(&checkpoint_dir)?;
                    let checkpoint_id = ralph::checkpoint::CheckpointId::from_string(&id);
                    let checkpoint = manager
                        .get_checkpoint(&checkpoint_id)?
                        .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", id))?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&checkpoint)?);
                    } else {
                        println!("\n{} Checkpoint Details", "Checkpoint:".cyan().bold());
                        println!("{}", "─".repeat(60));
                        println!("   ID: {}", checkpoint.id);
                        println!(
                            "   Created: {}",
                            checkpoint.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        println!("   Description: {}", checkpoint.description);
                        println!("   Git hash: {}", checkpoint.git_hash);
                        println!("   Git branch: {}", checkpoint.git_branch);
                        println!("   Iteration: {}", checkpoint.iteration);
                        println!(
                            "   Verified: {}",
                            if checkpoint.verified { "Yes" } else { "No" }
                        );
                        println!();
                        println!("   Quality Metrics:");
                        println!("     Tests total:     {}", checkpoint.metrics.test_total);
                        println!("     Tests passed:    {}", checkpoint.metrics.test_passed);
                        println!("     Tests failed:    {}", checkpoint.metrics.test_failed);
                        println!(
                            "     Clippy warnings: {}",
                            checkpoint.metrics.clippy_warnings
                        );
                        println!(
                            "     Security issues: {}",
                            checkpoint.metrics.security_issues
                        );

                        if !checkpoint.files_modified.is_empty() {
                            println!();
                            println!("   Modified files ({}):", checkpoint.files_modified.len());
                            for f in &checkpoint.files_modified {
                                println!("     - {}", f);
                            }
                        }

                        if !checkpoint.tags.is_empty() {
                            println!();
                            println!("   Tags: {}", checkpoint.tags.join(", "));
                        }
                    }
                }
            }
        }

        Commands::Plugins { action } => {
            let loader = PluginLoader::new().with_project_dir(&project_path);

            match action {
                PluginsAction::List { json } => {
                    let result = loader.load_plugins();

                    if json {
                        // Serialize manifest data as JSON
                        let output = serde_json::json!({
                            "plugins": result.manifests.iter().map(|m| {
                                serde_json::json!({
                                    "name": m.plugin.name,
                                    "version": m.plugin.version,
                                    "author": m.plugin.author,
                                    "description": m.plugin.description,
                                    "homepage": m.plugin.homepage,
                                    "license": m.plugin.license,
                                    "library_path": m.library.path,
                                })
                            }).collect::<Vec<_>>(),
                            "warnings": result.warnings,
                            "errors": result.errors,
                        });
                        println!("{}", serde_json::to_string_pretty(&output)?);
                    } else {
                        if result.manifests.is_empty() {
                            println!("\n{} No plugins found.", "Plugins:".cyan().bold());
                            println!();
                            println!("   Plugin locations:");
                            if let Some(user_dir) = PluginLoader::default_user_plugins_dir() {
                                println!("   - User:    {}", user_dir.display());
                            }
                            println!(
                                "   - Project: {}",
                                project_path.join(".ralph/plugins").display()
                            );
                        } else {
                            println!(
                                "\n{} Installed Plugins ({} total)",
                                "Plugins:".cyan().bold(),
                                result.manifests.len()
                            );
                            println!("{}", "─".repeat(60));

                            for manifest in &result.manifests {
                                println!(
                                    "   {} v{}",
                                    manifest.plugin.name.green().bold(),
                                    manifest.plugin.version
                                );
                                println!("     by {}", manifest.plugin.author);
                                if let Some(ref desc) = manifest.plugin.description {
                                    println!("     {}", desc);
                                }
                                if let Some(ref license) = manifest.plugin.license {
                                    println!("     License: {}", license);
                                }
                                println!();
                            }
                        }

                        // Show warnings
                        if !result.warnings.is_empty() {
                            println!("\n{}", "Warnings:".yellow().bold());
                            for warning in &result.warnings {
                                println!("   - {}", warning);
                            }
                        }

                        // Show errors
                        if !result.errors.is_empty() {
                            println!("\n{}", "Errors:".red().bold());
                            for error in &result.errors {
                                println!("   - {}", error);
                            }
                        }
                    }
                }

                PluginsAction::Info { name } => {
                    let result = loader.load_plugins();

                    let manifest = result
                        .manifests
                        .iter()
                        .find(|m| m.plugin.name == name)
                        .ok_or_else(|| anyhow::anyhow!("Plugin not found: {}", name))?;

                    println!("\n{} Plugin Details", "Plugin:".cyan().bold());
                    println!("{}", "─".repeat(60));
                    println!("   Name:        {}", manifest.plugin.name);
                    println!("   Version:     {}", manifest.plugin.version);
                    println!("   Author:      {}", manifest.plugin.author);
                    if let Some(ref desc) = manifest.plugin.description {
                        println!("   Description: {}", desc);
                    }
                    if let Some(ref homepage) = manifest.plugin.homepage {
                        println!("   Homepage:    {}", homepage);
                    }
                    if let Some(ref license) = manifest.plugin.license {
                        println!("   License:     {}", license);
                    }
                    println!();
                    println!("   Library Configuration:");
                    println!("     Path:        {}", manifest.library.path);
                    println!("     Entry point: {}", manifest.library.entry_point);
                    println!();
                    println!("   Runtime Configuration:");
                    println!("     Timeout:     {:?}", manifest.config.timeout);
                    println!(
                        "     Enabled:     {}",
                        if manifest.config.enabled { "Yes" } else { "No" }
                    );
                }
            }
        }

        Commands::Audit { action } => {
            match action {
                AuditAction::Show {
                    limit,
                    since,
                    event_type,
                    json,
                } => {
                    use ralph::audit::{AuditEventType, AuditReader};

                    let reader = AuditReader::new(project_path.clone())?;
                    let mut query = reader.query();

                    // Apply limit
                    query = query.limit(limit);

                    // Apply since filter
                    if let Some(since_str) = since {
                        let since_dt = chrono::DateTime::parse_from_rfc3339(&since_str)
                            .map_err(|e| anyhow::anyhow!("Invalid datetime format: {}. Use RFC 3339 format (e.g., 2024-01-01T00:00:00Z)", e))?
                            .with_timezone(&chrono::Utc);
                        query = query.since(since_dt);
                    }

                    // Apply event type filter
                    if let Some(event_type_str) = event_type {
                        let event_type: AuditEventType = event_type_str.parse()
                            .map_err(|_| anyhow::anyhow!(
                                "Invalid event type '{}'. Valid types: command_execution, gate_result, commit, session_start, session_end, checkpoint_created, rollback, config_change",
                                event_type_str
                            ))?;
                        query = query.event_type(event_type);
                    }

                    let entries = query.execute()?;

                    if json {
                        println!("{}", serde_json::to_string_pretty(&entries)?);
                    } else if entries.is_empty() {
                        println!("\n{} No audit entries found.", "Audit:".cyan().bold());
                    } else {
                        println!(
                            "\n{} Audit Log ({} entries)",
                            "Audit:".cyan().bold(),
                            entries.len()
                        );
                        println!("{}", "─".repeat(100));
                        // Print header row
                        let header = format!(
                            "{:<6} {:<20} {:<20} {:<12} {}",
                            "SEQ", "TIMESTAMP", "EVENT TYPE", "SESSION", "DETAILS"
                        );
                        println!("{}", header);
                        println!("{}", "─".repeat(100));

                        for entry in &entries {
                            let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S");
                            let details = format_audit_details(&entry.data, &entry.event_type);
                            let session_short = if entry.session_id.len() > 10 {
                                format!("{}...", &entry.session_id[..10])
                            } else {
                                entry.session_id.clone()
                            };

                            println!(
                                "{:<6} {:<20} {:<20} {:<12} {}",
                                entry.sequence,
                                timestamp,
                                entry.event_type.to_string(),
                                session_short,
                                details
                            );
                        }
                        println!("{}", "─".repeat(100));
                    }
                }

                AuditAction::Verify { repair, json } => {
                    let logger = ralph::AuditLogger::new(project_path.clone())?;

                    // First, verify the log
                    let verify_result = logger.verify()?;

                    // If repair requested and log is invalid, perform repair
                    let repair_result = if repair && !verify_result.is_valid {
                        Some(logger.repair()?)
                    } else {
                        None
                    };

                    if json {
                        // JSON output
                        let output = serde_json::json!({
                            "verification": {
                                "is_valid": verify_result.is_valid,
                                "entries_verified": verify_result.entries_verified,
                                "first_invalid_entry": verify_result.first_invalid_entry,
                                "error_description": verify_result.error_description,
                            },
                            "repair": repair_result.as_ref().map(|r| serde_json::json!({
                                "repaired": r.repaired,
                                "entries_removed": r.entries_removed,
                                "valid_entries_kept": r.valid_entries_kept,
                                "backup_path": r.backup_path,
                            })),
                        });
                        println!("{}", serde_json::to_string_pretty(&output)?);
                    } else {
                        // Human-readable output
                        println!("\n{} Audit Log Verification", "Audit:".cyan().bold());
                        println!("{}", "─".repeat(60));

                        if verify_result.is_valid {
                            println!("   {} Hash chain integrity verified", "✓".green().bold());
                            println!("   Entries verified: {}", verify_result.entries_verified);
                        } else {
                            println!("   {} Hash chain integrity check failed", "✗".red().bold());
                            println!(
                                "   Entries verified before corruption: {}",
                                verify_result.first_invalid_entry.unwrap_or(0)
                            );
                            if let Some(ref err) = verify_result.error_description {
                                println!("   Error: {}", err.red());
                            }
                        }

                        // Show repair result if repair was performed
                        if let Some(ref r) = repair_result {
                            println!();
                            println!("{}", "─".repeat(60));
                            if r.repaired {
                                println!("   {} Audit log repaired", "✓".green().bold());
                                println!("   Entries removed: {}", r.entries_removed);
                                println!("   Valid entries kept: {}", r.valid_entries_kept);
                                if let Some(ref backup) = r.backup_path {
                                    println!("   Backup saved to: {}", backup);
                                }
                            } else {
                                println!("   No repair needed");
                            }
                        } else if !verify_result.is_valid && !repair {
                            println!();
                            println!(
                                "   {} Use --repair to truncate at corruption point",
                                "Tip:".yellow()
                            );
                        }
                    }

                    // Exit with error code if verification failed and no repair was done
                    if !verify_result.is_valid && repair_result.is_none() {
                        std::process::exit(1);
                    }
                }
            }
        }

        Commands::Verify {
            mock,
            json,
            markdown,
            output,
        } => {
            use ralph::verify::{CcgVerifier, MockCcgVerifier, VerificationConfig};

            // Currently only mock verifier is available
            if !mock {
                eprintln!(
                    "{} The --mock flag is required until narsil-mcp integration is complete.",
                    "Error:".red().bold()
                );
                eprintln!("   Use: ralph verify --mock");
                std::process::exit(1);
            }

            // Create verifier
            let config = VerificationConfig {
                mock_mode: true,
                ..Default::default()
            };
            let verifier = MockCcgVerifier::new(config);

            // Run verification
            let report = verifier.verify_changes(project_path.to_str().unwrap_or("."))?;

            // Format output
            let output_content = if json {
                report.to_json()?
            } else if markdown {
                format_verification_report_markdown(&report)
            } else {
                format_verification_report_human(&report)
            };

            // Write to file or stdout
            if let Some(output_path) = output {
                std::fs::write(&output_path, &output_content)?;
                println!(
                    "{} Report written to: {}",
                    "OK".green().bold(),
                    output_path.display()
                );
            } else {
                println!("{}", output_content);
            }
        }

        Commands::Status { json } => {
            use ralph::stagnation::StatsPersistence;

            // Load predictor stats
            let stats_persistence = StatsPersistence::new(&project_path);
            let predictor_stats = stats_persistence.load_or_default()?;

            // Load session state if available
            let ralph_dir = project_path.join(".ralph");
            let session_persistence = session::persistence::SessionPersistence::new(&ralph_dir);
            let session_state = session_persistence.load().ok().flatten();

            // Get recent commits
            let recent_commits: Vec<String> = std::process::Command::new("git")
                .args(["log", "--oneline", "-5"])
                .current_dir(&project_path)
                .output()
                .ok()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

            if json {
                // JSON output
                let output = serde_json::json!({
                    "predictor": {
                        "total_predictions": predictor_stats.total_predictions(),
                        "correct_predictions": predictor_stats.correct_predictions(),
                        "accuracy": predictor_stats.accuracy(),
                        "last_updated": predictor_stats.last_updated(),
                    },
                    "session": session_state.as_ref().map(|s| serde_json::json!({
                        "iteration": s.loop_state.as_ref().map(|l| l.iteration),
                        "mode": s.loop_state.as_ref().map(|l| l.mode.to_string()),
                        "stagnation_count": s.loop_state.as_ref().map(|l| l.stagnation_count),
                    })),
                    "recent_commits": recent_commits,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                // Human-readable output
                println!("\n{} Project Status", "Ralph:".cyan().bold());
                println!("{}", "─".repeat(60));

                // Predictor stats section
                println!("\n{}", "Predictor Statistics:".bold());
                if predictor_stats.total_predictions() > 0 {
                    println!("   {}", predictor_stats.summary());
                    println!(
                        "   Last updated: {}",
                        predictor_stats
                            .last_updated()
                            .format("%Y-%m-%d %H:%M:%S UTC")
                    );
                } else {
                    println!("   No predictions recorded yet");
                }

                // Session state section
                println!("\n{}", "Session State:".bold());
                if let Some(ref state) = session_state {
                    if let Some(ref loop_state) = state.loop_state {
                        println!("   Iteration: {}", loop_state.iteration);
                        println!("   Mode: {}", loop_state.mode);
                        println!("   Stagnation count: {}", loop_state.stagnation_count);
                    } else {
                        println!("   No active loop state");
                    }
                } else {
                    println!("   No session state found");
                }

                // Recent commits section
                println!("\n{}", "Recent Commits:".bold());
                if recent_commits.is_empty() {
                    println!("   No commits found");
                } else {
                    for commit in &recent_commits {
                        println!("   {}", commit);
                    }
                }

                println!();
            }
        }
    }

    Ok(())
}

/// Format audit entry details for table display.
fn format_audit_details(data: &serde_json::Value, event_type: &ralph::AuditEventType) -> String {
    match event_type {
        ralph::AuditEventType::CommandExecution => {
            let command = data.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let exit_code = data.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
            let status = if exit_code == 0 { "✓" } else { "✗" };
            format!(
                "{} {} (exit: {})",
                status,
                truncate_str(command, 50),
                exit_code
            )
        }
        ralph::AuditEventType::GateResult => {
            let gate_name = data
                .get("gate_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let passed = data
                .get("passed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let status = if passed { "✓ passed" } else { "✗ failed" };
            format!("{}: {}", gate_name, status)
        }
        ralph::AuditEventType::Commit => {
            let hash = data
                .get("commit_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let message = data.get("message").and_then(|v| v.as_str()).unwrap_or("");
            format!(
                "{} - {}",
                &hash[..7.min(hash.len())],
                truncate_str(message, 40)
            )
        }
        ralph::AuditEventType::SessionStart | ralph::AuditEventType::SessionEnd => {
            "session boundary".to_string()
        }
        ralph::AuditEventType::CheckpointCreated => "checkpoint created".to_string(),
        ralph::AuditEventType::Rollback => "rollback performed".to_string(),
        ralph::AuditEventType::ConfigChange => {
            let setting = data.get("setting").and_then(|v| v.as_str()).unwrap_or("?");
            format!("changed: {}", setting)
        }
    }
}

/// Truncate a string to a maximum length with ellipsis.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Format verification report as human-readable text.
fn format_verification_report_human(report: &ralph::VerificationReport) -> String {
    use colored::Colorize;

    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "\n{} Verification Report\n",
        "Verify:".cyan().bold()
    ));
    output.push_str(&"─".repeat(60));
    output.push('\n');

    // Summary
    let status = if report.quality_improved {
        "✓ Quality improved".green().to_string()
    } else {
        "✗ Quality regression".red().to_string()
    };
    output.push_str(&format!("   Status: {}\n", status));
    output.push_str(&format!("   Summary: {}\n", report.summary));
    output.push_str(&format!(
        "   Improvement score: {:.0}%\n",
        report.improvement_score * 100.0
    ));
    output.push_str(&format!(
        "   Verified at: {}\n",
        report.verified_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Quality deltas
    if !report.deltas.is_empty() {
        output.push('\n');
        output.push_str("   Quality Deltas:\n");
        for delta in &report.deltas {
            let direction = if delta.is_improvement() { "↓" } else { "↑" };
            let color_direction = if delta.is_improvement() {
                direction.green().to_string()
            } else {
                direction.red().to_string()
            };
            output.push_str(&format!(
                "     {} {}: {:.1} → {:.1} ({:+.1}%)\n",
                color_direction,
                delta.metric,
                delta.before,
                delta.after,
                delta.improvement_percent()
            ));
        }
    }

    // Findings
    if !report.findings.is_empty() {
        output.push('\n');
        output.push_str("   Findings:\n");
        for finding in &report.findings {
            let severity_str = match finding.severity {
                ralph::VerificationSeverity::Info => "INFO".blue().to_string(),
                ralph::VerificationSeverity::Warning => "WARN".yellow().to_string(),
                ralph::VerificationSeverity::Error => "ERROR".red().to_string(),
                ralph::VerificationSeverity::Critical => "CRIT".red().bold().to_string(),
            };
            output.push_str(&format!(
                "     [{}] {}: {}\n",
                severity_str, finding.category, finding.message
            ));
        }
    }

    // Metadata
    if !report.metadata.is_empty() {
        output.push('\n');
        output.push_str("   Metadata:\n");
        for (key, value) in &report.metadata {
            output.push_str(&format!("     {}: {}\n", key, value));
        }
    }

    output.push_str(&"─".repeat(60));
    output.push('\n');

    output
}

/// Format verification report as Markdown.
fn format_verification_report_markdown(report: &ralph::VerificationReport) -> String {
    let mut output = String::new();

    // Header
    output.push_str("# Verification Report\n\n");

    // Summary
    let status = if report.quality_improved {
        "✅ Quality improved"
    } else {
        "❌ Quality regression"
    };
    output.push_str(&format!("**Status:** {}\n\n", status));
    output.push_str(&format!("**Summary:** {}\n\n", report.summary));
    output.push_str(&format!(
        "**Improvement Score:** {:.0}%\n\n",
        report.improvement_score * 100.0
    ));
    output.push_str(&format!(
        "**Verified at:** {}\n\n",
        report.verified_at.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Quality deltas
    if !report.deltas.is_empty() {
        output.push_str("## Quality Deltas\n\n");
        output.push_str("| Metric | Before | After | Change |\n");
        output.push_str("|--------|--------|-------|--------|\n");
        for delta in &report.deltas {
            let direction = if delta.is_improvement() { "↓" } else { "↑" };
            output.push_str(&format!(
                "| {} | {:.1} | {:.1} | {} {:+.1}% |\n",
                delta.metric,
                delta.before,
                delta.after,
                direction,
                delta.improvement_percent()
            ));
        }
        output.push('\n');
    }

    // Findings
    if !report.findings.is_empty() {
        output.push_str("## Findings\n\n");
        for finding in &report.findings {
            let severity_emoji = match finding.severity {
                ralph::VerificationSeverity::Info => "ℹ️",
                ralph::VerificationSeverity::Warning => "⚠️",
                ralph::VerificationSeverity::Error => "❌",
                ralph::VerificationSeverity::Critical => "🚨",
            };
            output.push_str(&format!(
                "- {} **{}**: {}\n",
                severity_emoji, finding.category, finding.message
            ));
        }
        output.push('\n');
    }

    // Metadata
    if !report.metadata.is_empty() {
        output.push_str("## Metadata\n\n");
        for (key, value) in &report.metadata {
            output.push_str(&format!("- **{}**: {}\n", key, value));
        }
    }

    output
}

/// Open a file in the system's default browser.
///
/// Uses platform-specific commands:
/// - macOS: `open`
/// - Linux: `xdg-open`
/// - Windows: `start`
///
/// Returns Ok(()) if the command was spawned successfully, Err otherwise.
fn open_in_browser(path: &std::path::Path) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()?;
    }

    Ok(())
}
