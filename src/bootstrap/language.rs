//! Language detection and representation for multi-language support.
//!
//! This module provides the `Language` enum representing all programming languages
//! supported by narsil-mcp, along with methods for identifying languages by file
//! extension and manifest files.
//!
//! # Example
//!
//! ```rust
//! use ralph::Language;
//!
//! // Get file extensions for a language
//! let extensions = Language::Rust.extensions();
//! assert!(extensions.contains(&".rs"));
//!
//! // Get manifest files
//! let manifests = Language::Rust.manifest_files();
//! assert!(manifests.contains(&"Cargo.toml"));
//!
//! // Parse from string
//! let lang: Language = "rust".parse().unwrap();
//! assert_eq!(lang, Language::Rust);
//!
//! // Display
//! assert_eq!(format!("{}", Language::Python), "Python");
//! ```

use std::fmt;
use std::str::FromStr;

/// All programming languages supported by narsil-mcp.
///
/// Each language has associated file extensions and manifest files used for
/// automatic language detection during project bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Language {
    /// Rust programming language
    Rust,
    /// Python programming language
    Python,
    /// JavaScript programming language
    JavaScript,
    /// TypeScript programming language
    TypeScript,
    /// Go programming language
    Go,
    /// Java programming language
    Java,
    /// C# programming language
    CSharp,
    /// C++ programming language
    Cpp,
    /// C programming language
    C,
    /// Ruby programming language
    Ruby,
    /// PHP programming language
    Php,
    /// Swift programming language
    Swift,
    /// Kotlin programming language
    Kotlin,
    /// Scala programming language
    Scala,
    /// Objective-C programming language
    ObjectiveC,
    /// Perl programming language
    Perl,
    /// Lua programming language
    Lua,
    /// Shell/Bash scripting
    Bash,
    /// PowerShell scripting
    PowerShell,
    /// R programming language
    R,
    /// Julia programming language
    Julia,
    /// Dart programming language
    Dart,
    /// Elixir programming language
    Elixir,
    /// Clojure programming language
    Clojure,
    /// Haskell programming language
    Haskell,
    /// OCaml programming language
    OCaml,
    /// F# programming language
    FSharp,
    /// Zig programming language
    Zig,
    /// Nim programming language
    Nim,
    /// Erlang programming language
    Erlang,
    /// Groovy programming language
    Groovy,
    /// SQL database language
    Sql,
}

/// Static array of all languages for iteration
static ALL_LANGUAGES: &[Language] = &[
    Language::Rust,
    Language::Python,
    Language::JavaScript,
    Language::TypeScript,
    Language::Go,
    Language::Java,
    Language::CSharp,
    Language::Cpp,
    Language::C,
    Language::Ruby,
    Language::Php,
    Language::Swift,
    Language::Kotlin,
    Language::Scala,
    Language::ObjectiveC,
    Language::Perl,
    Language::Lua,
    Language::Bash,
    Language::PowerShell,
    Language::R,
    Language::Julia,
    Language::Dart,
    Language::Elixir,
    Language::Clojure,
    Language::Haskell,
    Language::OCaml,
    Language::FSharp,
    Language::Zig,
    Language::Nim,
    Language::Erlang,
    Language::Groovy,
    Language::Sql,
];

