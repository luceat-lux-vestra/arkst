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
| Images | Yes | Yes | Yes | Yes | Yes | Inline and reference images retain nested alt content, logical destination, title, and source span. Project-relative local images resolve from the source entry directory through the explicit Typst source context; absolute paths and URI schemes are rejected with E8001, and no network fetching is performed. |
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

The successful PDF fixtures contain only features with a complete current
output path. Local relative Markdown images are now included in that path:
their logical destination is resolved from the source document directory
inside the explicit project root. Parent-relative references remain valid only
when they stay inside that root; missing files and backend-unsupported formats
fail through Typst, while absolute paths, URI schemes, and network fetching are
unsupported. Image alt content and titles remain in AST/IR for future
accessibility or HTML work, but are not emitted as PDF accessibility metadata
by the current Typst backend.

The slice still does not add arbitrary filesystem access, a Markdown parser, a
Rushdown change, a DOM, CSS, JavaScript, or a backend-rendered HTML semantic
path.

The target product path demonstrated here is:

```text
Rushdown -> Scribium Markdown AST -> scribium-core IR/evaluator
         -> scribium-typst -> Typst -> valid PDF
```

## Differential compatibility harness

The repository now contains a pinned, end-to-end Markdown compatibility
harness. It is a measurement and regression layer, not a second production
parser and not a claim of complete CommonMark or GFM support.

For each corpus example, the harness runs the pinned reference parser in XML
mode and maps that XML to a small canonical document tree. Scribium parses the
same source through its normal Markdown frontend and maps the frontend AST to
the same canonical tree. The comparison is structural: it does not compare
reference HTML strings with Scribium Typst strings. Source provenance is a
separate test layer and is not inferred from cmark source positions.

The canonical model covers document structure, headings, paragraphs, inline
text, emphasis, strong, strikethrough, blockquotes, nested ordered and
unordered lists, ordered starts, task-list state, fenced and inline code,
links, images, tables and alignment, thematic breaks, soft and hard breaks,
and raw HTML structure where the reference exposes it. Reference-private
structures are intentionally omitted. The harness reports `PASS`,
`KNOWN_MISMATCH`, `UNSUPPORTED`, and `HARNESS_ERROR`; every harness error
fails CI.

### Pinned references and provenance

All reference inputs are pinned by version and full commit in
[`tests/compat/references.toml`](../../../tests/compat/references.toml). The
preparation script checks out the exact commits and the CI job verifies the
checked-in corpus byte-for-byte against the pinned sources.

