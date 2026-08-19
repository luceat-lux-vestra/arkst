# Markdown differential gap analysis

Audit date: 2026-08-16

This is the root-cause analysis of the differential baseline that was live on
`origin/main` at `89863edee4d7713ddb44e9e466d382c00779372c`. The analysis was
performed against the checked-in corpus and the pinned reference executables;
it does not infer causes from section totals alone.

The latest-main revalidation used that live `origin/main` after the
positioned-empty span remediation. The metadata remediation was then measured
against the same pinned corpus and reference executables; its real-document
run includes the independently authored `14-metadata-normalization.md`
fixture.

## Current image support slice

Markdown image syntax is no longer an output-path gap. The pinned Rushdown
frontend exposes inline and reference images, and Scribium preserves each
image's nested alt inline tree, logical destination, optional title, and
source-backed span. `scribium-core` keeps that logical reference in the
backend-neutral IR and rejects absolute paths and URI schemes with E8001; it
does not resolve host paths or fetch remote resources.

For local relative destinations, `scribium-typst` emits `#image("...")` and
the native source-context backend resolves the path from the source entry's
logical directory inside the project-root mirror. The end-to-end tests cover
`./` and `../` SVG resources, a PNG resource, missing resources, project-root
escapes, and symlink escapes. Alt and title metadata are preserved through
IR but are not currently emitted as PDF accessibility metadata by Typst.

The CommonMark/GFM differential baseline has no image-related known mismatch;
the image work therefore does not remove any existing baseline exception. The
remaining known cases below retain their reviewed policy, canonicalization,
and pinned-Rushdown ownership.

## Evidence and method

The actual run used:

- Rushdown `e5eb4e4446541ea0ed53111c1b37e779283ff57c` (`0.18.0`);
- CommonMark corpus `0.31.2`, `9103e341a973013013bb1a80e13567007c5cef6f`;
- CommonMark cmark `0.31.2`, `eec0eeba6d31189fd828314576494566d539b1e3`;
- GFM corpus/parser `0.29.0.gfm.13`,
  `587a12bb54d95ac37241377e6ddc93ea0e45439b`.

The reference XML and Scribium frontend AST were mapped to the same
structural canonical tree. For each mismatch, the input, section, reference
tree, Scribium tree, first structural diff, frontend node kind, and source
range behavior were inspected. The generated report is the detailed evidence
record for a run; the lists below make the reviewed exception set and its
root-cause grouping durable in the repository.

For the selected metadata group, each affected source was checked against the
original bytes, cmark/cmark-gfm XML value, Rushdown node kind and source-backed
metadata value, Scribium frontend value, canonical tree, and enclosing source
span. Rushdown exposes link destinations/titles and fenced-code info as
source-backed values; the selected defect was the Scribium adapter retaining
that source spelling instead of deriving the semantic metadata value. The
earlier positioned-empty span batch remains separately recorded below; its
four `---`-leading cases are an intentional front-matter policy boundary.

`SCRIBIUM_FRONTEND` means the pinned parser already exposes enough information
for a source-backed adapter fix. `RUSHDOWN_BEHAVIOR` means the pinned parser's
observable construction or public utility behavior is the limiting boundary.
`CANONICAL_MAPPING` means the production AST should not be changed to satisfy
an invalid or policy-incompatible comparison. `INTENTIONAL_POLICY` is an
accepted Scribium output/provenance policy. `UNKNOWN` was not used after the
case review.

## Baseline and selected remediation

| Suite | At PR #73 merge | After positioned-empty batch (live base) | After metadata batch | After exact delimiter-run batch | After hard-break batch | Remaining known mismatches |
|---|---:|---:|---:|---:|---:|---:|
| CommonMark | 583 PASS / 69 KNOWN_MISMATCH | 617 PASS / 35 KNOWN_MISMATCH | 626 PASS / 26 KNOWN_MISMATCH | 630 PASS / 22 KNOWN_MISMATCH | 632 PASS / 20 KNOWN_MISMATCH | 20 |
| GFM | 601 PASS / 69 KNOWN_MISMATCH | 633 PASS / 37 KNOWN_MISMATCH | 642 PASS / 28 KNOWN_MISMATCH | 646 PASS / 24 KNOWN_MISMATCH | 648 PASS / 22 KNOWN_MISMATCH | 22 |

The selected batch is `code-segment-semantics`. Rushdown's code values are
`Lines::Segments`; each `Segment` carries source padding and a forced newline.
The old adapter converted only the raw source slice and inserted `\n` between
segments, then added an artificial space for list nesting. That produced
extra blank lines, dropped final line terminators, and changed indentation.
The fix consumes the public segment byte representation, concatenates the
segments without synthetic separators, and keeps the original AST span.

PR #73 resolved 44 CommonMark and 44 corresponding GFM cases. The current
positioned-empty batch resolves 34 CommonMark and 32 GFM cases. Only those
improvements were removed from the baselines; no unrelated mismatch was
reclassified. The metadata batch resolves 9 CommonMark and 9 GFM cases; the
baseline entries removed for this batch are listed in the fixed group below.
The exact delimiter-run batch resolves four CommonMark and four GFM cases;
only those eight code-span entries were removed from the baselines.
The hard-break trailing-whitespace batch resolves two CommonMark and two GFM
cases; only those four hard-break entries were removed from the baselines.

