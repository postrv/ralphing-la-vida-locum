//! Base template system for prompt generation.
//!
//! This module provides template loading and variable substitution for dynamic prompts.
//!
//! # Example
//!
//! ```
//! use ralph::prompt::templates::{PromptTemplates, TemplateMarker};
//!
//! let templates = PromptTemplates::with_defaults();
//! assert!(templates.markers().contains(&TemplateMarker::TaskContext));
//! ```

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Template variable markers that can be substituted in templates.
///
/// These markers define injection points for dynamic content.
///
/// # Example
///
/// ```
/// use ralph::prompt::templates::TemplateMarker;
///
/// let marker = TemplateMarker::TaskContext;
/// assert_eq!(marker.tag(), "{{TASK_CONTEXT}}");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateMarker {
    /// Current task context injection point.
    TaskContext,
    /// Error context injection point.
    ErrorContext,
    /// Quality gate status injection point.
    QualityStatus,
    /// Session statistics injection point.
    SessionStats,
    /// Previous attempt summaries injection point.
    AttemptHistory,
    /// Detected anti-patterns injection point.
    AntiPatterns,
    /// Historical guidance injection point.
    HistoricalGuidance,
    /// Code intelligence data injection point.
    CodeIntelligence,
    /// Language-specific quality rules injection point.
    LanguageRules,
    /// Code antipattern warnings injection point.
    CodeAntipatternWarnings,
    /// Custom section injection point.
    CustomSection,
}

impl TemplateMarker {
    /// Get the template tag string for this marker.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::TemplateMarker;
    ///
    /// assert_eq!(TemplateMarker::ErrorContext.tag(), "{{ERROR_CONTEXT}}");
    /// ```
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            TemplateMarker::TaskContext => "{{TASK_CONTEXT}}",
            TemplateMarker::ErrorContext => "{{ERROR_CONTEXT}}",
            TemplateMarker::QualityStatus => "{{QUALITY_STATUS}}",
            TemplateMarker::SessionStats => "{{SESSION_STATS}}",
            TemplateMarker::AttemptHistory => "{{ATTEMPT_HISTORY}}",
            TemplateMarker::AntiPatterns => "{{ANTI_PATTERNS}}",
            TemplateMarker::HistoricalGuidance => "{{HISTORICAL_GUIDANCE}}",
            TemplateMarker::CodeIntelligence => "{{CODE_INTELLIGENCE}}",
            TemplateMarker::LanguageRules => "{{LANGUAGE_RULES}}",
            TemplateMarker::CodeAntipatternWarnings => "{{CODE_ANTIPATTERN_WARNINGS}}",
            TemplateMarker::CustomSection => "{{CUSTOM_SECTION}}",
        }
    }

    /// Get all available markers.
    #[must_use]
    pub fn all() -> &'static [TemplateMarker] {
        &[
            TemplateMarker::TaskContext,
            TemplateMarker::ErrorContext,
            TemplateMarker::QualityStatus,
            TemplateMarker::SessionStats,
            TemplateMarker::AttemptHistory,
            TemplateMarker::AntiPatterns,
            TemplateMarker::HistoricalGuidance,
            TemplateMarker::CodeIntelligence,
            TemplateMarker::LanguageRules,
            TemplateMarker::CodeAntipatternWarnings,
            TemplateMarker::CustomSection,
        ]
    }

    /// Parse a tag string into a marker.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::TemplateMarker;
    ///
    /// assert_eq!(
    ///     TemplateMarker::from_tag("{{TASK_CONTEXT}}"),
    ///     Some(TemplateMarker::TaskContext)
    /// );
    /// assert_eq!(TemplateMarker::from_tag("invalid"), None);
    /// ```
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<TemplateMarker> {
        match tag {
            "{{TASK_CONTEXT}}" => Some(TemplateMarker::TaskContext),
            "{{ERROR_CONTEXT}}" => Some(TemplateMarker::ErrorContext),
            "{{QUALITY_STATUS}}" => Some(TemplateMarker::QualityStatus),
            "{{SESSION_STATS}}" => Some(TemplateMarker::SessionStats),
            "{{ATTEMPT_HISTORY}}" => Some(TemplateMarker::AttemptHistory),
            "{{ANTI_PATTERNS}}" => Some(TemplateMarker::AntiPatterns),
            "{{HISTORICAL_GUIDANCE}}" => Some(TemplateMarker::HistoricalGuidance),
            "{{CODE_INTELLIGENCE}}" => Some(TemplateMarker::CodeIntelligence),
            "{{LANGUAGE_RULES}}" => Some(TemplateMarker::LanguageRules),
            "{{CODE_ANTIPATTERN_WARNINGS}}" => Some(TemplateMarker::CodeAntipatternWarnings),
            "{{CUSTOM_SECTION}}" => Some(TemplateMarker::CustomSection),
            _ => None,
        }
    }
}

