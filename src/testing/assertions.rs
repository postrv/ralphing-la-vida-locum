//! Custom assertions for domain-specific testing.
//!
//! Provides expressive assertions for testing Ralph-specific behavior.

use super::traits::QualityGateResult;

/// Assert that a quality gate result passed.
///
/// # Panics
///
/// Panics with a descriptive message if the gate failed.
///
/// # Example
///
/// ```rust,ignore
/// let result = checker.run_clippy().unwrap();
/// assert_gate_passed(&result);
/// ```
pub fn assert_gate_passed(result: &QualityGateResult) {
    assert!(
        result.passed,
        "Expected quality gate to pass, but it failed.\nWarnings: {:?}\nFailures: {:?}",
        result.warnings,
        result.failures
    );
}

/// Assert that a quality gate result failed.
///
/// # Panics
///
/// Panics if the gate passed when it should have failed.
///
/// # Example
///
/// ```rust,ignore
/// let result = checker.run_clippy().unwrap();
/// assert_gate_failed(&result);
/// ```
pub fn assert_gate_failed(result: &QualityGateResult) {
    assert!(
        !result.passed,
        "Expected quality gate to fail, but it passed."
    );
}

/// Assert that a quality gate has specific warnings.
///
/// # Panics
///
/// Panics if the expected warning count doesn't match.
///
/// # Example
///
/// ```rust,ignore
/// let result = checker.run_clippy().unwrap();
/// assert_warning_count(&result, 3);
/// ```
pub fn assert_warning_count(result: &QualityGateResult, expected: usize) {
    assert_eq!(
        result.warnings.len(),
        expected,
        "Expected {} warnings, but got {}.\nWarnings: {:?}",
        expected,
        result.warnings.len(),
        result.warnings
    );
}

/// Assert that a quality gate has specific failures.
///
/// # Panics
///
/// Panics if the expected failure count doesn't match.
///
/// # Example
///
/// ```rust,ignore
/// let result = checker.run_tests().unwrap();
/// assert_failure_count(&result, 2);
/// ```
pub fn assert_failure_count(result: &QualityGateResult, expected: usize) {
    assert_eq!(
        result.failures.len(),
        expected,
        "Expected {} failures, but got {}.\nFailures: {:?}",
        expected,
        result.failures.len(),
        result.failures
    );
}

/// Assert that warnings contain a specific substring.
///
/// # Panics
///
/// Panics if no warning contains the expected substring.
///
/// # Example
///
/// ```rust,ignore
/// let result = checker.run_clippy().unwrap();
/// assert_warning_contains(&result, "unused variable");
/// ```
pub fn assert_warning_contains(result: &QualityGateResult, substring: &str) {
    let found = result.warnings.iter().any(|w| w.contains(substring));
    assert!(
        found,
        "Expected a warning containing '{}', but none found.\nWarnings: {:?}",
        substring,
        result.warnings
    );
}

/// Assert that failures contain a specific substring.
///
/// # Panics
///
/// Panics if no failure contains the expected substring.
///
/// # Example
///
/// ```rust,ignore
/// let result = checker.run_tests().unwrap();
/// assert_failure_contains(&result, "test_something");
/// ```
pub fn assert_failure_contains(result: &QualityGateResult, substring: &str) {
    let found = result.failures.iter().any(|f| f.contains(substring));
    assert!(
        found,
        "Expected a failure containing '{}', but none found.\nFailures: {:?}",
        substring,
        result.failures
    );
}

/// Assert that progress was made (commits or plan changes).
///
/// # Panics
///
/// Panics if no progress was detected.
pub fn assert_progress_made(commits_since: u32, plan_changed: bool) {
    assert!(
        commits_since > 0 || plan_changed,
        "Expected progress (commits: {}, plan_changed: {}), but none detected.",
        commits_since,
        plan_changed
    );
}

/// Assert that no progress was made.
///
/// # Panics
///
/// Panics if progress was detected.
pub fn assert_no_progress(commits_since: u32, plan_changed: bool) {
    assert!(
        commits_since == 0 && !plan_changed,
        "Expected no progress, but detected commits: {}, plan_changed: {}",
        commits_since,
        plan_changed
    );
}

/// Assert that iteration count is within bounds.
///
/// # Panics
///
/// Panics if iteration is outside the expected range.
pub fn assert_iteration_in_range(iteration: u32, min: u32, max: u32) {
    assert!(
        iteration >= min && iteration <= max,
        "Expected iteration {} to be in range [{}, {}]",
        iteration,
        min,
        max
    );
}

/// Assert that stagnation count matches expected.
///
/// # Panics
///
/// Panics if stagnation count doesn't match.
pub fn assert_stagnation_count(actual: u32, expected: u32) {
    assert_eq!(
        actual, expected,
        "Expected stagnation count {}, but got {}",
        expected, actual
    );
}

