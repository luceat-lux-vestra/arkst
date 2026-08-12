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

## Architecture ownership note

ADR-0015 refines physical crate ownership without changing this backend
strategy. Pure Typst lowering and the platform-neutral backend contract belong
to `scribium-typst`; `SubprocessBackend` belongs to
`scribium-typst-subprocess`. Future in-process and browser work remains
governed by ADR-0005 and ADR-0011.

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

## WASM Impact

### Frontend-only WASM (guaranteed)

The Scribium frontend (parse → evaluate → lower to Typst source) compiles
to WASM and runs in the browser. The generated `.typ` source is returned to
the host for external compilation.

```
.qd → Scribium WASM → generated .typ → server/native Typst → PDF
```

This is the guaranteed path and requires only `scribium-core` + `scribium-typst`
(lowering) on `wasm32-unknown-unknown`.

### Full browser compile (separate goal)

```
.qd → Scribium WASM → Typst WASM backend → PDF/SVG/HTML
```

Technically feasible (Typst is also Rust) but requires:
- Font loading and management
- Virtual filesystem
- Package resolution
- Asset loading (images, etc.)
- Memory budget and bundle size management

This is gated behind a separate `scribium-typst-web` crate and M7+ feasibility
verification. It does not block WASM frontend delivery.

### Backend Trait Adaptation

`TypstBackend` trait is split:

| Implementation | Crate | Target |
|---|---|---|
| `SubprocessBackend` | `scribium-typst-subprocess` | CLI |
| `InProcessBackend` | TBD: future dedicated adapter, per ADR-0011 re-evaluation | CLI, server |
| `WebBackend` | `scribium-typst-web` (M7+) | Browser WASM |

The trait itself stays in `scribium-typst` for all targets.

## References

- `crates/scribium-typst/src/backend.rs`
- Spike results in `docs/research/typst-backend-spike.md`
