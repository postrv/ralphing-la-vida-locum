//! Configuration inheritance and resolution system for Ralph.
//!
//! This module provides types and logic for loading configurations from multiple
//! sources (system, user, project) and merging them according to inheritance rules.
//! It also supports `extends` references for shared configuration files.
//!
//! # Configuration Levels
//!
//! Configurations are loaded from three levels with increasing priority:
//!
//! 1. **System** - Machine-wide defaults (e.g., `/etc/ralph/config.json`)
//! 2. **User** - User-specific settings (e.g., `~/.config/ralph/config.json`)
//! 3. **Project** - Project-specific overrides (`.claude/settings.json`)
//!
//! Higher priority levels override lower ones, with deep merging for nested objects.
//!
//! # Extends Support
//!
//! Configuration files can reference other configurations via the `extends` field:
//!
//! ```json
//! {
//!   "extends": "config/team-defaults.json",
//!   "gateWeights": {
//!     "unchanged_weight": 0.5
//!   }
//! }
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::config::{ConfigLoader, SharedConfigResolver};
//! use std::path::Path;
//!
//! // Load with inheritance chain
//! let loader = ConfigLoader::new();
//! let (config, chain) = loader.load_with_chain(Path::new("."))?;
//! println!("{}", chain.describe());
//!
//! // Load with extends support
//! let resolver = SharedConfigResolver::new(Path::new("."));
//! let config = resolver.load()?;
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::ProjectConfig;

// ============================================================================
// Configuration Level
// ============================================================================

/// Configuration level in the inheritance hierarchy.
///
/// Lower levels are overridden by higher levels:
/// System < User < Project
///
/// # Example
///
/// ```rust
/// use ralph::config::ConfigLevel;
///
/// assert!(ConfigLevel::System < ConfigLevel::User);
/// assert!(ConfigLevel::User < ConfigLevel::Project);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigLevel {
    /// System-wide configuration (lowest priority).
    ///
    /// On Unix: `/etc/ralph/config.json`
    /// On Windows: `%PROGRAMDATA%\ralph\config.json`
    System,
    /// User-specific configuration.
    ///
    /// Typically: `~/.config/ralph/config.json`
    User,
    /// Project-specific configuration (highest priority).
    ///
    /// Located at: `.claude/settings.json` in the project root.
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

// ============================================================================
// Configuration Source
// ============================================================================

/// A source in the configuration inheritance chain.
///
/// Tracks information about a configuration file that was (or could have been)
/// loaded during inheritance resolution.
#[derive(Debug, Clone)]
pub struct ConfigSource {
    /// The level of this config source in the hierarchy.
    pub level: ConfigLevel,
    /// Path to the config file.
    pub path: PathBuf,
    /// Whether the config was successfully loaded.
    ///
    /// `false` indicates the file doesn't exist or failed to load.
    pub loaded: bool,
}

impl ConfigSource {
    /// Create a new config source.
    ///
    /// # Arguments
    ///
    /// * `level` - The configuration level (system, user, or project)
    /// * `path` - Path to the configuration file
    /// * `loaded` - Whether the configuration was successfully loaded
    #[must_use]
    pub fn new(level: ConfigLevel, path: PathBuf, loaded: bool) -> Self {
        Self {
            level,
            path,
            loaded,
        }
    }
}

// ============================================================================
// Inheritance Chain
// ============================================================================

/// The full inheritance chain showing which configs were loaded.
///
/// This provides visibility into the configuration resolution process,
/// showing which files were checked and which were successfully loaded.
///
/// # Example
///
/// ```rust
/// use ralph::config::{ConfigLevel, InheritanceChain};
/// use std::path::PathBuf;
///
/// let mut chain = InheritanceChain::new();
/// chain.add_source(ConfigLevel::System, PathBuf::from("/etc/ralph/config.json"), false);
/// chain.add_source(ConfigLevel::User, PathBuf::from("~/.config/ralph/config.json"), true);
/// chain.add_source(ConfigLevel::Project, PathBuf::from(".claude/settings.json"), true);
///
/// println!("{}", chain.describe());
/// ```
#[derive(Debug, Clone, Default)]
pub struct InheritanceChain {
    /// All config sources in order (system, user, project).
    pub sources: Vec<ConfigSource>,
}

