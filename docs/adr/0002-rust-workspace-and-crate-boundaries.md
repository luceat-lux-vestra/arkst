# ADR-0002: Rust Workspace and Crate Boundaries

- **Status:** Superseded
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1

Superseded by ADR-0015.

The four-crate ownership decision below is historical. ADR-0015 defines the
current target crate boundaries; the remaining sections are retained as
historical context where they are not superseded.

## Context

The initial crate structure must support the compiler pipeline without
over-engineering. The Quarkdown-compatible syntax is a core language feature,
not an external plugin.

## Decision Drivers

- Minimal initial crate count
- Core language identity (Quarkdown-compatible) must be obvious from structure
- Clear separation of concerns for testing and future substitution
- Avoid premature modularization

## Considered Options

### Option 1: Monorepo with many crates (rejected)

`scribium-core`, `scribium-cli`, `scribium-typst`, `scribium-compat-quarkdown`,
`scribium-lsp`, `scribium-wasm` — too many empty crates before any need.

### Option 2: Five crates including compat adapter (rejected)

`scribium-compat-quarkdown` implies Quarkdown support is optional. Given that
Quarkdown compatibility is Scribium's first-class identity, this naming is
misleading.

### Option 3: Four crates, core owns its language identity (chosen)

```
scribium-cli
scribium-core        ← Quarkdown-compatible syntax is core, not a plugin
scribium-typst
scribium-test-support
```

## Decision

Use four crates. `scribium-core` owns parser, semantic analysis, evaluator,
built-ins, IR, source map, and the internal `compatibility/` module (which
handles only profile selection, divergence tracking, and diagnostics).

## Consequences

### Positive

- Clear that Quarkdown-compatible syntax IS Scribium's language
- No naming confusion about what's "core" vs what's "compat"
- Fewer integration boundaries to maintain early on

### Negative

- If a third frontend ever appears, splitting will be more work than starting split

### Risks

- Internal module structure must prevent spaghetti
- Mitigation: documented module boundaries in ARCHITECTURE.md

## Future crate additions (gated by demonstrated need)

```
scribium-wasm           (M6+)  ← thin WASM bindings for scribium-core
scribium-typst-native   (M6+)  ← subprocess backend (split from scribium-typst)
scribium-typst-web      (M7+)  ← full browser Typst compile (feasibility-gated)
scribium-lsp            (M6+)  ← LSP server
scribium-frontend-quarkdown  (if multi-frontend split is needed)
scribium-backend-typst       (if multi-backend split is needed)
```

## Platform Independence

`scribium-core` MUST compile for `wasm32-unknown-unknown`. CI verifies this on every push.

### Forbidden in core

- `std::fs` — no filesystem access
- `std::process` — no process execution
- `std::env` — no environment variable access
- `TcpStream` — no network access
- System clock dependency
- Global mutable state
- `std::path::PathBuf` in public API — use `VirtualPath(String)`

### VirtualProject: I/O-Free Core

```rust
pub struct VirtualProject {
    pub entry: VirtualPath,
    pub sources: SourceStore,
    pub assets: AssetStore,
}

pub fn compile(project: &VirtualProject) -> CompileResult;
```

Native CLI reads files from disk and builds `VirtualProject`. WASM frontend
builds it from in-memory sources. Core never touches the filesystem.

### Synchronous Core, Async Host

Core compilation is synchronous. Host loads files asynchronously before calling core.
Incremental resolution:

```rust
pub enum CompileStatus {
    Complete(CompileOutput),
    NeedsSources(Vec<VirtualPath>),
}
```

### Virtual Paths

Internal paths are logical, not OS-specific:

```rust
pub struct VirtualPath(String);
```

Examples: `chapter/intro.qd`, `assets/logo.svg`. Native CLI maps them to
`PathBuf` at the adapter layer.

### WASM Worker Architecture

```text
Editor UI
    │ postMessage
    ▼
Web Worker
    ├── scribium-wasm (thin JS bindings)
    └── scribium-core + scribium-typst (lowering only)
```

Full Typst compilation in the browser is a separate goal (see ADR-0005).
The frontend-only WASM target produces generated Typst source for server-side
or native Typst compilation.

## References

- `docs/ARCHITECTURE.md`
- `Cargo.toml` (workspace root)
