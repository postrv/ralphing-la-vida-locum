//! CCG (Compact Code Graph) data structures and loading.
//!
//! This module provides structures for working with CCG data from narsil-mcp.
//! CCG provides layered code intelligence:
//!
//! - L0 (Manifest): Basic repository metadata (~1-2KB)
//! - L1 (Architecture): Module hierarchy and public API (~10-50KB)
//! - L2 (Symbol Index): Complete symbol index with call edges (larger)
//!
//! When narsil-mcp is unavailable, all methods return None gracefully.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// L0: CCG Manifest
// ============================================================================

/// Layer 0 - CCG Manifest containing basic repository metadata.
///
/// This is the smallest layer (~1-2KB) and always fits in context.
/// Contains essential information for understanding the repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CcgManifest {
    /// Repository name.
    pub name: String,

    /// Repository path.
    pub path: PathBuf,

    /// Primary programming language.
    pub primary_language: Option<String>,

    /// Language breakdown by file count.
    pub languages: HashMap<String, LanguageStats>,

    /// Total number of files indexed.
    pub file_count: u32,

    /// Total number of symbols indexed.
    pub symbol_count: u32,

    /// Security summary.
    pub security_summary: SecuritySummary,

    /// Timestamp when the CCG was generated.
    pub generated_at: Option<String>,

    /// CCG schema version.
    pub schema_version: String,
}

impl Default for CcgManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: PathBuf::new(),
            primary_language: None,
            languages: HashMap::new(),
            file_count: 0,
            symbol_count: 0,
            security_summary: SecuritySummary::default(),
            generated_at: None,
            schema_version: "1.0".to_string(),
        }
    }
}

impl CcgManifest {
    /// Create a new CCG manifest with the given name and path.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            ..Default::default()
        }
    }

    /// Set the primary language.
    #[must_use]
    pub fn with_primary_language(mut self, language: impl Into<String>) -> Self {
        self.primary_language = Some(language.into());
        self
    }

    /// Add language statistics.
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>, stats: LanguageStats) -> Self {
        self.languages.insert(language.into(), stats);
        self
    }

    /// Set file and symbol counts.
    #[must_use]
    pub fn with_counts(mut self, files: u32, symbols: u32) -> Self {
        self.file_count = files;
        self.symbol_count = symbols;
        self
    }

    /// Set the security summary.
    #[must_use]
    pub fn with_security_summary(mut self, summary: SecuritySummary) -> Self {
        self.security_summary = summary;
        self
    }

    /// Check if the repository has any critical or high severity issues.
    pub fn has_blocking_issues(&self) -> bool {
        self.security_summary.critical > 0 || self.security_summary.high > 0
    }

    /// Get the total number of security issues.
    pub fn total_security_issues(&self) -> u32 {
        self.security_summary.critical
            + self.security_summary.high
            + self.security_summary.medium
            + self.security_summary.low
    }
}

/// Statistics for a single programming language.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LanguageStats {
    /// Number of files.
    pub file_count: u32,

    /// Total lines of code.
    pub lines_of_code: u32,

    /// Number of symbols (functions, types, etc.).
    pub symbol_count: u32,
}

impl LanguageStats {
    /// Create new language statistics.
    pub fn new(files: u32, lines: u32, symbols: u32) -> Self {
        Self {
            file_count: files,
            lines_of_code: lines,
            symbol_count: symbols,
        }
    }
}

/// Summary of security findings by severity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SecuritySummary {
    /// Critical severity issues.
    pub critical: u32,

    /// High severity issues.
    pub high: u32,

    /// Medium severity issues.
    pub medium: u32,

    /// Low severity issues.
    pub low: u32,
}

impl SecuritySummary {
    /// Create a new security summary.
    pub fn new(critical: u32, high: u32, medium: u32, low: u32) -> Self {
        Self {
            critical,
            high,
            medium,
            low,
        }
    }

    /// Check if there are any blocking issues (critical or high).
    pub fn has_blocking(&self) -> bool {
        self.critical > 0 || self.high > 0
    }

    /// Get total issue count.
    pub fn total(&self) -> u32 {
        self.critical + self.high + self.medium + self.low
    }
}

// ============================================================================
// L1: CCG Architecture
// ============================================================================

/// Layer 1 - CCG Architecture containing module hierarchy and public API.
///
/// This layer is larger (~10-50KB) but still fits in context.
/// Contains structural information about the codebase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CcgArchitecture {
    /// Module hierarchy.
    pub modules: Vec<Module>,

    /// Public API symbols (exported functions, types, etc.).
    pub public_api: Vec<PublicSymbol>,

    /// Entry points (main functions, exported handlers, etc.).
    pub entry_points: Vec<EntryPoint>,

    /// Cross-module dependencies.
    pub dependencies: Vec<ModuleDependency>,
}

impl CcgArchitecture {
    /// Create a new empty architecture.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a module.
    #[must_use]
    pub fn with_module(mut self, module: Module) -> Self {
        self.modules.push(module);
        self
    }

    /// Add a public API symbol.
    #[must_use]
    pub fn with_public_symbol(mut self, symbol: PublicSymbol) -> Self {
        self.public_api.push(symbol);
        self
    }

    /// Add an entry point.
    #[must_use]
    pub fn with_entry_point(mut self, entry_point: EntryPoint) -> Self {
        self.entry_points.push(entry_point);
        self
    }

    /// Add a module dependency.
    #[must_use]
    pub fn with_dependency(mut self, dependency: ModuleDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Get all module names.
    pub fn module_names(&self) -> Vec<&str> {
        self.modules.iter().map(|m| m.name.as_str()).collect()
    }

    /// Find a module by name.
    pub fn find_module(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name == name)
    }

    /// Get public API symbols for a specific module.
    pub fn public_api_for_module(&self, module: &str) -> Vec<&PublicSymbol> {
        self.public_api
            .iter()
            .filter(|s| s.module.as_deref() == Some(module))
            .collect()
    }
}

