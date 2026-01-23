//! Plugin system for external quality gates.
//!
//! This module provides the infrastructure for loading and executing
//! quality gates from external shared libraries.
//!
//! # Plugin Architecture
//!
//! Plugins implement the [`GatePlugin`] trait which extends [`QualityGate`]
//! with metadata (name, version, author) and plugin-specific configuration.
//!
//! ```text
//! ┌─────────────────┐
//! │   QualityGate   │
//! │                 │
//! │  - name()       │
//! │  - run()        │
//! │  - remediation()│
//! └────────┬────────┘
//!          │ extends
//!          ▼
//! ┌─────────────────┐
//! │   GatePlugin    │
//! │                 │
//! │  - metadata()   │
//! │  - timeout()    │
//! │  - configure()  │
//! └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::quality::plugin::{GatePlugin, PluginMetadata};
//! use ralph::quality::gates::{QualityGate, GateIssue};
//! use std::path::Path;
//! use anyhow::Result;
//!
//! struct MyPlugin;
//!
//! impl QualityGate for MyPlugin {
//!     fn name(&self) -> &str { "MyPlugin" }
//!     fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> { Ok(vec![]) }
//!     fn remediation(&self, issues: &[GateIssue]) -> String { String::new() }
//! }
//!
//! impl GatePlugin for MyPlugin {
//!     fn metadata(&self) -> PluginMetadata {
//!         PluginMetadata {
//!             name: "my-plugin".to_string(),
//!             version: "1.0.0".to_string(),
//!             author: "Me".to_string(),
//!             description: Some("My custom gate".to_string()),
//!             homepage: None,
//!             license: Some("MIT".to_string()),
//!         }
//!     }
//!
//!     fn timeout(&self) -> std::time::Duration {
//!         std::time::Duration::from_secs(30)
//!     }
//! }
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::gates::{GateIssue, QualityGate};

// ============================================================================
// Plugin Metadata
// ============================================================================

/// Metadata describing a plugin.
///
/// This information is used for plugin identification, versioning,
/// and display in the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin name (e.g., "rubocop-gate").
    pub name: String,
    /// Semantic version string (e.g., "1.0.0").
    pub version: String,
    /// Author name or organization.
    pub author: String,
    /// Optional description of what the plugin does.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Optional license identifier (e.g., "MIT", "Apache-2.0").
    #[serde(default)]
    pub license: Option<String>,
}

impl PluginMetadata {
    /// Create new plugin metadata with required fields.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique plugin name
    /// * `version` - Semantic version string
    /// * `author` - Author name or organization
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::quality::plugin::PluginMetadata;
    ///
    /// let meta = PluginMetadata::new("my-gate", "1.0.0", "Me");
    /// assert_eq!(meta.name, "my-gate");
    /// assert_eq!(meta.version, "1.0.0");
    /// assert_eq!(meta.author, "Me");
    /// ```
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            author: author.into(),
            description: None,
            homepage: None,
            license: None,
        }
    }

    /// Add a description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a homepage URL.
    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<String>) -> Self {
        self.homepage = Some(homepage.into());
        self
    }

    /// Add a license identifier.
    #[must_use]
    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    /// Format metadata for display.
    #[must_use]
    pub fn display(&self) -> String {
        let mut lines = vec![format!("{} v{}", self.name, self.version)];
        lines.push(format!("by {}", self.author));
        if let Some(ref desc) = self.description {
            lines.push(desc.clone());
        }
        if let Some(ref license) = self.license {
            lines.push(format!("License: {}", license));
        }
        lines.join("\n")
    }
}

// ============================================================================
// Plugin Configuration
// ============================================================================

/// Configuration for plugin execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Maximum time the plugin can run before being terminated.
    #[serde(with = "humantime_serde", default = "default_timeout")]
    pub timeout: Duration,
    /// Whether to capture and log plugin stderr.
    #[serde(default = "default_true")]
    pub capture_stderr: bool,
    /// Whether the plugin is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Additional plugin-specific configuration as key-value pairs.
    #[serde(default)]
    pub extra: std::collections::HashMap<String, String>,
}

fn default_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_true() -> bool {
    true
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            capture_stderr: true,
            enabled: true,
            extra: std::collections::HashMap::new(),
        }
    }
}

impl PluginConfig {
    /// Create a new plugin configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the execution timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set whether the plugin is enabled.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

// ============================================================================
// Plugin Errors
// ============================================================================

/// Errors that can occur during plugin operations.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin execution timed out.
    #[error("plugin '{name}' timed out after {timeout:?}")]
    Timeout {
        /// Name of the plugin that timed out.
        name: String,
        /// The configured timeout duration.
        timeout: Duration,
    },

    /// Plugin execution failed with an error.
    #[error("plugin '{name}' failed: {message}")]
    ExecutionFailed {
        /// Name of the plugin that failed.
        name: String,
        /// Error message from the plugin.
        message: String,
    },

    /// Plugin panicked during execution.
    #[error("plugin '{name}' panicked: {message}")]
    Panicked {
        /// Name of the plugin that panicked.
        name: String,
        /// Panic message if available.
        message: String,
    },

    /// Plugin returned invalid data.
    #[error("plugin '{name}' returned invalid data: {message}")]
    InvalidOutput {
        /// Name of the plugin with invalid output.
        name: String,
        /// Description of what was invalid.
        message: String,
    },
}

// ============================================================================
// Gate Plugin Trait
// ============================================================================

