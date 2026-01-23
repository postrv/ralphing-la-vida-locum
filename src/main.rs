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
    Validate,

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
        } => {
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

            let mut loop_config = LoopManagerConfig::new(project_path, config)
                .with_mode(mode)
                .with_max_iterations(max_iterations)
                .with_stagnation_threshold(stagnation_threshold)
                .with_doc_sync_interval(doc_sync_interval)
                .with_verbose(cli.verbose);

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

            manager.run().await?;
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
            let analytics = Analytics::new(project_path);

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

                ConfigAction::Validate => {
                    let settings_path = ProjectConfig::settings_path(&project_path);
                    let claude_md_path = ProjectConfig::claude_md_path(&project_path);

                    let mut valid = true;

                    // Check settings.json
                    if settings_path.exists() {
                        match ProjectConfig::load(&project_path) {
                            Ok(_) => println!("{} settings.json is valid", "OK".green()),
                            Err(e) => {
                                eprintln!("{} settings.json: {}", "Error:".red(), e);
                                valid = false;
                            }
                        }
                    } else {
                        println!(
                            "{} settings.json not found (using defaults)",
                            "Info:".blue()
                        );
                    }

                    // Check CLAUDE.md
                    if claude_md_path.exists() {
                        println!("{} CLAUDE.md exists", "OK".green());
                    } else {
                        println!("{} CLAUDE.md not found", "Warning:".yellow());
                    }

                    // Check MCP config
                    let mcp_path = project_path.join(".claude/mcp.json");
                    if mcp_path.exists() {
                        match std::fs::read_to_string(&mcp_path) {
                            Ok(content) => {
                                match serde_json::from_str::<serde_json::Value>(&content) {
                                    Ok(_) => println!("{} mcp.json is valid", "OK".green()),
                                    Err(e) => {
                                        eprintln!("{} mcp.json: {}", "Error:".red(), e);
                                        valid = false;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("{} Cannot read mcp.json: {}", "Error:".red(), e);
                                valid = false;
                            }
                        }
                    } else {
                        println!("{} mcp.json not found", "Info:".blue());
                    }

                    if !valid {
                        std::process::exit(1);
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
    }

    Ok(())
}
