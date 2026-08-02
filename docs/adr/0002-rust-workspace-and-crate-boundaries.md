# ADR-0002: Rust Workspace and Crate Boundaries

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1

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
scribium-lsp      (M6)
scribium-wasm     (M6+)
scribium-frontend-quarkdown  (if multi-frontend split is needed)
scribium-backend-typst       (if multi-backend split is needed)
```

## References

- `docs/ARCHITECTURE.md`
- `Cargo.toml` (workspace root)