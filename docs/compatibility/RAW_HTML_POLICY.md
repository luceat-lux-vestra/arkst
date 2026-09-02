# Raw HTML Compatibility Policy

Status: **Accepted compatibility policy**  
Review date: **2026-08-19**
Applies to: Markdown (`.md`), Quarkdown-compatible (`.qd`, `.scrib`), and the Typst backend boundary.

This document separates four concepts that must not be conflated:

1. recognizing raw HTML syntax;
2. preserving source-backed raw HTML nodes;
3. assigning backend-neutral document semantics to those nodes; and
4. emitting HTML as an output format.

**Raw HTML recognition is not equivalent to HTML semantic support.**

Arkst uses the pinned Rushdown parser as its Markdown substrate. Rushdown may recognize a construct as raw HTML and expose it as an opaque source-backed node without giving Arkst a DOM, an element/attribute model, CSS semantics, or a portable meaning for non-HTML targets.

## Reference contracts

### CommonMark and GFM

CommonMark permits raw HTML blocks and inline raw HTML according to its HTML-block and raw-HTML grammar. The grammar is not a small semantic tag whitelist: syntactically valid tags, attributes, comments, processing instructions, declarations, and CDATA-like forms can be recognized as raw HTML according to context.

Reference:

- CommonMark 0.31.2 specification: <https://spec.commonmark.org/0.31.2/>
- GitHub Flavored Markdown specification: <https://github.github.com/gfm/>

GFM inherits the raw-HTML model and additionally defines `tagfilter` behavior for HTML rendering. That renderer-side filtering is a security/output concern, not a restriction on whether the Markdown parser can recognize a raw HTML construct.

The Markdown compatibility contract is therefore:

- Rushdown owns CommonMark/GFM raw-HTML recognition.
- Arkst preserves the parser-owned source and spans.
- Arkst does **not** infer arbitrary HTML semantics merely because Rushdown recognized raw HTML.
- A bounded construct may be lowered only when its document meaning is exactly representable by existing backend-neutral IR without parsing HTML attributes or constructing a DOM.

### Quarkdown v2.5.1

Quarkdown deliberately does **not** treat mixed Markdown + raw HTML as part of its normal document language. Its v2.5.1 documentation states that mixed-content support was dropped in favor of target-agnostic document functions. Common HTML workarounds are represented by dedicated Quarkdown functions such as `.collapse` and `.container`.

Quarkdown exposes `.html` as an explicit last-resort escape hatch. The documented contract is target-specific and unsanitized: injected content is intended for an HTML rendering target and other targets may ignore it.

Reference:

- Quarkdown v2.5.1 HTML documentation: <https://github.com/iamgio/quarkdown/blob/v2.5.1/docs/html.qd>

The Quarkdown compatibility contract is therefore:

- ordinary mixed raw HTML in `.qd` / `.scrib` is **not** a compatibility feature;
- Arkst must not promote Markdown raw-HTML recognition into successful Quarkdown semantics;
- `.html` is a separate Quarkdown language feature with an implemented closed `Html` semantic slice; it does not enable arbitrary mixed HTML globally;
- `.html {<em>x</em>}` evaluates a single `String` argument into a closed target-specific semantic representation after the explicit capability check; and
- the HTML output backend remains future work, while the current Typst/PDF path deliberately omits the evaluated target-specific node. See [ADR-0018](../adr/0018-quarkdown-target-specific-native-content.md).

### Quarkdown `.html` is not Markdown raw HTML

The fixed Quarkdown v2.5.1 tag is `107ec3a9482f10d6f90d7580f8409b46a719d18e`.
Its `.html` function takes one regular `content: String` parameter, checks
`Permission.NativeContent`, and returns an `Html(content)` node. The function
is valid both as an isolated block call and inside a paragraph. The upstream
HTML renderer emits the evaluated string verbatim; the tracked plaintext and
GFM renderers return empty output for the node. Other-target ignoring is
therefore an evaluator-node/target-renderer contract, not an instruction to
discard the call during parsing.

The v2.5.1 CLI grants `native-content` through its default permission set, but
the capability is checked before node construction and denial raises a typed
missing-permission error. Arkst's compatibility-default compilation grants
`NativeContent`; the host/API may explicitly deny it through
`compile_with_capabilities`, which emits one source-backed `E3004` before
target-specific node creation. When `.html` is authorized, that builtin creates
only the closed `TargetSpecificContent { target: Html, ... }` value. The
`NativeContent` capability may also gate separately specified closed
native-content features; it does not authorize arbitrary target kinds, generic
native/MIME payloads, CSS, JavaScript, filesystem/network access, Markdown
mixed raw HTML, or Typst/backend-source injection. `.css` shares the upstream
permission but remains outside this slice.