## Root-cause groups

| Group | Cases (CommonMark / GFM) | Ownership | Priority | Status | Recommended action |
|---|---:|---|---|---|---|
| Code-segment semantics | 44 / 44 | `SCRIBIUM_FRONTEND` | P0 | Fixed | Keep segment padding/newline semantics and retain source spans. |
| Positioned empty/marker-only node spans | 34 / 32 resolved | `SCRIBIUM_FRONTEND` | P1 | RESOLVED | Recover source-backed line/delimiter spans from Rushdown position and metadata; retain UTF-8/CRLF and nested-container checks. |
| Leading `---` front-matter policy boundary | 2 / 2 | `INTENTIONAL_POLICY` | P2 | Accepted | Keep the current front-matter detector; it owns the leading document prologue before Markdown parsing. |
| Empty inline text nodes | 7 / 7 | `SCRIBIUM_FRONTEND` | P2 | Fixed | Do not emit empty `Inline::Text`; structural, nested, UTF-8, LF/CRLF, and provenance regressions are present. |
| Empty or unclosed fenced nodes | 4 / 4 | `SCRIBIUM_FRONTEND` | P2 | Fixed | Recover the original fenced opening/closing line span from the existing Rushdown CodeBlock node; do not synthesize a missing node. |
| Link/code metadata escape and entity normalization, including inline-link entity cases | 9 / 9 | `SCRIBIUM_FRONTEND` | P1 | Fixed | Normalize destination, title, and info values through one source-backed policy without reparsing; preserve original spans. |
| Text entity/reference normalization | 3 / 3 fixed | `SCRIBIUM_FRONTEND` / `CANONICAL_MAPPING` | P1 | Fixed | Adapt numeric zero to U+FFFD at the frontend boundary and preserve entity-produced LF inside one canonical text node. |
| Code-span delimiter matching | 4 / 4 | `SCRIBIUM_FRONTEND` | P1 | Fixed | Match maximal backtick runs with the opener's exact length while keeping the original span. |
| Hard-break trailing whitespace | 2 / 2 | `SCRIBIUM_FRONTEND` | P1 | Fixed | Exclude parser-classified delimiter spaces from semantic text; keep the original text/enclosing source spans. |
| HTML block canonicalization | 1 / 1 | `CANONICAL_MAPPING` | P3 | Accepted | Keep source-backed raw HTML; do not normalize blockquote markers into a production semantic HTML string. |
| Inline link destination/title entity sub-group | 2 / 2 | `SCRIBIUM_FRONTEND` | P1 | Fixed with metadata group | Included because inline destination/title uses the same source-backed adapter policy; no separate parser or span cause was found. |
| Autolink profile and linkify | 3 fixed CM / 3 accepted GFM | `CANONICAL_MAPPING` / `RUSHDOWN_BEHAVIOR` | P2 | Split: fixed / accepted | Select the reference profile in the harness; retain invalid-autolink/linkify behavior as a pinned Rushdown boundary. |
| GFM table alignment and escaped pipe | 2 / 2 fixed | `CANONICAL_MAPPING` / `SCRIBIUM_FRONTEND` | P2 | Fixed | Project body alignment independently from the production AST and consume Rushdown's semantic escaped code-span value. |

Priority reflects ordinary-document frequency and semantic damage, not case
count. Code blocks were selected first because one small adapter correction
fixed 88 suite cases, affects executable/documentation examples, and does not
cross the Rushdown, IR, evaluator, or Typst boundaries. Positioned empty nodes
were selected next because the revalidated AST evidence showed one frontend
span invariant affecting ordinary separators and empty containers, while
remaining within the same source-provenance boundary.

## Case-level root-cause index

The following lists are the reviewed case IDs. CommonMark and GFM IDs use the
same four-digit numbering as the pinned corpus. Within each group, the
reference/Scribium tree difference is represented by the stated structural
signature; the frontend node and span evidence are the corresponding AST
fields in `crates/scribium-markdown/src/parser.rs`.

### Code-segment semantics — fixed

Frontend evidence: `Block::CodeBlock { source, span, .. }`, with `source`
derived from Rushdown `Segment::bytes(source)` and `span` derived from the
original parser ranges. Focused UTF-8, CRLF, nested-list, newline, padding,
and exact source-range coverage is in
`crates/scribium-markdown/tests/code_blocks.rs`.

CommonMark:

`commonmark-0003`, `0005`, `0006`, `0007`, `0008`, `0107`, `0110`, `0111`,
`0112`, `0116`, `0119`, `0120`, `0122`, `0123`, `0124`, `0125`, `0127`,
`0129`, `0131`, `0132`, `0133`, `0134`, `0137`, `0139`, `0142`, `0143`,
`0191`, `0231`, `0254`, `0263`, `0264`, `0270`, `0271`, `0273`, `0274`,
`0278`, `0286`, `0287`, `0288`, `0289`, `0290`, `0318`, `0321`, `0324`.

GFM:

`gfm-0003`, `0005`, `0006`, `0007`, `0008`, `0077`, `0080`, `0081`, `0082`,
`0086`, `0089`, `0090`, `0092`, `0093`, `0094`, `0095`, `0097`, `0099`,
`0101`, `0102`, `0103`, `0104`, `0107`, `0109`, `0112`, `0113`, `0160`,
`0209`, `0232`, `0241`, `0242`, `0248`, `0249`, `0251`, `0252`, `0256`,
`0264`, `0265`, `0266`, `0267`, `0268`, `0298`, `0301`, `0304`.