impl std::fmt::Display for TemplateMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tag())
    }
}

/// A parsed template with its content and detected markers.
///
/// # Example
///
/// ```
/// use ralph::prompt::templates::Template;
///
/// let template = Template::new("# Title\n\n{{TASK_CONTEXT}}\n\nContent");
/// assert!(template.has_marker(ralph::prompt::templates::TemplateMarker::TaskContext));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Raw template content.
    content: String,
    /// Markers found in the template.
    markers: Vec<TemplateMarker>,
}

impl Template {
    /// Create a new template from content.
    ///
    /// Automatically detects markers in the content.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::Template;
    ///
    /// let template = Template::new("Hello {{TASK_CONTEXT}} world");
    /// assert_eq!(template.markers().len(), 1);
    /// ```
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let markers = Self::detect_markers(&content);
        Self { content, markers }
    }

    /// Get the raw template content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the markers found in this template.
    #[must_use]
    pub fn markers(&self) -> &[TemplateMarker] {
        &self.markers
    }

    /// Check if this template has a specific marker.
    #[must_use]
    pub fn has_marker(&self, marker: TemplateMarker) -> bool {
        self.markers.contains(&marker)
    }

    /// Detect all markers in the content.
    fn detect_markers(content: &str) -> Vec<TemplateMarker> {
        let mut markers = Vec::new();
        for marker in TemplateMarker::all() {
            if content.contains(marker.tag()) {
                markers.push(*marker);
            }
        }
        markers
    }

    /// Substitute a marker with content.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::{Template, TemplateMarker};
    ///
    /// let template = Template::new("Hello {{TASK_CONTEXT}}!");
    /// let result = template.substitute(TemplateMarker::TaskContext, "World");
    /// assert_eq!(result.content(), "Hello World!");
    /// ```
    #[must_use]
    pub fn substitute(&self, marker: TemplateMarker, replacement: &str) -> Template {
        let new_content = self.content.replace(marker.tag(), replacement);
        Template::new(new_content)
    }

    /// Substitute multiple markers at once.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::{Template, TemplateMarker};
    /// use std::collections::HashMap;
    ///
    /// let template = Template::new("{{TASK_CONTEXT}} and {{ERROR_CONTEXT}}");
    /// let mut subs = HashMap::new();
    /// subs.insert(TemplateMarker::TaskContext, "Task");
    /// subs.insert(TemplateMarker::ErrorContext, "Errors");
    ///
    /// let result = template.substitute_all(&subs);
    /// assert_eq!(result.content(), "Task and Errors");
    /// ```
    #[must_use]
    pub fn substitute_all(&self, substitutions: &HashMap<TemplateMarker, &str>) -> Template {
        let mut content = self.content.clone();
        for (marker, replacement) in substitutions {
            content = content.replace(marker.tag(), replacement);
        }
        Template::new(content)
    }

    /// Remove all unreplaced markers (clean up).
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::Template;
    ///
    /// let template = Template::new("Hello {{TASK_CONTEXT}}!");
    /// let cleaned = template.remove_unreplaced_markers();
    /// assert_eq!(cleaned.content(), "Hello !");
    /// ```
    #[must_use]
    pub fn remove_unreplaced_markers(&self) -> Template {
        let mut content = self.content.clone();
        for marker in TemplateMarker::all() {
            content = content.replace(marker.tag(), "");
        }
        // Clean up double newlines that may result from marker removal
        while content.contains("\n\n\n") {
            content = content.replace("\n\n\n", "\n\n");
        }
        Template::new(content)
    }

    /// Insert content before a marker (prepend section).
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::templates::{Template, TemplateMarker};
    ///
    /// let template = Template::new("# Title\n\n{{TASK_CONTEXT}}\n\nContent");
    /// let result = template.insert_before(TemplateMarker::TaskContext, "## New Section\n\n");
    /// assert!(result.content().contains("## New Section\n\n{{TASK_CONTEXT}}"));
    /// ```
    #[must_use]
    pub fn insert_before(&self, marker: TemplateMarker, content: &str) -> Template {
        let new_content = self
            .content
            .replace(marker.tag(), &format!("{}{}", content, marker.tag()));
        Template::new(new_content)
    }

    /// Insert content after a marker (append section).
    #[must_use]
    pub fn insert_after(&self, marker: TemplateMarker, content: &str) -> Template {
        let new_content = self
            .content
            .replace(marker.tag(), &format!("{}{}", marker.tag(), content));
        Template::new(new_content)
    }
}

