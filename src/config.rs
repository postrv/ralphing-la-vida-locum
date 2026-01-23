//! Configuration management for Ralph automation suite.

use crate::analytics::AnalyticsUploadConfig;
use crate::llm::LlmConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Default directories to ignore during file traversal
pub fn default_ignore_dirs() -> HashSet<&'static str> {
    [
        "node_modules",
        ".next",
        "_next",
        "target",
        ".venv",
        ".env",
        "__pycache__",
        "dist",
        "build",
        "out",
        "lib",
        "bin",
        "obj",
        "vendor",
        ".git",
        ".hg",
        ".svn",
        ".backup",
        ".archive",
        "_archive",
        "archive",
        ".turbo",
        "playwright-report",
        "test-results",
        ".ralph",
        ".claude",
        "coverage",
        ".nyc_output",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        "htmlcov",
        ".tox",
        "eggs",
        ".eggs",
        ".cowork",
    ]
    .into_iter()
    .collect()
}

/// Default files to ignore
pub fn default_ignore_files() -> HashSet<&'static str> {
    [
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "Cargo.lock",
        "poetry.lock",
        "Gemfile.lock",
        "composer.lock",
        ".DS_Store",
        "thumbs.db",
    ]
    .into_iter()
    .collect()
}

/// File extension categories
pub mod extensions {
    pub const CODE: &[&str] = &[
        ".rs", ".py", ".go", ".c", ".cpp", ".h", ".hpp", ".swift", ".kt", ".java", ".ts", ".tsx",
        ".js", ".jsx", ".mjs", ".cjs", ".vue", ".svelte", ".rb", ".php", ".cs", ".fs", ".ex",
        ".exs", ".erl", ".hs", ".ml", ".sql", ".surql", ".graphql", ".proto",
    ];

    pub const CONFIG: &[&str] = &[
        ".toml",
        ".yaml",
        ".yml",
        ".json",
        ".ini",
        ".cfg",
        ".conf",
        ".env.example",
        ".tf",
        ".tfvars",
        ".hcl",
    ];

    pub const DOCS: &[&str] = &[".md", ".mdx", ".txt", ".rst", ".adoc"];

    pub const WEB: &[&str] = &[".html", ".css", ".scss", ".sass", ".less"];

    pub const SHADER: &[&str] = &[".glsl", ".wgsl", ".hlsl", ".vert", ".frag", ".comp"];

    pub const SCRIPT: &[&str] = &[".sh", ".bash", ".zsh", ".fish", ".ps1", ".bat", ".cmd"];

    /// Get all extensions
    pub fn all() -> Vec<&'static str> {
        let mut all = Vec::new();
        all.extend_from_slice(CODE);
        all.extend_from_slice(CONFIG);
        all.extend_from_slice(DOCS);
        all.extend_from_slice(WEB);
        all.extend_from_slice(SHADER);
        all.extend_from_slice(SCRIPT);
        all
    }
}

/// Configuration for stagnation predictor risk weights (Phase 10.3).
///
/// This configuration allows customizing the risk weights used by the
/// stagnation predictor. You can either specify a preset profile or
/// provide custom weight values.
///
/// # Example settings.json
///
/// Using a preset:
/// ```json
/// {
///   "predictorWeights": {
///     "preset": "conservative"
///   }
/// }
/// ```
///
/// Using custom weights:
/// ```json
/// {
///   "predictorWeights": {
///     "commit_gap": 0.30,
///     "file_churn": 0.20,
///     "error_repeat": 0.20,
///     "test_stagnation": 0.15,
///     "mode_oscillation": 0.10,
///     "warning_growth": 0.05
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorWeightsConfig {
    /// Named preset to use: "balanced", "conservative", or "aggressive".
    ///
    /// If specified, this takes precedence over individual weight values.
    #[serde(default)]
    pub preset: Option<String>,

    /// Weight for iterations since last commit (default: 0.25).
    #[serde(default = "default_commit_gap")]
    pub commit_gap: f64,

    /// Weight for repeated file edits (default: 0.20).
    #[serde(default = "default_file_churn")]
    pub file_churn: f64,

    /// Weight for error repetition (default: 0.20).
    #[serde(default = "default_error_repeat")]
    pub error_repeat: f64,

    /// Weight for test count stagnation (default: 0.15).
    #[serde(default = "default_test_stagnation")]
    pub test_stagnation: f64,

    /// Weight for mode oscillation (default: 0.10).
    #[serde(default = "default_mode_oscillation")]
    pub mode_oscillation: f64,

    /// Weight for clippy warning growth (default: 0.10).
    #[serde(default = "default_warning_growth")]
    pub warning_growth: f64,
}

fn default_commit_gap() -> f64 {
    0.25
}

fn default_file_churn() -> f64 {
    0.20
}

fn default_error_repeat() -> f64 {
    0.20
}

fn default_test_stagnation() -> f64 {
    0.15
}

fn default_mode_oscillation() -> f64 {
    0.10
}

fn default_warning_growth() -> f64 {
    0.10
}

impl Default for PredictorWeightsConfig {
    fn default() -> Self {
        Self {
            preset: None,
            commit_gap: default_commit_gap(),
            file_churn: default_file_churn(),
            error_repeat: default_error_repeat(),
            test_stagnation: default_test_stagnation(),
            mode_oscillation: default_mode_oscillation(),
            warning_growth: default_warning_growth(),
        }
    }
}

impl PredictorWeightsConfig {
    /// Returns true if this config specifies a preset.
    #[must_use]
    pub fn has_preset(&self) -> bool {
        self.preset.is_some()
    }

    /// Returns the weight values as a tuple.
    ///
    /// Returns (commit_gap, file_churn, error_repeat, test_stagnation, mode_oscillation, warning_growth).
    #[must_use]
    pub fn weight_values(&self) -> (f64, f64, f64, f64, f64, f64) {
        (
            self.commit_gap,
            self.file_churn,
            self.error_repeat,
            self.test_stagnation,
            self.mode_oscillation,
            self.warning_growth,
        )
    }

    /// Validates the weight configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any weight is negative
    /// - Any weight is NaN or infinite
    /// - All weights are zero
    /// - Preset name is invalid
    pub fn validate(&self) -> Result<(), String> {
        // Validate preset name if specified
        if let Some(ref preset) = self.preset {
            match preset.to_lowercase().as_str() {
                "balanced" | "conservative" | "aggressive" => {}
                _ => {
                    return Err(format!(
                        "Invalid predictor weight preset '{}'. Valid options: balanced, conservative, aggressive",
                        preset
                    ));
                }
            }
        }

        // Validate weight values
        let weights = [
            ("commit_gap", self.commit_gap),
            ("file_churn", self.file_churn),
            ("error_repeat", self.error_repeat),
            ("test_stagnation", self.test_stagnation),
            ("mode_oscillation", self.mode_oscillation),
            ("warning_growth", self.warning_growth),
        ];

        for (name, value) in weights {
            if value.is_nan() {
                return Err(format!("{} weight is NaN", name));
            }
            if value.is_infinite() {
                return Err(format!("{} weight is infinite", name));
            }
            if value < 0.0 {
                return Err(format!("{} weight is negative: {}", name, value));
            }
        }

        let total = self.commit_gap
            + self.file_churn
            + self.error_repeat
            + self.test_stagnation
            + self.mode_oscillation
            + self.warning_growth;

        if total == 0.0 && self.preset.is_none() {
            return Err("All weights are zero - at least one must be positive".to_string());
        }

        Ok(())
    }
}

/// Project configuration loaded from .claude/settings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub permissions: PermissionsConfig,

    #[serde(default)]
    pub hooks: HooksConfig,

    #[serde(default = "default_true", rename = "respectGitignore")]
    pub respect_gitignore: bool,

    /// Configuration for weighted gate scoring (Sprint 9.1).
    ///
    /// Controls how gate results are weighted based on whether files of that
    /// language were changed in the current working tree.
    #[serde(default, rename = "gateWeights")]
    pub gate_weights: crate::quality::gates::GateWeightConfig,

    /// Configuration for context window prioritization (Sprint 9.2).
    ///
    /// Controls how files are prioritized for context inclusion based on
    /// language and change status.
    #[serde(default, rename = "contextPriority")]
    pub context_priority: crate::prompt::context_priority::ContextPriorityConfig,

    /// Configuration for stagnation predictor weights (Phase 10.3).
    ///
    /// Controls the risk factor weights used by the stagnation predictor.
    #[serde(default, rename = "predictorWeights")]
    pub predictor_weights: PredictorWeightsConfig,

    /// Configuration for LLM backend (Phase 12.2).
    ///
    /// Controls which LLM model to use and its options.
    #[serde(default)]
    pub llm: LlmConfig,

    /// Configuration for analytics upload (Phase 18.1).
    ///
    /// Controls whether analytics are uploaded to a remote endpoint
    /// and what privacy settings to apply. Disabled by default.
    #[serde(default)]
    pub analytics: AnalyticsUploadConfig,
}