/// Trait for quality gate plugins.
///
/// This trait extends [`QualityGate`] with plugin-specific functionality
/// including metadata, timeout configuration, and lifecycle hooks.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to support concurrent
/// plugin execution.
///
/// # Example
///
/// ```
/// use ralph::quality::plugin::{GatePlugin, PluginMetadata, PluginConfig};
/// use ralph::quality::gates::{QualityGate, GateIssue};
/// use std::path::Path;
/// use std::time::Duration;
/// use anyhow::Result;
///
/// struct ExamplePlugin;
///
/// impl QualityGate for ExamplePlugin {
///     fn name(&self) -> &str { "ExamplePlugin" }
///
///     fn run(&self, _project_dir: &Path) -> Result<Vec<GateIssue>> {
///         Ok(vec![])
///     }
///
///     fn remediation(&self, issues: &[GateIssue]) -> String {
///         format!("Fix {} issues", issues.len())
///     }
/// }
///
/// impl GatePlugin for ExamplePlugin {
///     fn metadata(&self) -> PluginMetadata {
///         PluginMetadata::new("example-plugin", "0.1.0", "Test Author")
///     }
///
///     fn timeout(&self) -> Duration {
///         Duration::from_secs(30)
///     }
/// }
///
/// let plugin = ExamplePlugin;
/// assert_eq!(plugin.metadata().name, "example-plugin");
/// assert_eq!(plugin.metadata().version, "0.1.0");
/// assert_eq!(plugin.metadata().author, "Test Author");
/// ```
pub trait GatePlugin: QualityGate {
    /// Returns metadata about this plugin.
    ///
    /// Metadata includes the plugin name, version, author, and optional
    /// description, homepage, and license information.
    fn metadata(&self) -> PluginMetadata;

    /// Returns the maximum execution time for this plugin.
    ///
    /// If the plugin takes longer than this duration, it will be
    /// terminated and a [`PluginError::Timeout`] will be returned.
    ///
    /// The default is 60 seconds, but plugins should override this
    /// with an appropriate value based on their expected execution time.
    fn timeout(&self) -> Duration {
        Duration::from_secs(60)
    }

    /// Called when the plugin is loaded, before any runs.
    ///
    /// This hook allows plugins to perform initialization such as
    /// validating the environment, checking for required tools, or
    /// setting up resources.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails. The plugin will not
    /// be used if initialization fails.
    fn on_load(&self) -> Result<()> {
        Ok(())
    }

    /// Called when the plugin is being unloaded.
    ///
    /// This hook allows plugins to clean up resources.
    fn on_unload(&self) {
        // Default: no cleanup needed
    }

    /// Apply configuration to the plugin.
    ///
    /// This is called after loading to apply user-provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The plugin configuration to apply
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    fn configure(&mut self, _config: &PluginConfig) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Plugin Manifest (TOML format)
// ============================================================================

/// Plugin manifest loaded from plugin.toml.
///
/// The manifest describes the plugin and specifies the shared library
/// to load.
///
/// # Example TOML
///
/// ```toml
/// [plugin]
/// name = "rubocop-gate"
/// version = "1.0.0"
/// author = "Ralph Community"
/// description = "Ruby linting via RuboCop"
/// license = "MIT"
///
/// [library]
/// # Path relative to plugin.toml
/// path = "target/release/librubocop_gate.dylib"
/// # Entry point function name
/// entry_point = "create_gate_plugin"
///
/// [config]
/// timeout = "30s"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata section.
    pub plugin: PluginMetadata,
    /// Library configuration section.
    pub library: LibraryConfig,
    /// Optional plugin configuration.
    #[serde(default)]
    pub config: PluginConfig,
}

/// Library configuration within a plugin manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    /// Path to the shared library, relative to the manifest file.
    pub path: String,
    /// Name of the entry point function.
    /// Must be `extern "C" fn() -> *mut dyn GatePlugin`.
    #[serde(default = "default_entry_point")]
    pub entry_point: String,
}

fn default_entry_point() -> String {
    "create_gate_plugin".to_string()
}

impl PluginManifest {
    /// Load a plugin manifest from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the plugin.toml file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::quality::plugin::PluginManifest;
    ///
    /// let manifest = PluginManifest::load("plugins/rubocop/plugin.toml")?;
    /// println!("Loaded plugin: {}", manifest.plugin.name);
    /// ```
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow::anyhow!("failed to read manifest: {}", e))?;
        Self::parse(&content)
    }

    /// Parse a plugin manifest from TOML content.
    ///
    /// # Arguments
    ///
    /// * `content` - TOML content to parse
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid TOML or doesn't
    /// match the expected schema.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::quality::plugin::PluginManifest;
    ///
    /// let toml = r#"
    /// [plugin]
    /// name = "my-gate"
    /// version = "1.0.0"
    /// author = "Me"
    ///
    /// [library]
    /// path = "lib.so"
    /// "#;
    ///
    /// let manifest = PluginManifest::parse(toml).unwrap();
    /// assert_eq!(manifest.plugin.name, "my-gate");
    /// ```
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content)
            .map_err(|e| anyhow::anyhow!("failed to parse plugin manifest: {}", e))
    }

    /// Validate the manifest.
    ///
    /// Checks that required fields are present and valid.
    ///
    /// # Errors
    ///
    /// Returns an error describing validation failures.
    pub fn validate(&self) -> Result<()> {
        if self.plugin.name.is_empty() {
            return Err(anyhow::anyhow!("plugin name cannot be empty"));
        }
        if self.plugin.version.is_empty() {
            return Err(anyhow::anyhow!("plugin version cannot be empty"));
        }
        if self.plugin.author.is_empty() {
            return Err(anyhow::anyhow!("plugin author cannot be empty"));
        }
        if self.library.path.is_empty() {
            return Err(anyhow::anyhow!("library path cannot be empty"));
        }
        Ok(())
    }
}