/// A module in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Module {
    /// Module name (e.g., "narsil", "prompt::builder").
    pub name: String,

    /// Path to the module.
    pub path: PathBuf,

    /// Module visibility.
    pub visibility: Visibility,

    /// Child modules.
    pub children: Vec<String>,

    /// Brief description (from doc comments).
    pub description: Option<String>,
}

impl Module {
    /// Create a new module.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            visibility: Visibility::Private,
            children: Vec::new(),
            description: None,
        }
    }

    /// Set visibility.
    #[must_use]
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Add a child module.
    #[must_use]
    pub fn with_child(mut self, child: impl Into<String>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Set description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Visibility of a symbol or module.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Public (accessible from anywhere).
    Public,

    /// Crate-visible (pub(crate)).
    Crate,

    /// Module-visible (pub(super) or private).
    #[default]
    Private,
}

/// A public API symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicSymbol {
    /// Symbol name.
    pub name: String,

    /// Fully qualified name.
    pub qualified_name: String,

    /// Symbol kind.
    pub kind: SymbolKind,

    /// Module containing this symbol.
    pub module: Option<String>,

    /// Brief description.
    pub description: Option<String>,

    /// Function signature (if applicable).
    pub signature: Option<String>,
}

impl PublicSymbol {
    /// Create a new public symbol.
    pub fn new(name: impl Into<String>, kind: SymbolKind) -> Self {
        let name = name.into();
        Self {
            qualified_name: name.clone(),
            name,
            kind,
            module: None,
            description: None,
            signature: None,
        }
    }

    /// Set the qualified name.
    #[must_use]
    pub fn with_qualified_name(mut self, qualified_name: impl Into<String>) -> Self {
        self.qualified_name = qualified_name.into();
        self
    }

    /// Set the module.
    #[must_use]
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the signature.
    #[must_use]
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }
}

/// Kind of symbol.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    /// Function.
    Function,
    /// Method.
    Method,
    /// Struct.
    Struct,
    /// Enum.
    Enum,
    /// Trait.
    Trait,
    /// Type alias.
    Type,
    /// Constant.
    Const,
    /// Static variable.
    Static,
    /// Module.
    Module,
    /// Macro.
    Macro,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "fn"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Const => write!(f, "const"),
            SymbolKind::Static => write!(f, "static"),
            SymbolKind::Module => write!(f, "mod"),
            SymbolKind::Macro => write!(f, "macro"),
        }
    }
}

/// An entry point in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntryPoint {
    /// Entry point name.
    pub name: String,

    /// Entry point kind.
    pub kind: EntryPointKind,

    /// File containing the entry point.
    pub file: PathBuf,

    /// Line number.
    pub line: Option<u32>,
}

impl EntryPoint {
    /// Create a new entry point.
    pub fn new(name: impl Into<String>, kind: EntryPointKind, file: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            kind,
            file: file.into(),
            line: None,
        }
    }

    /// Set the line number.
    #[must_use]
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }
}

/// Kind of entry point.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryPointKind {
    /// Main function.
    Main,
    /// Binary entry point.
    Binary,
    /// Library entry point.
    Library,
    /// Test entry point.
    Test,
    /// Benchmark entry point.
    Benchmark,
    /// HTTP handler.
    HttpHandler,
    /// CLI command.
    CliCommand,
}

impl std::fmt::Display for EntryPointKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryPointKind::Main => write!(f, "main"),
            EntryPointKind::Binary => write!(f, "binary"),
            EntryPointKind::Library => write!(f, "library"),
            EntryPointKind::Test => write!(f, "test"),
            EntryPointKind::Benchmark => write!(f, "benchmark"),
            EntryPointKind::HttpHandler => write!(f, "http_handler"),
            EntryPointKind::CliCommand => write!(f, "cli_command"),
        }
    }
}

/// A dependency between modules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleDependency {
    /// Source module.
    pub from: String,

    /// Target module.
    pub to: String,

    /// Dependency kind.
    pub kind: DependencyKind,
}

impl ModuleDependency {
    /// Create a new module dependency.
    pub fn new(from: impl Into<String>, to: impl Into<String>, kind: DependencyKind) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind,
        }
    }
}

/// Kind of dependency between modules.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// Uses types from the module.
    Uses,
    /// Calls functions in the module.
    Calls,
    /// Implements traits from the module.
    Implements,
    /// Extends types from the module.
    Extends,
}

// ============================================================================
// CCG Cache
// ============================================================================

/// Cache for CCG data within a session.
///
/// Avoids repeated calls to narsil-mcp for the same data.
#[derive(Debug, Default)]
pub struct CcgCache {
    /// Cached manifest.
    manifest: Option<CcgManifest>,

    /// Cached architecture.
    architecture: Option<CcgArchitecture>,
}

impl CcgCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get cached manifest.
    pub fn manifest(&self) -> Option<&CcgManifest> {
        self.manifest.as_ref()
    }

    /// Set cached manifest.
    pub fn set_manifest(&mut self, manifest: CcgManifest) {
        self.manifest = Some(manifest);
    }

    /// Get cached architecture.
    pub fn architecture(&self) -> Option<&CcgArchitecture> {
        self.architecture.as_ref()
    }

    /// Set cached architecture.
    pub fn set_architecture(&mut self, architecture: CcgArchitecture) {
        self.architecture = Some(architecture);
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.manifest = None;
        self.architecture = None;
    }

    /// Check if manifest is cached.
    pub fn has_manifest(&self) -> bool {
        self.manifest.is_some()
    }

    /// Check if architecture is cached.
    pub fn has_architecture(&self) -> bool {
        self.architecture.is_some()
    }
}

// ============================================================================
// L2: CCG Constraints
// ============================================================================

