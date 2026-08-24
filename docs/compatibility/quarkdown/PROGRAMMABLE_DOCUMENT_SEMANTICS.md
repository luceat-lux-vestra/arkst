# Programmable Document Semantics

This document records the architecture-gate investigation for Quarkdown
v2.5.1 and Scribium issue #61. It is a compatibility record, not a feature
implementation plan that changes the current supported surface.

- Tracked upstream: Quarkdown v2.5.1
- Resolved upstream tag: [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e)
- Scribium comparison baseline: `3829d847f1b45871b2315d729a1f432cf390e6da`
- Decision: [ADR-0020](../../adr/0020-programmable-document-semantic-model.md)

The value-origin, invocation-binding, conversion, and state-commit audit is
maintained separately in
[`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md). This document remains the
authority for programmable scope/evaluation architecture; it does not repeat
the #149 conversion matrix.

## Historical architecture gate and current status

The original architecture gate recorded in ADR-0020 selected the one-IR,
typed-value, evaluator-owned representation and deliberately deferred deciding
which concrete component consumers would be implemented. That historical
decision remains unchanged. The sections below additionally record current
implementation evidence at `3829d847f1b45871b2315d729a1f432cf390e6da`; they
are status updates, not a rewrite of the ADR or a claim of complete Quarkdown
programmable-document compatibility.

## Findings

### Function and output pipeline

The v2.5.1 path is structurally:

`FunctionCallWalkerParser` → `FunctionCallRefiner` → `FunctionCallNode` →
`FunctionCallNodeExpander` → `FunctionCallExpansionStage`.

`FunctionCallArgument.value` is lazy, but the regular binder evaluates normal
inline arguments in source order. The refiner represents the body as a lazy
`DynamicValue`; the binder evaluates it only when the body parameter is bound.
Conversion occurs before function invocation. The output visitor then maps
typed results to document nodes: `NodeValue` to its node, collections to
structured content, scalar values to block/inline output, and `VoidValue` to no
output. A dynamic result can therefore be a node, collection, iterable, or raw
Markdown value rather than only a string.

Relevant public source evidence:

- [`FunctionCallRefiner.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt)
- [`RegularArgumentsBinder.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt)
- [`FunctionCallNodeExpander.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/FunctionCallNodeExpander.kt)
- [`NodeOutputValueVisitor.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/output/node/NodeOutputValueVisitor.kt)
- [`ValueFactory.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt)

### Lambda and custom-function scope

`Lambda` retains a definition context, forks it at invocation, propagates the
calling context's libraries when available, and installs its parameters last.
Explicit parameters therefore mask outer implicit `.1`, `.2`, … values.
Headerless lambda parameters use those implicit names. `.function` stores a
callable definition rather than a text macro; its invocation has a definition
environment, a calling environment, a signature, a dynamic result, and a
separate output conversion step. `.var` can update an existing variable owner,
while a new declaration is local to its selected scope.

Source evidence: [`Lambda.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt), [`ScopeContext.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ScopeContext.kt), [`MutableContext.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/MutableContext.kt), and [`Flow.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt).

### Inline iteration callable bodies (2026-08-23)

The pinned v2.5.1 flow contract accepts the following bounded forms:

- `.foreach {iterable} {parameter: body}`;
- `.foreach {iterable} {body}` with implicit `.1`, `.2`, and later slots when
  the lambda header is absent; and
- `.repeat {count} {body}`, which delegates to the same ordered one-based
  iteration path as `.foreach`.

The upstream refiner initially represents the inline body as an ordinary raw
positional argument. The regular binder converts it to a `Lambda` only because
the target parameter is callable. An indented body remains a distinct lazy
block-body argument; neither form is reparsed or evaluated as an ordinary
eager content argument.

Scribium keeps that distinction at the frontend boundary with a narrow
source-backed `Value::InlineBody` → `IrValue::InlineBody` carrier. Source-
defined functions receive its ordinary structured content; only a resolved
native `.foreach`/`.repeat` target adapts its parameter metadata and body to
the existing `IrCallable`. Block and inline iteration bodies then converge on
the existing callable capture, parameter binding, `invoke_callable()`, ordered
result, rollback, and resource-budget paths.
The bounded slice does not generalize this to arbitrary inline component or
callback bodies.

### Component categories

Quarkdown does not expose one universal language type called “component”. The
observable categories are native scalar values, native node values, structured
Markdown/content values, mutating void functions, custom dynamic results, and
layout node values. Scribium now provides the typed
`IrValue::Component` carrier and materializes it into a typed `IrNode` only at
the document output boundary. `IrValue::Content` remains structured content;
`IrCallable` remains the language/evaluator carrier; surviving
`IrNode::FunctionCall` or `IrNode::ChainedFunctionCall` values may remain
unresolved structural compatibility forms rather than being mistaken for
completed semantic nodes.

