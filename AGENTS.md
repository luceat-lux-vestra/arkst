# Scribium — Agent Operations Guide

## Project Summary

**Scribium** is an independent, Apache-2.0 Quarkdown-compatible compiler and
toolchain powered by the official Typst compiler. Quarkdown compatibility is a
first-class requirement — not a deferred milestone.

- **Current milestone:** M0 Foundation
- **Stability:** Pre-alpha, experimental
- **Rendering backend:** Official Typst compiler (subprocess, in-process TBD)
- **Non-goals:** Custom PDF/HTML renderers, SaaS, package registry

## Source of Truth

Document priority (higher overrides lower):
1. User's latest explicit instruction
2. `SCRIBIUM_MASTER_EXECUTION_BRIEF.md`
3. Approved ADRs
4. `ARCHITECTURE.md` / `PRODUCT.md`
5. `ROADMAP.md`
6. `AGENTS.md`
7. Code and tests

## Quick Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p scribium-core
cargo run -p scribium-cli -- build examples/hello/main.qd
cargo run -p scribium-cli -- inspect examples/hello/main.qd --emit typst
```

## Architecture Boundaries

- Parser does not read the filesystem directly.
- Core does not depend on CLI.
- Lowering does not know CLI output formats.
- Diagnostics distinguish generated paths from original paths.
- Compatibility layer (`scribium-compat-quarkdown`) does not pollute core syntax.
- Typst-specific types do not leak into the public semantic layer.
- Shell execution and arbitrary network access are never added.
- Quarkdown-compatible syntax is part of core, not an external compatibility layer.
- The internal `compatibility/` module handles only profile selection, divergence tracking, and diagnostics.
- No dependency on copied Quarkdown implementation code.

## Coding Rules

- All public API items have docs.
- `unsafe` is prohibited by default (ADR override required).
- `unwrap`/`expect` allowed only in tests or bootstrap code where invariants
  are trivially obvious.
- Libraries never call `process::exit`.
- Errors are structured (typed error enums, diagnostic codes).
- Source spans are never discarded.
- Output is deterministic.
- Paths are platform-independent.
- No hidden global mutable state.
- No silent fallbacks.
- No user-facing panics.

## Test Requirements

| Change type        | Required tests                              |
|--------------------|---------------------------------------------|
| Parser change      | unit + snapshot + malformed input           |
| Semantic change    | unit + golden + error case                  |
| Lowering change    | golden Typst + end-to-end compile           |
| Diagnostic change  | source span snapshot                        |
| CLI change         | integration test + help snapshot            |
| Config change      | migration/default/invalid cases             |
| Compatibility      | provenance record + conformance fixture     |

## Documentation Update Matrix

| Change type               | Files to update                                              |
|---------------------------|--------------------------------------------------------------|
| Public syntax change      | SYNTAX.md + CHANGELOG.md + examples                          |
| Architecture change       | ARCHITECTURE.md + ADR                                        |
| CLI change                | README.md + help snapshot                                    |
| Compatibility change      | COMPATIBILITY matrix                                         |
| Security boundary change  | THREAT_MODEL.md + SECURITY.md                                |
| Release process change    | RELEASING.md                                                 |

## Forbidden Actions

- Copying Quarkdown source code or prior port code
- Hiding generated Typst fixture provenance
- Deleting failing tests to pass CI
- Batch-updating snapshots without review
- Disabling CI checks
- Overwriting user-authored files
- Large-scale refactors outside scope
- Adding dependencies without justification
- Committing secrets
- Arbitrary `curl | sh` in scripts
- Skipping name/license validation before release

## Session Checklist

**Start:**
1. `git status`
2. Current branch and linked issue
3. `AGENTS.md`
4. `ROADMAP.md`
5. Relevant ADRs
6. Relevant tests
7. Baseline test run

**End:**
1. Format
2. Clippy
3. Relevant tests
4. Full test when feasible
5. Docs update
6. Changelog decision
7. `git diff` review
8. Commit
9. Issue/PR update
10. Record next task