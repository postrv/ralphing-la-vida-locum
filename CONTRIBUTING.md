# Contributing to Ralph

Thank you for your interest in contributing to Ralph.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/postrv/ralph.git`
3. Create a branch: `git checkout -b feature/your-feature`

## Development Setup

```bash
# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test

# Check for warnings
cargo clippy

# Format code
cargo fmt
```

## Before Submitting

Please ensure your changes:

1. **Pass all tests**: `cargo test`
2. **Pass clippy**: `cargo clippy -- -D warnings`
3. **Are formatted**: `cargo fmt`
4. **Include tests** for new functionality
5. **Update documentation** if needed

## Pull Request Process

1. Update the README.md if you've changed CLI behavior
2. Add entries to CHANGELOG.md under "Unreleased"
3. Ensure CI passes on your PR
4. Request review from maintainers

## Code Style

- Follow Rust conventions
- Use meaningful variable and function names
- Keep functions focused and small
- Add comments for non-obvious logic

## Security

If you discover a security vulnerability, please email the maintainers privately rather than opening a public issue.

## Questions?

Open an issue for discussion before starting major work.
