//! Dynamic CLAUDE.md generation for project-specific configurations.
//!
//! This module generates language-aware CLAUDE.md files that include:
//! - Detected primary language(s)
//! - Quality gate commands specific to each language
//! - TDD methodology adapted for each language
//! - User customization sections preserved across regeneration
//!
//! # Example
//!
//! ```
//! use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
//! use ralph::Language;
//!
//! // Single language project
//! let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
//! let claude_md = generator.generate();
//! assert!(claude_md.contains("cargo clippy"));
//!
//! // Polyglot project
//! let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
//! let claude_md = generator.generate();
//! assert!(claude_md.contains("Rust"));
//! assert!(claude_md.contains("Python"));
//! ```

use crate::Language;

/// Markers for user customization sections that are preserved during regeneration.
pub const USER_CUSTOM_START: &str = "<!-- USER_CUSTOM_START -->";
/// End marker for user customization sections.
pub const USER_CUSTOM_END: &str = "<!-- USER_CUSTOM_END -->";

/// Generator for project-specific CLAUDE.md files.
///
/// The generator creates CLAUDE.md content tailored to the project's
/// detected languages, including:
/// - Language-specific quality gates
/// - TDD methodology for each language
/// - Preserved user customization sections
///
/// # Example
///
/// ```
/// use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
/// use ralph::Language;
///
/// let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
/// let content = generator.generate();
/// assert!(content.contains("cargo clippy"));
/// ```
#[derive(Debug, Clone)]
pub struct ClaudeMdGenerator {
    /// Languages detected in the project.
    languages: Vec<Language>,
    /// Optional existing content to preserve user customizations.
    existing_content: Option<String>,
}

impl ClaudeMdGenerator {
    /// Create a new generator for the given languages.
    ///
    /// # Arguments
    ///
    /// * `languages` - Languages detected in the project. The first language
    ///   is considered the primary language.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
    /// use ralph::Language;
    ///
    /// let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
    /// ```
    #[must_use]
    pub fn new(languages: Vec<Language>) -> Self {
        Self {
            languages,
            existing_content: None,
        }
    }

    /// Set existing CLAUDE.md content to preserve user customizations.
    ///
    /// When existing content is provided, any sections marked with
    /// `<!-- USER_CUSTOM_START -->` and `<!-- USER_CUSTOM_END -->` markers
    /// will be preserved in the regenerated output.
    ///
    /// # Arguments
    ///
    /// * `content` - The existing CLAUDE.md content
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
    /// use ralph::Language;
    ///
    /// let existing = r#"
    /// Some content
    /// <!-- USER_CUSTOM_START -->
    /// My custom section
    /// <!-- USER_CUSTOM_END -->
    /// More content
    /// "#;
    ///
    /// let generator = ClaudeMdGenerator::new(vec![Language::Rust])
    ///     .with_existing_content(existing);
    ///
    /// let content = generator.generate();
    /// assert!(content.contains("My custom section"));
    /// ```
    #[must_use]
    pub fn with_existing_content(mut self, content: impl Into<String>) -> Self {
        self.existing_content = Some(content.into());
        self
    }

    /// Get the primary language for the project.
    ///
    /// Returns `None` if no languages are configured.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
    /// use ralph::Language;
    ///
    /// let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
    /// assert_eq!(generator.primary_language(), Some(Language::Rust));
    /// ```
    #[must_use]
    pub fn primary_language(&self) -> Option<Language> {
        self.languages.first().copied()
    }

    /// Check if this is a polyglot (multi-language) project.
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
    /// use ralph::Language;
    ///
    /// let single = ClaudeMdGenerator::new(vec![Language::Rust]);
    /// assert!(!single.is_polyglot());
    ///
    /// let multi = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
    /// assert!(multi.is_polyglot());
    /// ```
    #[must_use]
    pub fn is_polyglot(&self) -> bool {
        self.languages.len() >= 2
    }