impl Language {
    /// Returns all supported languages.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::Language;
    ///
    /// let all = Language::all();
    /// assert!(all.len() >= 32);
    /// ```
    pub fn all() -> &'static [Language] {
        ALL_LANGUAGES
    }

    /// Returns the file extensions associated with this language.
    ///
    /// Extensions include the leading dot (e.g., ".rs", ".py").
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::Language;
    ///
    /// assert!(Language::Rust.extensions().contains(&".rs"));
    /// assert!(Language::Python.extensions().contains(&".py"));
    /// ```
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Language::Rust => &[".rs"],
            Language::Python => &[".py", ".pyi", ".pyw"],
            Language::JavaScript => &[".js", ".jsx", ".mjs", ".cjs"],
            Language::TypeScript => &[".ts", ".tsx", ".mts", ".cts"],
            Language::Go => &[".go"],
            Language::Java => &[".java"],
            Language::CSharp => &[".cs"],
            Language::Cpp => &[".cpp", ".cc", ".cxx", ".hpp", ".hxx", ".h++", ".hh"],
            Language::C => &[".c", ".h"],
            Language::Ruby => &[".rb", ".rake", ".gemspec", ".ru"],
            Language::Php => &[".php", ".phtml", ".php5", ".php7"],
            Language::Swift => &[".swift"],
            Language::Kotlin => &[".kt", ".kts"],
            Language::Scala => &[".scala", ".sc"],
            Language::ObjectiveC => &[".m", ".mm"],
            Language::Perl => &[".pl", ".pm", ".t"],
            Language::Lua => &[".lua"],
            Language::Bash => &[".sh", ".bash", ".zsh"],
            Language::PowerShell => &[".ps1", ".psm1", ".psd1"],
            Language::R => &[".r", ".R", ".Rmd"],
            Language::Julia => &[".jl"],
            Language::Dart => &[".dart"],
            Language::Elixir => &[".ex", ".exs"],
            Language::Clojure => &[".clj", ".cljs", ".cljc", ".edn"],
            Language::Haskell => &[".hs", ".lhs"],
            Language::OCaml => &[".ml", ".mli"],
            Language::FSharp => &[".fs", ".fsi", ".fsx"],
            Language::Zig => &[".zig"],
            Language::Nim => &[".nim", ".nims"],
            Language::Erlang => &[".erl", ".hrl"],
            Language::Groovy => &[".groovy", ".gradle"],
            Language::Sql => &[".sql"],
        }
    }

    /// Returns manifest/configuration files that indicate this language is used.
    ///
    /// Manifest files are project-level configuration files like `Cargo.toml`,
    /// `package.json`, `pyproject.toml`, etc. Some entries may use glob patterns
    /// like `*.csproj` for languages with variable manifest names.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::Language;
    ///
    /// assert!(Language::Rust.manifest_files().contains(&"Cargo.toml"));
    /// assert!(Language::Python.manifest_files().contains(&"pyproject.toml"));
    /// ```
    pub fn manifest_files(&self) -> &'static [&'static str] {
        match self {
            Language::Rust => &["Cargo.toml", "Cargo.lock"],
            Language::Python => &[
                "pyproject.toml",
                "setup.py",
                "setup.cfg",
                "requirements.txt",
                "Pipfile",
                "poetry.lock",
            ],
            Language::JavaScript => &["package.json", "package-lock.json", "yarn.lock", ".npmrc"],
            Language::TypeScript => &[
                "tsconfig.json",
                "package.json",
                "tsconfig.build.json",
                "tsconfig.base.json",
            ],
            Language::Go => &["go.mod", "go.sum", "go.work"],
            Language::Java => &[
                "pom.xml",
                "build.gradle",
                "build.gradle.kts",
                "settings.gradle",
                "settings.gradle.kts",
            ],
            Language::CSharp => &["*.csproj", "*.sln", "Directory.Build.props", "nuget.config"],
            Language::Cpp => &[
                "CMakeLists.txt",
                "Makefile",
                "meson.build",
                "conanfile.txt",
                "vcpkg.json",
            ],
            Language::C => &["CMakeLists.txt", "Makefile", "configure.ac", "configure"],
            Language::Ruby => &["Gemfile", "Gemfile.lock", "*.gemspec", "Rakefile"],
            Language::Php => &["composer.json", "composer.lock", "phpunit.xml"],
            Language::Swift => &["Package.swift", "*.xcodeproj", "*.xcworkspace", "Podfile"],
            Language::Kotlin => &[
                "build.gradle.kts",
                "settings.gradle.kts",
                "build.gradle",
                "gradle.properties",
            ],
            Language::Scala => &["build.sbt", "build.sc", "project/build.properties"],
            Language::ObjectiveC => &["*.xcodeproj", "*.xcworkspace", "Podfile", "Cartfile"],
            Language::Perl => &["cpanfile", "Makefile.PL", "dist.ini", "META.json"],
            Language::Lua => &["*.rockspec", ".luacheckrc"],
            Language::Bash => &[".bashrc", ".bash_profile", "Makefile"],
            Language::PowerShell => &["*.psd1", "psake.ps1"],
            Language::R => &["DESCRIPTION", "NAMESPACE", ".Rprofile", "renv.lock"],
            Language::Julia => &["Project.toml", "Manifest.toml"],
            Language::Dart => &["pubspec.yaml", "pubspec.lock"],
            Language::Elixir => &["mix.exs", "mix.lock"],
            Language::Clojure => &["project.clj", "deps.edn", "build.clj"],
            Language::Haskell => &["*.cabal", "stack.yaml", "cabal.project", "package.yaml"],
            Language::OCaml => &["dune-project", "dune", "*.opam", "esy.json"],
            Language::FSharp => &["*.fsproj", "*.fsx", "paket.dependencies"],
            Language::Zig => &["build.zig", "build.zig.zon"],
            Language::Nim => &["*.nimble", "nim.cfg"],
            Language::Erlang => &["rebar.config", "erlang.mk", "relx.config"],
            Language::Groovy => &["build.gradle", "settings.gradle", "Jenkinsfile"],
            Language::Sql => &["*.sql", "flyway.conf", "liquibase.properties"],
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::CSharp => "C#",
            Language::Cpp => "C++",
            Language::C => "C",
            Language::Ruby => "Ruby",
            Language::Php => "PHP",
            Language::Swift => "Swift",
            Language::Kotlin => "Kotlin",
            Language::Scala => "Scala",
            Language::ObjectiveC => "Objective-C",
            Language::Perl => "Perl",
            Language::Lua => "Lua",
            Language::Bash => "Bash",
            Language::PowerShell => "PowerShell",
            Language::R => "R",
            Language::Julia => "Julia",
            Language::Dart => "Dart",
            Language::Elixir => "Elixir",
            Language::Clojure => "Clojure",
            Language::Haskell => "Haskell",
            Language::OCaml => "OCaml",
            Language::FSharp => "F#",
            Language::Zig => "Zig",
            Language::Nim => "Nim",
            Language::Erlang => "Erlang",
            Language::Groovy => "Groovy",
            Language::Sql => "SQL",
        };
        write!(f, "{}", name)
    }
}

