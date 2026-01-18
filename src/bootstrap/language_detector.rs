//! Language detection for multi-language project support.
//!
//! This module provides automatic detection of programming languages used in a project
//! by analyzing file extensions and manifest files. It supports all 32 languages from
//! narsil-mcp and calculates confidence scores for each detected language.
//!
//! # Example
//!
//! ```rust,ignore
//! use ralph::bootstrap::language_detector::LanguageDetector;
//!
//! let detector = LanguageDetector::new("/path/to/project");
//! let languages = detector.detect();
//!
//! for lang in &languages {
//!     println!("{}: {:.0}% confidence ({} files)",
//!         lang.language, lang.confidence * 100.0, lang.file_count);
//! }
//!
//! if let Some(primary) = detector.primary_language() {
//!     println!("Primary language: {}", primary);
//! }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};
use walkdir::WalkDir;

use super::language::Language;

/// A detected language with confidence score and file statistics.
///
/// Confidence is calculated based on the number of source files and the presence
/// of manifest files (like `Cargo.toml` for Rust or `package.json` for JavaScript).
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedLanguage {
    /// The detected programming language
    pub language: Language,
    /// Confidence score from 0.0 to 1.0
    pub confidence: f32,
    /// Number of source files found for this language
    pub file_count: u32,
    /// Whether this is the primary (most confident) language
    pub primary: bool,
}

impl DetectedLanguage {
    /// Create a new detected language entry.
    pub fn new(language: Language, confidence: f32, file_count: u32, primary: bool) -> Self {
        Self {
            language,
            confidence,
            file_count,
            primary,
        }
    }
}

/// Detects programming languages used in a project directory.
///
/// The detector scans the project directory for source files and manifest files,
/// calculating confidence scores based on:
/// - Number of source files with matching extensions
/// - Presence of language-specific manifest files (weighted heavily)
///
/// Files in common ignore directories (`.git`, `node_modules`, `target`, etc.)
/// are automatically excluded from scanning.
pub struct LanguageDetector {
    project_dir: PathBuf,
}

impl LanguageDetector {
    /// Default threshold for polyglot detection (10%).
    ///
    /// A language must have at least this confidence level to be considered
    /// a significant part of a polyglot project.
    pub const DEFAULT_POLYGLOT_THRESHOLD: f32 = 0.10;