/// Template collection for all prompt modes.
///
/// # Example
///
/// ```
/// use ralph::prompt::templates::PromptTemplates;
///
/// let templates = PromptTemplates::with_defaults();
/// assert!(templates.get_template("build").is_some());
/// ```
#[derive(Debug, Clone, Default)]
pub struct PromptTemplates {
    /// Templates by mode name.
    templates: HashMap<String, Template>,
}

impl PromptTemplates {
    /// Create an empty template collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create templates with default embedded content.
    ///
    /// This provides fallback templates if no files are found.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut templates = Self::new();

        // Default build template
        templates.add_template("build", Template::new(DEFAULT_BUILD_TEMPLATE));
        // Default debug template
        templates.add_template("debug", Template::new(DEFAULT_DEBUG_TEMPLATE));
        // Default plan template
        templates.add_template("plan", Template::new(DEFAULT_PLAN_TEMPLATE));

        templates
    }

    /// Load templates from a directory.
    ///
    /// Looks for files matching `PROMPT_*.md` pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ralph::prompt::templates::PromptTemplates;
    ///
    /// let templates = PromptTemplates::load_from_dir("src/templates")?;
    /// ```
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let mut templates = Self::new();

        if !dir.exists() {
            return Ok(templates);
        }

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read templates directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("PROMPT_") && file_name.ends_with(".md") {
                    // Extract mode name: PROMPT_build.md -> build
                    let mode = file_name
                        .strip_prefix("PROMPT_")
                        .and_then(|s| s.strip_suffix(".md"))
                        .unwrap_or("");

                    if !mode.is_empty() {
                        let content = std::fs::read_to_string(&path).with_context(|| {
                            format!("Failed to read template: {}", path.display())
                        })?;
                        templates.add_template(mode, Template::new(content));
                    }
                }
            }
        }

        Ok(templates)
    }

    /// Load templates from a directory, falling back to defaults if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if files exist but cannot be read.
    pub fn load_or_defaults(dir: impl AsRef<Path>) -> Result<Self> {
        let mut templates = Self::load_from_dir(dir)?;

        // Add defaults for any missing modes
        let defaults = Self::with_defaults();
        for mode in ["build", "debug", "plan"] {
            if !templates.has_template(mode) {
                if let Some(template) = defaults.get_template(mode) {
                    templates.add_template(mode, template.clone());
                }
            }
        }

        Ok(templates)
    }

    /// Add a template for a mode.
    pub fn add_template(&mut self, mode: impl Into<String>, template: Template) {
        self.templates.insert(mode.into(), template);
    }

    /// Get a template by mode name.
    #[must_use]
    pub fn get_template(&self, mode: &str) -> Option<&Template> {
        self.templates.get(mode)
    }

    /// Check if a template exists for a mode.
    #[must_use]
    pub fn has_template(&self, mode: &str) -> bool {
        self.templates.contains_key(mode)
    }

    /// Get all available mode names.
    #[must_use]
    pub fn modes(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// Get all available markers across all templates.
    #[must_use]
    pub fn markers(&self) -> Vec<TemplateMarker> {
        let mut markers = Vec::new();
        for template in self.templates.values() {
            for marker in template.markers() {
                if !markers.contains(marker) {
                    markers.push(*marker);
                }
            }
        }
        markers
    }

    /// Validate that all templates have required markers.
    ///
    /// # Errors
    ///
    /// Returns an error if any template is missing required markers.
    pub fn validate(&self, required_markers: &[TemplateMarker]) -> Result<()> {
        for (mode, template) in &self.templates {
            for marker in required_markers {
                if !template.has_marker(*marker) {
                    bail!(
                        "Template '{}' is missing required marker: {}",
                        mode,
                        marker.tag()
                    );
                }
            }
        }
        Ok(())
    }

    /// Validate templates against a set of optional markers.
    ///
    /// Returns a list of warnings for missing optional markers.
    #[must_use]
    pub fn validate_optional(&self, optional_markers: &[TemplateMarker]) -> Vec<String> {
        let mut warnings = Vec::new();
        for (mode, template) in &self.templates {
            for marker in optional_markers {
                if !template.has_marker(*marker) {
                    warnings.push(format!(
                        "Template '{}' is missing optional marker: {}",
                        mode,
                        marker.tag()
                    ));
                }
            }
        }
        warnings
    }
}

