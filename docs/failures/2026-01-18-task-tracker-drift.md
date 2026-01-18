# Failure Mode: Task Tracker Drift

**Date:** 2026-01-18
**Severity:** HIGH
**Impact:** Loop executed work outside current sprint plan

---

## Summary

Ralph's autonomous loop implemented enterprise features from orphaned tasks in `.ralph/task_tracker.json` instead of following the sprint structure in `IMPLEMENTATION_PLAN.md`.

---

## Root Cause Analysis

### The Problem

1. **Task Selection by Number, Not Sprint**
   - Ralph's task selector chose tasks based on `task.id.number` field
   - The tracker accumulated 53 tasks from previous IMPLEMENTATION_PLAN.md versions
   - Tasks like "RBAC" (number 4) were selected before "Sprint 7: QualityGate Trait Refactor" (number 7a)

2. **Orphaned Tasks Not Cleared**
   - When IMPLEMENTATION_PLAN.md was updated for multi-language bootstrap, old tasks remained
   - Task tracker retained: Orchestrator Foundation, Team Management, SSO Integration, Campaign Management, RBAC, Audit Logging, Reporting, Quality Certification, narsil-cloud Integration
   - These were enterprise-tier features from an older roadmap

3. **No Sprint Affiliation in Task IDs**
   - `TaskId` struct captures `number`, `phase`, `title`, `original`
   - No field for sprint number or section
   - Task selection logic cannot distinguish Sprint 6 tasks from orphaned tasks

### Timeline of Events

1. **01:35-03:17 UTC** - Loop worked on legitimate core tasks (Task Tracker, MCP Client, Prompt Builder, etc.)
2. **09:58-10:47 UTC** - Sprint 6 work (Language enum, LanguageDetector, Bootstrap integration) - CORRECT
3. **10:47-11:12 UTC** - Loop switched to orphaned enterprise tasks:
   - RBAC (commit 4d38774)
   - Audit Logging (commit 6e70508)
   - Reporting (commit c7c84cd)
   - Quality Certification (commit 04d1a42)
4. **11:45 UTC** - CCG constraint system (Sprint 11 work done early - commit 2fabdbf)
5. **12:25-13:22 UTC** - Stagnation: loop couldn't find more orphaned tasks, repeatedly verified completed work

---

## Impact Assessment

### Unplanned Code Added (5,211 lines total)

| Module | File(s) | Lines | Tests |
|--------|---------|-------|-------|
| RBAC | src/enterprise/rbac.rs | 929 | 36 |
| Audit Logging | src/analytics.rs (partial) | ~500 | 16 |
| Reporting | src/reporting/generator.rs, export.rs, mod.rs | 1,298 | 25 |
| Quality Certification | src/reporting/certification.rs | 1,072 | 37 |
| CCG Constraints | src/narsil/ccg.rs (partial) | ~1,305 | ~40 |

### Planned Work NOT Done

- **Sprint 7**: Language-Specific Quality Gates (0% complete)
  - 7a: QualityGate Trait Refactor
  - 7b: Python Quality Gates
  - 7c: TypeScript/JavaScript Quality Gates
  - 7d: Go Quality Gates
  - 7e: Gate Auto-Detection

### Positive Side Effects

- Test count increased: 1,006 → 1,279 (+273 tests)
- All quality gates passed (clippy clean, tests passing)
- Code is production-quality (follows TDD, no warnings)
- Enterprise features are well-implemented and could be valuable in separate repo

---

## Recommended Fixes for Ralph

### Fix 1: Sprint-Aware Task Selection (Critical)

Add sprint affiliation to TaskId:

```rust
pub struct TaskId {
    pub number: u32,
    pub phase: Option<String>,
    pub title: String,
    pub original: String,
    pub sprint: Option<u32>,  // NEW: Sprint number this task belongs to
    pub subsection: Option<String>,  // NEW: e.g., "6a", "7b"
}
```

Task selection should:
1. Read current sprint from IMPLEMENTATION_PLAN.md "Current Focus" section
2. Only select tasks from that sprint
3. Fall back to next sprint only when current sprint is complete

### Fix 2: Task Tracker Invalidation (High Priority)

When IMPLEMENTATION_PLAN.md changes significantly:
1. Hash the task structure (section headers, task titles)
2. Compare with stored hash in task_tracker.json
3. If hash differs, warn user and offer to reset tracker
4. Never silently carry forward orphaned tasks

```rust
pub struct TaskTracker {
    tasks: HashMap<TaskId, Task>,
    plan_hash: String,  // NEW: Hash of IMPLEMENTATION_PLAN.md structure
    // ...
}

impl TaskTracker {
    pub fn validate_against_plan(&self, plan: &str) -> ValidationResult {
        let current_hash = Self::compute_plan_hash(plan);
        if current_hash != self.plan_hash {
            ValidationResult::PlanChanged { orphaned_tasks: self.find_orphaned() }
        } else {
            ValidationResult::Valid
        }
    }
}
```

### Fix 3: Orphaned Task Detection (High Priority)

On each loop iteration:
1. Parse IMPLEMENTATION_PLAN.md for all task headers
2. Compare with tasks in tracker
3. Mark tasks not found in plan as `Orphaned`
4. Skip orphaned tasks in selection
5. Log warning about orphaned tasks

### Fix 4: Sprint Completion Gate (Medium Priority)

Before selecting any task:
1. Check if current sprint has incomplete tasks
2. If yes, only select from current sprint
3. If current sprint complete, update "Current Focus" in IMPLEMENTATION_PLAN.md
4. Then proceed to next sprint

---

## Immediate Actions

### Action 1: Reset Task Tracker

```bash
rm .ralph/task_tracker.json
# Loop will recreate from IMPLEMENTATION_PLAN.md on next run
```

### Action 2: Update IMPLEMENTATION_PLAN.md

Change "Current Focus" from Sprint 6 to Sprint 7.

### Action 3: Extract Enterprise Features

Create separate `ralph-enterprise` private repo with:
- src/enterprise/ (RBAC)
- src/reporting/ (Reporting, Certification)
- Audit logging additions from src/analytics.rs

### Action 4: Revert Enterprise Commits from This Repo

Revert these commits in order:
1. 32cbaf7 - can_commit() tests (keep - legitimate)
2. 2fabdbf - CCG constraints (decide: keep as Sprint 11 early work or revert)
3. 18cd9c2 - doctest fix (keep - legitimate fix)
4. 04d1a42 - Quality certification (REVERT)
5. 4d38774 - RBAC (REVERT)
6. c7c84cd - Reporting (REVERT)
7. 6e70508 - Audit logging (REVERT)

Keep Sprint 6 commits:
- e99cc1f - Sprint 6c
- c25c3b1 - Sprint 6b
- 2f447d1 - Sprint 6a

---

## Prevention Checklist

- [ ] Implement sprint-aware task selection
- [ ] Add plan hash validation to task tracker
- [ ] Add orphaned task detection
- [ ] Add sprint completion gate
- [ ] Add warning when task tracker contains tasks not in plan
- [ ] Document task tracker reset procedure

---

## Lessons Learned

1. **Task selection must be sprint-scoped** - Numeric task IDs are ambiguous across plan versions
2. **Task tracker needs plan synchronization** - Stale tasks cause drift
3. **Sprint completion must gate progression** - Don't auto-advance to unrelated work
4. **Enterprise features need explicit roadmap** - They appeared from an older vision document

---

**Filed by:** Claude Code (post-incident analysis)
**Status:** Documented, fixes proposed
