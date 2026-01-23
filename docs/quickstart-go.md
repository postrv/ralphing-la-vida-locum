# Quick Start: Go Projects

Get Ralph up and running with your Go project in 5 minutes.

## Prerequisites

Before starting, ensure you have:

- **Ralph** installed ([installation guide](../README.md#installation))
- **Go 1.21+** installed
- **GitHub CLI** authenticated (`gh auth status`)
- **Claude Code 2.1.0+** for autonomous execution

### Recommended Go Tools

Ralph works best with these tools installed:

```bash
# Install recommended tools
go install golang.org/x/tools/cmd/goimports@latest
go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
go install golang.org/x/vuln/cmd/govulncheck@latest

# Verify installation
go version              # Go compiler
golangci-lint --version # Comprehensive linting
govulncheck --version   # Vulnerability scanning
```

## Step 1: Bootstrap Your Project

Navigate to your Go project and run the bootstrap command:

```bash
cd /path/to/your-go-project
ralph --project . bootstrap
```

**Expected output:**

```
Bootstrapping project: /path/to/your-go-project
  Detected languages:
    → Go (primary)            # 94% confidence
  Creating .claude/ directory
  Creating docs/ directory
  Writing CLAUDE.md
  Writing settings.json
  Writing IMPLEMENTATION_PLAN.md
  Writing PROMPT_build.md
Bootstrap complete!
```

Ralph auto-detects Go from `go.mod`, `go.sum`, or `.go` files.

## Step 2: Verify Detection

Check that Ralph correctly detected your project:

```bash
ralph --project . detect
```

**Expected output:**

```
Detected languages:
  → Go (primary)              # Based on go.mod
```

## Step 3: Create Your Implementation Plan

Edit `IMPLEMENTATION_PLAN.md` with your tasks:

```markdown
# Implementation Plan

## Current Sprint

### Phase 1: Core Package
- [ ] Create User struct with validation
- [ ] Add UserRepository interface
- [ ] Implement InMemoryUserRepository
- [ ] Write table-driven tests for all methods

### Phase 2: HTTP Layer
- [ ] Create HTTP handlers for user CRUD
- [ ] Add request/response DTOs
- [ ] Write integration tests with httptest
- [ ] Add graceful shutdown
```

## Step 4: Run the Loop

Start Ralph's autonomous execution loop:

```bash
# Plan phase: let Claude analyze and plan the implementation
ralph --project . loop --phase plan --max-iterations 5

# Build phase: autonomous coding
ralph --project . loop --phase build --max-iterations 20

# With verbose output for debugging
ralph --verbose --project . loop --phase build --max-iterations 10
```

## Quality Gates

Ralph enforces these quality gates for Go projects:

| Gate | Command | Requirement |
|------|---------|-------------|
| **Vet** | `go vet ./...` | 0 issues |
| **Lint** | `golangci-lint run` | 0 warnings |
| **Tests** | `go test ./...` | All pass |
| **Security** | `govulncheck ./...` | 0 vulnerabilities |

Ralph will not commit code that fails any gate.

## Project Structure

After bootstrap, your project will have:

```
your-go-project/
├── .claude/
│   ├── CLAUDE.md           # Project memory for Claude
│   ├── settings.json       # Permissions and hooks
│   ├── mcp.json            # MCP server configuration
│   ├── skills/             # Custom skills
│   └── agents/             # Subagents
├── docs/
│   ├── architecture.md     # Architecture template
│   └── api.md              # API documentation template
├── IMPLEMENTATION_PLAN.md  # Your task list
├── PROMPT_build.md         # Build phase prompt
├── PROMPT_plan.md          # Plan phase prompt
└── PROMPT_debug.md         # Debug phase prompt
```

## Example: HTTP API Project

Here's a complete example for a Go HTTP API:

### 1. Create Project

```bash
mkdir my-go-api && cd my-go-api
go mod init github.com/yourusername/my-go-api
```

### 2. Bootstrap

```bash
ralph --project . bootstrap
```

### 3. Implementation Plan

Create `IMPLEMENTATION_PLAN.md`:

```markdown
# Go REST API

## Sprint 1: Core API

### Phase 1.1: Domain Models
- [ ] Create Task struct with ID, Title, Done fields
- [ ] Add NewTask constructor with validation
- [ ] Write table-driven tests for Task

### Phase 1.2: Repository Layer
- [ ] Define TaskRepository interface
- [ ] Implement InMemoryTaskRepository
- [ ] Write tests for repository operations

### Phase 1.3: HTTP Handlers
- [ ] GET /tasks - list all tasks
- [ ] POST /tasks - create task
- [ ] PUT /tasks/{id} - update task
- [ ] DELETE /tasks/{id} - delete task
- [ ] Write integration tests with httptest
```

### 4. Run

```bash
ralph --project . loop --phase build --max-iterations 15
```

## Example: CLI Application

Here's an example for a CLI tool:

### 1. Create Project

```bash
mkdir my-cli-tool && cd my-cli-tool
go mod init github.com/yourusername/my-cli-tool
go get github.com/spf13/cobra@latest
```

### 2. Standard CLI Structure

```
my-cli-tool/
├── cmd/
│   ├── root.go           # Root command
│   ├── init.go           # init subcommand
│   └── run.go            # run subcommand
├── internal/
│   ├── config/
│   │   └── config.go     # Configuration handling
│   └── runner/
│       └── runner.go     # Core business logic
├── main.go               # Entry point
├── go.mod
└── go.sum
```

### 3. Implementation Plan

```markdown
# CLI Tool Implementation

## Sprint 1: Foundation

### Phase 1.1: Command Structure
- [ ] Set up cobra root command
- [ ] Add --config flag for config file path
- [ ] Add --verbose flag for debug output
- [ ] Write tests for flag parsing

### Phase 1.2: Config Management
- [ ] Create Config struct
- [ ] Implement LoadConfig from YAML file
- [ ] Add environment variable overrides
- [ ] Write tests for config loading
```

### 4. Bootstrap and Run

```bash
ralph --project . bootstrap
ralph --project . loop --phase build --max-iterations 15
```

## Troubleshooting

### "No Go detected"

Ensure you have at least one of:
- `go.mod` file
- `go.sum` file
- `*.go` files in the project

Initialize a Go module:
```bash
go mod init github.com/yourusername/your-project
```

### "golangci-lint not found"

Install golangci-lint:

```bash
go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
```

Or use the binary installation:
```bash
curl -sSfL https://raw.githubusercontent.com/golangci/golangci-lint/master/install.sh | sh -s -- -b $(go env GOPATH)/bin
```

### "govulncheck not found"

Install govulncheck:

```bash
go install golang.org/x/vuln/cmd/govulncheck@latest
```

### "Tests not discovered"

Ensure your test files:
- Are named `*_test.go`
- Are in the same package or a `_test` package
- Have functions named `TestXxx(t *testing.T)`

Example test file `user_test.go`:

```go
package user

import "testing"

func TestNewUser(t *testing.T) {
    tests := []struct {
        name    string
        email   string
        wantErr bool
    }{
        {"valid email", "user@example.com", false},
        {"empty email", "", true},
        {"invalid email", "notanemail", true},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            _, err := NewUser(tt.email)
            if (err != nil) != tt.wantErr {
                t.Errorf("NewUser() error = %v, wantErr %v", err, tt.wantErr)
            }
        })
    }
}
```

### "go vet finds issues"

Common issues and fixes:

**Printf format mismatch:**
```go
// Wrong
fmt.Printf("value: %s", 123)

// Correct
fmt.Printf("value: %d", 123)
```

**Unreachable code:**
```go
// Wrong
func example() int {
    return 1
    fmt.Println("never reached")  // go vet warns
}
```

**Struct field alignment:**
```go
// Better performance with aligned fields
type User struct {
    ID        int64   // 8 bytes
    Active    bool    // 1 byte + 7 padding
    Name      string  // 16 bytes
}
```

## golangci-lint Configuration

Create `.golangci.yml` for customized linting:

```yaml
run:
  timeout: 5m

linters:
  enable:
    - errcheck
    - gosimple
    - govet
    - ineffassign
    - staticcheck
    - unused
    - gofmt
    - goimports

linters-settings:
  errcheck:
    check-type-assertions: true
  govet:
    check-shadowing: true

issues:
  exclude-rules:
    - path: _test\.go
      linters:
        - errcheck
```

## Next Steps

- Read the [full documentation](../README.md)
- Explore [checkpoint and rollback](../README.md#checkpoint--rollback)
- Learn about [narsil-mcp integration](../README.md#narsil-mcp-integration)
- Set up [custom quality gates](./developing-gates.md)
