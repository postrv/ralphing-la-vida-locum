//! Configuration validation for Ralph.
//!
//! This module provides comprehensive validation of Ralph configuration files,
//! including JSON syntax validation, inheritance chain resolution, and field
//! validation.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::config::ConfigValidator;
//! use std::path::Path;
//!
//! let validator = ConfigValidator::new(Path::new("/path/to/project"));
//! let report = validator.validate()?;
//!
//! if report.is_valid() {
//!     println!("Configuration is valid!");
//! } else {
//!     for error in &report.errors {
//!         eprintln!("Error: {}", error);
//!     }
//!     std::process::exit(report.exit_code());
//! }
//! ```

use std::path::{Path, PathBuf};

use super::{
    ConfigLevel, ConfigLoader, ConfigLocations, ConfigSource, InheritanceChain, ProjectConfig,
    SharedConfigResolver,
};

/// Result of configuration validation.
///
/// Contains all errors and warnings found during validation, along with
/// metadata about the files that were checked and the inheritance chain.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::config::ValidationReport;
///
/// let report = ValidationReport::new();
/// assert!(report.is_valid()); // Empty report is valid
/// assert_eq!(report.exit_code(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Errors that prevent the configuration from being valid.
    pub errors: Vec<String>,
    /// Warnings that don't prevent validity but indicate potential issues.
    pub warnings: Vec<String>,
    /// The inheritance chain that was resolved.
    pub inheritance_chain: InheritanceChain,
    /// Files that were validated.
    pub files_checked: Vec<PathBuf>,
}

