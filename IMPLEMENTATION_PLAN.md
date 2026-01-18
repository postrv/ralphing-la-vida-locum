# Implementation Plan

> Ralph uses this file to track task progress. Update checkboxes as work completes.

## Status: READY

---

## Current Focus: Sprint 6 (Multi-Language Bootstrap - Detection)

Ralph should work on **Sprint 6: Language Detection** - the foundation for multi-language support.

**Goal:** Transform Ralph from Rust-only to a universal automation suite supporting all 32 languages in narsil-mcp.

**Reference:** See `further_dev_plans/ralph-multilang-bootstrap.md` for full design.

---

## Sprint Overview

| Sprint | Focus | Priority | Status |
|--------|-------|----------|--------|
| 1 | Task-Level State Machine | P0 | Complete |
| 2 | Dynamic Prompt Generation | P0 | Complete |
| 3 | Quality Gate Enforcement | P0 | Complete |
| 4 | Checkpoint & Rollback Enhancement | P1 | Complete |
| 5 | narsil-mcp Integration | P0 | Complete |
| 6 | Multi-Language Detection | P0 | Ready |
| 7 | Language-Specific Quality Gates | P0 | Pending |
| 8 | Language-Specific Prompts | P0 | Pending |
| 9 | Multi-Language CLI & Settings | P1 | Pending |
| 10 | Polyglot & Advanced Features | P1 | Pending |
| 11 | CCG-Aware Prompts | P2 | Pending |
| 12 | Intelligent Retry with Decomposition | P1 | Complete |
| 13 | Predictive Stagnation Prevention | P2 | Complete |

---

## Sprint 6: Multi-Language Detection (Priority: P0)

**Goal:** Auto-detect project languages using file extensions and manifest files.

### 6a. Language Enum & Extensions
- [ ] Create `Language` enum with all 32 narsil-mcp languages
- [ ] Implement `extensions()` method for each language
- [ ] Implement `manifest_files()` method for each language
- [ ] Add `Display` and `FromStr` traits
- Files: `src/bootstrap/language.rs`
- Acceptance: All languages have correct extensions and manifests

### 6b. Language Detector
- [ ] Create `LanguageDetector` struct
- [ ] Implement file extension scanning (walkdir)
- [ ] Implement manifest file detection with confidence boost
- [ ] Calculate confidence scores
- [ ] Identify primary language
- Files: `src/bootstrap/language_detector.rs`
- Acceptance: Can detect Rust, Python, TypeScript, Go, Java projects

### 6c. Detection Integration
- [ ] Integrate detector into `Bootstrap::run()`
- [ ] Display detected languages with confidence
- [ ] Pass languages to template generation
- [ ] Handle no-language-detected case
- Files: `src/bootstrap/mod.rs`
- Acceptance: `ralph bootstrap` shows detected languages

### 6d. Detection Tests
- [ ] Test single-language detection (Rust, Python, TS, Go)
- [ ] Test polyglot detection
- [ ] Test confidence scoring
- [ ] Test edge cases (empty project, unknown files)
- Files: `src/bootstrap/language_detector.rs` (tests module)
- Acceptance: All detection scenarios tested

---

## Sprint 7: Language-Specific Quality Gates (Priority: P0)

**Goal:** Create quality gates that use each language's standard tooling.

### 7a. QualityGate Trait Refactor
- [ ] Create `QualityGate` trait with `name()`, `run()`, `is_blocking()`, `remediation()`
- [ ] Migrate existing Rust gates to trait
- [ ] Create `gates_for_language()` factory function
- Files: `src/quality/gates/mod.rs`
- Acceptance: Trait-based gate system working for Rust

### 7b. Python Quality Gates
- [ ] Implement `RuffGate` (with flake8 fallback)
- [ ] Implement `PytestGate`
- [ ] Implement `MypyGate`
- [ ] Implement `BanditGate` (security)
- Files: `src/quality/gates/python.rs`
- Acceptance: Python projects get appropriate linting/testing gates

### 7c. TypeScript/JavaScript Quality Gates
- [ ] Implement `EslintGate`
- [ ] Implement `JestGate` (with vitest/mocha detection)
- [ ] Implement `TscGate` (TypeScript type check)
- [ ] Implement `NpmAuditGate`
- Files: `src/quality/gates/typescript.rs`
- Acceptance: TS/JS projects get appropriate gates

