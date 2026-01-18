//! Code intelligence builder for transforming narsil-mcp data into prompt context.
//!
//! This module provides the `CodeIntelligenceBuilder` which queries narsil-mcp
//! for call graph, reference, and dependency information, then transforms it
//! into `CodeIntelligenceContext` for use in prompt generation.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::narsil::{NarsilClient, NarsilConfig, CodeIntelligenceBuilder};
//!
//! let client = NarsilClient::new(NarsilConfig::default())?;
//! let intel = CodeIntelligenceBuilder::new(&client)
//!     .for_functions(&["process_request", "handle_error"])
//!     .for_files(&["src/handler.rs"])
//!     .build()?;
//!
//! // Use intel in prompt generation
//! let context = PromptContext::new().with_code_intelligence(intel);
//! ```

use crate::narsil::{Dependency, NarsilClient, NarsilError, Reference};
use crate::prompt::context::{
    CallGraphNode, CodeIntelligenceContext, ModuleDependency, ReferenceKind, SymbolReference,
};

/// Builder for constructing `CodeIntelligenceContext` from narsil-mcp queries.
///
/// Collects function names, symbols, and file paths to query, then executes
/// the queries against narsil-mcp and transforms the results.
///
/// # Example
///
/// ```rust,ignore
/// let intel = CodeIntelligenceBuilder::new(&client)
///     .for_functions(&["main", "process"])
///     .for_symbols(&["Config", "Error"])
///     .for_files(&["src/lib.rs"])
///     .build()?;
/// ```
pub struct CodeIntelligenceBuilder<'a> {
    client: &'a NarsilClient,
    functions: Vec<String>,
    symbols: Vec<String>,
    files: Vec<String>,
    max_call_depth: u32,
    include_transitive: bool,
}

impl<'a> CodeIntelligenceBuilder<'a> {
    /// Create a new builder with the given narsil client.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = CodeIntelligenceBuilder::new(&client);
    /// ```
    #[must_use]
    pub fn new(client: &'a NarsilClient) -> Self {
        Self {
            client,
            functions: Vec::new(),
            symbols: Vec::new(),
            files: Vec::new(),
            max_call_depth: 2,
            include_transitive: false,
        }
    }

    /// Add functions to query for call graph information.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.for_functions(&["process_request", "validate_input"]);
    /// ```
    #[must_use]
    pub fn for_functions(mut self, functions: &[&str]) -> Self {
        self.functions
            .extend(functions.iter().map(|s| s.to_string()));
        self
    }

    /// Add symbols to query for references.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.for_symbols(&["MyStruct", "ErrorKind"]);
    /// ```
    #[must_use]
    pub fn for_symbols(mut self, symbols: &[&str]) -> Self {
        self.symbols.extend(symbols.iter().map(|s| s.to_string()));
        self
    }

    /// Add files to query for dependency information.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.for_files(&["src/lib.rs", "src/main.rs"]);
    /// ```
    #[must_use]
    pub fn for_files(mut self, files: &[&str]) -> Self {
        self.files.extend(files.iter().map(|s| s.to_string()));
        self
    }

    /// Set the maximum depth for call graph traversal.
    ///
    /// Default is 2 (direct callers/callees only).
    #[must_use]
    pub fn with_max_call_depth(mut self, depth: u32) -> Self {
        self.max_call_depth = depth;
        self
    }

    /// Include transitive callers/callees in call graph.
    ///
    /// Default is false (only direct connections).
    #[must_use]
    pub fn with_transitive(mut self, include: bool) -> Self {
        self.include_transitive = include;
        self
    }