### Positioned empty/marker-only node spans — RESOLVED in this batch

Structural signature: reference has a thematic break, empty heading,
empty blockquote/list item, or empty-label link/image; the canonical Scribium
tree previously omitted the node because `node_span` had only a parser
position and no child/source segment. Rushdown already exposed the node kind,
position, container relationship, and (for links/images) destination metadata.

The frontend now recovers a block's original line boundary only for a
position-only Rushdown block node, and completes an empty-label link/image
span only from parser-owned destination/title metadata. Every recovered span
is checked against the original source and UTF-8 boundaries. No source is
rewritten, no Markdown is reparsed, and no Rushdown code or revision changed.

Resolved case counts: 34 CommonMark and 32 GFM. The corresponding baseline
entries were removed in this batch.

CommonMark resolved IDs:

`0011`, `0043`, `0047`, `0050`, `0051`, `0052`, `0053`, `0054`, `0057`,
`0058`, `0060`, `0061`, `0077`, `0079`, `0085`, `0088`, `0092`, `0094`,
`0099`, `0100`, `0101`, `0104`, `0105`, `0115`, `0218`, `0234`, `0239`,
`0240`, `0246`, `0280`, `0284`, `0484`, `0487`, `0581`.

GFM resolved IDs:

`0011`, `0013`, `0017`, `0020`, `0021`, `0022`, `0023`, `0024`, `0027`,
`0028`, `0030`, `0031`, `0047`, `0049`, `0055`, `0058`, `0062`, `0064`,
`0069`, `0070`, `0071`, `0074`, `0075`, `0085`, `0187`, `0212`, `0217`,
`0218`, `0224`, `0258`, `0262`, `0589`.

The remaining `---`-leading cases are a separate front-matter policy boundary,
not unresolved span recovery; they are recorded immediately below.

### Leading `---` front-matter policy boundary — deferred

The reference treats `---`-leading examples as thematic breaks or headings,
while Scribium's document entry point recognizes a leading front-matter block
before invoking the Markdown frontend. The Rushdown AST therefore never
contains the reference-visible nodes for these inputs. Changing this would
alter the accepted front-matter policy and is not a safe span recovery.

Ownership: `INTENTIONAL_POLICY`.

CommonMark: `commonmark-0096`, `0098`.

GFM: `gfm-0066`, `0068`.

### Empty inline text nodes — fixed frontend adapter

Structural signature: the canonical tree contained an extra empty `text` child
between real inline nodes. The AST conversion now omits a zero-length
Rushdown text segment instead of materializing
`Inline::Text { content: "", span }`. It does not merge or expand adjacent
source spans.

CommonMark: `commonmark-0138`, `0145`, `0187`, `0432`, `0505`, `0556`, `0587`.

GFM: `gfm-0108`, `0115`, `0156`, `0441`, `0513`, `0564`, `0595`.

All fourteen cases now pass. Independent regressions cover adjacent emphasis,
code spans, links, images, escapes, UTF-8, LF, CRLF, nested containers, and
source-span invariants.

### Empty or unclosed fenced nodes — fixed frontend span recovery

Structural signature: cmark exposes an empty fenced code block while the
pinned Rushdown tree exposes the corresponding `CodeBlockKind::Fenced` node,
position, info string, and empty `Lines::Segments([])`. The previous frontend
discarded that node because `code_block_span()` required a non-empty segment
maximum before considering the fenced delimiter. The adapter now recovers the
original opening/closing line boundary from the existing node position and
the source context, including a blockquote boundary, without creating a new
parser node or rewriting Markdown.

CommonMark: `commonmark-0126`, `0130`, `0144`, `0237`.

GFM: `gfm-0096`, `0100`, `0114`, `0215`.

All eight suite cases now pass. Focused regressions assert empty, closed,
info-bearing, unclosed, and blockquote fences retain original byte spans and
do not absorb sibling source.

### Metadata escape/entity normalization — fixed frontend batch

Ownership is `SCRIBIUM_FRONTEND`. The pinned Rushdown parser exposes the
destination, title, and fenced-code info as source-backed values, so the
source spelling was not a Rushdown behavior limitation. The previous adapter
used partial punctuation unescaping for inline destinations and otherwise
copied raw metadata into the frontend AST. The fix adds one narrow
`normalize_metadata` policy with explicit `LinkDestination`, `LinkTitle`, and
`CodeInfo` modes.

The policy is deliberately metadata-only:

1. One lexical/semantic pass scans the original metadata bytes. Backslash
   escapes use Rushdown's public punctuation classification, while named and
   numeric references are recognized only at their original source position;
   an escaped `&` therefore cannot introduce entity syntax. Non-escapable
   characters, escaped backslashes, and escaped spaces retain their CommonMark
   behavior.
2. Valid named and numeric references are decoded once using Rushdown's public
   utilities. Invalid entity-like text remains unchanged, and a decoded
   character is never rescanned as a new reference.
3. Link destination/title normalization applies to ordinary inline and
   reference links. Auto-link destinations remain on their existing parser
   path. Code info is normalized before `split_whitespace().next()` derives
   `language`, while the complete normalized info string remains in the AST.