The component carrier is backend-neutral. It may contain layout kind,
validated alignment/gap properties, semantic children, and source spans, but
never Typst names or Typst source.

### Document state

`Document.kt` uses read/write dual APIs. No argument reads the current
`DocumentInfo` field; an argument validates, mutates `MutableContext.documentInfo`,
and returns void. Scope contexts delegate ordinary document state to their
parent, while a subdocument has an isolated copy. Scribium represents the first
slice as evaluator-owned working document state shared by ordinary callable
child scopes, plus a final immutable `IrDocument.metadata.document_state`
snapshot. Typst lowering consumes the snapshot and does not replay calls.

The Document State Foundation slice is implemented. `.docname`,
`.docdescription`, `.doctype`, the bounded `.docauthor`/`.docauthors` slice,
and the bounded `.dockeywords` slice support their read/write APIs; the
bounded `.theme` slice is a setter-only document-state mutation. Writes
return no document output, `.docname` rejects blank values before mutation,
`.doctype` validates a closed `plain`/`paged`/`slides`/`docs` enum before
mutation, `.docauthor` appends minimal ordered author records while reading
the first author name, `.docauthors` appends validated authors with ordered
string info and returns an ordinary typed dictionary view, and `.dockeywords`
returns an ordered iterable while replacing the complete keyword list only
after all candidate elements validate. `.theme` accepts nullable scalar
`color` and `layout` parameters, binds both positionally or by name, accepts
`.none` as a null component, and lowercases supplied strings. Its regular
block body falls back to the final `layout` parameter upstream; Scribium
defers this path because the current frontend/IR exposes parsed nodes rather
than lossless raw body text. A `.theme` body is rejected before evaluation so
nested calls cannot execute or mutate document state. It replaces the complete
theme on every successful call, including an explicit empty setter. The
snapshot is serializable plain data with explicit defaults for older
serialized IR; the document type defaults to `plain`, missing authors and
keywords default to empty collections, missing author info defaults to
empty, and missing theme defaults to no committed theme. The
remaining document metadata fields remain deferred.

The bounded `.doclang` slice adds the same shared state path for a locale
record. Its argumentless getter returns the stored `localized_name` or an
empty String; its positional `locale` or named `locale:` setter performs
case-insensitive English-name lookup before canonical-tag lookup, then
replaces the locale and returns no document value. `.none` follows the pinned
nullable `String? = null` path and therefore uses the getter rather than
clearing state. The IR carries only plain `{ tag, localized_name }` data.

Because upstream delegates lookup to `java.util.Locale`, Scribium does not
use a JVM, OS, environment, or native locale database. The upstream `.doclang`
input is general case-insensitive English full-name or IETF BCP 47 tag lookup,
not an API restricted to built-in localization locales. The evaluator uses a
small checked-in deterministic table containing the ten locales named by the
public localization documentation (`zh`, `en`, `fr`, `de`, `it`, `ja`, `pl`,
`pt`, `ru`, `uk`) plus the pinned `LocaleTest` lookup examples `ko`, `en-US`,
and `fr-CA`. This is explicitly partial and bounded: valid BCP 47/name
identifiers outside the table fail rather than being accepted as compatible,
and remain a documented compatibility gap. Block-body fallback is also
deferred and rejected before nested body evaluation because the current
frontend/IR cannot provide upstream's lossless raw `DynamicValue` text.
Binding, candidate evaluation, String conversion, resolution, validation, and
one state commit are atomic, including rollback of nested state mutation after
a failed candidate. Ordinary callable child scopes share locale state, and
source-defined `.doclang` shadows the native builtin. Localization tables,
`.localize`, hyphenation, Typst/HTML language output, and locale-aware
rendering remain deferred.

### Domain conversion adapters

The v2.5.1 `ValueFactory` domain targets are implemented as production
branches of the evaluator-owned `InvocationValue`/`ValueOrigin` conversion
dispatcher. Scalar targets remain separate. `Size` stores a backend-neutral
number plus one of exactly `px`, `pt`, `cm`, `mm`, `in`, `em`, or `%`; omitted
units mean `px`, unit spelling is case-insensitive, and the decimal grammar is
not trimmed or widened to scientific notation. `Color` decodes the reviewed
Hex, RGB, RGBA, HSV/HSL, and CSS3 named-color families into numeric RGBA
channels. Hex accepts exactly `#RGB`, `#RGBA`, `#RRGGBB`, and `#RRGGBBAA`;
the alpha syntax is validated for compatibility, then discarded by the
v2.5.1 adaptation path so the semantic alpha remains `1.0`. Closed enums use
explicit allowed-value tables and Quarkdown's
lowercase/underscore-removed declaration naming rule; input underscores are
not removed. Typed domain values use identity conversion, dynamic textual
values use only their bounded parser, and static `StringValue` results do not
gain domain meaning.

