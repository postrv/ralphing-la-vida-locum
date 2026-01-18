# Implementation Plan

> Ralph uses this file to track task progress. Update checkboxes as work completes.

## Status: READY

---

## NEXT STEPS (Start Here)

**Ralph, do this NOW:**

1. **`reindex`** - Refresh narsil-mcp index before starting
2. **Review current sprint** - Check for incomplete tasks below
3. **Follow TDD** - Write failing tests FIRST, then implement, then commit
4. **`reindex`** - Refresh narsil-mcp index after completing

**Current task:** _Define your first sprint below_

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

## Sprint 1: [Your Sprint Name]

**Goal:** _Describe the sprint goal here_

### 1a. [Task Name]
- [ ] First subtask
- [ ] Second subtask
- [ ] Third subtask
- Files: `src/path/to/files.rs`
- Acceptance: _Describe what success looks like_

### 1b. [Task Name]
- [ ] First subtask
- [ ] Second subtask
- Files: `src/path/to/files.rs`
- Acceptance: _Describe what success looks like_

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
- Add new sprints as needed for your project

---

## Completed Sprints (Reference Only)

| Sprint | What Was Built |
|--------|----------------|
| - | _Move completed sprints here_ |
