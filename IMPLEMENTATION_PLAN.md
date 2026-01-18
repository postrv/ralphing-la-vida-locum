# Implementation Plan

> Ralph uses this file to track task progress. Update checkboxes as work completes.

## Status: READY

---

## Current Focus: Sprint 6 (CCG-Aware Prompts)

Ralph should work on **Sprint 6: CCG-Aware Prompts** - extending CCG integration.

**Prerequisites:**
- Sprint 5 (narsil-mcp Integration) complete
- narsil-mcp available with `--features graph` for CCG support

---

## Sprint Overview

| Sprint | Focus | Priority | Status |
|--------|-------|----------|--------|
| 1 | Task-Level State Machine | P0 | Complete |
| 2 | Dynamic Prompt Generation | P0 | Complete |
| 3 | Quality Gate Enforcement | P0 | Complete |
| 4 | Checkpoint & Rollback Enhancement | P1 | Complete |
| 5 | narsil-mcp Integration | P0 | Complete |
| 6 | CCG-Aware Prompts | P1 | Ready |
| 7 | Intelligent Retry with Decomposition | P1 | Complete |
| 8 | Predictive Stagnation Prevention | P2 | Complete |

---

## Sprint 6: CCG-Aware Prompts (Priority: P1)

**Goal:** Use CCG information to guide autonomous coding decisions.

### 6a. CCG Constraint Loading
- [ ] Parse CCG constraint specifications
- [ ] Support basic constraints (noDirectCalls, maxComplexity)
- [ ] Validate constraint syntax
- [ ] Store constraints for reference
- Files: `src/narsil/ccg.rs`
- Acceptance: Can load and parse constraints

### 6b. Constraint-Aware Prompts
- [ ] Inject relevant constraints into prompts
- [ ] Warn when working on constrained code
- [ ] Suggest constraint-compliant approaches
- Files: `src/prompt/builder.rs`
- Acceptance: Prompts reference relevant constraints

### 6c. Constraint Verification
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

### Sprint 7: Intelligent Retry with Decomposition
Automatic task decomposition and retry with focused guidance.
- Failure classification
- Recovery strategy generation
- Task decomposition
- Focused retry prompts

### Sprint 8: Predictive Stagnation Prevention
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

<!-- When the current sprint is done, add the marker: ALL TASKS COMPLETE -->