These adapters now feed the reviewed Stacked and Container layout consumers
and the bounded `.whitespace` inline consumer. The live `.doctype` consumer
remains a separate closed enum domain; Stacked main-axis, Stacked cross-axis,
and Container alignment enums retain their own typed identity, and layout gaps
reuse the existing `Size` adapter. Color/style consumers remain deferred.

### Layout classification

The v2.5.1 `Layout.kt` contract is recorded here for future compatibility work.
The reviewed block-body consumer slice is implemented. `.row`, `.column`, and
`.grid` construct one typed Stacked semantic component after argument binding,
conversion, and lazy body evaluation. `.center`, `.align`, and the bounded
`.container` sizing slice construct the typed Container component after their
argument validation and lazy Markdown block-body evaluation. `.landscape`
constructs a typed Landscape component after validating its no-argument,
required Markdown block-body contract. Typst lowering remains backend-owned and
does not add Typst constructs to the core IR.

| Function | v2.5.1 observable inputs | Validation/result |
|---|---|---|
| `row` | row layout, main-axis alignment, cross-axis alignment, optional gap, Markdown body | stacked semantic node |
| `column` | column layout, main-axis alignment, cross-axis alignment, optional gap, Markdown body | stacked semantic node |
| `grid` | positive integer columns, both alignments, general gap, vertical gap, horizontal gap, Markdown body | stacked semantic node; non-positive columns fail |
| `center` | full-width centered Container with a required Markdown block body and no non-body arguments | bounded Container semantic node |
| `align` | full-width Container, one required `alignment` positional or named argument, and required Markdown block body | bounded Container semantic node; `start`/`center`/`end` are closed and origin-aware |
| `container` | optional `width: Size`, `height: Size`, `fullwidth: Boolean`, and optional Markdown block body | bounded Container semantic node; empty/body-only, structured children, and lazy validation are supported |
| `landscape` | no non-body arguments and one required Markdown block body | typed Landscape semantic node; 90° counter-clockwise content transformation |

The remaining future surface includes broader style/layout properties and
component families; the direct `.container` consumer remains limited to this
sizing subset and does not become a generic layout engine.

### Stacked layout consumer slice (2026-08-21)

Implemented in this slice:

- typed `IrValue::Component(IrComponent)` value preservation;
- a closed backend-neutral `IrComponent::Stacked` model for row, column, and
  positive-column grid layouts;
- typed layout, main-axis/cross-axis alignment, `IrSize` gap, and `Vec<IrNode>`
  child fields;
- component and child source provenance, including `value_source_span()`;
- deterministic serde roundtrip and zero-column rejection;
- `.row`, `.column`, and `.grid` block calls with positional/named binding,
  exact defaults, origin-aware alignment/Size/Int conversion, duplicate and
  unknown-argument diagnostics, and grid gap precedence;
- lazy source-backed Markdown block-body evaluation into `Vec<IrNode>`;
- typed `IrValue::Component` to `IrNode::Component` materialization, including
  callable composition and nested components; and
- pure Typst lowering for Stacked row/column/grid nodes, with deterministic
  size conversion, alignment structure, source maps, and real Typst/PDF
  integration coverage.

### Container component consumer slices (2026-08-21)

`.center` is implemented as a bounded Container component consumer:

- `IrComponent::Container` carries optional `width`/`height`, `full_width`,
  optional logical alignment, structured `Vec<IrNode>` children, and the
  producing call span;
- `.center` accepts exactly one required Markdown block body, evaluates it
  lazily through the existing callable/body path, and rejects positional,
  named, inline, and lambda-body forms;
- typed component values remain intact through callable results, nested
  composition, Stacked interoperation, and failure-atomic materialization; and
- Typst lowering emits a full-width block with logical `center` alignment and
  retains child source-map provenance.

`.align` is implemented in the same Container semantic family:

- `IrEnumValue::ContainerAlignment` and `ClosedEnumTarget::ContainerAlignment`
  preserve a closed `start`/`center`/`end` domain with typed identity,
  case-insensitive dynamic names, and no static String coercion;
- exactly one `alignment` argument is accepted positionally or by name, with
  duplicate, unknown, missing, and excess bindings rejected;
- alignment binding/conversion completes before the required lazy Markdown
  block body is evaluated, and failures publish no partial Container; and
- nested, Stacked, callable, and source-provenance composition remains typed;
  native inline `.align` is block-only and fails closed.

The direct `.container` consumer is implemented only for the bounded sizing
slice:

