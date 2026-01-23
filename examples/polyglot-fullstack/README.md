# Polyglot Fullstack Demo

A complete example demonstrating Ralph's polyglot features with a **Next.js TypeScript frontend** and **FastAPI Python backend**.

## Overview

This project showcases how Ralph orchestrates quality gates across multiple languages:

- **Frontend**: Next.js 14 with TypeScript, ESLint, and type checking
- **Backend**: FastAPI with Python 3.10+, Ruff, mypy, and pytest
- **Shared**: OpenAPI specification and TypeScript types

## Project Structure

```
polyglot-fullstack/
├── frontend/               # Next.js TypeScript app
│   ├── app/                # App router pages
│   ├── components/         # React components
│   ├── lib/                # API client
│   ├── package.json
│   └── tsconfig.json
├── backend/                # FastAPI Python app
│   ├── app/                # Application code
│   │   ├── main.py         # API endpoints
│   │   ├── models.py       # Pydantic models
│   │   └── store.py        # In-memory storage
│   ├── tests/              # pytest test suite
│   ├── pyproject.toml
│   └── requirements.txt
├── shared/                 # Shared API definitions
│   ├── openapi.yaml        # OpenAPI 3.1 specification
│   └── types.ts            # TypeScript type definitions
├── IMPLEMENTATION_PLAN.md  # Sample task plan for Ralph
└── README.md               # This file
```

## Quick Start

### Prerequisites

- Node.js 18+ and npm
- Python 3.10+ and pip
- Ralph installed (`cargo install ralph` or from source)

### 1. Bootstrap the Project

```bash
cd examples/polyglot-fullstack
ralph --project . bootstrap
```

**Expected output:**
```
Bootstrapping project: examples/polyglot-fullstack
  Detected languages:
    → TypeScript (primary)    # From frontend/
    → Python                  # From backend/
  Creating .claude/ directory
  Creating docs/ directory
  Writing CLAUDE.md
Bootstrap complete!
```

### 2. Verify Detection

```bash
ralph --project . detect
```

**Expected output:**
```
Detected languages:
  → TypeScript (primary)
    Available gates: eslint, tsc
  → Python
    Available gates: ruff, mypy, pytest, bandit
```

### 3. Install Dependencies

```bash
# Frontend
cd frontend
npm install

# Backend
cd ../backend
python -m venv .venv
source .venv/bin/activate  # or .venv\Scripts\activate on Windows
pip install -r requirements.txt
```

### 4. Run Quality Gates Manually

```bash
# Frontend (from frontend/)
npm run lint
npm run type-check

# Backend (from backend/ with venv activated)
ruff check .
mypy app
pytest
```

### 5. Run Ralph Loop

```bash
# From the polyglot-fullstack directory
ralph --project . loop --phase build --max-iterations 3
```

Ralph will:
1. Run ESLint and TypeScript checks on the frontend
2. Run Ruff, mypy, and pytest on the backend
3. Block commits if any gate fails
4. Continue until the implementation plan is complete or max iterations reached

## Running the Application

### Start the Backend

```bash
cd backend
source .venv/bin/activate
uvicorn app.main:app --reload
```

The API will be available at `http://localhost:8000`.
- API docs: `http://localhost:8000/docs`
- OpenAPI spec: `http://localhost:8000/openapi.json`

### Start the Frontend

```bash
cd frontend
npm run dev
```

The app will be available at `http://localhost:3000`.

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/health` | Health check |
| GET | `/api/tasks` | List all tasks |
| POST | `/api/tasks` | Create a task |
| GET | `/api/tasks/{id}` | Get a task |
| PATCH | `/api/tasks/{id}` | Update a task |
| DELETE | `/api/tasks/{id}` | Delete a task |

## Quality Gates

Ralph enforces these gates on each commit:

### TypeScript (Frontend)

| Gate | Command | Requirement |
|------|---------|-------------|
| **ESLint** | `npm run lint` | 0 warnings |
| **TypeScript** | `npm run type-check` | 0 errors |

### Python (Backend)

| Gate | Command | Requirement |
|------|---------|-------------|
| **Ruff** | `ruff check .` | 0 warnings |
| **mypy** | `mypy app` | 0 errors |
| **pytest** | `pytest` | All pass |
| **Bandit** | `bandit -r app -ll` | 0 HIGH/CRITICAL |

## Using with Ralph

This example includes a sample `IMPLEMENTATION_PLAN.md` with tasks for extending the application. Try running Ralph in autonomous mode:

```bash
# Plan phase - analyze the codebase and plan implementation
ralph --project . loop --phase plan --max-iterations 5

# Build phase - implement the planned changes
ralph --project . loop --phase build --max-iterations 20
```

## Extending the Example

Ideas for practice:

1. **Add user authentication** - JWT tokens, login/logout endpoints
2. **Add task categories** - Tags or categories for tasks
3. **Add due dates** - Date fields with validation
4. **Persist to database** - Replace in-memory store with SQLite/PostgreSQL
5. **Add frontend tests** - Jest or Vitest for React component testing

## Troubleshooting

### "Module not found" in frontend

Ensure you've run `npm install` in the `frontend/` directory.

### "ruff: command not found"

Ensure your virtual environment is activated:
```bash
source backend/.venv/bin/activate
```

### Backend tests fail with import errors

Run pytest from the `backend/` directory:
```bash
cd backend
pytest
```

### CORS errors in browser

Ensure the backend is running on port 8000 and the frontend on port 3000. The backend is configured to allow CORS from `http://localhost:3000`.

## License

MIT License - This is an example project for demonstrating Ralph features.