/// Kind of constraint that can be applied to code.
///
/// CCG constraints define rules that code must follow. These are typically
/// specified in a project's CCG configuration and enforced during development.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintKind {
    /// Prohibits direct calls between specified functions/modules.
    /// Used to enforce architectural boundaries.
    NoDirectCalls,

    /// Limits cyclomatic complexity of functions.
    MaxComplexity,

    /// Limits the number of parameters a function can have.
    MaxParameters,

    /// Limits the lines of code in a function.
    MaxLines,

    /// Requires functions to have documentation.
    RequireDocs,

    /// Prohibits use of specific functions or types.
    Prohibited,

    /// Requires specific error handling patterns.
    ErrorHandling,

    /// Requires specific test coverage.
    TestCoverage,

    /// Custom constraint with arbitrary rule.
    Custom,
}

impl std::fmt::Display for ConstraintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintKind::NoDirectCalls => write!(f, "noDirectCalls"),
            ConstraintKind::MaxComplexity => write!(f, "maxComplexity"),
            ConstraintKind::MaxParameters => write!(f, "maxParameters"),
            ConstraintKind::MaxLines => write!(f, "maxLines"),
            ConstraintKind::RequireDocs => write!(f, "requireDocs"),
            ConstraintKind::Prohibited => write!(f, "prohibited"),
            ConstraintKind::ErrorHandling => write!(f, "errorHandling"),
            ConstraintKind::TestCoverage => write!(f, "testCoverage"),
            ConstraintKind::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for ConstraintKind {
    type Err = ();

    /// Parse a constraint kind from a string.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::narsil::ConstraintKind;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(ConstraintKind::from_str("noDirectCalls"), Ok(ConstraintKind::NoDirectCalls));
    /// assert_eq!(ConstraintKind::from_str("maxComplexity"), Ok(ConstraintKind::MaxComplexity));
    /// assert!(ConstraintKind::from_str("unknown").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "noDirectCalls" | "no_direct_calls" => Ok(ConstraintKind::NoDirectCalls),
            "maxComplexity" | "max_complexity" => Ok(ConstraintKind::MaxComplexity),
            "maxParameters" | "max_parameters" => Ok(ConstraintKind::MaxParameters),
            "maxLines" | "max_lines" => Ok(ConstraintKind::MaxLines),
            "requireDocs" | "require_docs" => Ok(ConstraintKind::RequireDocs),
            "prohibited" => Ok(ConstraintKind::Prohibited),
            "errorHandling" | "error_handling" => Ok(ConstraintKind::ErrorHandling),
            "testCoverage" | "test_coverage" => Ok(ConstraintKind::TestCoverage),
            "custom" => Ok(ConstraintKind::Custom),
            _ => Err(()),
        }
    }
}

/// Severity level for constraint violations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConstraintSeverity {
    /// Informational - not a blocking issue.
    Info,

    /// Warning - should be addressed but not blocking.
    #[default]
    Warning,

    /// Error - must be fixed before commit.
    Error,

    /// Critical - blocks all progress until resolved.
    Critical,
}

impl std::fmt::Display for ConstraintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintSeverity::Info => write!(f, "info"),
            ConstraintSeverity::Warning => write!(f, "warning"),
            ConstraintSeverity::Error => write!(f, "error"),
            ConstraintSeverity::Critical => write!(f, "critical"),
        }
    }
}

/// A CCG constraint specification.
///
/// Constraints define rules that code must follow. They can target specific
/// symbols, modules, or the entire codebase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CcgConstraint {
    /// Unique identifier for this constraint.
    pub id: String,

    /// Kind of constraint.
    pub kind: ConstraintKind,

    /// Human-readable description.
    pub description: String,

    /// Severity of violations.
    pub severity: ConstraintSeverity,

    /// Target symbols (function names, type names, module paths).
    /// Empty means applies to all code.
    pub targets: Vec<String>,

    /// Constraint value (e.g., max complexity number).
    pub value: Option<ConstraintValue>,

    /// Whether this constraint is currently active.
    pub enabled: bool,
}

impl CcgConstraint {
    /// Create a new constraint.
    pub fn new(
        id: impl Into<String>,
        kind: ConstraintKind,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            description: description.into(),
            severity: ConstraintSeverity::Warning,
            targets: Vec::new(),
            value: None,
            enabled: true,
        }
    }

    /// Set the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: ConstraintSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Add a target.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.targets.push(target.into());
        self
    }

    /// Set targets.
    #[must_use]
    pub fn with_targets(mut self, targets: Vec<String>) -> Self {
        self.targets = targets;
        self
    }

    /// Set the constraint value.
    #[must_use]
    pub fn with_value(mut self, value: ConstraintValue) -> Self {
        self.value = Some(value);
        self
    }

    /// Disable this constraint.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Check if this constraint applies to a given target.
    pub fn applies_to(&self, target: &str) -> bool {
        if self.targets.is_empty() {
            return true; // Applies to all
        }
        self.targets.iter().any(|t| {
            t == target
                || target.starts_with(&format!("{}::", t))
                || t.ends_with("*") && target.starts_with(t.trim_end_matches('*'))
        })
    }

    /// Get a concise prompt representation.
    pub fn to_prompt_string(&self) -> String {
        let target_str = if self.targets.is_empty() {
            "all code".to_string()
        } else if self.targets.len() == 1 {
            format!("`{}`", self.targets[0])
        } else {
            format!("{} targets", self.targets.len())
        };

        let value_str = self
            .value
            .as_ref()
            .map_or(String::new(), |v| format!(" ({})", v));

        format!(
            "- **{}** [{}]: {} → {}{}",
            self.kind, self.severity, self.description, target_str, value_str
        )
    }
}

/// Value associated with a constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ConstraintValue {
    /// Numeric value (e.g., max complexity = 10).
    Number(u32),

    /// String value (e.g., prohibited function name).
    String(String),

    /// List of strings (e.g., prohibited functions).
    List(Vec<String>),

    /// Boolean flag.
    Bool(bool),
}

impl std::fmt::Display for ConstraintValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintValue::Number(n) => write!(f, "{}", n),
            ConstraintValue::String(s) => write!(f, "{}", s),
            ConstraintValue::List(items) => write!(f, "[{}]", items.join(", ")),
            ConstraintValue::Bool(b) => write!(f, "{}", b),
        }
    }
}

