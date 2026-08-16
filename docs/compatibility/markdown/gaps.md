# Markdown differential gap analysis

Audit date: 2026-08-16

This is the root-cause analysis of the differential baseline that was live on
`origin/main` at `f70b77fbebe9427cd57f43b3197fcef9f6c76839`. The analysis was
performed against the checked-in corpus and the pinned reference executables;
it does not infer causes from section totals alone.

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

`SCRIBIUM_FRONTEND` means the pinned parser already exposes enough information
for a source-backed adapter fix. `RUSHDOWN_BEHAVIOR` means the pinned parser's
observable construction or public utility behavior is the limiting boundary.
`CANONICAL_MAPPING` means the production AST should not be changed to satisfy
an invalid or policy-incompatible comparison. `INTENTIONAL_POLICY` is an
accepted Scribium output/provenance policy. `UNKNOWN` was not used after the
case review.

## Baseline and selected remediation

| Suite | Before | After selected batch | Remaining known mismatches |
|---|---:|---:|---:|
| CommonMark | 539 PASS / 113 KNOWN_MISMATCH | 583 PASS / 69 KNOWN_MISMATCH | 69 |
| GFM | 557 PASS / 113 KNOWN_MISMATCH | 601 PASS / 69 KNOWN_MISMATCH | 69 |

The selected batch is `code-segment-semantics`. Rushdown's code values are
`Lines::Segments`; each `Segment` carries source padding and a forced newline.
The old adapter converted only the raw source slice and inserted `\n` between
segments, then added an artificial space for list nesting. That produced
extra blank lines, dropped final line terminators, and changed indentation.
The fix consumes the public segment byte representation, concatenates the
segments without synthetic separators, and keeps the original AST span.

Resolved case counts are 44 CommonMark and 44 corresponding GFM cases. The
full case lists are retained below. No unrelated mismatch was removed from a
baseline.

## Root-cause groups

| Group | Affected cases (CommonMark / GFM) | Ownership | Priority | Status | Recommended action |
|---|---:|---|---|---|---|
| Code-segment semantics | 44 / 44 | `SCRIBIUM_FRONTEND` | P0 | Fixed | Keep segment padding/newline semantics and retain source spans. |
| Positioned empty/marker-only node spans | 36 / 34 | `SCRIBIUM_FRONTEND` | P1 | Deferred | Add narrow span recovery from Rushdown node position and the original line boundary; prove UTF-8/CRLF and nested containers first. |
| Empty inline text nodes | 7 / 7 | `SCRIBIUM_FRONTEND` | P2 | Deferred | Do not emit empty `Inline::Text`; add structural and provenance regressions. |
| Empty or unclosed fenced nodes | 4 / 4 | `RUSHDOWN_BEHAVIOR` | P2 | Deferred | Do not synthesize missing code nodes; record a Rushdown/API boundary and revisit only with reviewed evidence. |
| Link/code metadata escape and entity normalization | 7 / 7 | `SCRIBIUM_FRONTEND` | P1 | Deferred | Normalize destination, title, and info values through a single source-backed policy without reparsing. |
| Text entity/reference normalization | 3 / 3 | Mixed: `SCRIBIUM_FRONTEND` / `RUSHDOWN_BEHAVIOR` | P1 | Deferred | Separate escaped-reference handling from Rushdown numeric/newline utility behavior. |
| Code-span delimiter matching | 4 / 4 | `SCRIBIUM_FRONTEND` | P1 | Deferred | Correct delimiter-run boundary detection while keeping the original span. |
| Hard-break trailing whitespace | 2 / 2 | `SCRIBIUM_FRONTEND` | P1 | Deferred | Exclude delimiter spaces from text content; keep them in the enclosing source span. |
| HTML block canonicalization | 1 / 1 | `CANONICAL_MAPPING` + `INTENTIONAL_POLICY` | P3 | Deferred | Keep source-backed raw HTML; do not normalize blockquote markers into a semantic HTML string. |
| Link destination/title entities | 2 / 2 | `SCRIBIUM_FRONTEND` | P1 | Deferred | Apply the same entity/escape policy to inline link metadata. |
| Autolink profile and linkify | 3 / 3 | `CANONICAL_MAPPING` / `RUSHDOWN_BEHAVIOR` | P2 | Deferred | Give CommonMark and GFM explicit harness profiles; track invalid-autolink/linkify behavior as Rushdown debt. |
| GFM table alignment and escaped pipe | 0 / 2 | `CANONICAL_MAPPING` / `RUSHDOWN_BEHAVIOR` | P2 | Deferred | Correct the canonical alignment projection separately from Rushdown table text behavior. |

Priority reflects ordinary-document frequency and semantic damage, not case
count. Code blocks were selected first because one small adapter correction
fixed 88 suite cases, affects executable/documentation examples, and does not
cross the Rushdown, IR, evaluator, or Typst boundaries.

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

### Positioned empty/marker-only node spans — deferred frontend fix

Structural signature: reference has a thematic break, empty heading,
empty blockquote/list item, or empty-label link/image; the canonical Scribium
tree omits the node because `node_span` has only a parser position and no
child/source segment. This is visible in the frontend AST as an omitted
`Block`/`Inline`, not as a backend or evaluator difference. A future fix must
derive only the original line or delimiter boundary and must reject any span
that is not a valid UTF-8 range.

CommonMark:

`commonmark-0011`, `0043`, `0047`, `0050`, `0051`, `0052`, `0053`, `0054`,
`0057`, `0058`, `0060`, `0061`, `0077`, `0079`, `0085`, `0088`, `0092`,
`0094`, `0096`, `0098`, `0099`, `0100`, `0101`, `0104`, `0105`, `0115`,
`0218`, `0234`, `0239`, `0240`, `0246`, `0280`, `0284`, `0484`, `0487`,
`0581`.

