//! Bootstrap module for setting up the automation suite in a project.
//!
//! This module creates all necessary directories, configuration files,
//! prompts, and scripts needed for the automation suite.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// Bootstrap manager
pub struct Bootstrap {
    project_dir: PathBuf,
}

impl Bootstrap {
    /// Create a new bootstrap manager
    pub fn new(project_dir: PathBuf) -> Self {
        Self { project_dir }
    }

    /// Run the bootstrap process
    pub fn run(&self, force: bool, install_git_hooks: bool) -> Result<()> {
        println!(
            "{} Bootstrapping automation suite in {}",
            "Info:".blue(),
            self.project_dir.display()
        );

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
    fn create_claude_config(&self, force: bool) -> Result<()> {
        let claude_md = self.project_dir.join(".claude/CLAUDE.md");
        let settings_json = self.project_dir.join(".claude/settings.json");

        if !claude_md.exists() || force {
            fs::write(&claude_md, include_str!("templates/CLAUDE.md"))?;
            println!("   Created: .claude/CLAUDE.md");
        }

        if !settings_json.exists() || force {
            fs::write(&settings_json, include_str!("templates/settings.json"))?;
            println!("   Created: .claude/settings.json");
        }

        Ok(())
    }

    /// Create MCP configuration
    fn create_mcp_config(&self, force: bool) -> Result<()> {
        let mcp_json = self.project_dir.join(".claude/mcp.json");

        if !mcp_json.exists() || force {
            fs::write(&mcp_json, include_str!("templates/mcp.json"))?;
            println!("   Created: .claude/mcp.json");
        }

        Ok(())
    }

    /// Create hook scripts
    fn create_hooks(&self, force: bool) -> Result<()> {
        let hooks = [
            ("security-filter.sh", include_str!("templates/hooks/security-filter.sh")),
            ("post-edit-scan.sh", include_str!("templates/hooks/post-edit-scan.sh")),
            ("end-of-turn.sh", include_str!("templates/hooks/end-of-turn.sh")),
            ("session-init.sh", include_str!("templates/hooks/session-init.sh")),
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
            ("docs-sync.md", include_str!("templates/skills/docs-sync.md")),
            ("project-analyst.md", include_str!("templates/skills/project-analyst.md")),
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
            ("adversarial-reviewer.md", include_str!("templates/agents/adversarial-reviewer.md")),
            ("security-auditor.md", include_str!("templates/agents/security-auditor.md")),
            ("supervisor.md", include_str!("templates/agents/supervisor.md")),
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
            ("PROMPT_plan.md", include_str!("templates/PROMPT_plan.md")),
            ("PROMPT_build.md", include_str!("templates/PROMPT_build.md")),
            ("PROMPT_debug.md", include_str!("templates/PROMPT_debug.md")),
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
            ("adr-template.md", include_str!("templates/docs/adr-template.md")),
            ("implementation-template.md", include_str!("templates/docs/implementation-template.md")),
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
            fs::write(&arch_path, include_str!("templates/docs/architecture.md"))?;
            println!("   Created: docs/architecture.md");
        }

        let api_path = self.project_dir.join("docs/api.md");
        if !api_path.exists() {
            fs::write(&api_path, include_str!("templates/docs/api.md"))?;
            println!("   Created: docs/api.md");
        }

        Ok(())
    }

    /// Create IMPLEMENTATION_PLAN.md
    fn create_implementation_plan(&self) -> Result<()> {
        let path = self.project_dir.join("IMPLEMENTATION_PLAN.md");
        if !path.exists() {
            fs::write(&path, include_str!("templates/IMPLEMENTATION_PLAN.md"))?;
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
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .open(&gitignore_path)?;
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
            ("pre-commit", include_str!("templates/git-hooks/pre-commit")),
            ("post-commit", include_str!("templates/git-hooks/post-commit")),
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

        assert!(temp.path().join(".claude/hooks/security-filter.sh").exists());
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
        assert!(temp.path().join(".claude/skills/project-analyst.md").exists());
    }

    #[test]
    fn test_bootstrap_creates_agents() {
        let temp = TempDir::new().unwrap();
        let bootstrap = Bootstrap::new(temp.path().to_path_buf());

        bootstrap.run(false, false).unwrap();

        assert!(temp.path().join(".claude/agents/adversarial-reviewer.md").exists());
        assert!(temp.path().join(".claude/agents/security-auditor.md").exists());
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
        assert!(temp.path().join("docs/templates/implementation-template.md").exists());
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
}
