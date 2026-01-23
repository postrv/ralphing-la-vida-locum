//! RuboCop Quality Gate Plugin for Ralph.
//!
//! This example plugin demonstrates how to create external quality gates
//! for the Ralph automation suite. It wraps the RuboCop Ruby linter and
//! integrates it as a Ralph quality gate.
//!
//! # Usage
//!
//! 1. Build the plugin as a shared library:
//!    ```bash
//!    cargo build --release
//!    ```
//!
//! 2. Copy to your plugins directory:
//!    ```bash
//!    mkdir -p ~/.ralph/plugins/rubocop-gate
//!    cp target/release/librubocop_gate.dylib ~/.ralph/plugins/rubocop-gate/
//!    cp plugin.toml ~/.ralph/plugins/rubocop-gate/
//!    ```
//!
//! 3. Ralph will automatically discover and load the plugin.
//!
//! # Plugin Development Guide
//!
//! This plugin serves as a template for creating your own quality gates.
//! Key steps:
//!
//! 1. Implement the [`QualityGate`] trait for your core check logic
//! 2. Implement the [`GatePlugin`] trait for metadata and configuration
//! 3. Export a `create_gate_plugin` function as the entry point
//!
//! # Example
//!
//! ```rust,ignore
//! use rubocop_gate::RubocopGatePlugin;
//! use ralph::quality::gates::QualityGate;
//! use std::path::Path;
//!
//! let plugin = RubocopGatePlugin::new();
//! let issues = plugin.run(Path::new("/path/to/ruby/project"))?;
//!
//! if issues.is_empty() {
//!     println!("RuboCop passed!");
//! } else {
//!     for issue in &issues {
//!         println!("{}", issue.format());
//!     }
//!     println!("{}", plugin.remediation(&issues));
//! }
//! ```

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use ralph::quality::gates::{GateIssue, IssueSeverity, QualityGate};
use ralph::quality::plugin::{GatePlugin, PluginMetadata};

// ============================================================================
// RuboCop JSON Output Structures
// ============================================================================

/// Top-level structure of RuboCop's JSON output.
#[derive(Debug, Deserialize)]
struct RubocopOutput {
    /// Metadata about the RuboCop run (present in JSON but unused).
    #[serde(rename = "metadata")]
    _metadata: RubocopMetadata,
    /// List of files with offenses.
    files: Vec<RubocopFile>,
    /// Summary statistics (present in JSON but unused).
    #[serde(rename = "summary")]
    _summary: RubocopSummary,
}

/// Metadata from RuboCop output (fields required for JSON schema completeness).
#[derive(Debug, Deserialize)]
struct RubocopMetadata {
    /// RuboCop version.
    #[serde(rename = "rubocop_version")]
    _rubocop_version: String,
    /// Ruby engine (e.g., "ruby").
    #[serde(rename = "ruby_engine")]
    _ruby_engine: String,
    /// Ruby version.
    #[serde(rename = "ruby_version")]
    _ruby_version: String,
}

/// A single file in RuboCop's output.
#[derive(Debug, Deserialize)]
struct RubocopFile {
    /// Path to the file.
    path: String,
    /// Offenses found in this file.
    offenses: Vec<RubocopOffense>,
}

/// A single offense (issue) from RuboCop.
#[derive(Debug, Deserialize)]
struct RubocopOffense {
    /// Severity level (convention, refactor, warning, error, fatal).
    severity: String,
    /// The offense message.
    message: String,
    /// The cop (rule) that triggered this offense.
    cop_name: String,
    /// Whether this offense is correctable.
    correctable: bool,
    /// Whether this offense was corrected (present in JSON but unused).
    #[serde(rename = "corrected")]
    _corrected: bool,
    /// Location information.
    location: RubocopLocation,
}

/// Location of an offense in the source code.
#[derive(Debug, Deserialize)]
struct RubocopLocation {
    /// Start line number (1-indexed).
    start_line: u32,
    /// Start column number (1-indexed).
    start_column: u32,
    /// Last line number (present in JSON but unused).
    #[serde(rename = "last_line")]
    _last_line: u32,
    /// Last column number (present in JSON but unused).
    #[serde(rename = "last_column")]
    _last_column: u32,
    /// Length of the offense (present in JSON but unused).
    #[serde(rename = "length")]
    _length: u32,
}