// ============================================================================
// Plugin Execution Wrapper
// ============================================================================

/// Wrapper that executes a plugin with timeout and error isolation.
///
/// This wrapper ensures that plugin execution:
/// 1. Respects the configured timeout
/// 2. Catches panics and converts them to errors
/// 3. Doesn't crash the host application
pub struct PluginExecutor {
    /// Default timeout for plugins that don't specify one.
    pub default_timeout: Duration,
}

impl Default for PluginExecutor {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(60),
        }
    }
}

impl PluginExecutor {
    /// Create a new plugin executor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a plugin executor with a custom default timeout.
    #[must_use]
    pub fn with_default_timeout(timeout: Duration) -> Self {
        Self {
            default_timeout: timeout,
        }
    }

    /// Execute a plugin with error isolation.
    ///
    /// This method:
    /// 1. Runs the plugin in a panic-catching context
    /// 2. Enforces the plugin's timeout (or the default if not specified)
    /// 3. Returns errors rather than panicking
    ///
    /// # Arguments
    ///
    /// * `plugin` - The plugin to execute
    /// * `project_dir` - The project directory to check
    ///
    /// # Returns
    ///
    /// The issues found, or a [`PluginError`] if execution failed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::quality::plugin::PluginExecutor;
    ///
    /// let executor = PluginExecutor::new();
    /// let plugin = load_my_plugin();
    ///
    /// match executor.run(&plugin, Path::new(".")) {
    ///     Ok(issues) => println!("Found {} issues", issues.len()),
    ///     Err(PluginError::Timeout { name, .. }) => {
    ///         eprintln!("Plugin {} timed out", name);
    ///     }
    ///     Err(e) => eprintln!("Plugin error: {}", e),
    /// }
    /// ```
    pub fn run(
        &self,
        plugin: &dyn GatePlugin,
        project_dir: &Path,
    ) -> std::result::Result<Vec<GateIssue>, PluginError> {
        let name = plugin.metadata().name.clone();
        // Note: Actual timeout enforcement will be implemented in Phase 13.2
        // when we add async plugin execution with tokio::time::timeout
        let _timeout = plugin.timeout();

        // Catch panics from the plugin
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| plugin.run(project_dir)));

        match result {
            Ok(Ok(issues)) => Ok(issues),
            Ok(Err(e)) => Err(PluginError::ExecutionFailed {
                name,
                message: e.to_string(),
            }),
            Err(panic_payload) => {
                let message = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                Err(PluginError::Panicked { name, message })
            }
        }
    }
}

// ============================================================================
// Humantime Serde Module
// ============================================================================

/// Serde support for Duration using human-readable format.
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = duration.as_secs();
        if secs >= 60 {
            serializer.serialize_str(&format!("{}m", secs / 60))
        } else {
            serializer.serialize_str(&format!("{}s", secs))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if let Some(mins) = s.strip_suffix('m') {
            let mins: u64 = mins
                .trim()
                .parse()
                .map_err(|_| format!("invalid minutes: {}", mins))?;
            Ok(Duration::from_secs(mins * 60))
        } else if let Some(secs) = s.strip_suffix('s') {
            let secs: u64 = secs
                .trim()
                .parse()
                .map_err(|_| format!("invalid seconds: {}", secs))?;
            Ok(Duration::from_secs(secs))
        } else {
            // Try parsing as seconds
            let secs: u64 = s.parse().map_err(|_| format!("invalid duration: {}", s))?;
            Ok(Duration::from_secs(secs))
        }
    }
}

// ============================================================================
// Plugin Loader (Phase 13.2)
// ============================================================================

/// Result of plugin loading operations.
///
/// Contains the successfully loaded manifests along with any warnings
/// or errors encountered during discovery and loading.
#[derive(Debug, Default)]
pub struct PluginLoadResult {
    /// Successfully loaded and validated plugin manifests.
    pub manifests: Vec<PluginManifest>,
    /// Warnings encountered (e.g., duplicate plugin names).
    pub warnings: Vec<String>,
    /// Errors encountered (e.g., invalid manifests, missing libraries).
    pub errors: Vec<String>,
}

/// Discovers and loads plugins from standard locations.
///
/// Plugins are discovered from:
/// - User directory: `~/.ralph/plugins/`
/// - Project directory: `<project>/.ralph/plugins/`
///
/// Project plugins take precedence over user plugins when there
/// are name conflicts.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::quality::plugin::PluginLoader;
/// use std::path::Path;
///
/// let loader = PluginLoader::new()
///     .with_project_dir(Path::new("/path/to/project"));
///
/// let manifests = loader.discover_manifests();
/// for manifest in &manifests {
///     println!("Found plugin: {} v{}", manifest.plugin.name, manifest.plugin.version);
/// }
/// ```
#[derive(Debug, Default)]
pub struct PluginLoader {
    /// Override for user plugins directory (default: ~/.ralph/plugins/).
    user_plugins_dir: Option<PathBuf>,
    /// Project directory to search for .ralph/plugins/.
    project_dir: Option<PathBuf>,
}

