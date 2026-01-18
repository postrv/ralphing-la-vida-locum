# Implementation Plan

> Ralph uses this file to track task progress. Update checkboxes as work completes.

## Status: READY

---

## NEXT STEPS (Start Here)

**Ralph, do this NOW:**

1. **`reindex`** - Refresh narsil-mcp index before starting
2. **Start Sprint 9** - Multi-Language CLI & Settings
3. **Follow TDD** - Write failing tests FIRST, then implement, then commit
4. **`reindex`** - Refresh narsil-mcp index after completing

**Current task:** Sprint 9b-9d - Language Settings Templates

---

## CRITICAL: TDD & Production Standards

**All work MUST follow Test-Driven Development (TDD):**
1. `reindex` - Refresh narsil-mcp index before starting
2. Write failing tests FIRST - before any implementation
3. Implement minimal code to make tests pass
4. Refactor while keeping tests green
5. Pass all quality gates before commit
6. `reindex` - Refresh narsil-mcp index after completing

**No shortcuts. No "tests later". Tests define the contract.**

---

## Sprint Overview

| Sprint | Focus | Status |
|--------|-------|--------|
| 1-6 | Foundation (State Machine, Prompts, Gates, Checkpoints, narsil-mcp, Language Detection) | Complete |
| 7 | Language-Specific Quality Gates | Complete |
| 8 | Language-Specific Prompts | Complete |
| **9** | **Multi-Language CLI & Settings** | **NEXT** |
| 10 | Polyglot & Advanced Features | Pending |
| 11 | CCG-Aware Prompts | Pending |
| 12-13 | Retry & Stagnation Prevention | Complete |

---

## Sprint 7: Language-Specific Quality Gates (Priority: P0) - COMPLETE

**Goal:** Create quality gates that use each language's standard tooling.

**Reference:** `further_dev_plans/ralph-multilang-bootstrap.md` for full design.

**Key existing code:** `src/quality/gates/mod.rs` (QualityGate trait + gates_for_language factory), `src/quality/gates/rust.rs` (Rust gates)

### 7a. QualityGate Trait Refactor ✓ COMPLETE
- [x] Create `QualityGate` trait with `name()`, `run()`, `is_blocking()`, `remediation()`
- [x] Migrate existing Rust gates (ClippyGate, TestGate, etc.) to new trait
- [x] Create `gates_for_language(Language) -> Vec<Box<dyn QualityGate>>` factory function
- [x] Add unit tests for trait and factory
- Files: `src/quality/gates/mod.rs`, `src/quality/gates/rust.rs`
- Acceptance: Trait-based gate system working for Rust, all existing tests pass

### 7b. Python Quality Gates ✓ COMPLETE
- [x] Implement `RuffGate` (with flake8 fallback detection)
- [x] Implement `PytestGate`
- [x] Implement `MypyGate`
- [x] Implement `BanditGate` (security)
- Files: `src/quality/gates/python.rs`
- Acceptance: Python projects get appropriate linting/testing gates

### 7c. TypeScript/JavaScript Quality Gates ✓ COMPLETE
- [x] Implement `EslintGate`
- [x] Implement `JestGate` (with vitest/mocha detection)
- [x] Implement `TscGate` (TypeScript type check)
- [x] Implement `NpmAuditGate`
- Files: `src/quality/gates/typescript.rs`
- Acceptance: TS/JS projects get appropriate gates

### 7d. Go Quality Gates ✓ COMPLETE
- [x] Implement `GoVetGate`
- [x] Implement `GolangciLintGate`
- [x] Implement `GoTestGate`
- [x] Implement `GovulncheckGate`
- Files: `src/quality/gates/go.rs`
- Acceptance: Go projects get appropriate gates

### 7e. Gate Auto-Detection ✓ COMPLETE
- [x] Create `detect_available_gates(Language) -> Vec<Box<dyn QualityGate>>` function
- [x] Check tool availability (e.g., `which ruff`) before adding gate
- [x] Combine gates for polyglot projects
- [x] Always include narsil-mcp security gate if available
- Files: `src/quality/gates/mod.rs`
- Acceptance: Only available tools are used as gates

---

## Sprint 8: Language-Specific Prompts (Priority: P0)

**Goal:** Generate language-appropriate build prompts and CLAUDE.md.

### 8a. Template Registry ✓ COMPLETE
- [x] Create `TemplateRegistry` struct
- [x] Implement `TemplateKind` enum (PromptBuild, ClaudeMd, etc.)
- [x] Load templates via `include_str!`
- [x] Implement `get(kind, language)` with generic fallback
- Files: `src/bootstrap/templates.rs`
- Acceptance: Registry loads all templates, get() with fallback works, 32 tests pass

### 8b-8e. Language Templates (Python, TypeScript, Go, Java) ✓ COMPLETE
- [x] Create `templates/{language}/PROMPT_build.md` for each
- [x] Create `templates/{language}/CLAUDE.md` for each
- [x] Include language-specific workflows and hard rules
- Files: `src/templates/{python,typescript,go,java}/`
- Acceptance: 55 template tests pass, each language has proper tooling references

---

## Sprint 9: Multi-Language CLI & Settings (Priority: P1)

### 9a. CLI Language Override ✓ COMPLETE
- [x] Add `--language` flag to bootstrap command
- [x] Support multiple `--language` flags for polyglot
- [x] Add `--detect-only` flag
- Files: `src/main.rs`, `src/bootstrap/mod.rs`
- Acceptance: CLI tests pass, language override works with aliases (ts, py, etc.)

### 9b-9d. Language Settings Templates
- [ ] Create `templates/{language}/settings.json` with safe permissions
- Files: `src/templates/{python,typescript,go}/settings.json`

---

## Sprint 10: Polyglot & Advanced Features (Priority: P1)

- [ ] Detect polyglot projects (multiple languages >10% confidence)
- [ ] Generate combined prompts with per-language sections
- [ ] Language-aware narsil-mcp config generation
- [ ] Additional language templates (Ruby, PHP, C#, generic fallback)

---

## Sprint 11: CCG-Aware Prompts (Priority: P2)

- [ ] Parse CCG constraint specifications
- [ ] Inject constraints into prompts
- [ ] Verify changes satisfy constraints

---

## Completed Sprints (Reference Only)

| Sprint | What Was Built |
|--------|----------------|
| 1 | Task tracker with state machine, IMPLEMENTATION_PLAN.md parsing |
| 2 | Prompt builder with fluent API, context injection |
| 3 | Quality gates (Clippy, Test, NoAllow, NoTodo, Security) |
| 4 | Checkpoint system with regression detection and rollback |
| 5 | narsil-mcp client with graceful degradation, CCG support |
| 6 | Language enum (32 languages), LanguageDetector with confidence scoring |
| 12 | Failure classification, task decomposition, focused retry |
| 13 | Stagnation pattern detection, risk scoring, preventive actions |

---

## Quality Gates (Must Pass Before Commit)

```
[ ] reindex                                    (start of task)
[ ] Tests written BEFORE implementation        (TDD verified)
[ ] cargo clippy --all-targets -- -D warnings  (0 warnings)
[ ] cargo test                                  (all pass)
[ ] No #[allow(...)] annotations added
[ ] No TODO/FIXME comments in new code
[ ] reindex                                    (end of task)
```

---

## Notes

- Ralph reads this file each iteration to select the next task
- Checkbox completion (`[x]`) signals progress
- Tasks are prioritized top-to-bottom within each sprint
- Reference `further_dev_plans/ralph-multilang-bootstrap.md` for detailed design

<!-- When Sprint 7 is done, update NEXT STEPS to point to Sprint 8 -->