/// A set of constraints with lookup and validation capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ConstraintSet {
    /// All constraints in the set.
    constraints: Vec<CcgConstraint>,
}

impl ConstraintSet {
    /// Create an empty constraint set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a constraint to the set.
    #[must_use]
    pub fn with_constraint(mut self, constraint: CcgConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Add multiple constraints.
    pub fn add(&mut self, constraint: CcgConstraint) {
        self.constraints.push(constraint);
    }

    /// Get all constraints.
    pub fn all(&self) -> &[CcgConstraint] {
        &self.constraints
    }

    /// Get enabled constraints only.
    pub fn enabled(&self) -> impl Iterator<Item = &CcgConstraint> {
        self.constraints.iter().filter(|c| c.enabled)
    }

    /// Find constraints that apply to a specific target.
    pub fn for_target(&self, target: &str) -> Vec<&CcgConstraint> {
        self.constraints
            .iter()
            .filter(|c| c.enabled && c.applies_to(target))
            .collect()
    }

    /// Find constraints by kind.
    pub fn by_kind(&self, kind: ConstraintKind) -> Vec<&CcgConstraint> {
        self.constraints
            .iter()
            .filter(|c| c.enabled && c.kind == kind)
            .collect()
    }

    /// Check if any blocking constraints exist (error or critical severity).
    pub fn has_blocking(&self) -> bool {
        self.constraints.iter().any(|c| {
            c.enabled
                && (c.severity == ConstraintSeverity::Error
                    || c.severity == ConstraintSeverity::Critical)
        })
    }

    /// Count constraints by severity.
    pub fn count_by_severity(&self, severity: ConstraintSeverity) -> usize {
        self.constraints
            .iter()
            .filter(|c| c.enabled && c.severity == severity)
            .count()
    }

    /// Get total count of enabled constraints.
    pub fn len(&self) -> usize {
        self.constraints.iter().filter(|c| c.enabled).count()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Generate a prompt section summarizing constraints.
    pub fn to_prompt_section(&self) -> String {
        let enabled: Vec<_> = self.enabled().collect();
        if enabled.is_empty() {
            return String::new();
        }

        let mut lines = vec!["### Active Constraints".to_string(), String::new()];

        for constraint in enabled.iter().take(10) {
            lines.push(constraint.to_prompt_string());
        }

        if enabled.len() > 10 {
            lines.push(format!(
                "\n*...and {} more constraints*",
                enabled.len() - 10
            ));
        }

        lines.push(String::new());
        lines.join("\n")
    }

    /// Parse constraints from JSON.
    ///
    /// Expected format:
    /// ```json
    /// {
    ///   "constraints": [
    ///     {
    ///       "id": "max-complexity",
    ///       "kind": "maxComplexity",
    ///       "description": "Functions should have low complexity",
    ///       "severity": "warning",
    ///       "value": 10
    ///     }
    ///   ]
    /// }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct ConstraintSetJson {
            constraints: Vec<CcgConstraint>,
        }

        let parsed: ConstraintSetJson = serde_json::from_str(json)?;
        Ok(Self {
            constraints: parsed.constraints,
        })
    }

    /// Validate all constraints have valid syntax.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        for constraint in &self.constraints {
            // Check for empty ID
            if constraint.id.is_empty() {
                errors.push("Constraint has empty ID".to_string());
            }

            // Check for empty description
            if constraint.description.is_empty() {
                errors.push(format!(
                    "Constraint '{}' has empty description",
                    constraint.id
                ));
            }

            // Check value requirements for specific kinds
            match constraint.kind {
                ConstraintKind::MaxComplexity
                | ConstraintKind::MaxParameters
                | ConstraintKind::MaxLines => match &constraint.value {
                    Some(ConstraintValue::Number(_)) => {}
                    _ => errors.push(format!(
                        "Constraint '{}' ({}) requires a numeric value",
                        constraint.id, constraint.kind
                    )),
                },
                ConstraintKind::Prohibited => match &constraint.value {
                    Some(ConstraintValue::String(_)) | Some(ConstraintValue::List(_)) => {}
                    _ => errors.push(format!(
                        "Constraint '{}' ({}) requires a string or list value",
                        constraint.id, constraint.kind
                    )),
                },
                _ => {} // Other kinds don't require specific values
            }
        }

        errors
    }
}

/// Result of a constraint compliance check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintViolation {
    /// The constraint that was violated.
    pub constraint_id: String,

    /// The target that violated the constraint.
    pub target: String,

    /// File where violation occurred.
    pub file: Option<String>,

    /// Line number where violation occurred.
    pub line: Option<u32>,

    /// Description of what went wrong.
    pub message: String,

    /// Suggested fix.
    pub suggestion: Option<String>,
}

impl ConstraintViolation {
    /// Create a new violation.
    pub fn new(
        constraint_id: impl Into<String>,
        target: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            constraint_id: constraint_id.into(),
            target: target.into(),
            file: None,
            line: None,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Set file location.
    #[must_use]
    pub fn with_location(mut self, file: impl Into<String>, line: u32) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self
    }

    /// Set suggestion.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Format as a warning message for prompts.
    pub fn to_warning(&self) -> String {
        let location = match (&self.file, self.line) {
            (Some(f), Some(l)) => format!(" at `{}:{}`", f, l),
            (Some(f), None) => format!(" in `{}`", f),
            _ => String::new(),
        };

        let suggestion = self
            .suggestion
            .as_ref()
            .map_or(String::new(), |s| format!("\n   💡 {}", s));

        format!(
            "⚠️ **{}** violated by `{}`{}: {}{}",
            self.constraint_id, self.target, location, self.message, suggestion
        )
    }
}