impl InheritanceChain {
    /// Create a new empty inheritance chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source to the chain.
    ///
    /// # Arguments
    ///
    /// * `level` - The configuration level
    /// * `path` - Path to the configuration file
    /// * `loaded` - Whether the file was successfully loaded
    pub fn add_source(&mut self, level: ConfigLevel, path: PathBuf, loaded: bool) {
        self.sources.push(ConfigSource::new(level, path, loaded));
    }

    /// Get the number of sources in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Check if the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Get the number of successfully loaded sources.
    #[must_use]
    pub fn loaded_count(&self) -> usize {
        self.sources.iter().filter(|s| s.loaded).count()
    }

    /// Get a formatted description of the inheritance chain for logging.
    ///
    /// Returns a multi-line string with each source listed along with
    /// its load status (checkmark for loaded, cross for not loaded).
    #[must_use]
    pub fn describe(&self) -> String {
        let mut lines = vec!["Configuration inheritance chain:".to_string()];
        for source in &self.sources {
            let status = if source.loaded { "+" } else { "-" };
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

// ============================================================================
// Array Merge Strategy
// ============================================================================

/// Strategy for merging arrays during config inheritance.
///
/// Controls how array values are combined when child configs override
/// parent configs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrayMergeStrategy {
    /// Replace arrays entirely (child replaces parent).
    ///
    /// This is the default behavior. When a child config specifies an
    /// array field, it completely replaces the parent's array.
    #[default]
    Replace,
    /// Merge arrays (combine parent and child, deduplicating).
    ///
    /// Elements from the child array are added to the parent array,
    /// with duplicates removed.
    Merge,
}

impl ArrayMergeStrategy {
    /// Check if this is the replace strategy.
    #[must_use]
    pub fn is_replace(&self) -> bool {
        matches!(self, Self::Replace)
    }

    /// Check if this is the merge strategy.
    #[must_use]
    pub fn is_merge(&self) -> bool {
        matches!(self, Self::Merge)
    }
}

// ============================================================================
// Config Locations
// ============================================================================

/// Default config file locations for different platforms.
///
/// Provides platform-specific paths for system and user configuration files.
///
/// # Platform-Specific Paths
///
/// ## Unix/Linux/macOS
/// - System: `/etc/ralph/config.json`
/// - User: `~/.config/ralph/config.json`
///
/// ## Windows
/// - System: `%PROGRAMDATA%\ralph\config.json`
/// - User: `%APPDATA%\ralph\config.json`
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

    /// Get the default system config path for the current platform.
    ///
    /// # Returns
    ///
    /// - On Unix: `Some("/etc/ralph/config.json")`
    /// - On Windows: `Some("%PROGRAMDATA%/ralph/config.json")`
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

    /// Get the default user config path using the platform's config directory.
    ///
    /// Uses the `dirs` crate to locate the user's config directory.
    ///
    /// # Returns
    ///
    /// The path to `{config_dir}/ralph/config.json`, or `None` if the
    /// config directory cannot be determined.
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

    /// Set a custom system config path.
    #[must_use]
    pub fn with_system_path(mut self, path: PathBuf) -> Self {
        self.system = Some(path);
        self
    }

    /// Set a custom user config path.
    #[must_use]
    pub fn with_user_path(mut self, path: PathBuf) -> Self {
        self.user = Some(path);
        self
    }
}

// ============================================================================
// Config Loader
// ============================================================================

/// Configuration loader with inheritance support.
///
/// Loads configuration from system, user, and project levels,
/// merging them according to the inheritance hierarchy.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::config::ConfigLoader;
/// use std::path::Path;
///
/// let loader = ConfigLoader::new()
///     .with_verbose(true)
///     .with_array_merge_strategy(ArrayMergeStrategy::Merge);
///
/// let config = loader.load(Path::new("."))?;
/// ```
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