GFM:

`gfm-0011`, `0013`, `0017`, `0020`, `0021`, `0022`, `0023`, `0024`, `0027`,
`0028`, `0030`, `0031`, `0047`, `0049`, `0055`, `0058`, `0062`, `0064`,
`0066`, `0068`, `0069`, `0070`, `0071`, `0074`, `0075`, `0085`, `0187`,
`0212`, `0217`, `0218`, `0224`, `0258`, `0262`, `0589`.

### Empty inline text nodes — deferred frontend fix

Structural signature: the canonical tree contains an extra empty `text` child
between real inline nodes. The AST conversion currently turns a zero-length
Rushdown text segment into `Inline::Text { content: "", span }`.

CommonMark: `commonmark-0138`, `0145`, `0187`, `0432`, `0505`, `0556`, `0587`.

GFM: `gfm-0108`, `0115`, `0156`, `0441`, `0513`, `0564`, `0595`.

### Empty or unclosed fenced nodes — Rushdown boundary

Structural signature: cmark exposes an empty fenced code block while the
pinned Rushdown tree does not produce a source-backed code node that the
adapter can safely convert. The affected cases include unclosed fences and
empty fence bodies inside blockquotes. No source is synthesized and no
Markdown is reparsed.

CommonMark: `commonmark-0126`, `0130`, `0144`, `0237`.

GFM: `gfm-0096`, `0100`, `0114`, `0215`.

### Metadata escape/entity normalization — deferred frontend fix

Structural signature: link destination/title or fenced-code info retains the
source escape/entity spelling instead of the reference semantic value. The
frontend nodes are `Inline::Link` or `Block::CodeBlock`; their enclosing spans
remain source-backed, but the metadata values need one consistent adapter
normalization policy.

CommonMark: `commonmark-0022`, `0023`, `0024`, `0032`, `0033`, `0034`, `0202`.

GFM: `gfm-0171`, `0318`, `0319`, `0320`, `0328`, `0329`, `0330`.

### Text entity/reference normalization — mixed ownership

Structural signature: escaped `&` is decoded twice, `&#0;` becomes a NUL
instead of the replacement character, or a numeric line-feed reference stays
inside one text node instead of becoming two soft breaks. The first case is
frontend normalization; the latter two follow the pinned Rushdown utility and
AST segmentation behavior and cannot be fixed by changing the backend.

CommonMark: `commonmark-0014`, `0026`, `0039`.

GFM: `gfm-0310`, `0322`, `0335`.

### Code-span delimiter matching — deferred frontend fix

Structural signature: the adapter's source-span scan accepts the first byte
of a longer backtick run as the closing delimiter, which changes inline-code
content or emits a wrong sibling boundary. Frontend node: `Inline::Code` with
an original source span; the span algorithm needs an exact delimiter-run
boundary.

CommonMark: `commonmark-0330`, `0331`, `0334`, `0340`.

GFM: `gfm-0340`, `0341`, `0344`, `0350`.

### Hard-break trailing whitespace — deferred frontend fix

Structural signature: the text child retains delimiter spaces that cmark
assigns to the hard-break syntax. The enclosing paragraph and hard-break spans
remain source-backed.

CommonMark: `commonmark-0226`, `0635`.

GFM: `gfm-0196`, `0655`.

### HTML block canonicalization — policy/mapping, no production change

Structural signature: the reference XML reports block HTML content normalized
inside a blockquote (`<div>\nfoo\n`), while Scribium's `Block::RawHtml` keeps
the exact original source (`<div>\n> foo\n`) for provenance and the existing
#71 output policy. The mismatch is not permission to strip source bytes or
expand the supported raw-HTML whitelist.

CommonMark: `commonmark-0174`.

GFM: `gfm-0143`.

### Link destination/title entities — deferred frontend fix

Structural signature: entity spelling remains in inline link metadata while
the reference value is decoded. This is separate from the reference-definition
and fence-info cases above because it is the inline destination/title path.

CommonMark: `commonmark-0503`, `0506`.

GFM: `gfm-0511`, `0514`.

### Autolink profile and linkify — mapping/Rushdown boundary

CommonMark `0611` and `0612` are linkified by the GFM-enabled Markdown parser
configuration even though the CommonMark oracle treats them as text; this is a
profile mismatch in the harness/adapter contract, not a reason to remove GFM
support from production. CommonMark `0608` and GFM `0610`/`0614` expose
invalid-autolink edge behavior from the pinned parser. GFM `0623` exposes the
pinned linkify URL truncation behavior.

CommonMark: `commonmark-0608`, `0611`, `0612`.

GFM: `gfm-0610`, `0614`, `0623`.

### GFM tables — mapping/Rushdown split

`gfm-0199` differs only in whether body-cell alignment is projected from the
table alignment metadata; this is a canonical mapping question. `gfm-0200`
retains an escaped pipe in the Rushdown text value, which is a pinned GFM
parser behavior and has no safe frontend workaround that avoids reparsing.

## Deferred priority work

The next high-value candidates are the positioned empty-node span recovery and
metadata normalization. The former affects ordinary thematic breaks and
container boundaries; the latter affects links and displayed code languages.
They require separate source-span regressions and should not be combined with
the selected code-segment change until their provenance contracts are proven.

The following remain explicitly outside this remediation batch: Rushdown
upgrade/fork/patch, parser replacement, source preprocessing or reparsing,
IR/evaluator changes, resource resolution, raw HTML policy expansion, and
`.let`/`.foreach`/`.repeat` semantics.
