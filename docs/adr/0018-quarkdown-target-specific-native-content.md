# ADR-0018: Quarkdown Target-Specific Native Content

- **Status:** Accepted
- **Date:** 2026-08-19
- **Owners:** Scribium maintainers
- **Related ADRs:** 0003, 0015, 0016, 0017
- **Upstream baseline:** Quarkdown `v2.5.1`
- **Resolved upstream tag commit:** `107ec3a9482f10d6f90d7580f8409b46a719d18e`

## Context

Quarkdown v2.5.1 exposes `.html` as an explicit, last-resort native-content
function. It is not the same language feature as Markdown raw HTML. The
function evaluates its content to a `String`, constructs an `Html` node, and
lets the selected target decide whether that node is rendered. The upstream
contract is therefore target-specific document semantics, not source-level
HTML recognition and not an instruction to translate HTML into Typst.

Scribium currently has a Rushdown-backed Markdown frontend, a backend-neutral
IR, an evaluator, and a Typst/PDF backend. It has no HTML renderer and no
compile/evaluation permission context. The current parser can preserve the
syntax as a Quarkdown directive call, but the current evaluator has no `.html`
builtin and the current IR has no target-specific native-content variant. The
existing fallback is evidence of the gap, not the compatibility contract.

## Decision drivers

- Preserve Quarkdown v2.5.1's document-observable target-specific behavior.
- Keep evaluated content, placement, and source provenance until backend
  selection.
- Keep Markdown raw HTML recognition and semantics completely separate.
- Make Typst/PDF behavior deterministic and intentionally unsupported without
  fabricating visible text or Typst source.
- Make permission checks explicit before an unsafe native-content value can
  reach a renderer.
- Keep the representation backend-neutral, WASM-capable, and closed to
  arbitrary backend code, MIME payloads, or extension-defined native escapes.
- Avoid adding an IR/evaluator/backend implementation before this contract is
  reviewed and the missing capability boundary exists.

## Upstream v2.5.1 evidence

All links below resolve to the fixed tag commit
[`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e).

### Function signature and argument evaluation

[`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L59-L90)
defines exactly one user-facing parameter:

```kotlin
fun html(
    @Injected context: Context,
    @LikelyBody content: String,
): NodeValue
```

The injected `context` is not a document argument. `content` is the only
regular argument, and its public parameter name permits the ordinary named
form `content:{...}` in addition to the documented positional/body forms.
There are no other `.html` named options. The function returns
`Html(content).wrappedAsValue()`.

