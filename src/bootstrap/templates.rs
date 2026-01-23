//! Template registry for language-specific prompts and configuration files.
//!
//! This module provides a registry for loading and serving language-specific templates
//! for prompts (build, plan, debug), CLAUDE.md files, and settings. Templates are
//! embedded at compile time via `include_str!` and serve as the basis for generating
//! project-specific configuration during bootstrap.
//!
//! # Overview
//!
//! The registry supports multiple template kinds for each programming language:
//! - Build prompts (`PROMPT_build.md`)
//! - Plan prompts (`PROMPT_plan.md`)
//! - Debug prompts (`PROMPT_debug.md`)
//! - Claude configuration (`CLAUDE.md`)
//! - Settings (`settings.json`)
//!
//! When a language-specific template isn't available, the registry falls back to
//! a generic (Rust-based) default template.
//!
//! # Example
//!
//! ```rust
//! use ralph::bootstrap::templates::{TemplateRegistry, TemplateKind};
//! use ralph::Language;
//!
//! let registry = TemplateRegistry::new();
//!
//! // Get a language-specific template
//! let rust_build = registry.get(TemplateKind::PromptBuild, Language::Rust);
//! assert!(!rust_build.is_empty());
//!
//! // Get a template with fallback (Python may fall back to generic)
//! let python_build = registry.get(TemplateKind::PromptBuild, Language::Python);
//! assert!(!python_build.is_empty());
//! ```

use std::collections::HashMap;

use super::language::Language;

/// Kinds of templates that can be registered and retrieved.
///
/// Each template kind corresponds to a specific configuration file or prompt
/// used during project bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateKind {
    /// Build phase prompt template (`PROMPT_build.md`)
    PromptBuild,
    /// Planning phase prompt template (`PROMPT_plan.md`)
    PromptPlan,
    /// Debug phase prompt template (`PROMPT_debug.md`)
    PromptDebug,
    /// Claude configuration file (`CLAUDE.md`)
    ClaudeMd,
    /// Settings file (`settings.json`)
    SettingsJson,
}

impl TemplateKind {
    /// Returns all template kinds.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::TemplateKind;
    ///
    /// let all = TemplateKind::all();
    /// assert_eq!(all.len(), 5);
    /// ```
    #[must_use]
    pub fn all() -> &'static [TemplateKind] {
        &[
            TemplateKind::PromptBuild,
            TemplateKind::PromptPlan,
            TemplateKind::PromptDebug,
            TemplateKind::ClaudeMd,
            TemplateKind::SettingsJson,
        ]
    }

    /// Returns the default filename for this template kind.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::TemplateKind;
    ///
    /// assert_eq!(TemplateKind::PromptBuild.filename(), "PROMPT_build.md");
    /// assert_eq!(TemplateKind::ClaudeMd.filename(), "CLAUDE.md");
    /// ```
    #[must_use]
    pub fn filename(&self) -> &'static str {
        match self {
            TemplateKind::PromptBuild => "PROMPT_build.md",
            TemplateKind::PromptPlan => "PROMPT_plan.md",
            TemplateKind::PromptDebug => "PROMPT_debug.md",
            TemplateKind::ClaudeMd => "CLAUDE.md",
            TemplateKind::SettingsJson => "settings.json",
        }
    }
}

/// Registry for language-specific templates.
///
/// The registry holds templates keyed by (Language, TemplateKind) pairs and provides
/// a `get()` method that falls back to a default template when a language-specific
/// one isn't available.
///
/// # Example
///
/// ```rust
/// use ralph::bootstrap::templates::{TemplateRegistry, TemplateKind};
/// use ralph::Language;
///
/// let registry = TemplateRegistry::new();
///
/// // All template kinds should be available for Rust
/// for kind in TemplateKind::all() {
///     let template = registry.get(*kind, Language::Rust);
///     assert!(!template.is_empty(), "Template {:?} should exist for Rust", kind);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TemplateRegistry {
    templates: HashMap<(Language, TemplateKind), String>,
    defaults: HashMap<TemplateKind, String>,
}