Semantic metadata is therefore normalized while provenance remains original:
the existing `ByteSpan` is still validated and indexed against the original
UTF-8 source, including CRLF and nested-container context. Inline link and
code tests assert that `source[span]` still contains the original escaped or
entity spelling. Reference-definition metadata remains source-backed by
Rushdown; its link-use span retains the established parser-produced use-site
range, while the definition bytes remain unchanged in the original document.
No source is rewritten and no Markdown is reparsed.

The independently authored regression suite covers escaped punctuation, named
and numeric references, invalid entity-like sequences, quoted and
parenthesized titles, UTF-8, LF, CRLF, adjacent inline content, reference
definitions, and normalized code-language extraction. The real-document
fixture `14-metadata-normalization.md` passes the full frontend → IR →
evaluator → Typst → PDF pipeline.

Resolved CommonMark IDs: `commonmark-0022`, `0023`, `0024`, `0032`, `0033`,
`0034`, `0202`, `0503`, `0506`.

Resolved GFM IDs: `gfm-0171`, `0318`, `0319`, `0320`, `0328`, `0329`, `0330`,
`0511`, `0514`.

The inline entity cases (`0503`, `0506`, `0511`, `0514`) are included in this
batch because their differential signatures are the same missing entity/escape
normalization in the inline link destination/title adapter path. No distinct
parser ownership or span algorithm was observed. There are no Rushdown-owned
leftovers in this selected metadata group; text entity/reference behavior and
other pinned Rushdown groups remain separate gaps below.

### Text entity/reference normalization — fixed frontend and canonical mapping

Structural signature: escaped `&` was decoded twice, `&#0;` became a NUL
instead of the replacement character, or a numeric line-feed reference was
projected as soft-break nodes by the compatibility harness. Escaped references
are normalized once from the original source; numeric zero is adapted to
U+FFFD at the Scribium frontend semantic boundary; and entity-produced LF
bytes remain inside one canonical text node. Explicit cmark `<softbreak>` nodes
continue to map to canonical soft breaks.

CommonMark: `commonmark-0014`, `0026`, `0039`.

GFM: `gfm-0310`, `0322`, `0335`.

All six cases now pass. The Rushdown utility/segmentation observations remain
covered as evidence, but they are no longer accepted Markdown mismatches.

### Code-span delimiter matching — fixed frontend adapter

Root cause: the Scribium frontend source-span adapter searched for an opener-
length backtick prefix and could accept that prefix from inside a longer
backtick run as the closing delimiter. That changed inline-code content or
emitted a wrong sibling boundary. The fix computes the maximal opener run and
scans source bytes one complete backtick run at a time, accepting only a run
whose length exactly matches the opener. A zero-width Rushdown text boundary
between adjacent code spans is also omitted from the semantic AST; its source
bytes remain represented by the soft-break/span sequence.

The adapter continues to use Rushdown's semantic code-span node/content and
the original source for recovery. It does not rewrite or reparse Markdown,
and the recovered `Inline::Code` span remains an original UTF-8 byte range,
including CRLF and nested-container offsets.

CommonMark: `commonmark-0330`, `0331`, `0334`, `0340`.

GFM: `gfm-0340`, `0341`, `0344`, `0350`.

All eight cases now pass. Remaining differential gaps are 22 CommonMark cases
and 24 GFM cases; the other gap groups below are unchanged.

### Hard-break trailing whitespace — fixed frontend adapter

Root cause: Rushdown marks a hard break with a `HARD_LINE_BREAK` qualifier. For
some LF inputs with three or more trailing spaces, it leaves a prefix of those
spaces in the preceding source-backed `Text` index and puts the qualifier on a
zero-width text node. The Scribium frontend converted that entire index into
semantic `Inline::Text` content, so delimiter spaces remained visible as text.
The CRLF and two-space forms confirmed the same boundary through a text node
whose hard-break span includes the delimiter and newline.

The fix belongs to the Scribium frontend adapter's `convert_inlines` boundary.
When Rushdown has already classified the node as a hard break, the adapter
removes only the contiguous ASCII spaces at the end of the affected semantic
text value and leaves its original `ByteSpan` unchanged. It does not apply a
blanket trim, rewrite source, or reparse Markdown. The hard break remains an
`Inline::HardBreak`, and enclosing/container spans retain the original UTF-8
and CRLF byte provenance.

CommonMark: `commonmark-0226`, `0635`.

GFM: `gfm-0196`, `0655`.

All four cases now pass. The baseline moved from 630 PASS / 22
`KNOWN_MISMATCH` to 632 PASS / 20 for CommonMark and from 646 PASS / 24 to
648 PASS / 22 for GFM. Focused regressions cover exact two-space and three-or-
more-space delimiters, soft breaks, UTF-8, CRLF, nested blockquotes/lists,
multiple breaks, and sibling inline nodes.

### HTML block canonicalization — policy/mapping, no production change

Structural signature: the reference XML reports block HTML content normalized
inside a blockquote (`<div>\nfoo\n`), while Scribium's `Block::RawHtml` keeps
the exact original source (`<div>\n> foo\n`) for provenance and the existing
#71 output policy. The mismatch is not permission to strip source bytes or
expand the supported raw-HTML whitelist.

CommonMark: `commonmark-0174`.

GFM: `gfm-0143`.

### Inline link destination/title entities — merged into metadata batch

