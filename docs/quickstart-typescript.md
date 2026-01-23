# Quick Start: TypeScript Projects

Get Ralph up and running with your TypeScript project in 5 minutes.

## Prerequisites

Before starting, ensure you have:

- **Ralph** installed ([installation guide](../README.md#installation))
- **Node.js 18+** with npm, yarn, or pnpm
- **GitHub CLI** authenticated (`gh auth status`)
- **Claude Code 2.1.0+** for autonomous execution

### Recommended TypeScript Tools

Ralph works best with these tools configured in your project:

```bash
# Install recommended dev dependencies
npm install -D typescript eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin
npm install -D vitest  # or jest

# Verify installation
npx tsc --version     # TypeScript compiler
npx eslint --version  # Linting
```

## Step 1: Bootstrap Your Project

Navigate to your TypeScript project and run the bootstrap command:

```bash
cd /path/to/your-typescript-project
ralph --project . bootstrap
```

**Expected output:**

```
Bootstrapping project: /path/to/your-typescript-project
  Detected languages:
    → TypeScript (primary)    # 95% confidence
  Creating .claude/ directory
  Creating docs/ directory
  Writing CLAUDE.md
  Writing settings.json
  Writing IMPLEMENTATION_PLAN.md
  Writing PROMPT_build.md
Bootstrap complete!
```

Ralph auto-detects TypeScript from `tsconfig.json`, `package.json` (with TypeScript deps), or `.ts`/`.tsx` files.

## Step 2: Verify Detection

Check that Ralph correctly detected your project:

```bash
ralph --project . detect
```

**Expected output:**

```
Detected languages:
  → TypeScript (primary)      # Based on tsconfig.json
```

## Step 3: Create Your Implementation Plan

Edit `IMPLEMENTATION_PLAN.md` with your tasks:

```markdown
# Implementation Plan

## Current Sprint

### Phase 1: Core Components
- [ ] Create UserCard component with props interface
- [ ] Add loading and error states
- [ ] Write unit tests with Vitest/Jest
- [ ] Style with Tailwind CSS

### Phase 2: API Integration
- [ ] Create api client with fetch wrapper
- [ ] Add type-safe API response handling
- [ ] Write integration tests
- [ ] Add error boundary component
```

## Step 4: Run the Loop

Start Ralph's autonomous execution loop:

```bash
# Plan phase: let Claude analyze and plan the implementation
ralph --project . loop --phase plan --max-iterations 5

# Build phase: autonomous coding
ralph --project . loop --phase build --max-iterations 20

# With verbose output for debugging
ralph --verbose --project . loop --phase build --max-iterations 10
```

## Quality Gates

Ralph enforces these quality gates for TypeScript projects:

| Gate | Command | Requirement |
|------|---------|-------------|
| **Lint** | `npm run lint` | 0 warnings |
| **Types** | `npm run typecheck` | 0 errors |
| **Tests** | `npm test` | All pass |
| **Security** | `npm audit` | 0 high/critical |

Ralph will not commit code that fails any gate.

### Required package.json Scripts

Ensure your `package.json` has these scripts:

```json
{
  "scripts": {
    "lint": "eslint . --ext .ts,.tsx",
    "typecheck": "tsc --noEmit",
    "test": "vitest run"
  }
}
```

## Project Structure

After bootstrap, your project will have:

```
your-typescript-project/
├── .claude/
│   ├── CLAUDE.md           # Project memory for Claude
│   ├── settings.json       # Permissions and hooks
│   ├── mcp.json            # MCP server configuration
│   ├── skills/             # Custom skills
│   └── agents/             # Subagents
├── docs/
│   ├── architecture.md     # Architecture template
│   └── api.md              # API documentation template
├── IMPLEMENTATION_PLAN.md  # Your task list
├── PROMPT_build.md         # Build phase prompt
├── PROMPT_plan.md          # Plan phase prompt
└── PROMPT_debug.md         # Debug phase prompt
```

## Example: Next.js Project

Here's a complete example for a Next.js project:

### 1. Create Project

```bash
npx create-next-app@latest my-nextjs-app --typescript --tailwind --eslint
cd my-nextjs-app
npm install -D vitest @testing-library/react @testing-library/jest-dom
```

### 2. Add Test Script

Update `package.json`:

```json
{
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "lint": "next lint",
    "typecheck": "tsc --noEmit",
    "test": "vitest run"
  }
}
```

### 3. Bootstrap

```bash
ralph --project . bootstrap
```

### 4. Implementation Plan

Create `IMPLEMENTATION_PLAN.md`:

```markdown
# Next.js Dashboard

## Sprint 1: Authentication

### Phase 1.1: Auth Components
- [ ] Create LoginForm component with validation
- [ ] Add useAuth hook for auth state
- [ ] Write tests for LoginForm
- [ ] Style with Tailwind

### Phase 1.2: API Routes
- [ ] POST /api/auth/login - authenticate user
- [ ] POST /api/auth/logout - clear session
- [ ] GET /api/auth/me - get current user
- [ ] Write integration tests for auth flow
```

### 5. Run

```bash
ralph --project . loop --phase build --max-iterations 15
```

## Example: Express API Project

Here's an example for a backend Express API:

### 1. Create Project

```bash
mkdir my-express-api && cd my-express-api
npm init -y
npm install express zod
npm install -D typescript @types/node @types/express
npm install -D eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin
npm install -D vitest supertest @types/supertest
npx tsc --init
```

### 2. Configure tsconfig.json

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "./dist",
    "rootDir": "./src"
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

### 3. Add Scripts

```json
{
  "scripts": {
    "build": "tsc",
    "lint": "eslint src --ext .ts",
    "typecheck": "tsc --noEmit",
    "test": "vitest run"
  }
}
```

### 4. Bootstrap and Run

```bash
ralph --project . bootstrap
ralph --project . loop --phase build --max-iterations 15
```

## Troubleshooting

### "No TypeScript detected"

Ensure you have at least one of:
- `tsconfig.json`
- `package.json` with `typescript` in dependencies
- `*.ts` or `*.tsx` files in the project

### "ESLint not found"

Install ESLint with TypeScript support:

```bash
npm install -D eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin
```

Create `eslint.config.js`:

```javascript
import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
);
```

### "typecheck script not found"

Add to `package.json`:

```json
{
  "scripts": {
    "typecheck": "tsc --noEmit"
  }
}
```

### "Tests not running"

Ensure Vitest or Jest is configured:

**Vitest** (recommended):
```bash
npm install -D vitest
```

Add `vitest.config.ts`:
```typescript
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
  },
});
```

**Jest**:
```bash
npm install -D jest ts-jest @types/jest
npx ts-jest config:init
```

## Next Steps

- Read the [full documentation](../README.md)
- Explore [checkpoint and rollback](../README.md#checkpoint--rollback)
- Learn about [narsil-mcp integration](../README.md#narsil-mcp-integration)
- Set up [custom quality gates](./developing-gates.md)
