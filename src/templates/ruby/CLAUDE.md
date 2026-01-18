# Project Memory - Ruby Automation Suite Enabled

## Environment
- Claude Code 2.1.0+ with skill hot-reload
- Opus 4.5 exclusively (Max x20)
- narsil-mcp for code intelligence and security
- Ralph automation suite for autonomous execution

## Current Work: Ruby Project

This is a Ruby project. Key conventions and tools:

- **Testing**: RSpec with describe/context/it blocks
- **Linting**: RuboCop with community style guide
- **Security**: Brakeman (Rails), Bundler-Audit
- **Formatting**: RuboCop auto-correct

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
```ruby
# rubocop:disable             # Fix the issue properly
# TODO: ...                   # Implement now or don't merge
# FIXME: ...                  # Fix now
raise 'not implemented'       # Implement or remove
fail 'unimplemented'          # Complete it
```

**REQUIRED PATTERNS - Always Use:**
```ruby
# @param name [String] Description of parameter
# @return [Boolean] Description of return value
# @raise [ArgumentError] When invalid input provided
def method_name(name)
  # Implementation
end
```

### Test-Driven Development (MANDATORY - NO EXCEPTIONS)

**Specs FIRST. Implementation SECOND. Always.**

**Every change follows this cycle:**
1. **REINDEX**: `reindex` - refresh narsil-mcp index before starting
2. **RED**: Write a failing RSpec spec that defines expected behavior
3. **GREEN**: Write minimal code to make the spec pass
4. **REFACTOR**: Clean up while keeping specs green
5. **REVIEW**: Run RuboCop + Brakeman
6. **COMMIT**: Only if ALL quality gates pass
7. **REINDEX**: `reindex` - refresh narsil-mcp index after completing

**If you write implementation before specs, STOP:**
1. Delete the implementation
2. Write the spec first
3. Then write minimal code to pass

**Spec Coverage Requirements:**
- Every public method: at least 1 spec
- Every class: exercised in integration specs
- Every error path: tested with `expect { }.to raise_error`
- Every edge case: empty inputs, boundaries, nil values

### Ruby Quality Tools

Run all checks before commit:
```bash
# Linting and auto-fix
bundle exec rubocop -a

# Run specs
bundle exec rspec

# Security (Rails projects)
bundle exec brakeman -q

# Dependency vulnerabilities
bundle audit check
```

If RuboCop warns about:
- `Style/FrozenStringLiteralComment` -> Add the magic comment
- `Lint/UnusedMethodArgument` -> Remove or use the argument
- `Metrics/MethodLength` -> Extract smaller methods

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
get_call_graph <method>     # Method relationships
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
[ ] Specs written BEFORE implementation        -> TDD verified
[ ] bundle exec rubocop                        -> 0 warnings
[ ] bundle exec rspec                          -> all pass
[ ] bundle audit check                         -> 0 vulnerabilities
[ ] No # rubocop:disable without justification
[ ] No TODO/FIXME comments in new code
[ ] All new public methods documented (YARD)
[ ] All new methods have specs
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

## RUBY-SPECIFIC CONVENTIONS

### Project Structure
```
project/
├── lib/
│   └── project/
│       ├── version.rb
│       ├── main.rb
│       └── ...
├── spec/
│   ├── spec_helper.rb
│   ├── project/
│   │   ├── main_spec.rb
│   │   └── ...
│   └── support/
├── Gemfile
├── Gemfile.lock
├── project.gemspec
└── README.md
```

### Testing with RSpec
```ruby
# frozen_string_literal: true

require 'spec_helper'

RSpec.describe Project::Calculator do
  subject(:calculator) { described_class.new }

  describe '#add' do
    it 'returns the sum of two numbers' do
      expect(calculator.add(2, 3)).to eq(5)
    end

    context 'with negative numbers' do
      it 'handles negative values correctly' do
        expect(calculator.add(-1, 5)).to eq(4)
      end
    end

    context 'with invalid input' do
      it 'raises ArgumentError for non-numeric input' do
        expect { calculator.add('a', 1) }.to raise_error(ArgumentError)
      end
    end
  end

  describe '#divide' do
    it 'returns the quotient' do
      expect(calculator.divide(10, 2)).to eq(5)
    end

    it 'raises ZeroDivisionError when dividing by zero' do
      expect { calculator.divide(1, 0) }.to raise_error(ZeroDivisionError)
    end
  end
end
```

---

## QUICK REFERENCE

```bash
# Run specs
bundle exec rspec

# Run specs with coverage
COVERAGE=true bundle exec rspec

# Lint and auto-fix
bundle exec rubocop -a

# Security scan (Rails)
bundle exec brakeman -q

# Dependency audit
bundle audit check

# Verify git environment
gh auth status
```