The current Typst/PDF behavior is deliberate silent omission after evaluation
and capability checking: retain the target-specific HTML semantic node until
backend selection, then emit no Typst output and no warning. A future HTML
backend may consume the node and emit the evaluated content verbatim. This is
not a generic native-content or MIME escape hatch.

### Typst

Typst is Arkst's rendering backend, not the owner of Markdown or Quarkdown raw-HTML parsing. Typst markup does not imply that arbitrary raw HTML embedded in an input document has portable Typst semantics. Typst's HTML-specific facilities belong to its HTML output model and are distinct from Arkst accepting raw HTML as source-language semantics.

Reference:

- Typst HTML reference: <https://typst.app/docs/reference/html/>

The Typst boundary must therefore remain downstream of Arkst's semantic decision. The backend must not be used to reinterpret unsupported source-language raw HTML after the evaluator.

## Arkst policy by input mode

| Input mode | Raw HTML recognition | Successful semantic support | Unsupported raw HTML | Explicit HTML escape hatch |
|---|---|---|---|---|
| Markdown (`.md`) | Rushdown/CommonMark+GFM | Bounded exact subset, including complete parser-owned HTML comments as semantic no-ops | Preserve provenance and fail closed with `E8001` at the document-output boundary | Not a Markdown language feature |
| Quarkdown (`.qd`, `.scrib`) | Rushdown may still expose parser nodes because it is the shared substrate | Ordinary mixed raw HTML is **not** Quarkdown-compatible and must not be promoted to document semantics | Must fail closed rather than inherit the Markdown semantic whitelist | Quarkdown `.html`, separately tracked and target-specific |
| Typst backend | Not a source-language parser responsibility | Receives only already accepted backend-neutral IR | Must not recover/reparse rejected source HTML | Typst HTML facilities are backend/output concerns |

## Bounded Markdown semantic subset

The current evidence-backed Markdown-to-Typst/PDF subset is intentionally small:

| Raw HTML form | Arkst semantic projection |
|---|---|
| `<em>...</em>` | `Emphasis` |
| `<strong>...</strong>` | `Strong` |
| `<del>...</del>` | `Strikethrough` |
| `<s>...</s>` | `Strikethrough` |
| `<br>`, `<br/>`, `<br />` | `HardBreak` |
| Complete parser-owned HTML comment | No-op: no IR node and no rendered output |

Matching is case-insensitive but otherwise deliberately strict and attribute-free. Nested combinations are valid only when the entire matched structure remains inside this exact whitelist.

This subset exists because each form has an exact existing backend-neutral IR meaning. It does **not** establish a precedent for progressively implementing an HTML parser.

Examples intentionally outside this semantic subset include:

- `<span class="...">` and other generic/style containers;
- `<a ...>` with HTML-specific attributes and behavior;
- `<code>...</code>` where HTML child semantics differ from a Markdown code span;
- `<u>`, `<mark>`, `<sub>`, `<sup>`, `<kbd>`, and arbitrary custom elements;
- scripts, style, event handlers, CSS, and JavaScript;
- arbitrary block HTML such as `<div>...</div>`;
- declarations, processing instructions, and CDATA as rendered content.

Being outside the subset does not mean Rushdown failed to parse the source. It means Arkst has no justified backend-neutral semantic projection for the current product path.

## HTML comments

Complete parser-recognized HTML comments are part of the bounded Markdown
semantic subset as a semantic no-op. This support is deliberately narrow:

- it applies only in Markdown mode (`.md`, case-insensitively), never in
  `.qd` or `.scrib` mode;
- it applies only to parser-owned `Inline::RawHtml` and `Block::RawHtml`
  nodes;
- the accepted token forms are the CommonMark 0.31.2 short forms `<!-->` and
  `<!--->`, or the ordinary `<!--` ... `-->` form ending at its first `-->`;
- inline comments produce no `IrInline` and no rendered output; and
- a block comment is accepted only when the entire parser-owned raw block is
  one complete comment, with at most the parser-supported leading indentation
  and the terminating line's insignificant ASCII boundary whitespace/line
  ending.

The block boundary is fail-closed. A raw block that starts with a comment but
has visible or additional raw content after the first comment token is not a
comment-only block and emits source-backed `E8001`. For example,
`<!-- note -->VISIBLE`, `<!--> VISIBLE -->`, and
`<!-- one --><!-- two -->` remain unsupported when Rushdown exposes each as
one raw block. An unterminated raw block also remains unsupported.

