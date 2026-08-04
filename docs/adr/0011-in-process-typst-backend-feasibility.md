# ADR-0011: In-Process Typst Backend Feasibility Investigation

- **Status:** Accepted
- **Date:** 2026-08-04
- **Owners:** Scribium Authors
- **Related issues:** #12

## Context

Scribium currently uses a subprocess-based Typst backend (`typst compile` CLI). The question is whether we can link the `typst` Rust crate directly as a library for in-process compilation, which would provide:

- Potential for faster compilation (no process spawn overhead)
- Better error handling (structured Rust errors vs subprocess stdout/stderr)
- Potential for WASM compilation (browser-based Typst rendering)
- Tighter integration with source maps and diagnostics

## Considered Options

### Option 1: Continue with Subprocess Backend (Current)
- **Pros:** Works today, stable, isolates Typst crashes, simple implementation
- **Cons:** Process spawn overhead (unmeasured), stdout/stderr parsing, no WASM path

### Option 2: In-Process Backend via `typst` Crate
- **Pros:** No process overhead, structured diagnostics, potential WASM support, better source map integration
- **Cons:** API instability (typst 0.x), complex `World` trait implementation, internal API exposure, compile time increase

### Option 3: Defer to M6+ (When WASM Backend is Re-evaluated)
- **Pros:** Wait for Typst ecosystem to mature
- **Cons:** Delay potential performance gains

## Decision Drivers

- **License:** `typst` crate is Apache-2.0 — compatible with Scribium's Apache-2.0
- **API Stability:** Typst 0.15.1 is 0.x — no stability guarantees
- **Public API:** Typst provides a public `typst::compile` function and a public `World` trait
- **World Trait:** The `World` trait requires implementing 7 required methods with complex types (`LazyHash`, `FileId`, `SourceDiagnostic`, etc.)
- **WASM Compilation:** `typst` crate + dependencies compile to `wasm32-unknown-unknown` (cargo check passes)
- **Compile Time:** Adding `typst` crate increases clean build time (measurement below)
- **MSRV:** typst 0.15.1 requires Rust 1.92; Scribium did not previously declare an explicit MSRV

## Investigation Results

| Criterion | Result |
|-----------|--------|
| License compatibility | ✅ Apache-2.0 (compatible) |
| Public `typst::compile` API | ✅ Exists and is public |
| Public `World` trait | ✅ Exists and is public |
| World trait required methods | 7 (`library`, `book`, `main`, `source`, `file`, `font`, `today`) |
| World implementation complexity | ⚠️ High — requires source loading, file loading, font discovery, font caching, package resolution, virtual filesystem, asset loading, caching, diagnostics |
| In-process compile test | ✅ Spike successful — 3 test cases pass |
| WASM cargo check | ✅ `typst` + deps compile to `wasm32-unknown-unknown` (cargo check passes) |
| MSRV impact | Scribium did not previously declare an explicit MSRV; typst 0.15.1 requires Rust 1.92 |
| Clean build time increase | Baseline 2.1s vs in-process 18.5s (5 iterations, mean) |
| Process spawn overhead | Not isolated; subprocess cold start 52ms vs in-process 1.3ms for simple rect fixture |

## Investigation Details

### Public Typst API

Typst 0.15.1 provides:
- `typst::compile<T>(world: &dyn World) -> Warned<SourceResult<T>> where T: Output` — public entry point
- `typst::World` trait — public trait with 7 required methods

The `World` trait requires implementing:
1. `library(&self) -> &LazyHash<Library>`
2. `book(&self) -> &LazyHash<FontBook>`
3. `main(&self) -> FileId`
4. `source(&self, id: FileId) -> FileResult<Source>`
5. `file(&self, id: FileId) -> FileResult<Bytes>`
6. `font(&self, index: usize) -> Option<Font>`
7. `today(&self, offset: Option<Duration>) -> Option<Datetime>`

Plus associated types and auxiliary types (`LazyHash`, `FileId`, `SourceDiagnostic`, `EcoVec`, `Duration`, `Datetime`, `FontBook`, `Library`, `Font`, `Bytes`, `Source`, `FileError`).

### World Implementation Complexity