/// Summary statistics from RuboCop (fields required for JSON schema completeness).
#[derive(Debug, Deserialize)]
struct RubocopSummary {
    /// Number of offenses found.
    #[serde(rename = "offense_count")]
    _offense_count: u32,
    /// Number of files inspected.
    #[serde(rename = "inspected_file_count")]
    _inspected_file_count: u32,
    /// Target file count.
    #[serde(rename = "target_file_count")]
    _target_file_count: u32,
}

// ============================================================================
// RuboCop Gate Plugin Implementation
// ============================================================================

/// A quality gate plugin that runs RuboCop on Ruby projects.
///
/// This plugin:
/// 1. Runs `rubocop --format json` on the project
/// 2. Parses the JSON output into structured issues
/// 3. Converts RuboCop severities to Ralph's `IssueSeverity`
/// 4. Provides remediation guidance for found issues
///
/// # Required Tool
///
/// This plugin requires `rubocop` to be installed and available in PATH.
/// Install with: `gem install rubocop`
#[derive(Debug, Default, Clone)]
pub struct RubocopGatePlugin {
    /// Custom timeout for RuboCop execution.
    timeout: Option<Duration>,
}

impl RubocopGatePlugin {
    /// Create a new RuboCop gate plugin with default settings.
    ///
    /// # Example
    ///
    /// ```
    /// use rubocop_gate::RubocopGatePlugin;
    /// use ralph::quality::gates::QualityGate;
    ///
    /// let plugin = RubocopGatePlugin::new();
    /// assert_eq!(plugin.name(), "RuboCop");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom timeout for RuboCop execution.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Parse RuboCop JSON output into gate issues.
    ///
    /// # Arguments
    ///
    /// * `json_output` - The raw JSON output from `rubocop --format json`
    ///
    /// # Returns
    ///
    /// A vector of `GateIssue` structs representing the offenses found.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON cannot be parsed.
    pub fn parse_rubocop_output(json_output: &str) -> Result<Vec<GateIssue>> {
        let output: RubocopOutput =
            serde_json::from_str(json_output).context("Failed to parse RuboCop JSON output")?;

        let mut issues = Vec::new();

        for file in output.files {
            for offense in file.offenses {
                let severity = Self::map_severity(&offense.severity);

                let mut issue = GateIssue::new(severity, &offense.message)
                    .with_location(&file.path, offense.location.start_line)
                    .with_column(offense.location.start_column)
                    .with_code(&offense.cop_name);

                if offense.correctable {
                    issue = issue.with_suggestion("Run `rubocop -a` to auto-correct this offense");
                }

                issues.push(issue);
            }
        }

        Ok(issues)
    }

    /// Map RuboCop severity to Ralph's `IssueSeverity`.
    fn map_severity(rubocop_severity: &str) -> IssueSeverity {
        match rubocop_severity.to_lowercase().as_str() {
            "fatal" => IssueSeverity::Critical,
            "error" => IssueSeverity::Error,
            "warning" => IssueSeverity::Warning,
            "convention" | "refactor" => IssueSeverity::Info,
            _ => IssueSeverity::Warning,
        }
    }

    /// Run RuboCop and capture output.
    fn execute_rubocop(&self, project_dir: &Path) -> Result<String> {
        let output = Command::new("rubocop")
            .arg("--format")
            .arg("json")
            .current_dir(project_dir)
            .output()
            .context("Failed to execute rubocop command. Is rubocop installed?")?;

        // RuboCop returns non-zero exit code when offenses are found,
        // but still outputs valid JSON. We only care about the JSON output.
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            // If stdout is empty but stderr has content, report it
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                return Err(anyhow::anyhow!("RuboCop error: {}", stderr.trim()));
            }
            // No output at all - likely no Ruby files
            return Ok(r#"{"metadata":{"rubocop_version":"unknown","ruby_engine":"ruby","ruby_version":"unknown"},"files":[],"summary":{"offense_count":0,"target_file_count":0,"inspected_file_count":0}}"#.to_string());
        }

        Ok(stdout.to_string())
    }
}

impl QualityGate for RubocopGatePlugin {
    fn name(&self) -> &str {
        "RuboCop"
    }

