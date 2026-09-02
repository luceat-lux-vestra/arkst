# ADR-0003: Typst as the Rendering Backend

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Arkst maintainers
- **Related issues:** #1

## Context

Arkst must render documents to PDF, HTML, SVG, and PNG. The choice is
between building custom renderers or delegating to an existing compiler.

## Decision Drivers

- Render quality must be competitive with professional typesetting
- Development cost must be minimal
- Must not duplicate existing mature tools
- Output format support (PDF, HTML, SVG, PNG)

## Considered Options

### Option 1: Custom PDF/HTML renderer (rejected)

Would require implementing font layout, page breaking, math rendering,
and HTML generation from scratch. Years of work for quality parity.

### Option 2: Pandoc pipeline (rejected)

Pandoc converts Markdown to many formats but cannot express Arkst's
programmable document features. Lossy round-trip, no source maps.

### Option 3: Typst compiler (chosen)

Typst provides professional-quality PDF, HTML, SVG, and PNG output.
It has a mature compiler with its own programming model. Arkst translates
its semantic model into Typst input.

## Decision

Typst is the exclusive rendering backend. Arkst never implements its own
PDF or HTML renderer. The interface is abstracted via the `TypstBackend` trait.

## Consequences

### Positive

- Professional typesetting quality from day one
- Four output formats (PDF, HTML, SVG, PNG) without custom rendering code
- Focus stays on language semantics and diagnostics

### Negative

- Arkst output quality depends on Typst compiler version
- Typst compilation errors must be mapped back to original source
- In-process embedding vs subprocess tradeoff (see ADR-0005)

### Risks

- Typst may change its output API incompatibly
- Mitigation: pin Typst version in CI and release notes

## References

- `crates/arkst-typst/src/backend.rs`
- ADR-0005 (backend strategy: subprocess vs in-process)