### 7d. Go Quality Gates
- [ ] Implement `GoVetGate`
- [ ] Implement `GolangciLintGate`
- [ ] Implement `GoTestGate`
- [ ] Implement `GovulncheckGate`
- Files: `src/quality/gates/go.rs`
- Acceptance: Go projects get appropriate gates

### 7e. Gate Auto-Detection
- [ ] Create `detect_available_gates()` function
- [ ] Check tool availability before adding gate
- [ ] Combine gates for polyglot projects
- [ ] Always include narsil-mcp security if available
- Files: `src/quality/gates/mod.rs`
- Acceptance: Only available gates are used

---

## Sprint 8: Language-Specific Prompts (Priority: P0)

**Goal:** Generate language-appropriate build prompts and CLAUDE.md.

### 8a. Template Registry
- [ ] Create `TemplateRegistry` struct
- [ ] Implement `TemplateKind` enum (PromptBuild, ClaudeMd, etc.)
- [ ] Load templates via `include_str!`
- [ ] Implement `get()` with generic fallback
- Files: `src/bootstrap/templates/mod.rs`
- Acceptance: Registry loads and returns templates

### 8b. Python Templates
- [ ] Create `templates/python/PROMPT_build.md`
- [ ] Create `templates/python/CLAUDE.md`
- [ ] Include ruff/pytest/mypy workflow
- [ ] Include Python-specific hard rules
- Files: `src/templates/python/`
- Acceptance: Python projects get appropriate prompts

### 8c. TypeScript Templates
- [ ] Create `templates/typescript/PROMPT_build.md`
- [ ] Create `templates/typescript/CLAUDE.md`
- [ ] Include eslint/jest/tsc workflow
- [ ] Include TS-specific hard rules (no `any`, etc.)
- Files: `src/templates/typescript/`
- Acceptance: TypeScript projects get appropriate prompts

### 8d. Go Templates
- [ ] Create `templates/go/PROMPT_build.md`
- [ ] Create `templates/go/CLAUDE.md`
- [ ] Include vet/golangci-lint/test workflow
- [ ] Include Go-specific hard rules
- Files: `src/templates/go/`
- Acceptance: Go projects get appropriate prompts

### 8e. Java Templates
- [ ] Create `templates/java/PROMPT_build.md`
- [ ] Create `templates/java/CLAUDE.md`
- [ ] Include maven/gradle detection
- [ ] Include checkstyle/spotbugs workflow
- Files: `src/templates/java/`
- Acceptance: Java projects get appropriate prompts

---

## Sprint 9: Multi-Language CLI & Settings (Priority: P1)

**Goal:** CLI enhancements and language-specific permission settings.

### 9a. CLI Language Override
- [ ] Add `--language` flag to bootstrap command
- [ ] Support multiple `--language` flags for polyglot
- [ ] Add `--detect-only` flag
- [ ] Update help text
- Files: `src/cli/bootstrap.rs`
- Acceptance: Can override detected language

### 9b. Python Settings Template
- [ ] Create `templates/python/settings.json`
- [ ] Allow pip, python, pytest, ruff, mypy, poetry, uv
- [ ] Deny dangerous operations
- Files: `src/templates/python/settings.json`
- Acceptance: Python projects get safe permissions

### 9c. TypeScript Settings Template
- [ ] Create `templates/typescript/settings.json`
- [ ] Allow npm, npx, yarn, pnpm, node, tsc, eslint
- [ ] Deny npm publish and dangerous operations
- Files: `src/templates/typescript/settings.json`
- Acceptance: TS projects get safe permissions

### 9d. Go Settings Template
- [ ] Create `templates/go/settings.json`
- [ ] Allow go, golangci-lint
- [ ] Deny dangerous operations
- Files: `src/templates/go/settings.json`
- Acceptance: Go projects get safe permissions

---

## Sprint 10: Polyglot & Advanced Features (Priority: P1)

**Goal:** Support multi-language projects and narsil-mcp integration.