    /// Build the code intelligence context by querying narsil-mcp.
    ///
    /// Returns an empty context (with `is_available = false`) if narsil-mcp
    /// is not available, rather than failing.
    ///
    /// # Errors
    ///
    /// Returns an error only for unrecoverable issues (e.g., parse errors).
    /// Unavailability is handled gracefully.
    pub fn build(self) -> Result<CodeIntelligenceContext, NarsilError> {
        // If narsil-mcp is not available, return empty context
        if !self.client.is_available() {
            return Ok(CodeIntelligenceContext::new());
        }

        let mut call_graph = Vec::new();
        let mut references = Vec::new();
        let mut dependencies = Vec::new();

        // Query call graphs for each function
        for function in &self.functions {
            if let Some(graph_json) = self.client.get_call_graph(function)? {
                if let Some(node) = parse_call_graph_node(&graph_json, function) {
                    call_graph.push(node);
                }
            }
        }

        // Query references for each symbol
        for symbol in &self.symbols {
            let refs = self.client.find_references(symbol)?;
            for narsil_ref in refs {
                references.push(convert_reference(symbol, &narsil_ref));
            }
        }

        // Query dependencies for each file
        for file in &self.files {
            let deps = self.client.get_dependencies(file)?;
            dependencies.push(convert_dependencies(file, &deps));
        }

        let mut context = CodeIntelligenceContext::new()
            .with_call_graph(call_graph)
            .with_references(references)
            .with_dependencies(dependencies);

        // Mark as available if we have any data
        if context.has_data() {
            context = context.mark_available();
        }

        Ok(context)
    }
}

/// Parse a call graph JSON response into a `CallGraphNode`.
///
/// Expected JSON format from narsil-mcp:
/// ```json
/// {
///     "function": "process_request",
///     "file": "src/handler.rs",
///     "line": 42,
///     "callers": ["main", "handle_http"],
///     "callees": ["validate", "execute"]
/// }
/// ```
fn parse_call_graph_node(json: &serde_json::Value, function_name: &str) -> Option<CallGraphNode> {
    let mut node = CallGraphNode::new(function_name);

    // Extract file location
    if let Some(file) = json.get("file").and_then(|v| v.as_str()) {
        node = node.with_file(file);
    }

    if let Some(line) = json.get("line").and_then(|v| v.as_u64()) {
        node = node.with_line(line as u32);
    }

    // Extract callers
    if let Some(callers) = json.get("callers").and_then(|v| v.as_array()) {
        let caller_names: Vec<String> = callers
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        node = node.with_callers(caller_names);
    }

    // Extract callees
    if let Some(callees) = json.get("callees").and_then(|v| v.as_array()) {
        let callee_names: Vec<String> = callees
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        node = node.with_callees(callee_names);
    }

    Some(node)
}

/// Convert a narsil `Reference` to a prompt `SymbolReference`.
fn convert_reference(symbol: &str, narsil_ref: &Reference) -> SymbolReference {
    let mut reference = SymbolReference::new(
        symbol,
        narsil_ref.file.to_string_lossy().to_string(),
        narsil_ref.line,
    );

    if let Some(column) = narsil_ref.column {
        reference = reference.with_column(column);
    }

    if let Some(context) = &narsil_ref.context {
        reference = reference.with_context(context.clone());
    }

    // Map kind string to ReferenceKind
    if let Some(kind_str) = &narsil_ref.kind {
        let kind = match kind_str.to_lowercase().as_str() {
            "definition" | "def" => ReferenceKind::Definition,
            "usage" | "use" | "read" => ReferenceKind::Usage,
            "call" => ReferenceKind::Call,
            "import" => ReferenceKind::Import,
            _ => ReferenceKind::Unknown,
        };
        reference = reference.with_kind(kind);
    }

    reference
}