    fn run(&self, project_dir: &Path) -> Result<Vec<GateIssue>> {
        tracing::info!("Running RuboCop quality gate on {}", project_dir.display());

        let json_output = self.execute_rubocop(project_dir)?;
        let issues = Self::parse_rubocop_output(&json_output)?;

        tracing::info!("RuboCop found {} issues", issues.len());
        Ok(issues)
    }

    fn remediation(&self, issues: &[GateIssue]) -> String {
        if issues.is_empty() {
            return "No RuboCop offenses found. Code follows Ruby style guidelines.".to_string();
        }

        let mut guidance = String::new();
        guidance.push_str("## RuboCop Remediation Guide\n\n");

        // Count issues by severity
        let errors = issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Error | IssueSeverity::Critical))
            .count();
        let warnings = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count();
        let conventions = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Info)
            .count();

        guidance.push_str(&format!(
            "Found {} total offenses: {} errors, {} warnings, {} conventions\n\n",
            issues.len(),
            errors,
            warnings,
            conventions
        ));

        // Quick fixes section
        guidance.push_str("### Quick Fixes\n\n");
        guidance.push_str("Many offenses can be auto-corrected:\n");
        guidance.push_str("```bash\n");
        guidance.push_str("# Safe auto-corrections only\n");
        guidance.push_str("bundle exec rubocop -a\n\n");
        guidance.push_str("# Include unsafe auto-corrections (review changes)\n");
        guidance.push_str("bundle exec rubocop -A\n");
        guidance.push_str("```\n\n");

        // Group issues by cop for targeted guidance
        let mut cops: std::collections::HashMap<&str, Vec<&GateIssue>> =
            std::collections::HashMap::new();
        for issue in issues {
            if let Some(ref code) = issue.code {
                cops.entry(code.as_str()).or_default().push(issue);
            }
        }

        if !cops.is_empty() {
            guidance.push_str("### Top Offending Cops\n\n");
            let mut cop_counts: Vec<_> = cops.iter().collect();
            cop_counts.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

            for (cop, cop_issues) in cop_counts.iter().take(5) {
                guidance.push_str(&format!("- **{}**: {} offenses\n", cop, cop_issues.len()));

                // Add specific guidance for common cops
                match **cop {
                    "Style/FrozenStringLiteralComment" => {
                        guidance.push_str(
                            "  Add `# frozen_string_literal: true` at the top of each file\n",
                        );
                    }
                    "Layout/TrailingWhitespace" => {
                        guidance.push_str("  Remove trailing whitespace from lines\n");
                    }
                    "Style/StringLiterals" => {
                        guidance.push_str("  Use consistent quote style (single or double)\n");
                    }
                    "Metrics/MethodLength" => {
                        guidance.push_str("  Extract logic into smaller helper methods\n");
                    }
                    "Lint/UnusedMethodArgument" => {
                        guidance.push_str(
                            "  Remove unused arguments or prefix with underscore (_)\n",
                        );
                    }
                    _ => {}
                }
            }
            guidance.push('\n');
        }

        // Documentation link
        guidance.push_str("### Documentation\n\n");
        guidance.push_str(
            "See [RuboCop documentation](https://docs.rubocop.org/) for detailed cop descriptions.\n",
        );

        guidance
    }

    fn required_tool(&self) -> Option<&str> {
        Some("rubocop")
    }
}

impl GatePlugin for RubocopGatePlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new("rubocop-gate", "1.0.0", "Ralph Community")
            .with_description("Ruby linting via RuboCop")
            .with_license("MIT")
            .with_homepage("https://github.com/example/ralph-rubocop-gate")
    }

    fn timeout(&self) -> Duration {
        self.timeout.unwrap_or_else(|| Duration::from_secs(120))
    }

    fn on_load(&self) -> Result<()> {
        // Verify rubocop is available
        if which::which("rubocop").is_err() {
            tracing::warn!(
                "RuboCop not found in PATH. Install with: gem install rubocop"
            );
        }
        Ok(())
    }
}

// ============================================================================
// Plugin Entry Point
// ============================================================================

