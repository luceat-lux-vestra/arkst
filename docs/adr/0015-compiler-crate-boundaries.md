# ADR-0015: Compiler Crate Boundaries

- **Status:** Proposed
- **Date:** 2026-08-12
- **Owners:** Scribium maintainers
- **Related ADRs:** 0002, 0014
- **Related work:** PR #46; `refactor/markdown-parser-foundation`

## Context

Scribium's source-location primitives, in-memory compilation project, and
compiler workflow have different responsibilities. The existing workspace
places these responsibilities behind `scribium-core`; the target architecture
separates them without changing the implementation in this documentation-only
step.

This ADR records the next target boundaries after the frontend ownership
decision in ADR-0014. It is intentionally incomplete. Later architecture
corrections will decide the remaining compiler boundaries.

## Decision 1: frontend ownership established by Architecture Correction 1

ADR-0014 establishes the target frontend ownership:

- `scribium-markdown` owns the Markdown frontend, including `BlockParser` and
  the Markdown frontend AST.
- `scribium-quarkdown` owns Quarkdown-specific grammar and its grammar-level
  result types.
- `scribium-markdown` depends on `scribium-quarkdown` for Quarkdown grammar;
  `scribium-quarkdown` must not depend on the Markdown frontend.
- Markdown parsing is not permanently owned by `scribium-core`. Its current
  physical location may remain there during migration, but that does not
  change the target ownership.

The frontend crates do not depend on `scribium-project`. A frontend parses a
particular source input with the minimum source context required to preserve
source locations; the final parser function signature is not defined here.

## Decision 2: `scribium-source` owns source-location primitives

The target `scribium-source` crate is the lowest-level, platform-independent
source-location abstraction. It owns source identity and source-location value
types, including:

- `SourceId`;
- `ByteSpan`;
- `SourceSpan`;
- `LineColumn`;
- source-position and range conversion primitives independent of a project;
  and
- low-level source-segment and source-range mapping primitives required by
  parsers.

The future segment-aware inline-input design may build on these types. The API
for that design is not defined by this ADR.

`scribium-source` must not own:

- `VirtualProject` or `VirtualProjectBuilder`;
- `ProjectMetadata`;
- `VirtualPathBuf`;
- `SourceStore` or `AssetStore`;
- parser implementation, Markdown AST, or Quarkdown grammar;
- compiler orchestration, evaluator, IR, or Typst lowering; or
- filesystem or network access.

The target crate is deterministic, filesystem-free, network-free, and
compatible with `wasm32-unknown-unknown`. It provides stable primitives that
frontends, project, diagnostics, IR, and lowering layers can share without a
dependency on `scribium-core`.

## Decision 3: `scribium-project` owns the in-memory compilation project

The target `scribium-project` crate owns the in-memory compilation project
model. Its architectural ownership includes:

- `VirtualPathBuf` and `VirtualPathError`;
- `SourceStore` and `SourceStoreError`;
- `AssetStore` and `AssetStoreError`;
- `VirtualProject`;
- `VirtualProjectBuilder`; and
- `ProjectMetadata`.

These types are not physically moved by this ADR. `SourceStore` is a
project-level collection of source files with virtual-path lookup and
`SourceId` assignment/lifecycle. `AssetStore` owns project resources such as
images and fonts. `VirtualProject` combines the entry path, source collection,
asset collection, and project metadata, so these concepts belong together at
the project layer.

`scribium-project` depends on `scribium-source` and may use `SourceId` and
other low-level source primitives. `scribium-source` must never depend on
`scribium-project`.

`VirtualProject` remains an I/O-free, deterministic compilation unit. The
native CLI or other host is responsible for filesystem discovery, filesystem
reads, path canonicalization, symlink policy, configuration loading, and
loading sources and assets before constructing a `VirtualProject`. WASM and
other embedders construct the same project directly from in-memory data.
Neither `scribium-project` nor lower compiler crates perform filesystem or
network I/O.

## Decision 4: `scribium-core` is the composition and orchestration layer

The long-term role of `scribium-core` is a stable Scribium compiler facade and
composition layer. Its top-level workflow is conceptually:

```text
VirtualProject
    ↓
select entry source
    ↓
frontend parse
    ↓
later compiler/evaluation stages
    ↓
CompileResult
```

`scribium-core` must not remain the implementation owner of:

- source identity or source-location primitives;
- project, source, or asset stores;
- virtual paths or `VirtualProject`;
- Markdown parser implementation; or
- Quarkdown grammar implementation.

`scribium-core` may later re-export selected stable public types from lower
crates to preserve a convenient user-facing API. Re-exporting a type does not
make `scribium-core` the implementation owner of that type.

## Decision 5: dependency direction

In the following diagram, `A -> B` means that A depends on B:

```text
scribium-project ---------> scribium-source
scribium-quarkdown -------> scribium-source   (only if source primitives are needed)
scribium-markdown --------> scribium-source
scribium-markdown --------> scribium-quarkdown
scribium-core ------------> scribium-project
scribium-core ------------> scribium-markdown
```

The frontend dependency rules are mandatory: `scribium-markdown` and
`scribium-quarkdown` do not depend on `scribium-project`, and neither frontend
depends on `scribium-core`. `scribium-quarkdown` does not depend on
`scribium-markdown`.

The following directions are forbidden:

```text
scribium-source  -X-> scribium-project
scribium-source  -X-> scribium-core
scribium-project -X-> scribium-markdown
scribium-project -X-> scribium-core
scribium-markdown -X-> scribium-project
scribium-markdown -X-> scribium-core
scribium-quarkdown -X-> scribium-markdown
scribium-quarkdown -X-> scribium-project
scribium-quarkdown -X-> scribium-core
```

This keeps the frontends usable without constructing an entire compilation
project and prevents cyclic compiler dependencies.

## Unresolved boundaries

ADR-0015 does not decide ownership of:

- diagnostics;
- source map generation;
- AST → IR lowering;
- IR;
- evaluator;
- built-ins;
- semantic analysis;
- compatibility layer; or
- Typst lowering/backend.

These boundaries will be filled in by subsequent architecture corrections.

## Migration and ADR history

This ADR records target ownership only. It does not add workspace members,
create crate directories, move Rust modules, change imports or public APIs, or
change tests and CI. ADR-0002 is not rewritten or marked Superseded here.
Once the complete replacement workspace architecture is settled, a later
correction may finalize ADR-0015 and decide the status of ADR-0002.
