# Project Memory - PHP Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: PHP Project

This is a PHP project. Key conventions and tools:

- **Testing**: PHPUnit with data providers and mocking
- **Static Analysis**: PHPStan (level max preferred)
- **Coding Standards**: PHP_CodeSniffer (PSR-12)
- **Security**: Composer audit, RIPS (if available)
- **Formatting**: PHP-CS-Fixer

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
```php
// @phpstan-ignore          // Fix the type error properly
// @codeCoverageIgnore      // Write proper tests
// TODO: ...                // Implement now or don't merge
// FIXME: ...               // Fix now
throw new \Exception('Not implemented');  // Implement it
```

**REQUIRED PATTERNS - Always Use:**
```php
/**
 * Short description of the method.
 *
 * @param string $name Description of parameter
 * @return bool Description of return value
 * @throws InvalidArgumentException When invalid input provided
 */
public function methodName(string $name): bool
{
    // Implementation
}
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing PHPUnit test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run PHPStan + PHPCS + composer audit
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every public method: at least 1 test
- Every class: exercised in integration tests
- Every error path: tested with `expectException()`
- Every edge case: empty inputs, boundaries, null values

### PHP Quality Tools

Run all checks before commit:
```bash
# Static analysis
./vendor/bin/phpstan analyse

# Coding standards
./vendor/bin/phpcs

# Run tests
./vendor/bin/phpunit

# Security audit
composer audit
```

If PHPStan warns about:
- Return type mismatch -> Fix the return type
- Parameter type mismatch -> Fix the parameter type
- Undefined variable -> Ensure proper initialization

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
get_call_graph <method>     # Method relationships
find_references <symbol>    # Impact analysis
get_dependencies <path>     # Namespace dependencies
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
[ ] ./vendor/bin/phpstan analyse               -> 0 errors
[ ] ./vendor/bin/phpcs                         -> 0 violations
[ ] ./vendor/bin/phpunit                       -> all pass
[ ] composer audit                             -> 0 vulnerabilities
[ ] No @phpstan-ignore without justification
[ ] No TODO/FIXME comments in new code
[ ] All new public methods documented (PHPDoc)
[ ] All new methods have tests
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

## PHP-SPECIFIC CONVENTIONS

### Project Structure
```
project/
├── src/
│   └── Namespace/
│       ├── Service.php
│       ├── Model.php
│       └── ...
├── tests/
│   ├── Unit/
│   │   └── Namespace/
│   │       └── ServiceTest.php
│   ├── Integration/
│   └── bootstrap.php
├── composer.json
├── composer.lock
├── phpstan.neon
├── phpcs.xml
└── phpunit.xml
```

### Testing with PHPUnit
```php
<?php

declare(strict_types=1);

namespace Tests\Unit\Service;

use PHPUnit\Framework\TestCase;
use App\Service\Calculator;

final class CalculatorTest extends TestCase
{
    private Calculator $calculator;

    protected function setUp(): void
    {
        $this->calculator = new Calculator();
    }

    public function testAddReturnsSumOfTwoNumbers(): void
    {
        $result = $this->calculator->add(2, 3);

        $this->assertSame(5, $result);
    }

    public function testAddHandlesNegativeNumbers(): void
    {
        $result = $this->calculator->add(-1, 5);

        $this->assertSame(4, $result);
    }

    public function testAddThrowsExceptionForInvalidInput(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        $this->expectExceptionMessage('Invalid input');

        $this->calculator->add('invalid', 1);
    }

    /**
     * @dataProvider additionProvider
     */
    public function testAddWithDataProvider(int $a, int $b, int $expected): void
    {
        $this->assertSame($expected, $this->calculator->add($a, $b));
    }

    public static function additionProvider(): array
    {
        return [
            'positive numbers' => [1, 2, 3],
            'negative numbers' => [-1, -2, -3],
            'mixed numbers' => [-1, 2, 1],
        ];
    }
}
```

---

## QUICK REFERENCE

```bash
# Run tests
./vendor/bin/phpunit

# Run tests with coverage
./vendor/bin/phpunit --coverage-html coverage

# Static analysis
./vendor/bin/phpstan analyse

# Coding standards check
./vendor/bin/phpcs

# Auto-fix coding standards
./vendor/bin/phpcbf

# Security audit
composer audit

# Verify git environment
gh auth status
```
