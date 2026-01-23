//! Prompt assembly and generation.
//!
//! This module provides the `DynamicPromptBuilder` which assembles complete
//! prompts from templates and context by substituting template markers with
//! generated sections.

use std::collections::HashMap;

use crate::prompt::context::PromptContext;
use crate::prompt::templates::{PromptTemplates, TemplateMarker};

use super::SectionBuilder;

/// Dynamic prompt builder that assembles complete prompts from templates and context.
///
/// # Example
///
/// ```
/// use ralph::prompt::builder::DynamicPromptBuilder;
/// use ralph::prompt::context::{PromptContext, SessionStats};
/// use ralph::prompt::templates::PromptTemplates;
///
/// let templates = PromptTemplates::with_defaults();
/// let builder = DynamicPromptBuilder::new(templates);
///
/// let context = PromptContext::new()
///     .with_session_stats(SessionStats::new(5, 2, 150));
///
/// let prompt = builder.build("build", &context);
/// assert!(prompt.is_ok());
/// ```
#[derive(Debug)]
pub struct DynamicPromptBuilder {
    templates: PromptTemplates,
}

impl DynamicPromptBuilder {
    /// Create a new builder with the given templates.
    #[must_use]
    pub fn new(templates: PromptTemplates) -> Self {
        Self { templates }
    }

    /// Build a complete prompt for the given mode and context.
    ///
    /// # Errors
    ///
    /// Returns an error if the template for the given mode doesn't exist.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::builder::DynamicPromptBuilder;
    /// use ralph::prompt::context::PromptContext;
    /// use ralph::prompt::templates::PromptTemplates;
    ///
    /// let templates = PromptTemplates::with_defaults();
    /// let builder = DynamicPromptBuilder::new(templates);
    ///
    /// let result = builder.build("build", &PromptContext::new());
    /// assert!(result.is_ok());
    /// ```
    pub fn build(&self, mode: &str, context: &PromptContext) -> anyhow::Result<String> {
        let template = self
            .templates
            .get_template(mode)
            .ok_or_else(|| anyhow::anyhow!("Template not found for mode: {}", mode))?;

        let mut substitutions = HashMap::new();

        // Build each section and add to substitutions
        if let Some(task) = &context.current_task {
            let section = SectionBuilder::build_task_section(task);
            substitutions.insert(TemplateMarker::TaskContext, section);
        } else {
            substitutions.insert(TemplateMarker::TaskContext, String::new());
        }

        let error_section = SectionBuilder::build_error_section(&context.errors);
        substitutions.insert(TemplateMarker::ErrorContext, error_section);

        let quality_section = SectionBuilder::build_quality_section(&context.quality_status);
        substitutions.insert(TemplateMarker::QualityStatus, quality_section);

        let session_section = SectionBuilder::build_session_section(&context.session_stats);
        substitutions.insert(TemplateMarker::SessionStats, session_section);

        let attempt_section = SectionBuilder::build_attempt_section(&context.attempt_summaries);
        substitutions.insert(TemplateMarker::AttemptHistory, attempt_section);

        let antipattern_section = SectionBuilder::build_antipattern_section(&context.anti_patterns);
        substitutions.insert(TemplateMarker::AntiPatterns, antipattern_section);

        // Code intelligence section
        let intelligence_section =
            SectionBuilder::build_intelligence_section(&context.code_intelligence);
        substitutions.insert(TemplateMarker::CodeIntelligence, intelligence_section);

        // Language-specific quality rules
        let language_rules_section =
            SectionBuilder::build_language_rules_section(&context.language_rules);
        substitutions.insert(TemplateMarker::LanguageRules, language_rules_section);

        // Code antipattern warnings
        let code_antipattern_section =
            SectionBuilder::build_code_antipattern_section(&context.code_antipattern_warnings);
        substitutions.insert(
            TemplateMarker::CodeAntipatternWarnings,
            code_antipattern_section,
        );

        // Historical guidance placeholder (populated by history module)
        substitutions.insert(TemplateMarker::HistoricalGuidance, String::new());

        // Custom section placeholder
        substitutions.insert(TemplateMarker::CustomSection, String::new());

        // Apply all substitutions
        let substitution_refs: HashMap<TemplateMarker, &str> = substitutions
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let result = template.substitute_all(&substitution_refs);

        // Clean up any remaining markers and extra newlines
        let cleaned = result.remove_unreplaced_markers();

        Ok(cleaned.content().to_string())
    }

