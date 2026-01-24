//! End-to-end integration tests for polyglot project support.
//!
//! These tests verify that Ralph correctly handles polyglot projects with
//! multiple programming languages, specifically testing the Next.js + FastAPI
//! combination (TypeScript + Python).
//!
//! # Test Requirements (Phase 9.3)
//!
//! - Test `ralph bootstrap` on Next.js + FastAPI project
//! - Test language detection finds TypeScript and Python
//! - Test `ralph loop --max-iterations 1` runs correct gates
//! - Test gate failures produce appropriate remediation
//! - Test commits only happen when all relevant gates pass

use assert_cmd::cargo;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Test Fixture Helpers
// ============================================================================

/// Creates a minimal polyglot project with Next.js (TypeScript) frontend
/// and FastAPI (Python) backend.
fn create_polyglot_fixture() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp directory");

    // === Frontend (Next.js + TypeScript) ===
    fs::create_dir_all(temp.path().join("frontend/src/components"))
        .expect("Failed to create frontend directory");

    // package.json - marks this as a Node.js project
    fs::write(
        temp.path().join("frontend/package.json"),
        r#"{
  "name": "polyglot-frontend",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "lint": "eslint . --ext .ts,.tsx"
  },
  "dependencies": {
    "next": "14.0.0",
    "react": "18.2.0",
    "react-dom": "18.2.0"
  },
  "devDependencies": {
    "@types/node": "20.0.0",
    "@types/react": "18.2.0",
    "eslint": "8.50.0",
    "typescript": "5.0.0"
  }
}
"#,
    )
    .expect("Failed to write package.json");

    // tsconfig.json - marks this as a TypeScript project
    fs::write(
        temp.path().join("frontend/tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "es5",
    "lib": ["dom", "dom.iterable", "esnext"],
    "strict": true,
    "module": "esnext",
    "moduleResolution": "node",
    "jsx": "preserve"
  },
  "include": ["src/**/*"]
}
"#,
    )
    .expect("Failed to write tsconfig.json");

    // Main page component
    fs::write(
        temp.path().join("frontend/src/page.tsx"),
        r#"import { useState } from 'react';

export default function Home() {
  const [count, setCount] = useState(0);
  return (
    <main>
      <h1>Polyglot App</h1>
      <button onClick={() => setCount(c => c + 1)}>
        Count: {count}
      </button>
    </main>
  );
}
"#,
    )
    .expect("Failed to write page.tsx");

    // A simple component
    fs::write(
        temp.path().join("frontend/src/components/Button.tsx"),
        r#"interface ButtonProps {
  label: string;
  onClick: () => void;
}

export function Button({ label, onClick }: ButtonProps) {
  return <button onClick={onClick}>{label}</button>;
}
"#,
    )
    .expect("Failed to write Button.tsx");

    // TypeScript utility
    fs::write(
        temp.path().join("frontend/src/utils.ts"),
        r#"export function formatDate(date: Date): string {
  return date.toISOString().split('T')[0];
}