fn default_true() -> bool {
    true
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            permissions: PermissionsConfig::default(),
            hooks: HooksConfig::default(),
            respect_gitignore: true, // Match the serde default
            gate_weights: crate::quality::gates::GateWeightConfig::default(),
            context_priority: crate::prompt::context_priority::ContextPriorityConfig::default(),
            predictor_weights: PredictorWeightsConfig::default(),
            llm: LlmConfig::default(),
            analytics: AnalyticsUploadConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub allow: Vec<String>,

    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    #[serde(rename = "PreToolUse", default)]
    pub pre_tool_use: Vec<HookMatcher>,

    #[serde(rename = "PostToolUse", default)]
    pub post_tool_use: Vec<HookMatcher>,

    #[serde(rename = "Stop", default)]
    pub stop: Vec<HookMatcher>,

    #[serde(rename = "SessionStart", default)]
    pub session_start: Vec<HookMatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HookCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCommand {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(default)]
    pub once: bool,
}

impl ProjectConfig {
    /// Load configuration from a project directory
    pub fn load(project_dir: &Path) -> anyhow::Result<Self> {
        let settings_path = Self::settings_path(project_dir);

        if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)?;
            let config: ProjectConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Get the settings.json path for a project
    pub fn settings_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".claude/settings.json")
    }

    /// Get the CLAUDE.md path for a project
    pub fn claude_md_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".claude/CLAUDE.md")
    }

    /// Get the analytics directory
    pub fn analytics_dir(project_dir: &Path) -> PathBuf {
        project_dir.join(".ralph")
    }

    /// Get the archive directory
    pub fn archive_dir(project_dir: &Path) -> PathBuf {
        project_dir.join(".archive")
    }

    /// Get the analysis directory
    pub fn analysis_dir(project_dir: &Path) -> PathBuf {
        project_dir.join(".ralph/analysis")
    }

    /// Check if a command is allowed by the permissions config
    pub fn is_command_allowed(&self, command: &str) -> bool {
        // Check deny list first
        for pattern in &self.permissions.deny {
            if Self::matches_permission_pattern(pattern, command) {
                return false;
            }
        }

        // If allow list is empty, allow by default
        if self.permissions.allow.is_empty() {
            return true;
        }

        // Check allow list
        for pattern in &self.permissions.allow {
            if Self::matches_permission_pattern(pattern, command) {
                return true;
            }
        }

        false
    }

    /// Check if a command matches a permission pattern
    fn matches_permission_pattern(pattern: &str, command: &str) -> bool {
        // Handle Bash(*) patterns
        if let Some(bash_pattern) = pattern
            .strip_prefix("Bash(")
            .and_then(|s| s.strip_suffix(")"))
        {
            // Simple glob matching
            if bash_pattern == "*" {
                return true;
            }
            if let Some(prefix) = bash_pattern.strip_suffix('*') {
                return command.starts_with(prefix);
            }
            if let Some(suffix) = bash_pattern.strip_prefix('*') {
                return command.ends_with(suffix);
            }
            return command == bash_pattern || command.starts_with(&format!("{} ", bash_pattern));
        }

        // Direct match
        pattern == command
    }
}

/// Dangerous command patterns to block
pub const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf /*",
    ":(){:|:&};:",
    "dd if=/dev/zero",
    "mkfs.",
    "> /dev/sd",
    "chmod 777",
    "chmod -R 777",
    "sudo rm",
    "sudo dd",
];

/// Secret patterns to detect in code
pub const SECRET_PATTERNS: &[&str] = &[
    r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*['"][^'"]+['"]"#,
    r#"(?i)(password|passwd|pwd)\s*[:=]\s*['"][^'"]+['"]"#,
    r#"(?i)(secret|token)\s*[:=]\s*['"][^'"]+['"]"#,
    r"(?i)(aws[_-]?access[_-]?key|aws[_-]?secret)",
    r"(?i)(private[_-]?key)\s*[:=]",
    r"-----BEGIN (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----",
];

// ============================================================================
// SSH Blocking Patterns
// ============================================================================

/// SSH-related patterns that should be blocked to enforce gh CLI usage
pub const SSH_BLOCKED_PATTERNS: &[&str] = &[
    // SSH key generation/management
    "ssh-keygen",
    "ssh-add",
    "ssh-agent",
    "eval $(ssh-agent",
    "ssh-agent -s",
    // SSH key file access
    "cat ~/.ssh",
    "cat /home/*/.ssh",
    "ls ~/.ssh",
    "ls /home/*/.ssh",
    "cat *id_rsa*",
    "cat *id_ed25519*",
    "cat *id_ecdsa*",
    "cat *id_dsa*",
    "cat *known_hosts*",
    "cat *authorized_keys*",
    // SSH directory operations
    "~/.ssh/",
    "/home/*/.ssh/",
    ".ssh/id_",
    ".ssh/config",
    // SSH connection attempts that should use gh
    "git@github.com:",
    "git clone git@",
    "git remote add * git@",
    "git remote set-url * git@",
    // Copying SSH keys
    "cp *id_rsa*",
    "cp *id_ed25519*",
    "scp *id_rsa*",
    "scp *id_ed25519*",
];

/// Check if a command attempts SSH operations
pub fn is_ssh_command(command: &str) -> bool {
    SSH_BLOCKED_PATTERNS.iter().any(|pattern| {
        // Handle glob patterns with * - simplified matching
        if pattern.contains('*') {
            // For patterns like "git remote add * git@", check if command contains both parts
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                if !prefix.is_empty() && !suffix.is_empty() {
                    // Both prefix and suffix exist
                    if let Some(prefix_pos) = command.find(prefix) {
                        let rest = &command[prefix_pos + prefix.len()..];
                        return rest.contains(suffix);
                    }
                    return false;
                }
            }
        }
        command.contains(pattern)
    })
}

/// Get alternative gh command for blocked SSH operation
pub fn suggest_gh_alternative(command: &str) -> Option<String> {
    // git@github.com:user/repo.git -> gh repo clone user/repo
    if command.contains("git clone git@github.com:") {
        if let Some(repo_part) = command.split("git@github.com:").nth(1) {
            let repo = repo_part.trim_end_matches(".git").trim();
            return Some(format!("gh repo clone {}", repo));
        }
    }

    // git remote add origin git@... -> gh repo set-default
    if command.contains("git remote") && command.contains("git@github.com:") {
        return Some("Use 'gh repo set-default' instead of git remote with SSH URL".to_string());
    }

    // ssh-keygen suggestions
    if command.contains("ssh-keygen") || command.contains("ssh-add") {
        return Some(
            "gh CLI handles authentication - no SSH keys needed. Run 'gh auth login'".to_string(),
        );
    }

    None
}

// ============================================================================
// Stagnation Configuration
// ============================================================================

/// Stagnation severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagnationLevel {
    /// Normal operation
    None,
    /// Minor stagnation - switch to debug mode (threshold reached)
    Warning,
    /// Significant stagnation - invoke supervisor (threshold * 2)
    Elevated,
    /// Critical - abort with diagnostic dump (threshold * 3)
    Critical,
}

impl StagnationLevel {
    /// Calculate stagnation level from count and threshold
    pub fn from_count(count: u32, threshold: u32) -> Self {
        if threshold == 0 {
            return StagnationLevel::None;
        }

        match count {
            c if c >= threshold * 3 => StagnationLevel::Critical,
            c if c >= threshold * 2 => StagnationLevel::Elevated,
            c if c >= threshold => StagnationLevel::Warning,
            _ => StagnationLevel::None,
        }
    }