/// Error returned when parsing an invalid language name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseLanguageError {
    input: String,
}

impl fmt::Display for ParseLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown language: '{}'", self.input)
    }
}

impl std::error::Error for ParseLanguageError {}

impl FromStr for Language {
    type Err = ParseLanguageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "rust" => Ok(Language::Rust),
            "python" | "py" => Ok(Language::Python),
            "javascript" | "js" => Ok(Language::JavaScript),
            "typescript" | "ts" => Ok(Language::TypeScript),
            "go" | "golang" => Ok(Language::Go),
            "java" => Ok(Language::Java),
            "csharp" | "c#" | "cs" => Ok(Language::CSharp),
            "cpp" | "c++" | "cplusplus" => Ok(Language::Cpp),
            "c" => Ok(Language::C),
            "ruby" | "rb" => Ok(Language::Ruby),
            "php" => Ok(Language::Php),
            "swift" => Ok(Language::Swift),
            "kotlin" | "kt" => Ok(Language::Kotlin),
            "scala" => Ok(Language::Scala),
            "objectivec" | "objective-c" | "objc" => Ok(Language::ObjectiveC),
            "perl" | "pl" => Ok(Language::Perl),
            "lua" => Ok(Language::Lua),
            "bash" | "shell" | "sh" => Ok(Language::Bash),
            "powershell" | "pwsh" | "ps1" => Ok(Language::PowerShell),
            "r" => Ok(Language::R),
            "julia" | "jl" => Ok(Language::Julia),
            "dart" => Ok(Language::Dart),
            "elixir" | "ex" => Ok(Language::Elixir),
            "clojure" | "clj" => Ok(Language::Clojure),
            "haskell" | "hs" => Ok(Language::Haskell),
            "ocaml" | "ml" => Ok(Language::OCaml),
            "fsharp" | "f#" | "fs" => Ok(Language::FSharp),
            "zig" => Ok(Language::Zig),
            "nim" => Ok(Language::Nim),
            "erlang" | "erl" => Ok(Language::Erlang),
            "groovy" => Ok(Language::Groovy),
            "sql" => Ok(Language::Sql),
            _ => Err(ParseLanguageError {
                input: s.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Language::all() tests
    // ============================================================

    #[test]
    fn test_all_returns_at_least_32_languages() {
        let all = Language::all();
        assert!(
            all.len() >= 32,
            "Expected at least 32 languages, got {}",
            all.len()
        );
    }

    #[test]
    fn test_all_contains_major_languages() {
        let all = Language::all();
        // These are the primary languages that must be supported
        let required = [
            Language::Rust,
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::Go,
            Language::Java,
            Language::CSharp,
            Language::Cpp,
            Language::C,
            Language::Ruby,
            Language::Php,
            Language::Swift,
            Language::Kotlin,
            Language::Scala,
        ];

        for lang in required {
            assert!(
                all.contains(&lang),
                "Language::all() should contain {:?}",
                lang
            );
        }
    }

    #[test]
    fn test_all_contains_no_duplicates() {
        let all = Language::all();
        let mut seen = std::collections::HashSet::new();
        for lang in all {
            assert!(
                seen.insert(lang),
                "Language::all() contains duplicate: {:?}",
                lang
            );
        }
    }

    // ============================================================
    // extensions() tests
    // ============================================================

    #[test]
    fn test_rust_extensions() {
        let exts = Language::Rust.extensions();
        assert!(exts.contains(&".rs"), "Rust should have .rs extension");
    }

    #[test]
    fn test_python_extensions() {
        let exts = Language::Python.extensions();
        assert!(exts.contains(&".py"), "Python should have .py extension");
        assert!(exts.contains(&".pyi"), "Python should have .pyi extension");
    }

    #[test]
    fn test_javascript_extensions() {
        let exts = Language::JavaScript.extensions();
        assert!(
            exts.contains(&".js"),
            "JavaScript should have .js extension"
        );
        assert!(
            exts.contains(&".jsx"),
            "JavaScript should have .jsx extension"
        );
        assert!(
            exts.contains(&".mjs"),
            "JavaScript should have .mjs extension"
        );
    }

    #[test]
    fn test_typescript_extensions() {
        let exts = Language::TypeScript.extensions();
        assert!(
            exts.contains(&".ts"),
            "TypeScript should have .ts extension"
        );
        assert!(
            exts.contains(&".tsx"),
            "TypeScript should have .tsx extension"
        );
    }

    #[test]
    fn test_go_extensions() {
        let exts = Language::Go.extensions();
        assert!(exts.contains(&".go"), "Go should have .go extension");
    }

    #[test]
    fn test_java_extensions() {
        let exts = Language::Java.extensions();
        assert!(exts.contains(&".java"), "Java should have .java extension");
    }

    #[test]
    fn test_csharp_extensions() {
        let exts = Language::CSharp.extensions();
        assert!(exts.contains(&".cs"), "C# should have .cs extension");
    }

    #[test]
    fn test_cpp_extensions() {
        let exts = Language::Cpp.extensions();
        assert!(exts.contains(&".cpp"), "C++ should have .cpp extension");
        assert!(exts.contains(&".cc"), "C++ should have .cc extension");
        assert!(exts.contains(&".cxx"), "C++ should have .cxx extension");
        assert!(exts.contains(&".hpp"), "C++ should have .hpp extension");
    }

    #[test]
    fn test_c_extensions() {
        let exts = Language::C.extensions();
        assert!(exts.contains(&".c"), "C should have .c extension");
        assert!(exts.contains(&".h"), "C should have .h extension");
    }

    #[test]
    fn test_ruby_extensions() {
        let exts = Language::Ruby.extensions();
        assert!(exts.contains(&".rb"), "Ruby should have .rb extension");
        assert!(exts.contains(&".rake"), "Ruby should have .rake extension");
    }

    #[test]
    fn test_php_extensions() {
        let exts = Language::Php.extensions();
        assert!(exts.contains(&".php"), "PHP should have .php extension");
    }

    #[test]
    fn test_swift_extensions() {
        let exts = Language::Swift.extensions();
        assert!(
            exts.contains(&".swift"),
            "Swift should have .swift extension"
        );
    }

    #[test]
    fn test_kotlin_extensions() {
        let exts = Language::Kotlin.extensions();
        assert!(exts.contains(&".kt"), "Kotlin should have .kt extension");
        assert!(exts.contains(&".kts"), "Kotlin should have .kts extension");
    }

    #[test]
    fn test_scala_extensions() {
        let exts = Language::Scala.extensions();
        assert!(
            exts.contains(&".scala"),
            "Scala should have .scala extension"
        );
    }

    #[test]
    fn test_all_extensions_start_with_dot() {
        for lang in Language::all() {
            for ext in lang.extensions() {
                assert!(
                    ext.starts_with('.'),
                    "Extension {:?} for {:?} should start with '.'",
                    ext,
                    lang
                );
            }
        }
    }

    #[test]
    fn test_no_empty_extensions() {
        for lang in Language::all() {
            assert!(
                !lang.extensions().is_empty(),
                "Language {:?} should have at least one extension",
                lang
            );
        }
    }

    // ============================================================
    // manifest_files() tests
    // ============================================================

    #[test]
    fn test_rust_manifests() {
        let manifests = Language::Rust.manifest_files();
        assert!(
            manifests.contains(&"Cargo.toml"),
            "Rust should have Cargo.toml"
        );
    }

    #[test]
    fn test_python_manifests() {
        let manifests = Language::Python.manifest_files();
        assert!(
            manifests.contains(&"pyproject.toml"),
            "Python should have pyproject.toml"
        );
        assert!(
            manifests.contains(&"setup.py"),
            "Python should have setup.py"
        );
        assert!(
            manifests.contains(&"requirements.txt"),
            "Python should have requirements.txt"
        );
    }

    #[test]
    fn test_javascript_manifests() {
        let manifests = Language::JavaScript.manifest_files();
        assert!(
            manifests.contains(&"package.json"),
            "JavaScript should have package.json"
        );
    }

    #[test]
    fn test_typescript_manifests() {
        let manifests = Language::TypeScript.manifest_files();
        assert!(
            manifests.contains(&"tsconfig.json"),
            "TypeScript should have tsconfig.json"
        );
        assert!(
            manifests.contains(&"package.json"),
            "TypeScript should have package.json"
        );
    }

    #[test]
    fn test_go_manifests() {
        let manifests = Language::Go.manifest_files();
        assert!(manifests.contains(&"go.mod"), "Go should have go.mod");
    }

    #[test]
    fn test_java_manifests() {
        let manifests = Language::Java.manifest_files();
        assert!(manifests.contains(&"pom.xml"), "Java should have pom.xml");
        assert!(
            manifests.contains(&"build.gradle"),
            "Java should have build.gradle"
        );
    }

    #[test]
    fn test_csharp_manifests() {
        let manifests = Language::CSharp.manifest_files();
        // C# uses glob patterns like *.csproj
        assert!(
            manifests.iter().any(|m| m.contains("csproj")),
            "C# should have csproj manifest"
        );
    }

    #[test]
    fn test_ruby_manifests() {
        let manifests = Language::Ruby.manifest_files();
        assert!(manifests.contains(&"Gemfile"), "Ruby should have Gemfile");
    }

    #[test]
    fn test_php_manifests() {
        let manifests = Language::Php.manifest_files();
        assert!(
            manifests.contains(&"composer.json"),
            "PHP should have composer.json"
        );
    }

    #[test]
    fn test_swift_manifests() {
        let manifests = Language::Swift.manifest_files();
        assert!(
            manifests.contains(&"Package.swift"),
            "Swift should have Package.swift"
        );
    }

    #[test]
    fn test_kotlin_manifests() {
        let manifests = Language::Kotlin.manifest_files();
        assert!(
            manifests.contains(&"build.gradle.kts"),
            "Kotlin should have build.gradle.kts"
        );
    }

    #[test]
    fn test_scala_manifests() {
        let manifests = Language::Scala.manifest_files();
        assert!(
            manifests.contains(&"build.sbt"),
            "Scala should have build.sbt"
        );
    }

    #[test]
    fn test_no_empty_manifests() {
        for lang in Language::all() {
            assert!(
                !lang.manifest_files().is_empty(),
                "Language {:?} should have at least one manifest file",
                lang
            );
        }
    }

    // ============================================================
    // Display trait tests
    // ============================================================

    #[test]
    fn test_display_rust() {
        assert_eq!(format!("{}", Language::Rust), "Rust");
    }

    #[test]
    fn test_display_python() {
        assert_eq!(format!("{}", Language::Python), "Python");
    }

    #[test]
    fn test_display_javascript() {
        assert_eq!(format!("{}", Language::JavaScript), "JavaScript");
    }

    #[test]
    fn test_display_typescript() {
        assert_eq!(format!("{}", Language::TypeScript), "TypeScript");
    }

    #[test]
    fn test_display_csharp() {
        assert_eq!(format!("{}", Language::CSharp), "C#");
    }

    #[test]
    fn test_display_cpp() {
        assert_eq!(format!("{}", Language::Cpp), "C++");
    }

    #[test]
    fn test_display_all_non_empty() {
        for lang in Language::all() {
            let display = format!("{}", lang);
            assert!(
                !display.is_empty(),
                "Display for {:?} should not be empty",
                lang
            );
        }
    }

    // ============================================================
    // FromStr trait tests
    // ============================================================

    #[test]
    fn test_fromstr_rust_lowercase() {
        let lang: Language = "rust".parse().unwrap();
        assert_eq!(lang, Language::Rust);
    }

    #[test]
    fn test_fromstr_rust_titlecase() {
        let lang: Language = "Rust".parse().unwrap();
        assert_eq!(lang, Language::Rust);
    }

    #[test]
    fn test_fromstr_rust_uppercase() {
        let lang: Language = "RUST".parse().unwrap();
        assert_eq!(lang, Language::Rust);
    }

    #[test]
    fn test_fromstr_python() {
        let lang: Language = "python".parse().unwrap();
        assert_eq!(lang, Language::Python);
    }

    #[test]
    fn test_fromstr_javascript() {
        let lang: Language = "javascript".parse().unwrap();
        assert_eq!(lang, Language::JavaScript);
    }

    #[test]
    fn test_fromstr_typescript() {
        let lang: Language = "typescript".parse().unwrap();
        assert_eq!(lang, Language::TypeScript);
    }

    #[test]
    fn test_fromstr_csharp_various() {
        // C# can be written many ways
        assert_eq!("csharp".parse::<Language>().unwrap(), Language::CSharp);
        assert_eq!("c#".parse::<Language>().unwrap(), Language::CSharp);
        assert_eq!("cs".parse::<Language>().unwrap(), Language::CSharp);
    }

    #[test]
    fn test_fromstr_cpp_various() {
        // C++ can be written many ways
        assert_eq!("cpp".parse::<Language>().unwrap(), Language::Cpp);
        assert_eq!("c++".parse::<Language>().unwrap(), Language::Cpp);
    }

    #[test]
    fn test_fromstr_unknown_language_error() {
        let result = "unknown_lang_xyz".parse::<Language>();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown_lang_xyz"));
    }

    #[test]
    fn test_fromstr_empty_string_error() {
        let result = "".parse::<Language>();
        assert!(result.is_err());
    }

    #[test]
    fn test_fromstr_roundtrip() {
        // Every language should roundtrip through Display and FromStr
        for lang in Language::all() {
            let display = format!("{}", lang);
            let parsed: Language = display.parse().unwrap_or_else(|_| {
                panic!(
                    "Should be able to parse Display output '{}' for {:?}",
                    display, lang
                )
            });
            assert_eq!(
                *lang, parsed,
                "Roundtrip failed for {:?}: '{}' parsed to {:?}",
                lang, display, parsed
            );
        }
    }

    // ============================================================
    // Edge cases and integration tests
    // ============================================================

    #[test]
    fn test_language_is_copy() {
        // Ensure Language is Copy (cheap to pass around)
        let lang = Language::Rust;
        let lang2 = lang; // Copy
        assert_eq!(lang, lang2);
    }

    #[test]
    fn test_language_is_hashable() {
        // Ensure Language can be used as HashMap key
        use std::collections::HashMap;
        let mut map: HashMap<Language, u32> = HashMap::new();
        map.insert(Language::Rust, 1);
        map.insert(Language::Python, 2);
        assert_eq!(map.get(&Language::Rust), Some(&1));
        assert_eq!(map.get(&Language::Python), Some(&2));
    }

    #[test]
    fn test_parse_language_error_display() {
        let err = ParseLanguageError {
            input: "foobar".to_string(),
        };
        assert_eq!(err.to_string(), "unknown language: 'foobar'");
    }
}
