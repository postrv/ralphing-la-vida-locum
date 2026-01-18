//! CCG constraint loader.
//!
//! This module loads CCG constraints from project configuration files.
//! Constraints are typically stored in `.ccg/constraints.json` and define
//! rules that code must follow.
//!
//! # Example
//!
//! ```rust
//! use ralph::narsil::ConstraintLoader;
//! use std::path::Path;
//!
//! let loader = ConstraintLoader::new(Path::new("."));
//! let constraints = loader.load().unwrap();
//! println!("Loaded {} constraints", constraints.len());
//! ```

use crate::narsil::{ConstraintSet, ConstraintSeverity};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Default path for CCG constraints relative to project root.
pub const DEFAULT_CONSTRAINTS_PATH: &str = ".ccg/constraints.json";

/// Errors that can occur when loading constraints.
#[derive(Debug, Error)]
pub enum ConstraintLoadError {
    /// IO error reading the constraints file.
    #[error("Failed to read constraints file: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON parsing error.
    #[error("Failed to parse constraints JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    /// Validation error in constraints.
    #[error("Constraint validation failed: {0}")]
    ValidationError(String),
}

/// Loader for CCG constraints from project configuration.
///
/// The loader looks for constraints in `.ccg/constraints.json` by default,
/// but a custom path can be specified.
#[derive(Debug)]
pub struct ConstraintLoader {
    /// Project root directory.
    project_dir: PathBuf,

    /// Custom path to constraints file (relative to project root).
    custom_path: Option<PathBuf>,
}

impl ConstraintLoader {
    /// Create a new constraint loader for the given project directory.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::narsil::ConstraintLoader;
    /// use std::path::Path;
    ///
    /// let loader = ConstraintLoader::new(Path::new("/path/to/project"));
    /// ```
    #[must_use]
    pub fn new(project_dir: impl AsRef<Path>) -> Self {
        Self {
            project_dir: project_dir.as_ref().to_path_buf(),
            custom_path: None,
        }
    }

