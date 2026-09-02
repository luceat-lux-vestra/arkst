# Arkst

[![Experimental](https://img.shields.io/badge/status-experimental-orange)](https://github.com/luceat-lux-vestra/arkst)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![CI](https://github.com/luceat-lux-vestra/arkst/actions/workflows/ci.yml/badge.svg)](https://github.com/luceat-lux-vestra/arkst/actions/workflows/ci.yml)

**Arkst is an independent, Apache-2.0 Quarkdown-compatible compiler and toolchain powered by the official Typst compiler.**

Arkst accepts Markdown and Quarkdown-compatible documents, evaluates supported programmable-document semantics into a backend-neutral IR, lowers that IR to Typst, and can invoke the official Typst compiler to produce PDF.

> Quarkdown compatibility is currently partial and evidence-based; the current verified baseline is referenced against Quarkdown v2.5.1. See [`docs/compatibility/quarkdown/`](docs/compatibility/quarkdown/) for the detailed inventory and evidence.

> Arkst is an independent project. It is not affiliated with, endorsed by, or sponsored by Typst GmbH or the Quarkdown project.

## What works today

The current verified document path is:

```text
Markdown or Quarkdown-compatible source (.md / .qd / .arkst)
→ pinned Rushdown Markdown substrate + Arkst Quarkdown frontend
→ backend-neutral IR
→ single evaluator
→ Arkst Typst lowering
→ generated Typst
→ official Typst compiler
→ PDF
```

For Markdown, Arkst has an end-to-end CommonMark/GFM compatibility harness and real-document PDF smoke coverage:

| Evidence | Current baseline |
|---|---:|
| CommonMark 0.31.2 corpus | 649 / 652 PASS, 3 accepted mismatches |
| cmark-gfm corpus | 664 / 670 PASS, 6 accepted mismatches |
| Supported real-document PDF smoke | 12 / 12 |

The supported Markdown output path includes paragraphs, headings, blockquotes, ordered and unordered lists, GFM task lists, fenced and indented code blocks, GFM tables with alignment, emphasis, strong, strikethrough, inline code, links, autolinks/linkify, thematic breaks, entities, escapes, and soft/hard line breaks.

Arkst also supports a deliberately bounded, attribute-free inline raw-HTML subset when it maps exactly to existing Markdown semantics: `<em>`, `<strong>`, `<del>`, `<s>`, and `<br>` variants. Other raw HTML is preserved with source provenance but rejected at the document-output boundary with `E8001`; Arkst does not contain a general HTML parser or DOM.

Project-relative local images are parsed, retained in the backend-neutral IR,
and lowered through the explicit Typst source-context/resource boundary.
Absolute paths, remote/network loading, and a general resource registry remain
unsupported or deferred.

**HTML, SVG, and PNG output backends are not implemented yet.** PDF output is experimental. The subprocess Typst backend is the default and is included in the default Arkst CLI build; it requires an installed Typst executable. The in-process backend is a native-only build-time opt-in through the `typst-inprocess` Cargo feature and also requires `--backend in-process` at runtime. It is unrelated to browser/WASM rendering. Generating `.typ` does not require Typst to be installed.

Detailed Markdown evidence and the accepted mismatch inventory live in [`docs/compatibility/markdown/`](docs/compatibility/markdown/).

## Quickstart

From a repository checkout:

```bash
# Validate a real Markdown example
cargo run -p arkst-cli -- check examples/markdown/basic.md

# Markdown → generated Typst
cargo run -p arkst-cli -- build examples/markdown/basic.md \
  --output target/examples/basic.typ

# Markdown → PDF (requires Typst on PATH)
cargo run -p arkst-cli -- build examples/markdown/basic.md \
  --format pdf --output target/examples/basic.pdf

# Optional native in-process PDF backend (build-time and runtime opt-in;
# does not use --typst-path)
cargo run -p arkst-cli --features typst-inprocess -- \
  build examples/markdown/basic.md \
  --format pdf --backend in-process --output target/examples/basic.pdf

# Quarkdown-compatible example
cargo run -p arkst-cli -- check examples/hello/main.qd
cargo run -p arkst-cli -- build examples/hello/main.qd \
  --output target/examples/hello.typ
```

The default `cargo build`/`cargo install` of `arkst-cli` packages only the
subprocess backend. To include the native in-process compiler graph, pass
`--features typst-inprocess` to the Cargo command and still select
`--backend in-process` when building a PDF.

Supported input extensions are `.md`, `.qd`, and `.arkst` (case-insensitive). Files without an extension are rejected. Native `.typ` passthrough is not implemented yet.

`--output` can override the destination. Missing output directories are created automatically, input files cannot be overwritten through aliases/symlinks/hard links, and output replacement is atomic on normal error-return paths. PDF builds use `--backend subprocess` by default and invoke the configured Typst executable directly rather than through a shell; use `--typst-path <PATH>` to select a specific binary. `--backend in-process` requires both the `typst-inprocess` Cargo feature and explicit runtime selection. Without the feature it fails with an instruction to rebuild; it never silently falls back to subprocess and does not enable browser/WASM rendering.

## Runnable examples

The public examples are intended to stay executable in CI rather than becoming syntax-only showcase files.

- [`examples/markdown/basic.md`](examples/markdown/basic.md) — ordinary Markdown document structure and inline syntax.
- [`examples/markdown/gfm.md`](examples/markdown/gfm.md) — task lists, tables, strikethrough, and GFM linkification/autolinks.
- [`examples/markdown/bounded-html.md`](examples/markdown/bounded-html.md) — the supported inline raw-HTML whitelist and its explicit boundary.
- [`examples/hello/main.qd`](examples/hello/main.qd) — a small Quarkdown-compatible example with variables, conditionals, strings, arithmetic, and numeric chaining.

For a larger independently authored Markdown corpus, see [`fixtures/markdown/real/`](fixtures/markdown/real/). The compatibility workflow builds the supported documents through the complete Markdown → IR → evaluator → Typst → PDF path.

## Small Markdown example

```markdown
# Release notes

Arkst preserves **structured Markdown** through a backend-neutral IR.

- [x] GFM task lists
- [x] Tables
- [x] Links and `inline code`

| Input | Output |
| :--- | ---: |
| Markdown | Typst / PDF |
```

Build it with:

```bash
cargo run -p arkst-cli -- build document.md --format pdf
```

## Quarkdown-compatible example

Arkst's programmable-document compatibility is growing in bounded, independently verified semantic slices. For example:

```quarkdown
.var {show_extra} {yes}

.if {.show_extra}
    This content is emitted lazily.

.string {hello}::concatenate with:{" world"}::capitalize
.sum {20} {22}
.pi::truncate {2}
```

The complete v2.5.1 public-language gap inventory, including unsupported and intentionally deferred surfaces, is maintained in [`docs/compatibility/quarkdown/GAP_INVENTORY.md`](docs/compatibility/quarkdown/GAP_INVENTORY.md).

## Front matter

A `---`-delimited block at the start of a document provides metadata (`title`, `author`, `date`, and custom keys). The current format is a flat line-based `key: value` form, **not full YAML**.

```markdown
---
title: My Document
author: Alice
---
# Heading
```

Nested objects, arrays, and block strings are not supported. Duplicate keys use the last occurrence, and custom metadata is stored deterministically in the IR.

## Current status

| Area | Status |
|---|---|
| Markdown/CommonMark+GFM frontend | Experimental, evidence-backed |
| Markdown → Typst | Implemented for the documented supported surface |
| Markdown → PDF | Implemented / experimental Typst process adapter |
| Bounded inline raw HTML input | Implemented / partial by design |
| General raw HTML semantics | Unsupported, fail-closed with `E8001` |
| Quarkdown variables/conditionals/callables/collections/string/numeric slices | Partial, implemented in bounded verified families |
| Quarkdown complete v2.5.1 compatibility | In progress |
| Images/resource resolution | Implemented / bounded project-relative local resources |
| Include/read/data loading | Partial / `.read`, `.json`, and `.include` through `VirtualProject` |
| HTML/SVG/PNG output | Not implemented |
| Native `.typ` passthrough | Planned |
| Watch/LSP | Planned |
| WASM | Deferred |

Arkst's canonical distribution contract approves the `arkst` CLI binary for
GitHub Releases, although no public Arkst release has shipped yet. crates.io /
public `cargo install` and distributed WASM remain disabled. The workspace
package, CLI, binary-release, WASM, and internal-tool decisions are recorded and
enforced in [`docs/engineering/DISTRIBUTION_POLICY.md`](docs/engineering/DISTRIBUTION_POLICY.md).

## Architecture

```text
┌────────────────────────────────────────────────────┐
│                    arkst-cli                     │
│              build | check | inspect               │
└──────────────────────┬─────────────────────────────┘
                       │
┌──────────────────────▼─────────────────────────────┐
│                   arkst-core                     │
│ frontend AST → IR → single evaluator               │
└──────────────────────┬─────────────────────────────┘
                       │ evaluated backend-neutral IR
┌──────────────────────▼─────────────────────────────┐
│                   arkst-typst                    │
│              pure IR → Typst lowering              │
└──────────────────────┬─────────────────────────────┘
                       │
                       ▼
              official Typst compiler
                       │
                       ▼
                      PDF
```

Rushdown is the pinned Markdown parser substrate. Arkst does not preprocess and reparse Markdown to implement Quarkdown semantics; programmable semantics are preserved structurally through the frontend AST and IR and evaluated once before backend lowering.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/adr/`](docs/adr/) for the architectural contracts.

## Roadmap

- **M0 Foundation** — repository bootstrap, ADRs, spikes, CI
- **M1 Vertical Slice** — first `.qd → PDF`
- **M2 Core Language** — Quarkdown core + Markdown MVP
- **M3 Programmable Documents** — components, host/data loading, richer document semantics
- **M4 Developer Experience** — watch, inspect, source maps
- **M5 Quarkdown Compatibility Convergence** — public-language coverage and verified-baseline promotion
- **M6 Library, LSP, WASM** — embedding and tooling
- **M7 Hardening** — fuzzing, benchmarks, 1.0 release

## License

Copyright 2026 The Arkst Authors

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. See [`LICENSE`](LICENSE).
