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
        // Future: load_python_templates(), load_typescript_templates(), etc.

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
        self.templates
            .insert((language, kind), content.to_string());
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
    /// For now, Rust uses the default templates. In the future, Rust-specific
    /// variations may be added.
    fn load_rust_templates(&mut self) {
        // Rust currently uses defaults - language-specific templates will be
        // added in Sprint 8b-8e
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
    fn test_has_language_specific_python_false() {
        let registry = TemplateRegistry::new();
        // Python templates not yet registered
        assert!(!registry.has_language_specific(TemplateKind::PromptBuild, Language::Python));
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
}