/// Convert narsil `Dependency` list to a `ModuleDependency`.
fn convert_dependencies(file: &str, deps: &[Dependency]) -> ModuleDependency {
    let imports: Vec<String> = deps
        .iter()
        .map(|d| d.path.to_string_lossy().to_string())
        .collect();

    ModuleDependency::new(file).with_imports(imports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narsil::NarsilConfig;
    use std::path::PathBuf;

    // =========================================================================
    // parse_call_graph_node tests
    // =========================================================================

    #[test]
    fn test_parse_call_graph_node_full() {
        let json = serde_json::json!({
            "function": "process_request",
            "file": "src/handler.rs",
            "line": 42,
            "callers": ["main", "handle_http"],
            "callees": ["validate", "execute"]
        });

        let node = parse_call_graph_node(&json, "process_request").unwrap();

        assert_eq!(node.function_name, "process_request");
        assert_eq!(node.file, Some("src/handler.rs".to_string()));
        assert_eq!(node.line, Some(42));
        assert_eq!(node.callers, vec!["main", "handle_http"]);
        assert_eq!(node.callees, vec!["validate", "execute"]);
    }

    #[test]
    fn test_parse_call_graph_node_minimal() {
        let json = serde_json::json!({
            "function": "simple"
        });

        let node = parse_call_graph_node(&json, "simple").unwrap();

        assert_eq!(node.function_name, "simple");
        assert!(node.file.is_none());
        assert!(node.callers.is_empty());
        assert!(node.callees.is_empty());
    }

    #[test]
    fn test_parse_call_graph_node_empty_arrays() {
        let json = serde_json::json!({
            "function": "isolated",
            "callers": [],
            "callees": []
        });

        let node = parse_call_graph_node(&json, "isolated").unwrap();

        assert!(node.callers.is_empty());
        assert!(node.callees.is_empty());
    }

    // =========================================================================
    // convert_reference tests
    // =========================================================================

    #[test]
    fn test_convert_reference_full() {
        let narsil_ref = Reference::new("src/lib.rs", 42)
            .with_column(10)
            .with_context("fn foo()")
            .with_kind("definition");

        let symbol_ref = convert_reference("foo", &narsil_ref);

        assert_eq!(symbol_ref.symbol, "foo");
        assert_eq!(symbol_ref.file, "src/lib.rs");
        assert_eq!(symbol_ref.line, 42);
        assert_eq!(symbol_ref.column, Some(10));
        assert_eq!(symbol_ref.context, Some("fn foo()".to_string()));
        assert_eq!(symbol_ref.kind, ReferenceKind::Definition);
    }

    #[test]
    fn test_convert_reference_minimal() {
        let narsil_ref = Reference::new("main.rs", 1);

        let symbol_ref = convert_reference("main", &narsil_ref);

        assert_eq!(symbol_ref.symbol, "main");
        assert_eq!(symbol_ref.file, "main.rs");
        assert_eq!(symbol_ref.line, 1);
        assert!(symbol_ref.column.is_none());
        assert_eq!(symbol_ref.kind, ReferenceKind::Unknown);
    }

    #[test]
    fn test_convert_reference_kind_mapping() {
        let test_cases = [
            ("definition", ReferenceKind::Definition),
            ("def", ReferenceKind::Definition),
            ("usage", ReferenceKind::Usage),
            ("use", ReferenceKind::Usage),
            ("read", ReferenceKind::Usage),
            ("call", ReferenceKind::Call),
            ("import", ReferenceKind::Import),
            ("unknown", ReferenceKind::Unknown),
            ("something_else", ReferenceKind::Unknown),
        ];

        for (kind_str, expected_kind) in test_cases {
            let narsil_ref = Reference::new("file.rs", 1).with_kind(kind_str);
            let symbol_ref = convert_reference("symbol", &narsil_ref);
            assert_eq!(
                symbol_ref.kind, expected_kind,
                "Failed for kind_str: {}",
                kind_str
            );
        }
    }

    // =========================================================================
    // convert_dependencies tests
    // =========================================================================

    #[test]
    fn test_convert_dependencies_full() {
        let deps = vec![
            Dependency::new("std::io"),
            Dependency::new("crate::util"),
            Dependency::new("super::config"),
        ];

        let module_dep = convert_dependencies("src/lib.rs", &deps);

        assert_eq!(module_dep.module_path, "src/lib.rs");
        assert_eq!(module_dep.imports.len(), 3);
        assert!(module_dep.imports.contains(&"std::io".to_string()));
        assert!(module_dep.imports.contains(&"crate::util".to_string()));
    }

    #[test]
    fn test_convert_dependencies_empty() {
        let deps: Vec<Dependency> = vec![];

        let module_dep = convert_dependencies("src/empty.rs", &deps);

        assert_eq!(module_dep.module_path, "src/empty.rs");
        assert!(module_dep.imports.is_empty());
    }

    // =========================================================================
    // CodeIntelligenceBuilder tests
    // =========================================================================

    #[test]
    fn test_builder_new() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();
        let builder = CodeIntelligenceBuilder::new(&client);

        assert!(builder.functions.is_empty());
        assert!(builder.symbols.is_empty());
        assert!(builder.files.is_empty());
        assert_eq!(builder.max_call_depth, 2);
        assert!(!builder.include_transitive);
    }

    #[test]
    fn test_builder_fluent_api() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let builder = CodeIntelligenceBuilder::new(&client)
            .for_functions(&["foo", "bar"])
            .for_symbols(&["MyStruct"])
            .for_files(&["src/lib.rs"])
            .with_max_call_depth(3)
            .with_transitive(true);

        assert_eq!(builder.functions, vec!["foo", "bar"]);
        assert_eq!(builder.symbols, vec!["MyStruct"]);
        assert_eq!(builder.files, vec!["src/lib.rs"]);
        assert_eq!(builder.max_call_depth, 3);
        assert!(builder.include_transitive);
    }

    #[test]
    fn test_builder_returns_empty_when_unavailable() {
        // Use a non-existent binary path to ensure unavailability
        let config = NarsilConfig::new(".").with_binary_path("/nonexistent/narsil-mcp");
        let client = NarsilClient::new(config).unwrap();

        let result = CodeIntelligenceBuilder::new(&client)
            .for_functions(&["foo"])
            .build();

        assert!(result.is_ok());
        let intel = result.unwrap();
        assert!(!intel.is_available);
        assert!(!intel.has_data());
    }

    #[test]
    fn test_builder_accumulates_functions() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let builder = CodeIntelligenceBuilder::new(&client)
            .for_functions(&["a", "b"])
            .for_functions(&["c"]);

        assert_eq!(builder.functions, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_builder_accumulates_symbols() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let builder = CodeIntelligenceBuilder::new(&client)
            .for_symbols(&["X"])
            .for_symbols(&["Y", "Z"]);

        assert_eq!(builder.symbols, vec!["X", "Y", "Z"]);
    }

    #[test]
    fn test_builder_accumulates_files() {
        let config = NarsilConfig::new(".");
        let client = NarsilClient::new(config).unwrap();

        let builder = CodeIntelligenceBuilder::new(&client)
            .for_files(&["a.rs"])
            .for_files(&["b.rs"]);

        assert_eq!(builder.files, vec!["a.rs", "b.rs"]);
    }

    // =========================================================================
    // Integration test with mock data
    // =========================================================================

    #[test]
    fn test_parse_call_graph_node_handles_nested_structure() {
        // Test alternate JSON structure that narsil-mcp might return
        let json = serde_json::json!({
            "name": "process",
            "function": "process",
            "location": {
                "file": "src/processor.rs",
                "line": 100
            },
            "callers": ["caller1"],
            "callees": ["callee1", "callee2"]
        });

        // Current implementation should handle direct properties
        let node = parse_call_graph_node(&json, "process").unwrap();

        assert_eq!(node.function_name, "process");
        // Note: nested location isn't handled by current impl, which is fine
        assert_eq!(node.callers, vec!["caller1"]);
        assert_eq!(node.callees, vec!["callee1", "callee2"]);
    }

    #[test]
    fn test_call_graph_node_is_hotspot_threshold() {
        let json = serde_json::json!({
            "function": "hotspot",
            "callers": ["a", "b", "c", "d"],
            "callees": ["e"]
        });

        let node = parse_call_graph_node(&json, "hotspot").unwrap();

        // 4 + 1 = 5 connections, which is the hotspot threshold
        assert!(node.is_hotspot());
    }

    #[test]
    fn test_reference_conversion_preserves_path() {
        // Test that PathBuf paths are correctly converted
        let narsil_ref = Reference {
            file: PathBuf::from("src/nested/deeply/module.rs"),
            line: 99,
            column: None,
            context: None,
            kind: None,
        };

        let symbol_ref = convert_reference("test_symbol", &narsil_ref);

        assert_eq!(symbol_ref.file, "src/nested/deeply/module.rs");
    }
}
