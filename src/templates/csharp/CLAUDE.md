# Project Memory - C# Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: C# Project

This is a C# project. Key conventions and tools:

- **Testing**: xUnit (preferred), NUnit, or MSTest
- **Static Analysis**: Roslyn analyzers, StyleCop
- **Security**: dotnet list package --vulnerable, OWASP dependency check
- **Formatting**: dotnet format, EditorConfig

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
```csharp
#pragma warning disable       // Fix the warning properly
[SuppressMessage(...)]        // Fix the analyzer issue
// TODO: ...                  // Implement now or don't merge
// FIXME: ...                 // Fix now
throw new NotImplementedException();  // Implement it
```

**REQUIRED PATTERNS - Always Use:**
```csharp
/// <summary>
/// Short description of the method.
/// </summary>
/// <param name="name">Description of parameter</param>
/// <returns>Description of return value</returns>
/// <exception cref="ArgumentException">When invalid input provided</exception>
public bool MethodName(string name)
{
    // Implementation
}
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing xUnit test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run analyzers + vulnerability check
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every public method: at least 1 test
- Every class: exercised in integration tests
- Every error path: tested with `Assert.Throws<T>()`
- Every edge case: empty inputs, boundaries, null values

### C# Quality Tools

Run all checks before commit:
```bash
# Build with warnings as errors
dotnet build --warnaserror

# Run tests
dotnet test

# Format check
dotnet format --verify-no-changes

# Vulnerability check
dotnet list package --vulnerable
```

If analyzers warn about:
- CS0168 (unused variable) -> Remove the variable
- CS8600 (null reference) -> Add null check or use nullable type
- CA1062 (validate parameter) -> Add argument validation

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
[ ] dotnet build --warnaserror                 -> 0 warnings
[ ] dotnet test                                -> all pass
[ ] dotnet list package --vulnerable           -> 0 vulnerabilities
[ ] No #pragma warning disable without justification
[ ] No TODO/FIXME comments in new code
[ ] All new public methods documented (XML docs)
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

## C#-SPECIFIC CONVENTIONS

### Project Structure
```
Solution/
├── src/
│   └── Project/
│       ├── Services/
│       │   └── Calculator.cs
│       ├── Models/
│       └── Project.csproj
├── tests/
│   └── Project.Tests/
│       ├── Services/
│       │   └── CalculatorTests.cs
│       └── Project.Tests.csproj
├── Solution.sln
├── Directory.Build.props
├── .editorconfig
└── README.md
```

### Testing with xUnit
```csharp
using Xunit;
using FluentAssertions;

namespace Project.Tests.Services;

public class CalculatorTests
{
    private readonly Calculator _calculator;

    public CalculatorTests()
    {
        _calculator = new Calculator();
    }

    [Fact]
    public void Add_WithTwoPositiveNumbers_ReturnsSumOfNumbers()
    {
        // Arrange
        int a = 2, b = 3;

        // Act
        var result = _calculator.Add(a, b);

        // Assert
        result.Should().Be(5);
    }

    [Fact]
    public void Add_WithNegativeNumbers_HandlesCorrectly()
    {
        var result = _calculator.Add(-1, 5);

        result.Should().Be(4);
    }

    [Fact]
    public void Add_WithInvalidInput_ThrowsArgumentException()
    {
        Action act = () => _calculator.Add(null!, 1);

        act.Should().Throw<ArgumentNullException>();
    }

    [Theory]
    [InlineData(1, 2, 3)]
    [InlineData(-1, -2, -3)]
    [InlineData(-1, 2, 1)]
    public void Add_WithVariousInputs_ReturnsExpectedResult(int a, int b, int expected)
    {
        var result = _calculator.Add(a, b);

        result.Should().Be(expected);
    }
}
```

---

## QUICK REFERENCE

```bash
# Build
dotnet build

# Build with warnings as errors
dotnet build --warnaserror

# Run tests
dotnet test

# Run tests with coverage
dotnet test --collect:"XPlat Code Coverage"

# Format code
dotnet format

# Check formatting
dotnet format --verify-no-changes

# Vulnerability check
dotnet list package --vulnerable

# Verify git environment
gh auth status
```
