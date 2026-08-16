# Markdown/CommonMark+GFM baseline audit

Audit date: 2026-08-16

This document records the end-to-end Markdown baseline at the pinned
Rushdown revision. It distinguishes parser capability from Scribium output
capability; a feature being parsed by Rushdown is not by itself an
end-to-end Scribium compatibility claim.

## Substrate and evidence

- Rushdown version: `0.18.0`
- Rushdown revision: `e5eb4e4446541ea0ed53111c1b37e779283ff57c`
- Dependency features used by Scribium: `std`, `html-entities`
- The pinned dependency, its public node model, parser extensions, and its
  CommonMark/GFM tests were inspected without modifying Rushdown.
- Representative PDF fixture:
  [`fixtures/markdown/commonmark_gfm_baseline.md`](../../../fixtures/markdown/commonmark_gfm_baseline.md)
- PDF integration coverage:
  `crates/scribium-typst/tests/backend_integration.rs`
- Frontend and source-span coverage:
  `crates/scribium-markdown/src/parser.rs` and
  `crates/scribium-markdown/tests/range_invariants.rs`
- UTF-8/CRLF semantic break coverage:
  `crates/scribium-core/src/lib.rs` and
  `crates/scribium-typst/tests/backend_integration.rs`

The existing `KNOWN_UPSTREAM_SOUNDNESS_RISK_ACCEPTED` decision is unchanged.
Rushdown remains the Markdown parser substrate; this audit introduces no
fork, patch, upgrade, or source reconstruction path.

## Capability matrix

`Yes*` means the semantic information is available with a stated limitation.
`No` means the current layer has a deliberate, source-backed gap rather than
silently flattening the syntax into another Markdown node.

| Feature | Rushdown parses? | Scribium frontend AST? | IR? | Typst lowering? | PDF E2E? | Status / gap |
|---|---:|---:|---:|---:|---:|---|
| Paragraphs | Yes | Yes | Yes | Yes | Yes | Representative fixture coverage. |
| ATX headings | Yes | Yes | Yes | Yes | Yes | Heading level preserved. |
| Setext headings | Yes | Yes | Yes | Yes | Yes | Heading level preserved; delimiter kind is not currently carried. |
| Thematic breaks | Yes | Yes | Yes | Yes | Yes | Lowered to a Typst line. |
| Blockquotes | Yes | Yes | Yes | Yes | Yes | Nested blockquotes are recursive. |
| Unordered lists | Yes | Yes | Yes | Yes | Yes | Nested and mixed list content is retained. |
| Ordered lists | Yes | Yes | Yes | Yes | Yes | Start ordinal is retained. |
| GFM task lists | Yes | Yes | Yes | Yes | Yes | Active/completed task state is retained. |
| Fenced code blocks | Yes | Yes | Yes | Yes | Yes | Body and first-token language are lowered; full info string is retained in AST/IR. |
| Indented code blocks | Yes | Yes | Yes | Yes | Yes | Lowered as code block without a language. |
| GFM tables | Yes | Yes | Yes | Yes | Yes | Alignment and nested inline structure are retained. |
| Plain text | Yes | Yes | Yes | Yes | Yes | Original source spans are retained. |
| Emphasis | Yes | Yes | Yes | Yes | Yes | Nested inline structure is retained. |
| Strong | Yes | Yes | Yes | Yes | Yes | Nested inline structure is retained. |
| Strikethrough | Yes | Yes | Yes | Yes | Yes | GFM extension; covered by existing and baseline tests. |
| Inline code | Yes | Yes | Yes | Yes | Yes | Opaque code-span content is preserved. |
| Links | Yes | Yes | Yes | Yes | Yes | Label, destination, title metadata, and source span are retained; Typst emits clickable links. |
| Images | Yes | Yes | Yes* | No | No | Destination, alt content, title, and span are retained in IR; resource resolution is intentionally deferred and emits deterministic E8001. No network or arbitrary filesystem access is added. |
| Soft line breaks | Yes | Yes | Yes | Yes | Yes | Preserved as an IR break and lowered as a Typst newline; UTF-8 + CRLF is covered end-to-end. |
| Hard line breaks | Yes | Yes | Yes | Yes | Yes | Backslash/two-space delimiter and UTF-8 + CRLF span remain source-backed; lowered as Typst hard break and PDF-tested. |
| Escaped punctuation | Yes | Yes | Yes | Yes | Yes | Normalized through Rushdown's public `unescape_puncts` utility and re-escaped for Typst markup. |
| Entities | Yes | Yes | Yes | Yes | Yes | HTML and numeric references are resolved through Rushdown's public utilities while retaining original spans. |
| Autolinks | Yes | Yes | Yes | Yes | Yes | URI autolinks and GFM linkify are lowered as clickable links. |
| Inline HTML | Yes | Yes | No | No | No | Preserved as frontend `RawHtml`; current IR/output path reports E8001. HTML normalization remains outside this slice. |
| Block HTML | Yes | Yes | No | No | No | Preserved as frontend `RawHtml`; current IR/output path reports E8001. |

## Scope boundary

The successful PDF fixture intentionally contains only features with a
complete current output path. Images and HTML remain explicit compatibility
gaps because implementing them safely requires a resource/HTML ownership
decision outside this baseline audit. This PR does not add network fetching,
arbitrary filesystem access, a Markdown parser, a Rushdown change, or a
backend-rendered HTML semantic path.

The target product path demonstrated here is:

```text
Rushdown -> Scribium Markdown AST -> scribium-core IR/evaluator
         -> scribium-typst -> Typst -> valid PDF
```
