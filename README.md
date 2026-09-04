# Arkst

[![Experimental](https://img.shields.io/badge/status-experimental-orange)](https://github.com/luceat-lux-vestra/arkst)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![CI](https://github.com/luceat-lux-vestra/arkst/actions/workflows/ci.yml/badge.svg)](https://github.com/luceat-lux-vestra/arkst/actions/workflows/ci.yml)

**An independent, Apache-2.0 Quarkdown-compatible document compiler and toolchain powered by the official Typst compiler.**

Arkst accepts Markdown and Quarkdown-compatible documents, evaluates supported programmable-document semantics into a backend-neutral IR, lowers that IR to Typst, and can invoke the official Typst compiler to produce PDF.

> Quarkdown compatibility is partial and evidence-based. The current verified baseline is referenced against Quarkdown v2.5.1 under [`docs/compatibility/quarkdown/`](docs/compatibility/quarkdown/).
>
> Arkst is independent and is not affiliated with, endorsed by, or sponsored by Typst GmbH or the Quarkdown project.

## Document pipeline

```text
Markdown / Quarkdown-compatible source
        │
        ▼
pinned Rushdown Markdown substrate
+ Arkst Quarkdown frontend
        │
        ▼
backend-neutral IR
        │
        ▼
single evaluator
        │
        ▼
Arkst Typst lowering
        │
        ▼
generated Typst
        │
        ▼
official Typst compiler
        │
        ▼
PDF
```

Programmable semantics are preserved structurally through the frontend and IR. Arkst does not implement Quarkdown semantics by preprocessing text and reparsing Markdown.

## Verified Markdown baseline

The Markdown path has corpus and real-document evidence:

| Evidence | Current baseline |
|---|---:|
| CommonMark 0.31.2 corpus | 649 / 652 PASS, 3 accepted mismatches |
| cmark-gfm corpus | 664 / 670 PASS, 6 accepted mismatches |
| Supported real-document PDF smoke | 12 / 12 |

The supported Markdown surface includes paragraphs, headings, blockquotes, ordered/unordered lists, GFM task lists and tables, fenced/indented code blocks, emphasis, strong, strikethrough, inline code, links, autolinks/linkify, thematic breaks, entities, escapes, and soft/hard line breaks.

Detailed evidence and accepted mismatch inventories live under [`docs/compatibility/markdown/`](docs/compatibility/markdown/).

## Quick start

From a repository checkout:

```bash
# Validate Markdown
cargo run -p arkst-cli -- check examples/markdown/basic.md

# Markdown → Typst
cargo run -p arkst-cli -- build examples/markdown/basic.md \
  --output target/examples/basic.typ

# Markdown → PDF (Typst must be on PATH)
cargo run -p arkst-cli -- build examples/markdown/basic.md \
  --format pdf --output target/examples/basic.pdf

# Quarkdown-compatible input
cargo run -p arkst-cli -- check examples/hello/main.qd
cargo run -p arkst-cli -- build examples/hello/main.qd \
  --output target/examples/hello.typ
```

Supported input extensions are `.md`, `.qd`, and `.arkst` (case-insensitive). Files without an extension are rejected. Native `.typ` passthrough is not implemented yet.

## PDF backends

The default CLI build uses the subprocess Typst backend and requires a Typst executable.

An optional native in-process backend is available only when the CLI is built with the `typst-inprocess` feature and explicitly selected at runtime:

```bash
cargo run -p arkst-cli --features typst-inprocess -- \
  build examples/markdown/basic.md \
  --format pdf --backend in-process --output target/examples/basic.pdf
```

The in-process backend is a native build-time/runtime opt-in. It is not a browser/WASM rendering path and never silently falls back to subprocess when the required feature is missing.

Generating `.typ` does not require Typst to be installed.

## Quarkdown-compatible semantics

Arkst implements Quarkdown compatibility in bounded, independently verified semantic slices. Current implemented families include portions of variables, conditionals, callables, collections, strings, numerics, data loading, and related document semantics.

For example:

```quarkdown
.var {show_extra} {yes}

.if {.show_extra}
    This content is emitted lazily.

.string {hello}::concatenate with:{" world"}::capitalize
.sum {20} {22}
.pi::truncate {2}
```

The complete public-language gap inventory, including unsupported and intentionally deferred surfaces, is maintained in [`docs/compatibility/quarkdown/GAP_INVENTORY.md`](docs/compatibility/quarkdown/GAP_INVENTORY.md).

## Resources and front matter

Project-relative local images are supported through the explicit source-context/resource boundary. Absolute paths, remote/network loading, and a general resource registry remain unsupported or deferred.

A `---`-delimited block at the start of a document provides flat metadata such as `title`, `author`, `date`, and custom keys:

```markdown
---
title: My Document
author: Alice
---
# Heading
```

This is a flat line-based `key: value` format, **not full YAML**. Nested objects, arrays, block strings, and general YAML semantics are not claimed.

## Raw HTML boundary

Arkst supports a deliberately bounded attribute-free inline raw-HTML subset when it maps exactly to existing Markdown semantics: `<em>`, `<strong>`, `<del>`, `<s>`, and `<br>` variants.

Other raw HTML is preserved with source provenance but rejected at the document-output boundary with `E8001`. Arkst does not contain a general HTML parser or DOM.

## Current status

| Area | Status |
|---|---|
| Markdown/CommonMark+GFM frontend | Experimental, evidence-backed |
| Markdown → Typst | Implemented for the documented supported surface |
| Markdown → PDF | Implemented / experimental Typst adapter |
| Bounded inline raw HTML | Implemented / partial by design |
| General raw HTML semantics | Unsupported, fail-closed with `E8001` |
| Quarkdown programmable semantics | Partial, verified in bounded families |
| Quarkdown v2.5.1 complete compatibility | In progress |
| Project-relative local images | Implemented within the bounded resource model |
| Include/read/data loading | Partial |
| HTML/SVG/PNG output | Not implemented |
| Native `.typ` passthrough | Planned |
| Watch/LSP | Planned |
| WASM | Deferred |

No public Arkst release has shipped yet. The approved GitHub distribution contract currently targets the `arkst` CLI binary; crates.io/public `cargo install` and distributed WASM remain disabled. See [`docs/engineering/DISTRIBUTION_POLICY.md`](docs/engineering/DISTRIBUTION_POLICY.md).

## Architecture

```text
┌─────────────────────────────────────────────┐
│                  arkst-cli                  │
│             build | check | inspect         │
└──────────────────────┬──────────────────────┘
                       │
┌──────────────────────▼──────────────────────┐
│                 arkst-core                  │
│         frontend AST → IR → evaluator       │
└──────────────────────┬──────────────────────┘
                       │ evaluated IR
┌──────────────────────▼──────────────────────┐
│                arkst-typst                  │
│              IR → Typst lowering            │
└──────────────────────┬──────────────────────┘
                       ▼
             official Typst compiler
                       ▼
                      PDF
```

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/adr/`](docs/adr/) for the architectural contracts.

## Runnable examples

- [`examples/markdown/basic.md`](examples/markdown/basic.md) — ordinary Markdown structure and inline syntax.
- [`examples/markdown/gfm.md`](examples/markdown/gfm.md) — task lists, tables, strikethrough, linkification/autolinks.
- [`examples/markdown/bounded-html.md`](examples/markdown/bounded-html.md) — supported raw-HTML whitelist and failure boundary.
- [`examples/hello/main.qd`](examples/hello/main.qd) — small Quarkdown-compatible programmable document.

These examples are intended to remain executable in CI rather than becoming syntax-only showcase files.

## Roadmap

- **M0 Foundation** — repository bootstrap, ADRs, spikes, CI
- **M1 Vertical Slice** — first `.qd → PDF`
- **M2 Core Language** — Quarkdown core + Markdown MVP
- **M3 Programmable Documents** — components, host/data loading, richer semantics
- **M4 Developer Experience** — watch, inspect, source maps
- **M5 Compatibility Convergence** — public-language coverage and verified-baseline promotion
- **M6 Library, LSP, WASM** — embedding and tooling
- **M7 Hardening** — fuzzing, benchmarks, 1.0 release

## License

Copyright 2026 The Arkst Authors

Licensed under the [Apache License, Version 2.0](LICENSE).