Implementing `World` for Scribium requires:
- Source and file loading
- Font discovery and font book construction
- Font caching
- Package resolution
- Virtual filesystem behavior
- Asset and image loading
- Deterministic path handling
- Repeated-load caching
- Structured diagnostics integration

Typst-specific types (`LazyHash`, `FileId`, `SourceDiagnostic`, `EcoVec`, `Duration`, `Datetime`, `FontBook`, `Library`, `Font`, `Bytes`, `Source`, `FileError`) must not leak into Scribium's public semantic layer.

### WASM Compilation

A minimal Rust crate depending on `typst = "0.15.1"` passes `cargo check --target wasm32-unknown-unknown` with Rust 1.92.0. However, this only verifies that the dependency graph compiles for the target — it does not prove that a browser-ready Typst backend is operational. Browser compilation still requires:

- Font loading and management
- Virtual filesystem
- Package and asset resolution
- JavaScript bindings (wasm-bindgen)
- Memory budget and bundle size management

### Build Time and Binary Size

**Clean build time:**
- Baseline (subprocess binary): 2.1s (5 iterations, mean)
- In-process spike: 18.5s (5 iterations, mean) 
- **Increase: ~16.4s** (not +500ms as previously claimed)

**Binary size (release):**
- Subprocess CLI: 458 KB (unstripped)
- In-process spike: 38 MB (unstripped) / 31 MB (stripped)
- **Increase: ~37.5 MB absolute / ~84x** (not previously measured)

**Runtime latency (in-process, 20 runs):**
- Simple rect: 548-1340 µs (mean: ~900 µs)
- Text with font: 550-1400 µs (mean: ~950 µs)

**Subprocess latency (comparison):**
- Simple rect: 52 ms
- Text with font: 2020 ms

**In-process vs subprocess runtime:**
- In-process is **50-100x faster** for the fixed synthetic fixtures used in this investigation
- End-to-end Typst CLI invocation took ~52 ms for simple rect, while warmed in-process path averaged ~1.3 ms. This difference must not be interpreted as isolated process-spawn cost.

**WASM cargo check:** ✅ Passes for `wasm32-unknown-unknown`

**MSRV:** The isolated Typst 0.15.1 spike was executed with Rust 1.92.0. This investigation does not establish or change Scribium's project-wide MSRV.

## Decision

**Deferred to M6+ (separate from WASM frontend bindings milestone).**

Rationale:
1. Typst 0.x API is unstable — investing in `World` implementation now risks rewrite at 1.0
2. `World` implementation scope is large (source loading, font management, package resolution, virtual filesystem, caching, diagnostics)
3. Subprocess backend is functionally complete and sufficient for M1-M5 requirements
4. Native `InProcessBackend` and browser `WebBackend` are separate concerns (see ADR-0005)
6. No measurable performance bottleneck has been identified with the subprocess backend for current milestones

## Consequences

### Positive
- Avoids premature API coupling to unstable Typst internals
- Subprocess backend remains reliable fallback
- Keeps scope focused on M1-M5 deliverables

### Negative
- Missed potential performance optimization for M1-M5
- No browser-based compilation until dedicated WebBackend work

### Risks
- Typst 1.0 API may still require significant adaptation
- WASM binary size may be large (needs investigation at M6)
- Native InProcessBackend and browser WebBackend are separate engineering efforts

## Validation Plan

At M6 kickoff (or when any re-evaluation trigger occurs):

1. Re-evaluate against the then-current Typst release
2. Prototype minimal `World` implementation for Scribium
3. Benchmark in-process vs subprocess (with methodology documented above)
4. Assess WASM bundle size with `wasm-opt`
5. Measure clean build time and binary size delta

### Re-evaluation Triggers

Revisit before v0.1 when at least one of the following becomes true:
- Subprocess diagnostic parsing blocks required functionality
- Watch-mode process overhead becomes measurable and material
- Native embedding is required by a server integration
- The project begins the dedicated browser WebBackend feasibility milestone

Do not assume a particular Typst 1.0 release date or stability guarantee. Re-evaluate against the then-current Typst release before implementation.

## References

- typst crate: https://crates.io/crates/typst (0.15.1, Apache-2.0)
- Typst GitHub: https://github.com/typst/typst
- Issue #12: In-process Typst backend feasibility investigation
- ADR-0005: Typst Backend Strategy (native vs browser backend separation)