These cases are retained as a visible sub-group because they were separately
listed in the earlier audit. Their ownership and fix are the same as the
metadata group above: entity spelling remained in source-backed inline link
metadata, while the reference semantic value was decoded. They are resolved
and are not separate baseline exceptions.

CommonMark: `commonmark-0503`, `0506`.

GFM: `gfm-0511`, `0514`.

### Autolink profile and linkify — fixed profile mapping / Rushdown boundary

CommonMark `0608`, `0611`, and `0612` were linkified by the GFM-enabled
Markdown parser configuration even though the CommonMark oracle treats them as
text. The harness now selects an explicit CommonMark profile. GFM `0610` and
`0614` still expose invalid-autolink edge behavior from the pinned parser, and
GFM `0623` exposes its pinned linkify URL truncation behavior.

CommonMark: `commonmark-0608`, `0611`, `0612`.

GFM: `gfm-0610`, `0614`, `0623`.

### GFM tables — fixed mapping/frontend adapter

`gfm-0199` differed only in whether body-cell alignment was projected from the
table alignment metadata; the compatibility projection now emits the cmark
profile's `none` body alignment without changing production table semantics.
`gfm-0200` now consumes Rushdown's public semantic code-span value (`|`) while
retaining the original source span for `` `\|` ``. Both cases pass without
source scanning or reparsing.

## Historical deferred priority work

At the intermediate 632/20 and 648/22 baseline, the remaining P1/P2 gaps were
separate frontend, canonical-mapping, policy, and Rushdown-boundary questions.
The final review follow-up below resolves the actionable frontend and mapping
items; the leading front-matter boundary remains a separate policy decision.

The following remain explicitly outside this historical remediation batch:
Rushdown upgrade/fork/patch, parser replacement, source preprocessing or
reparsing, raw HTML policy expansion, and `.let`/`.foreach`/`.repeat` semantics.
The later Markdown image slice adds only bounded local resource lowering; it
does not add remote acquisition, caching, or a general resource framework.

## Markdown remediation closure

Base: `4fb666ea11014ae8fe20bfb902f541f69e5a6588`

Audit date: 2026-08-16

Rushdown pin: `e5eb4e4446541ea0ed53111c1b37e779283ff57c`

The closure audit started from the exact `origin/main` base above. The
original 20 CommonMark and 22 GFM mismatches were independently rerun at case
level after each remediation. No baseline exception was removed without a
PASS result, and no expected result was changed.

### Final differential result

| Suite | Total | PASS | Accepted mismatch | UNSUPPORTED | New mismatch |
|---|---:|---:|---:|---:|---:|
| CommonMark | 652 | 649 | 3 | 0 | 0 |
| GFM | 670 | 664 | 6 | 0 | 0 |

The real-document corpus remained `12 successful_pdf / 2 expected_unsupported
/ 0 harness_error`. The final checked-in baseline contains exactly the three
CommonMark and six GFM accepted cases shown below.

### Case-level evidence contract

The following ledger is the case-level audit record, not a group-count
summary. A row with two IDs records two independent corpus cases whose source
and observed structural difference are identical. `T`, `SB`, `C`, `L`, and
`P` abbreviate canonical text, soft break, inline code, link, and paragraph.
`D[...]` is a document canonical tree. Source labels refer to the exact
original Markdown in the source catalog immediately below each table.

#### Fixed cases

