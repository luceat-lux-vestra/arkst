# Scribium

[![Experimental](https://img.shields.io/badge/status-experimental-orange)](https://github.com/luceat-lux-vestra/scribium)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![CI](https://github.com/luceat-lux-vestra/scribium/actions/workflows/ci.yml/badge.svg)](https://github.com/luceat-lux-vestra/scribium/actions/workflows/ci.yml)

**Scribium is an independent, Apache-2.0 Quarkdown-compatible compiler and toolchain powered by the official Typst compiler.**

> Scribium is an independent project. It is not affiliated with, endorsed by, or
> sponsored by Typst GmbH or the Quarkdown project.

---

## What is Scribium?

Scribium independently implements the Quarkdown syntax and core execution semantics,
connecting them to the official Typst compiler for high-quality typesetting.

```
Quarkdown-compatible source (.qd / .scrib)
or Markdown / native Typst
→ clean-room parser and evaluator
→ Quarkdown-compatible semantic model
→ Typst-oriented IR
→ Typst lowering
→ official Typst compiler
→ PDF / HTML / SVG / PNG
```

Quarkdown compatibility is a first-class requirement — not a deferred milestone.
Scribium reimplements the language independently from public documentation and
black-box behavior. No Quarkdown source code is copied or translated.

## Quickstart

```bash
# Build a document to generated Typst source (document.qd → document.typ)
scribium build examples/hello/main.qd

# Override the output path
scribium build examples/hello/main.qd --output out/main.typ

# Check for errors without compiling
scribium check examples/hello/main.qd

# Inspect intermediate representations
scribium inspect examples/hello/main.qd --emit typst

# Build a Markdown input (report.md → report.typ)
scribium build report.md
```

> The build refuses to overwrite the input file: an input with a `.typ`
> extension, or an explicit `--output` equal to the input path, is rejected.
> PDF/HTML/SVG/PNG backends are not implemented yet; requesting them fails
> with a clear error.

## Example (.qd)

```quarkdown
@import "template.typ"

@heading(level: 1)[Hello, Scribium]

This is a @strong[Quarkdown-compatible] document compiled through Typst to PDF.

@list[
  @item[Simple @code[lists] work]
  @item[Math is supported: $E = mc^2$]
]

@row[
  @col[Left]
  @col[Right]
]
```

## Front matter

A `---`-delimited block at the start of a document provides metadata
(`title`, `author`, `date`, and custom keys). The supported format is a
flat line-based `key: value` form, **not full YAML**:

- Keys and values are split on the first colon.
- Nested objects, arrays, and block strings are **not** supported.
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
| Dot-prefixed function calls             | Planned      |
| Positional/named/body arguments         | Planned      |
| Variables and conditionals              | Planned      |
| Iteration and components                | Planned      |
| Tables, math, footnotes                 | Planned      |
| Include/read, data loading              | Planned      |
| Typst escape blocks                     | Planned      |
| Quarkdown compatibility                 | Planned      |
| `watch` mode, source maps               | Planned      |
| LSP integration                         | Planned      |
| WASM support                            | Deferred     |

## Architecture

```
┌────────────────────────────────────────────────────┐
│                    scribium-cli                     │
│  build | check | inspect | watch                   │
└──────────────────────┬─────────────────────────────┘
                       │
┌──────────────────────▼─────────────────────────────┐
│                   scribium-core                     │
│  Markdown + Quarkdown-compatible syntax             │
│  → Parse → Semantic → Eval → IR → Lower → SourceMap│
│  ┌──────────────────────────────────────────────┐   │
│  │ compatibility/ (profile, divergence, diag)   │   │
│  └──────────────────────────────────────────────┘   │
└──────────────────────┬─────────────────────────────┘
                       │
┌──────────────────────▼─────────────────────────────┐
│                   scribium-typst                    │
│  TypstBackend trait → Subprocess/InProcess adapter │
│  → Typst Compiler → PDF / HTML / SVG / PNG         │
└────────────────────────────────────────────────────┘
```

## Roadmap

- **M0 Foundation** — Repository bootstrap, ADRs, spikes, CI
- **M1 Vertical Slice** — First `.qd → PDF` (dot-call, arguments, conditional)
- **M2 Core Language** — Quarkdown core + Markdown MVP (v0.1.0)
- **M3 Programmable Documents** — Components, data loading, iteration
- **M4 Developer Experience** — Watch, inspect, source maps
- **M5 Quarkdown Compatibility** — Expanded subset, matrix, conformance
- **M6 Library, LSP, WASM** — Embedding and tooling
- **M7 Hardening** — Fuzzing, benchmarks, 1.0 release

## License

Copyright 2026 The Scribium Authors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0