    /// Set a custom path for the constraints file.
    ///
    /// The path should be relative to the project root.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::narsil::ConstraintLoader;
    /// use std::path::Path;
    ///
    /// let loader = ConstraintLoader::new(Path::new("."))
    ///     .with_custom_path("config/constraints.json");
    /// ```
    #[must_use]
    pub fn with_custom_path(mut self, path: impl AsRef<Path>) -> Self {
        self.custom_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Get the path to the constraints file.
    #[must_use]
    pub fn constraints_path(&self) -> PathBuf {
        let relative = self
            .custom_path
            .as_deref()
            .unwrap_or(Path::new(DEFAULT_CONSTRAINTS_PATH));
        self.project_dir.join(relative)
    }

    /// Check if a constraints file exists.
    #[must_use]
    pub fn has_constraints(&self) -> bool {
        self.constraints_path().exists()
    }

    /// Load constraints from the project configuration.
    ///
    /// Returns an empty `ConstraintSet` if no constraints file exists.
    /// Returns an error if the file exists but cannot be parsed.
    ///
    /// # Errors
    ///
    /// Returns `ConstraintLoadError::IoError` if the file cannot be read.
    /// Returns `ConstraintLoadError::ParseError` if the JSON is invalid.
    /// Returns `ConstraintLoadError::ValidationError` if constraints are invalid.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ralph::narsil::ConstraintLoader;
    /// use std::path::Path;
    ///
    /// let loader = ConstraintLoader::new(Path::new("."));
    /// match loader.load() {
    ///     Ok(constraints) => println!("Loaded {} constraints", constraints.len()),
    ///     Err(e) => eprintln!("Failed to load: {}", e),
    /// }
    /// ```
    pub fn load(&self) -> Result<ConstraintSet, ConstraintLoadError> {
        let path = self.constraints_path();

        if !path.exists() {
            return Ok(ConstraintSet::new());
        }

        let content = std::fs::read_to_string(&path)?;
        let constraints = ConstraintSet::from_json(&content)?;

        // Validate constraints
        let errors = constraints.validate();
        if !errors.is_empty() {
            return Err(ConstraintLoadError::ValidationError(errors.join("; ")));
        }

        Ok(constraints)
    }

    /// Load constraints and filter to only blocking ones (error/critical severity).
    ///
    /// # Errors
    ///
    /// Same errors as `load()`.
    pub fn load_blocking(&self) -> Result<ConstraintSet, ConstraintLoadError> {
        let all = self.load()?;

        let blocking: Vec<_> = all
            .all()
            .iter()
            .filter(|c| c.severity >= ConstraintSeverity::Error)
            .cloned()
            .collect();

        let mut result = ConstraintSet::new();
        for c in blocking {
            result.add(c);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // =========================================================================
    // ConstraintLoader Construction Tests
    // =========================================================================

    #[test]
    fn test_constraint_loader_new() {
        let loader = ConstraintLoader::new("/path/to/project");
        assert_eq!(loader.project_dir, PathBuf::from("/path/to/project"));
        assert!(loader.custom_path.is_none());
    }

    #[test]
    fn test_constraint_loader_with_custom_path() {
        let loader = ConstraintLoader::new(".").with_custom_path("config/my-constraints.json");

        assert_eq!(
            loader.custom_path,
            Some(PathBuf::from("config/my-constraints.json"))
        );
    }

    #[test]
    fn test_constraint_loader_constraints_path_default() {
        let loader = ConstraintLoader::new("/project");
        assert_eq!(
            loader.constraints_path(),
            PathBuf::from("/project/.ccg/constraints.json")
        );
    }

    #[test]
    fn test_constraint_loader_constraints_path_custom() {
        let loader = ConstraintLoader::new("/project").with_custom_path("config/rules.json");

        assert_eq!(
            loader.constraints_path(),
            PathBuf::from("/project/config/rules.json")
        );
    }

    // =========================================================================
    // ConstraintLoader has_constraints Tests
    // =========================================================================

    #[test]
    fn test_constraint_loader_has_constraints_false_when_missing() {
        let dir = tempdir().unwrap();
        let loader = ConstraintLoader::new(dir.path());
        assert!(!loader.has_constraints());
    }

    #[test]
    fn test_constraint_loader_has_constraints_true_when_exists() {
        let dir = tempdir().unwrap();

        // Create the .ccg directory and constraints file
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();
        fs::write(ccg_dir.join("constraints.json"), "{}").unwrap();

        let loader = ConstraintLoader::new(dir.path());
        assert!(loader.has_constraints());
    }

    // =========================================================================
    // ConstraintLoader load Tests
    // =========================================================================

    #[test]
    fn test_constraint_loader_load_empty_when_file_missing() {
        let dir = tempdir().unwrap();
        let loader = ConstraintLoader::new(dir.path());

        let result = loader.load();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_constraint_loader_load_parses_valid_json() {
        let dir = tempdir().unwrap();
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();

        let json = r#"{
            "constraints": [
                {
                    "id": "max-complexity",
                    "kind": "maxComplexity",
                    "description": "Keep functions simple",
                    "severity": "warning",
                    "targets": [],
                    "value": 10,
                    "enabled": true
                }
            ]
        }"#;
        fs::write(ccg_dir.join("constraints.json"), json).unwrap();

        let loader = ConstraintLoader::new(dir.path());
        let result = loader.load();

        assert!(result.is_ok());
        let constraints = result.unwrap();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints.all()[0].id, "max-complexity");
    }

    #[test]
    fn test_constraint_loader_load_multiple_constraints() {
        let dir = tempdir().unwrap();
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();

        let json = r#"{
            "constraints": [
                {
                    "id": "max-complexity",
                    "kind": "maxComplexity",
                    "description": "Keep functions simple",
                    "severity": "warning",
                    "targets": [],
                    "value": 10,
                    "enabled": true
                },
                {
                    "id": "max-lines",
                    "kind": "maxLines",
                    "description": "Keep functions short",
                    "severity": "error",
                    "targets": ["core::*"],
                    "value": 100,
                    "enabled": true
                },
                {
                    "id": "no-direct-db",
                    "kind": "noDirectCalls",
                    "description": "Use repository pattern",
                    "severity": "critical",
                    "targets": ["api::*"],
                    "enabled": true
                }
            ]
        }"#;
        fs::write(ccg_dir.join("constraints.json"), json).unwrap();

        let loader = ConstraintLoader::new(dir.path());
        let result = loader.load();

        assert!(result.is_ok());
        let constraints = result.unwrap();
        assert_eq!(constraints.len(), 3);
    }

    #[test]
    fn test_constraint_loader_load_error_on_invalid_json() {
        let dir = tempdir().unwrap();
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();
        fs::write(ccg_dir.join("constraints.json"), "{ invalid json }").unwrap();

        let loader = ConstraintLoader::new(dir.path());
        let result = loader.load();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConstraintLoadError::ParseError(_)
        ));
    }

    #[test]
    fn test_constraint_loader_load_error_on_invalid_constraint() {
        let dir = tempdir().unwrap();
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();

        // maxComplexity requires a numeric value
        let json = r#"{
            "constraints": [
                {
                    "id": "bad-constraint",
                    "kind": "maxComplexity",
                    "description": "Missing value",
                    "severity": "warning",
                    "targets": [],
                    "enabled": true
                }
            ]
        }"#;
        fs::write(ccg_dir.join("constraints.json"), json).unwrap();

        let loader = ConstraintLoader::new(dir.path());
        let result = loader.load();

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConstraintLoadError::ValidationError(_)
        ));
    }

    #[test]
    fn test_constraint_loader_load_custom_path() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();

        let json = r#"{
            "constraints": [
                {
                    "id": "custom",
                    "kind": "requireDocs",
                    "description": "Require docs",
                    "severity": "info",
                    "targets": [],
                    "enabled": true
                }
            ]
        }"#;
        fs::write(config_dir.join("rules.json"), json).unwrap();

        let loader = ConstraintLoader::new(dir.path()).with_custom_path("config/rules.json");

        let result = loader.load();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    // =========================================================================
    // ConstraintLoader load_blocking Tests
    // =========================================================================

    #[test]
    fn test_constraint_loader_load_blocking_filters_by_severity() {
        let dir = tempdir().unwrap();
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();

        let json = r#"{
            "constraints": [
                {
                    "id": "info-constraint",
                    "kind": "requireDocs",
                    "description": "Info level",
                    "severity": "info",
                    "targets": [],
                    "enabled": true
                },
                {
                    "id": "warning-constraint",
                    "kind": "maxComplexity",
                    "description": "Warning level",
                    "severity": "warning",
                    "targets": [],
                    "value": 10,
                    "enabled": true
                },
                {
                    "id": "error-constraint",
                    "kind": "maxLines",
                    "description": "Error level",
                    "severity": "error",
                    "targets": [],
                    "value": 100,
                    "enabled": true
                },
                {
                    "id": "critical-constraint",
                    "kind": "noDirectCalls",
                    "description": "Critical level",
                    "severity": "critical",
                    "targets": [],
                    "enabled": true
                }
            ]
        }"#;
        fs::write(ccg_dir.join("constraints.json"), json).unwrap();

        let loader = ConstraintLoader::new(dir.path());
        let result = loader.load_blocking();

        assert!(result.is_ok());
        let blocking = result.unwrap();

        // Should only include error and critical (not info or warning)
        assert_eq!(blocking.len(), 2);

        let ids: Vec<_> = blocking.all().iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"error-constraint"));
        assert!(ids.contains(&"critical-constraint"));
        assert!(!ids.contains(&"info-constraint"));
        assert!(!ids.contains(&"warning-constraint"));
    }

    #[test]
    fn test_constraint_loader_load_blocking_empty_when_no_blocking() {
        let dir = tempdir().unwrap();
        let ccg_dir = dir.path().join(".ccg");
        fs::create_dir_all(&ccg_dir).unwrap();

        let json = r#"{
            "constraints": [
                {
                    "id": "warning-only",
                    "kind": "maxComplexity",
                    "description": "Just a warning",
                    "severity": "warning",
                    "targets": [],
                    "value": 10,
                    "enabled": true
                }
            ]
        }"#;
        fs::write(ccg_dir.join("constraints.json"), json).unwrap();

        let loader = ConstraintLoader::new(dir.path());
        let result = loader.load_blocking();

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // =========================================================================
    // ConstraintLoadError Tests
    // =========================================================================

    #[test]
    fn test_constraint_load_error_display() {
        let io_err = ConstraintLoadError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("Failed to read"));

        let parse_err = ConstraintLoadError::ParseError(
            serde_json::from_str::<serde_json::Value>("invalid").unwrap_err(),
        );
        assert!(parse_err.to_string().contains("Failed to parse"));

        let validation_err = ConstraintLoadError::ValidationError("missing value".to_string());
        assert!(validation_err.to_string().contains("validation failed"));
    }
}