The comment is discarded only during Markdown AST-to-IR lowering. The
parser-created block structure is retained, so a comment can separate two
lists or a list and an indented code block without causing a reparse or
post-hoc structural reconstruction.

Do not generalize this comment no-op into declaration, processing-instruction,
CDATA, arbitrary HTML, or generic invisible-HTML handling.

## Block HTML

Block HTML remains outside the supported Typst/PDF semantic path. Rushdown may expose a complete block as one opaque source-backed node, but Arkst must not:

- parse the block with a second HTML parser;
- reparse its contents as Markdown;
- strip tags with regular expressions;
- extract text heuristically; or
- silently drop unsupported visible content. A comment-only block is the
  explicit bounded exception documented above; trailing raw content is not.

The current fail-closed `E8001` behavior is the correct boundary until a separately justified portable semantic mapping exists.

## Resolved implementation divergence

The former implementation used the shared parser substrate's AST-to-IR
bounded raw-HTML adapter without an input-mode guard. Exact whitelist forms
could therefore flow through as successful semantics in Quarkdown-mode
documents, even though that is not a Quarkdown v2.5.1 feature.

This divergence is resolved. The compile entry boundary now determines one
internal source mode and passes it to both frontend parsing and AST-to-IR
conversion. The whitelist adapter is enabled only for Markdown; `.qd` and
`.scrib` preserve parser-exposed raw HTML as source-backed nodes and emit
`E8001`. Arbitrary/non-comment block raw HTML remains unsupported; complete
parser-owned Markdown comment-only blocks are the explicit bounded exception.

The implementation preserves these invariants:

- no Rushdown fork, patch, or upgrade;
- no second Markdown parser;
- no HTML DOM/parser introduced merely for this correction;
- no preprocess/source rewrite/reparse path;
- Markdown's existing bounded subset remains supported and covered;
- complete Markdown comments are discarded only as a parser-owned semantic
  no-op;
- `.qd` / `.scrib` ordinary mixed raw HTML fails closed;
- `.html` remains a separate language-feature decision.

## Implementation order

1. **Mode separation:** **Completed.** The Markdown bounded raw-HTML semantic adapter cannot become Quarkdown mixed-HTML support.
2. **Regression evidence:** **Completed.** Core end-to-end tests cover identical `.md`, `.qd`, and `.scrib` sources, the full whitelist, case-insensitive Markdown forms, nested structure, block HTML, and UTF-8/CRLF source spans.
3. **Comment decision:** **Completed.** Complete parser-owned Markdown HTML comments are a bounded semantic no-op; trailing, malformed, and non-comment raw HTML remains fail-closed.
4. **`.html` function:** **closed semantic slice implemented.** The evaluator
   creates only `TargetSpecificContent { target: Html, ... }` after the
   explicit `NativeContent` check (`E3004` on denial); Typst/PDF omit it
   silently. No HTML output backend exists, and `.css`/`.htmloptions` remain
   unsupported.
5. **Do not expand arbitrary HTML semantics** unless a concrete source-language compatibility requirement cannot be represented with existing portable IR/functions.

## Non-goals

This policy does not authorize:

- a general HTML parser or DOM inside Arkst;
- arbitrary HTML-to-Typst translation;
- CSS or JavaScript interpretation;
- unsafe HTML passthrough as a PDF/Typst workaround;
- expanding the Markdown subset merely because Typst or a future HTML backend could emit an equivalent element;
- using Rushdown's HTML renderer as Arkst's Quarkdown evaluator/backend.
- treating Quarkdown `.html` as permission-free or as a Markdown raw-HTML
  parser mode;
- making Typst/PDF emit a warning or visible text for an upstream-ignored
  `.html` node; or
- allowing the target-specific mechanism to authorize `.css`,
  JavaScript, SVG, LaTeX, Typst source, arbitrary MIME, or plugin payloads.

Rushdown's renderer can still be useful as a Markdown-only differential oracle, but production Arkst semantics continue through frontend AST -> backend-neutral IR -> single evaluator -> backend lowering.

## Related compatibility documents

- [Markdown/CommonMark+GFM baseline audit](markdown/README.md)
- [Quarkdown v2.5.1 gap inventory](quarkdown/GAP_INVENTORY.md)
- [Quarkdown compatibility specification](quarkdown/README.md)
- [Typst backend compatibility](typst/README.md)
- [ADR-0003: Typst as the rendering backend](../adr/0003-typst-as-the-rendering-backend.md)
- [ADR-0018: Quarkdown target-specific native content](../adr/0018-quarkdown-target-specific-native-content.md)