/// Assert that stagnation is below threshold.
///
/// # Panics
///
/// Panics if stagnation meets or exceeds threshold.
pub fn assert_not_stagnating(stagnation_count: u32, threshold: u32) {
    assert!(
        stagnation_count < threshold,
        "Expected not stagnating, but stagnation {} >= threshold {}",
        stagnation_count,
        threshold
    );
}

/// Assert that stagnation has been triggered.
///
/// # Panics
///
/// Panics if stagnation hasn't been triggered.
pub fn assert_stagnating(stagnation_count: u32, threshold: u32) {
    assert!(
        stagnation_count >= threshold,
        "Expected stagnating, but stagnation {} < threshold {}",
        stagnation_count,
        threshold
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_gate_passed_succeeds() {
        let result = QualityGateResult::pass();
        assert_gate_passed(&result); // Should not panic
    }

    #[test]
    #[should_panic(expected = "Expected quality gate to pass")]
    fn test_assert_gate_passed_fails() {
        let result = QualityGateResult::fail_with_warnings(vec!["warning".to_string()]);
        assert_gate_passed(&result);
    }

    #[test]
    fn test_assert_gate_failed_succeeds() {
        let result = QualityGateResult::fail_with_warnings(vec!["warning".to_string()]);
        assert_gate_failed(&result); // Should not panic
    }

    #[test]
    #[should_panic(expected = "Expected quality gate to fail")]
    fn test_assert_gate_failed_fails() {
        let result = QualityGateResult::pass();
        assert_gate_failed(&result);
    }

    #[test]
    fn test_assert_warning_count_succeeds() {
        let result = QualityGateResult::fail_with_warnings(vec![
            "warning 1".to_string(),
            "warning 2".to_string(),
        ]);
        assert_warning_count(&result, 2);
    }

    #[test]
    #[should_panic(expected = "Expected 3 warnings")]
    fn test_assert_warning_count_fails() {
        let result = QualityGateResult::fail_with_warnings(vec!["warning".to_string()]);
        assert_warning_count(&result, 3);
    }

    #[test]
    fn test_assert_failure_count_succeeds() {
        let result =
            QualityGateResult::fail_with_failures(vec!["test_a".to_string(), "test_b".to_string()]);
        assert_failure_count(&result, 2);
    }

    #[test]
    fn test_assert_warning_contains_succeeds() {
        let result =
            QualityGateResult::fail_with_warnings(vec!["unused variable `x`".to_string()]);
        assert_warning_contains(&result, "unused variable");
    }

    #[test]
    #[should_panic(expected = "Expected a warning containing")]
    fn test_assert_warning_contains_fails() {
        let result = QualityGateResult::fail_with_warnings(vec!["other warning".to_string()]);
        assert_warning_contains(&result, "unused variable");
    }

    #[test]
    fn test_assert_failure_contains_succeeds() {
        let result =
            QualityGateResult::fail_with_failures(vec!["test_database_connection".to_string()]);
        assert_failure_contains(&result, "database");
    }

    #[test]
    fn test_assert_progress_made_with_commits() {
        assert_progress_made(1, false); // Should not panic
    }

    #[test]
    fn test_assert_progress_made_with_plan_change() {
        assert_progress_made(0, true); // Should not panic
    }

    #[test]
    #[should_panic(expected = "Expected progress")]
    fn test_assert_progress_made_fails() {
        assert_progress_made(0, false);
    }

    #[test]
    fn test_assert_no_progress_succeeds() {
        assert_no_progress(0, false); // Should not panic
    }

    #[test]
    #[should_panic(expected = "Expected no progress")]
    fn test_assert_no_progress_fails_with_commits() {
        assert_no_progress(1, false);
    }

    #[test]
    fn test_assert_iteration_in_range_succeeds() {
        assert_iteration_in_range(5, 1, 10);
        assert_iteration_in_range(1, 1, 10); // Lower bound
        assert_iteration_in_range(10, 1, 10); // Upper bound
    }

    #[test]
    #[should_panic(expected = "Expected iteration")]
    fn test_assert_iteration_in_range_fails() {
        assert_iteration_in_range(15, 1, 10);
    }

    #[test]
    fn test_assert_stagnation_count_succeeds() {
        assert_stagnation_count(3, 3);
    }

    #[test]
    fn test_assert_not_stagnating_succeeds() {
        assert_not_stagnating(2, 5);
    }

    #[test]
    #[should_panic(expected = "Expected not stagnating")]
    fn test_assert_not_stagnating_fails() {
        assert_not_stagnating(5, 5);
    }

    #[test]
    fn test_assert_stagnating_succeeds() {
        assert_stagnating(5, 5);
        assert_stagnating(6, 5);
    }

    #[test]
    #[should_panic(expected = "Expected stagnating")]
    fn test_assert_stagnating_fails() {
        assert_stagnating(3, 5);
    }
}
