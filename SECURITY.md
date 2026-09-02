# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| < 0.1   | ❌ Pre-release, no security support |

## Reporting a Vulnerability

This project uses **GitHub private vulnerability reporting**.

Please report security vulnerabilities through the GitHub Security tab
at `https://github.com/luceat-lux-vestra/scribium/security/advisories/new`.

Do not report security vulnerabilities via public GitHub issues.

## Response Expectations

- You will receive an acknowledgment within 48 hours.
- The maintainer will investigate and provide a timeline for a fix.
- Coordinated disclosure will be used — the reporter and maintainer
  agree on a publication date before public disclosure.

## Scope

- The Arkst CLI and library code
- Build and release pipeline
- GitHub Actions workflows

## Out of Scope

- Typst compiler vulnerabilities (report to Typst GmbH)
- Rust toolchain vulnerabilities (report to Rust project)
- Third-party dependency vulnerabilities

## Unsafe Document Warning

Arkst processes untrusted input documents. The parser and evaluator
are designed with safety limits (recursion depth, expansion limits, etc.),
but no guarantees are made about security boundary enforcement in pre-1.0 versions.

Do not use Arkst to process untrusted documents in security-critical
contexts before v1.0.