// Default template content with markers

const DEFAULT_BUILD_TEMPLATE: &str = r#"# Build Phase - Production Standard

{{TASK_CONTEXT}}

{{ERROR_CONTEXT}}

{{QUALITY_STATUS}}

{{ANTI_PATTERNS}}

{{CODE_INTELLIGENCE}}

{{LANGUAGE_RULES}}

## Phase 1: PLAN
- Read IMPLEMENTATION_PLAN.md
- Select highest-priority incomplete task
- Use narsil-mcp for context: `get_call_graph`, `get_dependencies`, `find_references`
- Identify all types/functions that will be affected

## Phase 2: TEST FIRST (TDD)
**Before writing ANY implementation code:**
1. Write failing test(s) that define the expected behavior
2. Run tests to confirm they fail for the right reason
3. Document the behavioral contract in test comments
4. If modifying existing code, ensure existing tests still define correct behavior

**Test Requirements:**
- Every public function must have at least one test
- Every public type must be exercised in integration tests
- Edge cases must be tested (empty inputs, errors, boundaries)
- Use `#[should_panic]` for expected panic paths

## Phase 3: IMPLEMENT
- Write minimal code to make tests pass
- Use `find_references` to ensure no breaking changes
- Update inline documentation with `# Examples` and `# Panics`

**Implementation Rules:**
- NO `#[allow(...)]` annotations - fix warnings at source
- NO `#[dead_code]` - if it exists, it must be tested and used
- NO placeholder/stub implementations - fully implement or don't merge
- NO TODO/FIXME comments in merged code
- Every warning is a bug - resolve before proceeding

## Phase 4: REFACTOR
- Clean up implementation while keeping tests green
- Extract common patterns into helpers (only if used 3+ times)
- Ensure code follows existing project patterns

## Phase 5: REVIEW
- Run `cargo clippy --all-targets -- -D warnings` (treat warnings as errors)
- Run `cargo test` (all tests must pass)
- Run narsil-mcp security scans:
  - `scan_security` - resolve all CRITICAL/HIGH
  - `find_injection_vulnerabilities` - must be zero findings
  - `check_cwe_top25` - review any new findings
- Check documentation drift - update docs/ if API changed

## Phase 6: COMMIT
- Run full test suite one more time
- If ALL checks pass: `git add -A && git commit -m "feat: [description]"`
- Update IMPLEMENTATION_PLAN.md marking task complete
- If ANY check fails: DO NOT COMMIT - fix issues first

{{SESSION_STATS}}

## Hard Rules (Violations = Immediate Stop)

1. **NEVER modify existing tests to make them pass** - tests define correct behavior
2. **NEVER use #[allow(...)]** - fix the underlying issue
3. **NEVER leave dead code** - delete or wire in with tests
4. **NEVER commit with warnings** - `cargo clippy` must be clean
5. **NEVER commit with failing tests** - all tests must pass
6. **NEVER skip security review** - scan_security before every commit
7. **NEVER add code without tests** - TDD is mandatory
8. **ALWAYS search before implementing** - use `find_similar_code` to avoid duplication

## Quality Gates (Must Pass Before Commit)

```
[ ] cargo clippy --all-targets -- -D warnings  (0 warnings)
[ ] cargo test                                  (all pass)
[ ] scan_security                               (0 CRITICAL/HIGH)
[ ] find_injection_vulnerabilities              (0 findings)
[ ] All new public APIs documented              (docs/api.md updated)
[ ] All new types have tests                    (coverage verified)
```

## TDD Cycle Summary

```
RED    -> Write failing test
GREEN  -> Write minimal code to pass
REFACTOR -> Clean up, keeping tests green
REVIEW -> Security + clippy + full test suite
COMMIT -> Only if ALL gates pass
```
"#;

const DEFAULT_DEBUG_TEMPLATE: &str = r#"# Debug Phase - Problem Resolution

{{TASK_CONTEXT}}

{{ERROR_CONTEXT}}