impl TemplateRegistry {
    /// Creates a new template registry with all embedded templates loaded.
    ///
    /// Templates are loaded from the `src/templates/` directory at compile time.
    /// The registry includes:
    /// - Default templates (used as fallback for any language)
    /// - Language-specific templates for supported languages
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::TemplateRegistry;
    ///
    /// let registry = TemplateRegistry::new();
    /// // Registry is ready to use
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            templates: HashMap::new(),
            defaults: HashMap::new(),
        };

        // Load default templates (Rust-based, serve as fallback)
        registry.load_defaults();

        // Load language-specific templates
        registry.load_rust_templates();
        registry.load_python_templates();
        registry.load_typescript_templates();
        registry.load_go_templates();
        registry.load_java_templates();
        registry.load_ruby_templates();
        registry.load_php_templates();
        registry.load_csharp_templates();

        registry
    }

    /// Registers a default template for a given kind.
    ///
    /// Default templates are used when no language-specific template exists.
    fn register_default(&mut self, kind: TemplateKind, content: &str) {
        self.defaults.insert(kind, content.to_string());
    }

    /// Registers a language-specific template.
    fn register(&mut self, language: Language, kind: TemplateKind, content: &str) {
        self.templates.insert((language, kind), content.to_string());
    }

    /// Loads default templates from the templates directory.
    fn load_defaults(&mut self) {
        self.register_default(
            TemplateKind::PromptBuild,
            include_str!("../templates/PROMPT_build.md"),
        );
        self.register_default(
            TemplateKind::PromptPlan,
            include_str!("../templates/PROMPT_plan.md"),
        );
        self.register_default(
            TemplateKind::PromptDebug,
            include_str!("../templates/PROMPT_debug.md"),
        );
        self.register_default(
            TemplateKind::ClaudeMd,
            include_str!("../templates/CLAUDE.md"),
        );
        self.register_default(
            TemplateKind::SettingsJson,
            include_str!("../templates/settings.json"),
        );
    }

    /// Loads Rust-specific templates.
    ///
    /// Rust uses the default templates which are already Rust-focused.
    fn load_rust_templates(&mut self) {
        self.register(
            Language::Rust,
            TemplateKind::PromptBuild,
            include_str!("../templates/PROMPT_build.md"),
        );
        self.register(
            Language::Rust,
            TemplateKind::PromptPlan,
            include_str!("../templates/PROMPT_plan.md"),
        );
        self.register(
            Language::Rust,
            TemplateKind::PromptDebug,
            include_str!("../templates/PROMPT_debug.md"),
        );
        self.register(
            Language::Rust,
            TemplateKind::ClaudeMd,
            include_str!("../templates/CLAUDE.md"),
        );
        self.register(
            Language::Rust,
            TemplateKind::SettingsJson,
            include_str!("../templates/settings.json"),
        );
    }

    /// Loads Python-specific templates.
    ///
    /// Python templates include pytest, ruff/flake8, mypy, and bandit workflows.
    fn load_python_templates(&mut self) {
        self.register(
            Language::Python,
            TemplateKind::PromptBuild,
            include_str!("../templates/python/PROMPT_build.md"),
        );
        self.register(
            Language::Python,
            TemplateKind::ClaudeMd,
            include_str!("../templates/python/CLAUDE.md"),
        );
        self.register(
            Language::Python,
            TemplateKind::SettingsJson,
            include_str!("../templates/python/settings.json"),
        );
    }

    /// Loads TypeScript-specific templates.
    ///
    /// TypeScript templates include npm/yarn, ESLint, Jest/Vitest, and tsc workflows.
    fn load_typescript_templates(&mut self) {
        self.register(
            Language::TypeScript,
            TemplateKind::PromptBuild,
            include_str!("../templates/typescript/PROMPT_build.md"),
        );
        self.register(
            Language::TypeScript,
            TemplateKind::ClaudeMd,
            include_str!("../templates/typescript/CLAUDE.md"),
        );
        self.register(
            Language::TypeScript,
            TemplateKind::SettingsJson,
            include_str!("../templates/typescript/settings.json"),
        );
    }

    /// Loads Go-specific templates.
    ///
    /// Go templates include go test, go vet, golangci-lint, and govulncheck workflows.
    fn load_go_templates(&mut self) {
        self.register(
            Language::Go,
            TemplateKind::PromptBuild,
            include_str!("../templates/go/PROMPT_build.md"),
        );
        self.register(
            Language::Go,
            TemplateKind::ClaudeMd,
            include_str!("../templates/go/CLAUDE.md"),
        );
        self.register(
            Language::Go,
            TemplateKind::SettingsJson,
            include_str!("../templates/go/settings.json"),
        );
    }

    /// Loads Java-specific templates.
    ///
    /// Java templates include Maven/Gradle, JUnit, Checkstyle, and SpotBugs workflows.
    fn load_java_templates(&mut self) {
        self.register(
            Language::Java,
            TemplateKind::PromptBuild,
            include_str!("../templates/java/PROMPT_build.md"),
        );
        self.register(
            Language::Java,
            TemplateKind::ClaudeMd,
            include_str!("../templates/java/CLAUDE.md"),
        );
    }

    /// Loads Ruby-specific templates.
    ///
    /// Ruby templates include RSpec, RuboCop, Brakeman, and Bundler workflows.
    fn load_ruby_templates(&mut self) {
        self.register(
            Language::Ruby,
            TemplateKind::PromptBuild,
            include_str!("../templates/ruby/PROMPT_build.md"),
        );
        self.register(
            Language::Ruby,
            TemplateKind::ClaudeMd,
            include_str!("../templates/ruby/CLAUDE.md"),
        );
    }

    /// Loads PHP-specific templates.
    ///
    /// PHP templates include PHPUnit, PHPStan, PHP_CodeSniffer, and Composer workflows.
    fn load_php_templates(&mut self) {
        self.register(
            Language::Php,
            TemplateKind::PromptBuild,
            include_str!("../templates/php/PROMPT_build.md"),
        );
        self.register(
            Language::Php,
            TemplateKind::ClaudeMd,
            include_str!("../templates/php/CLAUDE.md"),
        );
    }

    /// Loads C#-specific templates.
    ///
    /// C# templates include xUnit, NUnit, MSTest, dotnet test, and Roslyn analyzer workflows.
    fn load_csharp_templates(&mut self) {
        self.register(
            Language::CSharp,
            TemplateKind::PromptBuild,
            include_str!("../templates/csharp/PROMPT_build.md"),
        );
        self.register(
            Language::CSharp,
            TemplateKind::ClaudeMd,
            include_str!("../templates/csharp/CLAUDE.md"),
        );
    }

    /// Gets a template for the given kind and language.
    ///
    /// If a language-specific template exists, it is returned. Otherwise,
    /// the default template is returned. If no template exists at all,
    /// an empty string is returned.
    ///
    /// # Arguments
    ///
    /// * `kind` - The kind of template to retrieve
    /// * `language` - The programming language
    ///
    /// # Returns
    ///
    /// The template content as a string slice, or an empty string if not found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::{TemplateRegistry, TemplateKind};
    /// use ralph::Language;
    ///
    /// let registry = TemplateRegistry::new();
    ///
    /// // Get Rust build prompt
    /// let template = registry.get(TemplateKind::PromptBuild, Language::Rust);
    /// assert!(template.contains("Build Phase"));
    ///
    /// // Unknown language falls back to default
    /// let template = registry.get(TemplateKind::PromptBuild, Language::Lua);
    /// assert!(template.contains("Build Phase"));
    /// ```
    #[must_use]
    pub fn get(&self, kind: TemplateKind, language: Language) -> &str {
        // Try language-specific template first
        if let Some(template) = self.templates.get(&(language, kind)) {
            return template;
        }

        // Fall back to default
        self.defaults.get(&kind).map(String::as_str).unwrap_or("")
    }

    /// Checks if a language-specific template exists (not falling back to default).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::{TemplateRegistry, TemplateKind};
    /// use ralph::Language;
    ///
    /// let registry = TemplateRegistry::new();
    ///
    /// // Rust has a specific template registered
    /// assert!(registry.has_language_specific(TemplateKind::PromptBuild, Language::Rust));
    ///
    /// // Lua does not (yet) have a specific template
    /// assert!(!registry.has_language_specific(TemplateKind::PromptBuild, Language::Lua));
    /// ```
    #[must_use]
    pub fn has_language_specific(&self, kind: TemplateKind, language: Language) -> bool {
        self.templates.contains_key(&(language, kind))
    }

    /// Returns a list of languages that have a specific template for the given kind.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::{TemplateRegistry, TemplateKind};
    ///
    /// let registry = TemplateRegistry::new();
    /// let languages = registry.languages_with_template(TemplateKind::PromptBuild);
    /// assert!(languages.contains(&ralph::Language::Rust));
    /// ```
    #[must_use]
    pub fn languages_with_template(&self, kind: TemplateKind) -> Vec<Language> {
        self.templates
            .keys()
            .filter(|(_, k)| *k == kind)
            .map(|(lang, _)| *lang)
            .collect()
    }

    /// Check if the given language list represents a polyglot project.
    ///
    /// A project is considered polyglot if it has two or more languages.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::TemplateRegistry;
    /// use ralph::Language;
    ///
    /// let registry = TemplateRegistry::new();
    /// assert!(!registry.is_polyglot_template(&[Language::Rust]));
    /// assert!(registry.is_polyglot_template(&[Language::Rust, Language::Python]));
    /// ```
    #[must_use]
    pub fn is_polyglot_template(&self, languages: &[Language]) -> bool {
        languages.len() >= 2
    }

    /// Get a combined template for polyglot projects.
    ///
    /// For single-language projects, returns the same as `get()`.
    /// For multi-language projects, generates a combined template with
    /// per-language sections.
    ///
    /// # Arguments
    ///
    /// * `kind` - The template kind to generate
    /// * `languages` - The languages to include in the template
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::bootstrap::templates::{TemplateRegistry, TemplateKind};
    /// use ralph::Language;
    ///
    /// let registry = TemplateRegistry::new();
    ///
    /// // Single language
    /// let single = registry.get_polyglot_prompt(TemplateKind::PromptBuild, &[Language::Rust]);
    ///
    /// // Polyglot
    /// let polyglot = registry.get_polyglot_prompt(
    ///     TemplateKind::PromptBuild,
    ///     &[Language::Rust, Language::Python],
    /// );
    /// ```
    #[must_use]
    pub fn get_polyglot_prompt(&self, kind: TemplateKind, languages: &[Language]) -> String {
        // Empty or single language: return standard template
        if languages.is_empty() {
            return self.get(kind, Language::Rust).to_string();
        }

        if languages.len() == 1 {
            return self.get(kind, languages[0]).to_string();
        }

        // Multiple languages: generate combined template
        self.generate_polyglot_template(kind, languages)
    }

    /// Generate a combined template for multiple languages.
    fn generate_polyglot_template(&self, kind: TemplateKind, languages: &[Language]) -> String {
        let mut combined = String::new();

        // Add polyglot header
        combined.push_str(&self.polyglot_header(kind, languages));

        // Add primary language section (first in list)
        if let Some(primary) = languages.first() {
            combined.push_str(&format!("\n## Primary Language: {}\n\n", primary));
            combined.push_str(self.get(kind, *primary));
        }

        // Add secondary language sections
        for lang in languages.iter().skip(1) {
            combined.push_str(&format!("\n\n---\n\n## Additional Language: {}\n\n", lang));
            combined.push_str(&self.extract_language_essentials(kind, *lang));
        }

        combined
    }

    /// Generate the polyglot header for combined templates.
    fn polyglot_header(&self, kind: TemplateKind, languages: &[Language]) -> String {
        let lang_list: Vec<String> = languages.iter().map(|l| l.to_string()).collect();

        format!(
            "# Polyglot Project: {}\n\n\
             This project uses multiple programming languages. Each section below \
             contains language-specific guidance.\n\n\
             **Languages:** {}\n\n\
             **Template:** {}\n",
            lang_list.join(" + "),
            lang_list.join(", "),
            kind.filename()
        )
    }

    /// Extract essential content from a language template for secondary sections.
    ///
    /// This provides a condensed version of the template focusing on:
    /// - Build commands
    /// - Quality gates
    /// - Key tooling
    fn extract_language_essentials(&self, kind: TemplateKind, language: Language) -> String {
        let template = self.get(kind, language);

        // For now, include the full template but mark it as secondary
        // In future iterations, we could parse and extract specific sections
        format!(
            "The following {} standards apply when working with {} code:\n\n{}",
            kind.filename(),
            language,
            template
        )
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // TemplateKind tests
    // ============================================================

    #[test]
    fn test_template_kind_all_returns_five_kinds() {
        let all = TemplateKind::all();
        assert_eq!(all.len(), 5, "Should have 5 template kinds");
    }

    #[test]
    fn test_template_kind_all_contains_required_kinds() {
        let all = TemplateKind::all();
        assert!(all.contains(&TemplateKind::PromptBuild));
        assert!(all.contains(&TemplateKind::PromptPlan));
        assert!(all.contains(&TemplateKind::PromptDebug));
        assert!(all.contains(&TemplateKind::ClaudeMd));
        assert!(all.contains(&TemplateKind::SettingsJson));
    }

    #[test]
    fn test_template_kind_filename_prompt_build() {
        assert_eq!(TemplateKind::PromptBuild.filename(), "PROMPT_build.md");
    }

    #[test]
    fn test_template_kind_filename_prompt_plan() {
        assert_eq!(TemplateKind::PromptPlan.filename(), "PROMPT_plan.md");
    }

    #[test]
    fn test_template_kind_filename_prompt_debug() {
        assert_eq!(TemplateKind::PromptDebug.filename(), "PROMPT_debug.md");
    }

    #[test]
    fn test_template_kind_filename_claude_md() {
        assert_eq!(TemplateKind::ClaudeMd.filename(), "CLAUDE.md");
    }

    #[test]
    fn test_template_kind_filename_settings_json() {
        assert_eq!(TemplateKind::SettingsJson.filename(), "settings.json");
    }

    #[test]
    fn test_template_kind_is_copy() {
        let kind = TemplateKind::PromptBuild;
        let kind2 = kind;
        assert_eq!(kind, kind2);
    }

    #[test]
    fn test_template_kind_is_hashable() {
        let mut map: HashMap<TemplateKind, u32> = HashMap::new();
        map.insert(TemplateKind::PromptBuild, 1);
        map.insert(TemplateKind::ClaudeMd, 2);
        assert_eq!(map.get(&TemplateKind::PromptBuild), Some(&1));
        assert_eq!(map.get(&TemplateKind::ClaudeMd), Some(&2));
    }

    // ============================================================
    // TemplateRegistry::new() tests
    // ============================================================

    #[test]
    fn test_registry_new_creates_instance() {
        let registry = TemplateRegistry::new();
        // Should not panic and should have defaults loaded
        assert!(!registry.defaults.is_empty());
    }

    #[test]
    fn test_registry_default_impl() {
        let registry = TemplateRegistry::default();
        // Default should work same as new()
        assert!(!registry.defaults.is_empty());
    }

    // ============================================================
    // Default template tests
    // ============================================================

    #[test]
    fn test_registry_has_default_prompt_build() {
        let registry = TemplateRegistry::new();
        let template = registry.defaults.get(&TemplateKind::PromptBuild);
        assert!(template.is_some(), "Should have default PromptBuild");
        assert!(!template.unwrap().is_empty());
    }

    #[test]
    fn test_registry_has_default_prompt_plan() {
        let registry = TemplateRegistry::new();
        let template = registry.defaults.get(&TemplateKind::PromptPlan);
        assert!(template.is_some(), "Should have default PromptPlan");
        assert!(!template.unwrap().is_empty());
    }

    #[test]
    fn test_registry_has_default_prompt_debug() {
        let registry = TemplateRegistry::new();
        let template = registry.defaults.get(&TemplateKind::PromptDebug);
        assert!(template.is_some(), "Should have default PromptDebug");
        assert!(!template.unwrap().is_empty());
    }

    #[test]
    fn test_registry_has_default_claude_md() {
        let registry = TemplateRegistry::new();
        let template = registry.defaults.get(&TemplateKind::ClaudeMd);
        assert!(template.is_some(), "Should have default ClaudeMd");
        assert!(!template.unwrap().is_empty());
    }

    #[test]
    fn test_registry_has_default_settings_json() {
        let registry = TemplateRegistry::new();
        let template = registry.defaults.get(&TemplateKind::SettingsJson);
        assert!(template.is_some(), "Should have default SettingsJson");
        assert!(!template.unwrap().is_empty());
    }

    #[test]
    fn test_registry_all_defaults_loaded() {
        let registry = TemplateRegistry::new();
        for kind in TemplateKind::all() {
            assert!(
                registry.defaults.contains_key(kind),
                "Should have default for {:?}",
                kind
            );
        }
    }

    // ============================================================
    // get() method tests
    // ============================================================

    #[test]
    fn test_get_rust_prompt_build() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Rust);
        assert!(!template.is_empty());
        assert!(template.contains("Build Phase"));
    }

    #[test]
    fn test_get_rust_claude_md() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::ClaudeMd, Language::Rust);
        assert!(!template.is_empty());
        assert!(template.contains("Project Memory"));
    }

    #[test]
    fn test_get_rust_settings_json() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Rust);
        assert!(!template.is_empty());
        assert!(template.contains("permissions"));
    }

    #[test]
    fn test_get_falls_back_to_default_for_unknown_language() {
        let registry = TemplateRegistry::new();
        // Lua doesn't have specific templates registered
        let template = registry.get(TemplateKind::PromptBuild, Language::Lua);
        assert!(!template.is_empty(), "Should fall back to default");
        assert!(template.contains("Build Phase"));
    }

    #[test]
    fn test_get_returns_empty_for_missing_template() {
        let registry = TemplateRegistry {
            templates: HashMap::new(),
            defaults: HashMap::new(),
        };
        let template = registry.get(TemplateKind::PromptBuild, Language::Rust);
        assert!(
            template.is_empty(),
            "Should return empty string when no template"
        );
    }

    #[test]
    fn test_get_all_kinds_for_rust() {
        let registry = TemplateRegistry::new();
        for kind in TemplateKind::all() {
            let template = registry.get(*kind, Language::Rust);
            assert!(
                !template.is_empty(),
                "Template {:?} should exist for Rust",
                kind
            );
        }
    }

    #[test]
    fn test_get_fallback_for_multiple_languages() {
        let registry = TemplateRegistry::new();
        // All these languages should fall back to default
        let languages = [
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::Go,
            Language::Java,
        ];

        for lang in languages {
            let template = registry.get(TemplateKind::PromptBuild, lang);
            assert!(
                !template.is_empty(),
                "Should have fallback template for {:?}",
                lang
            );
        }
    }

    // ============================================================
    // has_language_specific() tests
    // ============================================================

    #[test]
    fn test_has_language_specific_rust_true() {
        let registry = TemplateRegistry::new();
        assert!(registry.has_language_specific(TemplateKind::PromptBuild, Language::Rust));
    }

    #[test]
    fn test_has_language_specific_lua_false() {
        let registry = TemplateRegistry::new();
        assert!(!registry.has_language_specific(TemplateKind::PromptBuild, Language::Lua));
    }

    #[test]
    fn test_has_language_specific_python_true() {
        let registry = TemplateRegistry::new();
        // Python templates are now registered (Sprint 8b)
        assert!(registry.has_language_specific(TemplateKind::PromptBuild, Language::Python));
    }

    // ============================================================
    // languages_with_template() tests
    // ============================================================

    #[test]
    fn test_languages_with_template_prompt_build() {
        let registry = TemplateRegistry::new();
        let languages = registry.languages_with_template(TemplateKind::PromptBuild);
        assert!(languages.contains(&Language::Rust));
    }

    #[test]
    fn test_languages_with_template_all_kinds_have_rust() {
        let registry = TemplateRegistry::new();
        for kind in TemplateKind::all() {
            let languages = registry.languages_with_template(*kind);
            assert!(
                languages.contains(&Language::Rust),
                "Rust should have template for {:?}",
                kind
            );
        }
    }

    // ============================================================
    // Template content validation tests
    // ============================================================

    #[test]
    fn test_prompt_build_contains_tdd_reference() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Rust);
        assert!(
            template.contains("TDD") || template.contains("Test"),
            "Build prompt should reference TDD or testing"
        );
    }

    #[test]
    fn test_claude_md_contains_quality_standards() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::ClaudeMd, Language::Rust);
        assert!(
            template.contains("Quality") || template.contains("Standards"),
            "CLAUDE.md should contain quality standards"
        );
    }

    #[test]
    fn test_settings_json_is_valid_json() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Rust);
        // Should parse as valid JSON
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(template);
        assert!(parsed.is_ok(), "settings.json should be valid JSON");
    }

    // ============================================================
    // Python template tests (Sprint 8b)
    // ============================================================

    #[test]
    fn test_python_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::Python),
            "Python should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_python_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::Python),
            "Python should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_python_prompt_build_contains_pytest() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Python);
        assert!(
            template.contains("pytest") || template.contains("Pytest"),
            "Python build prompt should reference pytest"
        );
    }

    #[test]
    fn test_python_prompt_build_contains_ruff_or_flake8() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Python);
        assert!(
            template.contains("ruff") || template.contains("flake8") || template.contains("Ruff"),
            "Python build prompt should reference ruff or flake8"
        );
    }

    #[test]
    fn test_python_claude_md_contains_python_standards() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::ClaudeMd, Language::Python);
        assert!(
            template.contains("Python") || template.contains("python"),
            "Python CLAUDE.md should reference Python"
        );
    }

    // ============================================================
    // TypeScript template tests (Sprint 8c)
    // ============================================================

    #[test]
    fn test_typescript_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::TypeScript),
            "TypeScript should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_typescript_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::TypeScript),
            "TypeScript should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_typescript_prompt_build_contains_npm_or_yarn() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::TypeScript);
        assert!(
            template.contains("npm") || template.contains("yarn") || template.contains("pnpm"),
            "TypeScript build prompt should reference npm, yarn, or pnpm"
        );
    }

    #[test]
    fn test_typescript_prompt_build_contains_eslint() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::TypeScript);
        assert!(
            template.contains("eslint") || template.contains("ESLint"),
            "TypeScript build prompt should reference ESLint"
        );
    }

    #[test]
    fn test_typescript_claude_md_contains_typescript_standards() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::ClaudeMd, Language::TypeScript);
        assert!(
            template.contains("TypeScript") || template.contains("typescript"),
            "TypeScript CLAUDE.md should reference TypeScript"
        );
    }

    // ============================================================
    // Go template tests (Sprint 8d)
    // ============================================================

    #[test]
    fn test_go_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::Go),
            "Go should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_go_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::Go),
            "Go should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_go_prompt_build_contains_go_test() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Go);
        assert!(
            template.contains("go test") || template.contains("Go test"),
            "Go build prompt should reference go test"
        );
    }

    #[test]
    fn test_go_prompt_build_contains_go_vet() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Go);
        assert!(
            template.contains("go vet") || template.contains("golangci-lint"),
            "Go build prompt should reference go vet or golangci-lint"
        );
    }

    #[test]
    fn test_go_claude_md_contains_go_standards() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::ClaudeMd, Language::Go);
        assert!(
            template.contains("Go") || template.contains("golang"),
            "Go CLAUDE.md should reference Go"
        );
    }

    // ============================================================
    // Java template tests (Sprint 8e)
    // ============================================================

    #[test]
    fn test_java_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::Java),
            "Java should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_java_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::Java),
            "Java should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_java_prompt_build_contains_build_tool() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Java);
        assert!(
            template.contains("maven")
                || template.contains("Maven")
                || template.contains("gradle")
                || template.contains("Gradle"),
            "Java build prompt should reference Maven or Gradle"
        );
    }

    #[test]
    fn test_java_prompt_build_contains_junit() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Java);
        assert!(
            template.contains("JUnit") || template.contains("junit") || template.contains("test"),
            "Java build prompt should reference JUnit or testing"
        );
    }

    #[test]
    fn test_java_claude_md_contains_java_standards() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::ClaudeMd, Language::Java);
        assert!(
            template.contains("Java") || template.contains("java"),
            "Java CLAUDE.md should reference Java"
        );
    }

    // ============================================================
    // Cross-language template tests
    // ============================================================

    #[test]
    fn test_all_major_languages_have_templates() {
        let registry = TemplateRegistry::new();
        let major_languages = [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
            Language::Java,
        ];

        for lang in major_languages {
            assert!(
                registry.has_language_specific(TemplateKind::PromptBuild, lang),
                "{:?} should have PromptBuild template",
                lang
            );
            assert!(
                registry.has_language_specific(TemplateKind::ClaudeMd, lang),
                "{:?} should have ClaudeMd template",
                lang
            );
        }
    }

    #[test]
    fn test_all_language_templates_contain_tdd() {
        let registry = TemplateRegistry::new();
        let languages = [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
            Language::Java,
        ];

        for lang in languages {
            let template = registry.get(TemplateKind::PromptBuild, lang);
            assert!(
                template.contains("TDD") || template.contains("Test") || template.contains("test"),
                "{:?} build prompt should reference testing",
                lang
            );
        }
    }

    #[test]
    fn test_all_language_templates_contain_quality_gates() {
        let registry = TemplateRegistry::new();
        let languages = [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
            Language::Java,
        ];

        for lang in languages {
            let template = registry.get(TemplateKind::PromptBuild, lang);
            assert!(
                template.contains("Quality")
                    || template.contains("gate")
                    || template.contains("Gate"),
                "{:?} build prompt should reference quality gates",
                lang
            );
        }
    }

    // ============================================================
    // Language-specific settings.json tests (Sprint 9b-9d)
    // ============================================================

    #[test]
    fn test_python_has_specific_settings_json() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::SettingsJson, Language::Python),
            "Python should have a specific SettingsJson template"
        );
    }

    #[test]
    fn test_python_settings_json_is_valid_json() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Python);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(template);
        assert!(parsed.is_ok(), "Python settings.json should be valid JSON");
    }

    #[test]
    fn test_python_settings_json_contains_python_tools() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Python);
        assert!(
            template.contains("python") || template.contains("pytest"),
            "Python settings.json should reference Python tools"
        );
    }

    #[test]
    fn test_python_settings_json_allows_pytest() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Python);
        assert!(
            template.contains("pytest"),
            "Python settings.json should allow pytest"
        );
    }

    #[test]
    fn test_python_settings_json_allows_ruff() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Python);
        assert!(
            template.contains("ruff"),
            "Python settings.json should allow ruff"
        );
    }

    #[test]
    fn test_typescript_has_specific_settings_json() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::SettingsJson, Language::TypeScript),
            "TypeScript should have a specific SettingsJson template"
        );
    }

    #[test]
    fn test_typescript_settings_json_is_valid_json() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::TypeScript);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(template);
        assert!(
            parsed.is_ok(),
            "TypeScript settings.json should be valid JSON"
        );
    }

    #[test]
    fn test_typescript_settings_json_contains_npm() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::TypeScript);
        assert!(
            template.contains("npm") || template.contains("npx"),
            "TypeScript settings.json should reference npm"
        );
    }

    #[test]
    fn test_typescript_settings_json_allows_tsc() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::TypeScript);
        assert!(
            template.contains("tsc") || template.contains("npx tsc"),
            "TypeScript settings.json should allow tsc"
        );
    }

    #[test]
    fn test_typescript_settings_json_allows_eslint() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::TypeScript);
        assert!(
            template.contains("eslint"),
            "TypeScript settings.json should allow eslint"
        );
    }

    #[test]
    fn test_go_has_specific_settings_json() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::SettingsJson, Language::Go),
            "Go should have a specific SettingsJson template"
        );
    }

    #[test]
    fn test_go_settings_json_is_valid_json() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Go);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(template);
        assert!(parsed.is_ok(), "Go settings.json should be valid JSON");
    }

    #[test]
    fn test_go_settings_json_contains_go_tools() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Go);
        assert!(
            template.contains("\"go ") || template.contains("go test"),
            "Go settings.json should reference Go tools"
        );
    }

    #[test]
    fn test_go_settings_json_allows_go_test() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Go);
        assert!(
            template.contains("go ") || template.contains("Bash(go"),
            "Go settings.json should allow go commands"
        );
    }

    #[test]
    fn test_go_settings_json_allows_golangci_lint() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::SettingsJson, Language::Go);
        assert!(
            template.contains("golangci-lint"),
            "Go settings.json should allow golangci-lint"
        );
    }

    #[test]
    fn test_all_major_languages_have_settings_json() {
        let registry = TemplateRegistry::new();
        let major_languages = [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
        ];

        for lang in major_languages {
            assert!(
                registry.has_language_specific(TemplateKind::SettingsJson, lang),
                "{:?} should have SettingsJson template",
                lang
            );
        }
    }

    #[test]
    fn test_all_settings_json_have_permissions_section() {
        let registry = TemplateRegistry::new();
        let languages = [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
        ];

        for lang in languages {
            let template = registry.get(TemplateKind::SettingsJson, lang);
            assert!(
                template.contains("permissions"),
                "{:?} settings.json should have permissions section",
                lang
            );
        }
    }

    #[test]
    fn test_all_settings_json_deny_dangerous_commands() {
        let registry = TemplateRegistry::new();
        let languages = [
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
        ];

        for lang in languages {
            let template = registry.get(TemplateKind::SettingsJson, lang);
            assert!(
                template.contains("rm -rf /") || template.contains("deny"),
                "{:?} settings.json should deny dangerous commands",
                lang
            );
        }
    }

    // ============================================================
    // Polyglot prompt generation tests (Sprint 10)
    // ============================================================

    #[test]
    fn test_get_polyglot_prompt_with_single_language() {
        let registry = TemplateRegistry::new();
        let prompt = registry.get_polyglot_prompt(TemplateKind::PromptBuild, &[Language::Rust]);

        // Single language should return same as get()
        let single = registry.get(TemplateKind::PromptBuild, Language::Rust);
        assert_eq!(prompt, single);
    }

    #[test]
    fn test_get_polyglot_prompt_with_two_languages() {
        let registry = TemplateRegistry::new();
        let prompt = registry.get_polyglot_prompt(
            TemplateKind::PromptBuild,
            &[Language::Rust, Language::Python],
        );

        // Should contain polyglot header
        assert!(
            prompt.contains("Polyglot")
                || prompt.contains("polyglot")
                || prompt.contains("Multi-language"),
            "Polyglot prompt should indicate multi-language project"
        );

        // Should contain Rust section
        assert!(
            prompt.contains("Rust") || prompt.contains("cargo"),
            "Polyglot prompt should contain Rust section"
        );

        // Should contain Python section
        assert!(
            prompt.contains("Python") || prompt.contains("pytest"),
            "Polyglot prompt should contain Python section"
        );
    }

    #[test]
    fn test_get_polyglot_prompt_with_three_languages() {
        let registry = TemplateRegistry::new();
        let prompt = registry.get_polyglot_prompt(
            TemplateKind::PromptBuild,
            &[Language::Rust, Language::Python, Language::TypeScript],
        );

        // Should contain all three language references
        assert!(prompt.contains("Rust") || prompt.contains("cargo"));
        assert!(prompt.contains("Python") || prompt.contains("pytest"));
        assert!(prompt.contains("TypeScript") || prompt.contains("npm"));
    }

    #[test]
    fn test_get_polyglot_prompt_empty_languages_returns_default() {
        let registry = TemplateRegistry::new();
        let prompt = registry.get_polyglot_prompt(TemplateKind::PromptBuild, &[]);

        // Empty should return default template
        let default = registry.get(TemplateKind::PromptBuild, Language::Rust);
        assert!(!prompt.is_empty());
        assert_eq!(prompt, default);
    }

    #[test]
    fn test_get_polyglot_prompt_preserves_quality_gates_for_all_languages() {
        let registry = TemplateRegistry::new();
        let prompt = registry.get_polyglot_prompt(
            TemplateKind::PromptBuild,
            &[Language::Rust, Language::Python],
        );

        // Should reference quality concepts from both languages
        assert!(
            prompt.contains("Quality") || prompt.contains("gate") || prompt.contains("Gate"),
            "Polyglot prompt should contain quality gate references"
        );
    }

    #[test]
    fn test_get_polyglot_claude_md() {
        let registry = TemplateRegistry::new();
        let claude_md = registry.get_polyglot_prompt(
            TemplateKind::ClaudeMd,
            &[Language::Rust, Language::TypeScript],
        );

        // Should contain project info for both languages
        assert!(
            claude_md.contains("Rust") || claude_md.contains("cargo"),
            "Polyglot CLAUDE.md should reference Rust"
        );
        assert!(
            claude_md.contains("TypeScript") || claude_md.contains("npm"),
            "Polyglot CLAUDE.md should reference TypeScript"
        );
    }

    #[test]
    fn test_is_polyglot_template_with_multiple_languages() {
        let registry = TemplateRegistry::new();

        // Single language is not polyglot
        assert!(!registry.is_polyglot_template(&[Language::Rust]));

        // Multiple languages is polyglot
        assert!(registry.is_polyglot_template(&[Language::Rust, Language::Python]));

        // Empty is not polyglot
        assert!(!registry.is_polyglot_template(&[]));
    }

    // ============================================================
    // Additional language template tests (Sprint 10)
    // ============================================================

    #[test]
    fn test_ruby_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::Ruby),
            "Ruby should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_ruby_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::Ruby),
            "Ruby should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_ruby_prompt_build_contains_rspec() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Ruby);
        assert!(
            template.contains("rspec") || template.contains("RSpec"),
            "Ruby build prompt should reference RSpec"
        );
    }

    #[test]
    fn test_ruby_prompt_build_contains_rubocop() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Ruby);
        assert!(
            template.contains("rubocop") || template.contains("RuboCop"),
            "Ruby build prompt should reference RuboCop"
        );
    }

    #[test]
    fn test_php_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::Php),
            "PHP should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_php_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::Php),
            "PHP should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_php_prompt_build_contains_phpunit() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::Php);
        assert!(
            template.contains("phpunit") || template.contains("PHPUnit"),
            "PHP build prompt should reference PHPUnit"
        );
    }

    #[test]
    fn test_csharp_has_specific_prompt_build() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::PromptBuild, Language::CSharp),
            "C# should have a specific PromptBuild template"
        );
    }

    #[test]
    fn test_csharp_has_specific_claude_md() {
        let registry = TemplateRegistry::new();
        assert!(
            registry.has_language_specific(TemplateKind::ClaudeMd, Language::CSharp),
            "C# should have a specific ClaudeMd template"
        );
    }

    #[test]
    fn test_csharp_prompt_build_contains_dotnet_test() {
        let registry = TemplateRegistry::new();
        let template = registry.get(TemplateKind::PromptBuild, Language::CSharp);
        assert!(
            template.contains("dotnet test")
                || template.contains("xunit")
                || template.contains("MSTest"),
            "C# build prompt should reference dotnet test or testing framework"
        );
    }

    #[test]
    fn test_generic_fallback_exists() {
        let registry = TemplateRegistry::new();
        // Lua doesn't have specific templates, should use generic
        let template = registry.get(TemplateKind::PromptBuild, Language::Lua);
        assert!(
            !template.is_empty(),
            "Languages without specific templates should get generic fallback"
        );
        assert!(
            template.contains("Quality") || template.contains("TDD"),
            "Generic template should contain quality/TDD guidance"
        );
    }
}