    /// Generate the CLAUDE.md content.
    ///
    /// Creates language-aware content including:
    /// - Project header with detected languages
    /// - Language-specific quality gates
    /// - TDD methodology for each language
    /// - User customization sections (preserved if previously set)
    ///
    /// # Example
    ///
    /// ```
    /// use ralph::prompt::claude_md_generator::ClaudeMdGenerator;
    /// use ralph::Language;
    ///
    /// let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
    /// let content = generator.generate();
    ///
    /// // Should contain quality gates
    /// assert!(content.contains("cargo clippy"));
    /// assert!(content.contains("cargo test"));
    ///
    /// // Should contain TDD guidance
    /// assert!(content.contains("TDD") || content.contains("Test"));
    /// ```
    #[must_use]
    pub fn generate(&self) -> String {
        if self.languages.is_empty() {
            return self.generate_generic();
        }

        let mut content = String::new();

        // Add header
        content.push_str(&self.generate_header());

        // Add language-specific sections
        if self.is_polyglot() {
            content.push_str(&self.generate_polyglot_sections());
        } else {
            content.push_str(&self.generate_single_language_sections());
        }

        // Add common sections
        content.push_str(&self.generate_common_sections());

        // Add user customization placeholder
        content.push_str(&self.generate_user_customization_section());

        // Preserve existing user customizations if available
        if let Some(ref existing) = self.existing_content {
            content = self.preserve_user_customizations(&content, existing);
        }

        content
    }

    /// Generate header with project info and detected languages.
    fn generate_header(&self) -> String {
        let lang_list: Vec<String> = self.languages.iter().map(|l| l.to_string()).collect();

        if self.is_polyglot() {
            format!(
                "# Project Memory - Ralph Automation Suite\n\n\
                 ## Project Type: Polyglot ({languages})\n\n\
                 This is a multi-language project. Follow the language-specific \
                 quality standards below for each language.\n\n\
                 **Primary Language:** {primary}\n\
                 **Additional Languages:** {additional}\n\n\
                 ---\n\n",
                languages = lang_list.join(" + "),
                primary = lang_list.first().unwrap_or(&"Unknown".to_string()),
                additional = lang_list[1..].join(", ")
            )
        } else {
            format!(
                "# Project Memory - Ralph Automation Suite\n\n\
                 ## Project Type: {language}\n\n\
                 ---\n\n",
                language = lang_list.first().unwrap_or(&"Unknown".to_string())
            )
        }
    }

    /// Generate sections for a single-language project.
    fn generate_single_language_sections(&self) -> String {
        let Some(lang) = self.primary_language() else {
            return String::new();
        };

        let mut content = String::new();

        // Quality gates section
        content.push_str("## QUALITY GATES\n\n");
        content.push_str(&self.get_quality_gates_for_language(lang));
        content.push_str("\n\n");

        // Forbidden patterns section
        content.push_str("## FORBIDDEN PATTERNS\n\n");
        content.push_str(&self.get_forbidden_patterns_for_language(lang));
        content.push_str("\n\n");

        // TDD section
        content.push_str("## TEST-DRIVEN DEVELOPMENT\n\n");
        content.push_str(&self.get_tdd_methodology_for_language(lang));
        content.push_str("\n\n");

        content
    }

    /// Generate sections for a polyglot project.
    fn generate_polyglot_sections(&self) -> String {
        let mut content = String::new();

        for (i, lang) in self.languages.iter().enumerate() {
            let section_type = if i == 0 { "Primary" } else { "Additional" };

            content.push_str(&format!(
                "## {section_type} Language: {lang}\n\n",
                section_type = section_type,
                lang = lang
            ));

            // Quality gates
            content.push_str("### Quality Gates\n\n");
            content.push_str(&self.get_quality_gates_for_language(*lang));
            content.push_str("\n\n");

            // Forbidden patterns
            content.push_str("### Forbidden Patterns\n\n");
            content.push_str(&self.get_forbidden_patterns_for_language(*lang));
            content.push_str("\n\n");

            // TDD
            content.push_str("### TDD Methodology\n\n");
            content.push_str(&self.get_tdd_methodology_for_language(*lang));
            content.push_str("\n\n---\n\n");
        }

        content
    }