| Role | Version | Revision | Source and license |
|---|---|---|---|
| CommonMark specification corpus | `0.31.2` | `9103e341a973013013bb1a80e13567007c5cef6f` | [commonmark/commonmark-spec](https://github.com/commonmark/commonmark-spec), CC-BY-SA-4.0 |
| CommonMark reference parser | `0.31.2` | `eec0eeba6d31189fd828314576494566d539b1e3` | [commonmark/cmark](https://github.com/commonmark/cmark), BSD-2-Clause and component licenses in `COPYING` |
| GFM specification/parser corpus | `0.29.0.gfm.13` | `587a12bb54d95ac37241377e6ddc93ea0e45439b` | [github/cmark-gfm](https://github.com/github/cmark-gfm), BSD-2-Clause and component licenses in `COPYING` |

The cmark and cmark-gfm repositories are test-oracle inputs only. They are
not Scribium production dependencies, and their source and tests are not
copied into the implementation.

### Current corpus baseline

The checked-in corpus contains all 652 enabled CommonMark 0.31.2 examples and
670 enabled cmark-gfm examples. The current observed result at the pinned
revisions is:

| Suite | Total | PASS | KNOWN_MISMATCH | UNSUPPORTED | New mismatch |
|---|---:|---:|---:|---:|---:|
| CommonMark | 652 | 649 | 3 | 0 | 0 |
| GFM | 670 | 664 | 6 | 0 | 0 |

The baseline files
[`commonmark.json`](../../../tests/compat/baselines/commonmark.json) and
[`gfm.json`](../../../tests/compat/baselines/gfm.json) store stable case IDs,
the corpus/reference revisions, and the explicitly accepted current non-PASS
exception set. They are reviewed data, never automatically updated by CI.

CI fails when a prior pass becomes a mismatch, when a new mismatch appears,
when a corpus case disappears, or when a pinned revision changes without a
review-visible baseline/corpus update. Existing known mismatches remain
visible in the report. A known mismatch that becomes a pass is reported as an
improvement but also makes the baseline exception stale, so CI requires the
entry to be explicitly removed. After removal, the pass is accepted without an
exception and any later mismatch is a new mismatch that fails CI. A change
between `KNOWN_MISMATCH` and `UNSUPPORTED` also fails until the baseline is
reviewed. The mismatch count is therefore a snapshot of measured gaps, not a
completeness score.

The first root-cause remediation batch corrected Rushdown code-segment
serialization at the Scribium frontend boundary. It preserves segment padding
and forced newlines instead of joining source slices with synthetic separators;
the change resolves 44 CommonMark and 44 GFM cases without changing the
Rushdown pin or the evaluator/IR/backend pipeline.

The next remediation batch corrected positioned empty-node span recovery at the
same frontend boundary. It uses Rushdown node positions and metadata to retain
the original source-backed line or delimiter span, resolving 34 CommonMark and
32 GFM cases. The batch adds independently authored UTF-8, LF, CRLF, nested,
empty, adjacent, and real-document coverage without changing the Rushdown pin.

The current metadata remediation batch normalizes source-backed link
destination/title values, including reference-definition metadata, and fenced
code info strings at the frontend boundary. It resolves 9 CommonMark and 9 GFM
cases, including the inline-link entity sub-group, without changing source
spans, the Rushdown pin, or the IR/evaluator/backend pipeline. The independently
authored metadata regression suite covers escapes, named and numeric entities,
invalid entity-like text, UTF-8, CRLF, nested title forms, and language
extraction; real-document smoke now includes `14-metadata-normalization.md`.

The complete root-cause analysis, case-level ownership classification, and
final accepted groups are in
[`gaps.md`](gaps.md). Remaining differences cover the leading front-matter
policy boundary, HTML canonicalization, and pinned GFM autolink/linkify
behavior. The final audit found no actionable Scribium-owned Markdown defect.
Quarkdown `.let` evaluator semantics are tracked separately under Issue #61;
this Markdown audit does not assess programmable-document behavior.

### Real document corpus and output smoke

The independently authored mixed-feature corpus is in
[`fixtures/markdown/real`](../../../fixtures/markdown/real) and is described
by its [`manifest.json`](../../../fixtures/markdown/real/manifest.json). It
contains 14 documents covering headings, nested containers, lists, tables,
task items, strikethrough, code/info strings, links, autolinks, bounded HTML,
Unicode, metadata escapes/entities, LF, and CRLF. The supported twelve documents are required to complete
the normal pipeline and produce a non-empty valid PDF:

```text
Markdown -> frontend AST -> IR -> evaluator -> Typst -> Typst compiler -> PDF
```

The current smoke result is 12/12 successful PDFs. Two HTML-policy fixtures
are expected unsupported and must produce E8001; they are evidence of the
bounded output policy, not silently accepted documents. PDFs are generated in
CI artifacts and are not committed. Validation checks the PDF header and
non-empty output, while generated Typst is checked for the manifest's textual
markers; no PDF byte-for-byte golden is used.

### Raw HTML and differential observations

Raw HTML is reported separately from general differential equality:

1. frontend representation matches the expected raw-HTML structure;
2. the exact attribute-free bounded subset is semantically supported;
3. complete HTML can be preserved with provenance but is output-unsupported;
4. malformed or incomplete forms are rejected or remain parser recovery, with
   no HTML semantic claim.

The real corpus explicitly covers `<em>x</em>`, `<strong>x</strong>`,
`<del>x</del>`, `<s>x</s>`, `<br>`, unsupported `<span>x</span>`, block HTML,
comments, malformed mismatched tags, and mixed Markdown/HTML. The harness
records structural differences if the reference observation and the #71
bounded policy diverge; it does not broaden the whitelist to make a case pass.

### CI reports

The separate `markdown-compat` Ubuntu job prepares the pinned references,
rebuilds both reference parsers, verifies the checked-in corpora, runs the
differential and real-document checks, and uploads an artifact containing:

- `compatibility-report.json` and `compatibility-report.md`;
- per-case failed diffs under `failed-case-diffs/`;
- generated real-document Typst under `real-typst/`;
- generated real-document PDFs under `real-pdf/` when available.

The local equivalent writes these files under the selected output directory,
for example `target/markdown-compat-official/`. A `PASS` means canonical
parser/document semantic equality for the pinned case only. It does not mean
Markdown complete, CommonMark fully compliant, or GFM fully compliant.
