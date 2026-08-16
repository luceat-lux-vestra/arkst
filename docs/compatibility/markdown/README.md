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
- Raw HTML audit and adapter coverage:
  `crates/scribium-markdown/tests/raw_html.rs`
- Raw HTML semantic/diagnostic coverage:
  `crates/scribium-core/src/ast_to_ir.rs`,
  `crates/scribium-cli/src/commands.rs`, and
  `crates/scribium-typst/tests/backend_integration.rs`
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
| Inline HTML: exact attribute-free emphasis/strong/strike/break subset | Yes | Yes (`RawHtml` segments with source spans) | Yes; existing `Emphasis`, `Strong`, `Strikethrough`, and `HardBreak` nodes | Yes | Yes | Only `<em>...</em>`, `<strong>...</strong>`, `<del>...</del>`, `<s>...</s>`, and `<br>`, `<br/>`, or `<br />`; tags are matched from Rushdown's sibling segments without an HTML parser. Nested combinations are covered when every tag is in this whitelist. |
| Other valid inline HTML | Yes | Yes (`RawHtml` segments with source spans) | No | No | No | Attributes, unknown elements, comments, declarations, processing instructions, CDATA, and ambiguous/mismatched combinations remain opaque and produce E8001. They are never flattened into a successful output path. |
| Block HTML | Yes, for Rushdown HTML block kinds 1–7 | Yes (`RawHtml` with the complete source span) | No | No | No | The block is preserved as one opaque source-backed node; AST→IR emits E8001 and the CLI refuses Typst/PDF output. Markdown-looking text inside the block is not parsed as Markdown. |
| Malformed/incomplete HTML-like input | Not as `RawHtml` | Ordinary Rushdown/Markdown text or recovery nodes, according to parser context | No HTML semantic claim | No HTML semantic claim | No HTML semantic claim | The adapter does not reinterpret incomplete input as HTML. This is parser behavior, not supported HTML compatibility. |

## Pinned Rushdown raw HTML audit

The following observations are from Rushdown `0.18.0` at
`e5eb4e4446541ea0ed53111c1b37e779283ff57c`, not from CommonMark prose alone.
The adapter tests in
`crates/scribium-markdown/tests/raw_html.rs` are the independent regression
evidence for this table.

The classification codes are: **A**, structurally exposed enough for the
bounded semantic adapter; **B**, exposed only as opaque source-backed
`RawHtml`; **C**, not represented by the pinned parser; and **D**,
parser-dependent or ambiguous and therefore unsupported.

| Input form | Pinned Rushdown representation | Adapter classification | Policy |
|---|---|---|---|
| Complete inline opening/closing tags | Each accepted tag is one inline `RawHtml` node; opening and closing tags are separate siblings. Rushdown does not expose a paired element or attribute model. | A only for the exact attribute-free whitelist; otherwise B/D. | Whitelist tags can use existing IR structure. All other tags are preserved and diagnosed. |
| Self-closing inline tags | One inline `RawHtml` node. | A for `<br>`, `<br/>`, and `<br />`; B otherwise. | Supported break forms lower to the existing hard-break IR node. |
| Quoted or unquoted attributes | One opaque inline `RawHtml` node containing the original tag source. | B. | Attributes are not semantically inspected or discarded; E8001 is emitted. |
| Comments | One opaque inline `RawHtml` node when complete; block-position comments use an opaque HTML block. | B. | Preserved with provenance and rejected at AST→IR with E8001. |
| Declarations and processing-instruction-like forms | One opaque inline `RawHtml` node when complete; block-position forms use an opaque HTML block. | B. | No declaration, PI, or XML semantics are added. E8001 is emitted. |
| CDATA | One opaque inline `RawHtml` node when complete; block-position CDATA uses an opaque HTML block. | B. | Preserved and diagnosed; no text extraction is performed. |
| Nested tags | No HTML nesting tree. Markdown children can still occur between sibling raw tag nodes. | A only when the complete sibling sequence consists of whitelist tags and ordinary Markdown children; otherwise D. | The bounded adapter matches a whitelist stack only. It does not parse arbitrary HTML. |
| HTML containing Markdown-looking text | Inline text and Markdown nodes remain whatever Rushdown exposed; block HTML remains one opaque block and its body is not Markdown-parsed. | D when equivalence depends on unavailable HTML boundaries; B for opaque blocks. | No synthetic Markdown or HTML-to-Markdown reparsing is performed. |
| Markdown surrounding HTML | Ordinary Markdown siblings remain separate from inline `RawHtml`; block HTML is separated by Rushdown block boundaries. | A for the whitelist; B/D otherwise. | Supported inline semantics do not consume surrounding Markdown. |
| UTF-8, LF, and CRLF | Source-backed segments retain byte offsets in the original source; CRLF remains part of the source ranges where Rushdown includes it. | A for provenance; semantic classification still depends on the construct. | AST and diagnostics retain original byte spans. |
| Malformed or incomplete forms | The inline scanner declines incomplete tags/comments/PI/declarations/CDATA, so they are not `RawHtml`; block recognition is also context- and terminator-dependent. | C or D. | Scribium does not reinterpret parser-rejected input as HTML. |

The audit therefore has three policy outcomes:

- **Semantically supported:** only the exact, attribute-free inline whitelist
  listed in the capability matrix. It maps to existing backend-neutral IR and
  has real Typst/PDF coverage.
- **Preserved but unsupported:** every other complete raw HTML construct that
  Rushdown exposes. The frontend retains the original source and span, while
  AST→IR emits deterministic E8001.
- **Rejected / diagnostic:** an unsupported raw HTML node is an error at the
  document-output boundary. The CLI does not write Typst or invoke a PDF
  backend after E8001. Ambiguous or mismatched whitelist nesting is handled
  the same way rather than being partially lowered.

## Scope boundary

The successful PDF fixture intentionally contains only features with a
complete current output path. Images and non-whitelist HTML remain explicit
compatibility gaps because resource resolution and general HTML normalization
are outside this bounded slice. This PR does not add network fetching,
arbitrary filesystem access, a Markdown parser, a Rushdown change, a DOM, CSS,
JavaScript, or a backend-rendered HTML semantic path.

The target product path demonstrated here is:

```text
Rushdown -> Scribium Markdown AST -> scribium-core IR/evaluator
         -> scribium-typst -> Typst -> valid PDF
```