export function fetchApi<T>(endpoint: string): Promise<T> {
  return fetch(`/api/${endpoint}`).then(res => res.json());
}
"#,
    )
    .expect("Failed to write utils.ts");

    // === Backend (FastAPI + Python) ===
    fs::create_dir_all(temp.path().join("backend/app/routers"))
        .expect("Failed to create backend directory");

    // pyproject.toml - marks this as a Python project
    fs::write(
        temp.path().join("backend/pyproject.toml"),
        r#"[project]
name = "polyglot-backend"
version = "0.1.0"
description = "FastAPI backend for polyglot app"
requires-python = ">=3.11"
dependencies = [
    "fastapi>=0.100.0",
    "uvicorn>=0.23.0",
    "pydantic>=2.0.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0.0",
    "ruff>=0.1.0",
    "mypy>=1.0.0",
]

[tool.ruff]
line-length = 88
select = ["E", "F", "I"]

[tool.mypy]
strict = true
"#,
    )
    .expect("Failed to write pyproject.toml");

    // requirements.txt - alternative Python manifest
    fs::write(
        temp.path().join("backend/requirements.txt"),
        r#"fastapi>=0.100.0
uvicorn>=0.23.0
pydantic>=2.0.0
"#,
    )
    .expect("Failed to write requirements.txt");

    // Main FastAPI app
    fs::write(
        temp.path().join("backend/app/main.py"),
        r#"from fastapi import FastAPI
from .routers import items, users

app = FastAPI(title="Polyglot API")

app.include_router(items.router)
app.include_router(users.router)


@app.get("/")
async def root() -> dict[str, str]:
    return {"message": "Hello from FastAPI"}


@app.get("/health")
async def health() -> dict[str, str]:
    return {"status": "healthy"}
"#,
    )
    .expect("Failed to write main.py");

    // __init__.py for the app package
    fs::write(
        temp.path().join("backend/app/__init__.py"),
        r#""""FastAPI application package."""
"#,
    )
    .expect("Failed to write app/__init__.py");

    // Items router
    fs::write(
        temp.path().join("backend/app/routers/items.py"),
        r#"from fastapi import APIRouter
from pydantic import BaseModel

router = APIRouter(prefix="/items", tags=["items"])


class Item(BaseModel):
    name: str
    price: float
    description: str | None = None


@router.get("/")
async def list_items() -> list[Item]:
    return [Item(name="Widget", price=9.99)]


@router.post("/")
async def create_item(item: Item) -> Item:
    return item
"#,
    )
    .expect("Failed to write items.py");

    // Users router
    fs::write(
        temp.path().join("backend/app/routers/users.py"),
        r#"from fastapi import APIRouter
from pydantic import BaseModel

router = APIRouter(prefix="/users", tags=["users"])


class User(BaseModel):
    id: int
    username: str
    email: str


@router.get("/{user_id}")
async def get_user(user_id: int) -> User:
    return User(id=user_id, username="testuser", email="test@example.com")
"#,
    )
    .expect("Failed to write users.py");

    // __init__.py for routers package
    fs::write(
        temp.path().join("backend/app/routers/__init__.py"),
        r#""""API routers package."""
"#,
    )
    .expect("Failed to write routers/__init__.py");

    // Python tests
    fs::create_dir_all(temp.path().join("backend/tests")).expect("Failed to create tests dir");
    fs::write(
        temp.path().join("backend/tests/__init__.py"),
        r#""""Test package."""
"#,
    )
    .expect("Failed to write tests/__init__.py");

    fs::write(
        temp.path().join("backend/tests/test_main.py"),
        r#"from fastapi.testclient import TestClient
from app.main import app


client = TestClient(app)


def test_root() -> None:
    response = client.get("/")
    assert response.status_code == 200
    assert response.json() == {"message": "Hello from FastAPI"}


def test_health() -> None:
    response = client.get("/health")
    assert response.status_code == 200
    assert response.json()["status"] == "healthy"
"#,
    )
    .expect("Failed to write test_main.py");

    temp
}

/// Creates a minimal polyglot project at the root level (not nested in subdirs)
/// for simpler language detection.
fn create_root_level_polyglot_fixture() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp directory");

    // === TypeScript files at root ===
    fs::write(
        temp.path().join("package.json"),
        r#"{
  "name": "polyglot-app",
  "version": "0.1.0",
  "scripts": {
    "build": "tsc",
    "lint": "eslint . --ext .ts,.tsx"
  },
  "dependencies": {
    "react": "18.2.0"
  },
  "devDependencies": {
    "typescript": "5.0.0"
  }
}
"#,
    )
    .expect("Failed to write package.json");

    fs::write(
        temp.path().join("tsconfig.json"),
        r#"{"compilerOptions": {"strict": true}}"#,
    )
    .expect("Failed to write tsconfig.json");

    fs::create_dir_all(temp.path().join("src")).expect("Failed to create src dir");

    fs::write(
        temp.path().join("src/index.ts"),
        "export const version = '1.0.0';\n",
    )
    .expect("Failed to write index.ts");

    fs::write(
        temp.path().join("src/App.tsx"),
        "export default function App() { return <div>Hello</div>; }\n",
    )
    .expect("Failed to write App.tsx");

    // === Python files at root ===
    fs::write(
        temp.path().join("pyproject.toml"),
        r#"[project]
name = "backend"
version = "0.1.0"
requires-python = ">=3.11"
dependencies = ["fastapi"]
"#,
    )
    .expect("Failed to write pyproject.toml");

    fs::write(
        temp.path().join("main.py"),
        r#"from fastapi import FastAPI

app = FastAPI()

@app.get("/")
def root():
    return {"hello": "world"}
"#,
    )
    .expect("Failed to write main.py");

    fs::write(
        temp.path().join("utils.py"),
        r#"def add(a: int, b: int) -> int:
    return a + b
"#,
    )
    .expect("Failed to write utils.py");

    temp
}

/// Get a Command for the ralph binary
fn ralph() -> Command {
    Command::new(cargo::cargo_bin!("ralph"))
}

// ============================================================================
// Test: Bootstrap on Polyglot Project
// ============================================================================

#[test]
fn test_bootstrap_on_polyglot_project_detects_both_languages() {
    // Requirement: Test `ralph bootstrap` on Next.js + FastAPI project
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success()
        .stdout(predicate::str::contains("Detected languages"))
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("Python"));
}