`@LikelyBody` is documentation metadata only; it has no runtime effect
([`QuarkdocAnnotations.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/annotation/QuarkdocAnnotations.kt#L3-L11)).
The ordinary argument binder reserves an explicit body parameter when one is
declared, otherwise binds a body to the last parameter, and converts a
non-String value for a `String` parameter with its `toString()` value
([`RegularArgumentsBinder.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt#L47-L76),
[`RegularArgumentsBinder.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt#L98-L150)).

Inline arguments are trimmed at the outer argument boundary and represented as
`RawInlineExpression`; the expression evaluator can compose literal text with
nested function-call expressions
([`FunctionCallRefiner.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt#L41-L67),
[`RawInlineExpression.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/expression/RawInlineExpression.kt#L7-L25),
[`ValueFactory.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt#L538-L597)).
Thus `.html` consumes the evaluated `String` value; it must not be implemented
by slicing the original source or by reparsing synthesized Markdown. The
spelling `{variable}` is not established by these sources as a separate
interpolation language. Without a nested Quarkdown expression, those braces
are part of the String; nested function expressions are evaluated according
to the normal expression rules.

An indented block body is captured as a `DynamicValue` containing plain text;
nested function calls in that body are not executed by default
([`FunctionCallRefiner.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt#L58-L67)).
For `.html`, the body then binds to the sole `String` parameter. Interior body
whitespace is therefore content, subject to the upstream function-call body
indentation normalization, while inline outer argument whitespace is trimmed.

### Block and inline placement

The function-call lexer marks a call as block-level only when no non-whitespace
content follows the parsed call on its header line; otherwise the paragraph
path retains it as an inline call
([`FunctionCallPatterns.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/lexer/patterns/FunctionCallPatterns.kt#L20-L72)).
The documented examples are therefore both valid:

```text
.html {<em>world</em>}

**Hello** .html {<em>world</em>}!
```

The AST class is in the `block` package, but
[`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/base/block/Html.kt#L1-L14)
implements the generic `Node` interface rather than a block-only interface.
The function-call expander chooses a block or inline output mapper from the
call placement, but the `NodeValue` visitors return an arbitrary node unchanged
when it is neither Markdown paragraph content nor a scalar
([`FunctionCallNodeExpander.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/FunctionCallNodeExpander.kt#L37-L47),
[`BlockNodeOutputValueVisitor.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/output/node/BlockNodeOutputValueVisitor.kt#L31-L48),
[`InlineNodeOutputValueVisitor.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/output/node/InlineNodeOutputValueVisitor.kt#L37-L53)).
`Paragraph.text` is an ordered `List<Node>`, so the same `Html` node can occur
between surrounding inline nodes
([`InlineContent.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/InlineContent.kt#L1-L7),
[`Paragraph.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/base/block/Paragraph.kt#L12-L29)).

Conclusion: upstream does not require separate upstream Html classes for
inline and block placement. Scribium must nevertheless preserve placement in
its own IR carriers so `text + target-specific node + text` ordering cannot be
lost. A block-only native-content node is incorrect.

### Rendering and unsupported targets

The HTML renderer returns `node.content` directly, with no escaping or
sanitization
([`BaseHtmlNodeRenderer.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-html/src/main/kotlin/com/quarkdown/rendering/html/node/BaseHtmlNodeRenderer.kt#L230-L240)).
The tracked plaintext and GFM Markdown renderer visitors return an empty
string for the same node
([`PlainTextNodeRenderer.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-plaintext/src/main/kotlin/com/quarkdown/rendering/plaintext/node/PlainTextNodeRenderer.kt#L80-L84),
[`GfmNodeRenderer.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-markdown/src/main/kotlin/com/quarkdown/rendering/markdown/node/GfmNodeRenderer.kt#L165-L169)).
The generic `NodeVisitor` only supplies dispatch for `Html`; target visitors
own the output decision
([`NodeVisitor.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/visitor/node/NodeVisitor.kt#L95-L105)).
The public HTML documentation explicitly says that other rendering targets
ignore the content
([`docs/html.qd`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/docs/html.qd#L43-L58)).

The exact v2.5.1 behavior is therefore:

| Target | Behavior |
|---|---|
| HTML | Emit the evaluated String verbatim/unescaped. |
| Non-HTML renderer | Visit the node as a valid semantic node and produce no output. |
| Permission denied | Fail before node creation; do not silently ignore the function. |

### Permission behavior

`html()` calls `context.requirePermission(Permission.NativeContent, ...)`
before constructing the node and reports the message `Cannot inject native HTML
content` when the capability is missing. `NativeContent` is named
`native-content`, and v2.5.1's `Permission.DEFAULT_SET` includes it
([`Permission.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/Permission.kt#L37-L61),
[`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L80-L90)).
The CLI computes permissions as `DEFAULT_SET + --allow - --deny`, so ordinary
CLI execution grants `native-content` by default and `--deny native-content`
removes it
([`ExecuteCommand.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-cli/src/main/kotlin/com/quarkdown/cli/exec/ExecuteCommand.kt#L255-L275)).
Denial raises `MissingPermissionException`, whose diagnostic includes the
missing permission and `--allow native-content`; its v2.5.1 exit code is 72
([`MissingPermissionException.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/MissingPermissionException.kt#L7-L28),
[`ExitCodes.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ExitCodes.kt#L38-L45)).

`.css` delegates to `.html` and shares `NativeContent`, but CSS is outside
this slice and is not authorized by this ADR
([`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L92-L121)).

## Considered representations

| Representation | Decision | Reason |
|---|---|---|
| Markdown `RawHtml` | Rejected | It would make source-level `<em>x</em>` recognition authorize `.html` semantics and would conflate two different languages. |
| `IrInline::Text` or escaped text | Rejected | HTML would become visible literal text or be escaped instead of target-native content. |
| `IrNode::Paragraph` or block-only node | Rejected | It loses inline ordering and misrepresents the upstream generic `Node` placement. |
| `RawTypst` or a Typst lowering shortcut | Rejected | It violates backend-neutral ownership and cannot represent HTML target selection. |
| Evaluator-side unconditional discard | Rejected | It loses semantic information before backend selection and cannot support a future HTML backend. |
| Arbitrary MIME/backend payload or plugin escape hatch | Rejected | It creates an unbounded native-code surface and is outside the accepted architecture. |
| Closed target-specific content payload | Chosen direction | It preserves target, evaluated content, placement carrier, and provenance while allowing each backend to make an explicit decision. |

## Chosen representation direction

When implementation begins, introduce a backend-neutral, typed target-specific
content concept equivalent to:

```text
TargetSpecificContent {
    target: NativeTarget::Html,
    content: String,
    span: SourceSpan,
}
```

This is a representation direction, not a request to add the type in this
PR. The target discriminator must be a closed Scribium-owned enum. For this
slice it has exactly one supported variant: `Html`. Adding another variant
requires its own public-evidence and architecture review; the type must not
accept arbitrary MIME strings, backend source, CSS, JavaScript, SVG, LaTeX,
Typst, or extension-defined native payloads.

The payload is a semantic document fragment: it remains in the evaluated
document until backend selection, but it is renderable only by a backend that
explicitly supports its target. The IR must preserve its `SourceSpan` and its
placement carrier. With the current separate `IrNode`/`IrInline` shape, the
implementation is expected to use a shared closed payload plus explicit block
and inline carriers, or an equivalent single content-sequence mechanism. It
must not force all target-specific content through a block-only node.

The engine/evaluator owns construction. It evaluates the `.html` argument to
the `String` contract, checks the narrowly scoped native-content capability,
then constructs the semantic payload. The frontend owns Quarkdown call syntax
and provenance, but does not source-slice HTML or recognize ordinary mixed raw
HTML. The renderer does not reinterpret a directive call or reparse Markdown.

## Inline vs block requirement

One semantic target-specific payload is sufficient; two source-language
features are not. The IR carrier must support both of these ordered contexts:

```text
block sequence:  [TargetSpecificContent(Html)]
inline sequence: [Text("Hello"), TargetSpecificContent(Html), Text("!")]
```

The inline carrier is mandatory because upstream's `Html` node can be inserted
into `Paragraph.text` between ordinary nodes. Any implementation that only
stores a block node, wraps inline content in a paragraph, or flattens it into
text is incompatible with the documented example.

## Evaluation ownership

`.html` evaluation belongs in the engine/evaluator's typed callable path. The
eventual builtin must receive a value with the upstream String conversion
semantics, retain the resulting string as opaque native HTML, and avoid
Markdown parsing, DOM construction, source rewriting, or synthesized-source
reparsing. An indented body remains a String body; inline nested expressions
follow the ordinary evaluator before the final String boundary.

The evaluator must preserve a target-specific node for a permitted call even
when the selected backend later ignores it. Permission checking must occur
before the node is published, not in an HTML renderer and not as an
unconditional evaluator no-op.

## Permission ownership

Scribium currently has no public compile/evaluation capability set. That is an
implementation prerequisite for `.html`; the current pipeline must not grant
an implicit unsafe escape hatch. The next implementation slice must either
introduce a narrowly scoped evaluator capability equivalent to
`NativeContent`, or record an approved equivalent in the host boundary before
the builtin is enabled. The host/CLI owns grant and denial policy; the
platform-neutral evaluator receives an explicit capability context.

The initial capability must be closed and scoped to the native-content contract
under review. It must not automatically authorize `.css`, `.htmloptions`,
Markdown raw HTML, JavaScript, CSS interpretation, filesystem access, network
access, or arbitrary backend injection. A denied capability produces a
structured Scribium diagnostic before semantic node creation. No broad
permission framework is introduced by this ADR.

## Backend lowering ownership

`scribium-html`, when an HTML backend exists, consumes only
`TargetSpecificContent { target: Html, ... }` and emits the stored content
verbatim, subject to the explicit capability and host trust boundary. It does
not need a generic HTML parser or DOM for `.html`; the payload is already the
evaluated opaque String required by upstream.

`scribium-typst` owns the Typst decision for the same backend-neutral node. It
must not translate HTML to Typst, escape it as visible text, pass it to Typst
raw/code syntax, or reparse it. The node is intentionally omitted from Typst
output.

## Typst/PDF unsupported-target behavior

Scribium chooses the v2.5.1-compatible silent-ignore behavior:

```text
evaluate .html
    -> check NativeContent capability
    -> retain TargetSpecificContent(Html, evaluated String, span)
    -> Typst/PDF lowering emits no output and no warning
```

This is Option A from the compatibility review. A warning would add an
observable Scribium divergence where v2.5.1 explicitly permits other targets
to ignore the content; a hard error would be a larger divergence. Permission
denial remains an error because upstream checks permission in the function
before target rendering. The semantic node should survive until backend
selection even though the Typst lowering result is empty, so a future HTML
backend can consume the same evaluated document without reevaluating source.

## Future HTML backend behavior

A future HTML backend is the only backend authorized by this ADR to render the
`Html` target. It emits the evaluated content verbatim/unescaped in the exact
block or inline position represented by the carrier. It must expose the
security/trust boundary of unsanitized native HTML and must not silently
sanitize it while claiming v2.5.1 compatibility. Sanitization, if offered as
a separate product mode, requires a separate compatibility and security
decision.

No HTML backend, HTML parser, DOM, CSS engine, JavaScript engine, or output
pipeline is created by this ADR.

## Markdown raw HTML isolation

Markdown raw HTML and Quarkdown `.html` remain separate at every layer:

| Source | Owner and meaning |
|---|---|
| `<em>x</em>` or `<!-- comment -->` in `.md` | Rushdown-owned Markdown raw HTML; only the bounded Markdown adapter policy applies. |
| `.html {<em>x</em>}` in `.qd`/`.scrib` | Quarkdown function call; evaluated String becomes target-specific native HTML only after the future capability gate. |
| `<em>x</em>` in `.qd`/`.scrib` | Ordinary mixed raw HTML; remains source-backed and fails closed with `E8001`. |

Adding the `.html` builtin must never broaden the Quarkdown frontend's raw
HTML recognition or enable ordinary mixed raw HTML. Rushdown remains pinned at
`e5eb4e4446541ea0ed53111c1b37e779283ff57c`; this ADR does not patch, fork, or
upgrade it.

## WASM implications

The target-specific payload, closed target enum, source span, evaluator
capability context, and Typst omission are platform-neutral and must remain
WASM-capable. They require no filesystem, process, network, native path, or
host-global state. A future HTML renderer may be selected by a host, but the
core representation must not execute HTML, JavaScript, CSS, or arbitrary
native code. Host capability grant and renderer/output policy remain outside
the WASM-safe compiler core.

## Security implications

Quarkdown explicitly documents `.html` output as unsanitized and potentially
vulnerable. Scribium must not silently convert that fact into a trusted
default. The evaluator capability is the authorization point, and the future
HTML backend is the execution/output sink. Provenance and diagnostics must
remain attached to the original call so hosts can audit the source that
requested native content. Typst/PDF omission is not a sanitization mechanism
and must not be advertised as HTML security filtering.

## Rejected alternatives

- Treating `.html` as Markdown `RawHtml` would allow ordinary mixed HTML and
  collapse frontend ownership boundaries.
- Storing the string as visible text, a paragraph, or Typst raw source would
  change the output semantics and/or introduce backend injection.
- Dropping the call during evaluation would make backend selection impossible
  and discard provenance.
- Adding a generic native-content/MIME/plugin system would exceed the one
  closed HTML variant supported by this evidence.
- Building an HTML parser or DOM would implement a different feature from the
  upstream function's opaque String contract.
- Implementing `.css`, `.htmloptions`, or related functions would expand the
  requested slice and is explicitly deferred.

## Consequences

This ADR makes `.html` a documented compatibility gap with a mechanical next
implementation shape but deliberately leaves implementation pending. The next
slice must add the capability context and the closed target-specific IR
carriers together with evaluator, HTML-backend, Typst omission, diagnostic,
provenance, WASM, and security tests. It must not broaden Markdown raw HTML.

Until then, Scribium must not claim `.html` compatibility. Current evidence
fixtures document parser/evaluator fallback shapes and existing `E3010`/
`E8001` behavior only.

## Explicit non-goals

This ADR does not:

- implement `.html` evaluation or rendering;
- add a target-specific IR variant in this PR;
- add an HTML backend, parser, DOM, CSS, JavaScript, SVG, LaTeX, or Typst
  native-code path;
- change Rushdown or its pin;
- enable ordinary mixed raw HTML in `.qd`/`.scrib`;
- implement `.css`, `.htmloptions`, or a general permission framework; or
- define a generic plugin, MIME, or backend escape-hatch system.