    /// Create a new language detector for the given project directory.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::bootstrap::language_detector::LanguageDetector;
    ///
    /// let detector = LanguageDetector::new("/path/to/project");
    /// ```
    pub fn new<P: AsRef<Path>>(project_dir: P) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
        }
    }

    /// Detect all languages used in the project.
    ///
    /// Returns a vector of detected languages sorted by confidence (highest first).
    /// The first language in the list (if any) will have `primary: true`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = LanguageDetector::new(".");
    /// let languages = detector.detect();
    ///
    /// for lang in languages {
    ///     if lang.primary {
    ///         println!("Primary: {}", lang.language);
    ///     }
    /// }
    /// ```
    pub fn detect(&self) -> Vec<DetectedLanguage> {
        let mut counts: HashMap<Language, u32> = HashMap::new();

        // Count source files by extension
        for entry in WalkDir::new(&self.project_dir)
            .into_iter()
            .filter_entry(|e| !Self::should_skip_dir(e.path()))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                let ext_with_dot = format!(".{}", ext);
                for lang in Language::all() {
                    if lang.extensions().contains(&ext_with_dot.as_str()) {
                        *counts.entry(*lang).or_default() += 1;
                    }
                }
            }
        }

        // Boost confidence for manifest files
        for lang in Language::all() {
            for manifest in lang.manifest_files() {
                if Self::manifest_exists(&self.project_dir, manifest) {
                    // Heavy weight for manifests - indicates intentional project setup
                    *counts.entry(*lang).or_default() += 100;
                }
            }
        }

        // Convert to DetectedLanguage with confidence scores
        let total: u32 = counts.values().sum();
        let mut detected: Vec<_> = counts
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(lang, count)| DetectedLanguage {
                language: lang,
                confidence: count as f32 / total.max(1) as f32,
                file_count: count,
                primary: false,
            })
            .collect();

        // Sort by confidence (highest first)
        detected.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Mark the first as primary
        if let Some(first) = detected.first_mut() {
            first.primary = true;
        }

        detected
    }

    /// Get the primary (most confident) language for this project.
    ///
    /// Returns `None` if no source files are detected.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = LanguageDetector::new(".");
    /// if let Some(lang) = detector.primary_language() {
    ///     println!("This is a {} project", lang);
    /// }
    /// ```
    pub fn primary_language(&self) -> Option<Language> {
        self.detect()
            .into_iter()
            .find(|d| d.primary)
            .map(|d| d.language)
    }

    /// Check if this is a polyglot project (multiple significant languages).
    ///
    /// A project is considered polyglot if it has two or more languages with
    /// confidence scores at or above the default threshold (10%).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = LanguageDetector::new(".");
    /// if detector.is_polyglot() {
    ///     println!("This is a polyglot project!");
    /// }
    /// ```
    #[must_use]
    pub fn is_polyglot(&self) -> bool {
        self.is_polyglot_with_threshold(Self::DEFAULT_POLYGLOT_THRESHOLD)
    }

    /// Check if this is a polyglot project with a custom threshold.
    ///
    /// A project is considered polyglot if it has two or more languages with
    /// confidence scores at or above the given threshold.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum confidence (0.0 to 1.0) for a language to count
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = LanguageDetector::new(".");
    /// // Check with 20% threshold
    /// if detector.is_polyglot_with_threshold(0.20) {
    ///     println!("This project has multiple major languages!");
    /// }
    /// ```
    #[must_use]
    pub fn is_polyglot_with_threshold(&self, threshold: f32) -> bool {
        self.polyglot_languages_with_threshold(threshold).len() >= 2
    }

    /// Get all languages that meet the polyglot threshold.
    ///
    /// Returns languages with confidence at or above the default threshold (10%),
    /// sorted by confidence (highest first).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = LanguageDetector::new(".");
    /// for lang in detector.polyglot_languages() {
    ///     println!("{}: {:.0}%", lang.language, lang.confidence * 100.0);
    /// }
    /// ```
    #[must_use]
    pub fn polyglot_languages(&self) -> Vec<DetectedLanguage> {
        self.polyglot_languages_with_threshold(Self::DEFAULT_POLYGLOT_THRESHOLD)
    }

    /// Get all languages above a custom confidence threshold.
    ///
    /// Returns languages with confidence at or above the given threshold,
    /// sorted by confidence (highest first).
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum confidence (0.0 to 1.0) for inclusion
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let detector = LanguageDetector::new(".");
    /// // Get languages with at least 5% presence
    /// let languages = detector.polyglot_languages_with_threshold(0.05);
    /// ```
    #[must_use]
    pub fn polyglot_languages_with_threshold(&self, threshold: f32) -> Vec<DetectedLanguage> {
        self.detect()
            .into_iter()
            .filter(|d| d.confidence >= threshold)
            .collect()
    }

    /// Check if a manifest file exists, supporting glob patterns.
    fn manifest_exists(project_dir: &Path, manifest: &str) -> bool {
        if manifest.contains('*') {
            // Handle glob patterns like "*.csproj"
            if let Ok(glob) = Glob::new(manifest) {
                let mut builder = GlobSetBuilder::new();
                builder.add(glob);
                if let Ok(set) = builder.build() {
                    // Check only top-level directory for manifest files
                    if let Ok(entries) = std::fs::read_dir(project_dir) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            if let Some(name) = entry.file_name().to_str() {
                                if set.is_match(name) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
            false
        } else {
            // Exact match
            project_dir.join(manifest).exists()
        }
    }

    /// Check if a directory should be skipped during scanning.
    fn should_skip_dir(path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            matches!(
                name,
                ".git"
                    | "node_modules"
                    | "target"
                    | "dist"
                    | "build"
                    | ".cache"
                    | "__pycache__"
                    | ".venv"
                    | "venv"
                    | ".tox"
                    | "vendor"
                    | ".bundle"
                    | "Pods"
                    | ".gradle"
                    | ".idea"
                    | ".vs"
                    | ".vscode"
            )
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ============================================================
    // DetectedLanguage struct tests
    // ============================================================

    #[test]
    fn test_detected_language_new() {
        let detected = DetectedLanguage::new(Language::Rust, 0.75, 10, true);
        assert_eq!(detected.language, Language::Rust);
        assert!((detected.confidence - 0.75).abs() < f32::EPSILON);
        assert_eq!(detected.file_count, 10);
        assert!(detected.primary);
    }

    #[test]
    fn test_detected_language_clone() {
        let original = DetectedLanguage::new(Language::Python, 0.5, 5, false);
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // ============================================================
    // LanguageDetector::new() tests
    // ============================================================

    #[test]
    fn test_detector_new_with_path() {
        let detector = LanguageDetector::new("/tmp/test");
        assert_eq!(detector.project_dir, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_detector_new_with_pathbuf() {
        let path = PathBuf::from("/some/project");
        let detector = LanguageDetector::new(&path);
        assert_eq!(detector.project_dir, path);
    }

    // ============================================================
    // Empty project tests
    // ============================================================

    #[test]
    fn test_detect_empty_project() {
        let temp = TempDir::new().unwrap();
        let detector = LanguageDetector::new(temp.path());

        let detected = detector.detect();
        assert!(detected.is_empty(), "Empty project should have no languages");
    }

    #[test]
    fn test_primary_language_empty_project() {
        let temp = TempDir::new().unwrap();
        let detector = LanguageDetector::new(temp.path());

        assert!(
            detector.primary_language().is_none(),
            "Empty project should have no primary language"
        );
    }

    // ============================================================
    // Single language detection tests
    // ============================================================

    #[test]
    fn test_detect_rust_project() {
        let temp = TempDir::new().unwrap();

        // Create Rust project structure
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(temp.path().join("src/lib.rs"), "pub fn foo() {}").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        assert!(!detected.is_empty(), "Should detect languages");
        let rust = detected.iter().find(|d| d.language == Language::Rust);
        assert!(rust.is_some(), "Should detect Rust");
        assert!(rust.unwrap().primary, "Rust should be primary");
    }

    #[test]
    fn test_detect_python_project() {
        let temp = TempDir::new().unwrap();

        // Create Python project structure
        fs::write(temp.path().join("pyproject.toml"), "[project]\nname = \"test\"").unwrap();
        fs::write(temp.path().join("main.py"), "print('hello')").unwrap();
        fs::write(temp.path().join("utils.py"), "def helper(): pass").unwrap();
        fs::create_dir(temp.path().join("tests")).unwrap();
        fs::write(temp.path().join("tests/test_main.py"), "def test_foo(): pass").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let python = detected.iter().find(|d| d.language == Language::Python);
        assert!(python.is_some(), "Should detect Python");
        assert!(python.unwrap().primary, "Python should be primary");
    }

    #[test]
    fn test_detect_typescript_project() {
        let temp = TempDir::new().unwrap();

        // Create TypeScript project structure
        fs::write(temp.path().join("package.json"), "{}").unwrap();
        fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("src/index.ts"), "export const x = 1;").unwrap();
        fs::write(temp.path().join("src/utils.ts"), "export function foo() {}").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let ts = detected.iter().find(|d| d.language == Language::TypeScript);
        assert!(ts.is_some(), "Should detect TypeScript");
        assert!(ts.unwrap().primary, "TypeScript should be primary");
    }

    #[test]
    fn test_detect_go_project() {
        let temp = TempDir::new().unwrap();

        // Create Go project structure
        fs::write(temp.path().join("go.mod"), "module test").unwrap();
        fs::write(temp.path().join("main.go"), "package main\nfunc main() {}").unwrap();
        fs::write(temp.path().join("utils.go"), "package main\nfunc foo() {}").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let go = detected.iter().find(|d| d.language == Language::Go);
        assert!(go.is_some(), "Should detect Go");
        assert!(go.unwrap().primary, "Go should be primary");
    }

    #[test]
    fn test_detect_java_project() {
        let temp = TempDir::new().unwrap();

        // Create Java project structure
        fs::write(temp.path().join("pom.xml"), "<project></project>").unwrap();
        fs::create_dir_all(temp.path().join("src/main/java")).unwrap();
        fs::write(
            temp.path().join("src/main/java/Main.java"),
            "public class Main {}",
        )
        .unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let java = detected.iter().find(|d| d.language == Language::Java);
        assert!(java.is_some(), "Should detect Java");
        assert!(java.unwrap().primary, "Java should be primary");
    }

    // ============================================================
    // Polyglot detection tests
    // ============================================================

    #[test]
    fn test_detect_polyglot_project() {
        let temp = TempDir::new().unwrap();

        // Create a polyglot project (Python backend + TypeScript frontend)
        fs::write(temp.path().join("pyproject.toml"), "[project]\nname = \"backend\"").unwrap();
        fs::create_dir(temp.path().join("backend")).unwrap();
        fs::write(temp.path().join("backend/app.py"), "from flask import Flask").unwrap();
        fs::write(temp.path().join("backend/models.py"), "class User: pass").unwrap();
        fs::write(temp.path().join("backend/routes.py"), "def get_users(): pass").unwrap();

        fs::write(temp.path().join("package.json"), "{}").unwrap();
        fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        fs::create_dir(temp.path().join("frontend")).unwrap();
        fs::write(temp.path().join("frontend/App.tsx"), "export default function App() {}").unwrap();
        fs::write(temp.path().join("frontend/utils.ts"), "export function foo() {}").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        // Should detect both languages
        let python = detected.iter().find(|d| d.language == Language::Python);
        let ts = detected.iter().find(|d| d.language == Language::TypeScript);

        assert!(python.is_some(), "Should detect Python");
        assert!(ts.is_some(), "Should detect TypeScript");

        // Both should have confidence > 0
        assert!(python.unwrap().confidence > 0.0);
        assert!(ts.unwrap().confidence > 0.0);

        // Only one should be primary
        let primary_count = detected.iter().filter(|d| d.primary).count();
        assert_eq!(primary_count, 1, "Exactly one language should be primary");
    }

    // ============================================================
    // Confidence scoring tests
    // ============================================================

    #[test]
    fn test_confidence_sums_to_one() {
        let temp = TempDir::new().unwrap();

        // Create a project with multiple languages
        fs::write(temp.path().join("main.py"), "print('hello')").unwrap();
        fs::write(temp.path().join("script.js"), "console.log('hi')").unwrap();
        fs::write(temp.path().join("app.rb"), "puts 'hi'").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let total_confidence: f32 = detected.iter().map(|d| d.confidence).sum();
        assert!(
            (total_confidence - 1.0).abs() < 0.01,
            "Total confidence should be approximately 1.0, got {}",
            total_confidence
        );
    }

    #[test]
    fn test_manifest_boosts_confidence() {
        let temp = TempDir::new().unwrap();

        // Create a project where Rust has fewer files but has Cargo.toml
        // and JavaScript has more files but no package.json
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        fs::write(temp.path().join("src.rs"), "").unwrap();

        // Multiple JS files without manifest
        for i in 0..5 {
            fs::write(temp.path().join(format!("file{}.js", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let rust = detected.iter().find(|d| d.language == Language::Rust);
        let js = detected.iter().find(|d| d.language == Language::JavaScript);

        assert!(rust.is_some(), "Should detect Rust");
        assert!(js.is_some(), "Should detect JavaScript");

        // Rust should have higher confidence due to Cargo.toml despite fewer files
        assert!(
            rust.unwrap().confidence > js.unwrap().confidence,
            "Rust with manifest should have higher confidence than JS without"
        );
    }

    // ============================================================
    // Directory skip tests
    // ============================================================

    #[test]
    fn test_skips_node_modules() {
        let temp = TempDir::new().unwrap();

        // Create a TypeScript project
        fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        fs::write(temp.path().join("index.ts"), "export const x = 1;").unwrap();

        // Add node_modules with lots of JS files (should be ignored)
        fs::create_dir_all(temp.path().join("node_modules/some-package")).unwrap();
        for i in 0..100 {
            fs::write(
                temp.path().join(format!("node_modules/some-package/file{}.js", i)),
                "",
            )
            .unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let ts = detected.iter().find(|d| d.language == Language::TypeScript);
        let js = detected.iter().find(|d| d.language == Language::JavaScript);

        assert!(ts.is_some(), "Should detect TypeScript");
        // JS should either not be detected or have much lower count
        if let Some(js) = js {
            assert!(
                js.file_count < 10,
                "node_modules JS files should be ignored"
            );
        }
    }

    #[test]
    fn test_skips_target_directory() {
        let temp = TempDir::new().unwrap();

        // Create a Rust project
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        fs::write(temp.path().join("src.rs"), "fn main() {}").unwrap();

        // Add target directory (should be ignored)
        fs::create_dir_all(temp.path().join("target/debug")).unwrap();
        for i in 0..50 {
            fs::write(temp.path().join(format!("target/debug/dep{}.rs", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let rust = detected.iter().find(|d| d.language == Language::Rust);
        assert!(rust.is_some(), "Should detect Rust");
        // File count should be 101 (Cargo.toml boost) + 1 (src.rs) = 102
        // NOT 102 + 50 from target
        assert!(
            rust.unwrap().file_count < 110,
            "target directory files should be ignored, got {} files",
            rust.unwrap().file_count
        );
    }

    #[test]
    fn test_skips_venv_directory() {
        let temp = TempDir::new().unwrap();

        // Create a Python project
        fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();
        fs::write(temp.path().join("main.py"), "print('hello')").unwrap();

        // Add venv directory (should be ignored)
        fs::create_dir_all(temp.path().join(".venv/lib/python3.11/site-packages")).unwrap();
        for i in 0..50 {
            fs::write(
                temp.path()
                    .join(format!(".venv/lib/python3.11/site-packages/pkg{}.py", i)),
                "",
            )
            .unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let python = detected.iter().find(|d| d.language == Language::Python);
        assert!(python.is_some(), "Should detect Python");
        assert!(
            python.unwrap().file_count < 110,
            ".venv directory files should be ignored"
        );
    }

    // ============================================================
    // Glob pattern tests
    // ============================================================

    #[test]
    fn test_manifest_glob_pattern_csproj() {
        let temp = TempDir::new().unwrap();

        // Create a C# project with .csproj file
        fs::write(temp.path().join("MyProject.csproj"), "<Project></Project>").unwrap();
        fs::write(temp.path().join("Program.cs"), "class Program {}").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let csharp = detected.iter().find(|d| d.language == Language::CSharp);
        assert!(csharp.is_some(), "Should detect C# via *.csproj glob");
        assert!(csharp.unwrap().primary, "C# should be primary");
    }

    #[test]
    fn test_manifest_glob_pattern_gemspec() {
        let temp = TempDir::new().unwrap();

        // Create a Ruby gem project
        fs::write(temp.path().join("mygem.gemspec"), "Gem::Specification.new").unwrap();
        fs::write(temp.path().join("lib.rb"), "module MyGem; end").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let ruby = detected.iter().find(|d| d.language == Language::Ruby);
        assert!(ruby.is_some(), "Should detect Ruby via *.gemspec glob");
    }

    // ============================================================
    // Edge case tests
    // ============================================================

    #[test]
    fn test_unknown_file_extensions() {
        let temp = TempDir::new().unwrap();

        // Create files with unknown extensions
        fs::write(temp.path().join("data.xyz"), "some data").unwrap();
        fs::write(temp.path().join("config.unknown"), "config").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        assert!(
            detected.is_empty(),
            "Unknown extensions should not detect any language"
        );
    }

    #[test]
    fn test_files_without_extensions() {
        let temp = TempDir::new().unwrap();

        // Create files without extensions
        fs::write(temp.path().join("Makefile"), "all: build").unwrap();
        fs::write(temp.path().join("Dockerfile"), "FROM ubuntu").unwrap();
        fs::write(temp.path().join("README"), "readme content").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        // These shouldn't crash - they just won't match any language by extension
        // (Some might match via manifest files in real scenarios)
        assert!(detected.is_empty() || detected.iter().all(|d| d.file_count > 0));
    }

    #[test]
    fn test_deeply_nested_files() {
        let temp = TempDir::new().unwrap();

        // Create deeply nested structure
        let deep_path = temp.path().join("a/b/c/d/e/f");
        fs::create_dir_all(&deep_path).unwrap();
        fs::write(deep_path.join("deep.rs"), "fn deep() {}").unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        let rust = detected.iter().find(|d| d.language == Language::Rust);
        assert!(rust.is_some(), "Should find deeply nested Rust files");
    }

    #[test]
    fn test_primary_language_returns_highest_confidence() {
        let temp = TempDir::new().unwrap();

        // Create a project clearly dominated by Rust
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..10 {
            fs::write(temp.path().join(format!("file{}.rs", i)), "").unwrap();
        }
        // Add a single Python file
        fs::write(temp.path().join("script.py"), "").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let primary = detector.primary_language();

        assert_eq!(primary, Some(Language::Rust), "Rust should be primary");
    }

    // ============================================================
    // Sorting tests
    // ============================================================

    #[test]
    fn test_detected_languages_sorted_by_confidence() {
        let temp = TempDir::new().unwrap();

        // Create files for multiple languages with clear hierarchy
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..10 {
            fs::write(temp.path().join(format!("file{}.rs", i)), "").unwrap();
        }
        for i in 0..5 {
            fs::write(temp.path().join(format!("script{}.py", i)), "").unwrap();
        }
        fs::write(temp.path().join("single.js"), "").unwrap();

        let detector = LanguageDetector::new(temp.path());
        let detected = detector.detect();

        // Check that they're sorted by confidence (descending)
        for window in detected.windows(2) {
            assert!(
                window[0].confidence >= window[1].confidence,
                "Languages should be sorted by confidence descending"
            );
        }

        // First should be primary
        assert!(detected[0].primary, "First language should be primary");
        // Others should not
        for lang in &detected[1..] {
            assert!(!lang.primary, "Only first language should be primary");
        }
    }

    // ============================================================
    // Polyglot detection tests (Sprint 10)
    // ============================================================

    #[test]
    fn test_is_polyglot_returns_false_for_single_language() {
        let temp = TempDir::new().unwrap();

        // Pure Rust project
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..10 {
            fs::write(temp.path().join(format!("file{}.rs", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        assert!(
            !detector.is_polyglot(),
            "Single-language project should not be polyglot"
        );
    }

    #[test]
    fn test_is_polyglot_returns_true_for_significant_secondary_language() {
        let temp = TempDir::new().unwrap();

        // Python backend with significant TypeScript frontend (both >10%)
        fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();
        for i in 0..5 {
            fs::write(temp.path().join(format!("backend{}.py", i)), "").unwrap();
        }

        fs::write(temp.path().join("package.json"), "{}").unwrap();
        fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        for i in 0..5 {
            fs::write(temp.path().join(format!("frontend{}.ts", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        assert!(
            detector.is_polyglot(),
            "Project with 2+ languages >10% should be polyglot"
        );
    }

    #[test]
    fn test_is_polyglot_returns_false_when_secondary_below_threshold() {
        let temp = TempDir::new().unwrap();

        // Rust project with tiny Python script (<10%)
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..50 {
            fs::write(temp.path().join(format!("file{}.rs", i)), "").unwrap();
        }
        // Just 1 Python file among 150 (100 from manifest + 50 files)
        fs::write(temp.path().join("script.py"), "").unwrap();

        let detector = LanguageDetector::new(temp.path());
        assert!(
            !detector.is_polyglot(),
            "Single significant language with minor secondary should not be polyglot"
        );
    }

    #[test]
    fn test_polyglot_languages_returns_all_above_threshold() {
        let temp = TempDir::new().unwrap();

        // Three significant languages
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..3 {
            fs::write(temp.path().join(format!("rust{}.rs", i)), "").unwrap();
        }

        fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();
        for i in 0..3 {
            fs::write(temp.path().join(format!("python{}.py", i)), "").unwrap();
        }

        fs::write(temp.path().join("package.json"), "{}").unwrap();
        for i in 0..3 {
            fs::write(temp.path().join(format!("script{}.js", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());
        let polyglot = detector.polyglot_languages();

        assert!(
            polyglot.len() >= 2,
            "Should have at least 2 polyglot languages, got {}",
            polyglot.len()
        );

        // All returned should have >10% confidence
        for lang in &polyglot {
            assert!(
                lang.confidence >= 0.10,
                "Polyglot language {} should have >=10% confidence, has {}",
                lang.language,
                lang.confidence
            );
        }
    }

    #[test]
    fn test_polyglot_languages_with_custom_threshold() {
        let temp = TempDir::new().unwrap();

        // Balanced project: both languages have manifests for significant weight
        // Rust: Cargo.toml (100) + 10 files = 110
        // Python: pyproject.toml (100) + 5 files = 105
        // Total: 215
        // Rust: 110/215 ≈ 51%, Python: 105/215 ≈ 49%
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..10 {
            fs::write(temp.path().join(format!("rust{}.rs", i)), "").unwrap();
        }
        fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();
        for i in 0..5 {
            fs::write(temp.path().join(format!("python{}.py", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());

        // With 5% threshold, Python should be included
        let lenient = detector.polyglot_languages_with_threshold(0.05);
        let python_in_lenient = lenient.iter().any(|l| l.language == Language::Python);
        assert!(
            python_in_lenient,
            "Python should be included with 5% threshold"
        );

        // With 55% threshold, neither should pass (both are ~50%)
        let strict = detector.polyglot_languages_with_threshold(0.55);
        assert!(
            strict.is_empty(),
            "No language should pass 55% threshold in balanced project"
        );
    }

    #[test]
    fn test_polyglot_languages_returns_empty_for_empty_project() {
        let temp = TempDir::new().unwrap();
        let detector = LanguageDetector::new(temp.path());
        let polyglot = detector.polyglot_languages();
        assert!(polyglot.is_empty(), "Empty project should have no polyglot languages");
    }

    #[test]
    fn test_is_polyglot_with_custom_threshold() {
        let temp = TempDir::new().unwrap();

        // Create balanced project with both manifests
        // Rust: Cargo.toml (100) + 5 files = 105
        // Python: pyproject.toml (100) + 5 files = 105
        // Total: 210, each ~50%
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        for i in 0..5 {
            fs::write(temp.path().join(format!("rust{}.rs", i)), "").unwrap();
        }
        fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();
        for i in 0..5 {
            fs::write(temp.path().join(format!("python{}.py", i)), "").unwrap();
        }

        let detector = LanguageDetector::new(temp.path());

        // Should be polyglot at default 10% threshold (both >10%)
        assert!(detector.is_polyglot_with_threshold(0.10));

        // Should still be polyglot at 40% threshold (both ~50%)
        assert!(detector.is_polyglot_with_threshold(0.40));

        // Should NOT be polyglot at 60% threshold (neither is >60%)
        assert!(!detector.is_polyglot_with_threshold(0.60));
    }

    #[test]
    fn test_default_polyglot_threshold_constant() {
        // Verify the constant is 0.10 (10%)
        assert!(
            (LanguageDetector::DEFAULT_POLYGLOT_THRESHOLD - 0.10).abs() < f32::EPSILON,
            "Default polyglot threshold should be 10%"
        );
    }
}