    /// Check if this level should trigger supervisor
    pub fn should_invoke_supervisor(&self) -> bool {
        matches!(self, StagnationLevel::Elevated | StagnationLevel::Critical)
    }

    /// Check if this level should abort
    pub fn should_abort(&self) -> bool {
        matches!(self, StagnationLevel::Critical)
    }
}

impl std::fmt::Display for StagnationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "NONE"),
            Self::Warning => write!(f, "WARNING"),
            Self::Elevated => write!(f, "ELEVATED"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

// ============================================================================
// Git/GitHub Verification
// ============================================================================

use std::process::Command;

/// Result of git environment verification
#[derive(Debug)]
pub struct GitEnvironmentCheck {
    /// gh CLI is installed
    pub gh_installed: bool,
    /// gh CLI is authenticated
    pub gh_authenticated: bool,
    /// git is installed
    pub git_installed: bool,
    /// Current git user
    pub git_user: Option<String>,
    /// Current git email
    pub git_email: Option<String>,
    /// SSH agent running (warning if true - should use gh)
    pub ssh_agent_running: bool,
    /// Errors encountered
    pub errors: Vec<String>,
    /// Warnings
    pub warnings: Vec<String>,
}

impl GitEnvironmentCheck {
    /// Check if environment is ready
    pub fn is_ready(&self) -> bool {
        self.gh_installed && self.gh_authenticated && self.git_installed
    }
}

/// Verify the git environment is properly configured for Ralph
pub fn verify_git_environment() -> GitEnvironmentCheck {
    let mut check = GitEnvironmentCheck {
        gh_installed: false,
        gh_authenticated: false,
        git_installed: false,
        git_user: None,
        git_email: None,
        ssh_agent_running: false,
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    // Check git installed
    match Command::new("git").args(["--version"]).output() {
        Ok(output) if output.status.success() => {
            check.git_installed = true;
        }
        _ => {
            check.errors.push("git not installed".to_string());
        }
    }

    // Get git user config
    if let Ok(output) = Command::new("git").args(["config", "user.name"]).output() {
        if output.status.success() {
            check.git_user = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    if let Ok(output) = Command::new("git").args(["config", "user.email"]).output() {
        if output.status.success() {
            check.git_email = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    // Check gh CLI installed
    match Command::new("gh").args(["--version"]).output() {
        Ok(output) if output.status.success() => {
            check.gh_installed = true;
        }
        _ => {
            check
                .errors
                .push("gh CLI not installed - required for GitHub operations".to_string());
        }
    }

    // Check gh CLI authenticated
    if check.gh_installed {
        match Command::new("gh").args(["auth", "status"]).output() {
            Ok(output) if output.status.success() => {
                check.gh_authenticated = true;
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                check.errors.push(format!(
                    "gh CLI not authenticated - run 'gh auth login': {}",
                    stderr.trim()
                ));
            }
            Err(e) => {
                check
                    .errors
                    .push(format!("Failed to check gh auth status: {}", e));
            }
        }
    }

    // Check if SSH agent is running (warning - should use gh instead)
    if std::env::var("SSH_AUTH_SOCK").is_ok() {
        check.ssh_agent_running = true;
        check
            .warnings
            .push("SSH agent detected - Ralph prefers gh CLI for GitHub operations".to_string());
    }

    check
}

// ============================================================================
// Configuration Inheritance (Phase 17.1)
// ============================================================================

/// Configuration level in the inheritance hierarchy.
///
/// Lower levels are overridden by higher levels:
/// System < User < Project
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigLevel {
    /// System-wide configuration (lowest priority)
    System,
    /// User-specific configuration
    User,
    /// Project-specific configuration (highest priority)
    Project,
}

impl std::fmt::Display for ConfigLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
        }
    }
}

/// A source in the configuration inheritance chain.
#[derive(Debug, Clone)]
pub struct ConfigSource {
    /// The level of this config source
    pub level: ConfigLevel,
    /// Path to the config file
    pub path: PathBuf,
    /// Whether the config was successfully loaded
    pub loaded: bool,
}

/// The full inheritance chain showing which configs were loaded.
#[derive(Debug, Clone, Default)]
pub struct InheritanceChain {
    /// All config sources in order (system, user, project)
    pub sources: Vec<ConfigSource>,
}

impl InheritanceChain {
    /// Create a new empty inheritance chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source to the chain.
    pub fn add_source(&mut self, level: ConfigLevel, path: PathBuf, loaded: bool) {
        self.sources.push(ConfigSource {
            level,
            path,
            loaded,
        });
    }

    /// Get a formatted description of the inheritance chain for logging.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut lines = vec!["Configuration inheritance chain:".to_string()];
        for source in &self.sources {
            let status = if source.loaded { "✓" } else { "✗" };
            lines.push(format!(
                "  {} [{}] {}",
                status,
                source.level,
                source.path.display()
            ));
        }
        lines.join("\n")
    }
}

/// Strategy for merging arrays during config inheritance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrayMergeStrategy {
    /// Replace arrays entirely (child replaces parent)
    #[default]
    Replace,
    /// Merge arrays (combine parent and child, deduplicating)
    Merge,
}

/// Default config file locations for different platforms.
#[derive(Debug, Clone)]
pub struct ConfigLocations {
    system: Option<PathBuf>,
    user: Option<PathBuf>,
}

impl Default for ConfigLocations {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLocations {
    /// Create default config locations for the current platform.
    #[must_use]
    pub fn new() -> Self {
        let system = Self::default_system_path();
        let user = Self::default_user_path();
        Self { system, user }
    }

    /// Get the default system config path.
    #[must_use]
    pub fn default_system_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("PROGRAMDATA")
                .ok()
                .map(|p| PathBuf::from(p).join("ralph").join("config.json"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            Some(PathBuf::from("/etc/ralph/config.json"))
        }
    }

    /// Get the default user config path.
    #[must_use]
    pub fn default_user_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("ralph").join("config.json"))
    }

    /// Get the system config path.
    #[must_use]
    pub fn system_path(&self) -> Option<&PathBuf> {
        self.system.as_ref()
    }

    /// Get the user config path.
    #[must_use]
    pub fn user_path(&self) -> Option<&PathBuf> {
        self.user.as_ref()
    }
}

