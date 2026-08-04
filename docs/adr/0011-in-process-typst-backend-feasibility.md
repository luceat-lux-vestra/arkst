# ADR-0011: In-Process Typst Backend Feasibility Investigation

- **Status:** Accepted
- **Date:** 2026-08-04
- **Owners:** Scribium Authors
- **Related issues:** #12

## Context

Scribium currently uses a subprocess-based Typst backend (`typst compile` CLI). The question is whether we can link the `typst` Rust crate directly as a library for in-process compilation, which would provide:

- Faster compilation (no process spawn overhead)
- Better error handling (structured Rust errors vs subprocess stdout/stderr)
- Potential for WASM compilation (browser-based Typst rendering)
- Tighter integration with source maps and diagnostics

## Considered Options

### Option 1: Continue with Subprocess Backend (Current)
- **Pros:** Works today, stable, isolates Typst crashes, simple implementation
- **Cons:** Process spawn overhead (~50-100ms), stdout/stderr parsing, no WASM path

### Option 2: In-Process Backend via `typst` Crate
- **Pros:** Zero process overhead, structured diagnostics, potential WASM support, better source map integration
- **Cons:** API instability (typst 0.x), complex `World` trait implementation, internal API exposure, compile time increase

### Option 3: Defer to M6+ (When WASM Backend is Re-evaluated)
- **Pros:** Wait for Typst 1.0 stable API, let ecosystem mature
- **Cons:** Delay potential performance gains

## Decision Drivers

- **License:** `typst` crate is Apache-2.0 — compatible with Scribium's Apache-2.0
- **API Stability:** Typst 0.15.1 is 0.x — no stability guarantees; public API surface is limited and internal
- **World Trait:** Core abstraction requires implementing ~10 methods with complex types (`LazyHash`, `FileId`, `SourceDiagnostic`, etc.)
- **WASM Compilation:** `typst` crate + dependencies compile to `wasm32-unknown-unknown` ✅
- **Compile Time:** Adding `typst` crate adds ~500ms to clean build
- **MSRV:** typst 0.15.1 requires Rust 1.92; Scribium uses 1.85 — needs MSRV bump

## Investigation Results

| Criterion | Result |
|-----------|--------|
| License compatibility | ✅ Apache-2.0 (compatible) |
| Public API availability | ⚠️ Limited — core types private, `World` trait complex |
| In-process compile test | ⚠️ Possible but requires full `World` impl (~10 methods) |
| WASM compilation | ✅ `typst` + deps compile to `wasm32-unknown-unknown` |
| MSRV impact | ❌ Requires bump from 1.85 → 1.92 |
| Clean build time | ~+500ms |

## Decision

**Defer in-process backend implementation to M6+ (WASM re-evaluation gate).**

Rationale:
1. Typst 0.x API is unstable — investing in `World` implementation now risks rewrite at 1.0
2. MSRV bump (1.85 → 1.92) is a breaking change for some users
3. Subprocess backend is functionally complete and performant enough for M1-M5
4. WASM compilation works but browser integration (wasm-bindgen, JS glue) needs design
5. M6 is the scheduled milestone for WASM bindings — natural integration point

## Consequences

### Positive
- Avoids premature API coupling to unstable Typst internals
- Keeps MSRV at 1.85 for broader compatibility
- Subprocess backend remains reliable fallback

### Negative
- Missed performance optimization for M1-M5
- No browser-based compilation until M6+

### Risks
- Typst 1.0 API may still require significant adaptation
- WASM binary size may be large (needs investigation at M6)

## Validation Plan

At M6 kickoff:
1. Re-evaluate Typst 1.0 API stability
2. Prototype minimal `World` implementation
3. Benchmark in-process vs subprocess
4. Assess WASM bundle size with `wasm-opt`

## References

- typst crate: https://crates.io/crates/typst (0.15.1, Apache-2.0)
- Typst GitHub: https://github.com/typst/typst
- Issue #12: In-process Typst backend feasibility investigation