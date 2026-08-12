# ADR-0009: Error Handling and Exit Codes

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1, #9

## Context

Scribium must have a consistent error model with stable error codes, structured
diagnostics, and predictable exit codes.

## Decision Drivers

- Errors must be machine-parseable (JSON) and human-readable
- Error codes must be stable across versions
- Libraries must never call `process::exit`
- Source locations must always be preserved

## Considered Options

### Option 1: Anyhow + thiserror (chosen)

Standard Rust error handling. `thiserror` for library errors, `anyhow` for
CLI error reporting. No custom error framework.

### Option 2: Custom error codec framework

Too much overhead for initial scope. Revisit if error complexity warrants it.

## Decision

Use `thiserror` for library crate error types and `anyhow` for CLI error
aggregation. Error codes are assigned per diagnostic, not per error type.

ADR-0009 owns the stable diagnostic-code ranges and error/exit-code policy.
`scribium-diagnostics` owns the shared diagnostic representation. The compiler
stage detecting a problem owns that diagnostic's semantics and construction;
`scribium-core` aggregates diagnostics but is not their implementation owner.

Conceptually:

```text
syntax/parser       -> frontend
semantic/evaluation -> scribium-engine
compatibility       -> scribium-compat
Typst lowering      -> scribium-typst
Typst execution     -> concrete Typst backend adapter
project/config      -> responsible project/host layer
all use
    ↓
scribium-diagnostics representation
scribium-core
    ↓
aggregates results
```

## Exit codes

| Code | Meaning                |
|------|------------------------|
| 0    | Success                |
| 1    | User error (syntax, semantic, evaluation, lowering, compat) |
| 2    | Configuration or IO error |
| 3    | Typst backend error    |
| 125  | Internal invariant violation (bug) |
| 130  | Interrupted by SIGINT  |

## Error code ranges

| Range  | Category      |
|--------|---------------|
| E1xxx  | Syntax        |
| E2xxx  | Semantic      |
| E3xxx  | Evaluation    |
| E4xxx  | Lowering      |
| E5xxx  | Typst backend |
| E6xxx  | Project/config |
| E7xxx  | IO/assets     |
| E8xxx  | Compatibility |
| E9xxx  | Internal invariant |

## Diagnostic JSON format

```json
{
  "code": "E0123",
  "severity": "error",
  "message": "expected expression, found end of input",
  "primary": { "file": "src/main.qd", "line": 5, "column": 12 },
  "secondary": [],
  "hints": ["check for missing closing bracket"]
}
```

## References

- `crates/scribium-core/src/diagnostics.rs` (current implementation location
  during physical migration, not target ownership)
- `crates/scribium-cli/src/exit.rs`
- ADR-0015: Compiler crate boundaries