    /// Build a prompt with custom sections.
    ///
    /// Allows adding additional sections not covered by the standard context.
    pub fn build_with_custom(
        &self,
        mode: &str,
        context: &PromptContext,
        custom_sections: &HashMap<TemplateMarker, String>,
    ) -> anyhow::Result<String> {
        let template = self
            .templates
            .get_template(mode)
            .ok_or_else(|| anyhow::anyhow!("Template not found for mode: {}", mode))?;

        let mut substitutions = HashMap::new();

        // Build standard sections
        if let Some(task) = &context.current_task {
            substitutions.insert(
                TemplateMarker::TaskContext,
                SectionBuilder::build_task_section(task),
            );
        } else {
            substitutions.insert(TemplateMarker::TaskContext, String::new());
        }

        substitutions.insert(
            TemplateMarker::ErrorContext,
            SectionBuilder::build_error_section(&context.errors),
        );
        substitutions.insert(
            TemplateMarker::QualityStatus,
            SectionBuilder::build_quality_section(&context.quality_status),
        );
        substitutions.insert(
            TemplateMarker::SessionStats,
            SectionBuilder::build_session_section(&context.session_stats),
        );
        substitutions.insert(
            TemplateMarker::AttemptHistory,
            SectionBuilder::build_attempt_section(&context.attempt_summaries),
        );
        substitutions.insert(
            TemplateMarker::AntiPatterns,
            SectionBuilder::build_antipattern_section(&context.anti_patterns),
        );
        substitutions.insert(
            TemplateMarker::CodeIntelligence,
            SectionBuilder::build_intelligence_section(&context.code_intelligence),
        );
        substitutions.insert(
            TemplateMarker::LanguageRules,
            SectionBuilder::build_language_rules_section(&context.language_rules),
        );
        substitutions.insert(
            TemplateMarker::CodeAntipatternWarnings,
            SectionBuilder::build_code_antipattern_section(&context.code_antipattern_warnings),
        );
        substitutions.insert(TemplateMarker::HistoricalGuidance, String::new());
        substitutions.insert(TemplateMarker::CustomSection, String::new());

        // Override with custom sections
        for (marker, content) in custom_sections {
            substitutions.insert(*marker, content.clone());
        }

        let substitution_refs: HashMap<TemplateMarker, &str> = substitutions
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let result = template.substitute_all(&substitution_refs);
        let cleaned = result.remove_unreplaced_markers();

        Ok(cleaned.content().to_string())
    }

    /// Get a reference to the templates.
    #[must_use]
    pub fn templates(&self) -> &PromptTemplates {
        &self.templates
    }
}

impl Default for DynamicPromptBuilder {
    fn default() -> Self {
        Self::new(PromptTemplates::with_defaults())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::context::{
        CallGraphNode, CodeIntelligenceContext, CurrentTaskContext, ErrorContext, ErrorSeverity,
        TaskPhase,
    };

    #[test]
    fn test_dynamic_prompt_builder_new() {
        let builder = DynamicPromptBuilder::default();
        assert!(builder.templates().has_template("build"));
        assert!(builder.templates().has_template("debug"));
        assert!(builder.templates().has_template("plan"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_basic() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("Build Phase"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_with_task() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new().with_current_task(CurrentTaskContext::new(
            "2.1",
            "Test task",
            TaskPhase::Implementation,
        ));

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("2.1"));
        assert!(prompt.contains("Test task"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_with_errors() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new().with_error(ErrorContext::new(
            "E0308",
            "mismatched",
            ErrorSeverity::Error,
        ));

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("E0308"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_unknown_mode() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("unknown", &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_dynamic_prompt_builder_build_debug_mode() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("debug", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("Debug Phase"));
    }

    #[test]
    fn test_dynamic_prompt_builder_build_plan_mode() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let result = builder.build("plan", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("Plan Phase"));
    }

    #[test]
    fn test_dynamic_prompt_builder_removes_unreplaced_markers() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new(); // No errors, no task

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        // Should not contain raw markers
        assert!(!prompt.contains("{{TASK_CONTEXT}}"));
        assert!(!prompt.contains("{{ERROR_CONTEXT}}"));
    }

    #[test]
    fn test_dynamic_prompt_builder_with_custom_sections() {
        let builder = DynamicPromptBuilder::default();
        let context = PromptContext::new();

        let mut custom = HashMap::new();
        custom.insert(
            TemplateMarker::CustomSection,
            "## Custom\n\nCustom content".to_string(),
        );

        let result = builder.build_with_custom("build", &context, &custom);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dynamic_prompt_builder_with_code_intelligence() {
        let builder = DynamicPromptBuilder::default();
        let intel = CodeIntelligenceContext::new()
            .with_call_graph(vec![CallGraphNode::new("test_func")])
            .mark_available();
        let context = PromptContext::new().with_code_intelligence(intel);

        let result = builder.build("build", &context);
        assert!(result.is_ok());

        let prompt = result.unwrap();
        assert!(prompt.contains("test_func") || prompt.contains("Code Intelligence"));
    }
}