/// Compliance verification result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComplianceResult {
    /// Whether all constraints passed.
    pub compliant: bool,

    /// List of violations found.
    pub violations: Vec<ConstraintViolation>,

    /// Constraints that were checked.
    pub checked_count: usize,

    /// Timestamp of the check.
    pub checked_at: Option<String>,
}

impl ComplianceResult {
    /// Create a new compliant result.
    pub fn passed(checked_count: usize) -> Self {
        Self {
            compliant: true,
            violations: Vec::new(),
            checked_count,
            checked_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    /// Create a failed result with violations.
    pub fn failed(violations: Vec<ConstraintViolation>, checked_count: usize) -> Self {
        Self {
            compliant: false,
            violations,
            checked_count,
            checked_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    /// Add a violation.
    pub fn add_violation(&mut self, violation: ConstraintViolation) {
        self.violations.push(violation);
        self.compliant = false;
    }

    /// Get blocking violations (error or critical).
    pub fn blocking_violations(&self) -> Vec<&ConstraintViolation> {
        // All violations are considered blocking for now
        self.violations.iter().collect()
    }

    /// Generate a prompt section with compliance warnings.
    pub fn to_prompt_section(&self) -> String {
        if self.compliant || self.violations.is_empty() {
            return String::new();
        }

        let mut lines = vec!["### ⚠️ Constraint Violations".to_string(), String::new()];

        for violation in self.violations.iter().take(5) {
            lines.push(violation.to_warning());
        }

        if self.violations.len() > 5 {
            lines.push(format!(
                "\n*...and {} more violations*",
                self.violations.len() - 5
            ));
        }

        lines.push(String::new());
        lines.join("\n")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Constraint Tests
    // =========================================================================

    #[test]
    fn test_constraint_kind_from_str() {
        use std::str::FromStr;
        assert_eq!(
            ConstraintKind::from_str("noDirectCalls"),
            Ok(ConstraintKind::NoDirectCalls)
        );
        assert_eq!(
            ConstraintKind::from_str("no_direct_calls"),
            Ok(ConstraintKind::NoDirectCalls)
        );
        assert_eq!(
            ConstraintKind::from_str("maxComplexity"),
            Ok(ConstraintKind::MaxComplexity)
        );
        assert_eq!(
            ConstraintKind::from_str("max_complexity"),
            Ok(ConstraintKind::MaxComplexity)
        );
        assert_eq!(
            ConstraintKind::from_str("maxParameters"),
            Ok(ConstraintKind::MaxParameters)
        );
        assert_eq!(
            ConstraintKind::from_str("maxLines"),
            Ok(ConstraintKind::MaxLines)
        );
        assert_eq!(
            ConstraintKind::from_str("requireDocs"),
            Ok(ConstraintKind::RequireDocs)
        );
        assert_eq!(
            ConstraintKind::from_str("prohibited"),
            Ok(ConstraintKind::Prohibited)
        );
        assert!(ConstraintKind::from_str("unknown").is_err());
    }

    #[test]
    fn test_constraint_kind_display() {
        assert_eq!(
            format!("{}", ConstraintKind::NoDirectCalls),
            "noDirectCalls"
        );
        assert_eq!(
            format!("{}", ConstraintKind::MaxComplexity),
            "maxComplexity"
        );
    }

    #[test]
    fn test_constraint_new() {
        let constraint =
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Keep complexity low");
        assert_eq!(constraint.id, "c1");
        assert_eq!(constraint.kind, ConstraintKind::MaxComplexity);
        assert_eq!(constraint.description, "Keep complexity low");
        assert_eq!(constraint.severity, ConstraintSeverity::Warning);
        assert!(constraint.enabled);
        assert!(constraint.targets.is_empty());
    }

    #[test]
    fn test_constraint_builder() {
        let constraint = CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Keep it simple")
            .with_severity(ConstraintSeverity::Error)
            .with_target("module::function")
            .with_value(ConstraintValue::Number(10));

        assert_eq!(constraint.severity, ConstraintSeverity::Error);
        assert_eq!(constraint.targets, vec!["module::function"]);
        assert_eq!(constraint.value, Some(ConstraintValue::Number(10)));
    }

    #[test]
    fn test_constraint_applies_to() {
        let global = CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Global");
        assert!(global.applies_to("any_function"));
        assert!(global.applies_to("module::function"));

        let targeted = CcgConstraint::new("c2", ConstraintKind::MaxComplexity, "Targeted")
            .with_target("core::process");
        assert!(targeted.applies_to("core::process"));
        assert!(targeted.applies_to("core::process::inner"));
        assert!(!targeted.applies_to("other::function"));

        let wildcard = CcgConstraint::new("c3", ConstraintKind::MaxComplexity, "Wildcard")
            .with_target("api::*");
        assert!(wildcard.applies_to("api::handler"));
        assert!(wildcard.applies_to("api::routes"));
        assert!(!wildcard.applies_to("core::handler"));
    }

    #[test]
    fn test_constraint_disabled() {
        let constraint = CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Test").disabled();
        assert!(!constraint.enabled);
    }

    #[test]
    fn test_constraint_with_targets() {
        let targets = vec![
            "module1::func1".to_string(),
            "module2::func2".to_string(),
            "module3::*".to_string(),
        ];
        let constraint = CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Multi-target")
            .with_targets(targets);

        assert_eq!(constraint.targets.len(), 3);
        assert!(constraint.applies_to("module1::func1"));
        assert!(constraint.applies_to("module2::func2"));
        assert!(constraint.applies_to("module3::anything"));
        assert!(!constraint.applies_to("module4::func"));
    }

    #[test]
    fn test_constraint_to_prompt_string() {
        let constraint =
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Keep complexity low")
                .with_target("process_request")
                .with_value(ConstraintValue::Number(10));

        let prompt = constraint.to_prompt_string();
        assert!(prompt.contains("maxComplexity"));
        assert!(prompt.contains("process_request"));
        assert!(prompt.contains("10"));
    }

    // =========================================================================
    // ConstraintSet Tests
    // =========================================================================

    #[test]
    fn test_constraint_set_new() {
        let set = ConstraintSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_constraint_set_with_constraint() {
        let set = ConstraintSet::new().with_constraint(CcgConstraint::new(
            "c1",
            ConstraintKind::MaxComplexity,
            "Test",
        ));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_constraint_set_add() {
        let mut set = ConstraintSet::new();
        set.add(CcgConstraint::new(
            "c1",
            ConstraintKind::MaxComplexity,
            "Test",
        ));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_constraint_set_enabled() {
        let set = ConstraintSet::new()
            .with_constraint(CcgConstraint::new(
                "c1",
                ConstraintKind::MaxComplexity,
                "Enabled",
            ))
            .with_constraint(
                CcgConstraint::new("c2", ConstraintKind::MaxLines, "Disabled").disabled(),
            );

        let enabled: Vec<_> = set.enabled().collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "c1");
    }

    #[test]
    fn test_constraint_set_for_target() {
        let set = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "For core")
                    .with_target("core::*"),
            )
            .with_constraint(CcgConstraint::new("c2", ConstraintKind::MaxLines, "Global"));

        let core_constraints = set.for_target("core::process");
        assert_eq!(core_constraints.len(), 2);

        let other_constraints = set.for_target("api::handler");
        assert_eq!(other_constraints.len(), 1);
        assert_eq!(other_constraints[0].id, "c2");
    }

    #[test]
    fn test_constraint_set_by_kind() {
        let set = ConstraintSet::new()
            .with_constraint(CcgConstraint::new(
                "c1",
                ConstraintKind::MaxComplexity,
                "Test1",
            ))
            .with_constraint(CcgConstraint::new(
                "c2",
                ConstraintKind::MaxComplexity,
                "Test2",
            ))
            .with_constraint(CcgConstraint::new("c3", ConstraintKind::MaxLines, "Test3"));

        let complexity = set.by_kind(ConstraintKind::MaxComplexity);
        assert_eq!(complexity.len(), 2);
    }

    #[test]
    fn test_constraint_set_has_blocking() {
        let non_blocking = ConstraintSet::new().with_constraint(CcgConstraint::new(
            "c1",
            ConstraintKind::MaxComplexity,
            "Test",
        ));
        assert!(!non_blocking.has_blocking());

        let blocking = ConstraintSet::new().with_constraint(
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Test")
                .with_severity(ConstraintSeverity::Error),
        );
        assert!(blocking.has_blocking());
    }

    #[test]
    fn test_constraint_set_count_by_severity() {
        let set = ConstraintSet::new()
            .with_constraint(CcgConstraint::new(
                "c1",
                ConstraintKind::MaxComplexity,
                "Test1",
            ))
            .with_constraint(
                CcgConstraint::new("c2", ConstraintKind::MaxLines, "Test2")
                    .with_severity(ConstraintSeverity::Error),
            )
            .with_constraint(
                CcgConstraint::new("c3", ConstraintKind::RequireDocs, "Test3")
                    .with_severity(ConstraintSeverity::Error),
            );

        assert_eq!(set.count_by_severity(ConstraintSeverity::Warning), 1);
        assert_eq!(set.count_by_severity(ConstraintSeverity::Error), 2);
    }

    #[test]
    fn test_constraint_set_from_json() {
        let json = r#"{
            "constraints": [
                {
                    "id": "max-complexity",
                    "kind": "maxComplexity",
                    "description": "Keep functions simple",
                    "severity": "warning",
                    "targets": [],
                    "value": 10,
                    "enabled": true
                }
            ]
        }"#;

        let set = ConstraintSet::from_json(json).unwrap();
        assert_eq!(set.len(), 1);
        assert_eq!(set.all()[0].id, "max-complexity");
    }

    #[test]
    fn test_constraint_set_validate() {
        let mut set = ConstraintSet::new();

        // Valid constraint
        set.add(
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Valid")
                .with_value(ConstraintValue::Number(10)),
        );

        // Missing value for maxComplexity
        set.add(CcgConstraint::new(
            "c2",
            ConstraintKind::MaxComplexity,
            "Missing value",
        ));

        // Empty description
        set.add(
            CcgConstraint::new("c3", ConstraintKind::MaxLines, "")
                .with_value(ConstraintValue::Number(100)),
        );

        let errors = set.validate();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_constraint_set_to_prompt_section() {
        let set = ConstraintSet::new().with_constraint(
            CcgConstraint::new("c1", ConstraintKind::MaxComplexity, "Keep it simple")
                .with_value(ConstraintValue::Number(10)),
        );

        let section = set.to_prompt_section();
        assert!(section.contains("Active Constraints"));
        assert!(section.contains("maxComplexity"));
    }

    // =========================================================================
    // ConstraintViolation Tests
    // =========================================================================

    #[test]
    fn test_constraint_violation_new() {
        let violation = ConstraintViolation::new("c1", "process", "Complexity too high");
        assert_eq!(violation.constraint_id, "c1");
        assert_eq!(violation.target, "process");
        assert_eq!(violation.message, "Complexity too high");
    }

    #[test]
    fn test_constraint_violation_with_location() {
        let violation =
            ConstraintViolation::new("c1", "process", "Error").with_location("src/main.rs", 42);
        assert_eq!(violation.file, Some("src/main.rs".to_string()));
        assert_eq!(violation.line, Some(42));
    }

    #[test]
    fn test_constraint_violation_to_warning() {
        let violation = ConstraintViolation::new("c1", "process", "Complexity = 15, max = 10")
            .with_location("src/main.rs", 42)
            .with_suggestion("Break into smaller functions");

        let warning = violation.to_warning();
        assert!(warning.contains("c1"));
        assert!(warning.contains("process"));
        assert!(warning.contains("src/main.rs:42"));
        assert!(warning.contains("Break into smaller functions"));
    }

    // =========================================================================
    // ComplianceResult Tests
    // =========================================================================

    #[test]
    fn test_compliance_result_passed() {
        let result = ComplianceResult::passed(5);
        assert!(result.compliant);
        assert!(result.violations.is_empty());
        assert_eq!(result.checked_count, 5);
    }

    #[test]
    fn test_compliance_result_failed() {
        let violations = vec![ConstraintViolation::new("c1", "process", "Error")];
        let result = ComplianceResult::failed(violations, 5);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_compliance_result_add_violation() {
        let mut result = ComplianceResult::passed(5);
        assert!(result.compliant);

        result.add_violation(ConstraintViolation::new("c1", "process", "Error"));
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_compliance_result_blocking_violations() {
        let violations = vec![
            ConstraintViolation::new("c1", "func1", "Complexity too high"),
            ConstraintViolation::new("c2", "func2", "Too many lines"),
        ];
        let result = ComplianceResult::failed(violations, 2);

        let blocking = result.blocking_violations();
        assert_eq!(blocking.len(), 2);
        assert_eq!(blocking[0].constraint_id, "c1");
        assert_eq!(blocking[1].constraint_id, "c2");
    }

    #[test]
    fn test_compliance_result_to_prompt_section() {
        let result = ComplianceResult::failed(
            vec![ConstraintViolation::new("c1", "process", "Too complex")],
            1,
        );

        let section = result.to_prompt_section();
        assert!(section.contains("Constraint Violations"));
        assert!(section.contains("c1"));
    }

    // =========================================================================
    // CcgManifest Tests
    // =========================================================================

    #[test]
    fn test_ccg_manifest_default() {
        let manifest = CcgManifest::default();
        assert!(manifest.name.is_empty());
        assert!(manifest.path.as_os_str().is_empty());
        assert_eq!(manifest.file_count, 0);
        assert_eq!(manifest.symbol_count, 0);
        assert_eq!(manifest.schema_version, "1.0");
    }

    #[test]
    fn test_ccg_manifest_new() {
        let manifest = CcgManifest::new("test-repo", "/path/to/repo");
        assert_eq!(manifest.name, "test-repo");
        assert_eq!(manifest.path.to_str().unwrap(), "/path/to/repo");
    }

    #[test]
    fn test_ccg_manifest_builder() {
        let manifest = CcgManifest::new("my-project", ".")
            .with_primary_language("rust")
            .with_language("rust", LanguageStats::new(10, 1000, 50))
            .with_counts(10, 50)
            .with_security_summary(SecuritySummary::new(0, 1, 5, 10));

        assert_eq!(manifest.primary_language, Some("rust".to_string()));
        assert_eq!(manifest.file_count, 10);
        assert_eq!(manifest.symbol_count, 50);
        assert!(manifest.languages.contains_key("rust"));
        assert_eq!(manifest.security_summary.high, 1);
    }

    #[test]
    fn test_ccg_manifest_has_blocking_issues() {
        let no_issues = CcgManifest::default();
        assert!(!no_issues.has_blocking_issues());

        let with_high =
            CcgManifest::default().with_security_summary(SecuritySummary::new(0, 1, 0, 0));
        assert!(with_high.has_blocking_issues());

        let with_critical =
            CcgManifest::default().with_security_summary(SecuritySummary::new(1, 0, 0, 0));
        assert!(with_critical.has_blocking_issues());

        let only_medium =
            CcgManifest::default().with_security_summary(SecuritySummary::new(0, 0, 5, 0));
        assert!(!only_medium.has_blocking_issues());
    }

    #[test]
    fn test_ccg_manifest_total_security_issues() {
        let manifest =
            CcgManifest::default().with_security_summary(SecuritySummary::new(1, 2, 3, 4));
        assert_eq!(manifest.total_security_issues(), 10);
    }

    #[test]
    fn test_ccg_manifest_serialization() {
        let manifest = CcgManifest::new("test", ".")
            .with_primary_language("rust")
            .with_counts(5, 20);

        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: CcgManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(manifest, deserialized);
    }

    // =========================================================================
    // LanguageStats Tests
    // =========================================================================

    #[test]
    fn test_language_stats_new() {
        let stats = LanguageStats::new(10, 500, 25);
        assert_eq!(stats.file_count, 10);
        assert_eq!(stats.lines_of_code, 500);
        assert_eq!(stats.symbol_count, 25);
    }

    // =========================================================================
    // SecuritySummary Tests
    // =========================================================================

    #[test]
    fn test_security_summary_new() {
        let summary = SecuritySummary::new(1, 2, 3, 4);
        assert_eq!(summary.critical, 1);
        assert_eq!(summary.high, 2);
        assert_eq!(summary.medium, 3);
        assert_eq!(summary.low, 4);
    }

    #[test]
    fn test_security_summary_has_blocking() {
        assert!(!SecuritySummary::default().has_blocking());
        assert!(SecuritySummary::new(1, 0, 0, 0).has_blocking());
        assert!(SecuritySummary::new(0, 1, 0, 0).has_blocking());
        assert!(!SecuritySummary::new(0, 0, 1, 1).has_blocking());
    }

    #[test]
    fn test_security_summary_total() {
        assert_eq!(SecuritySummary::new(1, 2, 3, 4).total(), 10);
        assert_eq!(SecuritySummary::default().total(), 0);
    }

    // =========================================================================
    // CcgArchitecture Tests
    // =========================================================================

    #[test]
    fn test_ccg_architecture_default() {
        let arch = CcgArchitecture::default();
        assert!(arch.modules.is_empty());
        assert!(arch.public_api.is_empty());
        assert!(arch.entry_points.is_empty());
        assert!(arch.dependencies.is_empty());
    }

    #[test]
    fn test_ccg_architecture_builder() {
        let arch = CcgArchitecture::new()
            .with_module(Module::new("core", "src/core"))
            .with_public_symbol(PublicSymbol::new("process", SymbolKind::Function))
            .with_entry_point(EntryPoint::new("main", EntryPointKind::Main, "src/main.rs"))
            .with_dependency(ModuleDependency::new("core", "utils", DependencyKind::Uses));

        assert_eq!(arch.modules.len(), 1);
        assert_eq!(arch.public_api.len(), 1);
        assert_eq!(arch.entry_points.len(), 1);
        assert_eq!(arch.dependencies.len(), 1);
    }

    #[test]
    fn test_ccg_architecture_module_names() {
        let arch = CcgArchitecture::new()
            .with_module(Module::new("core", "src/core"))
            .with_module(Module::new("utils", "src/utils"));

        let names = arch.module_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"core"));
        assert!(names.contains(&"utils"));
    }

    #[test]
    fn test_ccg_architecture_find_module() {
        let arch = CcgArchitecture::new()
            .with_module(Module::new("core", "src/core").with_visibility(Visibility::Public));

        assert!(arch.find_module("core").is_some());
        assert!(arch.find_module("nonexistent").is_none());

        let module = arch.find_module("core").unwrap();
        assert_eq!(module.visibility, Visibility::Public);
    }

    #[test]
    fn test_ccg_architecture_public_api_for_module() {
        let arch = CcgArchitecture::new()
            .with_public_symbol(PublicSymbol::new("foo", SymbolKind::Function).with_module("core"))
            .with_public_symbol(PublicSymbol::new("bar", SymbolKind::Function).with_module("utils"))
            .with_public_symbol(PublicSymbol::new("baz", SymbolKind::Function).with_module("core"));

        let core_symbols = arch.public_api_for_module("core");
        assert_eq!(core_symbols.len(), 2);
    }

    #[test]
    fn test_ccg_architecture_serialization() {
        let arch = CcgArchitecture::new().with_module(Module::new("test", "src/test"));

        let json = serde_json::to_string(&arch).unwrap();
        let deserialized: CcgArchitecture = serde_json::from_str(&json).unwrap();

        assert_eq!(arch, deserialized);
    }

    // =========================================================================
    // Module Tests
    // =========================================================================

    #[test]
    fn test_module_new() {
        let module = Module::new("core", "src/core");
        assert_eq!(module.name, "core");
        assert_eq!(module.path.to_str().unwrap(), "src/core");
        assert_eq!(module.visibility, Visibility::Private);
        assert!(module.children.is_empty());
    }

    #[test]
    fn test_module_builder() {
        let module = Module::new("core", "src/core")
            .with_visibility(Visibility::Public)
            .with_child("sub1")
            .with_child("sub2")
            .with_description("Core functionality");

        assert_eq!(module.visibility, Visibility::Public);
        assert_eq!(module.children.len(), 2);
        assert_eq!(module.description, Some("Core functionality".to_string()));
    }

    // =========================================================================
    // PublicSymbol Tests
    // =========================================================================

    #[test]
    fn test_public_symbol_new() {
        let symbol = PublicSymbol::new("process", SymbolKind::Function);
        assert_eq!(symbol.name, "process");
        assert_eq!(symbol.qualified_name, "process");
        assert_eq!(symbol.kind, SymbolKind::Function);
    }

    #[test]
    fn test_public_symbol_builder() {
        let symbol = PublicSymbol::new("Config", SymbolKind::Struct)
            .with_qualified_name("crate::config::Config")
            .with_module("config")
            .with_description("Configuration struct")
            .with_signature("pub struct Config { ... }");

        assert_eq!(symbol.qualified_name, "crate::config::Config");
        assert_eq!(symbol.module, Some("config".to_string()));
        assert!(symbol.description.is_some());
        assert!(symbol.signature.is_some());
    }

    // =========================================================================
    // EntryPoint Tests
    // =========================================================================

    #[test]
    fn test_entry_point_new() {
        let entry = EntryPoint::new("main", EntryPointKind::Main, "src/main.rs");
        assert_eq!(entry.name, "main");
        assert_eq!(entry.kind, EntryPointKind::Main);
        assert_eq!(entry.file.to_str().unwrap(), "src/main.rs");
        assert!(entry.line.is_none());
    }

    #[test]
    fn test_entry_point_with_line() {
        let entry = EntryPoint::new("main", EntryPointKind::Main, "src/main.rs").with_line(10);
        assert_eq!(entry.line, Some(10));
    }

    // =========================================================================
    // ModuleDependency Tests
    // =========================================================================

    #[test]
    fn test_module_dependency_new() {
        let dep = ModuleDependency::new("core", "utils", DependencyKind::Uses);
        assert_eq!(dep.from, "core");
        assert_eq!(dep.to, "utils");
        assert_eq!(dep.kind, DependencyKind::Uses);
    }

    // =========================================================================
    // CcgCache Tests
    // =========================================================================

    #[test]
    fn test_ccg_cache_new() {
        let cache = CcgCache::new();
        assert!(!cache.has_manifest());
        assert!(!cache.has_architecture());
    }

    #[test]
    fn test_ccg_cache_set_and_get_manifest() {
        let mut cache = CcgCache::new();
        let manifest = CcgManifest::new("test", ".");

        assert!(cache.manifest().is_none());

        cache.set_manifest(manifest.clone());

        assert!(cache.has_manifest());
        assert_eq!(cache.manifest().unwrap().name, "test");
    }

    #[test]
    fn test_ccg_cache_set_and_get_architecture() {
        let mut cache = CcgCache::new();
        let arch = CcgArchitecture::new().with_module(Module::new("test", "src/test"));

        assert!(cache.architecture().is_none());

        cache.set_architecture(arch);

        assert!(cache.has_architecture());
        assert_eq!(cache.architecture().unwrap().modules.len(), 1);
    }

    #[test]
    fn test_ccg_cache_clear() {
        let mut cache = CcgCache::new();
        cache.set_manifest(CcgManifest::new("test", "."));
        cache.set_architecture(CcgArchitecture::new());

        assert!(cache.has_manifest());
        assert!(cache.has_architecture());

        cache.clear();

        assert!(!cache.has_manifest());
        assert!(!cache.has_architecture());
    }
}
