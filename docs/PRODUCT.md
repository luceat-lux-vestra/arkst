# PRODUCT — Scribium

## Problem Statement

Markdown offers excellent authoring ergonomics and ecosystem compatibility
but lacks precise typesetting and programmatic document features.

Typst provides high-quality typesetting and programming capabilities but
requires learning a new syntax and migrating existing Markdown documents.

Quarkdown-style functional Markdown extensions are compelling for developers
but are tied to specific implementations and licenses.

Scribium bridges these gaps with an independent, permissively licensed
implementation that combines Markdown familiarity with programmable documents
and Typst-quality output.

## Target Users

1. Developers writing technical documentation, reports, lecture materials,
   books, and slides in Markdown
2. Users who need both PDF and web output from a single source
3. Users who need variables, conditionals, iteration, and components beyond
   plain Markdown
4. Users who want Typst typesetting without writing documents entirely in Typst
5. Users migrating existing Quarkdown-style documents to an independent toolchain
6. Developers embedding document compilation in Rust, JavaScript, or WASM apps

## Jobs to Be Done

- **Author in familiar syntax:** Write documents using Markdown and
  Quarkdown-compatible extensions
- **Program documents:** Use variables, conditionals, loops, and components
  to generate dynamic content
- **Compile to multiple formats:** Produce PDF, HTML, SVG, and PNG from one source
- **Check without compiling:** Validate documents incrementally
- **Watch and iterate:** Automatic rebuild on source changes
- **Inspect intermediates:** Debug the compilation pipeline
- **Migrate existing documents:** Transition from Quarkdown or plain Markdown

## Product Principles

1. **Quarkdown compatibility is a first-class requirement** — not a deferred
   milestone or optional plugin
2. **Typst is the rendering backend** — no custom PDF/HTML renderers
3. **One CLI, one config, one project model** — users don't juggle tools
4. **Semantic parity, not pixel parity** — PDF and HTML serve different purposes
5. **Escape hatch** — users can insert native Typst when needed
6. **Safe by default** — no shell execution, no network, scoped filesystem
7. **Deterministic** — same input always produces same output
8. **Diagnostics first** — clear error messages with source locations

## User Journeys

### New Document

```
scribium new my-report
→ creates project structure with template
→ edit source files
→ scribium build → PDF
```

### Iterative Writing

```
scribium watch docs/
→ edit source in editor
→ auto-rebuild on save
→ PDF updates in viewer
```

### CI/CD Pipeline

```
scribium check src/ --format json
→ structured diagnostics
→ fail on error
```

## Output Targets

- **PDF** (via Typst compiler)
- **HTML** (via Typst compiler)
- **SVG** (via Typst compiler)
- **PNG** (via Typst compiler)
- **Bundle** (multiple formats in one command)

## Differentiation

- **Independent clean-room implementation** — no dependency on Quarkdown codebase
- **Apache-2.0** — permissive licensing for commercial use
- **Single tool** — one CLI handles all input formats
- **Diagnostic quality** — structured errors with source locations
- **Source maps** — Typst diagnostics mapped back to original source
- **Safe defaults** — no shell execution, scoped filesystem, resource limits

## Non-Goals

- Custom PDF/HTML renderer
- SaaS/web editor
- Custom package registry
- Unlimited network access from documents
- Shell execution from documents
- 100% Quarkdown compatibility at v0.1

## Success Metrics

- Clean checkout → `scribium build example.qd` → PDF works
- Typst compilation errors show original source location
- Generated Typst is deterministic and inspectable
- CI passes on Linux, macOS, Windows
- Snapshot tests catch regressions