| Corpus case ID | Source | Reference canonical result | Scribium canonical result / first difference | Rushdown observable AST/value | Scribium frontend AST/value and source span | Final ownership | Decision |
|---|---|---|---|---|---|---|---|
| `commonmark-0014` / `gfm-0310` | S1 | `D[P[T("... &ouml; ...")]]` | Same canonical tree after fix. Before: escaped `&` was decoded as `ö` (double processing). | One source-backed Text stream includes the bytes `\\&ouml;`; Rushdown does not expose a second entity parse. | Source-backed Text values retain `&ouml;` at original bytes; CM paragraph `0..174`, GFM paragraph `0..174`; child spans remain original source ranges. | `SCRIBIUM_FRONTEND` | Fixed by one-pass source normalization; no decoded output is rescanned. |
| `commonmark-0138` / `gfm-0108` | S9 | `D[P[C(" "),SB,T("aaa")]]` | Same after fix. Before: an extra zero-length `T("")` node appeared between the code span and break. | CodeSpan `0..7`, zero-width Text boundary at the following delimiter/newline transition, then Text `8..11`. | `P 0..11`; `C 0..7`, `SB 7..8`, `T 8..11`; no empty semantic Text. | `SCRIBIUM_FRONTEND` | Fixed by omitting zero-length parser Text segments without merging or expanding spans. |
| `commonmark-0145` / `gfm-0115` | S10 | `D[P[C("aa"),SB,T("foo")]]` | Same after fix. Before: extra `T("")` after the code span. | Rushdown exposes code value `aa` and the zero-width boundary separately. | `P 0..14`; `C 0..10`, `SB 10..11`, `T 11..14`. | `SCRIBIUM_FRONTEND` | Fixed; surrounding original spans are unchanged. |
| `commonmark-0187` / `gfm-0156` | S12 | `D[P[T("Foo"),SB,raw_html("<a href=\"bar\">"),SB,T("baz")]]` | Same after fix. Before: extra `T("")` around inline HTML/break boundaries. | Text and inline RawHtml nodes are source-backed, with a zero-length Text segment at the boundary. | `P 0..22`; `T 0..3`, `SB 3..4`, RawHtml `4..18`, `SB 18..19`, `T 19..22`. | `SCRIBIUM_FRONTEND` | Fixed; nested inline/source spans remain parser-owned. |
| `commonmark-0432` / `gfm-0441` | S14 | Nested strong/emphasis tree with `T("foo ")`, `T("bar ")`, `T("baz")`, `SB`, `T("bim")`, `T(" bop")`. | Same after fix. Before: a zero-length Text child disturbed the nested emphasis tree. | Rushdown supplies nested Strong/Emphasis/Strong nodes plus a zero-width Text boundary. | `P 0..29`; outer Strong `0..27`, nested source spans `2..6`, `6..22`, `11..16`, and `23..27`; `SB 18..19`. | `SCRIBIUM_FRONTEND` | Fixed by dropping only the empty semantic node. |
| `commonmark-0505` / `gfm-0513` | S15 | Three links, each destination `/url`, title `title`, separated by soft breaks. | Same after fix. Before: an empty Text child followed each link/title boundary. | Rushdown exposes each link destination/title as source-backed metadata and the zero-width inline boundary. | `P 0..62`; links `0..20`, `21..41`, `42..62`; breaks `20..21`, `41..42`. | `SCRIBIUM_FRONTEND` | Fixed; metadata and original link spans are preserved. |
| `commonmark-0556` / `gfm-0564` | S16 | `D[P[L("foo"),SB,T("[]")]]` | Same after fix. Before: `T("")` appeared between the reference link and literal brackets. | Rushdown supplies reference-link metadata and the surrounding zero-width Text segments. | `P 0..9`; link `0..4`, `SB 5..7`, bracket Text spans `7..8` and `8..9`. | `SCRIBIUM_FRONTEND` | Fixed without synthesizing or enlarging the link span. |
| `commonmark-0587` / `gfm-0595` | S17 | `D[P[image("foo"),SB,T("[]")]]` | Same after fix. Before: `T("")` appeared after the reference image. | Rushdown supplies reference-image metadata and the zero-width boundary. | `P 0..10`; image `0..5`, `SB 6..8`, bracket Text spans `8..9` and `9..10`. | `SCRIBIUM_FRONTEND` | Fixed; image and enclosing spans remain original. |
| `commonmark-0608` | S18 | `D[P[T("< https://foo.bar >")]]` | Same after fix. Before: the harness used the GFM parser profile and produced a link/text split. | CommonMark parser profile does not linkify this source; GFM profile behavior is separate. | CommonMark frontend `P 0..19`, Text `0..19`. | `CANONICAL_MAPPING` | Fixed by selecting the CommonMark profile in the compatibility frontend. |
| `commonmark-0611` | S19 | `D[P[T("https://example.com")]]` | Same after fix. Before: GFM linkification produced a Link. | CommonMark parser profile exposes plain Text; only GFM enables linkify. | CommonMark frontend `P 0..19`, Text `0..19`. | `CANONICAL_MAPPING` | Fixed by preserving the reference profile boundary. |
| `commonmark-0612` | S20 | `D[P[T("foo@bar.example.com")]]` | Same after fix. Before: GFM email linkification produced a Link. | CommonMark parser profile exposes plain Text; GFM linkify is not enabled. | CommonMark frontend `P 0..19`, Text `0..19`. | `CANONICAL_MAPPING` | Fixed by the explicit CommonMark parser profile. |
| `gfm-0199` | S21 | Header cells center/right; body cells `align=none`. | Same after fix. Before: canonical projection copied header alignment to body cells. | Rushdown AST has header and body cells carrying Center/Right column metadata. | Production table AST remains `TableRow 35..45` with body Center/Right cells; projection maps body alignment to `none` only. | `CANONICAL_MAPPING` | Fixed in the differential projection; production semantics were not changed. |
| `gfm-0200` | S22 | Inline code value `|`; escaped pipe in strong text also `|`. | Same after fix. Before: code value was `\\|` because the adapter used the raw source slice. | Rushdown public `CodeSpan::str(source)` returns `|`; raw source span is `` `\\|` ``. | Table `0..49`; code span `26..30`, raw source `` `\\|` ``, semantic value `|`. | `SCRIBIUM_FRONTEND` | Fixed by using the pinned public code-span value while retaining the original span. |

#### Final case decisions after review follow-up

