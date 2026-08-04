# ADR-0011: In-Process Typst Backend Feasibility Investigation

- **Status:** Proposed
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
| In-process compile test | ⚠️ Possible but requires full `World` implementation |
| WASM cargo check | ✅ `typst` + deps compile to `wasm32-unknown-unknown` (cargo check passes) |
| MSRV impact | Scribium did not previously declare an explicit MSRV; typst 0.15.1 requires Rust 1.92 |
| Clean build time increase | Not measured in this investigation |
| Process spawn overhead | Not measured in this investigation |

## Investigation Details

### Public Typst API

Typst 0.15.1 provides:
- `typst::compile(world: &dyn World) -> Warned<Result<Document, EcoVec<SourceDiagnostic>>>` — public entry point
- `typst::World` trait — public trait with 7 required methods

The `World` trait requires implementing:
1. `library(&self) -> &LazyHash<Library>`
2. `book(&self) -> &Book`
3. `main(&self) -> FileId`
4. `source(&self, id: FileId) -> FileResult<Arc<str>>`
5. `file(&self, id: FileId) -> FileResult<Arc<[u8]>>`
6. `font(&self, id: usize) -> FileResult<Arc<[u8]>>`
7. `today(&self, offset: Option<i64>) -> Option<T>`

Plus associated types and auxiliary types (`LazyHash`, `FileId`, `SourceDiagnostic`, `EcoVec`, `T`, `Book`, `Library`).

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

Typst-specific types (`LazyHash`, `FileId`, `SourceDiagnostic`, `EcoVec`, `T`, `Book`, `Library`) must not leak into Scribium's public semantic layer.

### WASM Compilation

A minimal Rust crate depending on `typst = "0.15.1"` passes `cargo check --target wasm32-unknown-unknown` with Rust 1.92.0. However, this only verifies that the dependency graph compiles for the target — it does not prove that a browser-ready Typst backend is operational. Browser compilation still requires:

- Font loading and management
- Virtual filesystem
- Package and asset resolution
- JavaScript bindings (wasm-bindgen)
- Memory budget and bundle size management

### Build Time and Binary Size

Clean build time increase and binary size increase were **not measured in this investigation**. Previous claims of "+500ms build time" and "50-100ms process overhead" were unsubstantiated.

To measure these properly, the following methodology should be used:
```bash
cargo clean
/usr/bin/time -p cargo build --workspace
cargo clean
/usr/bin/time -p cargo build --manifest-path tools/spikes/typst-in-process/Cargo.toml
```

Measurements should record: OS, CPU, RAM, rustc version, cargo version, Typst version, build profile, date, cold vs warm build, and at least 5 iterations with mean and range.

Binary size comparison (release, stripped vs unstripped):
- Baseline subprocess CLI: Not measured
- Minimal in-process spike: Not measured
- Absolute increase: Not measured
- Percentage increase: Not measured

### MSRV

Before this PR, Scribium did not declare an explicit MSRV and used the floating `stable` toolchain. This PR (and the associated PR #19 changes) establishes Rust 1.92 as Scribium's first explicit MSRV, matching Typst 0.15.1's compiler requirement. There was no prior official MSRV of 1.85 to "bump from."

## Decision

**Deferred to M6+ (separate from WASM frontend bindings milestone).**

Rationale:
1. Typst 0.x API is unstable — investing in `World` implementation now risks rewrite at 1.0
2. `World` implementation scope is large (source loading, font management, package resolution, virtual filesystem, caching, diagnostics)
3. Subprocess backend is functionally complete and sufficient for M1-M5 requirements
4. Native `InProcessBackend` and browser `WebBackend` are separate concerns (see ADR-0005)
5. No measurable performance bottleneck has been identified with the subprocess backend for current milestones

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