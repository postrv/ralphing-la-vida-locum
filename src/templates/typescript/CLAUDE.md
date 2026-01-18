# Project Memory - TypeScript Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: TypeScript Project

This is a TypeScript project. Key conventions and tools:

- **Testing**: Jest or Vitest
- **Linting**: ESLint with TypeScript plugin
- **Type Checking**: tsc (TypeScript compiler)
- **Security**: npm audit for vulnerability scanning
- **Formatting**: Prettier

---

## GIT AUTHENTICATION (CRITICAL)

### Required Setup
Ralph requires `gh` CLI for all GitHub operations. SSH key access is **blocked**.

```bash
# Verify gh CLI is authenticated
gh auth status

# If not authenticated, run:
gh auth login
```

### Rules - Non-Negotiable
1. **ALWAYS** use `gh` CLI for GitHub operations
2. **NEVER** attempt SSH key operations (ssh-keygen, ssh-add, etc.)
3. **NEVER** access ~/.ssh/ directory
4. **NEVER** use git@github.com: URLs
5. Use `gh auth status` to verify authentication
6. Use `gh repo clone` instead of `git clone git@github.com:`

---

## PRODUCTION STANDARDS (Non-Negotiable)

### Code Quality - Zero Tolerance Policy

**FORBIDDEN PATTERNS - Never Use:**
```typescript
// @ts-ignore              // Fix the type error properly
// eslint-disable          // Fix the linting issue
any                        // Use proper types
// TODO: ...               // Implement now or don't merge
// FIXME: ...              // Fix now
as any                     // Type properly instead
```

**REQUIRED PATTERNS - Always Use:**
```typescript
/**
 * Short description of function.
 *
 * @param arg - Description of argument
 * @returns Description of return value
 * @throws {ErrorType} When this happens
 *
 * @example
 * ```typescript
 * const result = functionName("input");
 * ```
 */
function functionName(arg: Type): ReturnType {
  // ...
}
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Tests FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing Jest/Vitest test that defines expected behavior
3. **GREEN**: Write minimal code to make the test pass
4. **REFACTOR**: Clean up while keeping tests green
5. **REVIEW**: Run ESLint + tsc + npm audit
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before tests, STOP:**
1. Delete the implementation
2. Write the test first
3. Then write minimal code to pass

**Test Coverage Requirements:**
- Every exported function: at least 1 unit test
- Every class: exercised in integration tests
- Every error path: tested with `expect().toThrow()`
- Every edge case: empty inputs, boundaries, null/undefined

### TypeScript Quality Tools

Run all checks before commit:
```bash
# Package manager (pick one)
npm run lint && npm run typecheck && npm test
yarn lint && yarn typecheck && yarn test
pnpm lint && pnpm typecheck && pnpm test

# Security audit
npm audit
yarn audit
pnpm audit
```

If ESLint warns about:
- `@typescript-eslint/no-unused-vars` -> Remove the unused variable
- `@typescript-eslint/no-explicit-any` -> Use proper types
- `no-console` -> Remove console statements

---

## MCP SERVERS

### narsil-mcp (Code Intelligence - Optional)

narsil-mcp is optional. All features gracefully degrade when unavailable.

**Security (run before committing, if available):**
```bash
scan_security           # Find vulnerabilities
find_injection_vulnerabilities  # SQL/XSS/command injection
check_cwe_top25        # CWE Top 25 checks
```

**Context Gathering:**
```bash
get_call_graph <function>   # Function relationships
find_references <symbol>    # Impact analysis
get_dependencies <path>     # Module dependencies
find_similar_code <query>   # Find related code
```

### Graceful Degradation Policy

When narsil-mcp is unavailable:
- Security gates are skipped (log warning)
- Code intelligence returns empty results
- Ralph continues to function normally

---

## QUALITY GATES (Pre-Commit Checklist)

**Start of Task:**
```
[ ] reindex                                    -> narsil-mcp index refreshed
```

**Mandatory (always enforced):**
```
[ ] Tests written BEFORE implementation        -> TDD verified
[ ] npm run lint (ESLint)                      -> 0 warnings
[ ] npm run typecheck (tsc)                    -> 0 errors
[ ] npm test                                   -> all pass
[ ] npm audit                                  -> 0 high/critical
[ ] No @ts-ignore without justification
[ ] No any types without justification
[ ] No TODO/FIXME comments in new code
[ ] All new exports documented (JSDoc)
[ ] All new functions have tests
[ ] gh auth status                             -> authenticated
```

**Optional (if narsil-mcp available):**
```
[ ] scan_security                              -> 0 CRITICAL/HIGH
[ ] find_injection_vulnerabilities             -> 0 findings
```

**End of Task:**
```
[ ] reindex                                    -> narsil-mcp index updated
```

---

## TYPESCRIPT-SPECIFIC CONVENTIONS

### Project Structure
```
project/
├── src/
│   ├── index.ts          # Main entry point
│   ├── types.ts          # Shared type definitions
│   ├── utils/
│   │   └── helpers.ts
│   └── modules/
│       └── feature.ts
├── tests/
│   ├── setup.ts          # Test setup
│   └── feature.test.ts
├── package.json
├── tsconfig.json
├── eslint.config.js
└── README.md
```

### Imports
```typescript
// External packages first
import express from 'express';
import { z } from 'zod';

// Internal modules second
import { Config } from './config';
import { handleError } from './utils/error';

// Types last
import type { Request, Response } from 'express';
```

### Testing with Jest/Vitest
```typescript
import { describe, it, expect, beforeEach } from 'vitest';
// or: import { describe, it, expect, beforeEach } from '@jest/globals';

describe('Feature', () => {
  beforeEach(() => {
    // Setup before each test
  });

  it('should handle basic case', () => {
    const result = functionUnderTest('input');
    expect(result).toBe(expected);
  });

  it('should handle edge case', () => {
    const result = functionUnderTest('');
    expect(result).toBeUndefined();
  });

  it('should throw on invalid input', () => {
    expect(() => functionUnderTest(null as any)).toThrow('invalid');
  });

  it.each([
    ['a', 1],
    ['b', 2],
  ])('should handle %s and return %i', (input, expected) => {
    expect(functionUnderTest(input)).toBe(expected);
  });
});
```

---

## QUICK REFERENCE

```bash
# Install dependencies
npm install
yarn install
pnpm install

# Run tests
npm test
yarn test
pnpm test

# Lint and fix
npm run lint -- --fix
yarn lint --fix

# Type check
npm run typecheck
yarn typecheck

# Security audit
npm audit
yarn audit

# Verify git environment
gh auth status
```