{{QUALITY_STATUS}}

{{ATTEMPT_HISTORY}}

{{ANTI_PATTERNS}}

## Current Focus

You are in debug mode because the build phase encountered persistent issues.
Your goal is to identify and resolve the root cause of the current blocker.

## Debug Strategy

### Step 1: Understand the Error
- Read the error messages carefully
- Identify the root cause vs. symptoms
- Check if this is a recurring error

### Step 2: Gather Context
- Use narsil-mcp to understand affected code:
  - `get_call_graph` - see how the code is used
  - `find_references` - find all usages
  - `get_data_flow` - trace data flow
- Read the relevant source files

### Step 3: Isolate the Issue
- Create a minimal test case that reproduces the issue
- Verify the test fails for the expected reason
- Ensure no other tests are affected

### Step 4: Fix the Issue
- Make the minimal change necessary
- Keep existing tests passing
- Add a test for the fixed behavior

### Step 5: Verify the Fix
- Run all tests
- Run clippy
- Ensure no new warnings or errors

{{SESSION_STATS}}

## Debug Rules

1. **Fix the root cause**, not the symptom
2. **Add a test** for every bug fixed
3. **Don't change existing tests** unless they were wrong
4. **Commit only** when all checks pass
5. **If stuck**, step back and re-analyze
"#;

const DEFAULT_PLAN_TEMPLATE: &str = r#"# Plan Phase - Strategic Analysis

{{TASK_CONTEXT}}

{{HISTORICAL_GUIDANCE}}

## Planning Objective

Create a detailed implementation plan for the current task.
Focus on understanding the problem before proposing solutions.

## Planning Steps

### Step 1: Understand Requirements
- Read the task description carefully
- Identify acceptance criteria
- Note any constraints or dependencies

### Step 2: Analyze the Codebase
- Use narsil-mcp to understand existing patterns:
  - `find_similar_code` - find related implementations
  - `get_project_structure` - understand organization
  - `get_import_graph` - see module dependencies
- Identify files that will need changes

### Step 3: Design the Solution
- Propose an approach that fits existing patterns
- List specific changes needed
- Identify potential risks

### Step 4: Create the Plan
- Break down into small, testable steps
- Estimate complexity for each step
- Order by dependencies

{{SESSION_STATS}}

## Planning Output

Document your plan in IMPLEMENTATION_PLAN.md with:
- Clear task breakdown
- Specific file changes
- Test requirements
- Acceptance criteria
"#;

#[cfg(test)]
mod tests {
    use super::*;

    // TemplateMarker tests

    #[test]
    fn test_template_marker_tag() {
        assert_eq!(TemplateMarker::TaskContext.tag(), "{{TASK_CONTEXT}}");
        assert_eq!(TemplateMarker::ErrorContext.tag(), "{{ERROR_CONTEXT}}");
        assert_eq!(TemplateMarker::QualityStatus.tag(), "{{QUALITY_STATUS}}");
        assert_eq!(TemplateMarker::SessionStats.tag(), "{{SESSION_STATS}}");
    }

    #[test]
    fn test_template_marker_from_tag() {
        assert_eq!(
            TemplateMarker::from_tag("{{TASK_CONTEXT}}"),
            Some(TemplateMarker::TaskContext)
        );
        assert_eq!(
            TemplateMarker::from_tag("{{ERROR_CONTEXT}}"),
            Some(TemplateMarker::ErrorContext)
        );
        assert_eq!(TemplateMarker::from_tag("invalid"), None);
        assert_eq!(TemplateMarker::from_tag("{{UNKNOWN}}"), None);
    }

    #[test]
    fn test_template_marker_all() {
        let all = TemplateMarker::all();
        assert!(all.contains(&TemplateMarker::TaskContext));
        assert!(all.contains(&TemplateMarker::ErrorContext));
        assert!(all.contains(&TemplateMarker::QualityStatus));
        assert!(all.contains(&TemplateMarker::SessionStats));
        assert!(all.contains(&TemplateMarker::CodeIntelligence));
        assert!(all.contains(&TemplateMarker::LanguageRules));
        assert!(all.contains(&TemplateMarker::CodeAntipatternWarnings));
        assert!(all.contains(&TemplateMarker::CustomSection));
        assert_eq!(all.len(), 11);
    }

    #[test]
    fn test_template_marker_display() {
        assert_eq!(
            format!("{}", TemplateMarker::TaskContext),
            "{{TASK_CONTEXT}}"
        );
    }

