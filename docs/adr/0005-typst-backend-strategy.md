# ADR-0005: Typst Backend Strategy

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1, #6, #7

## Context

Scribium must invoke the Typst compiler. Two approaches exist: subprocess
(CLI wrapper) or in-process embedding (library linking).

## Decision Drivers

- Speed of initial implementation
- Diagnostic integration quality
- Binary size and portability
- WASM feasibility
- Long-term maintainability

## Considered Options

### Option 1: Subprocess adapter (chosen for M1)

Calls `typst compile` or `typst --version` via subprocess. Reads stdout/stderr
for diagnostics. Parses JSON diagnostic output when available.

### Option 2: In-process embedding (future)

Links Typst as a Rust library crate. Direct access to Typst's World trait.
Better diagnostic integration but higher implementation complexity and binary size.

## Decision

Start with subprocess adapter for M1 vertical slice. The `TypstBackend` trait
abstracts the backend choice. Investigate in-process embedding feasibility
before v0.1. Both backends do not need to be permanently maintained.

## Consequences

### Positive

- M1 can ship without Typst Rust library complexity
- CLI behavior matches `typst compile` exactly
- Simple diagnostic parsing from Typst's output

### Negative

- Requires `typst` binary on PATH (or configured path)
- Diagnostic parsing is lossy compared to library API
- WASM compilation impossible with subprocess backend
- Process management overhead for `watch` mode

### Risks

- Typst CLI output format is not a stable API
- Mitigation: document the parsed format, test against known versions

## References

- `crates/scribium-typst/src/backend.rs`
- Spike results in `docs/research/typst-backend-spike.md`