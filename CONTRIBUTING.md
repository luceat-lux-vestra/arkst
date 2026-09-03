# Contributing to Arkst

## Prerequisites

- Rust 1.97+ with `rust-toolchain.toml` for pinned version
- Typst 0.15+ (optional for compile tests)
- Git with conventional commits

## Quick Start

```bash
git clone https://github.com/luceat-lux-vestra/arkst.git
cd arkst
cargo build
cargo test
```

## Issue-First Workflow

1. Search existing issues before creating a new one.
2. Create an issue describing the problem and proposed solution.
3. Wait for discussion and acceptance before starting work.
4. Reference the issue in your branch and commits.

## Roadmap Milestone Governance

[`docs/ROADMAP.md`](docs/ROADMAP.md) is the canonical roadmap. Native GitHub
milestones, their milestone-owned issues, and the authoritative umbrella or
tracker are the execution metadata for that roadmap. A milestone shell is not
implementation authorization: future M4–M7 shells may remain open while their
roadmap status is `Not started`, and implementation children attach only after
the milestone is activated. Cross-cutting compatibility/reconciliation,
research, host-capability, and generic hardening work remains milestone-null
when it spans milestones or has no single primary owner, with the reason
recorded in the governing tracker.

### Activation invariant

Before the first implementation PR of a roadmap milestone:

1. The native GitHub milestone exists and is open.
2. The roadmap status agrees with activation.
3. The authoritative umbrella or tracker is assigned to the native milestone.
4. Every open milestone-owned child is assigned to that native milestone.
5. Cross-cutting issues are explicitly classified rather than accidentally
   omitted.
6. Milestone assignment does not bypass dependency, design, evidence, review,
   or merge gates.

### Closure invariant

Before closing a roadmap or native milestone:

1. The milestone exit and evidence gate is complete.
2. The milestone-owned open issue count is zero, or every exception has been
   explicitly moved or reclassified.
3. The roadmap, governance documentation, umbrella, and evidence agree.
4. The umbrella or tracker is completed.
5. Only then is the native milestone closed.
6. GitHub state is fresh-read to verify the closed state and zero open owned
   issues.

### Fresh-task audit

Before starting the next implementation task, verify the current `main` SHA,
current roadmap and native milestone state, parent or umbrella, dependencies,
milestone assignment, and roadmap/governance consistency.

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

## Engineering standard

All human and AI-assisted contributions are held to
[`docs/ENGINEERING.md`](docs/ENGINEERING.md). That standard covers correctness,
accepted architecture, invariants, provenance, diagnostics, testing,
performance, dependencies, and security. AI-generated implementation is not
exempt from those requirements or from architecture review.

In particular, compatibility changes require public-specification provenance,
independently authored conformance evidence, documented compatibility levels,
and review of any deliberate divergence. Current physical code is migration
state and does not override ADR-0014/0015 ownership.

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
- AI agents must stop and request architecture review rather than inventing
  crate ownership, dependency direction, semantic/IR layers, compatibility
  exceptions, security capabilities, plugin systems, or backend escape hatches
