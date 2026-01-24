# Implementation Plan

> Ralph Task Tracker Stability Fixes (Sprint 20)

---

## Current Sprint

**Goal**: Fix task tracker stability issues that cause Ralph to get stuck on orphaned/stale tasks after session restart

### 1. Phase 20.1: Startup Plan Validation

**Description**: Add validation at loop startup to detect and clear orphaned tasks before the first iteration runs.

**Requirements**:
- [x] Add `validate_on_startup()` method to TaskTracker that marks orphaned tasks and clears stale current_task
- [x] Add `clear_current_task_if_orphaned()` method to TaskTracker
- [x] Call startup validation in LoopManager::run() before the main loop begins
- [x] Add tests for startup validation behavior

**Quality Gates**:
```bash
cargo test --lib task_tracker
cargo clippy --all-targets -- -D warnings
```

### 2. Phase 20.2: Defensive Task Selection

**Description**: Ensure select_next_task() skips orphaned tasks through the orphan flag system.

**Requirements**:
- [x] Add method to check if a task title exists in plan content (`task_exists_in_plan` - test helper)
- [x] select_next_task() already skips orphaned tasks; defensive check via `validate_on_startup()` before first iteration
- [x] Add tests for defensive task selection (4 tests verifying orphan flag behavior)

**Quality Gates**:
```bash
cargo test --lib task_tracker
cargo clippy --all-targets -- -D warnings
```

### 3. Phase 20.3: Integration & Documentation

**Description**: End-to-end testing and documentation of the stability improvements.

**Requirements**:
- [x] Add integration test simulating session restart with changed plan (`test_select_next_task_uses_orphan_flag_correctly`)
- [x] Update module documentation with startup validation behavior (added "Startup Validation & Orphan Detection" section)
- [x] Run full test suite and clippy (2195 tests passing, 0 warnings)

**Quality Gates**:
```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

---

## Notes for Claude

- Follow TDD: Write failing test first, then minimal implementation, then refactor
- Run quality gates before committing
- One task at a time: Complete current task before starting next
- Mark checkboxes as you complete requirements

---

## Completed

> Move completed tasks here or to a separate archive file.