| Corpus case ID | Source | Reference canonical result | Scribium canonical result / first structural difference | Rushdown observable AST/value | Scribium frontend AST/value and source span | Final ownership | Decision |
|---|---|---|---|---|---|---|---|
| `commonmark-0026` / `gfm-0322` | S2 | `D[P[T("# Ӓ Ϡ �")]]` | Same after fix. Before: semantic value ended in U+0000. | Text index covers `&#0;`; pinned utility returns U+0000, which is the observed substrate value. | CM Text `0..25`; GFM Text `0..20` plus `20..25`; both retain original source spans and now expose U+FFFD semantically. | `SCRIBIUM_FRONTEND` | Fixed by mapping numeric-zero utility output to U+FFFD at the existing one-pass source normalization boundary. |
| `commonmark-0039` / `gfm-0335` | S3 | `D[P[T("foo\\n\\nbar")]]` | Same after fix. Before: `xml_text_nodes()` split a text value's entity-produced LF bytes into soft breaks. | One Rushdown Text segment covers `foo&#10;&#10;bar`; no parser soft-break qualifiers are required. | Paragraph/Text source span `0..16`, value `foo\n\nbar`. | `CANONICAL_MAPPING` | Fixed by preserving non-empty `<text>` values as one canonical text node; explicit `<softbreak>` nodes remain soft breaks. |
| `commonmark-0096` / `gfm-0066` | S4 | Thematic break, H2 `Foo`, H2 `Bar`, paragraph `Baz`. | H2 `Bar`, paragraph `Baz`; leading prologue is consumed before Markdown parsing. | Raw Rushdown would expose thematic break `0`, H2 `Foo` `4`, H2 `Bar` `12`, paragraph `Baz` `20`; document entry policy consumes `0..12`. | Front matter `0..12`; H2 `12..15`; paragraph `20..23`. | `INTENTIONAL_POLICY` | Accepted intentional Markdown-profile deviation; changing it changes document ownership policy. |
| `commonmark-0098` / `gfm-0068` | S5 | Two thematic breaks. | Empty document after leading front-matter consumption. | Raw Rushdown exposes thematic breaks at `0` and `4`; the document entry policy consumes the leading block. | No Markdown frontend nodes; consumed source remains owned by document policy. | `INTENTIONAL_POLICY` | Accepted intentional Markdown-profile deviation. |
| `commonmark-0126` / `gfm-0096` | S6 | Empty fenced code block, `info=""`, value empty. | Same after fix: empty CodeBlock with original span `0..4`. Before: `code_block_span()` returned `None` on empty segments. | Rushdown CodeBlock kind Fenced, `pos=0`, `Lines::Segments([])`, and no body segment; opener node is present. | Frontend CodeBlock span `0..4`, empty body, original opening line retained. | `SCRIBIUM_FRONTEND` | Fixed by recovering the fenced opening-line span from the existing parser node. |
| `commonmark-0130` / `gfm-0100` | S7 | Empty closed fenced code block, value empty. | Same after fix: CodeBlock span `0..8`. Before: empty segments caused node loss. | Fenced CodeBlock `pos=0`, empty segments; source context contains the closing fence. | Frontend CodeBlock span `0..8`, empty body, original opening/closing source retained. | `SCRIBIUM_FRONTEND` | Fixed without synthesizing a parser node or changing the AST ownership. |
| `commonmark-0144` / `gfm-0114` | S8 | Empty fenced code block, `info=";"`, value empty. | Same after fix: CodeBlock span `0..11`, info `;`. Before: empty segments caused node loss. | Fenced CodeBlock `pos=0`, `info=";"`, empty segments. | Frontend CodeBlock span `0..11`, info `;`, empty body, original source retained. | `SCRIBIUM_FRONTEND` | Fixed by source-backed span recovery at the frontend boundary. |
| `commonmark-0174` / `gfm-0143` | S11 | Block raw HTML value `<div>\nfoo\n` inside a blockquote. | Block raw HTML preserves `<div>\n> foo\n`; first difference is canonical quote-marker removal. | Rushdown HtmlBlock kind 6 exposes semantic value without quote markers. | RawHtml value is original source-backed `<div>\n> foo\n`, span `2..14`; enclosing blockquote `0..14`. | `CANONICAL_MAPPING` | Accepted canonical/provenance boundary; production raw HTML is not rewritten to match cmark XML. |
| `commonmark-0237` / `gfm-0215` | S13 | Blockquote contains an empty code block, then paragraph `foo`, then empty code block. | Same after fix: blockquote CodeBlock `2..6`, paragraph `6..9`, root CodeBlock `10..14`. Before: both fenced nodes were omitted. | Rushdown exposes code nodes at positions `2` and `10`, both empty `Lines::Segments([])`, with the parser-owned container boundary visible through siblings. | Frontend retains both CodeBlocks and their original source spans; sibling paragraph is not absorbed. | `SCRIBIUM_FRONTEND` | Fixed with boundary-aware recovery from existing node position and source context. |
| `gfm-0610` | S23 | Partial autolink for `http://foo.bar/baz` followed by text ` bim>`. | Plain Text split at the same parser boundary; no Link node. | Rushdown GFM emits Text `<http://foo.bar/baz` span `0..19` and Text ` bim>` span `19..24`. | Frontend retains those original spans and values; no linkifier is added. | `RUSHDOWN_BEHAVIOR` | Accepted pinned invalid-autolink behavior. |
| `gfm-0614` | S24 | Email autolink for `foo+@bar.example.com`. | Plain Text `<foo+@bar.example.com>`; no Link node. | Rushdown GFM emits one source-backed Text value for the escaped-plus form and no Link. | Frontend Text span `0..23`, original source-backed value. | `RUSHDOWN_BEHAVIOR` | Accepted pinned invalid-autolink behavior; no independent email scanner. |
| `gfm-0623` | S25 | `www.commonmark.org/a.b` is linkified as the complete URL. | Rushdown links only `www.commonmark.org/a`, leaving `.b.` as Text. | Rushdown Link destination is `http://www.commonmark.org/a`; following `.b.` remains Text. | Frontend preserves the parser Link/source span and trailing Text spans. | `RUSHDOWN_BEHAVIOR` | Accepted pinned linkify boundary; no production URL scanner. |