    /// Get the current system config path.
    #[must_use]
    pub fn system_config_path(&self) -> Option<&PathBuf> {
        self.system_config_path.as_ref()
    }

    /// Get the current user config path.
    #[must_use]
    pub fn user_config_path(&self) -> Option<&PathBuf> {
        self.user_config_path.as_ref()
    }

    /// Get the current array merge strategy.
    #[must_use]
    pub fn array_merge_strategy(&self) -> ArrayMergeStrategy {
        self.array_merge_strategy
    }

    /// Load configuration with inheritance from the given project directory.
    ///
    /// Loads and merges configurations from system, user, and project levels,
    /// with project-level settings taking highest priority.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory
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
    /// This method provides visibility into which config files were loaded
    /// during the resolution process.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory
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
            (serde_json::Value::Array(parent_arr), serde_json::Value::Array(child_arr)) => {
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
// Extendable Config
// ============================================================================

/// Configuration with optional `extends` field for shared configs.
///
/// This wrapper type allows parsing configuration files that may extend
/// other configuration files.
///
/// # Example Configuration
///
/// ```json
/// {
///   "extends": "config/team-defaults.json",
///   "gateWeights": {
///     "unchanged_weight": 0.5
///   }
/// }
/// ```
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

impl ExtendableConfig {
    /// Check if this config extends another config.
    #[must_use]
    pub fn has_extends(&self) -> bool {
        self.extends.is_some()
    }

    /// Check if the extends reference is a URL.
    #[must_use]
    pub fn extends_is_url(&self) -> bool {
        self.extends
            .as_ref()
            .map(|s| s.starts_with("http://") || s.starts_with("https://"))
            .unwrap_or(false)
    }
}

// ============================================================================
// Shared Config Error
// ============================================================================

/// Error type for shared config operations.
///
/// Covers all error cases that can occur when resolving shared configurations
/// with `extends` support.
#[derive(Debug)]
pub enum SharedConfigError {
    /// The specified config file was not found.
    NotFound {
        /// Path to the missing configuration file.
        path: PathBuf,
    },
    /// URL extends is not yet supported.
    UrlNotSupported {
        /// The URL that was specified in the `extends` field.
        url: String,
    },
    /// Circular extends detected.
    CircularExtends {
        /// The cycle of paths that form the circular reference.
        cycle: Vec<PathBuf>,
    },
    /// Failed to parse config file.
    ParseError {
        /// Path to the configuration file with the parse error.
        path: PathBuf,
        /// Description of the parse error.
        error: String,
    },
    /// Config validation failed.
    ValidationError {
        /// Path to the configuration file that failed validation.
        path: PathBuf,
        /// Description of the validation error.
        error: String,
    },
    /// IO error reading config.
    IoError {
        /// Path to the configuration file that caused the IO error.
        path: PathBuf,
        /// The underlying IO error.
        error: std::io::Error,
    },
}

impl std::fmt::Display for SharedConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { path } => {
                write!(
                    f,
                    "Config file not found: {} does not exist",
                    path.display()
                )
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

impl std::error::Error for SharedConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl SharedConfigError {
    /// Create a new NotFound error.
    #[must_use]
    pub fn not_found(path: PathBuf) -> Self {
        Self::NotFound { path }
    }

    /// Create a new UrlNotSupported error.
    #[must_use]
    pub fn url_not_supported(url: String) -> Self {
        Self::UrlNotSupported { url }
    }

    /// Create a new CircularExtends error.
    #[must_use]
    pub fn circular_extends(cycle: Vec<PathBuf>) -> Self {
        Self::CircularExtends { cycle }
    }

    /// Create a new ParseError.
    #[must_use]
    pub fn parse_error(path: PathBuf, error: String) -> Self {
        Self::ParseError { path, error }
    }

    /// Create a new ValidationError.
    #[must_use]
    pub fn validation_error(path: PathBuf, error: String) -> Self {
        Self::ValidationError { path, error }
    }

    /// Create a new IoError.
    #[must_use]
    pub fn io_error(path: PathBuf, error: std::io::Error) -> Self {
        Self::IoError { path, error }
    }

    /// Check if this is a NotFound error.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }

    /// Check if this is a CircularExtends error.
    #[must_use]
    pub fn is_circular(&self) -> bool {
        matches!(self, Self::CircularExtends { .. })
    }
}

// ============================================================================
// Shared Config Resolver
// ============================================================================

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
    /// Default maximum extends depth.
    pub const DEFAULT_MAX_DEPTH: usize = 10;

    /// Create a new resolver for the given project directory.
    #[must_use]
    pub fn new(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_path_buf(),
            max_depth: Self::DEFAULT_MAX_DEPTH,
        }
    }

    /// Set the maximum extends depth.
    ///
    /// This prevents runaway recursion in case of deeply nested extends chains.
    /// Default is 10.
    #[must_use]
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Get the project directory.
    #[must_use]
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    /// Get the maximum extends depth.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.max_depth
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
        let mut visited = HashSet::new();

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

    /// Resolve an extends path relative to the project directory.
    ///
    /// # Arguments
    ///
    /// * `extends_path` - The path specified in the `extends` field
    ///
    /// # Errors
    ///
    /// Returns an error if the path is a URL (not yet supported).
    pub fn resolve_extends_path(&self, extends_path: &str) -> Result<PathBuf, SharedConfigError> {
        // Check if it's a URL
        if extends_path.starts_with("http://") || extends_path.starts_with("https://") {
            return Err(SharedConfigError::UrlNotSupported {
                url: extends_path.to_string(),
            });
        }

        Ok(self.project_dir.join(extends_path))
    }

    /// Load a config file and recursively resolve extends.
    fn load_with_extends(
        &self,
        config_path: &Path,
        visited: &mut HashSet<PathBuf>,
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
            // Resolve and load the extended config
            let resolved_path = self.resolve_extends_path(extends_path)?;
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use tempfile::TempDir;

    // ------------------------------------------------------------------------
    // ConfigLevel Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_config_level_ordering() {
        assert!(ConfigLevel::System < ConfigLevel::User);
        assert!(ConfigLevel::User < ConfigLevel::Project);
        assert!(ConfigLevel::System < ConfigLevel::Project);
    }

    #[test]
    fn test_config_level_display() {
        assert_eq!(format!("{}", ConfigLevel::System), "system");
        assert_eq!(format!("{}", ConfigLevel::User), "user");
        assert_eq!(format!("{}", ConfigLevel::Project), "project");
    }

    #[test]
    fn test_config_level_equality() {
        assert_eq!(ConfigLevel::System, ConfigLevel::System);
        assert_ne!(ConfigLevel::System, ConfigLevel::User);
    }

    // ------------------------------------------------------------------------
    // ConfigSource Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_config_source_new() {
        let source = ConfigSource::new(
            ConfigLevel::User,
            PathBuf::from("/home/user/.config/ralph/config.json"),
            true,
        );

        assert_eq!(source.level, ConfigLevel::User);
        assert_eq!(
            source.path,
            PathBuf::from("/home/user/.config/ralph/config.json")
        );
        assert!(source.loaded);
    }

