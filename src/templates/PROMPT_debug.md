# Debug Phase - Autonomous Unsticking

**Mode**: AUTONOMOUS - DO NOT ASK QUESTIONS
**Status**: Stagnation detected - execute recovery protocol

## IMMEDIATE ACTIONS (Execute in order)

### Step 1: Diagnose State
Run these commands and analyze output:
```bash
cargo test 2>&1 | head -50
cargo clippy --all-targets -- -D warnings 2>&1 | head -30
git status --short
git log --oneline -5
```

### Step 2: Identify and Fix ONE Issue
Based on Step 1 output:
- If tests fail → Fix the failing test (write minimal fix)
- If clippy warns → Fix the clippy warning
- If nothing fails → Check PROMPT_build.md for next task
- If task unclear → Run `cargo test` to verify clean state, then commit

### Step 3: Commit and Resume
After fixing ONE issue:
```bash
git add -A
git commit -m "fix: [describe what was fixed]"
```

Then continue with the next task from PROMPT_build.md.

## Anti-Stagnation Rules

1. **DO NOT** output summaries without making changes
2. **DO NOT** ask what to work on - just execute
3. **DO NOT** wait for user input
4. **ALWAYS** make at least one commit per iteration
5. **ALWAYS** update IMPLEMENTATION_PLAN.md with progress

## If Truly Blocked

If the same issue persists after 3 attempts:
1. Document the blocker in IMPLEMENTATION_PLAN.md:
   ```markdown
   ## BLOCKED - [issue]
   - Attempts: [what was tried]
   - Error: [exact error message]
   - Needs: [human intervention or external dependency]
   ```
2. Mark the task as blocked and move to the next task
3. Continue working on other tasks

## Quality Gates

Before ANY commit:
```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

Both must pass. If they don't, fix them first.