/// Entry point for dynamic loading.
///
/// This function is called by Ralph's plugin loader to create an instance
/// of the plugin. The function signature must be:
///
/// ```rust,ignore
/// #[no_mangle]
/// pub extern "C" fn create_gate_plugin() -> *mut dyn GatePlugin
/// ```
///
/// The plugin loader will call this function and take ownership of the
/// returned pointer using `Box::from_raw`.
///
/// # Safety
///
/// This uses a raw pointer to a trait object across FFI. While Rust warns about
/// this (`improper_ctypes_definitions`), it is safe when:
/// 1. Both plugin and host are compiled with the same Rust toolchain
/// 2. The host correctly calls `Box::from_raw` to take ownership
///
/// This is the standard pattern for Rust plugin systems and is explicitly
/// documented in Ralph's plugin API.
#[no_mangle]
#[allow(improper_ctypes_definitions)] // Safe for Rust-to-Rust plugin systems
pub extern "C" fn create_gate_plugin() -> *mut dyn GatePlugin {
    let plugin = RubocopGatePlugin::new();
    Box::into_raw(Box::new(plugin))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Test: RuboCop plugin compiles as shared library
    // =========================================================================
    // This is verified by the build process itself - if the crate compiles
    // with crate-type = ["cdylib"], the test passes.

    #[test]
    fn test_plugin_can_be_created() {
        let plugin = RubocopGatePlugin::new();
        assert_eq!(plugin.name(), "RuboCop");
    }

    #[test]
    fn test_create_gate_plugin_entry_point() {
        // Test the entry point function
        let ptr = create_gate_plugin();
        assert!(!ptr.is_null());

        // Clean up - convert back to Box so it's properly dropped
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }

    // =========================================================================
    // Test: Plugin runs `rubocop` command
    // =========================================================================

    #[test]
    fn test_required_tool_is_rubocop() {
        let plugin = RubocopGatePlugin::new();
        assert_eq!(plugin.required_tool(), Some("rubocop"));
    }

    #[test]
    fn test_plugin_implements_quality_gate() {
        let plugin = RubocopGatePlugin::new();

        // Verify the plugin implements QualityGate
        let gate: &dyn QualityGate = &plugin;
        assert_eq!(gate.name(), "RuboCop");
        assert!(gate.is_blocking());
    }

    // =========================================================================
    // Test: Plugin parses RuboCop JSON output
    // =========================================================================

    #[test]
    fn test_parse_empty_output() {
        let json = r#"{
            "metadata": {
                "rubocop_version": "1.50.0",
                "ruby_engine": "ruby",
                "ruby_version": "3.2.0"
            },
            "files": [],
            "summary": {
                "offense_count": 0,
                "target_file_count": 0,
                "inspected_file_count": 0
            }
        }"#;

        let issues = RubocopGatePlugin::parse_rubocop_output(json).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_parse_single_offense() {
        let json = r#"{
            "metadata": {
                "rubocop_version": "1.50.0",
                "ruby_engine": "ruby",
                "ruby_version": "3.2.0"
            },
            "files": [{
                "path": "lib/example.rb",
                "offenses": [{
                    "severity": "convention",
                    "message": "Missing frozen string literal comment.",
                    "cop_name": "Style/FrozenStringLiteralComment",
                    "correctable": true,
                    "corrected": false,
                    "location": {
                        "start_line": 1,
                        "start_column": 1,
                        "last_line": 1,
                        "last_column": 1,
                        "length": 1
                    }
                }]
            }],
            "summary": {
                "offense_count": 1,
                "target_file_count": 1,
                "inspected_file_count": 1
            }
        }"#;

        let issues = RubocopGatePlugin::parse_rubocop_output(json).unwrap();
        assert_eq!(issues.len(), 1);

        let issue = &issues[0];
        assert_eq!(issue.severity, IssueSeverity::Info);
        assert!(issue.message.contains("frozen string literal"));
        assert_eq!(issue.code, Some("Style/FrozenStringLiteralComment".to_string()));
        assert_eq!(issue.file, Some(std::path::PathBuf::from("lib/example.rb")));
        assert_eq!(issue.line, Some(1));
        assert_eq!(issue.column, Some(1));
    }

    #[test]
    fn test_parse_multiple_offenses() {
        let json = r#"{
            "metadata": {
                "rubocop_version": "1.50.0",
                "ruby_engine": "ruby",
                "ruby_version": "3.2.0"
            },
            "files": [{
                "path": "lib/example.rb",
                "offenses": [
                    {
                        "severity": "error",
                        "message": "Syntax error",
                        "cop_name": "Lint/Syntax",
                        "correctable": false,
                        "corrected": false,
                        "location": {
                            "start_line": 5,
                            "start_column": 10,
                            "last_line": 5,
                            "last_column": 15,
                            "length": 5
                        }
                    },
                    {
                        "severity": "warning",
                        "message": "Unused variable",
                        "cop_name": "Lint/UselessAssignment",
                        "correctable": false,
                        "corrected": false,
                        "location": {
                            "start_line": 10,
                            "start_column": 3,
                            "last_line": 10,
                            "last_column": 8,
                            "length": 5
                        }
                    }
                ]
            }],
            "summary": {
                "offense_count": 2,
                "target_file_count": 1,
                "inspected_file_count": 1
            }
        }"#;

        let issues = RubocopGatePlugin::parse_rubocop_output(json).unwrap();
        assert_eq!(issues.len(), 2);

        // First issue should be error severity
        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert_eq!(issues[0].line, Some(5));

        // Second issue should be warning severity
        assert_eq!(issues[1].severity, IssueSeverity::Warning);
        assert_eq!(issues[1].line, Some(10));
    }

    #[test]
    fn test_parse_multiple_files() {
        let json = r#"{
            "metadata": {
                "rubocop_version": "1.50.0",
                "ruby_engine": "ruby",
                "ruby_version": "3.2.0"
            },
            "files": [
                {
                    "path": "lib/foo.rb",
                    "offenses": [{
                        "severity": "convention",
                        "message": "Issue in foo",
                        "cop_name": "Style/Something",
                        "correctable": true,
                        "corrected": false,
                        "location": {
                            "start_line": 1,
                            "start_column": 1,
                            "last_line": 1,
                            "last_column": 1,
                            "length": 1
                        }
                    }]
                },
                {
                    "path": "lib/bar.rb",
                    "offenses": [{
                        "severity": "convention",
                        "message": "Issue in bar",
                        "cop_name": "Style/Something",
                        "correctable": true,
                        "corrected": false,
                        "location": {
                            "start_line": 2,
                            "start_column": 1,
                            "last_line": 2,
                            "last_column": 1,
                            "length": 1
                        }
                    }]
                }
            ],
            "summary": {
                "offense_count": 2,
                "target_file_count": 2,
                "inspected_file_count": 2
            }
        }"#;

        let issues = RubocopGatePlugin::parse_rubocop_output(json).unwrap();
        assert_eq!(issues.len(), 2);

        assert_eq!(issues[0].file, Some(std::path::PathBuf::from("lib/foo.rb")));
        assert_eq!(issues[1].file, Some(std::path::PathBuf::from("lib/bar.rb")));
    }

    #[test]
    fn test_parse_invalid_json() {
        let invalid_json = "not valid json";
        let result = RubocopGatePlugin::parse_rubocop_output(invalid_json);
        assert!(result.is_err());
    }

    // =========================================================================
    // Test: Plugin produces GateIssue list
    // =========================================================================

    #[test]
    fn test_severity_mapping_fatal() {
        assert_eq!(
            RubocopGatePlugin::map_severity("fatal"),
            IssueSeverity::Critical
        );
    }

    #[test]
    fn test_severity_mapping_error() {
        assert_eq!(
            RubocopGatePlugin::map_severity("error"),
            IssueSeverity::Error
        );
    }

    #[test]
    fn test_severity_mapping_warning() {
        assert_eq!(
            RubocopGatePlugin::map_severity("warning"),
            IssueSeverity::Warning
        );
    }

    #[test]
    fn test_severity_mapping_convention() {
        assert_eq!(
            RubocopGatePlugin::map_severity("convention"),
            IssueSeverity::Info
        );
    }

    #[test]
    fn test_severity_mapping_refactor() {
        assert_eq!(
            RubocopGatePlugin::map_severity("refactor"),
            IssueSeverity::Info
        );
    }

    #[test]
    fn test_severity_mapping_unknown() {
        assert_eq!(
            RubocopGatePlugin::map_severity("unknown"),
            IssueSeverity::Warning
        );
    }

    #[test]
    fn test_correctable_offense_has_suggestion() {
        let json = r#"{
            "metadata": {
                "rubocop_version": "1.50.0",
                "ruby_engine": "ruby",
                "ruby_version": "3.2.0"
            },
            "files": [{
                "path": "lib/example.rb",
                "offenses": [{
                    "severity": "convention",
                    "message": "Use double quotes",
                    "cop_name": "Style/StringLiterals",
                    "correctable": true,
                    "corrected": false,
                    "location": {
                        "start_line": 1,
                        "start_column": 1,
                        "last_line": 1,
                        "last_column": 5,
                        "length": 4
                    }
                }]
            }],
            "summary": {
                "offense_count": 1,
                "target_file_count": 1,
                "inspected_file_count": 1
            }
        }"#;

        let issues = RubocopGatePlugin::parse_rubocop_output(json).unwrap();
        assert!(issues[0].suggestion.is_some());
        assert!(issues[0].suggestion.as_ref().unwrap().contains("rubocop -a"));
    }

    // =========================================================================
    // Test: Plugin provides remediation guidance
    // =========================================================================

    #[test]
    fn test_remediation_empty_issues() {
        let plugin = RubocopGatePlugin::new();
        let guidance = plugin.remediation(&[]);
        assert!(guidance.contains("No RuboCop offenses found"));
    }

    #[test]
    fn test_remediation_with_issues() {
        let plugin = RubocopGatePlugin::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Error, "Syntax error")
                .with_code("Lint/Syntax"),
            GateIssue::new(IssueSeverity::Warning, "Unused variable")
                .with_code("Lint/UselessAssignment"),
            GateIssue::new(IssueSeverity::Info, "Missing comment")
                .with_code("Style/FrozenStringLiteralComment"),
        ];

        let guidance = plugin.remediation(&issues);

        // Should have sections
        assert!(guidance.contains("## RuboCop Remediation Guide"));
        assert!(guidance.contains("### Quick Fixes"));
        assert!(guidance.contains("rubocop -a"));
        assert!(guidance.contains("### Top Offending Cops"));
        assert!(guidance.contains("### Documentation"));

        // Should count issues correctly
        assert!(guidance.contains("3 total offenses"));
        assert!(guidance.contains("1 errors"));
        assert!(guidance.contains("1 warnings"));
        assert!(guidance.contains("1 conventions"));
    }

    #[test]
    fn test_remediation_specific_cop_guidance() {
        let plugin = RubocopGatePlugin::new();
        let issues = vec![
            GateIssue::new(IssueSeverity::Info, "Missing frozen string literal")
                .with_code("Style/FrozenStringLiteralComment"),
        ];

        let guidance = plugin.remediation(&issues);
        assert!(guidance.contains("frozen_string_literal: true"));
    }

    // =========================================================================
    // Test: Plugin implements GatePlugin trait
    // =========================================================================

    #[test]
    fn test_plugin_metadata() {
        let plugin = RubocopGatePlugin::new();
        let meta = plugin.metadata();

        assert_eq!(meta.name, "rubocop-gate");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.author, "Ralph Community");
        assert!(meta.description.is_some());
        assert!(meta.description.unwrap().contains("RuboCop"));
        assert_eq!(meta.license, Some("MIT".to_string()));
    }

    #[test]
    fn test_plugin_default_timeout() {
        let plugin = RubocopGatePlugin::new();
        assert_eq!(plugin.timeout(), Duration::from_secs(120));
    }

    #[test]
    fn test_plugin_custom_timeout() {
        let plugin = RubocopGatePlugin::new().with_timeout(Duration::from_secs(60));
        assert_eq!(plugin.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_plugin_on_load_succeeds() {
        let plugin = RubocopGatePlugin::new();
        // on_load should not fail even if rubocop isn't installed
        assert!(plugin.on_load().is_ok());
    }

    // =========================================================================
    // Test: Plugin extends QualityGate trait (from GatePlugin)
    // =========================================================================

    #[test]
    fn test_gate_plugin_is_quality_gate() {
        let plugin = RubocopGatePlugin::new();

        // Can use as both QualityGate and GatePlugin
        let quality_gate: &dyn QualityGate = &plugin;
        let gate_plugin: &dyn GatePlugin = &plugin;

        assert_eq!(quality_gate.name(), "RuboCop");
        assert_eq!(gate_plugin.metadata().name, "rubocop-gate");
    }
}
