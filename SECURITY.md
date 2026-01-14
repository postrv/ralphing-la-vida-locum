# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Ralph, please report it responsibly:

1. **Do NOT** open a public GitHub issue for security vulnerabilities
2. Email the maintainers privately with details
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes (optional)

We aim to respond to security reports within 48 hours and will work with you to understand and address the issue.

## Security Features

Ralph implements multiple layers of security to prevent dangerous operations during autonomous execution:

### Command Filtering

Ralph blocks dangerous command patterns including:
- Destructive filesystem operations (`rm -rf /`, `rm -rf ~`)
- Disk operations (`dd if=/dev/zero`, `mkfs.*`)
- Permission escalation (`chmod 777`, `sudo rm`)
- Fork bombs and resource exhaustion
- SSH key access (enforces `gh` CLI usage)
- Network exfiltration patterns

### Secret Detection

Ralph scans for secrets in code including:
- API keys and tokens
- Passwords
- AWS credentials
- Private keys (RSA, DSA, EC, Ed25519)

### narsil-mcp Integration

When available, Ralph uses narsil-mcp for:
- OWASP Top 10 scanning
- CWE Top 25 checking
- Injection vulnerability detection
- Dependency vulnerability scanning

### Configurable Permissions

Projects can configure allow/deny lists in `.claude/settings.json`:

```json
{
  "permissions": {
    "allow": ["Bash(git *)", "Bash(cargo *)"],
    "deny": ["Bash(rm -rf *)"]
  }
}
```

## Security Best Practices

When using Ralph:

1. **Review settings.json** - Configure appropriate permission boundaries
2. **Use gh CLI** - Ralph enforces `gh` CLI for GitHub operations
3. **Run security scans** - Use `ralph hook scan-modified` before commits
4. **Monitor analytics** - Check `.ralph/analytics.jsonl` for suspicious patterns
5. **Keep dependencies updated** - Run `cargo audit` regularly

## Known Limitations

- Ralph relies on Claude Code's own safety mechanisms for many operations
- Pattern-based blocking may not catch all obfuscated dangerous commands
- Security scanning requires narsil-mcp (optional dependency)

## Security Contacts

For security issues, contact the project maintainers through:
- Private email (preferred for vulnerabilities)
- GitHub Security Advisories

## Acknowledgments

We appreciate responsible disclosure and will acknowledge security researchers who help improve Ralph's security (with their permission).