- an empty or optional Markdown block body constructs an unaligned Container;
- positional and named `width`, `height`, and `fullwidth` arguments use the
  existing invocation-origin infrastructure and typed `IrSize`/Boolean
  conversion; duplicate, unknown, and deferred-known parameters fail with a
  source-backed diagnostic;
- argument binding, conversion, and validation complete before the body is
  evaluated, so invalid sizing never executes a failing body or publishes a
  partial component; and
- Typst lowering emits `#block` sizing in deterministic `width`, `height`
  order, with explicit width taking precedence over `fullwidth`, and emits no
  `#align` wrapper when alignment is `None`.

Deferred from these slices:

- `StyleOptions` and `.float`;
- `.fullspan`, general String → Markdown body conversion, and inline Container
  insertion;
- direct `.container` style parameters including `float`, `fullspan`,
  `classname`, alignment/text alignment, colors, borders, margin/padding,
  radius, and font/text-style properties.

Deferred from this slice:

- general DynamicValue String → Markdown body conversion;
- inline Stacked insertion (Stacked is block-only);
- `.box`, `.clip`, `.figure`, and other layout families; and
- pixel-identical reproduction of upstream HTML/CSS rendering.

The component remains backend-neutral in value context and may pass through
variables and callable results before the typed block output boundary.

### Landscape component consumer slice (2026-08-21)

`.landscape` is implemented as a bounded typed component consumer:

- the native call accepts no positional or named arguments and requires one
  Markdown block body; validation completes before lazy body evaluation;
- lambda bodies and inline native calls fail closed with source-backed
  diagnostics, while source-defined `landscape` functions retain precedence;
- `IrComponent::Landscape` preserves ordered `Vec<IrNode>` children, nested
  `Container`/`Stacked`/`Align`/Landscape composition, callable value-flow, and
  call/child source provenance; and
- Typst lowering emits `#rotate(-90deg, reflow: true)[...]`, preserving body
  order and layout footprint in flow. It does not mutate page orientation or
  page size and does not emit `page(flipped: true)`.

The upstream feature is documented as experimental. `.float` remains deferred
because Quarkdown's subsequent-content wrapping is not equivalent to Typst's
`place(float: true)`, and `.fullspan` remains deferred because its in-flow
multi-column full-column span is not equivalent to a parent-scoped floating
placement. Neither is inferred from this component.

### Whitespace inline consumer slice (2026-08-21)

The v2.5.1 upstream node category is the inline `Whitespace` node, not a
layout component. Its public signature is `.whitespace width?: Size?
height?: Size?`; the two parameters accept positional or named binding in
`width`, then `height` order, with no body.

Scribium implements this bounded contract as a backend-neutral
`IrInline::Whitespace` semantic value:

- with neither dimension supplied, it represents one non-breaking whitespace
  character, equivalent to Quarkdown's `&nbsp;` behavior;
- with one or both dimensions supplied, it represents an empty fixed-size
  rectangle; a missing axis is normalized to semantic size zero;
- conversion reuses the existing `InvocationValue`/`ValueOrigin`-aware `Size`
  adapter, including typed `IrSize` identity, dynamic textual Size parsing,
  and static String non-coercion;
- evaluated inline calls preserve surrounding content order and the call's
  source span, while standalone calls use the existing scalar/inline output
  materialization boundary; and
- Typst lowering emits the NBSP character for the dimensionless form and a
  deterministic empty `#box` with explicit zero dimensions otherwise, using
  the existing `IrSize` formatter.

This slice does not add a generalized inline component/value hierarchy,
universal `NodeValue`, or String-to-Markdown conversion. Those broader
composition boundaries remain deferred.

### `.br` inline hard-break consumer slice (2026-08-21)

The v2.5.1 public function is argumentless: `.br` accepts no positional or
named arguments, no block body, and no lambda body, and returns the inline
`LineBreak` node wrapped as a value. The pinned v2.5.1 `Text.kt` and the
current upstream `main` implementation are unchanged for this function.

Scribium resolves native `.br` calls only after the existing source-defined
callable lookup, preserving user-function precedence. After signature
validation, it returns one `IrValue::Content` carrier containing a paragraph
with the existing backend-neutral `IrInline::HardBreak { span }`, where the
span is the original `.br` call span. The normal inline/block materializers
place that hard break without a second evaluator or a new IR/value variant.

Validation is fail-closed and precedes evaluation of invalid argument or body
contents. Invalid positional, named, multiple-argument, block-body, and
lambda-body forms publish no hard-break output. The existing Typst
`IrInline::HardBreak` lowering is the only backend path involved.

The v2.5.1 `NodeUtils.toPlainText()` projection omits `LineBreak`; Scribium's
existing `IrInline::HardBreak` plaintext projection is therefore reused
without a `.br`-specific `.plaintext` branch. This bounded slice does not
implement or imply generalized inline components, `.text`, `.codespan`,
`.clip`, `.float`, or `.fullspan`.