/// Configuration loader with inheritance support.
///
/// Loads configuration from system, user, and project levels,
/// merging them according to the inheritance hierarchy.
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    system_config_path: Option<PathBuf>,
    user_config_path: Option<PathBuf>,
    array_merge_strategy: ArrayMergeStrategy,
    verbose: bool,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    /// Create a new config loader with default paths.
    #[must_use]
    pub fn new() -> Self {
        let locations = ConfigLocations::new();
        Self {
            system_config_path: locations.system,
            user_config_path: locations.user,
            array_merge_strategy: ArrayMergeStrategy::default(),
            verbose: false,
        }
    }

    /// Set a custom system config path.
    #[must_use]
    pub fn with_system_config_path(mut self, path: PathBuf) -> Self {
        self.system_config_path = Some(path);
        self
    }

    /// Set a custom user config path.
    #[must_use]
    pub fn with_user_config_path(mut self, path: PathBuf) -> Self {
        self.user_config_path = Some(path);
        self
    }

    /// Set the array merge strategy.
    #[must_use]
    pub fn with_array_merge_strategy(mut self, strategy: ArrayMergeStrategy) -> Self {
        self.array_merge_strategy = strategy;
        self
    }

    /// Enable or disable verbose logging.
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Load configuration with inheritance from the given project directory.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing a config file fails. Missing config files
    /// are silently ignored.
    pub fn load(&self, project_dir: &Path) -> anyhow::Result<ProjectConfig> {
        let (config, chain) = self.load_with_chain(project_dir)?;
        if self.verbose {
            eprintln!("{}", chain.describe());
        }
        Ok(config)
    }

    /// Load configuration and return the inheritance chain.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing a config file fails.
    pub fn load_with_chain(
        &self,
        project_dir: &Path,
    ) -> anyhow::Result<(ProjectConfig, InheritanceChain)> {
        let mut chain = InheritanceChain::new();
        let mut merged = serde_json::Value::Object(serde_json::Map::new());

        // Load system config
        if let Some(ref system_path) = self.system_config_path {
            let loaded = self.load_and_merge(&mut merged, system_path)?;
            chain.add_source(ConfigLevel::System, system_path.clone(), loaded);
        }

        // Load user config
        if let Some(ref user_path) = self.user_config_path {
            let loaded = self.load_and_merge(&mut merged, user_path)?;
            chain.add_source(ConfigLevel::User, user_path.clone(), loaded);
        }

        // Load project config
        let project_path = ProjectConfig::settings_path(project_dir);
        let loaded = self.load_and_merge(&mut merged, &project_path)?;
        chain.add_source(ConfigLevel::Project, project_path, loaded);

        // Parse the merged config
        let config: ProjectConfig = serde_json::from_value(merged)?;

        Ok((config, chain))
    }

    /// Load a config file and merge it into the accumulated config.
    ///
    /// Returns true if the file was loaded, false if it doesn't exist.
    fn load_and_merge(
        &self,
        accumulated: &mut serde_json::Value,
        path: &Path,
    ) -> anyhow::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        let content = std::fs::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;

        self.deep_merge(accumulated, value);
        Ok(true)
    }

    /// Deep merge two JSON values, with child overriding parent.
    fn deep_merge(&self, parent: &mut serde_json::Value, child: serde_json::Value) {
        match (parent, child) {
            (serde_json::Value::Object(parent_map), serde_json::Value::Object(child_map)) => {
                for (key, child_value) in child_map {
                    match parent_map.get_mut(&key) {
                        Some(parent_value) => {
                            self.deep_merge(parent_value, child_value);
                        }
                        None => {
                            parent_map.insert(key, child_value);
                        }
                    }
                }
            }
            (
                serde_json::Value::Array(parent_arr),
                serde_json::Value::Array(child_arr),
            ) => {
                match self.array_merge_strategy {
                    ArrayMergeStrategy::Replace => {
                        *parent_arr = child_arr;
                    }
                    ArrayMergeStrategy::Merge => {
                        // Add child elements that aren't in parent
                        for child_elem in child_arr {
                            if !parent_arr.contains(&child_elem) {
                                parent_arr.push(child_elem);
                            }
                        }
                    }
                }
            }
            (parent, child) => {
                *parent = child;
            }
        }
    }
}

// ============================================================================
// Shared Gate Configuration (Phase 17.2)
// ============================================================================

/// Configuration with optional `extends` field for shared configs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtendableConfig {
    /// Path to a config file to extend from (relative to project root).
    ///
    /// URLs are reserved for future cloud support but not currently implemented.
    #[serde(default)]
    pub extends: Option<String>,

    /// All other configuration fields.
    #[serde(flatten)]
    pub config: serde_json::Value,
}

/// Error type for shared config operations.
#[derive(Debug)]
pub enum SharedConfigError {
    /// The specified config file was not found.
    NotFound { path: PathBuf },
    /// URL extends is not yet supported.
    UrlNotSupported { url: String },
    /// Circular extends detected.
    CircularExtends { cycle: Vec<PathBuf> },
    /// Failed to parse config file.
    ParseError { path: PathBuf, error: String },
    /// Config validation failed.
    ValidationError { path: PathBuf, error: String },
    /// IO error reading config.
    IoError { path: PathBuf, error: std::io::Error },
}

impl std::fmt::Display for SharedConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { path } => {
                write!(f, "Config file not found: {} does not exist", path.display())
            }
            Self::UrlNotSupported { url } => {
                write!(
                    f,
                    "URL extends not yet supported (cloud feature coming soon): {}",
                    url
                )
            }
            Self::CircularExtends { cycle } => {
                let paths: Vec<String> = cycle.iter().map(|p| p.display().to_string()).collect();
                write!(f, "Circular extends detected: {}", paths.join(" -> "))
            }
            Self::ParseError { path, error } => {
                write!(f, "Failed to parse config {}: {}", path.display(), error)
            }
            Self::ValidationError { path, error } => {
                write!(
                    f,
                    "Validation error in config {}: {}",
                    path.display(),
                    error
                )
            }
            Self::IoError { path, error } => {
                write!(f, "Failed to read config {}: {}", path.display(), error)
            }
        }
    }
}

impl std::error::Error for SharedConfigError {}

/// Resolver for shared gate configurations with extends support.
///
/// This resolver handles loading configurations that can extend other
/// configuration files, allowing teams to share common settings.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::config::SharedConfigResolver;
/// use std::path::Path;
///
/// let resolver = SharedConfigResolver::new(Path::new("/path/to/project"));
/// let config = resolver.load()?;
/// ```
///
/// # Config Format
///
/// Configs can reference other configs via the `extends` field:
///
/// ```json
/// {
///   "extends": "config/team-defaults.json",
///   "gateWeights": {
///     "unchanged_weight": 0.5
///   }
/// }
/// ```
///
/// The extended config's values are merged with the current config,
/// with current config values taking precedence.
#[derive(Debug, Clone)]
pub struct SharedConfigResolver {
    project_dir: PathBuf,
    max_depth: usize,
}

