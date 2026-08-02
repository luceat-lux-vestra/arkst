# Contributing to Scribium

## Prerequisites

- Rust 1.97+ with `rust-toolchain.toml` for pinned version
- Typst 0.15+ (optional for compile tests)
- Git with conventional commits

## Quick Start

```bash
git clone https://github.com/luceat-lux-vestra/scribium.git
cd scribium
cargo build
cargo test
```

## Issue-First Workflow

1. Search existing issues before creating a new one.
2. Create an issue describing the problem and proposed solution.
3. Wait for discussion and acceptance before starting work.
4. Reference the issue in your branch and commits.

## Branch Convention

```
feat/<issue>-short-description
fix/<issue>-short-description
docs/<issue>-short-description
refactor/<issue>-short-description
spike/<issue>-short-description
release/vX.Y.Z
```

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(parser): preserve spans for fenced code blocks
fix(lowering): escape Typst text delimiters
docs(adr): decide Typst backend strategy
```

- Imperative present tense
- No trailing period
- 72-character subject line
- Breaking changes in footer: `BREAKING CHANGE: ...`

## Code Style

- `cargo fmt` before committing
- `cargo clippy` must pass with `-D warnings`
- Public API items must have doc comments
- Tests required for new functionality
- Errors are structured with diagnostic codes
- Source spans preserved through the pipeline

## ADR Process

Significant architecture decisions require an Architecture Decision Record:

1. Copy `docs/adr/0000-template.md`
2. Write the decision context and options
3. Submit as part of your PR
4. Link the ADR in your commit message

## Compatibility and Provenance

All compatibility features with external systems (Quarkdown, etc.) require:

- Provenance record of the public specification source
- Observed input/output pairs
- Known semantic differences documented
- No source code copied from the original implementation

## Pull Request Checklist

- [ ] Summary
- [ ] Motivation
- [ ] Linked issue
- [ ] Design/ADR
- [ ] Tests (unit + snapshot + golden)
- [ ] Documentation updated
- [ ] Changelog decision
- [ ] Format + clippy pass
- [ ] Compatibility impact considered
- [ ] Security impact considered

## AI-Assisted Contributions

AI-assisted code generation is permitted under these rules:

- Generated code must not infringe on third-party copyrights
- All generated code follows the same quality and test standards
- AI-generated contributions are subject to the same review process
- Provenance of AI-generated compatibility code must be documented