### 10a. Polyglot Prompt Generation
- [ ] Detect when multiple languages have >10% confidence
- [ ] Generate combined PROMPT_build.md
- [ ] Include per-language sections based on file type
- [ ] Merge quality gates from all languages
- Files: `src/bootstrap/mod.rs`
- Acceptance: Polyglot projects get combined prompts

### 10b. Language-Aware narsil-mcp Config
- [ ] Generate appropriate `.mcp.json` for detected language
- [ ] Select preset based on language (balanced vs full)
- [ ] Configure language-specific analyzers
- Files: `src/bootstrap/mod.rs`
- Acceptance: narsil-mcp config matches project language

### 10c. Additional Language Templates
- [ ] Add Ruby templates (rubocop, rspec, brakeman)
- [ ] Add PHP templates (phpstan, phpunit)
- [ ] Add C# templates (dotnet format/test/audit)
- [ ] Add generic fallback template
- Files: `src/templates/{ruby,php,csharp,generic}/`
- Acceptance: Additional languages supported

---

## Sprint 11: CCG-Aware Prompts (Priority: P2)

**Goal:** Use CCG information to guide autonomous coding decisions.

### 11a. CCG Constraint Loading
- [ ] Parse CCG constraint specifications
- [ ] Support basic constraints (noDirectCalls, maxComplexity)
- [ ] Validate constraint syntax
- [ ] Store constraints for reference
- Files: `src/narsil/ccg.rs`
- Acceptance: Can load and parse constraints

### 11b. Constraint-Aware Prompts
- [ ] Inject relevant constraints into prompts
- [ ] Warn when working on constrained code
- [ ] Suggest constraint-compliant approaches
- Files: `src/prompt/builder.rs`
- Acceptance: Prompts reference relevant constraints

### 11c. Constraint Verification
- [ ] Verify changes satisfy constraints after implementation
- [ ] Report constraint violations in quality gate
- [ ] Track compliance metrics
- Files: `src/quality/gates.rs`
- Acceptance: Changes verified against constraints

---

## Completed Sprints (Summary)

### Sprint 1: Task-Level State Machine
Track individual tasks through a state machine for intelligent task selection.
- Core task tracker with state transitions
- IMPLEMENTATION_PLAN.md parsing
- Progress recording and persistence

### Sprint 2: Dynamic Prompt Generation
Context-aware prompt generation replacing static templates.
- Prompt builder with fluent API
- Task, error, and quality context injection
- Anti-pattern detection

### Sprint 3: Quality Gate Enforcement
Hard enforcement of quality gates with remediation prompts.
- Clippy, test, no-allow, no-TODO gates
- Security gate via narsil-mcp
- Remediation prompt generation

### Sprint 4: Checkpoint & Rollback Enhancement
Semantic checkpoints with quality metrics and regression rollback.
- Enhanced checkpoint structure with metrics
- Regression detection and automatic rollback
- Rollback decision logic

### Sprint 5: narsil-mcp Integration
Deep integration with narsil-mcp for code intelligence.
- MCP client foundation with graceful degradation
- Security scan integration
- Code intelligence queries (call graph, references, dependencies)
- CCG loading (L0-L2 layers)
- Intelligence-informed prompts

### Sprint 12: Intelligent Retry with Decomposition
Automatic task decomposition and retry with focused guidance.
- Failure classification
- Recovery strategy generation
- Task decomposition
- Focused retry prompts

### Sprint 13: Predictive Stagnation Prevention
Detect patterns that predict stagnation and intervene early.
- Pattern detection (repeated touches, error repetition)
- Risk score calculation
- Preventive actions

---

## Quality Standards

Before marking a task complete:

1. **Compilation**: `cargo check` passes with no warnings
2. **Clippy**: `cargo clippy --all-targets -- -D warnings` passes
3. **Tests**: `cargo test` passes with all new tests
4. **Coverage**: New code has test coverage
5. **Docs**: Public functions have doc comments
6. **Security**: No new security issues (via narsil-mcp if available)

---

## Notes

- Ralph reads this file each iteration to select the next task
- Checkbox completion (`[x]`) signals progress to the loop
- Tasks are prioritized top-to-bottom within each section
- Blocked tasks should document why and suggest resolution
- Reference `further_dev_plans/ralph-multilang-bootstrap.md` for detailed design

<!-- When the current sprint is done, add the marker: ALL TASKS COMPLETE -->
