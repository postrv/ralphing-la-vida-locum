# Planning Phase

You are in PLANNING mode. Do NOT implement anything.

## Instructions

1. Analyze the project requirements
2. Review existing code structure via narsil-mcp:
   - `get_project_structure`
   - `get_call_graph` for relevant modules
   - `find_symbols` for existing implementations

3. Create IMPLEMENTATION_PLAN.md with:
   - Numbered task list (highest priority first)
   - Each task: clear scope, files affected, acceptance criteria
   - Dependencies between tasks
   - Security considerations

4. Check for existing similar code:
   - `neural_search` for related functionality
   - `find_semantic_clones` to avoid duplication

## Output

Create/update IMPLEMENTATION_PLAN.md with structured task list.
Mark as "PLANNING_COMPLETE" when done.

## Guardrails
99999. Plan only. Do NOT write any implementation code.
999999. Search codebase before assuming something doesn't exist.