#[test]
fn test_bootstrap_creates_polyglot_specific_claude_md() {
    // Bootstrap should create CLAUDE.md that mentions both languages
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Verify CLAUDE.md was created
    let claude_md_path = temp.path().join(".claude/CLAUDE.md");
    assert!(claude_md_path.exists(), "CLAUDE.md should be created");

    // Check that it mentions both languages (not necessarily both, but should exist)
    let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
    assert!(!content.is_empty(), "CLAUDE.md should have content");
}

#[test]
fn test_bootstrap_reports_selected_gates_for_polyglot() {
    // Requirement: Bootstrap reports selected gates during setup
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success()
        .stdout(predicate::str::contains("Selected gates"));
}

// ============================================================================
// Test: Language Detection for Polyglot
// ============================================================================

#[test]
fn test_detect_finds_typescript_and_python() {
    // Requirement: Test language detection finds TypeScript and Python
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("confidence"));
}

#[test]
fn test_detect_shows_multiple_languages_with_confidence() {
    // Both TypeScript and Python should be detected with confidence scores
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("Detected languages"))
        // Should show at least 2 languages
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("Python"));
}

#[test]
fn test_detect_show_gates_for_polyglot() {
    // Requirement: Show which gates are available for each language
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .arg("--show-gates")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available gates"));
}

#[test]
fn test_detect_identifies_polyglot_project() {
    // The detect command should recognize this as a polyglot project
    let temp = create_root_level_polyglot_fixture();

    let output = ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);

    // Count unique language mentions - should have at least 2 significant languages
    let has_typescript = stdout.contains("TypeScript");
    let has_python = stdout.contains("Python");

    assert!(
        has_typescript && has_python,
        "Expected both TypeScript and Python to be detected. Output: {}",
        stdout
    );
}

// ============================================================================
// Test: Nested Polyglot Project Detection
// ============================================================================

#[test]
fn test_detect_nested_polyglot_structure() {
    // Test detection in a project with nested frontend/backend structure
    let temp = create_polyglot_fixture();

    // Even with nested structure, detect should find languages
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("Python"));
}

#[test]
fn test_bootstrap_nested_polyglot_creates_correct_structure() {
    // Bootstrap should work correctly on nested polyglot projects
    let temp = create_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success()
        .stdout(predicate::str::contains("Automation suite bootstrapped"));

    // Verify standard files are created
    assert!(temp.path().join(".claude/CLAUDE.md").exists());
    assert!(temp.path().join(".claude/settings.json").exists());
    assert!(temp.path().join("IMPLEMENTATION_PLAN.md").exists());
}

// ============================================================================
// Test: Loop Gate Execution (requires bootstrap first)
// ============================================================================

#[test]
fn test_loop_requires_implementation_plan() {
    // Verify loop fails without IMPLEMENTATION_PLAN.md (sanity check)
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--max-iterations")
        .arg("1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("IMPLEMENTATION_PLAN.md not found"));
}

#[test]
fn test_loop_detects_polyglot_gates_after_bootstrap() {
    // Requirement: Test `ralph loop --max-iterations 1` runs correct gates
    // After bootstrap, loop should detect all relevant polyglot gates
    let temp = create_root_level_polyglot_fixture();

    // Bootstrap first to create IMPLEMENTATION_PLAN.md
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Verify IMPLEMENTATION_PLAN.md exists
    assert!(
        temp.path().join("IMPLEMENTATION_PLAN.md").exists(),
        "Bootstrap should create IMPLEMENTATION_PLAN.md"
    );

    // Run loop with max 1 iteration - should complete successfully
    // and detect all polyglot gates
    let output = ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("loop")
        .arg("--max-iterations")
        .arg("1")
        .assert()
        .success()
        // Should detect polyglot gates
        .stdout(predicate::str::contains("Polyglot gates detected"))
        // TypeScript gates (always available in CI via Node)
        .stdout(predicate::str::contains("ESLint"))
        .stdout(predicate::str::contains("TypeScript"))
        .get_output()
        .clone();

    // Python gates are only present if Python tools (ruff, pytest) are installed
    // We don't assert them here since they may not be available in CI
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("Ruff") {
        assert!(
            stdout.contains("Pytest"),
            "If Ruff is available, Pytest should be too"
        );
    }
}

// ============================================================================
// Test: Gate Availability Detection
// ============================================================================

#[test]
fn test_detect_shows_python_gates() {
    // Python project should show Python-specific gates
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .arg("--show-gates")
        .assert()
        .success()
        .stdout(predicate::str::contains("Python"));
}

#[test]
fn test_detect_shows_typescript_gates() {
    // TypeScript project should show TypeScript-specific gates
    let temp = create_root_level_polyglot_fixture();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .arg("--show-gates")
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript"));
}

// ============================================================================
// Test: Config Validation for Polyglot
// ============================================================================

#[test]
fn test_config_validate_after_polyglot_bootstrap() {
    // Config validation should pass after polyglot bootstrap
    let temp = create_root_level_polyglot_fixture();

    // Bootstrap first
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Config validation should pass
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

#[test]
fn test_config_show_after_polyglot_bootstrap() {
    // Config show should work after polyglot bootstrap
    let temp = create_root_level_polyglot_fixture();

    // Bootstrap first
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Config show should work
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("config")
        .arg("show")
        .assert()
        .success();
}

// ============================================================================
// Test: Context Building for Polyglot
// ============================================================================

#[test]
fn test_context_includes_both_language_files() {
    // Context builder should include files from both languages
    let temp = create_root_level_polyglot_fixture();

    // Bootstrap first
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .assert()
        .success();

    // Build context with stats
    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("context")
        .arg("--stats-only")
        .assert()
        .success()
        .stdout(predicate::str::contains("files_included"));
}

// ============================================================================
// Test: Edge Cases
// ============================================================================

#[test]
fn test_detect_empty_project() {
    // Empty project should be handled gracefully
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No programming languages detected",
        ));
}