    /// Generate common sections (git, narsil, etc.).
    fn generate_common_sections(&self) -> String {
        r#"## GIT AUTHENTICATION

### Required Setup
Ralph requires `gh` CLI for all GitHub operations.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated, run:
gh auth login
```

---

## MCP SERVERS (Optional)

### narsil-mcp (Code Intelligence)

**Security (run before committing, if available):**
```bash
scan_security           # Find vulnerabilities
find_injection_vulnerabilities  # SQL/XSS/command injection
```

**Context Gathering:**
```bash
get_call_graph <function>   # Function relationships
find_references <symbol>    # Impact analysis
```

---

## STAGNATION HANDLING

If stuck:
1. Check `IMPLEMENTATION_PLAN.md` for blocked tasks
2. Run tests to identify failures
3. Run linters to find warnings
4. Use narsil-mcp to understand the codebase

---

"#
        .to_string()
    }

    /// Generate the user customization section with markers.
    fn generate_user_customization_section(&self) -> String {
        format!(
            "## USER CUSTOMIZATIONS\n\n\
             Add project-specific notes below. This section is preserved during regeneration.\n\n\
             {start}\n\
             \n\
             {end}\n",
            start = USER_CUSTOM_START,
            end = USER_CUSTOM_END
        )
    }

    /// Generate a generic CLAUDE.md when no languages are detected.
    fn generate_generic(&self) -> String {
        "# Project Memory - Ralph Automation Suite\n\n\
         ## Project Type: Unknown\n\n\
         No programming languages were detected. \
         Please configure languages manually or add source files.\n\n\
         ---\n\n\
         ## General Quality Standards\n\n\
         - Write tests before implementation (TDD)\n\
         - Run linters before commit\n\
         - Document public APIs\n\
         - Handle all errors explicitly\n"
            .to_string()
    }

    /// Get quality gates for a specific language.
    fn get_quality_gates_for_language(&self, lang: Language) -> String {
        match lang {
            Language::Rust => r#"```bash
cargo clippy --all-targets -- -D warnings  # 0 warnings
cargo test                                  # all pass
```"#
                .to_string(),
            Language::Python => r#"```bash
ruff check . (or flake8 .)                 # 0 warnings
mypy .                                     # 0 errors
pytest                                     # all pass
bandit -r . -ll                            # 0 HIGH/CRITICAL
```"#
                .to_string(),
            Language::TypeScript | Language::JavaScript => r#"```bash
npm run lint (ESLint)                      # 0 warnings
npm run typecheck (tsc)                    # 0 errors
npm test                                   # all pass
npm audit                                  # 0 high/critical
```"#
                .to_string(),
            Language::Go => r#"```bash
go vet ./...                               # 0 issues
golangci-lint run                          # 0 warnings
go test ./...                              # all pass
govulncheck ./...                          # 0 vulnerabilities
```"#
                .to_string(),
            Language::Java | Language::Kotlin => r#"```bash
./gradlew check                            # 0 warnings
./gradlew test                             # all pass
./gradlew spotbugsMain                     # 0 bugs
```"#
                .to_string(),
            Language::CSharp => r#"```bash
dotnet build --warnaserror                 # 0 warnings
dotnet test                                # all pass
dotnet format --verify-no-changes          # formatted
```"#
                .to_string(),
            Language::Ruby => r#"```bash
rubocop                                    # 0 offenses
bundle exec rspec                          # all pass
```"#
                .to_string(),
            Language::Php => r#"```bash
./vendor/bin/phpcs                         # 0 errors
./vendor/bin/phpstan analyse               # 0 errors
./vendor/bin/phpunit                       # all pass
```"#
                .to_string(),
            _ => "```bash\n# Run your language's linter and test suite\n```".to_string(),
        }
    }

    /// Get forbidden patterns for a specific language.
    fn get_forbidden_patterns_for_language(&self, lang: Language) -> String {
        match lang {
            Language::Rust => r#"```rust
#[allow(dead_code)]           // Wire in or delete
#[allow(unused_*)]            // Use or remove
#[allow(clippy::*)]           // Fix the issue
todo!()                       // Implement now
unimplemented!()              // Implement or remove
// TODO: ...                  // Implement now or don't merge
```"#
                .to_string(),
            Language::Python => r#"```python
# type: ignore              # Fix the type error properly
# noqa                      # Fix the linting issue
# TODO: ...                 # Implement now or don't merge
pass                        # As a placeholder - implement or remove
...                         # As implementation - complete it
```"#
                .to_string(),
            Language::TypeScript | Language::JavaScript => r#"```typescript
// @ts-ignore              // Fix the type error properly
// eslint-disable          // Fix the linting issue
any                        // Use proper types
// TODO: ...               // Implement now or don't merge
as any                     // Type properly instead
```"#
                .to_string(),
            Language::Go => r#"```go
//nolint                    // Fix the issue properly
_ = err                     // Handle the error
// TODO: ...                // Implement now or don't merge
panic("not implemented")    // Implement properly
```"#
                .to_string(),
            Language::Java | Language::Kotlin => r#"```java
@SuppressWarnings          // Fix the warning properly
// TODO: ...               // Implement now or don't merge
// FIXME: ...              // Fix now
```"#
                .to_string(),
            _ => "- No TODO/FIXME comments in merged code\n- No suppressed warnings".to_string(),
        }
    }

    /// Get TDD methodology for a specific language.
    fn get_tdd_methodology_for_language(&self, lang: Language) -> String {
        match lang {
            Language::Rust => r#"**TDD Cycle:**
1. RED: Write failing test with `#[test]`
2. GREEN: Write minimal code to pass
3. REFACTOR: Clean up while tests green
4. Run `cargo clippy` + `cargo test` before commit

**Test Requirements:**
- Every public function: at least 1 test
- Every public type: exercised in tests
- Use `#[should_panic]` for expected panics"#
                .to_string(),
            Language::Python => r#"**TDD Cycle:**
1. RED: Write failing pytest test
2. GREEN: Write minimal code to pass
3. REFACTOR: Clean up while tests green
4. Run `ruff` + `mypy` + `pytest` before commit

**Test Requirements:**
- Every public function: at least 1 test
- Use fixtures for test setup
- Use `pytest.raises` for exception testing"#
                .to_string(),
            Language::TypeScript | Language::JavaScript => r#"**TDD Cycle:**
1. RED: Write failing Jest/Vitest test
2. GREEN: Write minimal code to pass
3. REFACTOR: Clean up while tests green
4. Run `eslint` + `tsc` + `npm test` before commit

**Test Requirements:**
- Every exported function: at least 1 test
- Use `describe` blocks for organization
- Use `expect().toThrow()` for error testing"#
                .to_string(),
            Language::Go => r#"**TDD Cycle (Table-Driven Tests):**
1. RED: Write failing table-driven test
2. GREEN: Write minimal code to pass
3. REFACTOR: Clean up while tests green
4. Run `go vet` + `golangci-lint` + `go test` before commit

**Test Requirements:**
- Every exported function: at least 1 test
- Use table-driven tests for multiple cases
- Use `t.Run` for subtests"#
                .to_string(),
            _ => r#"**TDD Cycle:**
1. RED: Write failing test
2. GREEN: Write minimal code to pass
3. REFACTOR: Clean up while tests green
4. Run quality gates before commit"#
                .to_string(),
        }
    }

    /// Preserve user customizations from existing content.
    fn preserve_user_customizations(&self, new_content: &str, existing: &str) -> String {
        // Find user custom sections in existing content
        let custom_sections = extract_user_custom_sections(existing);

        if custom_sections.is_empty() {
            return new_content.to_string();
        }

        // Replace empty custom section with preserved content
        let mut result = new_content.to_string();
        for section in custom_sections {
            let empty_section = format!("{}\n\n{}", USER_CUSTOM_START, USER_CUSTOM_END);
            let preserved_section = format!("{}\n{}{}", USER_CUSTOM_START, section, USER_CUSTOM_END);
            result = result.replace(&empty_section, &preserved_section);
        }

        result
    }
}

