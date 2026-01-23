//! Bootstrap module for setting up the automation suite in a project.
//!
//! This module creates all necessary directories, configuration files,
//! prompts, and scripts needed for the automation suite.
//!
//! # Submodules
//!
//! - [`language`] - Language enum and representation for multi-language support
//! - [`language_detector`] - Automatic language detection for projects
//! - [`templates`] - Template registry for language-specific prompts and configuration

pub mod language;
pub mod language_detector;
pub mod templates;

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use language::Language;
use language_detector::{DetectedLanguage, LanguageDetector};

use crate::prompt::claude_md_generator::ClaudeMdGenerator;
use crate::quality::gates::detect_available_gates;

/// Bootstrap manager for setting up the automation suite in a project.
///
/// # Example
///
/// ```rust,ignore
/// use ralph::bootstrap::Bootstrap;
/// use ralph::bootstrap::language::Language;
///
/// // Auto-detect languages
/// let bootstrap = Bootstrap::new(PathBuf::from("."));
/// bootstrap.run(false, false)?;
///
/// // Override languages
/// let bootstrap = Bootstrap::new(PathBuf::from("."))
///     .with_languages(vec![Language::Rust, Language::Python]);
/// bootstrap.run(false, false)?;
/// ```
pub struct Bootstrap {
    project_dir: PathBuf,
    /// Optional language override. If None, auto-detect. If Some and non-empty, use these.
    language_override: Option<Vec<Language>>,
}