    // Template tests

    #[test]
    fn test_template_new() {
        let template = Template::new("Hello {{TASK_CONTEXT}} world");
        assert_eq!(template.markers().len(), 1);
        assert!(template.has_marker(TemplateMarker::TaskContext));
    }

    #[test]
    fn test_template_multiple_markers() {
        let template = Template::new("{{TASK_CONTEXT}} and {{ERROR_CONTEXT}}");
        assert_eq!(template.markers().len(), 2);
        assert!(template.has_marker(TemplateMarker::TaskContext));
        assert!(template.has_marker(TemplateMarker::ErrorContext));
    }

    #[test]
    fn test_template_no_markers() {
        let template = Template::new("No markers here");
        assert!(template.markers().is_empty());
    }

    #[test]
    fn test_template_substitute() {
        let template = Template::new("Hello {{TASK_CONTEXT}}!");
        let result = template.substitute(TemplateMarker::TaskContext, "World");
        assert_eq!(result.content(), "Hello World!");
        assert!(!result.has_marker(TemplateMarker::TaskContext));
    }

    #[test]
    fn test_template_substitute_all() {
        let template = Template::new("{{TASK_CONTEXT}} and {{ERROR_CONTEXT}}");
        let mut subs = HashMap::new();
        subs.insert(TemplateMarker::TaskContext, "Task");
        subs.insert(TemplateMarker::ErrorContext, "Errors");

        let result = template.substitute_all(&subs);
        assert_eq!(result.content(), "Task and Errors");
    }

    #[test]
    fn test_template_remove_unreplaced_markers() {
        let template = Template::new("Hello {{TASK_CONTEXT}}!");
        let cleaned = template.remove_unreplaced_markers();
        assert_eq!(cleaned.content(), "Hello !");
    }

    #[test]
    fn test_template_insert_before() {
        let template = Template::new("# Title\n\n{{TASK_CONTEXT}}\n\nContent");
        let result = template.insert_before(TemplateMarker::TaskContext, "## Section\n\n");
        assert!(result.content().contains("## Section\n\n{{TASK_CONTEXT}}"));
    }

    #[test]
    fn test_template_insert_after() {
        let template = Template::new("{{TASK_CONTEXT}}\n\nContent");
        let result = template.insert_after(TemplateMarker::TaskContext, "\n## After\n");
        assert!(result.content().contains("{{TASK_CONTEXT}}\n## After\n"));
    }

    // PromptTemplates tests

    #[test]
    fn test_prompt_templates_new() {
        let templates = PromptTemplates::new();
        assert!(templates.modes().is_empty());
    }

    #[test]
    fn test_prompt_templates_with_defaults() {
        let templates = PromptTemplates::with_defaults();
        assert!(templates.has_template("build"));
        assert!(templates.has_template("debug"));
        assert!(templates.has_template("plan"));
    }

    #[test]
    fn test_prompt_templates_add_and_get() {
        let mut templates = PromptTemplates::new();
        templates.add_template("custom", Template::new("Custom content"));

        assert!(templates.has_template("custom"));
        assert!(templates.get_template("custom").is_some());
        assert!(!templates.has_template("nonexistent"));
    }

    #[test]
    fn test_prompt_templates_modes() {
        let templates = PromptTemplates::with_defaults();
        let modes = templates.modes();
        assert!(modes.contains(&"build"));
        assert!(modes.contains(&"debug"));
        assert!(modes.contains(&"plan"));
    }

    #[test]
    fn test_prompt_templates_markers() {
        let templates = PromptTemplates::with_defaults();
        let markers = templates.markers();
        assert!(markers.contains(&TemplateMarker::TaskContext));
        assert!(markers.contains(&TemplateMarker::ErrorContext));
    }