The final accepted case set is CommonMark `0096`, `0098`, `0174` and GFM
`0066`, `0068`, `0143`, `0610`, `0614`, `0623`.

All 42 originally listed mismatches therefore have a concrete final
classification and decision. `UNKNOWN` is not present. The fixed cases are
removed from the checked-in baseline only after the final report showed PASS;
the accepted cases remain visible as `KNOWN_MISMATCH` with the ownership and
reason recorded in the baseline files.

### Exact source catalog

These are the original corpus sources used by the ledger; no source was
rewritten before parsing.

S1 (`commonmark-0014`, `gfm-0310`):

```markdown
\*not emphasized*
\<br/> not a tag
\[not a link](/foo)
\`not code`
1\. not a list
\* not a list
\# not a heading
\[foo]: /url "not a reference"
\&ouml; not a character entity
```

S2 (`commonmark-0026`, `gfm-0322`):

```markdown
&#35; &#1234; &#992; &#0;
```

S3 (`commonmark-0039`, `gfm-0335`):

```markdown
foo&#10;&#10;bar
```

S4 (`commonmark-0096`, `gfm-0066`):

```markdown
---
Foo
---
Bar
---
Baz
```

S5 (`commonmark-0098`, `gfm-0068`):

```markdown
---
---
```

S6 (`commonmark-0126`, `gfm-0096`):

````markdown
```
````

S7 (`commonmark-0130`, `gfm-0100`):

````markdown
```
```
````

S8 (`commonmark-0144`, `gfm-0114`):

`````markdown
````;
````
`````

S9 (`commonmark-0138`, `gfm-0108`):

````markdown
``` ```
aaa
````

S10 (`commonmark-0145`, `gfm-0115`):

````markdown
``` aa ```
foo
````

S11 (`commonmark-0174`, `gfm-0143`):

```markdown
> <div>
> foo

bar
```

S12 (`commonmark-0187`, `gfm-0156`):

```markdown
Foo
<a href="bar">
baz
```

S13 (`commonmark-0237`, `gfm-0215`):

````markdown
> ```
foo
```
````

S14 (`commonmark-0432`, `gfm-0441`):

```markdown
**foo *bar **baz**
bim* bop**
```

S15 (`commonmark-0505`, `gfm-0513`):

```markdown
[link](/url "title")
[link](/url 'title')
[link](/url (title))
```

S16 (`commonmark-0556`, `gfm-0564`):

The exact JSON-style source spelling is
`"[foo]\u0020\n[]\n\n[foo]: /url \"title\"\n"`, where `\u0020` is one
ASCII space before the first newline.

S17 (`commonmark-0587`, `gfm-0595`):

The exact JSON-style source spelling is
`"![foo]\u0020\n[]\n\n[foo]: /url \"title\"\n"`, where `\u0020` is one
ASCII space before the first newline.

S18 (`commonmark-0608`):

```markdown
< https://foo.bar >
```

S19 (`commonmark-0611`):

```markdown
https://example.com
```

S20 (`commonmark-0612`):

```markdown
foo@bar.example.com
```

S21 (`gfm-0199`):

```markdown
| abc | defghi |
:-: | -----------:
bar | baz
```

S22 (`gfm-0200`):

```markdown
| f\|oo  |
| ------ |
| b `\|` az |
| b **\|** im |
```

S23 (`gfm-0610`):

```markdown
<http://foo.bar/baz bim>
```

S24 (`gfm-0614`):

```markdown
<foo\+@bar.example.com>
```

S25 (`gfm-0623`):

```markdown
Visit www.commonmark.org.

Visit www.commonmark.org/a.b.
```

### Closure conditions

- The empty inline text fix is production frontend behavior, not a
  compatibility-only hiding rule. It has independently authored LF, CRLF,
  UTF-8, nested-container, adjacent-inline, link, image, escape, and source
  span regressions in `crates/scribium-markdown/tests/empty_inline_text.rs`.
- Entity/reference normalization is independently evidenced: escaped source
  spelling and numeric-zero replacement are `SCRIBIUM_FRONTEND` one-pass
  adaptations, while entity-produced LF is a `CANONICAL_MAPPING` projection
  fix. None of these remains a known mismatch.
- Empty/unclosed fenced cases are `SCRIBIUM_FRONTEND` fixes: Rushdown already
  exposes the fenced nodes, positions, info values, and empty segment vectors;
  the adapter now retains their original source spans.
- Canonical mapping fixes are confined to the compatibility projection. The
  GFM table body alignment change does not alter the production table AST;
  CommonMark/GFM profile selection does not add a production linkifier.
- No source rewrite, preprocessing, generated-Markdown reparse, reference
  parser in production, parser duplication, or synthetic parser node was
  introduced. Recovered fenced spans are original source ranges. Original
  byte-span provenance remains intact.
- Rushdown remains pinned at
  `e5eb4e4446541ea0ed53111c1b37e779283ff57c`.

No remaining known mismatch is an actionable Scribium-owned Markdown defect.
The remaining cases are limited to verified pinned GFM autolink/linkify
behavior, intentional Scribium document policy, and an accepted
canonical/provenance boundary. Markdown remediation is therefore considered
closed. Future changes to these accepted boundaries require new evidence or
an intentional parser/profile policy change.
