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
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        kind: DependencyKind,
    ) -> Self {
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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

        let with_high = CcgManifest::default()
            .with_security_summary(SecuritySummary::new(0, 1, 0, 0));
        assert!(with_high.has_blocking_issues());

        let with_critical = CcgManifest::default()
            .with_security_summary(SecuritySummary::new(1, 0, 0, 0));
        assert!(with_critical.has_blocking_issues());

        let only_medium = CcgManifest::default()
            .with_security_summary(SecuritySummary::new(0, 0, 5, 0));
        assert!(!only_medium.has_blocking_issues());
    }

    #[test]
    fn test_ccg_manifest_total_security_issues() {
        let manifest = CcgManifest::default()
            .with_security_summary(SecuritySummary::new(1, 2, 3, 4));
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
            .with_public_symbol(
                PublicSymbol::new("foo", SymbolKind::Function).with_module("core"),
            )
            .with_public_symbol(
                PublicSymbol::new("bar", SymbolKind::Function).with_module("utils"),
            )
            .with_public_symbol(
                PublicSymbol::new("baz", SymbolKind::Function).with_module("core"),
            );

        let core_symbols = arch.public_api_for_module("core");
        assert_eq!(core_symbols.len(), 2);
    }

    #[test]
    fn test_ccg_architecture_serialization() {
        let arch = CcgArchitecture::new()
            .with_module(Module::new("test", "src/test"));

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