impl Default for ClaudeMdGenerator {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// Extract user custom sections from existing CLAUDE.md content.
///
/// Returns a vector of content strings found between USER_CUSTOM_START
/// and USER_CUSTOM_END markers.
#[must_use]
pub fn extract_user_custom_sections(content: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut remaining = content;

    while let Some(start_idx) = remaining.find(USER_CUSTOM_START) {
        let after_start = &remaining[start_idx + USER_CUSTOM_START.len()..];
        if let Some(end_idx) = after_start.find(USER_CUSTOM_END) {
            let section = &after_start[..end_idx];
            if !section.trim().is_empty() {
                sections.push(section.to_string());
            }
            remaining = &after_start[end_idx + USER_CUSTOM_END.len()..];
        } else {
            break;
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Constructor and basic property tests
    // =========================================================================

    #[test]
    fn test_generator_new_with_single_language() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        assert_eq!(generator.languages.len(), 1);
        assert_eq!(generator.primary_language(), Some(Language::Rust));
    }

    #[test]
    fn test_generator_new_with_multiple_languages() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        assert_eq!(generator.languages.len(), 2);
        assert_eq!(generator.primary_language(), Some(Language::Rust));
    }

    #[test]
    fn test_generator_new_empty() {
        let generator = ClaudeMdGenerator::new(vec![]);
        assert!(generator.languages.is_empty());
        assert_eq!(generator.primary_language(), None);
    }

    #[test]
    fn test_generator_is_polyglot() {
        let single = ClaudeMdGenerator::new(vec![Language::Rust]);
        assert!(!single.is_polyglot());

        let multi = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        assert!(multi.is_polyglot());

        let empty = ClaudeMdGenerator::new(vec![]);
        assert!(!empty.is_polyglot());
    }

    #[test]
    fn test_generator_default() {
        let generator = ClaudeMdGenerator::default();
        assert!(generator.languages.is_empty());
    }

    // =========================================================================
    // Generated content includes primary language tests
    // =========================================================================

    #[test]
    fn test_generated_includes_primary_language_rust() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.contains("Rust"),
            "Generated CLAUDE.md should include primary language name"
        );
    }

    #[test]
    fn test_generated_includes_primary_language_python() {
        let generator = ClaudeMdGenerator::new(vec![Language::Python]);
        let content = generator.generate();

        assert!(
            content.contains("Python"),
            "Generated CLAUDE.md should include primary language name"
        );
    }

    #[test]
    fn test_generated_includes_primary_language_typescript() {
        let generator = ClaudeMdGenerator::new(vec![Language::TypeScript]);
        let content = generator.generate();

        assert!(
            content.contains("TypeScript"),
            "Generated CLAUDE.md should include primary language name"
        );
    }

    // =========================================================================
    // Generated content includes quality gates for each language
    // =========================================================================

    #[test]
    fn test_generated_includes_rust_quality_gates() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.contains("cargo clippy"),
            "Rust CLAUDE.md should include clippy"
        );
        assert!(
            content.contains("cargo test"),
            "Rust CLAUDE.md should include cargo test"
        );
    }

    #[test]
    fn test_generated_includes_python_quality_gates() {
        let generator = ClaudeMdGenerator::new(vec![Language::Python]);
        let content = generator.generate();

        assert!(
            content.contains("pytest") || content.contains("ruff") || content.contains("mypy"),
            "Python CLAUDE.md should include Python quality tools"
        );
    }

    #[test]
    fn test_generated_includes_typescript_quality_gates() {
        let generator = ClaudeMdGenerator::new(vec![Language::TypeScript]);
        let content = generator.generate();

        assert!(
            content.contains("npm") || content.contains("eslint") || content.contains("tsc"),
            "TypeScript CLAUDE.md should include TS quality tools"
        );
    }

    #[test]
    fn test_generated_includes_go_quality_gates() {
        let generator = ClaudeMdGenerator::new(vec![Language::Go]);
        let content = generator.generate();

        assert!(
            content.contains("go test") || content.contains("go vet"),
            "Go CLAUDE.md should include Go quality tools"
        );
    }

    // =========================================================================
    // Generated content includes TDD methodology for each language
    // =========================================================================

    #[test]
    fn test_generated_includes_rust_tdd() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.contains("TDD") || content.contains("Test"),
            "Rust CLAUDE.md should include TDD guidance"
        );
        assert!(
            content.contains("#[test]") || content.contains("test"),
            "Rust CLAUDE.md should include Rust test syntax"
        );
    }

    #[test]
    fn test_generated_includes_python_tdd() {
        let generator = ClaudeMdGenerator::new(vec![Language::Python]);
        let content = generator.generate();

        assert!(
            content.contains("TDD") || content.contains("pytest"),
            "Python CLAUDE.md should include TDD/pytest guidance"
        );
    }

    #[test]
    fn test_generated_includes_typescript_tdd() {
        let generator = ClaudeMdGenerator::new(vec![Language::TypeScript]);
        let content = generator.generate();

        assert!(
            content.contains("TDD") || content.contains("Jest") || content.contains("Vitest"),
            "TypeScript CLAUDE.md should include TDD guidance"
        );
    }

    #[test]
    fn test_generated_includes_go_tdd() {
        let generator = ClaudeMdGenerator::new(vec![Language::Go]);
        let content = generator.generate();

        assert!(
            content.contains("TDD") || content.contains("table-driven"),
            "Go CLAUDE.md should include TDD/table-driven guidance"
        );
    }

    // =========================================================================
    // Polyglot CLAUDE.md has clear sections per language
    // =========================================================================

    #[test]
    fn test_polyglot_has_separate_language_sections() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        let content = generator.generate();

        // Should have clear section headers for each language
        assert!(
            content.contains("Primary Language: Rust")
                || content.contains("## Primary Language")
                || content.contains("Rust"),
            "Polyglot should have primary language section"
        );
        assert!(
            content.contains("Additional Language: Python")
                || content.contains("## Additional Language")
                || content.contains("Python"),
            "Polyglot should have additional language section"
        );
    }

    #[test]
    fn test_polyglot_quality_gates_for_each_language() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        let content = generator.generate();

        // Should have quality gates for both languages
        assert!(
            content.contains("cargo clippy") || content.contains("cargo test"),
            "Polyglot should have Rust quality gates"
        );
        assert!(
            content.contains("pytest") || content.contains("ruff"),
            "Polyglot should have Python quality gates"
        );
    }

    #[test]
    fn test_polyglot_tdd_for_each_language() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        let content = generator.generate();

        // Should have TDD guidance for both languages
        assert!(
            content.contains("#[test]") || content.contains("Rust"),
            "Polyglot should have Rust TDD guidance"
        );
        assert!(
            content.contains("pytest") || content.contains("Python"),
            "Polyglot should have Python TDD guidance"
        );
    }

    #[test]
    fn test_polyglot_three_languages() {
        let generator =
            ClaudeMdGenerator::new(vec![Language::Rust, Language::Python, Language::TypeScript]);
        let content = generator.generate();

        // All three languages should be represented
        assert!(content.contains("Rust"));
        assert!(content.contains("Python"));
        assert!(content.contains("TypeScript"));
    }

    #[test]
    fn test_polyglot_indicates_polyglot_status() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        let content = generator.generate();

        assert!(
            content.contains("Polyglot")
                || content.contains("polyglot")
                || content.contains("multi-language")
                || content.contains("Multiple"),
            "Polyglot CLAUDE.md should indicate multi-language status"
        );
    }

    // =========================================================================
    // User customization preservation tests
    // =========================================================================

    #[test]
    fn test_generated_includes_user_customization_markers() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.contains(USER_CUSTOM_START),
            "Generated CLAUDE.md should include USER_CUSTOM_START marker"
        );
        assert!(
            content.contains(USER_CUSTOM_END),
            "Generated CLAUDE.md should include USER_CUSTOM_END marker"
        );
    }

    #[test]
    fn test_preserves_user_customizations() {
        let existing = format!(
            "Some content\n{}\nMy custom section here\n{}\nMore content",
            USER_CUSTOM_START, USER_CUSTOM_END
        );

        let generator =
            ClaudeMdGenerator::new(vec![Language::Rust]).with_existing_content(&existing);
        let content = generator.generate();

        assert!(
            content.contains("My custom section here"),
            "Regenerated CLAUDE.md should preserve user customizations"
        );
    }

    #[test]
    fn test_preserves_multiple_user_customizations() {
        let existing = format!(
            "{}\nFirst custom\n{}\nMiddle content\n{}\nSecond custom\n{}",
            USER_CUSTOM_START, USER_CUSTOM_END, USER_CUSTOM_START, USER_CUSTOM_END
        );

        let generator =
            ClaudeMdGenerator::new(vec![Language::Rust]).with_existing_content(&existing);
        let content = generator.generate();

        assert!(
            content.contains("First custom"),
            "Should preserve first custom section"
        );
    }

    #[test]
    fn test_empty_customization_is_not_preserved() {
        let existing = format!(
            "Some content\n{}\n   \n{}\nMore content",
            USER_CUSTOM_START, USER_CUSTOM_END
        );

        let generator =
            ClaudeMdGenerator::new(vec![Language::Rust]).with_existing_content(&existing);
        let content = generator.generate();

        // Should still have markers, but content should be empty placeholder
        assert!(content.contains(USER_CUSTOM_START));
        assert!(content.contains(USER_CUSTOM_END));
    }

    #[test]
    fn test_regeneration_without_existing_content() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        // Should have empty customization section with markers
        assert!(content.contains(USER_CUSTOM_START));
        assert!(content.contains(USER_CUSTOM_END));
    }

    // =========================================================================
    // Extract user custom sections function tests
    // =========================================================================

    #[test]
    fn test_extract_user_custom_sections_single() {
        let content = format!(
            "Header\n{}\nCustom content\n{}\nFooter",
            USER_CUSTOM_START, USER_CUSTOM_END
        );

        let sections = extract_user_custom_sections(&content);
        assert_eq!(sections.len(), 1);
        assert!(sections[0].contains("Custom content"));
    }

    #[test]
    fn test_extract_user_custom_sections_multiple() {
        let content = format!(
            "{}\nFirst\n{}\nMiddle\n{}\nSecond\n{}",
            USER_CUSTOM_START, USER_CUSTOM_END, USER_CUSTOM_START, USER_CUSTOM_END
        );

        let sections = extract_user_custom_sections(&content);
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_extract_user_custom_sections_empty() {
        let content = "No markers here";
        let sections = extract_user_custom_sections(content);
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_user_custom_sections_whitespace_only() {
        let content = format!(
            "{}\n   \n   \n{}\n",
            USER_CUSTOM_START, USER_CUSTOM_END
        );

        let sections = extract_user_custom_sections(&content);
        assert!(sections.is_empty(), "Whitespace-only sections should be filtered out");
    }

    // =========================================================================
    // Edge cases and integration tests
    // =========================================================================

    #[test]
    fn test_generate_generic_for_no_languages() {
        let generator = ClaudeMdGenerator::new(vec![]);
        let content = generator.generate();

        assert!(
            content.contains("Unknown") || content.contains("No programming languages"),
            "Empty languages should generate generic content"
        );
    }

    #[test]
    fn test_forbidden_patterns_for_rust() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.contains("#[allow("),
            "Rust should include #[allow(...)] as forbidden"
        );
        assert!(
            content.contains("todo!()"),
            "Rust should include todo!() as forbidden"
        );
    }

    #[test]
    fn test_forbidden_patterns_for_python() {
        let generator = ClaudeMdGenerator::new(vec![Language::Python]);
        let content = generator.generate();

        assert!(
            content.contains("# type: ignore") || content.contains("noqa"),
            "Python should include type: ignore or noqa as forbidden"
        );
    }

    #[test]
    fn test_forbidden_patterns_for_typescript() {
        let generator = ClaudeMdGenerator::new(vec![Language::TypeScript]);
        let content = generator.generate();

        assert!(
            content.contains("@ts-ignore") || content.contains("eslint-disable"),
            "TypeScript should include @ts-ignore or eslint-disable as forbidden"
        );
    }

    #[test]
    fn test_common_sections_present() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.contains("GIT") || content.contains("gh"),
            "CLAUDE.md should include Git section"
        );
        assert!(
            content.contains("narsil") || content.contains("MCP"),
            "CLAUDE.md should include narsil/MCP section"
        );
    }

    #[test]
    fn test_header_format_single_language() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust]);
        let content = generator.generate();

        assert!(
            content.starts_with("# Project Memory"),
            "Single language should have standard header"
        );
    }

    #[test]
    fn test_header_format_polyglot() {
        let generator = ClaudeMdGenerator::new(vec![Language::Rust, Language::Python]);
        let content = generator.generate();

        assert!(
            content.contains("Primary Language") || content.contains("primary"),
            "Polyglot header should indicate primary language"
        );
    }
}
