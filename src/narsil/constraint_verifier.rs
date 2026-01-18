//! Constraint verification module.
//!
//! This module provides verification of code against CCG constraints.
//! It integrates with narsil-mcp for complexity and call graph analysis
//! when available, and gracefully degrades without it.
//!
//! # Example
//!
//! ```rust
//! use ralph::narsil::{ConstraintVerifier, ConstraintSet, CcgConstraint, ConstraintKind, ConstraintValue, FunctionMetrics};
//!
//! let constraints = ConstraintSet::new()
//!     .with_constraint(
//!         CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep functions simple")
//!             .with_value(ConstraintValue::Number(10))
//!     );
//!
//! let verifier = ConstraintVerifier::new(constraints);
//!
//! // Verify a function with known metrics
//! let metrics = FunctionMetrics::new("process_data")
//!     .with_complexity(15);  // Exceeds max of 10
//!
//! let result = verifier.verify_function(&metrics);
//! assert!(!result.compliant);
//! assert_eq!(result.violations.len(), 1);
//! ```

use crate::narsil::{
    CcgConstraint, ComplianceResult, ConstraintKind, ConstraintSet, ConstraintValue,
    ConstraintViolation, NarsilClient,
};

/// Metrics for a function that can be checked against constraints.
///
/// # Example
///
/// ```rust
/// use ralph::narsil::FunctionMetrics;
///
/// let metrics = FunctionMetrics::new("my_function")
///     .with_complexity(5)
///     .with_lines(50)
///     .with_parameters(3)
///     .with_location("src/lib.rs", 42);
/// ```
#[derive(Debug, Clone, Default)]
pub struct FunctionMetrics {
    /// Function name (may include module path).
    pub name: String,

    /// File path where the function is defined.
    pub file: Option<String>,

    /// Line number where the function starts.
    pub line: Option<u32>,

    /// Cyclomatic complexity of the function.
    pub complexity: Option<u32>,

    /// Number of lines in the function body.
    pub lines: Option<u32>,

    /// Number of parameters the function takes.
    pub parameters: Option<u32>,

    /// Functions that this function calls directly.
    pub calls: Vec<String>,
}

impl FunctionMetrics {
    /// Create new metrics for a function.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::narsil::FunctionMetrics;
    ///
    /// let metrics = FunctionMetrics::new("process");
    /// assert_eq!(metrics.name, "process");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the cyclomatic complexity.
    #[must_use]
    pub fn with_complexity(mut self, complexity: u32) -> Self {
        self.complexity = Some(complexity);
        self
    }

    /// Set the number of lines.
    #[must_use]
    pub fn with_lines(mut self, lines: u32) -> Self {
        self.lines = Some(lines);
        self
    }

    /// Set the number of parameters.
    #[must_use]
    pub fn with_parameters(mut self, parameters: u32) -> Self {
        self.parameters = Some(parameters);
        self
    }

    /// Set the file location.
    #[must_use]
    pub fn with_location(mut self, file: impl Into<String>, line: u32) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self
    }

    /// Add a function call.
    #[must_use]
    pub fn with_call(mut self, callee: impl Into<String>) -> Self {
        self.calls.push(callee.into());
        self
    }

    /// Add multiple function calls.
    #[must_use]
    pub fn with_calls(mut self, callees: Vec<String>) -> Self {
        self.calls.extend(callees);
        self
    }
}

/// Verifies code against a set of constraints.
///
/// The verifier checks function metrics against constraints like
/// `MaxComplexity`, `MaxLines`, and `MaxParameters`. When integrated
/// with narsil-mcp, it can also verify architectural constraints
/// like `NoDirectCalls`.
///
/// # Example
///
/// ```rust
/// use ralph::narsil::{ConstraintVerifier, ConstraintSet, CcgConstraint, ConstraintKind, ConstraintValue, FunctionMetrics};
///
/// let constraints = ConstraintSet::new()
///     .with_constraint(
///         CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Keep functions short")
///             .with_value(ConstraintValue::Number(100))
///     );
///
/// let verifier = ConstraintVerifier::new(constraints);
///
/// let metrics = FunctionMetrics::new("long_function")
///     .with_lines(150);
///
/// let result = verifier.verify_function(&metrics);
/// assert!(!result.compliant);
/// ```
pub struct ConstraintVerifier {
    /// The set of constraints to verify against.
    constraints: ConstraintSet,