    #[test]
    fn test_prompt_templates_validate() {
        let templates = PromptTemplates::with_defaults();

        // Should pass - defaults have task context
        let result = templates.validate(&[TemplateMarker::TaskContext]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_prompt_templates_validate_missing() {
        let mut templates = PromptTemplates::new();
        templates.add_template("minimal", Template::new("No markers"));

        // Should fail - missing required marker
        let result = templates.validate(&[TemplateMarker::TaskContext]);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_templates_validate_optional() {
        let mut templates = PromptTemplates::new();
        templates.add_template("partial", Template::new("Has {{TASK_CONTEXT}}"));

        let warnings = templates
            .validate_optional(&[TemplateMarker::TaskContext, TemplateMarker::ErrorContext]);

        // Should have warning for missing ErrorContext
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("ERROR_CONTEXT"));
    }

    #[test]
    fn test_prompt_templates_load_from_nonexistent_dir() {
        let result = PromptTemplates::load_from_dir("/nonexistent/path");
        assert!(result.is_ok());
        assert!(result.unwrap().modes().is_empty());
    }

    #[test]
    fn test_prompt_templates_load_or_defaults_nonexistent() {
        let result = PromptTemplates::load_or_defaults("/nonexistent/path");
        assert!(result.is_ok());

        let templates = result.unwrap();
        assert!(templates.has_template("build"));
        assert!(templates.has_template("debug"));
        assert!(templates.has_template("plan"));
    }

    // Default template content tests

    #[test]
    fn test_default_build_template_has_markers() {
        let template = Template::new(DEFAULT_BUILD_TEMPLATE);
        assert!(template.has_marker(TemplateMarker::TaskContext));
        assert!(template.has_marker(TemplateMarker::ErrorContext));
        assert!(template.has_marker(TemplateMarker::QualityStatus));
        assert!(template.has_marker(TemplateMarker::SessionStats));
        assert!(template.has_marker(TemplateMarker::AntiPatterns));
        assert!(template.has_marker(TemplateMarker::CodeIntelligence));
    }

    #[test]
    fn test_default_debug_template_has_markers() {
        let template = Template::new(DEFAULT_DEBUG_TEMPLATE);
        assert!(template.has_marker(TemplateMarker::TaskContext));
        assert!(template.has_marker(TemplateMarker::ErrorContext));
        assert!(template.has_marker(TemplateMarker::QualityStatus));
        assert!(template.has_marker(TemplateMarker::AttemptHistory));
        assert!(template.has_marker(TemplateMarker::AntiPatterns));
    }

    #[test]
    fn test_default_plan_template_has_markers() {
        let template = Template::new(DEFAULT_PLAN_TEMPLATE);
        assert!(template.has_marker(TemplateMarker::TaskContext));
        assert!(template.has_marker(TemplateMarker::HistoricalGuidance));
        assert!(template.has_marker(TemplateMarker::SessionStats));
    }

    // Integration tests

    #[test]
    fn test_template_full_substitution_workflow() {
        let template = Template::new("# Build\n\n{{TASK_CONTEXT}}\n\n{{ERROR_CONTEXT}}\n\nContent");

        let mut subs = HashMap::new();
        subs.insert(TemplateMarker::TaskContext, "## Current Task\n\nDoing X");
        subs.insert(TemplateMarker::ErrorContext, "## Errors\n\nNone");

        let result = template.substitute_all(&subs);

        assert!(result.content().contains("## Current Task"));
        assert!(result.content().contains("Doing X"));
        assert!(result.content().contains("## Errors"));
        assert!(result.content().contains("None"));
        assert!(!result.has_marker(TemplateMarker::TaskContext));
        assert!(!result.has_marker(TemplateMarker::ErrorContext));
    }

    #[test]
    fn test_template_partial_substitution_with_cleanup() {
        let template = Template::new("{{TASK_CONTEXT}}\n\n{{ERROR_CONTEXT}}\n\nContent");

        // Only substitute one marker
        let partial = template.substitute(TemplateMarker::TaskContext, "Task Info");

        // Clean up unreplaced markers
        let cleaned = partial.remove_unreplaced_markers();

        assert!(cleaned.content().contains("Task Info"));
        assert!(!cleaned.content().contains("{{ERROR_CONTEXT}}"));
    }

    // Serialization tests

    #[test]
    fn test_template_marker_serialize() {
        let marker = TemplateMarker::TaskContext;
        let json = serde_json::to_string(&marker).unwrap();
        assert_eq!(json, "\"TaskContext\"");
    }

    #[test]
    fn test_template_marker_deserialize() {
        let json = "\"ErrorContext\"";
        let marker: TemplateMarker = serde_json::from_str(json).unwrap();
        assert_eq!(marker, TemplateMarker::ErrorContext);
    }

    #[test]
    fn test_template_serialize_roundtrip() {
        let template = Template::new("{{TASK_CONTEXT}} content");
        let json = serde_json::to_string(&template).unwrap();
        let restored: Template = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.content(), template.content());
        assert_eq!(restored.markers(), template.markers());
    }
}
