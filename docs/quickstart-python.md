# Quick Start: Python Projects

Get Ralph up and running with your Python project in 5 minutes.

## Prerequisites

Before starting, ensure you have:

- **Ralph** installed ([installation guide](../README.md#installation))
- **Python 3.9+** with pip or a virtual environment
- **GitHub CLI** authenticated (`gh auth status`)
- **Claude Code 2.1.0+** for autonomous execution

### Recommended Python Tools

Ralph works best with these tools installed:

```bash
# Install recommended tools
pip install ruff mypy pytest bandit

# Verify installation
ruff --version    # Linting and formatting
mypy --version    # Type checking
pytest --version  # Testing
bandit --version  # Security scanning
```

## Step 1: Bootstrap Your Project

Navigate to your Python project and run the bootstrap command:

```bash
cd /path/to/your-python-project
ralph --project . bootstrap
```

**Expected output:**

```
Bootstrapping project: /path/to/your-python-project
  Detected languages:
    → Python (primary)        # 92% confidence
  Creating .claude/ directory
  Creating docs/ directory
  Writing CLAUDE.md
  Writing settings.json
  Writing IMPLEMENTATION_PLAN.md
  Writing PROMPT_build.md
Bootstrap complete!
```

Ralph auto-detects Python from `pyproject.toml`, `requirements.txt`, `setup.py`, or `.py` files.

## Step 2: Verify Detection

Check that Ralph correctly detected your project:

```bash
ralph --project . detect
```

**Expected output:**

```
Detected languages:
  → Python (primary)          # Based on pyproject.toml
```

## Step 3: Create Your Implementation Plan

Edit `IMPLEMENTATION_PLAN.md` with your tasks:

```markdown
# Implementation Plan

## Current Sprint

### Phase 1: Core API
- [ ] Create user model with SQLAlchemy
- [ ] Add authentication endpoint (POST /auth/login)
- [ ] Write tests for login flow
- [ ] Add JWT token generation

### Phase 2: Integration
- [ ] Connect to PostgreSQL database
- [ ] Add pytest fixtures for database testing
- [ ] Integration test: full auth flow
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

Ralph enforces these quality gates for Python projects:

| Gate | Command | Requirement |
|------|---------|-------------|
| **Lint** | `ruff check .` | 0 warnings |
| **Types** | `mypy .` | 0 errors |
| **Tests** | `pytest` | All pass |
| **Security** | `bandit -r . -ll` | 0 HIGH/CRITICAL |

Ralph will not commit code that fails any gate.

## Project Structure

After bootstrap, your project will have:

```
your-python-project/
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

## Example: FastAPI Project

Here's a complete example for a FastAPI project:

### 1. Create Project

```bash
mkdir my-fastapi-app && cd my-fastapi-app
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows
pip install fastapi uvicorn pytest httpx ruff mypy bandit
```

### 2. Bootstrap

```bash
ralph --project . bootstrap
```

### 3. Implementation Plan

Create `IMPLEMENTATION_PLAN.md`:

```markdown
# FastAPI Todo App

## Sprint 1: Core API

### Phase 1.1: Models
- [ ] Create TodoItem pydantic model
- [ ] Add validation for title (non-empty, max 100 chars)
- [ ] Write tests for model validation

### Phase 1.2: Endpoints
- [ ] GET /todos - list all todos
- [ ] POST /todos - create todo
- [ ] DELETE /todos/{id} - delete todo
- [ ] Write integration tests for all endpoints
```

### 4. Run

```bash
ralph --project . loop --phase build --max-iterations 15
```

## Troubleshooting

### "No Python detected"

Ensure you have at least one of:
- `pyproject.toml`
- `requirements.txt`
- `setup.py`
- `*.py` files in the root or `src/` directory

### "ruff not found"

Install ruff:
```bash
pip install ruff
```

Or use flake8 instead (update `.claude/CLAUDE.md` to reference flake8).

### "mypy errors on third-party packages"

Add a `mypy.ini` or section in `pyproject.toml`:

```toml
[tool.mypy]
ignore_missing_imports = true
```

### "Tests not discovered"

Ensure your test files:
- Are named `test_*.py` or `*_test.py`
- Are in a `tests/` directory or alongside the code
- Import pytest: `import pytest`

## Next Steps

- Read the [full documentation](../README.md)
- Explore [checkpoint and rollback](../README.md#checkpoint--rollback)
- Learn about [narsil-mcp integration](../README.md#narsil-mcp-integration)
- Set up [custom quality gates](./developing-gates.md)