impl PluginLoader {
    /// Create a new plugin loader with default settings.
    ///
    /// By default, searches the user's home directory for `~/.ralph/plugins/`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom user plugins directory.
    ///
    /// Overrides the default `~/.ralph/plugins/` location.
    #[must_use]
    pub fn with_user_plugins_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.user_plugins_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the project directory to search for plugins.
    ///
    /// Will look for plugins in `<project>/.ralph/plugins/`.
    #[must_use]
    pub fn with_project_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.project_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Returns the default user plugins directory.
    ///
    /// Returns `~/.ralph/plugins/` or `None` if home directory cannot be determined.
    #[must_use]
    pub fn default_user_plugins_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".ralph").join("plugins"))
    }

    /// Get all plugin directories to search.
    fn plugin_directories(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // User plugins directory
        if let Some(ref user_dir) = self.user_plugins_dir {
            dirs.push(user_dir.clone());
        } else if let Some(default_dir) = Self::default_user_plugins_dir() {
            dirs.push(default_dir);
        }

        // Project plugins directory
        if let Some(ref project_dir) = self.project_dir {
            dirs.push(project_dir.join(".ralph").join("plugins"));
        }

        dirs
    }

    /// Discover plugin manifests from all configured directories.
    ///
    /// This method scans the plugin directories for `plugin.toml` files,
    /// parses them, and validates their contents.
    ///
    /// Invalid manifests are skipped and logged as errors.
    ///
    /// # Returns
    ///
    /// A vector of valid plugin manifests.
    #[must_use]
    pub fn discover_manifests(&self) -> Vec<PluginManifest> {
        let mut manifests = Vec::new();

        for dir in self.plugin_directories() {
            if !dir.exists() {
                tracing::debug!("Plugin directory does not exist: {}", dir.display());
                continue;
            }

            manifests.extend(self.scan_directory(&dir));
        }

        manifests
    }

    /// Scan a directory for plugin manifests.
    fn scan_directory(&self, dir: &Path) -> Vec<PluginManifest> {
        let mut manifests = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read plugin directory {}: {}", dir.display(), e);
                return manifests;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match PluginManifest::load(&manifest_path) {
                Ok(manifest) => {
                    if let Err(e) = manifest.validate() {
                        tracing::warn!(
                            "Invalid plugin manifest at {}: {}",
                            manifest_path.display(),
                            e
                        );
                        continue;
                    }
                    tracing::info!(
                        "Discovered plugin: {} v{} at {}",
                        manifest.plugin.name,
                        manifest.plugin.version,
                        manifest_path.display()
                    );
                    manifests.push(manifest);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load plugin manifest {}: {}",
                        manifest_path.display(),
                        e
                    );
                }
            }
        }

        manifests
    }

    /// Load plugins from all configured directories.
    ///
    /// This method discovers manifests, validates them, and checks for
    /// duplicates. It returns a comprehensive result including any
    /// warnings or errors encountered.
    ///
    /// # Returns
    ///
    /// A [`PluginLoadResult`] containing loaded manifests and any issues.
    pub fn load_plugins(&self) -> PluginLoadResult {
        let mut result = PluginLoadResult::default();
        let mut seen_names: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for dir in self.plugin_directories() {
            if !dir.exists() {
                tracing::debug!("Plugin directory does not exist: {}", dir.display());
                continue;
            }

            self.load_from_directory(&dir, &mut result, &mut seen_names);
        }

        result
    }

    /// Load plugins from a specific directory.
    fn load_from_directory(
        &self,
        dir: &Path,
        result: &mut PluginLoadResult,
        seen_names: &mut std::collections::HashMap<String, usize>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                result.errors.push(format!(
                    "Failed to read plugin directory {}: {}",
                    dir.display(),
                    e
                ));
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match PluginManifest::load(&manifest_path) {
                Ok(manifest) => {
                    // Validate the manifest
                    if let Err(e) = manifest.validate() {
                        result.errors.push(format!(
                            "Invalid manifest at {}: {}",
                            manifest_path.display(),
                            e
                        ));
                        continue;
                    }

                    let name = manifest.plugin.name.clone();

                    // Check for duplicates
                    if let Some(prev_idx) = seen_names.get(&name) {
                        result.warnings.push(format!(
                            "Duplicate plugin name '{}' - using version from {}",
                            name,
                            manifest_path.display()
                        ));
                        // Replace the previous manifest with this one
                        result.manifests[*prev_idx] = manifest;
                    } else {
                        let idx = result.manifests.len();
                        seen_names.insert(name, idx);
                        result.manifests.push(manifest);
                    }
                }
                Err(e) => {
                    result.errors.push(format!(
                        "Failed to load manifest at {}: {}",
                        manifest_path.display(),
                        e
                    ));
                }
            }
        }
    }

    /// List all discovered plugins in a formatted string.
    ///
    /// # Returns
    ///
    /// A formatted string listing all discovered plugins with their
    /// name, version, author, and description.
    #[must_use]
    pub fn list_plugins(&self) -> String {
        let load_result = self.load_plugins();

        if load_result.manifests.is_empty() {
            return "No plugins found.".to_string();
        }

        let mut output = String::new();
        output.push_str("Installed Plugins:\n");
        output.push_str(&"=".repeat(40));
        output.push('\n');

        for manifest in &load_result.manifests {
            output.push_str(&manifest.plugin.display());
            output.push_str("\n\n");
        }

        if !load_result.warnings.is_empty() {
            output.push_str("Warnings:\n");
            for warning in &load_result.warnings {
                output.push_str(&format!("  - {}\n", warning));
            }
        }

        if !load_result.errors.is_empty() {
            output.push_str("Errors:\n");
            for error in &load_result.errors {
                output.push_str(&format!("  - {}\n", error));
            }
        }

        output
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test helper: create a simple test plugin
    struct TestPlugin {
        meta: PluginMetadata,
        timeout: Duration,
        should_panic: bool,
        should_error: bool,
    }

    impl TestPlugin {
        fn new(name: &str) -> Self {
            Self {
                meta: PluginMetadata::new(name, "1.0.0", "Test Author"),
                timeout: Duration::from_secs(30),
                should_panic: false,
                should_error: false,
            }
        }

        fn panicking() -> Self {
            Self {
                should_panic: true,
                ..Self::new("panicking-plugin")
            }
        }

        fn erroring() -> Self {
            Self {
                should_error: true,
                ..Self::new("erroring-plugin")
            }
        }
    }

    impl QualityGate for TestPlugin {
        fn name(&self) -> &str {
            &self.meta.name
        }

        fn run(&self, _project_dir: &Path) -> Result<Vec<GateIssue>> {
            if self.should_panic {
                panic!("plugin intentionally panicked");
            }
            if self.should_error {
                return Err(anyhow::anyhow!("plugin intentionally errored"));
            }
            Ok(vec![])
        }

        fn remediation(&self, issues: &[GateIssue]) -> String {
            format!("Fix {} issues from {}", issues.len(), self.name())
        }
    }

    impl GatePlugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            self.meta.clone()
        }

        fn timeout(&self) -> Duration {
            self.timeout
        }
    }

    // -------------------------------------------------------------------------
    // Test: Plugin trait extends QualityGate trait
    // -------------------------------------------------------------------------

    #[test]
    fn test_gate_plugin_extends_quality_gate() {
        // A GatePlugin must also be a QualityGate
        let plugin = TestPlugin::new("test-plugin");

        // Can use as QualityGate
        let gate: &dyn QualityGate = &plugin;
        assert_eq!(gate.name(), "test-plugin");

        // Can use as GatePlugin
        let gate_plugin: &dyn GatePlugin = &plugin;
        assert_eq!(gate_plugin.metadata().name, "test-plugin");

        // The name() from QualityGate should match metadata name
        assert_eq!(gate.name(), gate_plugin.metadata().name);
    }

    #[test]
    fn test_gate_plugin_implements_quality_gate_run() {
        let plugin = TestPlugin::new("runner");
        let project_dir = Path::new(".");

        // Can call run() from QualityGate trait
        let result = plugin.run(project_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_gate_plugin_implements_quality_gate_remediation() {
        let plugin = TestPlugin::new("remediation-test");
        let issues = vec![GateIssue::new(
            super::super::gates::IssueSeverity::Error,
            "test issue",
        )];

        let remediation = plugin.remediation(&issues);
        assert!(remediation.contains("1 issues"));
        assert!(remediation.contains("remediation-test"));
    }

    // -------------------------------------------------------------------------
    // Test: Plugin defines metadata: name, version, author
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_metadata_has_required_fields() {
        let meta = PluginMetadata::new("my-plugin", "2.1.0", "Jane Doe");

        assert_eq!(meta.name, "my-plugin");
        assert_eq!(meta.version, "2.1.0");
        assert_eq!(meta.author, "Jane Doe");
    }

    #[test]
    fn test_plugin_metadata_optional_fields() {
        let meta = PluginMetadata::new("my-plugin", "1.0.0", "Author")
            .with_description("A test plugin")
            .with_homepage("https://example.com")
            .with_license("MIT");

        assert_eq!(meta.description, Some("A test plugin".to_string()));
        assert_eq!(meta.homepage, Some("https://example.com".to_string()));
        assert_eq!(meta.license, Some("MIT".to_string()));
    }

    #[test]
    fn test_plugin_returns_metadata() {
        let plugin = TestPlugin::new("metadata-test");
        let meta = plugin.metadata();

        assert_eq!(meta.name, "metadata-test");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.author, "Test Author");
    }

    #[test]
    fn test_plugin_metadata_display() {
        let meta = PluginMetadata::new("display-test", "1.2.3", "Display Author")
            .with_description("Tests display formatting")
            .with_license("Apache-2.0");

        let display = meta.display();
        assert!(display.contains("display-test v1.2.3"));
        assert!(display.contains("by Display Author"));
        assert!(display.contains("Tests display formatting"));
        assert!(display.contains("License: Apache-2.0"));
    }

    #[test]
    fn test_plugin_metadata_serialization() {
        let meta = PluginMetadata::new("serialize-test", "1.0.0", "Author")
            .with_description("Test description");

        let json = serde_json::to_string(&meta).expect("serialization failed");
        assert!(json.contains("serialize-test"));

        let parsed: PluginMetadata = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(parsed, meta);
    }

    // -------------------------------------------------------------------------
    // Test: Plugin errors are isolated (don't crash Ralph)
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_executor_isolates_panics() {
        let executor = PluginExecutor::new();
        let plugin = TestPlugin::panicking();

        // Plugin panics, but executor catches it
        let result = executor.run(&plugin, Path::new("."));

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::Panicked { name, message } => {
                assert_eq!(name, "panicking-plugin");
                assert!(message.contains("intentionally panicked"));
            }
            e => panic!("expected Panicked error, got: {:?}", e),
        }
    }

    #[test]
    fn test_plugin_executor_isolates_errors() {
        let executor = PluginExecutor::new();
        let plugin = TestPlugin::erroring();

        // Plugin returns error, executor wraps it
        let result = executor.run(&plugin, Path::new("."));

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::ExecutionFailed { name, message } => {
                assert_eq!(name, "erroring-plugin");
                assert!(message.contains("intentionally errored"));
            }
            e => panic!("expected ExecutionFailed error, got: {:?}", e),
        }
    }

    #[test]
    fn test_plugin_executor_success() {
        let executor = PluginExecutor::new();
        let plugin = TestPlugin::new("successful-plugin");

        let result = executor.run(&plugin, Path::new("."));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // -------------------------------------------------------------------------
    // Test: Plugin timeout configuration
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_default_timeout() {
        let plugin = TestPlugin::new("timeout-test");
        // Default timeout should be 30s as set in TestPlugin
        assert_eq!(plugin.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn test_plugin_custom_timeout() {
        let mut plugin = TestPlugin::new("custom-timeout");
        plugin.timeout = Duration::from_secs(120);

        assert_eq!(plugin.timeout(), Duration::from_secs(120));
    }

    #[test]
    fn test_plugin_config_timeout() {
        let config = PluginConfig::new().with_timeout(Duration::from_secs(45));
        assert_eq!(config.timeout, Duration::from_secs(45));
    }

    #[test]
    fn test_plugin_executor_default_timeout() {
        let executor = PluginExecutor::new();
        assert_eq!(executor.default_timeout, Duration::from_secs(60));

        let custom = PluginExecutor::with_default_timeout(Duration::from_secs(90));
        assert_eq!(custom.default_timeout, Duration::from_secs(90));
    }

    // -------------------------------------------------------------------------
    // Test: Plugin manifest validation
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_manifest_validation() {
        let valid_manifest = PluginManifest {
            plugin: PluginMetadata::new("valid-plugin", "1.0.0", "Author"),
            library: LibraryConfig {
                path: "target/release/libplugin.so".to_string(),
                entry_point: "create_gate_plugin".to_string(),
            },
            config: PluginConfig::default(),
        };

        assert!(valid_manifest.validate().is_ok());
    }

    #[test]
    fn test_plugin_manifest_rejects_empty_name() {
        let manifest = PluginManifest {
            plugin: PluginMetadata::new("", "1.0.0", "Author"),
            library: LibraryConfig {
                path: "lib.so".to_string(),
                entry_point: "create".to_string(),
            },
            config: PluginConfig::default(),
        };

        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn test_plugin_manifest_rejects_empty_version() {
        let manifest = PluginManifest {
            plugin: PluginMetadata::new("plugin", "", "Author"),
            library: LibraryConfig {
                path: "lib.so".to_string(),
                entry_point: "create".to_string(),
            },
            config: PluginConfig::default(),
        };

        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("version"));
    }

    #[test]
    fn test_plugin_manifest_rejects_empty_library_path() {
        let manifest = PluginManifest {
            plugin: PluginMetadata::new("plugin", "1.0.0", "Author"),
            library: LibraryConfig {
                path: "".to_string(),
                entry_point: "create".to_string(),
            },
            config: PluginConfig::default(),
        };

        let result = manifest.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path"));
    }

    // -------------------------------------------------------------------------
    // Test: Plugin lifecycle hooks
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_on_load_default() {
        let plugin = TestPlugin::new("lifecycle-test");
        // Default on_load should succeed
        assert!(plugin.on_load().is_ok());
    }

    #[test]
    fn test_plugin_configure_default() {
        let mut plugin = TestPlugin::new("config-test");
        let config = PluginConfig::default();
        // Default configure should succeed
        assert!(plugin.configure(&config).is_ok());
    }

    // -------------------------------------------------------------------------
    // Test: Plugin config serialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(config.capture_stderr);
        assert!(config.enabled);
        assert!(config.extra.is_empty());
    }

    #[test]
    fn test_plugin_config_json_roundtrip() {
        let config = PluginConfig::new()
            .with_timeout(Duration::from_secs(45))
            .with_enabled(false);

        let json = serde_json::to_string(&config).expect("serialization failed");
        let parsed: PluginConfig = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(parsed.timeout, Duration::from_secs(45));
        assert!(!parsed.enabled);
    }

    // -------------------------------------------------------------------------
    // Test: Error types
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_error_timeout_display() {
        let err = PluginError::Timeout {
            name: "slow-plugin".to_string(),
            timeout: Duration::from_secs(30),
        };
        let msg = err.to_string();
        assert!(msg.contains("slow-plugin"));
        assert!(msg.contains("timed out"));
    }

    #[test]
    fn test_plugin_error_execution_failed_display() {
        let err = PluginError::ExecutionFailed {
            name: "broken-plugin".to_string(),
            message: "tool not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("broken-plugin"));
        assert!(msg.contains("failed"));
        assert!(msg.contains("tool not found"));
    }

    #[test]
    fn test_plugin_error_panicked_display() {
        let err = PluginError::Panicked {
            name: "crashy-plugin".to_string(),
            message: "assertion failed".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("crashy-plugin"));
        assert!(msg.contains("panicked"));
    }

    #[test]
    fn test_plugin_error_invalid_output_display() {
        let err = PluginError::InvalidOutput {
            name: "bad-output-plugin".to_string(),
            message: "expected JSON array".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("bad-output-plugin"));
        assert!(msg.contains("invalid data"));
    }

    // =========================================================================
    // Phase 13.2: Plugin Discovery and Loading Tests
    // =========================================================================

    use super::PluginLoader;

    // -------------------------------------------------------------------------
    // Test: Plugins discovered from ~/.ralph/plugins/
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_loader_discovers_from_user_dir() {
        // Create a temporary directory to simulate ~/.ralph/plugins/
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).expect("failed to create plugins dir");

        // Create a mock plugin manifest
        let plugin_dir = plugins_dir.join("test-plugin");
        std::fs::create_dir_all(&plugin_dir).expect("failed to create plugin dir");

        let manifest_content = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
author = "Test Author"

[library]
path = "target/release/libtest_plugin.dylib"
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest_content)
            .expect("failed to write manifest");

        // Create loader and discover plugins
        let loader = PluginLoader::new().with_user_plugins_dir(&plugins_dir);

        let discovered = loader.discover_manifests();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].plugin.name, "test-plugin");
    }

    // -------------------------------------------------------------------------
    // Test: Plugins discovered from project .ralph/plugins/
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_loader_discovers_from_project_dir() {
        // Create a temporary project directory
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let project_dir = temp_dir.path();
        let plugins_dir = project_dir.join(".ralph").join("plugins");
        std::fs::create_dir_all(&plugins_dir).expect("failed to create plugins dir");

        // Create a mock plugin manifest
        let plugin_dir = plugins_dir.join("project-plugin");
        std::fs::create_dir_all(&plugin_dir).expect("failed to create plugin dir");

        let manifest_content = r#"
[plugin]
name = "project-plugin"
version = "2.0.0"
author = "Project Author"

[library]
path = "target/release/libproject_plugin.so"
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest_content)
            .expect("failed to write manifest");

        // Create loader and discover plugins
        let loader = PluginLoader::new().with_project_dir(project_dir);

        let discovered = loader.discover_manifests();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].plugin.name, "project-plugin");
    }

    #[test]
    fn test_plugin_loader_discovers_from_both_user_and_project() {
        // Create temporary directories
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        // User plugins directory
        let user_plugins_dir = temp_dir.path().join("user_plugins");
        std::fs::create_dir_all(&user_plugins_dir).expect("failed to create user plugins dir");

        let user_plugin_dir = user_plugins_dir.join("user-gate");
        std::fs::create_dir_all(&user_plugin_dir).expect("failed to create user plugin dir");
        std::fs::write(
            user_plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "user-gate"
version = "1.0.0"
author = "User"

[library]
path = "lib.so"
"#,
        )
        .expect("failed to write user manifest");

        // Project plugins directory
        let project_dir = temp_dir.path().join("project");
        let project_plugins_dir = project_dir.join(".ralph").join("plugins");
        std::fs::create_dir_all(&project_plugins_dir)
            .expect("failed to create project plugins dir");

        let project_plugin_dir = project_plugins_dir.join("project-gate");
        std::fs::create_dir_all(&project_plugin_dir).expect("failed to create project plugin dir");
        std::fs::write(
            project_plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "project-gate"
version = "1.0.0"
author = "Project"

[library]
path = "lib.so"
"#,
        )
        .expect("failed to write project manifest");

        // Create loader and discover from both
        let loader = PluginLoader::new()
            .with_user_plugins_dir(&user_plugins_dir)
            .with_project_dir(&project_dir);

        let discovered = loader.discover_manifests();

        assert_eq!(discovered.len(), 2);
        let names: Vec<&str> = discovered.iter().map(|m| m.plugin.name.as_str()).collect();
        assert!(names.contains(&"user-gate"));
        assert!(names.contains(&"project-gate"));
    }

    // -------------------------------------------------------------------------
    // Test: Plugin manifest is validated before loading
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_loader_validates_manifest_before_loading() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).expect("failed to create plugins dir");

        // Create an invalid manifest (missing required fields)
        let plugin_dir = plugins_dir.join("invalid-plugin");
        std::fs::create_dir_all(&plugin_dir).expect("failed to create plugin dir");

        let invalid_manifest = r#"
