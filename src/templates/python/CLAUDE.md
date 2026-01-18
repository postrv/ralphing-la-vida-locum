# Project Memory - Python Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: Python Project

This is a Python project. Key conventions and tools:

- **Testing**: pytest with fixtures and parametrize
- **Linting**: ruff (preferred) or flake8
- **Type Checking**: mypy with strict mode
- **Security**: bandit for vulnerability scanning
- **Formatting**: ruff format or black

---

## GIT AUTHENTICATION (CRITICAL)

### Required Setup
Ralph requires `gh` CLI for all GitHub operations. SSH key access is **blocked**.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated, run:
gh auth login
```

### Rules - Non-Negotiable
1. **ALWAYS** use `gh` CLI for GitHub operations
2. **NEVER** attempt SSH key operations (ssh-keygen, ssh-add, etc.)
3. **NEVER** access ~/.ssh/ directory
4. **NEVER** use git@github.com: URLs
5. Use `gh auth status` to verify authentication
6. Use `gh repo clone` instead of `git clone git@github.com:`

---

## PRODUCTION STANDARDS (Non-Negotiable)

### Code Quality - Zero Tolerance Policy

**FORBIDDEN PATTERNS - Never Use:**
```python
# type: ignore              # Fix the type error properly
# noqa                      # Fix the linting issue
# TODO: ...                 # Implement now or don't merge
# FIXME: ...                # Fix now
pass                        # As a placeholder - implement or remove
...                         # As implementation - complete it
```

**REQUIRED PATTERNS - Always Use:**
```python
def function(arg: Type) -> ReturnType:
    """Short description.

    Args:
        arg: Description of argument.

    Returns:
        Description of return value.

    Raises:
        ExceptionType: When this happens.
    """
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing pytest test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run ruff + mypy + bandit
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every public function: at least 1 unit test
- Every class: exercised in integration tests
- Every error path: tested with `pytest.raises`
- Every edge case: empty inputs, boundaries, None values

### Python Quality Tools

Run all checks before commit:
```bash
# Linting (pick one)
ruff check . --fix
flake8 .

# Type checking
mypy .

# Tests
pytest

# Security
bandit -r . -ll
```

If ruff/flake8 warns about:
- `F401` unused import -> Remove the import
- `F841` unused variable -> Remove or use it
- `E501` line too long -> Break the line appropriately

---

## MCP SERVERS

### narsil-mcp (Code Intelligence - Optional)

narsil-mcp is optional. All features gracefully degrade when unavailable.

**Security (run before committing, if available):**
```bash
scan_security           # Find vulnerabilities
find_injection_vulnerabilities  # SQL/XSS/command injection
check_cwe_top25        # CWE Top 25 checks
```

**Context Gathering:**
```bash
get_call_graph <function>   # Function relationships
find_references <symbol>    # Impact analysis
get_dependencies <path>     # Module dependencies
find_similar_code <query>   # Find related code
```

### Graceful Degradation Policy

When narsil-mcp is unavailable:
- Security gates are skipped (log warning)
- Code intelligence returns empty results
- Ralph continues to function normally

---

## QUALITY GATES (Pre-Commit Checklist)

**Start of Task:**
```
[ ] reindex                                    -> narsil-mcp index refreshed
```

**Mandatory (always enforced):**
```
[ ] Tests written BEFORE implementation        -> TDD verified
[ ] ruff check . (or flake8 .)                 -> 0 warnings
[ ] mypy .                                     -> 0 errors
[ ] pytest                                     -> all pass
[ ] bandit -r . -ll                            -> 0 HIGH/CRITICAL
[ ] No # type: ignore without justification
[ ] No TODO/FIXME comments in new code
[ ] All new public functions documented
[ ] All new functions have tests
[ ] gh auth status                             -> authenticated
```

**Optional (if narsil-mcp available):**
```
[ ] scan_security                              -> 0 CRITICAL/HIGH
[ ] find_injection_vulnerabilities             -> 0 findings
```

**End of Task:**
```
[ ] reindex                                    -> narsil-mcp index updated
```

---

## PYTHON-SPECIFIC CONVENTIONS

### Project Structure
```
project/
├── src/
│   └── package/
│       ├── __init__.py
│       ├── module.py
│       └── ...
├── tests/
│   ├── conftest.py       # Shared fixtures
│   ├── test_module.py
│   └── ...
├── pyproject.toml
├── requirements.txt
└── README.md
```

### Imports
```python
# Standard library first
import os
import sys

# Third-party second
import pytest
import requests

# Local imports last
from package import module
```

### Testing with pytest
```python
import pytest

class TestFeature:
    """Tests for feature module."""

    def test_basic_case(self):
        """Test the happy path."""
        result = function_under_test("input")
        assert result == expected

    def test_edge_case(self):
        """Test edge cases."""
        result = function_under_test("")
        assert result is None

    def test_raises_on_invalid(self):
        """Test error handling."""
        with pytest.raises(ValueError, match="invalid"):
            function_under_test(None)

    @pytest.mark.parametrize("input,expected", [
        ("a", 1),
        ("b", 2),
    ])
    def test_parametrized(self, input, expected):
        """Test multiple cases."""
        assert function_under_test(input) == expected
```

---

## QUICK REFERENCE

```bash
# Run tests
pytest

# Run tests with coverage
pytest --cov=src

# Lint and fix
ruff check . --fix

# Type check
mypy .

# Security scan
bandit -r . -ll

# Verify git environment
gh auth status
```