impl Bootstrap {
    /// Create a new bootstrap manager.
    ///
    /// By default, languages are auto-detected. Use [`with_languages`](Self::with_languages)
    /// to override.
    #[must_use]
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            project_dir,
            language_override: None,
        }
    }

    /// Set specific languages to use instead of auto-detection.
    ///
    /// This is useful when:
    /// - The project uses languages that aren't auto-detected
    /// - You want to force a specific language configuration
    /// - Setting up a polyglot project with explicit language list
    ///
    /// # Arguments
    ///
    /// * `languages` - Languages to use. If empty, falls back to auto-detection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::bootstrap::Bootstrap;
    /// use ralph::bootstrap::language::Language;
    ///
    /// // Single language override
    /// let bootstrap = Bootstrap::new(PathBuf::from("."))
    ///     .with_languages(vec![Language::Rust]);
    ///
    /// // Polyglot project with multiple languages
    /// let bootstrap = Bootstrap::new(PathBuf::from("."))
    ///     .with_languages(vec![Language::TypeScript, Language::Python, Language::Go]);
    /// ```
    #[must_use]
    pub fn with_languages(mut self, languages: Vec<Language>) -> Self {
        self.language_override = Some(languages);
        self
    }

    /// Get the effective languages for this project.
    ///
    /// Returns languages in this priority order:
    /// 1. Explicit override (if non-empty)
    /// 2. Auto-detected languages
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::bootstrap::Bootstrap;
    /// use ralph::bootstrap::language::Language;
    ///
    /// // With override
    /// let bootstrap = Bootstrap::new(PathBuf::from("."))
    ///     .with_languages(vec![Language::Rust]);
    /// assert_eq!(bootstrap.effective_languages(), vec![Language::Rust]);
    ///
    /// // Without override, auto-detects
    /// let bootstrap = Bootstrap::new(PathBuf::from("."));
    /// let languages = bootstrap.effective_languages(); // auto-detected
    /// ```
    #[must_use]
    pub fn effective_languages(&self) -> Vec<Language> {
        // If we have a non-empty override, use it
        if let Some(ref languages) = self.language_override {
            if !languages.is_empty() {
                return languages.clone();
            }
        }

        // Fall back to auto-detection
        self.detect_languages()
            .into_iter()
            .map(|d| d.language)
            .collect()
    }

    /// Detect programming languages used in the project.
    ///
    /// Returns a vector of detected languages sorted by confidence (highest first).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let bootstrap = Bootstrap::new(PathBuf::from("."));
    /// let languages = bootstrap.detect_languages();
    /// for lang in &languages {
    ///     println!("{}: {:.0}%", lang.language, lang.confidence * 100.0);
    /// }
    /// ```
    pub fn detect_languages(&self) -> Vec<DetectedLanguage> {
        let detector = LanguageDetector::new(&self.project_dir);
        detector.detect()
    }

    /// Get the primary (most confident) language for this project.
    ///
    /// Returns `None` if no source files are detected.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let bootstrap = Bootstrap::new(PathBuf::from("."));
    /// if let Some(lang) = bootstrap.primary_language() {
    ///     println!("This is a {} project", lang);
    /// }
    /// ```
    pub fn primary_language(&self) -> Option<Language> {
        let detector = LanguageDetector::new(&self.project_dir);
        detector.primary_language()
    }

    /// Generate a language-aware MCP configuration.
    ///
    /// Creates an MCP configuration JSON string that is optimized for the
    /// project's detected or specified languages. The configuration includes:
    /// - Standard narsil-mcp flags (--git, --call-graph, --persist, --watch)
    /// - Language-appropriate settings
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::bootstrap::Bootstrap;
    /// use ralph::bootstrap::language::Language;
    ///
    /// let bootstrap = Bootstrap::new(PathBuf::from("."))
    ///     .with_languages(vec![Language::Rust, Language::Python]);
    ///
    /// let config = bootstrap.generate_mcp_config();
    /// println!("{}", config);
    /// ```
    #[must_use]
    pub fn generate_mcp_config(&self) -> String {
        let languages = self.effective_languages();

        // Build the args array
        let mut args = vec![
            "--repos".to_string(),
            ".".to_string(),
            "--git".to_string(),
            "--call-graph".to_string(),
            "--persist".to_string(),
            "--watch".to_string(),
        ];

        // Add language-specific exclude patterns if we have languages
        if !languages.is_empty() {
            // narsil-mcp will auto-detect languages, but we can add exclude patterns
            // to avoid indexing irrelevant directories
            let excludes = self.get_language_excludes(&languages);
            for exclude in excludes {
                args.push("--exclude".to_string());
                args.push(exclude);
            }
        }

        // Build the JSON structure
        let args_json: Vec<String> = args.iter().map(|a| format!("\"{}\"", a)).collect();

        format!(
            r#"{{
  "mcpServers": {{
    "narsil-mcp": {{
      "command": "narsil-mcp",
      "args": [
        {}
      ]
    }}
  }}
}}"#,
            args_json.join(",\n        ")
        )
    }

    /// Get exclude patterns appropriate for the given languages.
    ///
    /// Returns patterns that should be excluded from indexing to improve
    /// performance and reduce noise.
    fn get_language_excludes(&self, languages: &[Language]) -> Vec<String> {
        let mut excludes = Vec::new();

        for lang in languages {
            match lang {
                Language::Rust => {
                    excludes.push("target".to_string());
                }
                Language::Python => {
                    excludes.push("__pycache__".to_string());
                    excludes.push(".venv".to_string());
                    excludes.push("venv".to_string());
                    excludes.push(".tox".to_string());
                    excludes.push("*.egg-info".to_string());
                }
                Language::TypeScript | Language::JavaScript => {
                    excludes.push("node_modules".to_string());
                    excludes.push("dist".to_string());
                    excludes.push(".next".to_string());
                }
                Language::Go => {
                    excludes.push("vendor".to_string());
                }
                Language::Java => {
                    excludes.push("target".to_string());
                    excludes.push("build".to_string());
                    excludes.push(".gradle".to_string());
                }
                _ => {}
            }
        }

        // Remove duplicates
        excludes.sort();
        excludes.dedup();

        excludes
    }

    /// Display detected languages with confidence scores and selected gates.
    fn display_detected_languages(&self) {
        let detected = self.detect_languages();

        if detected.is_empty() {
            println!("   {} No programming languages detected", "Note:".yellow());
            println!("   This is an empty or unrecognized project type");
            return;
        }

        println!("\n{} Detected languages:", "Languages:".cyan());
        for lang in &detected {
            let marker = if lang.primary { "→" } else { " " };
            let primary_tag = if lang.primary {
                " (primary)".green().to_string()
            } else {
                String::new()
            };
            println!(
                "   {} {}: {:.0}% confidence ({} files){}",
                marker,
                lang.language.to_string().bold(),
                lang.confidence * 100.0,
                lang.file_count,
                primary_tag
            );
        }

        // Get languages with sufficient confidence for gate selection
        let significant_languages: Vec<_> = detected
            .iter()
            .filter(|d| d.confidence >= LanguageDetector::DEFAULT_POLYGLOT_THRESHOLD)
            .map(|d| d.language)
            .collect();

        // Detect and display selected gates
        let available_gates = detect_available_gates(&self.project_dir, &significant_languages);
        if !available_gates.is_empty() {
            println!("\n{} Selected gates:", "Gates:".cyan());
            for gate in &available_gates {
                let blocking_tag = if gate.is_blocking() { "" } else { " (non-blocking)" };
                println!("   ✓ {}{}", gate.name(), blocking_tag);
            }
        }

        println!();
    }

    /// Run the bootstrap process
    pub fn run(&self, force: bool, install_git_hooks: bool) -> Result<()> {
        println!(
            "{} Bootstrapping automation suite in {}",
            "Info:".blue(),
            self.project_dir.display()
        );

        // Detect and display project languages
        self.display_detected_languages();

        // Create directory structure
        self.create_directories()?;

        // Create configuration files
        self.create_claude_config(force)?;
        self.create_mcp_config(force)?;

        // Create hooks
        self.create_hooks(force)?;

        // Create skills
        self.create_skills(force)?;

        // Create agents
        self.create_agents(force)?;

        // Create prompt templates
        self.create_prompts(force)?;

        // Create scripts (these are informational - actual scripts are in Rust)
        self.create_scripts(force)?;

        // Create documentation templates
        self.create_doc_templates(force)?;

        // Create IMPLEMENTATION_PLAN.md if it doesn't exist
        self.create_implementation_plan()?;

        // Update .gitignore
        self.update_gitignore()?;

        // Install git hooks if requested
        if install_git_hooks {
            self.install_git_hooks(force)?;
        }

        Ok(())
    }

    /// Create directory structure
    fn create_directories(&self) -> Result<()> {
        let dirs = [
            ".claude/skills",
            ".claude/agents",
            ".claude/commands",
            ".claude/hooks",
            ".claude/rules",
            ".ralph/analysis",
            ".ralph/benchmarks",
            ".archive/docs",
            ".archive/decisions",
            ".archive/code",
            ".cowork/tasks",
            "docs/decisions",
            "docs/implementation",
            "docs/runbooks",
            "docs/templates",
            "scripts",
        ];

        for dir in dirs {
            let path = self.project_dir.join(dir);
            if !path.exists() {
                fs::create_dir_all(&path)
                    .with_context(|| format!("Failed to create directory: {}", path.display()))?;
                println!("   Created: {}", dir);
            }
        }

        Ok(())
    }

    /// Create CLAUDE.md configuration
    ///
    /// Generates a language-aware CLAUDE.md using the ClaudeMdGenerator.
    /// The generator creates content tailored to the project's detected
    /// languages with appropriate quality gates and TDD methodology.
    fn create_claude_config(&self, force: bool) -> Result<()> {
        let claude_md = self.project_dir.join(".claude/CLAUDE.md");
        let settings_json = self.project_dir.join(".claude/settings.json");

        if !claude_md.exists() || force {
            // Get effective languages for this project
            let languages = self.effective_languages();

            // Read existing content if it exists to preserve user customizations
            let generator = if claude_md.exists() {
                let existing = fs::read_to_string(&claude_md)?;
                ClaudeMdGenerator::new(languages).with_existing_content(existing)
            } else {
                ClaudeMdGenerator::new(languages)
            };

            // Generate and write the CLAUDE.md
            let content = generator.generate();
            fs::write(&claude_md, content)?;
            println!("   Created: .claude/CLAUDE.md");
        }

        if !settings_json.exists() || force {
            fs::write(&settings_json, include_str!("../templates/settings.json"))?;
            println!("   Created: .claude/settings.json");
        }

        Ok(())
    }

    /// Create MCP configuration
    fn create_mcp_config(&self, force: bool) -> Result<()> {
        let mcp_json = self.project_dir.join(".claude/mcp.json");

        if !mcp_json.exists() || force {
            fs::write(&mcp_json, include_str!("../templates/mcp.json"))?;
            println!("   Created: .claude/mcp.json");
        }

        Ok(())
    }

    /// Create hook scripts
    fn create_hooks(&self, force: bool) -> Result<()> {
        let hooks = [
            (
                "security-filter.sh",
                include_str!("../templates/hooks/security-filter.sh"),
            ),
            (
                "post-edit-scan.sh",
                include_str!("../templates/hooks/post-edit-scan.sh"),
            ),
            (
                "end-of-turn.sh",
                include_str!("../templates/hooks/end-of-turn.sh"),
            ),
            (
                "session-init.sh",
                include_str!("../templates/hooks/session-init.sh"),
            ),
        ];

        for (name, content) in hooks {
            let path = self.project_dir.join(".claude/hooks").join(name);
            if !path.exists() || force {
                fs::write(&path, content)?;
                // Make executable
                #[cfg(unix)]
                {
                    let mut perms = fs::metadata(&path)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&path, perms)?;
                }
                println!("   Created: .claude/hooks/{}", name);
            }
        }

        Ok(())
    }

    /// Create skill definitions
    fn create_skills(&self, force: bool) -> Result<()> {
        let skills = [
            (
                "docs-sync.md",
                include_str!("../templates/skills/docs-sync.md"),
            ),
            (
                "project-analyst.md",
                include_str!("../templates/skills/project-analyst.md"),
            ),
        ];

        for (name, content) in skills {
            let path = self.project_dir.join(".claude/skills").join(name);
            if !path.exists() || force {
                fs::write(&path, content)?;
                println!("   Created: .claude/skills/{}", name);
            }
        }

        Ok(())
    }

    /// Create agent definitions
    fn create_agents(&self, force: bool) -> Result<()> {
        let agents = [
            (
                "adversarial-reviewer.md",
                include_str!("../templates/agents/adversarial-reviewer.md"),
            ),
            (
                "security-auditor.md",
                include_str!("../templates/agents/security-auditor.md"),
            ),
            (
                "supervisor.md",
                include_str!("../templates/agents/supervisor.md"),
            ),
        ];

        for (name, content) in agents {
            let path = self.project_dir.join(".claude/agents").join(name);
            if !path.exists() || force {
                fs::write(&path, content)?;
                println!("   Created: .claude/agents/{}", name);
            }
        }

        Ok(())
    }

    /// Create prompt templates
    fn create_prompts(&self, force: bool) -> Result<()> {
        let prompts = [
            (
                "PROMPT_plan.md",
                include_str!("../templates/PROMPT_plan.md"),
            ),
            (
                "PROMPT_build.md",
                include_str!("../templates/PROMPT_build.md"),
            ),
            (
                "PROMPT_debug.md",
                include_str!("../templates/PROMPT_debug.md"),
            ),
        ];

        for (name, content) in prompts {
            let path = self.project_dir.join(name);
            if !path.exists() || force {
                fs::write(&path, content)?;
                println!("   Created: {}", name);
            }
        }

        Ok(())
    }

    /// Create utility scripts
    fn create_scripts(&self, _force: bool) -> Result<()> {
        // Note: With Rust implementation, we don't need shell scripts
        // but we create a helper README
        let readme = self.project_dir.join("scripts/README.md");
        if !readme.exists() {
            fs::write(
                &readme,
                r#"# Automation Scripts

The Ralph automation suite is implemented in Rust. Use the `ralph` command instead of shell scripts:

```bash
# Context building
ralph context -o context.txt
ralph context -m docs -o docs-context.txt

# Archive management
ralph archive --stale-days 90 --dry-run
ralph archive --stale-days 90

# Project analysis
ralph analyze

# Main loop
ralph loop plan --max-iterations 5
ralph loop build --max-iterations 50

# Analytics
ralph analytics --last 10
```

For legacy compatibility, you can also use:
```bash
ralph bootstrap  # Set up project structure
```
"#,
            )?;
            println!("   Created: scripts/README.md");
        }

        Ok(())
    }

    /// Create documentation templates
    fn create_doc_templates(&self, force: bool) -> Result<()> {
        let templates = [
            (
                "adr-template.md",
                include_str!("../templates/docs/adr-template.md"),
            ),
            (
                "implementation-template.md",
                include_str!("../templates/docs/implementation-template.md"),
            ),
        ];

        for (name, content) in templates {
            let path = self.project_dir.join("docs/templates").join(name);
            if !path.exists() || force {
                fs::write(&path, content)?;
                println!("   Created: docs/templates/{}", name);
            }
        }

        // Create architecture.md and api.md stubs
        let arch_path = self.project_dir.join("docs/architecture.md");
        if !arch_path.exists() {
            fs::write(
                &arch_path,
                include_str!("../templates/docs/architecture.md"),
            )?;
            println!("   Created: docs/architecture.md");
        }

        let api_path = self.project_dir.join("docs/api.md");
        if !api_path.exists() {
            fs::write(&api_path, include_str!("../templates/docs/api.md"))?;
            println!("   Created: docs/api.md");
        }

        Ok(())
    }

    /// Create IMPLEMENTATION_PLAN.md
    fn create_implementation_plan(&self) -> Result<()> {
        let path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
        if !path.exists() {
            fs::write(&path, include_str!("../templates/IMPLEMENTATION_PLAN.md"))?;
            println!("   Created: IMPLEMENTATION_PLAN.md");
        }

        Ok(())
    }

    /// Update .gitignore
    fn update_gitignore(&self) -> Result<()> {
        let gitignore_path = self.project_dir.join(".gitignore");
        let additions = r#"
# Ralph Automation Suite
.ralph/
.archive/
.claude/CLAUDE.local.md
context*.txt
sbom.json
*.log
"#;

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;
            if !content.contains("# Ralph Automation Suite") {
                let mut file = fs::OpenOptions::new().append(true).open(&gitignore_path)?;
                use std::io::Write;
                write!(file, "{}", additions)?;
                println!("   Updated: .gitignore");
            }
        } else {
            fs::write(&gitignore_path, additions.trim_start())?;
            println!("   Created: .gitignore");
        }

        Ok(())
    }

    /// Install git hooks
    fn install_git_hooks(&self, force: bool) -> Result<()> {
        let git_hooks_dir = self.project_dir.join(".git/hooks");

        if !git_hooks_dir.exists() {
            println!(
                "   {} Git hooks directory not found (not a git repo?)",
                "Warning:".yellow()
            );
            return Ok(());
        }

        let hooks = [
            (
                "pre-commit",
                include_str!("../templates/git-hooks/pre-commit"),
            ),
            (
                "post-commit",
                include_str!("../templates/git-hooks/post-commit"),
            ),
        ];

        for (name, content) in hooks {
            let path = git_hooks_dir.join(name);
            if !path.exists() || force {
                fs::write(&path, content)?;
                #[cfg(unix)]
                {
                    let mut perms = fs::metadata(&path)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&path, perms)?;
                }
                println!("   Created: .git/hooks/{}", name);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bootstrap_creates_directories() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        // Check key directories exist
        assert!(temp.path().join(".claude").exists());
        assert!(temp.path().join(".claude/skills").exists());
        assert!(temp.path().join(".claude/agents").exists());
        assert!(temp.path().join(".claude/hooks").exists());
        assert!(temp.path().join(".ralph").exists());
        assert!(temp.path().join(".ralph/analysis").exists());
        assert!(temp.path().join(".archive").exists());
        assert!(temp.path().join("docs").exists());
        assert!(temp.path().join("scripts").exists());
    }

    #[test]
    fn test_bootstrap_creates_config_files() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp.path().join(".claude/CLAUDE.md").exists());
        assert!(temp.path().join(".claude/settings.json").exists());
        assert!(temp.path().join(".claude/mcp.json").exists());
    }

    #[test]
    fn test_bootstrap_creates_hooks() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp
            .path()
            .join(".claude/hooks/security-filter.sh")
            .exists());
        assert!(temp.path().join(".claude/hooks/post-edit-scan.sh").exists());
        assert!(temp.path().join(".claude/hooks/end-of-turn.sh").exists());
        assert!(temp.path().join(".claude/hooks/session-init.sh").exists());
    }

    #[test]
    fn test_bootstrap_creates_skills() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp.path().join(".claude/skills/docs-sync.md").exists());
        assert!(temp
            .path()
            .join(".claude/skills/project-analyst.md")
            .exists());
    }

    #[test]
    fn test_bootstrap_creates_agents() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp
            .path()
            .join(".claude/agents/adversarial-reviewer.md")
            .exists());
        assert!(temp
            .path()
            .join(".claude/agents/security-auditor.md")
            .exists());
        assert!(temp.path().join(".claude/agents/supervisor.md").exists());
    }

    #[test]
    fn test_bootstrap_creates_prompts() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp.path().join("PROMPT_plan.md").exists());
        assert!(temp.path().join("PROMPT_build.md").exists());
        assert!(temp.path().join("PROMPT_debug.md").exists());
    }

    #[test]
    fn test_bootstrap_creates_implementation_plan() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp.path().join("IMPLEMENTATION_PLAN.md").exists());
    }

    #[test]
    fn test_bootstrap_creates_doc_templates() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp.path().join("docs/templates/adr-template.md").exists());
        assert!(temp
            .path()
            .join("docs/templates/implementation-template.md")
            .exists());
        assert!(temp.path().join("docs/architecture.md").exists());
        assert!(temp.path().join("docs/api.md").exists());
    }

    #[test]
    fn test_bootstrap_updates_gitignore() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains("# Ralph Automation Suite"));
        assert!(gitignore.contains(".ralph/"));
        assert!(gitignore.contains(".archive/"));
    }

    #[test]
    fn test_bootstrap_does_not_overwrite_without_force() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        // First run
        bootstrap.run(false, false).unwrap();

        // Modify a file
        let claude_md = temp.path().join(".claude/CLAUDE.md");
        std::fs::write(&claude_md, "custom content").unwrap();

        // Second run without force
        bootstrap.run(false, false).unwrap();

        // File should still have custom content
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert_eq!(content, "custom content");
    }

    #[test]
    fn test_bootstrap_overwrites_with_force() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        // First run
        bootstrap.run(false, false).unwrap();

        // Modify a file
        let claude_md = temp.path().join(".claude/CLAUDE.md");
        std::fs::write(&claude_md, "custom content").unwrap();

        // Second run with force
        bootstrap.run(true, false).unwrap();

        // File should have default content
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("Project Memory"));
    }

    // ============================================================
    // Language Detection Integration Tests (Sprint 6c)
    // ============================================================

    #[test]
    fn test_bootstrap_detects_rust_project() {
        let temp = TempDir::new().unwrap();

        // Create a Rust project
        std::fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        std::fs::create_dir(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/main.rs"), "fn main() {}").unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());
        let detected = bootstrap.detect_languages();

        assert!(!detected.is_empty(), "Should detect languages");
        let rust = detected
            .iter()
            .find(|d| d.language == crate::bootstrap::language::Language::Rust);
        assert!(rust.is_some(), "Should detect Rust");
        assert!(rust.unwrap().primary, "Rust should be primary");
    }

    #[test]
    fn test_bootstrap_detects_python_project() {
        let temp = TempDir::new().unwrap();

        // Create a Python project
        std::fs::write(
            temp.path().join("pyproject.toml"),
            "[project]\nname = \"test\"",
        )
        .unwrap();
        std::fs::write(temp.path().join("main.py"), "print('hello')").unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());
        let detected = bootstrap.detect_languages();

        let python = detected
            .iter()
            .find(|d| d.language == crate::bootstrap::language::Language::Python);
        assert!(python.is_some(), "Should detect Python");
        assert!(python.unwrap().primary, "Python should be primary");
    }

    #[test]
    fn test_bootstrap_detects_empty_project() {
        let temp = TempDir::new().unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());
        let detected = bootstrap.detect_languages();

        assert!(
            detected.is_empty(),
            "Empty project should have no detected languages"
        );
    }

    #[test]
    fn test_bootstrap_primary_language() {
        let temp = TempDir::new().unwrap();

        // Create a Go project
        std::fs::write(temp.path().join("go.mod"), "module test").unwrap();
        std::fs::write(temp.path().join("main.go"), "package main\nfunc main() {}").unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());
        let primary = bootstrap.primary_language();

        assert!(primary.is_some(), "Should have a primary language");
        assert_eq!(primary.unwrap(), crate::bootstrap::language::Language::Go);
    }

    #[test]
    fn test_bootstrap_primary_language_empty_project() {
        let temp = TempDir::new().unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());
        let primary = bootstrap.primary_language();

        assert!(
            primary.is_none(),
            "Empty project should have no primary language"
        );
    }

    #[test]
    fn test_bootstrap_run_logs_detected_languages() {
        let temp = TempDir::new().unwrap();

        // Create a TypeScript project
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        std::fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        std::fs::create_dir(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/index.ts"), "export const x = 1;").unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        // This should run successfully and print language info
        let result = bootstrap.run(false, false);
        assert!(result.is_ok(), "Bootstrap should succeed");

        // Verify TypeScript was detected
        let detected = bootstrap.detect_languages();
        let ts = detected
            .iter()
            .find(|d| d.language == crate::bootstrap::language::Language::TypeScript);
        assert!(ts.is_some(), "Should detect TypeScript");
    }

    // ============================================================
    // Language Override Tests (Sprint 9a)
    // ============================================================

    #[test]
    fn test_bootstrap_with_language_override() {
        let temp = TempDir::new().unwrap();

        // Create a Python project
        std::fs::write(temp.path().join("main.py"), "print('hello')").unwrap();

        // But override to use Rust
        let bootstrap =
            Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![Language::Rust]);

        let languages = bootstrap.effective_languages();
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0], Language::Rust);
    }

    #[test]
    fn test_bootstrap_with_multiple_language_overrides() {
        let temp = TempDir::new().unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![
            Language::Rust,
            Language::Python,
            Language::TypeScript,
        ]);

        let languages = bootstrap.effective_languages();
        assert_eq!(languages.len(), 3);
        assert!(languages.contains(&Language::Rust));
        assert!(languages.contains(&Language::Python));
        assert!(languages.contains(&Language::TypeScript));
    }

    #[test]
    fn test_bootstrap_effective_languages_without_override() {
        let temp = TempDir::new().unwrap();

        // Create a Go project
        std::fs::write(temp.path().join("go.mod"), "module test").unwrap();
        std::fs::write(temp.path().join("main.go"), "package main").unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        // Without override, should auto-detect Go
        let languages = bootstrap.effective_languages();
        assert!(!languages.is_empty());
        assert!(languages.contains(&Language::Go));
    }

    #[test]
    fn test_bootstrap_language_override_empty_vec_uses_detection() {
        let temp = TempDir::new().unwrap();

        // Create a Rust project
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        std::fs::create_dir(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/main.rs"), "fn main() {}").unwrap();

        // Empty override should fall back to detection
        let bootstrap = Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![]);

        let languages = bootstrap.effective_languages();
        // Should detect Rust since override is empty
        assert!(languages.contains(&Language::Rust));
    }

    #[test]
    fn test_bootstrap_run_with_language_override() {
        let temp = TempDir::new().unwrap();

        // Override to use Python even though no Python files exist
        let bootstrap =
            Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![Language::Python]);

        // Should succeed with override
        let result = bootstrap.run(false, false);
        assert!(
            result.is_ok(),
            "Bootstrap with language override should succeed"
        );

        // Verify effective languages
        let languages = bootstrap.effective_languages();
        assert_eq!(languages, vec![Language::Python]);
    }

    #[test]
    fn test_bootstrap_builder_pattern_chaining() {
        let temp = TempDir::new().unwrap();

        let bootstrap = Bootstrap::new(temp.path().to_path_buf())
            .with_languages(vec![Language::TypeScript, Language::JavaScript]);

        // Builder pattern should preserve the language override
        let languages = bootstrap.effective_languages();
        assert_eq!(languages.len(), 2);
        assert!(languages.contains(&Language::TypeScript));
        assert!(languages.contains(&Language::JavaScript));
    }

    // ============================================================
    // Language-aware MCP config tests (Sprint 10)
    // ============================================================

    #[test]
    fn test_generate_mcp_config_for_rust() {
        let temp = TempDir::new().unwrap();
        let bootstrap =
            Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![Language::Rust]);

        let config = bootstrap.generate_mcp_config();

        // Should be valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&config).expect("MCP config should be valid JSON");

        // Should have narsil-mcp server
        assert!(parsed["mcpServers"]["narsil-mcp"].is_object());

        // Should have basic flags
        let args = parsed["mcpServers"]["narsil-mcp"]["args"]
            .as_array()
            .unwrap();
        let args_str: Vec<&str> = args.iter().filter_map(|a| a.as_str()).collect();
        assert!(args_str.contains(&"--git"));
        assert!(args_str.contains(&"--call-graph"));
    }

    #[test]
    fn test_generate_mcp_config_for_python() {
        let temp = TempDir::new().unwrap();
        let bootstrap =
            Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![Language::Python]);

        let config = bootstrap.generate_mcp_config();
        let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();

        // Should have narsil-mcp configured
        assert!(parsed["mcpServers"]["narsil-mcp"].is_object());
    }

    #[test]
    fn test_generate_mcp_config_for_polyglot() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf()).with_languages(vec![
            Language::Rust,
            Language::Python,
            Language::TypeScript,
        ]);

        let config = bootstrap.generate_mcp_config();
        let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();

        // Should have narsil-mcp configured
        assert!(parsed["mcpServers"]["narsil-mcp"].is_object());

        // Should have standard flags
        let args = parsed["mcpServers"]["narsil-mcp"]["args"]
            .as_array()
            .unwrap();
        let args_str: Vec<&str> = args.iter().filter_map(|a| a.as_str()).collect();
        assert!(args_str.contains(&"--git"));
    }

    #[test]
    fn test_generate_mcp_config_empty_languages_uses_default() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        let config = bootstrap.generate_mcp_config();

        // Should still be valid JSON with default config
        let parsed: serde_json::Value =
            serde_json::from_str(&config).expect("MCP config should be valid JSON");
        assert!(parsed["mcpServers"]["narsil-mcp"].is_object());
    }
}
