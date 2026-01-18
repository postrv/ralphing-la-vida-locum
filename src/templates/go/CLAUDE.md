# Project Memory - Go Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: Go Project

This is a Go project. Key conventions and tools:

- **Testing**: go test with table-driven tests
- **Linting**: go vet + golangci-lint
- **Formatting**: gofmt / goimports
- **Security**: govulncheck for vulnerability scanning

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
```go
//nolint                    // Fix the issue properly
_ = err                     // Handle the error
// TODO: ...                // Implement now or don't merge
// FIXME: ...               // Fix now
panic("not implemented")    // Implement properly
```

**REQUIRED PATTERNS - Always Use:**
```go
// FunctionName does something useful.
//
// It takes an input and returns a result.
//
// Example:
//
//	result, err := FunctionName("input")
//	if err != nil {
//	    log.Fatal(err)
//	}
func FunctionName(input string) (Result, error) {
    if input == "" {
        return Result{}, errors.New("input cannot be empty")
    }
    // ...
}
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing go test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run go vet + golangci-lint + govulncheck
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every exported function: at least 1 unit test
- Every package: integration tests for key workflows
- Every error path: tested explicitly
- Every edge case: empty inputs, nil pointers, boundaries

### Go Quality Tools

Run all checks before commit:
```bash
# Format code
gofmt -w .
goimports -w .

# Vet for suspicious constructs
go vet ./...

# Comprehensive linting
golangci-lint run

# Run tests
go test ./...

# Run tests with coverage
go test -coverprofile=coverage.out ./...

# Security vulnerabilities
govulncheck ./...
```

If golangci-lint warns about:
- `deadcode` -> Remove the dead code
- `errcheck` -> Handle the error
- `ineffassign` -> Remove the ineffective assignment
- `staticcheck` -> Fix the static analysis issue

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
get_dependencies <path>     # Package dependencies
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
[ ] go vet ./...                               -> 0 issues
[ ] golangci-lint run                          -> 0 warnings
[ ] go test ./...                              -> all pass
[ ] govulncheck ./...                          -> 0 vulnerabilities
[ ] No //nolint without justification
[ ] No ignored errors (_, _ = ...)
[ ] No TODO/FIXME comments in new code
[ ] All new exports documented (GoDoc)
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

## GO-SPECIFIC CONVENTIONS

### Project Structure
```
project/
├── cmd/
│   └── app/
│       └── main.go       # Entry point
├── internal/
│   ├── service/
│   │   └── service.go
│   └── repository/
│       └── repo.go
├── pkg/
│   └── utils/
│       └── helpers.go
├── go.mod
├── go.sum
└── README.md
```

### Imports
```go
import (
    // Standard library first
    "context"
    "fmt"

    // External packages second
    "github.com/pkg/errors"
    "go.uber.org/zap"

    // Internal packages last
    "project/internal/service"
)
```

### Table-Driven Tests
```go
func TestFunction(t *testing.T) {
    tests := []struct {
        name    string
        input   string
        want    int
        wantErr bool
    }{
        {
            name:  "basic case",
            input: "hello",
            want:  5,
        },
        {
            name:  "empty input",
            input: "",
            want:  0,
        },
        {
            name:    "invalid input",
            input:   "invalid",
            wantErr: true,
        },
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got, err := Function(tt.input)
            if (err != nil) != tt.wantErr {
                t.Errorf("Function() error = %v, wantErr %v", err, tt.wantErr)
                return
            }
            if got != tt.want {
                t.Errorf("Function() = %v, want %v", got, tt.want)
            }
        })
    }
}
```

### Error Handling
```go
// Always handle errors - never ignore them
result, err := doSomething()
if err != nil {
    return fmt.Errorf("doSomething failed: %w", err)
}

// Use errors.Is and errors.As for comparison
if errors.Is(err, ErrNotFound) {
    // Handle not found
}
```

---

## QUICK REFERENCE

```bash
# Build
go build ./...

# Run tests
go test ./...

# Run tests with verbose output
go test -v ./...

# Run tests with coverage
go test -coverprofile=coverage.out ./...
go tool cover -html=coverage.out

# Vet code
go vet ./...

# Lint code
golangci-lint run

# Check for vulnerabilities
govulncheck ./...

# Verify git environment
gh auth status
```