impl SharedConfigResolver {
    /// Create a new resolver for the given project directory.
    #[must_use]
    pub fn new(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_path_buf(),
            max_depth: 10, // Prevent runaway recursion
        }
    }

    /// Set the maximum extends depth (default: 10).
    #[must_use]
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Load the project configuration with extends resolution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - An extended config file is not found
    /// - A URL extends is used (not yet supported)
    /// - Circular extends are detected
    /// - A config file fails to parse
    pub fn load(&self) -> Result<ProjectConfig, SharedConfigError> {
        let settings_path = ProjectConfig::settings_path(&self.project_dir);

        if !settings_path.exists() {
            // No project settings, return defaults
            return Ok(ProjectConfig::default());
        }

        // Track visited paths for circular detection
        let mut visited = std::collections::HashSet::new();

        // Load and merge the config chain
        let merged = self.load_with_extends(&settings_path, &mut visited, 0)?;

        // Parse the final merged config
        serde_json::from_value(merged).map_err(|e| SharedConfigError::ParseError {
            path: settings_path,
            error: e.to_string(),
        })
    }

    /// Validate the configuration including all extended configs.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails for any config in the chain.
    pub fn validate(&self) -> Result<(), SharedConfigError> {
        let config = self.load()?;

        // Validate predictor weights
        if let Err(e) = config.predictor_weights.validate() {
            return Err(SharedConfigError::ValidationError {
                path: ProjectConfig::settings_path(&self.project_dir),
                error: e,
            });
        }

        Ok(())
    }

    /// Load a config file and recursively resolve extends.
    fn load_with_extends(
        &self,
        config_path: &Path,
        visited: &mut std::collections::HashSet<PathBuf>,
        depth: usize,
    ) -> Result<serde_json::Value, SharedConfigError> {
        // Check recursion depth
        if depth > self.max_depth {
            return Err(SharedConfigError::CircularExtends {
                cycle: visited.iter().cloned().collect(),
            });
        }

        // Canonicalize path for consistent comparison
        let canonical_path = config_path
            .canonicalize()
            .unwrap_or_else(|_| config_path.to_path_buf());

        // Check for circular reference
        if visited.contains(&canonical_path) {
            let mut cycle: Vec<PathBuf> = visited.iter().cloned().collect();
            cycle.push(canonical_path);
            return Err(SharedConfigError::CircularExtends { cycle });
        }

        // Check if file exists
        if !config_path.exists() {
            return Err(SharedConfigError::NotFound {
                path: config_path.to_path_buf(),
            });
        }

        // Mark as visited
        visited.insert(canonical_path.clone());

        // Read and parse the config
        let content =
            std::fs::read_to_string(config_path).map_err(|e| SharedConfigError::IoError {
                path: config_path.to_path_buf(),
                error: e,
            })?;

        let extendable: ExtendableConfig =
            serde_json::from_str(&content).map_err(|e| SharedConfigError::ParseError {
                path: config_path.to_path_buf(),
                error: e.to_string(),
            })?;

        // Start with the base config (if extends is specified)
        let mut merged = if let Some(ref extends_path) = extendable.extends {
            // Check if it's a URL
            if extends_path.starts_with("http://") || extends_path.starts_with("https://") {
                return Err(SharedConfigError::UrlNotSupported {
                    url: extends_path.clone(),
                });
            }

            // Resolve path relative to project root
            let resolved_path = self.project_dir.join(extends_path);
            self.load_with_extends(&resolved_path, visited, depth + 1)?
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };

        // Merge current config on top of base
        self.deep_merge(&mut merged, extendable.config);

        // Unmark as visited (for non-circular paths that converge)
        visited.remove(&canonical_path);

        Ok(merged)
    }

    /// Deep merge two JSON values, with child overriding parent.
    fn deep_merge(&self, parent: &mut serde_json::Value, child: serde_json::Value) {
        match (parent, child) {
            (serde_json::Value::Object(parent_map), serde_json::Value::Object(child_map)) => {
                for (key, child_value) in child_map {
                    match parent_map.get_mut(&key) {
                        Some(parent_value) => {
                            self.deep_merge(parent_value, child_value);
                        }
                        None => {
                            parent_map.insert(key, child_value);
                        }
                    }
                }
            }
            (parent, child) => {
                *parent = child;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_ignore_dirs() {
        let dirs = default_ignore_dirs();
        assert!(dirs.contains("node_modules"));
        assert!(dirs.contains("target"));
        assert!(dirs.contains(".git"));
    }

    #[test]
    fn test_default_ignore_files() {
        let files = default_ignore_files();
        assert!(files.contains("package-lock.json"));
        assert!(files.contains("Cargo.lock"));
    }

    #[test]
    fn test_extensions_all() {
        let all = extensions::all();
        assert!(all.contains(&".rs"));
        assert!(all.contains(&".md"));
        assert!(all.contains(&".json"));
    }

    #[test]
    fn test_project_config_default() {
        let config = ProjectConfig::default();
        assert!(config.respect_gitignore);
        assert!(config.permissions.allow.is_empty());
        assert!(config.permissions.deny.is_empty());
    }

    #[test]
    fn test_project_config_load_missing() {
        let temp = TempDir::new().unwrap();
        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(config.respect_gitignore);
    }

    #[test]
    fn test_project_config_load_existing() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": false, "permissions": {"allow": ["Bash(git *)"], "deny": []}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(!config.respect_gitignore);
        assert_eq!(config.permissions.allow.len(), 1);
    }

    #[test]
    fn test_is_command_allowed_empty_allow() {
        let config = ProjectConfig::default();
        // Empty allow list means allow everything (except deny list)
        assert!(config.is_command_allowed("git status"));
        assert!(config.is_command_allowed("npm install"));
    }

    #[test]
    fn test_is_command_allowed_deny_takes_precedence() {
        let config = ProjectConfig {
            permissions: PermissionsConfig {
                allow: vec!["Bash(*)".to_string()],
                deny: vec!["Bash(rm -rf /)".to_string()],
            },
            ..Default::default()
        };
        assert!(!config.is_command_allowed("rm -rf /"));
        assert!(config.is_command_allowed("git status"));
    }

    #[test]
    fn test_is_command_allowed_glob_patterns() {
        let config = ProjectConfig {
            permissions: PermissionsConfig {
                allow: vec!["Bash(git *)".to_string(), "Bash(npm *)".to_string()],
                deny: vec![],
            },
            ..Default::default()
        };
        assert!(config.is_command_allowed("git status"));
        assert!(config.is_command_allowed("git commit -m test"));
        assert!(config.is_command_allowed("npm install"));
        assert!(!config.is_command_allowed("rm -rf /"));
    }

    #[test]
    fn test_path_helpers() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        assert_eq!(
            ProjectConfig::settings_path(path),
            path.join(".claude/settings.json")
        );
        assert_eq!(
            ProjectConfig::claude_md_path(path),
            path.join(".claude/CLAUDE.md")
        );
        assert_eq!(ProjectConfig::analytics_dir(path), path.join(".ralph"));
        assert_eq!(ProjectConfig::archive_dir(path), path.join(".archive"));
        assert_eq!(
            ProjectConfig::analysis_dir(path),
            path.join(".ralph/analysis")
        );
    }

    #[test]
    fn test_dangerous_patterns() {
        assert!(DANGEROUS_PATTERNS.contains(&"rm -rf /"));
        assert!(DANGEROUS_PATTERNS.contains(&"chmod 777"));
    }

    #[test]
    fn test_secret_patterns_valid_regex() {
        for pattern in SECRET_PATTERNS {
            assert!(
                regex::Regex::new(pattern).is_ok(),
                "Invalid pattern: {}",
                pattern
            );
        }
    }

    // ============================================================================
    // SSH Blocking Tests
    // ============================================================================

    #[test]
    fn test_ssh_patterns_blocked() {
        assert!(is_ssh_command("ssh-keygen -t rsa"));
        assert!(is_ssh_command("ssh-add ~/.ssh/id_rsa"));
        assert!(is_ssh_command("ssh-agent -s"));
        assert!(is_ssh_command("cat ~/.ssh/id_rsa"));
        assert!(is_ssh_command("ls ~/.ssh"));
        assert!(is_ssh_command("git clone git@github.com:user/repo.git"));
        assert!(is_ssh_command(
            "git remote add origin git@github.com:user/repo"
        ));
    }

    #[test]
    fn test_non_ssh_commands_allowed() {
        assert!(!is_ssh_command("git status"));
        assert!(!is_ssh_command("gh repo clone user/repo"));
        assert!(!is_ssh_command("cat README.md"));
        assert!(!is_ssh_command("ls -la"));
        assert!(!is_ssh_command("git push origin main"));
    }

    #[test]
    fn test_suggest_gh_alternative_clone() {
        let alt = suggest_gh_alternative("git clone git@github.com:user/repo.git");
        assert!(alt.is_some());
        let suggestion = alt.unwrap();
        assert!(suggestion.contains("gh repo clone"));
        assert!(suggestion.contains("user/repo"));
    }

    #[test]
    fn test_suggest_gh_alternative_keygen() {
        let alt = suggest_gh_alternative("ssh-keygen -t ed25519");
        assert!(alt.is_some());
        let suggestion = alt.unwrap();
        assert!(suggestion.contains("gh auth login"));
    }

    #[test]
    fn test_suggest_gh_alternative_remote() {
        let alt = suggest_gh_alternative("git remote add origin git@github.com:user/repo");
        assert!(alt.is_some());
        let suggestion = alt.unwrap();
        assert!(suggestion.contains("gh repo set-default"));
    }

    #[test]
    fn test_suggest_gh_alternative_none() {
        let alt = suggest_gh_alternative("git status");
        assert!(alt.is_none());
    }

    // ============================================================================
    // Stagnation Level Tests
    // ============================================================================

    #[test]
    fn test_stagnation_level_from_count() {
        assert_eq!(StagnationLevel::from_count(0, 5), StagnationLevel::None);
        assert_eq!(StagnationLevel::from_count(4, 5), StagnationLevel::None);
        assert_eq!(StagnationLevel::from_count(5, 5), StagnationLevel::Warning);
        assert_eq!(StagnationLevel::from_count(9, 5), StagnationLevel::Warning);
        assert_eq!(
            StagnationLevel::from_count(10, 5),
            StagnationLevel::Elevated
        );
        assert_eq!(
            StagnationLevel::from_count(14, 5),
            StagnationLevel::Elevated
        );
        assert_eq!(
            StagnationLevel::from_count(15, 5),
            StagnationLevel::Critical
        );
        assert_eq!(
            StagnationLevel::from_count(100, 5),
            StagnationLevel::Critical
        );
    }

    #[test]
    fn test_stagnation_level_zero_threshold() {
        // Edge case: zero threshold should always return None
        assert_eq!(StagnationLevel::from_count(0, 0), StagnationLevel::None);
        assert_eq!(StagnationLevel::from_count(100, 0), StagnationLevel::None);
    }

    #[test]
    fn test_stagnation_level_should_invoke_supervisor() {
        assert!(!StagnationLevel::None.should_invoke_supervisor());
        assert!(!StagnationLevel::Warning.should_invoke_supervisor());
        assert!(StagnationLevel::Elevated.should_invoke_supervisor());
        assert!(StagnationLevel::Critical.should_invoke_supervisor());
    }

    #[test]
    fn test_stagnation_level_should_abort() {
        assert!(!StagnationLevel::None.should_abort());
        assert!(!StagnationLevel::Warning.should_abort());
        assert!(!StagnationLevel::Elevated.should_abort());
        assert!(StagnationLevel::Critical.should_abort());
    }

    #[test]
    fn test_stagnation_level_display() {
        assert_eq!(format!("{}", StagnationLevel::None), "NONE");
        assert_eq!(format!("{}", StagnationLevel::Warning), "WARNING");
        assert_eq!(format!("{}", StagnationLevel::Elevated), "ELEVATED");
        assert_eq!(format!("{}", StagnationLevel::Critical), "CRITICAL");
    }

    // ============================================================================
    // Git Environment Check Tests
    // ============================================================================

    #[test]
    fn test_git_environment_check_is_ready() {
        let check = GitEnvironmentCheck {
            gh_installed: true,
            gh_authenticated: true,
            git_installed: true,
            git_user: Some("test".to_string()),
            git_email: Some("test@example.com".to_string()),
            ssh_agent_running: false,
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        assert!(check.is_ready());
    }

    #[test]
    fn test_git_environment_check_not_ready_no_gh() {
        let check = GitEnvironmentCheck {
            gh_installed: false,
            gh_authenticated: false,
            git_installed: true,
            git_user: None,
            git_email: None,
            ssh_agent_running: false,
            errors: vec!["gh CLI not installed".to_string()],
            warnings: Vec::new(),
        };
        assert!(!check.is_ready());
    }

    #[test]
    fn test_git_environment_check_not_ready_no_auth() {
        let check = GitEnvironmentCheck {
            gh_installed: true,
            gh_authenticated: false,
            git_installed: true,
            git_user: None,
            git_email: None,
            ssh_agent_running: false,
            errors: vec!["gh CLI not authenticated".to_string()],
            warnings: Vec::new(),
        };
        assert!(!check.is_ready());
    }

    // =========================================================================
    // Gate Weight Configuration Tests (Sprint 9, Phase 9.1)
    // =========================================================================

    #[test]
    fn test_project_config_gate_weights_default() {
        let config = ProjectConfig::default();
        assert!(
            (config.gate_weights.changed_weight - 1.0).abs() < f64::EPSILON,
            "Default changed_weight should be 1.0"
        );
        assert!(
            (config.gate_weights.unchanged_weight - 0.3).abs() < f64::EPSILON,
            "Default unchanged_weight should be 0.3"
        );
    }

    #[test]
    fn test_project_config_load_with_gate_weights() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"gateWeights": {"changed_weight": 1.0, "unchanged_weight": 0.5}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(
            (config.gate_weights.unchanged_weight - 0.5).abs() < f64::EPSILON,
            "Custom unchanged_weight should be loaded"
        );
    }

    #[test]
    fn test_project_config_gate_weights_missing_uses_default() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(
            (config.gate_weights.changed_weight - 1.0).abs() < f64::EPSILON,
            "Missing gate_weights should use default changed_weight"
        );
        assert!(
            (config.gate_weights.unchanged_weight - 0.3).abs() < f64::EPSILON,
            "Missing gate_weights should use default unchanged_weight"
        );
    }

    // =========================================================================
    // Context Priority Configuration Tests (Sprint 9, Phase 9.2)
    // =========================================================================

    #[test]
    fn test_project_config_context_priority_default() {
        let config = ProjectConfig::default();
        assert!(
            (config.context_priority.changed_score - 10.0).abs() < f64::EPSILON,
            "Default changed_score should be 10.0"
        );
        assert!(
            (config.context_priority.primary_language_score - 5.0).abs() < f64::EPSILON,
            "Default primary_language_score should be 5.0"
        );
        assert!(
            config.context_priority.include_related_tests,
            "Default include_related_tests should be true"
        );
    }

    #[test]
    fn test_project_config_load_with_context_priority() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"contextPriority": {"changed_score": 20.0, "include_related_tests": false}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(
            (config.context_priority.changed_score - 20.0).abs() < f64::EPSILON,
            "Custom changed_score should be loaded"
        );
        assert!(
            !config.context_priority.include_related_tests,
            "Custom include_related_tests should be loaded"
        );
    }

    #[test]
    fn test_project_config_context_priority_missing_uses_default() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(
            (config.context_priority.changed_score - 10.0).abs() < f64::EPSILON,
            "Missing context_priority should use default changed_score"
        );
        assert!(
            (config.context_priority.primary_language_score - 5.0).abs() < f64::EPSILON,
            "Missing context_priority should use default primary_language_score"
        );
    }

    // =========================================================================
    // Predictor Weights Configuration Tests (Phase 10.3)
    // =========================================================================

    #[test]
    fn test_predictor_weights_config_default() {
        let config = PredictorWeightsConfig::default();
        assert!(config.preset.is_none());
        assert!((config.commit_gap - 0.25).abs() < f64::EPSILON);
        assert!((config.file_churn - 0.20).abs() < f64::EPSILON);
        assert!((config.error_repeat - 0.20).abs() < f64::EPSILON);
        assert!((config.test_stagnation - 0.15).abs() < f64::EPSILON);
        assert!((config.mode_oscillation - 0.10).abs() < f64::EPSILON);
        assert!((config.warning_growth - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_predictor_weights_config_validation_valid() {
        let config = PredictorWeightsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_predictor_weights_config_validation_invalid_preset() {
        let config = PredictorWeightsConfig {
            preset: Some("invalid_preset".to_string()),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Invalid predictor weight preset"));
    }

    #[test]
    fn test_predictor_weights_config_validation_negative_weight() {
        let config = PredictorWeightsConfig {
            commit_gap: -0.1,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("negative"));
    }

    #[test]
    fn test_predictor_weights_config_validation_all_zero_without_preset() {
        let config = PredictorWeightsConfig {
            preset: None,
            commit_gap: 0.0,
            file_churn: 0.0,
            error_repeat: 0.0,
            test_stagnation: 0.0,
            mode_oscillation: 0.0,
            warning_growth: 0.0,
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("zero"));
    }

    #[test]
    fn test_predictor_weights_config_validation_all_zero_with_preset() {
        // With a preset, zero custom weights are ok because preset takes precedence
        let config = PredictorWeightsConfig {
            preset: Some("conservative".to_string()),
            commit_gap: 0.0,
            file_churn: 0.0,
            error_repeat: 0.0,
            test_stagnation: 0.0,
            mode_oscillation: 0.0,
            warning_growth: 0.0,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_predictor_weights_config_has_preset() {
        let without_preset = PredictorWeightsConfig::default();
        assert!(!without_preset.has_preset());

        let with_preset = PredictorWeightsConfig {
            preset: Some("balanced".to_string()),
            ..Default::default()
        };
        assert!(with_preset.has_preset());
    }

    #[test]
    fn test_predictor_weights_config_weight_values() {
        let config = PredictorWeightsConfig {
            preset: None,
            commit_gap: 0.30,
            file_churn: 0.25,
            error_repeat: 0.20,
            test_stagnation: 0.10,
            mode_oscillation: 0.10,
            warning_growth: 0.05,
        };
        let (cg, fc, er, ts, mo, wg) = config.weight_values();
        assert!((cg - 0.30).abs() < f64::EPSILON);
        assert!((fc - 0.25).abs() < f64::EPSILON);
        assert!((er - 0.20).abs() < f64::EPSILON);
        assert!((ts - 0.10).abs() < f64::EPSILON);
        assert!((mo - 0.10).abs() < f64::EPSILON);
        assert!((wg - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_project_config_predictor_weights_default() {
        let config = ProjectConfig::default();
        assert!(config.predictor_weights.preset.is_none());
        assert!((config.predictor_weights.commit_gap - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_project_config_load_with_predictor_weights_preset() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"predictorWeights": {"preset": "conservative"}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert_eq!(
            config.predictor_weights.preset.as_deref(),
            Some("conservative")
        );
    }

    #[test]
    fn test_project_config_load_with_predictor_weights_custom() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": 0.40, "file_churn": 0.30}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(config.predictor_weights.preset.is_none());
        assert!((config.predictor_weights.commit_gap - 0.40).abs() < f64::EPSILON);
        assert!((config.predictor_weights.file_churn - 0.30).abs() < f64::EPSILON);
        // Other weights should use defaults
        assert!((config.predictor_weights.error_repeat - 0.20).abs() < f64::EPSILON);
    }

    #[test]
    fn test_project_config_predictor_weights_missing_uses_default() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert!(config.predictor_weights.preset.is_none());
        assert!((config.predictor_weights.commit_gap - 0.25).abs() < f64::EPSILON);
    }

    // =========================================================================
    // LLM Configuration Tests (Phase 12.2)
    // =========================================================================

    #[test]
    fn test_project_config_llm_default() {
        let config = ProjectConfig::default();
        assert_eq!(config.llm.model, "claude");
        assert_eq!(config.llm.api_key_env, "ANTHROPIC_API_KEY");
        assert!(config.llm.options.is_empty());
    }

    #[test]
    fn test_project_config_load_with_llm_model() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"llm": {"model": "claude", "api_key_env": "MY_KEY"}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert_eq!(config.llm.model, "claude");
        assert_eq!(config.llm.api_key_env, "MY_KEY");
    }

    #[test]
    fn test_project_config_load_with_llm_options() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"llm": {"model": "claude", "options": {"variant": "sonnet"}}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert_eq!(config.llm.model, "claude");
        assert_eq!(config.llm.claude_variant(), "sonnet");
    }

    #[test]
    fn test_project_config_llm_missing_uses_default() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert_eq!(config.llm.model, "claude");
        assert_eq!(config.llm.api_key_env, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_project_config_llm_partial_uses_defaults() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        // Only specify model, api_key_env should use default
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"llm": {"model": "claude"}}"#,
        )
        .unwrap();

        let config = ProjectConfig::load(temp.path()).unwrap();
        assert_eq!(config.llm.model, "claude");
        assert_eq!(config.llm.api_key_env, "ANTHROPIC_API_KEY");
    }

    // =========================================================================
    // Configuration Inheritance Tests (Phase 17.1)
    // =========================================================================

    #[test]
    fn test_config_inheritance_project_inherits_from_user() {
        let temp = TempDir::new().unwrap();

        // Create user config with some values
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"respectGitignore": false, "predictorWeights": {"commit_gap": 0.40}}"#,
        )
        .unwrap();

        // Create project config with different values (should override)
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"file_churn": 0.35}}"#,
        )
        .unwrap();

        // Load with inheritance
        let loader = ConfigLoader::new()
            .with_user_config_path(user_config_dir.join("config.json"));
        let config = loader.load(&project_dir).unwrap();

        // Project value should be used (file_churn from project)
        assert!(
            (config.predictor_weights.file_churn - 0.35).abs() < f64::EPSILON,
            "Project's file_churn should be used"
        );
        // User value should be inherited (commit_gap from user)
        assert!(
            (config.predictor_weights.commit_gap - 0.40).abs() < f64::EPSILON,
            "User's commit_gap should be inherited"
        );
        // User value should be inherited (respectGitignore from user)
        assert!(
            !config.respect_gitignore,
            "User's respectGitignore should be inherited"
        );
    }

    #[test]
    fn test_config_inheritance_user_inherits_from_system() {
        let temp = TempDir::new().unwrap();

        // Create system config
        let system_config_dir = temp.path().join("system_config");
        std::fs::create_dir_all(&system_config_dir).unwrap();
        std::fs::write(
            system_config_dir.join("config.json"),
            r#"{"respectGitignore": false, "llm": {"model": "claude", "api_key_env": "SYSTEM_KEY"}}"#,
        )
        .unwrap();

        // Create user config (should override system)
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"llm": {"api_key_env": "USER_KEY"}}"#,
        )
        .unwrap();

        // Create empty project dir
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Load with inheritance
        let loader = ConfigLoader::new()
            .with_system_config_path(system_config_dir.join("config.json"))
            .with_user_config_path(user_config_dir.join("config.json"));
        let config = loader.load(&project_dir).unwrap();

        // User value should override system
        assert_eq!(
            config.llm.api_key_env, "USER_KEY",
            "User's api_key_env should override system"
        );
        // System value should be inherited (respectGitignore)
        assert!(
            !config.respect_gitignore,
            "System's respectGitignore should be inherited"
        );
    }

    #[test]
    fn test_config_inheritance_explicit_override() {
        let temp = TempDir::new().unwrap();

        // Create system config
        let system_config_dir = temp.path().join("system_config");
        std::fs::create_dir_all(&system_config_dir).unwrap();
        std::fs::write(
            system_config_dir.join("config.json"),
            r#"{"respectGitignore": false, "predictorWeights": {"commit_gap": 0.10}}"#,
        )
        .unwrap();

        // Create user config (overrides system)
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"predictorWeights": {"commit_gap": 0.20}}"#,
        )
        .unwrap();

        // Create project config (overrides user and system)
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": 0.30}}"#,
        )
        .unwrap();

        // Load with inheritance
        let loader = ConfigLoader::new()
            .with_system_config_path(system_config_dir.join("config.json"))
            .with_user_config_path(user_config_dir.join("config.json"));
        let config = loader.load(&project_dir).unwrap();

        // Project value should override all
        assert!(
            (config.predictor_weights.commit_gap - 0.30).abs() < f64::EPSILON,
            "Project's commit_gap should override user and system"
        );
        // System value should be inherited where not overridden
        assert!(
            !config.respect_gitignore,
            "System's respectGitignore should be inherited"
        );
    }

    #[test]
    fn test_config_inheritance_arrays_merged() {
        let temp = TempDir::new().unwrap();

        // Create user config with some allowed commands
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"permissions": {"allow": ["Bash(git *)"], "deny": []}}"#,
        )
        .unwrap();

        // Create project config with additional allowed commands
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["Bash(cargo *)"], "deny": []}}"#,
        )
        .unwrap();

        // Load with inheritance and array merging
        let loader = ConfigLoader::new()
            .with_user_config_path(user_config_dir.join("config.json"))
            .with_array_merge_strategy(ArrayMergeStrategy::Merge);
        let config = loader.load(&project_dir).unwrap();

        // Arrays should be merged
        assert!(
            config.permissions.allow.contains(&"Bash(git *)".to_string()),
            "User's git permission should be in merged array"
        );
        assert!(
            config.permissions.allow.contains(&"Bash(cargo *)".to_string()),
            "Project's cargo permission should be in merged array"
        );
        assert_eq!(
            config.permissions.allow.len(),
            2,
            "Merged array should have 2 elements"
        );
    }

    #[test]
    fn test_config_inheritance_arrays_replaced_when_configured() {
        let temp = TempDir::new().unwrap();

        // Create user config with some allowed commands
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"permissions": {"allow": ["Bash(git *)"], "deny": []}}"#,
        )
        .unwrap();

        // Create project config with different allowed commands
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["Bash(cargo *)"], "deny": []}}"#,
        )
        .unwrap();

        // Load with replacement strategy (not merge)
        let loader = ConfigLoader::new()
            .with_user_config_path(user_config_dir.join("config.json"))
            .with_array_merge_strategy(ArrayMergeStrategy::Replace);
        let config = loader.load(&project_dir).unwrap();

        // Project arrays should replace user arrays
        assert!(
            !config.permissions.allow.contains(&"Bash(git *)".to_string()),
            "User's git permission should NOT be present with Replace strategy"
        );
        assert!(
            config.permissions.allow.contains(&"Bash(cargo *)".to_string()),
            "Project's cargo permission should be present"
        );
        assert_eq!(
            config.permissions.allow.len(),
            1,
            "Replaced array should have only 1 element"
        );
    }

    #[test]
    fn test_config_inheritance_chain_logged() {
        let temp = TempDir::new().unwrap();

        // Create system config
        let system_config_dir = temp.path().join("system_config");
        std::fs::create_dir_all(&system_config_dir).unwrap();
        std::fs::write(
            system_config_dir.join("config.json"),
            r#"{"respectGitignore": false}"#,
        )
        .unwrap();

        // Create user config
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"llm": {"model": "claude"}}"#,
        )
        .unwrap();

        // Create project config
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": 0.30}}"#,
        )
        .unwrap();

        // Load with verbose mode to get inheritance chain
        let loader = ConfigLoader::new()
            .with_system_config_path(system_config_dir.join("config.json"))
            .with_user_config_path(user_config_dir.join("config.json"))
            .with_verbose(true);
        let (_, chain) = loader.load_with_chain(&project_dir).unwrap();

        // Verify inheritance chain
        assert_eq!(chain.sources.len(), 3, "Should have 3 config sources");

        // Check system level
        assert_eq!(chain.sources[0].level, ConfigLevel::System);
        assert!(chain.sources[0].loaded, "System config should be loaded");

        // Check user level
        assert_eq!(chain.sources[1].level, ConfigLevel::User);
        assert!(chain.sources[1].loaded, "User config should be loaded");

        // Check project level
        assert_eq!(chain.sources[2].level, ConfigLevel::Project);
        assert!(chain.sources[2].loaded, "Project config should be loaded");
    }

    #[test]
    fn test_config_inheritance_missing_configs_handled() {
        let temp = TempDir::new().unwrap();

        // Create only project config (no system or user)
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": 0.30}}"#,
        )
        .unwrap();

        // Load with non-existent system and user paths
        let loader = ConfigLoader::new()
            .with_system_config_path(temp.path().join("nonexistent/system.json"))
            .with_user_config_path(temp.path().join("nonexistent/user.json"));
        let config = loader.load(&project_dir).unwrap();

        // Should still work with just project config
        assert!(
            (config.predictor_weights.commit_gap - 0.30).abs() < f64::EPSILON,
            "Project's commit_gap should be used"
        );
        // Other values should be defaults
        assert!(
            config.respect_gitignore,
            "Default respectGitignore should be true"
        );
    }

    #[test]
    fn test_config_loader_default_paths() {
        // Test that default paths are correctly computed
        let paths = ConfigLocations::default();

        // System path should exist on the filesystem structure (not necessarily the file)
        assert!(
            paths.system_path().is_some(),
            "System path should be defined"
        );

        // User path should exist
        assert!(paths.user_path().is_some(), "User path should be defined");
    }

    #[test]
    fn test_config_level_display() {
        assert_eq!(format!("{}", ConfigLevel::System), "system");
        assert_eq!(format!("{}", ConfigLevel::User), "user");
        assert_eq!(format!("{}", ConfigLevel::Project), "project");
    }

    #[test]
    fn test_config_level_ordering() {
        // System < User < Project (for precedence)
        assert!(ConfigLevel::System < ConfigLevel::User);
        assert!(ConfigLevel::User < ConfigLevel::Project);
        assert!(ConfigLevel::System < ConfigLevel::Project);
    }

    // =========================================================================
    // Shared Gate Configuration Tests (Phase 17.2)
    // =========================================================================

    #[test]
    fn test_shared_config_reference_external_file() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a shared config file in the project
        let shared_config_dir = project_dir.join("config");
        std::fs::create_dir_all(&shared_config_dir).unwrap();
        std::fs::write(
            shared_config_dir.join("team-gates.json"),
            r#"{
                "gateWeights": {
                    "changed_weight": 1.0,
                    "unchanged_weight": 0.5
                }
            }"#,
        )
        .unwrap();

        // Create project config that extends the shared config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/team-gates.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let config = resolver.load().unwrap();

        // Should have inherited the gate weights from the external file
        assert!(
            (config.gate_weights.unchanged_weight - 0.5).abs() < f64::EPSILON,
            "Should inherit unchanged_weight from external config"
        );
    }

    #[test]
    fn test_shared_config_resolved_relative_to_project_root() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a shared config in a nested directory
        let nested_dir = project_dir.join("configs/team/gates");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(
            nested_dir.join("strict.json"),
            r#"{"predictorWeights": {"commit_gap": 0.50}}"#,
        )
        .unwrap();

        // Create project config with relative path
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "configs/team/gates/strict.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let config = resolver.load().unwrap();

        // Path should resolve relative to project root
        assert!(
            (config.predictor_weights.commit_gap - 0.50).abs() < f64::EPSILON,
            "Should resolve external config relative to project root"
        );
    }

    #[test]
    fn test_shared_config_url_placeholder_for_future_cloud() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create project config with a URL reference (future cloud feature)
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "https://example.com/team-config.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let result = resolver.load();

        // URL extends should return a clear "not yet supported" error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("URL") || err_msg.contains("not supported") || err_msg.contains("cloud"),
            "Should indicate URL extends is not yet supported: {}",
            err_msg
        );
    }

    #[test]
    fn test_shared_config_missing_external_produces_clear_error() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create project config referencing non-existent file
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "nonexistent/config.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let result = resolver.load();

        // Should produce a clear error about missing file
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nonexistent") || err_msg.contains("not found") || err_msg.contains("does not exist"),
            "Error should mention the missing file path: {}",
            err_msg
        );
    }

    #[test]
    fn test_shared_config_validation_includes_external_configs() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a shared config with invalid values
        let shared_config_dir = project_dir.join("config");
        std::fs::create_dir_all(&shared_config_dir).unwrap();
        std::fs::write(
            shared_config_dir.join("invalid-config.json"),
            r#"{"predictorWeights": {"commit_gap": -0.5}}"#,
        )
        .unwrap();

        // Create project config that extends the invalid config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/invalid-config.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let validation_result = resolver.validate();

        // Validation should catch errors in external configs
        assert!(validation_result.is_err());
        let err_msg = validation_result.unwrap_err().to_string();
        assert!(
            err_msg.contains("negative") || err_msg.contains("invalid"),
            "Validation should catch invalid values in external config: {}",
            err_msg
        );
    }

    #[test]
    fn test_shared_config_chained_extends() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create base config
        let config_dir = project_dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("base.json"),
            r#"{"gateWeights": {"changed_weight": 0.8, "unchanged_weight": 0.2}}"#,
        )
        .unwrap();

        // Create intermediate config that extends base
        std::fs::write(
            config_dir.join("team.json"),
            r#"{"extends": "config/base.json", "predictorWeights": {"commit_gap": 0.35}}"#,
        )
        .unwrap();

        // Create project config that extends team config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/team.json", "respectGitignore": false}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let config = resolver.load().unwrap();

        // Should inherit from both configs in chain
        assert!(
            (config.gate_weights.unchanged_weight - 0.2).abs() < f64::EPSILON,
            "Should inherit unchanged_weight from base config"
        );
        assert!(
            (config.predictor_weights.commit_gap - 0.35).abs() < f64::EPSILON,
            "Should inherit commit_gap from team config"
        );
        assert!(
            !config.respect_gitignore,
            "Project's explicit value should override"
        );
    }

    #[test]
    fn test_shared_config_circular_extends_detected() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create config A that extends config B
        let config_dir = project_dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("a.json"),
            r#"{"extends": "config/b.json"}"#,
        )
        .unwrap();

        // Create config B that extends config A (circular)
        std::fs::write(
            config_dir.join("b.json"),
            r#"{"extends": "config/a.json"}"#,
        )
        .unwrap();

        // Create project config that extends config A
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/a.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let result = resolver.load();

        // Should detect and error on circular extends
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        let err_msg_lower = err_msg.to_lowercase();
        assert!(
            err_msg_lower.contains("circular") || err_msg_lower.contains("cycle"),
            "Should detect circular extends: {}",
            err_msg
        );
    }

    #[test]
    fn test_shared_config_project_overrides_extended() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a shared config with specific values
        let config_dir = project_dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("team.json"),
            r#"{
                "gateWeights": {"changed_weight": 0.8, "unchanged_weight": 0.4},
                "predictorWeights": {"commit_gap": 0.40}
            }"#,
        )
        .unwrap();

        // Create project config that extends but overrides some values
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{
                "extends": "config/team.json",
                "gateWeights": {"unchanged_weight": 0.6}
            }"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(project_dir);
        let config = resolver.load().unwrap();

        // Project values should override extended values
        assert!(
            (config.gate_weights.unchanged_weight - 0.6).abs() < f64::EPSILON,
            "Project's unchanged_weight should override extended config"
        );
        // But non-overridden values should be inherited
        assert!(
            (config.gate_weights.changed_weight - 0.8).abs() < f64::EPSILON,
            "Should inherit changed_weight from extended config"
        );
        assert!(
            (config.predictor_weights.commit_gap - 0.40).abs() < f64::EPSILON,
            "Should inherit commit_gap from extended config"
        );
    }
}