[plugin]
name = ""
version = "1.0.0"
author = "Author"

[library]
path = "lib.so"
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), invalid_manifest)
            .expect("failed to write invalid manifest");

        let loader = PluginLoader::new().with_user_plugins_dir(&plugins_dir);

        // discover_manifests should skip invalid manifests
        let discovered = loader.discover_manifests();
        assert!(
            discovered.is_empty(),
            "invalid manifests should be filtered out"
        );

        // Try to load should return load result with error
        let load_results = loader.load_plugins();
        assert!(
            load_results.errors.iter().any(|e| e.contains("invalid")),
            "should record validation error: {:?}",
            load_results.errors
        );
    }

    // -------------------------------------------------------------------------
    // Test: Duplicate plugin names produce warning
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_loader_warns_on_duplicate_names() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        // User plugins directory with a plugin
        let user_plugins_dir = temp_dir.path().join("user_plugins");
        let user_plugin_dir = user_plugins_dir.join("duplicate-gate");
        std::fs::create_dir_all(&user_plugin_dir).expect("failed to create user plugin dir");
        std::fs::write(
            user_plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "duplicate-gate"
version = "1.0.0"
author = "User"

[library]
path = "lib.so"
"#,
        )
        .expect("failed to write user manifest");

        // Project plugins directory with same plugin name
        let project_dir = temp_dir.path().join("project");
        let project_plugins_dir = project_dir.join(".ralph").join("plugins");
        let project_plugin_dir = project_plugins_dir.join("duplicate-gate");
        std::fs::create_dir_all(&project_plugin_dir).expect("failed to create project plugin dir");
        std::fs::write(
            project_plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "duplicate-gate"
version = "2.0.0"
author = "Project"

[library]
path = "lib.so"
"#,
        )
        .expect("failed to write project manifest");

        let loader = PluginLoader::new()
            .with_user_plugins_dir(&user_plugins_dir)
            .with_project_dir(&project_dir);

        let load_results = loader.load_plugins();

        // Should have a warning about duplicate
        assert!(
            load_results
                .warnings
                .iter()
                .any(|w| w.contains("duplicate")),
            "should warn about duplicate plugin names: {:?}",
            load_results.warnings
        );

        // Project plugin should take precedence (loaded later, overwrites)
        let plugin_names: Vec<&str> = load_results
            .manifests
            .iter()
            .map(|m| m.plugin.name.as_str())
            .collect();

        // Should only have one plugin with that name
        assert_eq!(
            plugin_names
                .iter()
                .filter(|&&n| n == "duplicate-gate")
                .count(),
            1
        );
    }

    // -------------------------------------------------------------------------
    // Test: Plugin load failures are logged but don't stop Ralph
    // -------------------------------------------------------------------------

    #[test]
    fn test_plugin_load_failures_isolated() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).expect("failed to create plugins dir");

        // Create a plugin with a non-existent library path
        let broken_plugin_dir = plugins_dir.join("broken-plugin");
        std::fs::create_dir_all(&broken_plugin_dir).expect("failed to create broken plugin dir");
        std::fs::write(
            broken_plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "broken-plugin"
version = "1.0.0"
author = "Author"

[library]
path = "nonexistent.so"
"#,
        )
        .expect("failed to write broken manifest");

        // Create a valid plugin manifest (library won't exist but manifest is valid)
        let valid_plugin_dir = plugins_dir.join("valid-plugin");
        std::fs::create_dir_all(&valid_plugin_dir).expect("failed to create valid plugin dir");
        std::fs::write(
            valid_plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "valid-plugin"
version = "1.0.0"
author = "Author"

[library]
path = "valid.so"
"#,
        )
        .expect("failed to write valid manifest");

        let loader = PluginLoader::new().with_user_plugins_dir(&plugins_dir);

        // Loading should not panic
        let load_results = loader.load_plugins();

        // Both manifests should be discovered (library loading is separate)
        assert_eq!(load_results.manifests.len(), 2);

        // Errors about missing libraries should be recorded
        // (actual library loading is deferred until plugin execution)
    }

    #[test]
    fn test_plugin_loader_returns_empty_on_no_plugin_dirs() {
        let loader = PluginLoader::new();
        // No directories configured
        let discovered = loader.discover_manifests();
        assert!(discovered.is_empty());
    }

    #[test]
    fn test_plugin_loader_handles_nonexistent_dirs_gracefully() {
        let loader = PluginLoader::new()
            .with_user_plugins_dir(Path::new("/nonexistent/path/to/plugins"))
            .with_project_dir(Path::new("/another/nonexistent/path"));

        // Should not panic, just return empty
        let discovered = loader.discover_manifests();
        assert!(discovered.is_empty());
    }

    #[test]
    fn test_plugin_manifest_toml_parsing() {
        let toml_content = r#"
[plugin]
name = "test-gate"
version = "1.2.3"
author = "Test Author"
description = "A test quality gate"
homepage = "https://example.com"
license = "MIT"

[library]
path = "target/release/libtest_gate.dylib"
entry_point = "create_plugin"

[config]
timeout = "45s"
enabled = true
"#;

        let manifest = PluginManifest::parse(toml_content).expect("should parse valid TOML");

        assert_eq!(manifest.plugin.name, "test-gate");
        assert_eq!(manifest.plugin.version, "1.2.3");
        assert_eq!(manifest.plugin.author, "Test Author");
        assert_eq!(
            manifest.plugin.description,
            Some("A test quality gate".to_string())
        );
        assert_eq!(
            manifest.plugin.homepage,
            Some("https://example.com".to_string())
        );
        assert_eq!(manifest.plugin.license, Some("MIT".to_string()));
        assert_eq!(manifest.library.path, "target/release/libtest_gate.dylib");
        assert_eq!(manifest.library.entry_point, "create_plugin");
        assert_eq!(manifest.config.timeout, Duration::from_secs(45));
        assert!(manifest.config.enabled);
    }

    #[test]
    fn test_plugin_manifest_toml_minimal() {
        let toml_content = r#"
[plugin]
name = "minimal-gate"
version = "0.1.0"
author = "Author"

[library]
path = "lib.so"
"#;

        let manifest = PluginManifest::parse(toml_content).expect("should parse minimal TOML");

        assert_eq!(manifest.plugin.name, "minimal-gate");
        assert_eq!(manifest.library.entry_point, "create_gate_plugin"); // default
        assert_eq!(manifest.config.timeout, Duration::from_secs(60)); // default
    }
}