## Compatibility matrix

| Feature | Quarkdown v2.5.1 behavior | Scribium current behavior | Architecture decision | Implementation status | Deferred work |
|---|---|---|---|---|---|
| Custom functions | `.function` defines a callable with a signature, lexical definition context, caller propagation, dynamic result, and separate output conversion | Bounded user functions with immutable definition capture, a lookup-only caller overlay, typed values, semantic-owner writeback for existing caller-visible variables, and isolated invocation child maps | Keep `IrCallable` as evaluator value; compose definition and caller layers without replacing capture or treating parameters as owners | Partial, caller overlay and scoped writeback implemented | Broader stdlib/component call surface and mutable context/library parity |
| Lambda explicit parameters | Parameters bind by signature and shadow outer implicit parameters | Explicit lambda parameters and shadowing are supported in the bounded evaluator slice | Parameter installation is last in child scope | Bounded implemented | Broader upstream scope fixtures |
| Lambda implicit parameters | Headerless parameters are `.1`, `.2`, …; explicit parameters mask them | Headerless invocation scope is nearest-first; a missing local implicit slot can resolve a propagated caller slot, while explicit scope remains a hard mask | Preserve implicit scope as evaluator state, never backend state | Bounded implemented | Broader upstream scope fixtures |
| Lexical/calling scope | Definition context is retained; mutable calling context can be propagated; nested lookup is nearest-first; `.var` arguments are evaluated using normal call/invocation lookup semantics before mutation resolves the owner | Definition capture remains immutable; caller-visible variables/functions and the visible caller lambda scope are overlaid only for one invocation; local writes remain isolated; the evaluated `.var` value is then written back to the resolved semantic owner | Adopt hybrid target model with explicit definition, caller-overlay, and invocation layers; evaluation scope and mutation owner remain distinct | Bounded compatibility implemented | Definition-capture mutation and broader mutable-scope parity |
| Lazy body evaluation | Body is a lazy `DynamicValue`; unreachable conditional/body paths do not execute it | Block bodies are evaluated on the existing callable path after binding; conversion failures precede body execution | Preserve eager inline vs lazy body timing | Bounded implemented | More differential lazy-body fixtures |
| Inline iteration body | Regular binding first resolves the callee parameter; an inline likely-body becomes a `Lambda` only for a callable target, while an ordinary dynamic parameter receives content | `Value::InlineBody`/`IrValue::InlineBody` preserves structured content and callable metadata until source-defined/native resolution; native iteration adapts it into the shared `IrCallable` path | Keep contextual inline bodies target-sensitive and source-backed | Bounded `.foreach`/`.repeat` support implemented, including direct/chain source-defined shadowing | Generalized inline component/callback bodies |
| DynamicValue result | Dynamic results may be scalar, node, iterable, collection, or Markdown/content and are converted at output boundary | Typed `IrValue`, `IrValue::Content`, and closed `IrValue::Component` preserve semantic values; completed Stacked values materialize only at the typed block boundary | Keep component values backend-neutral until lossless output materialization | Bounded Stacked consumer implemented | General DynamicValue conversion and broader component families |
| Component/node result | `NodeValue` carries a semantic AST node; output visitors place it block/inline | `.row`/`.column`/`.grid` and bounded `.center` produce typed `IrValue::Component` values and materialize as `IrNode::Component`; nested children and spans remain structured | Distinguish evaluated component values from unresolved calls and materialize only at a lossless typed boundary | Reviewed Stacked slice and bounded `.center` implemented | Inline component insertion and other component families |
| Document-state mutation | Document APIs read with no argument, mutate shared mutable document info with an argument, and return void; `.theme` is a setter-only exception | Evaluator-owned state shared by ordinary callable child scopes and caller-overlay invocations; final `IrMetadata.document_state` snapshot; `.docname`, `.docdescription`, `.doctype`, bounded `.docauthor`/`.docauthors`, bounded `.dockeywords`, bounded `.doclang`, and bounded `.theme` are implemented with bounded conversion | Evaluator-owned shared working state plus final `IrDocument` snapshot | Document State Foundation and caller sharing implemented; `.docname`, `.docdescription`, `.doctype`, `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, and `.theme` implemented at bounded evidenced boundaries. `.doclang` uses deterministic checked-in locale records and preserves nullable `.none` getter behavior; upstream block-body fallback remains a documented deferred gap | Valid BCP 47/name locale records outside the checked-in table, `.localize`, localization tables, theme resolution/validation/defaults, hyphenation, rendering/layout metadata, front-matter merge policy, and remaining document fields |
| `.captionposition` document-state | Nullable `default`, `figures`, `tables`, and `@Name("code") codeBlocks`; partial state merged into current document layout state; regular body fallback targets final `codeBlocks` upstream | Typed evaluator-owned caption state uses post-evaluation shared state as the successful merge base, commits one immutable `IrCaptionPositionInfo`, and rejects bodies before evaluation because raw `DynamicValue` text is unavailable | Preserve nested successful mutations and explicit-versus-inherited overrides; keep body fallback separate from parsed `CallBody` semantics | Implemented at bounded evaluator/IR boundary; upstream body-to-final-`codeBlocks` fallback is an explicit compatibility gap | Raw body representation and caption rendering/placement |
| `row` | Stacked row with alignments, optional gap, and Markdown body | `.row` binds `alignment`, `cross`, and `gap`, evaluates a required block body lazily, and creates a typed Row component | Backend-neutral component value, then semantic node; Typst names remain in lowering | Implemented for reviewed block-body Stacked slice | General String → Markdown body conversion and broader layout families |
| `column` | Stacked column with alignments, optional gap, and Markdown body | `.column` binds the same typed arguments with column gap semantics and creates a typed Column component | Same backend-neutral component boundary | Implemented for reviewed block-body Stacked slice | General String → Markdown body conversion and broader layout families |
| `grid` | Positive integer columns, alignments, general/vertical/horizontal gaps, Markdown body; non-positive columns fail | `.grid` validates a dedicated integral positive `columns` boundary and applies `vgap ?: gap` / `hgap ?: gap` before constructing a typed Grid component | Validate before component construction and keep the result typed | Implemented for reviewed block-body Stacked slice | General String → Markdown body conversion and broader layout families |
| `.whitespace` | Inline `Whitespace` node with optional `width: Size?` and `height: Size?`; positional order is width then height; no dimensions emit NBSP, while a supplied dimension creates a fixed-size empty rectangle | `IrInline::Whitespace` preserves the inline semantic, source span, surrounding order, typed Size conversion, zero for an omitted axis, and the existing standalone output bridge | Keep Whitespace inline and backend-neutral; reuse `InvocationValue`/`ValueOrigin` and materialize only through existing output boundaries | Implemented for the bounded argument, inline, block-output, provenance, and Typst slice | General inline node/value composition and other inline layout features |
| `.br` | Argumentless inline `LineBreak` producer; no positional/named arguments or block/lambda body; `toPlainText()` omits the line break | Existing evaluator/native-call path materializes exactly one `IrInline::HardBreak` with the call span, preserves inline order and source-defined `br` precedence, validates before body evaluation, and reuses existing plaintext/Typst paths | Reuse `IrInline::HardBreak`; do not add `IrInline::Br`, `IrValue::LineBreak`, or a generic inline component | Implemented for the bounded v2.5.1 slice | `.text`, `.codespan`, `.clip`, `.float`, `.fullspan`, and generalized inline component/value conversion |
| `Size` conversion | `ValueFactory.size` parses typed/numeric/unit values with domain rules | Backend-neutral `IrSize` conversion is consumed by row/column/grid gaps and `.whitespace` for the exact seven-unit decimal grammar, with typed identity and origin-gated text | Domain-specific origin-aware conversion adapter | Implemented for Stacked gaps and bounded `.whitespace` | Other Size consumers |
| `Color` conversion | `ValueFactory.color` accepts typed colors or domain text decoding | Backend-neutral `IrColor` conversion implements the ordered Hex/RGB/RGBA/HSV-HSL/Named decoder families and numeric channels | Domain-specific origin-aware conversion adapter | Implemented | Color consumers, style, and component semantics |
| Enum conversion | Closed enum values are matched through the allowed value set and public names | Explicit closed enum adapter preserves `DocumentType`, Stacked main-axis, and Stacked cross-axis domains with case-insensitive public names and no static String coercion | Closed domain adapter; no reflective generic coercion | Implemented for `.doctype` and Stacked layout | Other closed enum consumers |
| Markdown conversion | Markdown/content conversion parses a raw dynamic value in the frontend context; node output is semantic | Already-parsed `IrValue::Content` is supported; String → Markdown reparsing is not | Content remains structured; raw String conversion requires a future explicit frontend/provenance contract | Partial | Content conversion boundary |
| Component conversion | Dynamic result can become a node/layout value through typed output visitors | Closed typed `IrValue::Component`/`IrComponent` is constructed by the reviewed source calls and materialized as one typed block node | Backend-neutral component value, origin-gated construction, typed output materialization | Implemented for reviewed block-body Stacked slice and bounded `.center` | General String → Markdown body conversion and inline insertion |

## Normative evaluator rules

The observable order is: callee resolution; inline argument evaluation in
source order; named/positional binding; origin-aware conversion; definition
capture restoration; caller-visible lookup overlay; invocation child-scope
creation; parameter installation; lazy body decision; call execution; typed
result construction; document-state commit; result materialization.

The bounded scope implementation keeps these layers explicit:

    definition capture
            ↓ fallback
    caller-visible variables/functions and lambda scope
            ↓ fallback
    invocation child scope
            ↓ highest precedence
    invocation parameters

The caller overlay is a lookup-only invocation layer. It does not replace or
mutate IrCallableCapture, or copy project/source/diagnostic runtime state.
Invocation parameters and copied capture/overlay bindings are lookup bindings,
not variable owners. A successful assignment to an existing caller-visible
semantic owner is explicitly written back after callable completion; newly
declared variables remain invocation-local. Document state remains the
explicit shared runtime handle established by the Document State Foundation.

For this slice, definition capture, caller lookup overlay, invocation
parameter precedence, implicit-parameter precedence, and scoped owner writeback
are implemented. Definition-capture mutation and broader mutable scope/library
parity remain partial/deferred.

`InvocationValue` and `ValueOrigin` from PR #105 remain mandatory. Dynamic
textual values may use bounded target adapters; static `StringValue` does not
gain arbitrary typed meaning. In particular, dynamic `"2"` and `"2.0"` can
continue to satisfy the existing integer rule, dynamic `"1.5"` fails, and
static `StringValue("2")` fails.

Failures do not publish partial semantic output or partial callable/iteration
variable mutation. Local child bindings remain local, and successful owner
writeback occurs only after the callable completes successfully. Document-state
commits retain their established shared-runtime semantics. Unverified upstream
cases remain compatibility gaps rather than invented transactions.

Provenance stays source-backed: argument/conversion failures point to the
argument expression, body failures to the original body, component properties
to their property source, and nested children retain their own spans. Typst
source-map entries are generated during lowering from those original spans.

## R13 / #61 closure audit

At the current main snapshot
`3829d847f1b45871b2315d729a1f432cf390e6da`, the targeted #61
programmable-document foundation has no remaining concrete M2 semantic blocker
requiring evaluator behavior changes. R13 reconciles the record and adds no new
evaluator semantics.

The currently evidenced foundations are:

- function/callable representation and the `CallOutcome` result boundary;
- invocation evaluation order, lexical definition capture, caller lookup
  overlay, and invocation-parameter precedence;
- scoped mutation/writeback with parameter-as-lookup-binding and new-local
  isolation;
- block and native contextual inline iteration through one callable evaluator
  path, including explicit/implicit parameters and Pair destructuring;
- typed component/value materialization, including the bounded Row, Column,
  positive-column Grid, Container, and Landscape slices;
- deterministic failure behavior, source-backed diagnostics, and R10 resource
  limits;
- one backend-neutral IR model, including structurally preserved unresolved
  calls; and
- pure Typst lowering plus real Typst/PDF integration for implemented
  component slices.

| #61 acceptance criterion | Concrete current evidence |
|---|---|
| Target behavior is classified against public Quarkdown behavior | `README.md` Feature Matrix, `GAP_INVENTORY.md`, pinned v2.5.1 sources, and `crates/scribium-test-support/src/lib.rs::ConformanceCase::verify` compatibility-level checks |
| Evaluator contracts are documented | `ADR-0020`, this document's Normative evaluator rules, `crates/scribium-engine/src/evaluator.rs`, and `CallOutcome::{Value, NoValue, Failed, Unresolved}` |
| Scoping and evaluation order are tested | `crates/scribium-core/src/lib.rs::compile_captured_callable_uses_definition_fallback_and_caller_shadowing`, `compile_invocation_parameters_shadow_caller_and_definition_bindings`, `compile_callable_var_updates_owner_without_overwriting_shadowing_parameter`, and `crates/scribium-engine/src/evaluator.rs::foreach_reassignment_updates_existing_caller_variable_but_new_locals_stay_local` |
| Nested and inline/block contexts are covered | `compile_inline_foreach_and_repeat_use_the_shared_callable_path`, `compile_inline_foreach_preserves_pair_destructuring`, `compile_source_defined_foreach_and_repeat_shadow_native_direct_and_chain`, and `crates/scribium-markdown/src/parser.rs::iteration_inline_body_preserves_contextual_metadata_without_eager_lambda_coercion` |
| Diagnostics reference original Scribium spans | `compile_inline_foreach_failure_is_atomic_and_source_backed`, `failed_callable_reassignment_is_atomic_and_keeps_the_inner_span`, stacked invalid-argument/body tests in `crates/scribium-core/tests/quarkdown_stacked_layout.rs`, and source-span assertions in the frontend/AST-to-IR tests |
| AST → evaluator → IR behavior is deterministic | `crates/scribium-core/src/lib.rs::source_ids_are_independent_of_builder_insertion_order`, `compile_result_is_independent_of_source_insertion_order`, `crates/scribium-engine/src/evaluator.rs::evaluation_is_immutable_and_deterministic`, and `crates/scribium-test-support/src/lib.rs::tests::quarkdown_conformance_corpus_obeys_declared_levels` |
| Compatibility docs distinguish supported semantics from parsed-only syntax | README compatibility levels and Feature Matrix, `GAP_INVENTORY.md` classification rows, and semantic IR/Typst/diagnostic golden requirements in `fixtures/quarkdown-conformance/README.md` |
| Implemented component slices cross the backend boundary | `crates/scribium-core/tests/quarkdown_stacked_layout.rs`, `crates/scribium-ir/src/lib.rs::stacked_components_roundtrip_deterministically_for_row_column_and_grid`, and `crates/scribium-typst-subprocess/tests/backend_integration.rs::integration_stacked_layouts_lower_to_valid_typst_and_pdf` |

Closing #61 records completion of the targeted evaluator and
programmable-document foundation. It does **not** claim complete Quarkdown
v2.5.1 compatibility.

Major deferred families remain separately tracked: the complete public
stdlib/component/style/layout surface; generalized DynamicValue conversion and
arbitrary inline component/callback bodies; `.extend`, `.box`, `.clip`,
`.figure`, `.float`, `.fullspan`, and broader layout families; remaining
document-context and data-loading functions; host/process/environment
semantics; and unrelated M3+ work. Raw HTML policy and behavior in issue #58
are separate and are not part of this closure audit.

## Intentionally deferred

This slice does not implement String → Markdown or String → component
conversion, inline Stacked/Container insertion, the deferred direct
`.container` style parameters, `StyleOptions`, `.float`, `.fullspan`, a generic layout
engine, a parallel evaluator, filesystem/network features, or an evaluator rewrite. No architecture
prototype or feature snapshot was
necessary: the existing Rust types and exhaustive backend consumer are enough
to select the representation at the document level.

The remaining direct `.container` style parameters and related layout families
remain deferred. Valid BCP 47/name `.doclang` records outside Scribium's
checked-in bounded table, `.localize`,
localization tables, theme resolution/validation/defaults, rendering policy,
hyphenation, front-matter/document-state merging, generalized DynamicValue
conversion, and other document metadata remain deferred. `.docauthor`,
`.docauthors`, `.dockeywords`, `.doclang`, `.theme`, and `.captionposition` are
implemented only at their bounded evaluator/IR document-state boundaries.

### Bounded `.captionposition` state (Issue #145)

The pinned v2.5.1 implementation defines `CaptionPositionInfo` with a
non-nullable `default` initialized to `BOTTOM` and nullable `figures`, `tables`,
and `codeBlocks` overrides. `Document.captionPosition` accepts the named
source alias `code` for `codeBlocks`, constructs a partial state, and calls the
generated `CaptionPositionInfo.merge(currentPosition)` extension. Amber's
v2.2.0 merge generator keeps non-nullable receiver properties and uses the
receiver's nullable properties first, falling back to `other` only when they
are null. Consequently, default updates preserve existing per-kind overrides,
while omitted or nullable `.none` overrides preserve their previous values.

The regular binder also maps an indented body to the final bindable parameter.
For `.captionposition`, that means upstream's block body falls back to
`codeBlocks` as raw `DynamicValue` text. Scribium records this as an explicit
compatibility gap: the current frontend/IR boundary exposes parsed `CallBody`
nodes rather than lossless raw body text, so the setter rejects a body before
body evaluation, matching the existing `.theme` boundary.

Scribium maps that contract to the existing evaluator-owned `DocumentState` and
immutable `IrDocument.metadata.document_state` snapshot. The representation is
closed and backend-neutral: `IrCaptionPosition` has only `Top` and `Bottom`,
and `IrCaptionPositionInfo` retains the explicit-versus-inherited distinction
for each override. The evaluator validates the raw regular-argument binding
shape before evaluating candidates, evaluates and converts all bound values,
uses the post-evaluation shared state as the merge base, restores the complete
pre-invocation state on any failure, and commits once. This preserves successful
nested caption-state mutations from argument evaluation. Callable scopes share
the document state, and source-defined direct/chained `captionposition` calls
shadow the native setter under the established policy.

This slice returns no document content and stops at the immutable IR snapshot.
It does not implement caption rendering, Typst/HTML placement, `.figure`,
`.table`, `.code`, or a generalized layout metadata framework. Independent
observable evidence is retained in
`fixtures/quarkdown-conformance/cases/captionposition-document-state/` and the
focused core/evaluator/IR tests.

The `.br` slice does not promote the broader text/layout family: `.text`,
`.codespan`, `.clip`, `.float`, and `.fullspan` remain separate deferred work.