    /// Optional narsil-mcp client for advanced analysis.
    client: Option<NarsilClient>,
}

impl ConstraintVerifier {
    /// Create a new verifier with the given constraints.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::narsil::{ConstraintVerifier, ConstraintSet};
    ///
    /// let verifier = ConstraintVerifier::new(ConstraintSet::new());
    /// ```
    #[must_use]
    pub fn new(constraints: ConstraintSet) -> Self {
        Self {
            constraints,
            client: None,
        }
    }

    /// Attach a narsil-mcp client for advanced verification.
    ///
    /// When a client is attached, the verifier can perform additional
    /// checks using call graph analysis.
    #[must_use]
    pub fn with_narsil_client(mut self, client: NarsilClient) -> Self {
        self.client = Some(client);
        self
    }

    /// Verify a single function against all applicable constraints.
    ///
    /// Returns a `ComplianceResult` indicating whether the function
    /// complies with all constraints and listing any violations.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::narsil::{ConstraintVerifier, ConstraintSet, CcgConstraint, ConstraintKind, ConstraintValue, FunctionMetrics};
    ///
    /// let constraints = ConstraintSet::new()
    ///     .with_constraint(
    ///         CcgConstraint::new("max-params", ConstraintKind::MaxParameters, "Limit parameters")
    ///             .with_value(ConstraintValue::Number(5))
    ///     );
    ///
    /// let verifier = ConstraintVerifier::new(constraints);
    /// let metrics = FunctionMetrics::new("many_params")
    ///     .with_parameters(8);
    ///
    /// let result = verifier.verify_function(&metrics);
    /// assert!(!result.compliant);
    /// ```
    #[must_use]
    pub fn verify_function(&self, metrics: &FunctionMetrics) -> ComplianceResult {
        let applicable = self.constraints.for_target(&metrics.name);
        let checked_count = applicable.len();
        let mut violations = Vec::new();

        for constraint in &applicable {
            if let Some(violation) = self.check_constraint(constraint, metrics) {
                violations.push(violation);
            }
        }

        if violations.is_empty() {
            ComplianceResult::passed(checked_count)
        } else {
            ComplianceResult::failed(violations, checked_count)
        }
    }

    /// Verify multiple functions at once.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ralph::narsil::{ConstraintVerifier, ConstraintSet, FunctionMetrics};
    ///
    /// let verifier = ConstraintVerifier::new(ConstraintSet::new());
    /// let functions = vec![
    ///     FunctionMetrics::new("func1"),
    ///     FunctionMetrics::new("func2"),
    /// ];
    ///
    /// let result = verifier.verify_functions(&functions);
    /// assert!(result.compliant);
    /// ```
    #[must_use]
    pub fn verify_functions(&self, functions: &[FunctionMetrics]) -> ComplianceResult {
        let mut all_violations = Vec::new();
        let mut total_checked = 0;

        for metrics in functions {
            let result = self.verify_function(metrics);
            total_checked += result.checked_count;
            all_violations.extend(result.violations);
        }

        if all_violations.is_empty() {
            ComplianceResult::passed(total_checked)
        } else {
            ComplianceResult::failed(all_violations, total_checked)
        }
    }

    /// Get the constraint set being used for verification.
    #[must_use]
    pub fn constraints(&self) -> &ConstraintSet {
        &self.constraints
    }

