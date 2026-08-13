# Scribium

[![Experimental](https://img.shields.io/badge/status-experimental-orange)](https://github.com/luceat-lux-vestra/scribium)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![CI](https://github.com/luceat-lux-vestra/scribium/actions/workflows/ci.yml/badge.svg)](https://github.com/luceat-lux-vestra/scribium/actions/workflows/ci.yml)

**Scribium is an independent, Apache-2.0 Quarkdown-compatible compiler and toolchain powered by the official Typst compiler.**

> Scribium targets complete compatibility with the publicly documented Quarkdown document language while tracking stable upstream evolution. Current verified compatibility is partial and evidence-based; the current verified baseline is referenced against Quarkdown v2.5.1. See `docs/compatibility/quarkdown/` for the matrix, evidence, and compatibility debt.

> Scribium is an independent project. It is not affiliated with, endorsed by, or
> sponsored by Typst GmbH or the Quarkdown project.

---

## What is Scribium?

Scribium independently implements the Quarkdown syntax and document-observable
semantics, connecting them to the official Typst compiler for high-quality
typesetting.

```
Quarkdown-compatible source (.qd / .scrib)
or Markdown
→ clean-room parser and evaluator
→ Quarkdown-compatible semantic model
→ backend-neutral IR
→ scribium-typst lowering
→ concrete Typst compiler adapter
→ official Typst compiler
→ PDF / HTML / SVG / PNG
```

Quarkdown compatibility is a correctness contract and long-term product target,
not a deferred optional plugin. Current claims remain limited to behavior backed
by conformance evidence. Scribium reimplements the language independently from
public documentation and permitted black-box behavior. No Quarkdown source code,
tests, or fixtures are copied or translated.

## Quickstart

```bash
# Build a document to generated Typst source (document.qd → document.typ)
scribium build examples/hello/main.qd

# Build a document to PDF (document.qd → document.pdf)
scribium build examples/hello/main.qd --format pdf

# Override the output path
scribium build examples/hello/main.qd --output out/main.typ
scribium build examples/hello/main.qd --format pdf --output out/main.pdf

# Select a specific Typst executable for PDF output (defaults to `typst` on PATH)
scribium build examples/hello/main.qd --format pdf --typst-path /opt/typst/bin/typst

# Check for errors without compiling
scribium check examples/hello/main.qd

# Inspect intermediate representations
scribium inspect examples/hello/main.qd --emit typst

# Build a Markdown input (report.md → report.typ)
scribium build report.md
```

> Supported inputs are `.qd`, `.scrib`, and `.md` (case-insensitive; files
> without an extension are rejected). A `.typ` input is rejected until Typst
> passthrough is implemented. The build refuses to overwrite the input file:
> an explicit `--output` that resolves to the input — including via
> `.`/`..` components (resolved in component order with symlinks
> interpreted as reached, and rejected before any directory is created, so
> a rejected build never leaves empty directories behind), symlinks, or
> hard links — is rejected. Distinct targets behind a symlink (e.g.
> `link/../document.qd` with `link -> ../other/subdir`) are accepted and
> written to their real location. On Windows, root-relative output paths
> (`\out\main.typ`) are resolved from the current drive's root, and
> drive-relative paths (`C:out\main.typ`) are rejected with a clear error
> because they depend on the per-drive current-directory state. Missing
> output directories (e.g. `out/` for `--output out/main.typ`) are created
> automatically. Output is written
> atomically: the content goes to a uniquely named temporary file (created
> exclusively, retrying up to 32 candidate names) in the output directory
> and is renamed into place, so
> readers never observe a partially written output and an erroring build
> leaves no partial file (temporary files are cleaned up on error-return
> paths; an abrupt crash or forced kill may leave one). This is not a
> crash-durability guarantee — the output directory is not fsynced, so
> power loss may not preserve the newest file. On Unix, replacing an
> existing output keeps its permission bits, and new outputs use the
> standard `0666 & !umask` mode (same as `fs::write`). **PDF via external
> Typst subprocess is experimental; HTML/SVG/PNG backends are not
> implemented yet; requesting them fails with a clear error.** PDF builds
> invoke the configured Typst executable (`typst` on `PATH` by default,
> overridable with `--typst-path <PATH>`) directly via `std::process::Command`
> — never through a shell. A `--format typst` build does not require a Typst
> install. Generated PDFs are validated for non-empty output and a `%PDF-`
> header before being written.

## Example (.qd)

```quarkdown
.heading level:{1}
    Hello, Scribium

This is a .strong {Quarkdown} document compiled through Typst to PDF.

.list
    .item
        Simple code lists work
    .item
        Math is supported: $E = mc^2$

.row
    .col
        Left
    .col
        Right
```

## Front matter

A `---`-delimited block at the start of a document provides metadata
(`title`, `author`, `date`, and custom keys). The supported format is a
flat line-based `key: value` form, **not full YAML**:

- Keys and values are split on the first colon.
- Nested objects, arrays, and block strings are **not** supported.
- Delimiters and metadata lines must start at column 0; indented keys reject
  the block, which is preserved as regular Markdown instead of being flattened.
- Duplicate keys: last occurrence wins.
- Custom metadata is stored in the IR in a deterministic (lexicographic) order.

```markdown
---
title: My Document
author: Alice
---
# Heading
```

## Current Status

| Feature                                 | Status       |
|-----------------------------------------|--------------|
| Markdown heading, paragraph             | Experimental |
| Emphasis, strong                        | Experimental |
| Lists                                   | Experimental |
| Inline links (`[text](url)`)               | Experimental |
| Inline code spans (`` `code` ``)           | Experimental |
| Dot-prefixed function calls               | Experimental / Parsed |
| Positional/named/body arguments           | Experimental / Parsed |
| Variables and conditionals              | Experimental / Implemented |
| Iteration and components                | Planned      |
| Tables, math, footnotes                 | Planned      |
| Include/read, data loading              | Planned      |
| Native `.typ` passthrough (host-level)  | Planned      |
| Quarkdown compatibility                 | Partial / evidence-based |
| `watch` mode, source maps               | Planned      |
| LSP integration                         | Planned      |
| WASM support                            | Deferred     |
| **PDF via Typst subprocess**            | **Experimental** |

## Architecture

```
┌────────────────────────────────────────────────────┐
│                    scribium-cli                     │
│  build | check | inspect | watch                   │
└──────────────────────┬─────────────────────────────┘
                       │
┌──────────────────────▼─────────────────────────────┐
│                   scribium-core                     │
│  facade/orchestration → frontend → engine → IR      │
│  (current physical code may still be consolidated)  │
└──────────────────────┬─────────────────────────────┘
                       │
                       ▼ normalized backend-neutral IR
┌────────────────────────────────────────────────────┐
│                   scribium-typst                    │
│  pure IR → Typst lowering + source maps             │
└──────────────────────┬─────────────────────────────┘
                       ▼ host-selected concrete adapter
              official Typst compiler
```

## Roadmap

- **M0 Foundation** — Repository bootstrap, ADRs, spikes, CI
- **M1 Vertical Slice** — First `.qd → PDF` (dot-call, arguments, conditional)
- **M2 Core Language** — Quarkdown core + Markdown MVP (v0.1.0)
- **M3 Programmable Documents** — Components, data loading, iteration
- **M4 Developer Experience** — Watch, inspect, source maps
- **M5 Quarkdown Compatibility Convergence** — Public-language coverage, matrix, conformance, and verified-baseline promotion
- **M6 Library, LSP, WASM** — Embedding and tooling
- **M7 Hardening** — Fuzzing, benchmarks, 1.0 release

## License

Copyright 2026 The Scribium Authors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