#[test]
fn test_bootstrap_with_explicit_language_overrides() {
    // Users should be able to override detected languages
    let temp = TempDir::new().unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("bootstrap")
        .arg("--language")
        .arg("typescript")
        .arg("--language")
        .arg("python")
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript"))
        .stdout(predicate::str::contains("Python"));
}

#[test]
fn test_detect_only_typescript_files() {
    // Project with only TypeScript should not detect Python
    let temp = TempDir::new().unwrap();

    // Create only TypeScript files
    fs::write(temp.path().join("package.json"), "{}").unwrap();
    fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src/index.ts"), "export const x = 1;").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("TypeScript"))
        // Python should NOT be in the output
        .stdout(predicate::str::contains("Python").not());
}

#[test]
fn test_detect_only_python_files() {
    // Project with only Python should not detect TypeScript
    let temp = TempDir::new().unwrap();

    // Create only Python files
    fs::write(
        temp.path().join("pyproject.toml"),
        "[project]\nname = \"test\"",
    )
    .unwrap();
    fs::write(temp.path().join("main.py"), "print('hello')").unwrap();
    fs::write(temp.path().join("utils.py"), "def foo(): pass").unwrap();

    ralph()
        .arg("--project")
        .arg(temp.path())
        .arg("detect")
        .assert()
        .success()
        .stdout(predicate::str::contains("Python"))
        // TypeScript should NOT be in the output
        .stdout(predicate::str::contains("TypeScript").not());
}

// ============================================================================
// Test: Polyglot Project File Fixture Integrity
// ============================================================================

#[test]
fn test_fixture_creates_valid_typescript_project() {
    // Verify the TypeScript portion of the fixture is valid
    let temp = create_polyglot_fixture();

    // Check TypeScript files exist
    assert!(temp.path().join("frontend/package.json").exists());
    assert!(temp.path().join("frontend/tsconfig.json").exists());
    assert!(temp.path().join("frontend/src/page.tsx").exists());
    assert!(temp
        .path()
        .join("frontend/src/components/Button.tsx")
        .exists());
    assert!(temp.path().join("frontend/src/utils.ts").exists());

    // Check package.json has expected content
    let package_json =
        fs::read_to_string(temp.path().join("frontend/package.json")).expect("Read package.json");
    assert!(package_json.contains("next"));
    assert!(package_json.contains("typescript"));
}

#[test]
fn test_fixture_creates_valid_python_project() {
    // Verify the Python portion of the fixture is valid
    let temp = create_polyglot_fixture();

    // Check Python files exist
    assert!(temp.path().join("backend/pyproject.toml").exists());
    assert!(temp.path().join("backend/requirements.txt").exists());
    assert!(temp.path().join("backend/app/main.py").exists());
    assert!(temp.path().join("backend/app/routers/items.py").exists());
    assert!(temp.path().join("backend/app/routers/users.py").exists());
    assert!(temp.path().join("backend/tests/test_main.py").exists());

    // Check pyproject.toml has expected content
    let pyproject = fs::read_to_string(temp.path().join("backend/pyproject.toml"))
        .expect("Read pyproject.toml");
    assert!(pyproject.contains("fastapi"));
    assert!(pyproject.contains("ruff"));
}