    // ------------------------------------------------------------------------
    // InheritanceChain Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_inheritance_chain_new() {
        let chain = InheritanceChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert_eq!(chain.loaded_count(), 0);
    }

    #[test]
    fn test_inheritance_chain_add_source() {
        let mut chain = InheritanceChain::new();
        chain.add_source(
            ConfigLevel::System,
            PathBuf::from("/etc/ralph/config.json"),
            false,
        );
        chain.add_source(
            ConfigLevel::User,
            PathBuf::from("~/.config/ralph/config.json"),
            true,
        );
        chain.add_source(
            ConfigLevel::Project,
            PathBuf::from(".claude/settings.json"),
            true,
        );

        assert_eq!(chain.len(), 3);
        assert_eq!(chain.loaded_count(), 2);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_inheritance_chain_describe() {
        let mut chain = InheritanceChain::new();
        chain.add_source(
            ConfigLevel::System,
            PathBuf::from("/etc/ralph/config.json"),
            false,
        );
        chain.add_source(
            ConfigLevel::User,
            PathBuf::from("/home/user/.config/ralph/config.json"),
            true,
        );

        let description = chain.describe();
        assert!(description.contains("Configuration inheritance chain:"));
        assert!(description.contains("[system]"));
        assert!(description.contains("[user]"));
        assert!(description.contains("- [system]")); // Not loaded
        assert!(description.contains("+ [user]")); // Loaded
    }

    // ------------------------------------------------------------------------
    // ArrayMergeStrategy Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_array_merge_strategy_default() {
        let strategy = ArrayMergeStrategy::default();
        assert!(strategy.is_replace());
        assert!(!strategy.is_merge());
    }

    #[test]
    fn test_array_merge_strategy_merge() {
        let strategy = ArrayMergeStrategy::Merge;
        assert!(strategy.is_merge());
        assert!(!strategy.is_replace());
    }

    // ------------------------------------------------------------------------
    // ConfigLocations Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_config_locations_new() {
        let locations = ConfigLocations::new();

        // System path should exist
        assert!(locations.system_path().is_some());

        // User path depends on environment
        // Just verify the method works
        let _ = locations.user_path();
    }

    #[test]
    fn test_config_locations_default_system_path() {
        let path = ConfigLocations::default_system_path();
        assert!(path.is_some());

        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(path.unwrap(), PathBuf::from("/etc/ralph/config.json"));
        }
    }

    #[test]
    fn test_config_locations_with_custom_paths() {
        let locations = ConfigLocations::new()
            .with_system_path(PathBuf::from("/custom/system.json"))
            .with_user_path(PathBuf::from("/custom/user.json"));

        assert_eq!(
            locations.system_path(),
            Some(&PathBuf::from("/custom/system.json"))
        );
        assert_eq!(
            locations.user_path(),
            Some(&PathBuf::from("/custom/user.json"))
        );
    }

    // ------------------------------------------------------------------------
    // ConfigLoader Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_config_loader_new() {
        let loader = ConfigLoader::new();
        assert!(loader.system_config_path().is_some());
        assert_eq!(loader.array_merge_strategy(), ArrayMergeStrategy::Replace);
    }

    #[test]
    fn test_config_loader_with_methods() {
        let loader = ConfigLoader::new()
            .with_system_config_path(PathBuf::from("/custom/system.json"))
            .with_user_config_path(PathBuf::from("/custom/user.json"))
            .with_array_merge_strategy(ArrayMergeStrategy::Merge)
            .with_verbose(true);

        assert_eq!(
            loader.system_config_path(),
            Some(&PathBuf::from("/custom/system.json"))
        );
        assert_eq!(
            loader.user_config_path(),
            Some(&PathBuf::from("/custom/user.json"))
        );
        assert_eq!(loader.array_merge_strategy(), ArrayMergeStrategy::Merge);
    }

    #[test]
    fn test_config_loader_load_missing_project() {
        let temp = TempDir::new().unwrap();
        let loader = ConfigLoader::new()
            .with_system_config_path(PathBuf::from("/nonexistent/system.json"))
            .with_user_config_path(PathBuf::from("/nonexistent/user.json"));

        let result = loader.load(temp.path());
        assert!(result.is_ok());

        let config = result.unwrap();
        // Should return default config
        assert!(config.respect_gitignore);
    }

    #[test]
    fn test_config_loader_load_with_chain() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": false}"#,
        )
        .unwrap();

        let loader = ConfigLoader::new()
            .with_system_config_path(PathBuf::from("/nonexistent/system.json"))
            .with_user_config_path(PathBuf::from("/nonexistent/user.json"));

        let (config, chain) = loader.load_with_chain(temp.path()).unwrap();

        assert!(!config.respect_gitignore);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.loaded_count(), 1); // Only project config loaded
    }

    #[test]
    fn test_config_loader_inheritance_project_overrides_user() {
        let temp = TempDir::new().unwrap();

        // Create user config
        let user_config_path = temp.path().join("user_config.json");
        std::fs::write(
            &user_config_path,
            r#"{"respectGitignore": true, "predictorWeights": {"commit_gap": 0.5}}"#,
        )
        .unwrap();

        // Create project config
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": false}"#,
        )
        .unwrap();

        let loader = ConfigLoader::new()
            .with_system_config_path(PathBuf::from("/nonexistent/system.json"))
            .with_user_config_path(user_config_path);

        let (config, _chain) = loader.load_with_chain(temp.path()).unwrap();

        // Project should override user
        assert!(!config.respect_gitignore);
        // But non-overridden values should be inherited from user
        assert!((config.predictor_weights.commit_gap - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_loader_array_merge_replace() {
        let temp = TempDir::new().unwrap();

        // Create user config with permissions
        let user_config_path = temp.path().join("user_config.json");
        std::fs::write(
            &user_config_path,
            r#"{"permissions": {"allow": ["Bash(git *)", "Bash(npm *)"], "deny": []}}"#,
        )
        .unwrap();

        // Create project config with different permissions
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["Bash(cargo *)"], "deny": []}}"#,
        )
        .unwrap();

        let loader = ConfigLoader::new()
            .with_system_config_path(PathBuf::from("/nonexistent"))
            .with_user_config_path(user_config_path)
            .with_array_merge_strategy(ArrayMergeStrategy::Replace);

        let (config, _) = loader.load_with_chain(temp.path()).unwrap();

        // With Replace strategy, project permissions should completely override user
        assert_eq!(config.permissions.allow.len(), 1);
        assert!(config.permissions.allow.contains(&"Bash(cargo *)".to_string()));
    }

    #[test]
    fn test_config_loader_array_merge_merge() {
        let temp = TempDir::new().unwrap();

        // Create user config with permissions
        let user_config_path = temp.path().join("user_config.json");
        std::fs::write(
            &user_config_path,
            r#"{"permissions": {"allow": ["Bash(git *)"], "deny": []}}"#,
        )
        .unwrap();

        // Create project config with different permissions
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"permissions": {"allow": ["Bash(cargo *)"], "deny": []}}"#,
        )
        .unwrap();

        let loader = ConfigLoader::new()
            .with_system_config_path(PathBuf::from("/nonexistent"))
            .with_user_config_path(user_config_path)
            .with_array_merge_strategy(ArrayMergeStrategy::Merge);

        let (config, _) = loader.load_with_chain(temp.path()).unwrap();

        // With Merge strategy, both permissions should be present
        assert_eq!(config.permissions.allow.len(), 2);
        assert!(config.permissions.allow.contains(&"Bash(git *)".to_string()));
        assert!(config.permissions.allow.contains(&"Bash(cargo *)".to_string()));
    }

    // ------------------------------------------------------------------------
    // ExtendableConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extendable_config_no_extends() {
        let config: ExtendableConfig = serde_json::from_str(r#"{"key": "value"}"#).unwrap();

        assert!(!config.has_extends());
        assert!(!config.extends_is_url());
    }

    #[test]
    fn test_extendable_config_with_local_extends() {
        let config: ExtendableConfig =
            serde_json::from_str(r#"{"extends": "config/base.json", "key": "value"}"#).unwrap();

        assert!(config.has_extends());
        assert!(!config.extends_is_url());
        assert_eq!(config.extends.as_ref().unwrap(), "config/base.json");
    }

    #[test]
    fn test_extendable_config_with_url_extends() {
        let config: ExtendableConfig =
            serde_json::from_str(r#"{"extends": "https://example.com/config.json"}"#).unwrap();

        assert!(config.has_extends());
        assert!(config.extends_is_url());
    }

    // ------------------------------------------------------------------------
    // SharedConfigError Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_shared_config_error_not_found() {
        let err = SharedConfigError::not_found(PathBuf::from("/missing/config.json"));
        assert!(err.is_not_found());
        assert!(!err.is_circular());

        let display = format!("{}", err);
        assert!(display.contains("not found"));
        assert!(display.contains("/missing/config.json"));
    }

    #[test]
    fn test_shared_config_error_url_not_supported() {
        let err = SharedConfigError::url_not_supported("https://example.com/config.json".to_string());

        let display = format!("{}", err);
        assert!(display.contains("URL extends not yet supported"));
        assert!(display.contains("https://example.com/config.json"));
    }

    #[test]
    fn test_shared_config_error_circular_extends() {
        let err = SharedConfigError::circular_extends(vec![
            PathBuf::from("a.json"),
            PathBuf::from("b.json"),
            PathBuf::from("a.json"),
        ]);

        assert!(err.is_circular());
        assert!(!err.is_not_found());

        let display = format!("{}", err);
        assert!(display.contains("Circular extends detected"));
        assert!(display.contains("a.json"));
        assert!(display.contains("b.json"));
    }

    #[test]
    fn test_shared_config_error_parse_error() {
        let err = SharedConfigError::parse_error(
            PathBuf::from("invalid.json"),
            "unexpected token".to_string(),
        );

        let display = format!("{}", err);
        assert!(display.contains("Failed to parse"));
        assert!(display.contains("invalid.json"));
        assert!(display.contains("unexpected token"));
    }

    #[test]
    fn test_shared_config_error_validation_error() {
        let err = SharedConfigError::validation_error(
            PathBuf::from("config.json"),
            "invalid weight".to_string(),
        );

        let display = format!("{}", err);
        assert!(display.contains("Validation error"));
        assert!(display.contains("invalid weight"));
    }

    #[test]
    fn test_shared_config_error_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = SharedConfigError::io_error(PathBuf::from("config.json"), io_err);

        let display = format!("{}", err);
        assert!(display.contains("Failed to read"));
        assert!(display.contains("access denied"));

        // Test source() returns the underlying IO error
        assert!(err.source().is_some());
    }

    // ------------------------------------------------------------------------
    // SharedConfigResolver Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_shared_config_resolver_new() {
        let resolver = SharedConfigResolver::new(Path::new("/project"));

        assert_eq!(resolver.project_dir(), Path::new("/project"));
        assert_eq!(resolver.max_depth(), SharedConfigResolver::DEFAULT_MAX_DEPTH);
    }

    #[test]
    fn test_shared_config_resolver_with_max_depth() {
        let resolver = SharedConfigResolver::new(Path::new("/project")).with_max_depth(5);

        assert_eq!(resolver.max_depth(), 5);
    }

    #[test]
    fn test_shared_config_resolver_resolve_extends_path_local() {
        let resolver = SharedConfigResolver::new(Path::new("/project"));
        let result = resolver.resolve_extends_path("config/base.json");

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/project/config/base.json")
        );
    }

    #[test]
    fn test_shared_config_resolver_resolve_extends_path_url() {
        let resolver = SharedConfigResolver::new(Path::new("/project"));
        let result = resolver.resolve_extends_path("https://example.com/config.json");

        assert!(result.is_err());
        match result.unwrap_err() {
            SharedConfigError::UrlNotSupported { url } => {
                assert_eq!(url, "https://example.com/config.json");
            }
            _ => panic!("Expected UrlNotSupported error"),
        }
    }

    #[test]
    fn test_shared_config_resolver_load_no_settings() {
        let temp = TempDir::new().unwrap();
        let resolver = SharedConfigResolver::new(temp.path());

        let config = resolver.load().unwrap();

        // Should return default config when no settings.json exists
        assert!(config.respect_gitignore);
    }

    #[test]
    fn test_shared_config_resolver_load_simple() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"respectGitignore": false}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let config = resolver.load().unwrap();

        assert!(!config.respect_gitignore);
    }

    #[test]
    fn test_shared_config_resolver_load_with_extends() {
        let temp = TempDir::new().unwrap();

        // Create base config
        std::fs::create_dir_all(temp.path().join("config")).unwrap();
        std::fs::write(
            temp.path().join("config/base.json"),
            r#"{"respectGitignore": true, "predictorWeights": {"commit_gap": 0.5}}"#,
        )
        .unwrap();

        // Create project config that extends base
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"extends": "config/base.json", "respectGitignore": false}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let config = resolver.load().unwrap();

        // Project should override base for respectGitignore
        assert!(!config.respect_gitignore);
        // But should inherit commit_gap from base
        assert!((config.predictor_weights.commit_gap - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shared_config_resolver_load_chained_extends() {
        let temp = TempDir::new().unwrap();

        // Create base config (level 0)
        std::fs::create_dir_all(temp.path().join("config")).unwrap();
        std::fs::write(
            temp.path().join("config/base.json"),
            r#"{"predictorWeights": {"commit_gap": 0.1, "file_churn": 0.2}}"#,
        )
        .unwrap();

        // Create team config that extends base (level 1)
        std::fs::write(
            temp.path().join("config/team.json"),
            r#"{"extends": "config/base.json", "predictorWeights": {"commit_gap": 0.3}}"#,
        )
        .unwrap();

        // Create project config that extends team (level 2)
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"extends": "config/team.json", "respectGitignore": false}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let config = resolver.load().unwrap();

        // Project should override respectGitignore
        assert!(!config.respect_gitignore);
        // Team should override commit_gap from base
        assert!((config.predictor_weights.commit_gap - 0.3).abs() < f64::EPSILON);
        // Base file_churn should be inherited through
        assert!((config.predictor_weights.file_churn - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shared_config_resolver_circular_extends_detection() {
        let temp = TempDir::new().unwrap();

        // Create config A that extends B
        std::fs::create_dir_all(temp.path().join("config")).unwrap();
        std::fs::write(
            temp.path().join("config/a.json"),
            r#"{"extends": "config/b.json"}"#,
        )
        .unwrap();

        // Create config B that extends A (circular!)
        std::fs::write(
            temp.path().join("config/b.json"),
            r#"{"extends": "config/a.json"}"#,
        )
        .unwrap();

        // Create project config that extends A
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"extends": "config/a.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let result = resolver.load();

        assert!(result.is_err());
        assert!(result.unwrap_err().is_circular());
    }

    #[test]
    fn test_shared_config_resolver_missing_extends() {
        let temp = TempDir::new().unwrap();

        // Create project config that extends non-existent file
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"extends": "config/missing.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let result = resolver.load();

        assert!(result.is_err());
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_shared_config_resolver_url_extends_not_supported() {
        let temp = TempDir::new().unwrap();

        // Create project config that tries to extend a URL
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"extends": "https://example.com/config.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let result = resolver.load();

        assert!(result.is_err());
        match result.unwrap_err() {
            SharedConfigError::UrlNotSupported { .. } => {}
            other => panic!("Expected UrlNotSupported error, got {:?}", other),
        }
    }

    #[test]
    fn test_shared_config_resolver_validate() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": 0.25}}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let result = resolver.validate();

        assert!(result.is_ok());
    }

    #[test]
    fn test_shared_config_resolver_validate_invalid() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": -0.5}}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path());
        let result = resolver.validate();

        assert!(result.is_err());
        match result.unwrap_err() {
            SharedConfigError::ValidationError { error, .. } => {
                assert!(error.contains("negative"));
            }
            other => panic!("Expected ValidationError, got {:?}", other),
        }
    }

    #[test]
    fn test_shared_config_resolver_max_depth_exceeded() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("config")).unwrap();

        // Create a chain of configs that's too deep
        // With max_depth=2, we can only go 2 levels deep
        for i in 0..5 {
            let extends = if i < 4 {
                format!(r#"{{"extends": "config/level{}.json"}}"#, i + 1)
            } else {
                r#"{}"#.to_string()
            };
            std::fs::write(temp.path().join(format!("config/level{}.json", i)), extends).unwrap();
        }

        std::fs::create_dir_all(temp.path().join(".claude")).unwrap();
        std::fs::write(
            temp.path().join(".claude/settings.json"),
            r#"{"extends": "config/level0.json"}"#,
        )
        .unwrap();

        let resolver = SharedConfigResolver::new(temp.path()).with_max_depth(2);
        let result = resolver.load();

        assert!(result.is_err());
        assert!(result.unwrap_err().is_circular()); // Max depth exceeded triggers this error
    }
}