    /// Check if a narsil-mcp client is attached and available.
    ///
    /// When narsil-mcp is available, the verifier can perform additional
    /// checks using call graph and complexity analysis.
    #[must_use]
    pub fn is_narsil_available(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.is_available())
    }

    /// Get the call graph for a function from narsil-mcp.
    ///
    /// Returns the call graph data if narsil-mcp is available and the
    /// function is found, otherwise returns None.
    pub fn get_call_graph(&self, function: &str) -> Option<serde_json::Value> {
        self.client.as_ref()?.get_call_graph(function).ok().flatten()
    }

    /// Check a single constraint against function metrics.
    fn check_constraint(
        &self,
        constraint: &CcgConstraint,
        metrics: &FunctionMetrics,
    ) -> Option<ConstraintViolation> {
        match constraint.kind {
            ConstraintKind::MaxComplexity => self.check_max_complexity(constraint, metrics),
            ConstraintKind::MaxLines => self.check_max_lines(constraint, metrics),
            ConstraintKind::MaxParameters => self.check_max_parameters(constraint, metrics),
            ConstraintKind::NoDirectCalls => self.check_no_direct_calls(constraint, metrics),
            _ => None, // Other constraint types not yet implemented
        }
    }

    /// Check MaxComplexity constraint.
    fn check_max_complexity(
        &self,
        constraint: &CcgConstraint,
        metrics: &FunctionMetrics,
    ) -> Option<ConstraintViolation> {
        let Some(complexity) = metrics.complexity else {
            return None; // Can't verify without metrics
        };

        let Some(ConstraintValue::Number(max)) = &constraint.value else {
            return None; // Invalid constraint
        };

        if complexity > *max {
            let mut violation = ConstraintViolation::new(
                &constraint.id,
                &metrics.name,
                format!(
                    "Complexity {} exceeds maximum of {}",
                    complexity, max
                ),
            )
            .with_suggestion(format!(
                "Refactor into smaller functions to reduce complexity below {}",
                max
            ));

            if let (Some(file), Some(line)) = (&metrics.file, metrics.line) {
                violation = violation.with_location(file, line);
            }

            Some(violation)
        } else {
            None
        }
    }

    /// Check MaxLines constraint.
    fn check_max_lines(
        &self,
        constraint: &CcgConstraint,
        metrics: &FunctionMetrics,
    ) -> Option<ConstraintViolation> {
        let lines = metrics.lines?;

        let Some(ConstraintValue::Number(max)) = &constraint.value else {
            return None;
        };

        if lines > *max {
            let mut violation = ConstraintViolation::new(
                &constraint.id,
                &metrics.name,
                format!("Function has {} lines, maximum is {}", lines, max),
            )
            .with_suggestion(format!(
                "Extract logic into helper functions to reduce below {} lines",
                max
            ));

            if let (Some(file), Some(line)) = (&metrics.file, metrics.line) {
                violation = violation.with_location(file, line);
            }

            Some(violation)
        } else {
            None
        }
    }

    /// Check MaxParameters constraint.
    fn check_max_parameters(
        &self,
        constraint: &CcgConstraint,
        metrics: &FunctionMetrics,
    ) -> Option<ConstraintViolation> {
        let parameters = metrics.parameters?;

        let Some(ConstraintValue::Number(max)) = &constraint.value else {
            return None;
        };

        if parameters > *max {
            let mut violation = ConstraintViolation::new(
                &constraint.id,
                &metrics.name,
                format!(
                    "Function has {} parameters, maximum is {}",
                    parameters, max
                ),
            )
            .with_suggestion("Consider using a struct or builder pattern to group parameters");

            if let (Some(file), Some(line)) = (&metrics.file, metrics.line) {
                violation = violation.with_location(file, line);
            }

            Some(violation)
        } else {
            None
        }
    }

    /// Check NoDirectCalls constraint.
    fn check_no_direct_calls(
        &self,
        constraint: &CcgConstraint,
        metrics: &FunctionMetrics,
    ) -> Option<ConstraintViolation> {
        // Get the list of prohibited callees from constraint value
        let prohibited: Vec<&str> = match &constraint.value {
            Some(ConstraintValue::String(s)) => vec![s.as_str()],
            Some(ConstraintValue::List(items)) => items.iter().map(String::as_str).collect(),
            _ => return None, // No prohibited calls specified
        };

        // Check if any calls match prohibited patterns
        for call in &metrics.calls {
            for prohibited_pattern in &prohibited {
                if Self::matches_pattern(call, prohibited_pattern) {
                    let mut violation = ConstraintViolation::new(
                        &constraint.id,
                        &metrics.name,
                        format!(
                            "Direct call to '{}' is prohibited by constraint",
                            call
                        ),
                    )
                    .with_suggestion(format!(
                        "Use an abstraction layer instead of calling '{}' directly",
                        call
                    ));

                    if let (Some(file), Some(line)) = (&metrics.file, metrics.line) {
                        violation = violation.with_location(file, line);
                    }

                    return Some(violation);
                }
            }
        }

        None
    }

    /// Check if a call matches a prohibited pattern.
    fn matches_pattern(call: &str, pattern: &str) -> bool {
        if pattern.ends_with('*') {
            let prefix = pattern.trim_end_matches('*');
            call.starts_with(prefix)
        } else {
            call == pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // FunctionMetrics Tests
    // =========================================================================

    #[test]
    fn test_function_metrics_new() {
        let metrics = FunctionMetrics::new("process");
        assert_eq!(metrics.name, "process");
        assert!(metrics.complexity.is_none());
        assert!(metrics.lines.is_none());
        assert!(metrics.parameters.is_none());
        assert!(metrics.file.is_none());
        assert!(metrics.line.is_none());
        assert!(metrics.calls.is_empty());
    }

    #[test]
    fn test_function_metrics_builder() {
        let metrics = FunctionMetrics::new("handle_request")
            .with_complexity(15)
            .with_lines(200)
            .with_parameters(6)
            .with_location("src/handler.rs", 42)
            .with_call("db::query")
            .with_call("cache::get");

        assert_eq!(metrics.name, "handle_request");
        assert_eq!(metrics.complexity, Some(15));
        assert_eq!(metrics.lines, Some(200));
        assert_eq!(metrics.parameters, Some(6));
        assert_eq!(metrics.file, Some("src/handler.rs".to_string()));
        assert_eq!(metrics.line, Some(42));
        assert_eq!(metrics.calls, vec!["db::query", "cache::get"]);
    }

    #[test]
    fn test_function_metrics_with_calls_vec() {
        let metrics = FunctionMetrics::new("process")
            .with_calls(vec!["a::func".to_string(), "b::func".to_string()]);

        assert_eq!(metrics.calls.len(), 2);
    }

    // =========================================================================
    // ConstraintVerifier Construction Tests
    // =========================================================================

    #[test]
    fn test_constraint_verifier_new() {
        let constraints = ConstraintSet::new();
        let verifier = ConstraintVerifier::new(constraints);
        assert!(verifier.constraints().is_empty());
    }

    #[test]
    fn test_constraint_verifier_with_constraints() {
        let constraints = ConstraintSet::new()
            .with_constraint(CcgConstraint::new(
                "test",
                ConstraintKind::MaxComplexity,
                "Test constraint",
            ));

        let verifier = ConstraintVerifier::new(constraints);
        assert_eq!(verifier.constraints().len(), 1);
    }

    // =========================================================================
    // MaxComplexity Verification Tests
    // =========================================================================

    #[test]
    fn test_verify_max_complexity_passes_when_under_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("simple_func").with_complexity(5);

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_verify_max_complexity_passes_at_exact_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("edge_func").with_complexity(10);

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant);
    }

    #[test]
    fn test_verify_max_complexity_fails_when_over_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Keep it simple")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("complex_func").with_complexity(15);

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].constraint_id, "max-complexity");
        assert!(result.violations[0].message.contains("15"));
        assert!(result.violations[0].message.contains("10"));
    }

    #[test]
    fn test_verify_max_complexity_includes_location_when_available() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(5)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("func")
            .with_complexity(10)
            .with_location("src/main.rs", 42);

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations[0].file, Some("src/main.rs".to_string()));
        assert_eq!(result.violations[0].line, Some(42));
    }

    #[test]
    fn test_verify_max_complexity_skips_without_metrics() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("func"); // No complexity set

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant); // Can't fail what we can't verify
    }

    // =========================================================================
    // MaxLines Verification Tests
    // =========================================================================

    #[test]
    fn test_verify_max_lines_passes_when_under_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Keep functions short")
                .with_value(ConstraintValue::Number(100)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("short_func").with_lines(50);

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant);
    }

    #[test]
    fn test_verify_max_lines_fails_when_over_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Keep functions short")
                .with_value(ConstraintValue::Number(100)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("long_func").with_lines(150);

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("150"));
        assert!(result.violations[0].message.contains("100"));
    }

    // =========================================================================
    // MaxParameters Verification Tests
    // =========================================================================

    #[test]
    fn test_verify_max_parameters_passes_when_under_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-params", ConstraintKind::MaxParameters, "Limit parameters")
                .with_value(ConstraintValue::Number(5)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("good_func").with_parameters(3);

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant);
    }

    #[test]
    fn test_verify_max_parameters_fails_when_over_limit() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-params", ConstraintKind::MaxParameters, "Limit parameters")
                .with_value(ConstraintValue::Number(5)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("bad_func").with_parameters(8);

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("8"));
        assert!(result.violations[0].message.contains("5"));
    }

    // =========================================================================
    // NoDirectCalls Verification Tests
    // =========================================================================

    #[test]
    fn test_verify_no_direct_calls_passes_when_no_prohibited_calls() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("no-db", ConstraintKind::NoDirectCalls, "Use repository pattern")
                .with_value(ConstraintValue::String("db::query".to_string())),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("handler")
            .with_call("repository::find")
            .with_call("cache::get");

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant);
    }

    #[test]
    fn test_verify_no_direct_calls_fails_with_prohibited_call() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("no-db", ConstraintKind::NoDirectCalls, "Use repository pattern")
                .with_value(ConstraintValue::String("db::query".to_string())),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("bad_handler")
            .with_call("db::query")
            .with_call("cache::get");

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("db::query"));
    }

    #[test]
    fn test_verify_no_direct_calls_with_list_value() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("no-raw-io", ConstraintKind::NoDirectCalls, "Use abstractions")
                .with_value(ConstraintValue::List(vec![
                    "std::fs::read".to_string(),
                    "std::fs::write".to_string(),
                ])),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("bad_handler")
            .with_call("std::fs::read");

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
    }

    #[test]
    fn test_verify_no_direct_calls_with_wildcard() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("no-raw-io", ConstraintKind::NoDirectCalls, "Use abstractions")
                .with_value(ConstraintValue::String("std::fs::*".to_string())),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("handler")
            .with_call("std::fs::read_to_string");

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
    }

    // =========================================================================
    // Multiple Constraints Tests
    // =========================================================================

    #[test]
    fn test_verify_multiple_constraints_all_pass() {
        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                    .with_value(ConstraintValue::Number(10)),
            )
            .with_constraint(
                CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Test")
                    .with_value(ConstraintValue::Number(100)),
            );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("good_func")
            .with_complexity(5)
            .with_lines(50);

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant);
        assert_eq!(result.checked_count, 2);
    }

    #[test]
    fn test_verify_multiple_constraints_some_fail() {
        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                    .with_value(ConstraintValue::Number(10)),
            )
            .with_constraint(
                CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Test")
                    .with_value(ConstraintValue::Number(100)),
            );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("mixed_func")
            .with_complexity(15) // Fails
            .with_lines(50);     // Passes

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].constraint_id, "max-complexity");
    }

    #[test]
    fn test_verify_multiple_constraints_all_fail() {
        let constraints = ConstraintSet::new()
            .with_constraint(
                CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                    .with_value(ConstraintValue::Number(10)),
            )
            .with_constraint(
                CcgConstraint::new("max-lines", ConstraintKind::MaxLines, "Test")
                    .with_value(ConstraintValue::Number(100)),
            );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("bad_func")
            .with_complexity(15)  // Fails
            .with_lines(150);     // Fails

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 2);
    }

    // =========================================================================
    // Targeted Constraints Tests
    // =========================================================================

    #[test]
    fn test_verify_targeted_constraint_applies_to_matching_function() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("core-complexity", ConstraintKind::MaxComplexity, "Core must be simple")
                .with_target("core::*")
                .with_value(ConstraintValue::Number(5)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("core::process")
            .with_complexity(10);

        let result = verifier.verify_function(&metrics);
        assert!(!result.compliant);
    }

    #[test]
    fn test_verify_targeted_constraint_does_not_apply_to_other_functions() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("core-complexity", ConstraintKind::MaxComplexity, "Core must be simple")
                .with_target("core::*")
                .with_value(ConstraintValue::Number(5)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("api::handler")
            .with_complexity(10);

        let result = verifier.verify_function(&metrics);
        assert!(result.compliant); // Constraint doesn't apply
    }

    // =========================================================================
    // verify_functions Tests
    // =========================================================================

    #[test]
    fn test_verify_functions_empty_list() {
        let verifier = ConstraintVerifier::new(ConstraintSet::new());
        let result = verifier.verify_functions(&[]);
        assert!(result.compliant);
    }

    #[test]
    fn test_verify_functions_multiple_all_pass() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let functions = vec![
            FunctionMetrics::new("func1").with_complexity(5),
            FunctionMetrics::new("func2").with_complexity(8),
        ];

        let result = verifier.verify_functions(&functions);
        assert!(result.compliant);
        assert_eq!(result.checked_count, 2);
    }

    #[test]
    fn test_verify_functions_multiple_some_fail() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let functions = vec![
            FunctionMetrics::new("good_func").with_complexity(5),
            FunctionMetrics::new("bad_func").with_complexity(15),
        ];

        let result = verifier.verify_functions(&functions);
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].target, "bad_func");
    }

    // =========================================================================
    // Suggestions Tests
    // =========================================================================

    #[test]
    fn test_violation_includes_suggestion() {
        let constraints = ConstraintSet::new().with_constraint(
            CcgConstraint::new("max-complexity", ConstraintKind::MaxComplexity, "Test")
                .with_value(ConstraintValue::Number(10)),
        );

        let verifier = ConstraintVerifier::new(constraints);
        let metrics = FunctionMetrics::new("complex_func").with_complexity(20);

        let result = verifier.verify_function(&metrics);
        assert!(result.violations[0].suggestion.is_some());
        assert!(result.violations[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("Refactor"));
    }

    // =========================================================================
    // Pattern Matching Tests
    // =========================================================================

    #[test]
    fn test_matches_pattern_exact() {
        assert!(ConstraintVerifier::matches_pattern("db::query", "db::query"));
        assert!(!ConstraintVerifier::matches_pattern("db::query", "db::execute"));
    }

    #[test]
    fn test_matches_pattern_wildcard() {
        assert!(ConstraintVerifier::matches_pattern("std::fs::read", "std::fs::*"));
        assert!(ConstraintVerifier::matches_pattern("std::fs::write", "std::fs::*"));
        assert!(!ConstraintVerifier::matches_pattern("std::io::read", "std::fs::*"));
    }

    #[test]
    fn test_matches_pattern_prefix() {
        assert!(ConstraintVerifier::matches_pattern("core::module::func", "core::*"));
        assert!(!ConstraintVerifier::matches_pattern("api::handler", "core::*"));
    }

    // =========================================================================
    // Narsil Client Integration Tests
    // =========================================================================

    #[test]
    fn test_is_narsil_available_false_without_client() {
        let verifier = ConstraintVerifier::new(ConstraintSet::new());
        assert!(!verifier.is_narsil_available());
    }

    #[test]
    fn test_get_call_graph_none_without_client() {
        let verifier = ConstraintVerifier::new(ConstraintSet::new());
        assert!(verifier.get_call_graph("some_function").is_none());
    }
}