impl ValidationReport {
    /// Create a new empty validation report.
    ///
    /// An empty report is considered valid.
    #[must_use]
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            inheritance_chain: InheritanceChain::new(),
            files_checked: Vec::new(),
        }
    }

    /// Returns true if the configuration is valid (no errors).
    ///
    /// Warnings do not affect validity.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the exit code for the validation.
    ///
    /// Returns 0 if valid, 1 if invalid.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.is_valid() {
            0
        } else {
            1
        }
    }

    /// Generate a human-readable summary of the validation result.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_valid() {
            if self.warnings.is_empty() {
                "Configuration is valid.".to_string()
            } else {
                format!(
                    "Configuration is valid with {} warning(s).",
                    self.warnings.len()
                )
            }
        } else {
            format!(
                "Configuration is invalid with {} error(s).",
                self.errors.len()
            )
        }
    }

    /// Generate a verbose report including all details.
    ///
    /// The report includes:
    /// - Configuration validation header
    /// - Inheritance chain with load status
    /// - Files that were checked
    /// - All errors (if any)
    /// - All warnings (if any)
    /// - Final status summary
    #[must_use]
    pub fn verbose_report(&self) -> String {
        // Initialize with header
        let mut lines = vec![
            "Configuration Validation Report".to_string(),
            "\u{2500}".repeat(50),
            String::new(),
            "Inheritance chain:".to_string(),
        ];

        // Inheritance chain
        if self.inheritance_chain.sources.is_empty() {
            lines.push("  (no config files found)".to_string());
        } else {
            for source in &self.inheritance_chain.sources {
                let status = if source.loaded { "\u{2713}" } else { "\u{2717}" };
                lines.push(format!(
                    "  {} [{}] {}",
                    status,
                    source.level,
                    source.path.display()
                ));
            }
        }

        // Files checked
        if !self.files_checked.is_empty() {
            lines.push(String::new());
            lines.push(format!("Files checked ({}):", self.files_checked.len()));
            for file in &self.files_checked {
                lines.push(format!("  - {}", file.display()));
            }
        }

        // Errors
        if !self.errors.is_empty() {
            lines.push(String::new());
            lines.push(format!("Errors ({}):", self.errors.len()));
            for error in &self.errors {
                lines.push(format!("  \u{2717} {}", error));
            }
        }

        // Warnings
        if !self.warnings.is_empty() {
            lines.push(String::new());
            lines.push(format!("Warnings ({}):", self.warnings.len()));
            for warning in &self.warnings {
                lines.push(format!("  \u{26a0} {}", warning));
            }
        }

        // Final status
        lines.push(String::new());
        lines.push(format!("Status: {}", self.summary()));

        lines.join("\n")
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates project configuration files.
///
/// `ConfigValidator` performs comprehensive validation of Ralph configuration,
/// including:
/// - JSON syntax validation for settings.json
/// - Inheritance chain resolution (system -> user -> project)
/// - `extends` reference validation
/// - Field validation (e.g., predictor weights, LLM config)
/// - Optional file validation (CLAUDE.md, mcp.json)
///
/// # Example
///
/// ```rust,ignore
/// use ralph::config::ConfigValidator;
/// use std::path::Path;
///
/// let validator = ConfigValidator::new(Path::new("/path/to/project"));
/// let report = validator.validate()?;
///
/// if report.is_valid() {
///     println!("Configuration is valid!");
/// } else {
///     for error in &report.errors {
///         eprintln!("Error: {}", error);
///     }
///     std::process::exit(report.exit_code());
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ConfigValidator {
    project_dir: PathBuf,
    system_config_path: Option<PathBuf>,
    user_config_path: Option<PathBuf>,
}

impl ConfigValidator {
    /// Create a new validator for the given project directory.
    ///
    /// Uses default system and user config paths based on the current platform.
    #[must_use]
    pub fn new(project_dir: &Path) -> Self {
        let locations = ConfigLocations::new();
        Self {
            project_dir: project_dir.to_path_buf(),
            system_config_path: locations.system_path().cloned(),
            user_config_path: locations.user_path().cloned(),
        }
    }

    /// Set a custom system config path for testing.
    #[must_use]
    pub fn with_system_config_path(mut self, path: PathBuf) -> Self {
        self.system_config_path = Some(path);
        self
    }

    /// Set a custom user config path for testing.
    #[must_use]
    pub fn with_user_config_path(mut self, path: PathBuf) -> Self {
        self.user_config_path = Some(path);
        self
    }

    /// Validate the configuration and return a detailed report.
    ///
    /// # Errors
    ///
    /// Returns an error only for unexpected I/O failures. Validation errors
    /// are reported in the `ValidationReport`.
    ///
    /// # Validation Steps
    ///
    /// 1. Check settings.json syntax
    /// 2. Resolve inheritance chain (system -> user -> project)
    /// 3. Validate `extends` references
    /// 4. Validate predictor weights configuration
    /// 5. Validate LLM configuration
    /// 6. Check mcp.json syntax (if exists)
    /// 7. Warn if CLAUDE.md is missing
    pub fn validate(&self) -> anyhow::Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Check settings.json syntax and load inheritance chain
        let settings_path = ProjectConfig::settings_path(&self.project_dir);
        report.files_checked.push(settings_path.clone());

        if settings_path.exists() {
            // Validate JSON syntax first
            match std::fs::read_to_string(&settings_path) {
                Ok(content) => {
                    if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
                        report.errors.push(format!(
                            "settings.json syntax error: {} (parse failed at line {}, column {})",
                            e,
                            e.line(),
                            e.column()
                        ));
                        // Can't continue if JSON is invalid
                        return Ok(report);
                    }
                }
                Err(e) => {
                    report
                        .errors
                        .push(format!("Cannot read settings.json: {}", e));
                    return Ok(report);
                }
            }

            // Validate inheritance chain
            let loader = ConfigLoader::new()
                .with_system_config_path(
                    self.system_config_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("/nonexistent")),
                )
                .with_user_config_path(
                    self.user_config_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("/nonexistent")),
                );

            match loader.load_with_chain(&self.project_dir) {
                Ok((_, chain)) => {
                    report.inheritance_chain = chain;
                }
                Err(e) => {
                    report
                        .errors
                        .push(format!("Inheritance chain error: {}", e));
                }
            }

            // Validate extends references using SharedConfigResolver
            let resolver = SharedConfigResolver::new(&self.project_dir);
            match resolver.load() {
                Ok(config) => {
                    // Validate the loaded config
                    if let Err(e) = config.predictor_weights.validate() {
                        report
                            .errors
                            .push(format!("Predictor weights validation error: {}", e));
                    }

                    if let Err(e) = config.llm.validate() {
                        report
                            .errors
                            .push(format!("LLM config validation error: {}", e));
                    }
                }
                Err(e) => {
                    report.errors.push(format!("{}", e));
                }
            }
        } else {
            // No settings.json - warn and use defaults
            report
                .warnings
                .push("settings.json not found - using defaults".to_string());

            // Still set up inheritance chain for the missing file
            report
                .inheritance_chain
                .add_source(ConfigLevel::Project, settings_path, false);
        }

        // Check mcp.json if it exists
        let mcp_path = self.project_dir.join(".claude/mcp.json");
        if mcp_path.exists() {
            report.files_checked.push(mcp_path.clone());
            match std::fs::read_to_string(&mcp_path) {
                Ok(content) => {
                    if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
                        report.errors.push(format!(
                            "mcp.json syntax error: {} (parse failed at line {}, column {})",
                            e,
                            e.line(),
                            e.column()
                        ));
                    }
                }
                Err(e) => {
                    report.errors.push(format!("Cannot read mcp.json: {}", e));
                }
            }
        }

        // Check CLAUDE.md exists (warn if missing)
        let claude_md_path = ProjectConfig::claude_md_path(&self.project_dir);
        if claude_md_path.exists() {
            report.files_checked.push(claude_md_path);
        } else {
            report
                .warnings
                .push("CLAUDE.md not found - project instructions may be missing".to_string());
        }

        // Add system/user config paths to inheritance chain if they exist
        if let Some(ref system_path) = self.system_config_path {
            let loaded = system_path.exists();
            if report.inheritance_chain.sources.is_empty()
                || report.inheritance_chain.sources[0].level != ConfigLevel::System
            {
                let mut new_sources = vec![ConfigSource {
                    level: ConfigLevel::System,
                    path: system_path.clone(),
                    loaded,
                }];
                new_sources.append(&mut report.inheritance_chain.sources);
                report.inheritance_chain.sources = new_sources;
            }
        }

        if let Some(ref user_path) = self.user_config_path {
            let loaded = user_path.exists();
            // Find the right position to insert (after system, before project)
            let insert_pos = report
                .inheritance_chain
                .sources
                .iter()
                .position(|s| s.level == ConfigLevel::Project)
                .unwrap_or(report.inheritance_chain.sources.len());

            if !report
                .inheritance_chain
                .sources
                .iter()
                .any(|s| s.level == ConfigLevel::User)
            {
                report.inheritance_chain.sources.insert(
                    insert_pos,
                    ConfigSource {
                        level: ConfigLevel::User,
                        path: user_path.clone(),
                        loaded,
                    },
                );
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // ValidationReport Tests
    // =========================================================================

    #[test]
    fn test_validation_report_new_is_valid() {
        let report = ValidationReport::new();
        assert!(report.is_valid());
        assert!(report.errors.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_validation_report_default_is_valid() {
        let report = ValidationReport::default();
        assert!(report.is_valid());
        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn test_validation_report_with_errors_is_invalid() {
        let mut report = ValidationReport::new();
        report.errors.push("Test error".to_string());
        assert!(!report.is_valid());
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn test_validation_report_with_warnings_is_valid() {
        let mut report = ValidationReport::new();
        report.warnings.push("Test warning".to_string());
        assert!(report.is_valid());
        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn test_validation_report_summary_valid() {
        let report = ValidationReport::new();
        assert_eq!(report.summary(), "Configuration is valid.");
    }

    #[test]
    fn test_validation_report_summary_valid_with_warnings() {
        let mut report = ValidationReport::new();
        report.warnings.push("Warning 1".to_string());
        report.warnings.push("Warning 2".to_string());
        assert_eq!(
            report.summary(),
            "Configuration is valid with 2 warning(s)."
        );
    }

    #[test]
    fn test_validation_report_summary_invalid() {
        let mut report = ValidationReport::new();
        report.errors.push("Error 1".to_string());
        report.errors.push("Error 2".to_string());
        report.errors.push("Error 3".to_string());
        assert_eq!(
            report.summary(),
            "Configuration is invalid with 3 error(s)."
        );
    }

    #[test]
    fn test_validation_report_exit_code_valid() {
        let report = ValidationReport::new();
        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn test_validation_report_exit_code_invalid() {
        let mut report = ValidationReport::new();
        report.errors.push("Error".to_string());
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn test_validation_report_verbose_report_format() {
        let mut report = ValidationReport::new();
        report.errors.push("Test error".to_string());
        report.warnings.push("Test warning".to_string());
        report.files_checked.push(PathBuf::from("/test/file.json"));

        let verbose = report.verbose_report();

        // Check header
        assert!(verbose.contains("Configuration Validation Report"));

        // Check inheritance chain section
        assert!(verbose.contains("Inheritance chain:"));

        // Check files checked section
        assert!(verbose.contains("Files checked"));
        assert!(verbose.contains("/test/file.json"));

        // Check errors section
        assert!(verbose.contains("Errors (1):"));
        assert!(verbose.contains("Test error"));

        // Check warnings section
        assert!(verbose.contains("Warnings (1):"));
        assert!(verbose.contains("Test warning"));

        // Check status
        assert!(verbose.contains("Status:"));
    }

    #[test]
    fn test_validation_report_verbose_includes_inheritance_chain() {
        let mut report = ValidationReport::new();
        report.inheritance_chain.add_source(
            ConfigLevel::Project,
            PathBuf::from("/project/.claude/settings.json"),
            true,
        );

        let verbose = report.verbose_report();
        assert!(verbose.contains("Inheritance chain:"));
        assert!(verbose.contains("[project]"));
    }

    #[test]
    fn test_validation_report_verbose_empty_inheritance_chain() {
        let report = ValidationReport::new();
        let verbose = report.verbose_report();
        assert!(verbose.contains("(no config files found)"));
    }

    // =========================================================================
    // ConfigValidator Tests - Valid Configurations
    // =========================================================================

    #[test]
    fn test_validator_valid_project_config_syntax() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a valid settings.json
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"respectGitignore": true, "permissions": {"allow": [], "deny": []}}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate();

        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(report.is_valid());
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_validator_empty_config_is_valid() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create an empty but valid JSON config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(project_dir.join(".claude/settings.json"), r#"{}"#).unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        // Empty config is valid (all fields have defaults)
        assert!(result.is_valid());
    }

    // =========================================================================
    // ConfigValidator Tests - Invalid JSON Syntax
    // =========================================================================

    #[test]
    fn test_validator_invalid_json_syntax() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create an invalid JSON file
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"respectGitignore": true, invalid json here"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate();

        assert!(result.is_ok()); // validate() returns Ok with errors in report
        let report = result.unwrap();
        assert!(!report.is_valid());
        assert!(!report.errors.is_empty());
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("syntax") || e.contains("parse")));
    }

    #[test]
    fn test_validator_invalid_json_reports_line_column() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create invalid JSON
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            "{\n  \"key\": invalid\n}",
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let report = validator.validate().unwrap();

        assert!(!report.is_valid());
        // Should include line and column info
        assert!(report.errors.iter().any(|e| e.contains("line")));
        assert!(report.errors.iter().any(|e| e.contains("column")));
    }

    // =========================================================================
    // ConfigValidator Tests - Inheritance Chain Resolution
    // =========================================================================

    #[test]
    fn test_validator_inheritance_chain_resolution() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create user config
        let user_config_dir = temp.path().join("user_config");
        std::fs::create_dir_all(&user_config_dir).unwrap();
        std::fs::write(
            user_config_dir.join("config.json"),
            r#"{"predictorWeights": {"commit_gap": 0.30}}"#,
        )
        .unwrap();

        // Create project config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir)
            .with_user_config_path(user_config_dir.join("config.json"));
        let result = validator.validate();

        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(report.is_valid());
        // Should have chain info
        assert!(!report.inheritance_chain.sources.is_empty());
    }

    #[test]
    fn test_validator_inheritance_chain_includes_all_levels() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create system config
        let system_config = temp.path().join("system.json");
        std::fs::write(&system_config, r#"{}"#).unwrap();

        // Create user config
        let user_config = temp.path().join("user.json");
        std::fs::write(&user_config, r#"{}"#).unwrap();

        // Create project config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(project_dir.join(".claude/settings.json"), r#"{}"#).unwrap();

        let validator = ConfigValidator::new(project_dir)
            .with_system_config_path(system_config)
            .with_user_config_path(user_config);
        let report = validator.validate().unwrap();

        // Should have all three levels in chain
        let levels: Vec<_> = report
            .inheritance_chain
            .sources
            .iter()
            .map(|s| s.level)
            .collect();
        assert!(levels.contains(&ConfigLevel::System));
        assert!(levels.contains(&ConfigLevel::User));
        assert!(levels.contains(&ConfigLevel::Project));
    }

    // =========================================================================
    // ConfigValidator Tests - Extends Reference Validation
    // =========================================================================

    #[test]
    fn test_validator_extends_reference_exists() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a shared config
        let config_dir = project_dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("team.json"),
            r#"{"predictorWeights": {"commit_gap": 0.40}}"#,
        )
        .unwrap();

        // Create project config that extends the shared config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/team.json"}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate();

        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(report.is_valid());
    }

    #[test]
    fn test_validator_extends_reference_missing() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create project config that extends a non-existent config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/nonexistent.json"}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate();

        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(!report.is_valid());
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("nonexistent") || e.contains("not found")));
    }

    #[test]
    fn test_validator_circular_extends_detected() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create circular extends
        let config_dir = project_dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("a.json"), r#"{"extends": "config/b.json"}"#).unwrap();
        std::fs::write(config_dir.join("b.json"), r#"{"extends": "config/a.json"}"#).unwrap();

        // Create project config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/a.json"}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| e.to_lowercase().contains("circular")));
    }

    #[test]
    fn test_validator_deep_extends_chain() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create a deep extends chain (a -> b -> c)
        let config_dir = project_dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("c.json"),
            r#"{"predictorWeights": {"commit_gap": 0.30}}"#,
        )
        .unwrap();
        std::fs::write(config_dir.join("b.json"), r#"{"extends": "config/c.json"}"#).unwrap();
        std::fs::write(config_dir.join("a.json"), r#"{"extends": "config/b.json"}"#).unwrap();

        // Create project config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"extends": "config/a.json"}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        assert!(result.is_valid());
    }

    // =========================================================================
    // ConfigValidator Tests - Predictor Weights Validation
    // =========================================================================

    #[test]
    fn test_validator_invalid_predictor_weights_negative() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create config with invalid predictor weight (negative value)
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"commit_gap": -0.5}}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate();

        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(!report.is_valid());
        assert!(report.errors.iter().any(|e| e.contains("negative")));
    }

    #[test]
    fn test_validator_invalid_predictor_weights_preset() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create config with invalid preset
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"preset": "invalid_preset"}}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.contains("preset")));
    }

    #[test]
    fn test_validator_valid_predictor_weights_preset() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create config with valid preset
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"predictorWeights": {"preset": "conservative"}}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        assert!(result.is_valid());
    }

    // =========================================================================
    // ConfigValidator Tests - mcp.json Validation
    // =========================================================================

    #[test]
    fn test_validator_checks_mcp_json() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create valid settings.json
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        // Create invalid mcp.json
        std::fs::write(project_dir.join(".claude/mcp.json"), r#"{"invalid json"#).unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.contains("mcp.json")));
    }

    #[test]
    fn test_validator_warns_missing_claude_md() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create valid settings.json but no CLAUDE.md
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(
            project_dir.join(".claude/settings.json"),
            r#"{"respectGitignore": true}"#,
        )
        .unwrap();

        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate().unwrap();

        // Should have a warning about missing CLAUDE.md
        assert!(result.warnings.iter().any(|w| w.contains("CLAUDE.md")));
    }

    #[test]
    fn test_validator_no_settings_uses_defaults() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create empty project dir (no .claude folder)
        let validator = ConfigValidator::new(project_dir);
        let result = validator.validate();

        assert!(result.is_ok());
        let report = result.unwrap();
        // No config files is valid (uses defaults)
        assert!(report.is_valid());
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("settings") || w.contains("defaults")));
    }

    // =========================================================================
    // ConfigValidator Tests - Builder Methods
    // =========================================================================

    #[test]
    fn test_validator_with_system_config_path() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create project config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(project_dir.join(".claude/settings.json"), r#"{}"#).unwrap();

        // Create system config
        let system_config = temp.path().join("system.json");
        std::fs::write(&system_config, r#"{"respectGitignore": false}"#).unwrap();

        let validator = ConfigValidator::new(project_dir).with_system_config_path(system_config);

        let report = validator.validate().unwrap();
        assert!(report.is_valid());
        // Check that system config is in inheritance chain
        assert!(report
            .inheritance_chain
            .sources
            .iter()
            .any(|s| s.level == ConfigLevel::System));
    }

    #[test]
    fn test_validator_chained_builder_methods() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path();

        // Create project config
        std::fs::create_dir_all(project_dir.join(".claude")).unwrap();
        std::fs::write(project_dir.join(".claude/settings.json"), r#"{}"#).unwrap();

        // Create system and user configs
        let system_config = temp.path().join("system.json");
        let user_config = temp.path().join("user.json");
        std::fs::write(&system_config, r#"{}"#).unwrap();
        std::fs::write(&user_config, r#"{}"#).unwrap();

        let validator = ConfigValidator::new(project_dir)
            .with_system_config_path(system_config)
            .with_user_config_path(user_config);

        let report = validator.validate().unwrap();
        assert!(report.is_valid());
    }
}
