# Programmable Document Semantics

This document records the architecture-gate investigation for Quarkdown
v2.5.1 and Scribium issue #61. It is a compatibility record, not a feature
implementation plan that changes the current supported surface.

- Tracked upstream: Quarkdown v2.5.1
- Resolved upstream tag: [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e)
- Scribium comparison baseline: `40cfdc6bff5c4beb452370b5d27cb15172bb8830`
- Decision: [ADR-0020](../../adr/0020-programmable-document-semantic-model.md)

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

### Component categories

Quarkdown does not expose one universal language type called “component”. The
observable categories are native scalar values, native node values, structured
Markdown/content values, mutating void functions, custom dynamic results, and
layout node values. Scribium now provides the typed
`IrValue::Component` carrier and materializes it into a typed `IrNode` only at
the document output boundary. `IrValue::Content` remains structured content;
`IrCallable` remains the language/evaluator carrier; a surviving
`IrNode::FunctionCall` remains unresolved structural syntax.

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
`.docdescription`, and `.doctype` support the read/write dual API; writes
return no document output, `.docname` rejects blank values before mutation,
and `.doctype` validates a closed `plain`/`paged`/`slides`/`docs` enum before
mutation. The snapshot is serializable plain data with explicit defaults for
older serialized IR; the document type defaults to `plain`.

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

These adapters intentionally have no layout, style, or source component
consumer yet. The live `.doctype` consumer uses the closed enum adapter;
Size/Color consumers and layout alignment enums remain deferred. The typed
component value foundation is separate from those future argument binders.

### Layout classification

The v2.5.1 `Layout.kt` contract is recorded here for future compatibility work.
No layout builtin or lowering is implemented by this architecture gate.

| Function | v2.5.1 observable inputs | Validation/result |
|---|---|---|
| `row` | row layout, main-axis alignment, cross-axis alignment, optional gap, Markdown body | stacked semantic node |
| `column` | column layout, main-axis alignment, cross-axis alignment, optional gap, Markdown body | stacked semantic node |
| `grid` | positive integer columns, both alignments, general gap, vertical gap, horizontal gap, Markdown body | stacked semantic node; non-positive columns fail |

The future surface requires `Size`, alignment enums, validated integers,
structured content, and component/node conversion. It does not authorize their
implementation here.

### Component value foundation slice (2026-08-21)

Implemented in this slice:

- typed `IrValue::Component(IrComponent)` value preservation;
- a closed backend-neutral `IrComponent::Stacked` model for row, column, and
  positive-column grid layouts;
- typed layout, main-axis/cross-axis alignment, `IrSize` gap, and `Vec<IrNode>`
  child fields;
- component and child source provenance, including `value_source_span()`;
- deterministic serde roundtrip and zero-column rejection; and
- an explicit output-materialization gate: block and inline component output
  fails with a source-backed diagnostic instead of flattening, stringifying, or
  silently dropping the value.

Deferred from this slice:

- `.row`, `.column`, and `.grid` source builtins;
- layout enum and `Size` argument binding/default application;
- body-to-component construction;
- `IrNode`/inline component materialization and pure Typst layout lowering.

The component value remains valid in value context and may pass through
variables and callable results. No compatibility claim for source row,
column, or grid behavior is promoted by this foundation.

## Compatibility matrix

| Feature | Quarkdown v2.5.1 behavior | Scribium current behavior | Architecture decision | Implementation status | Deferred work |
|---|---|---|---|---|---|
| Custom functions | `.function` defines a callable with a signature, lexical definition context, caller propagation, dynamic result, and separate output conversion | Bounded user functions with immutable definition capture, a lookup-only caller overlay, typed values, and isolated invocation child maps | Keep `IrCallable` as evaluator value; compose definition and caller layers without replacing capture | Partial, caller overlay implemented | Broader stdlib/component call surface and owner-mutation parity |
| Lambda explicit parameters | Parameters bind by signature and shadow outer implicit parameters | Explicit lambda parameters and shadowing are supported in the bounded evaluator slice | Parameter installation is last in child scope | Bounded implemented | Broader upstream scope fixtures |
| Lambda implicit parameters | Headerless parameters are `.1`, `.2`, …; explicit parameters mask them | Headerless invocation scope is nearest-first; a missing local implicit slot can resolve a propagated caller slot, while explicit scope remains a hard mask | Preserve implicit scope as evaluator state, never backend state | Bounded implemented | Broader upstream scope fixtures |
| Lexical/calling scope | Definition context is retained; mutable calling context can be propagated; nested lookup is nearest-first | Definition capture remains immutable; caller-visible variables/functions and the visible caller lambda scope are overlaid only for one invocation; local writes remain isolated | Adopt hybrid target model with explicit definition, caller-overlay, and invocation layers | Bounded compatibility implemented | Parent-owner reassignment and broader mutable-scope parity |
| Lazy body evaluation | Body is a lazy `DynamicValue`; unreachable conditional/body paths do not execute it | Block bodies are evaluated on the existing callable path after binding; conversion failures precede body execution | Preserve eager inline vs lazy body timing | Bounded implemented | More differential lazy-body fixtures |
| DynamicValue result | Dynamic results may be scalar, node, iterable, collection, or Markdown/content and are converted at output boundary | Typed `IrValue`, `IrValue::Content`, and the closed `IrValue::Component` foundation preserve semantic values; unresolved calls are preserved | Keep component values backend-neutral until output materialization | Component value foundation implemented; output materialization deferred | Component source construction and node materialization |
| Component/node result | `NodeValue` carries a semantic AST node; output visitors place it block/inline | Target-specific HTML remains a closed semantic node slice; `IrComponent::Stacked` is a typed value-only foundation with source provenance | Distinguish evaluated component values from unresolved calls and materialize only at a lossless typed boundary | Typed component foundation implemented; no source consumer | Row/column/grid construction, typed node, and Typst lowering |
| Document-state mutation | Document APIs read with no argument, mutate shared mutable document info with an argument, and return void | Evaluator-owned state shared by ordinary callable child scopes and caller-overlay invocations; final `IrMetadata.document_state` snapshot; `.docname`, `.docdescription`, and `.doctype` are implemented with bounded conversion | Evaluator-owned shared working state plus final `IrDocument` snapshot | Document State Foundation and caller sharing implemented; `.docname`, `.docdescription`, and `.doctype` implemented | Remaining document fields |
| `row` | Stacked row with alignments, optional gap, and Markdown body | Source builtin not implemented; the typed `IrComponent::Stacked(Row)` value shape is available for the next construction slice | Backend-neutral component value, then semantic node; no Typst representation in IR | Value foundation implemented; source behavior deferred | Row argument binding, body construction, node materialization, lowering |
| `column` | Stacked column with alignments, optional gap, and Markdown body | Source builtin not implemented; the typed `IrComponent::Stacked(Column)` value shape is available for the next construction slice | Same backend-neutral component boundary | Value foundation implemented; source behavior deferred | Column argument binding, body construction, node materialization, lowering |
| `grid` | Positive integer columns, alignments, general/vertical/horizontal gaps, Markdown body; non-positive columns fail | Source builtin not implemented; typed `Grid { columns: NonZeroU32 }` rejects zero in the public IR model | Validate before component construction and keep the result typed | Value foundation implemented; source behavior deferred | Grid argument binding, body construction, node materialization, lowering |
| `Size` conversion | `ValueFactory.size` parses typed/numeric/unit values with domain rules | Backend-neutral `IrSize` conversion is implemented for the exact seven-unit decimal grammar, with typed identity and origin-gated text | Domain-specific origin-aware conversion adapter | Implemented | Size consumers and layout semantics |
| `Color` conversion | `ValueFactory.color` accepts typed colors or domain text decoding | Backend-neutral `IrColor` conversion implements the ordered Hex/RGB/RGBA/HSV-HSL/Named decoder families and numeric channels | Domain-specific origin-aware conversion adapter | Implemented | Color consumers, style, and component semantics |
| Enum conversion | Closed enum values are matched through the allowed value set and public names | Explicit closed enum adapter is implemented; `.doctype` consumes `DocumentType` with case-insensitive public names and no static String coercion | Closed domain adapter; no reflective generic coercion | Implemented for `.doctype` | Alignment/layout enum consumers |
| Markdown conversion | Markdown/content conversion parses a raw dynamic value in the frontend context; node output is semantic | Already-parsed `IrValue::Content` is supported; String → Markdown reparsing is not | Content remains structured; raw String conversion requires a future explicit frontend/provenance contract | Partial | Content conversion boundary |
| Component conversion | Dynamic result can become a node/layout value through typed output visitors | Closed typed `IrValue::Component`/`IrComponent::Stacked` carrier exists; source construction and DynamicValue layout binding do not | Backend-neutral component value, origin-gated construction, typed output materialization | Value carrier/model and output gate implemented | Source row/column/grid construction and typed node materialization |

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
mutate IrCallableCapture, copy project/source/diagnostic runtime state, or turn
child variable writes into parent-owner mutation. Document state remains the
explicit shared runtime handle established by the Document State Foundation.
Parent-owner reassignment and broader mutable-scope parity remain
partial/deferred.

For this slice, definition capture, caller lookup overlay, invocation
parameter precedence, and implicit-parameter precedence are implemented.
Parent-owner reassignment mutation and other mutable scope parity remain
partial/deferred.

`InvocationValue` and `ValueOrigin` from PR #105 remain mandatory. Dynamic
textual values may use bounded target adapters; static `StringValue` does not
gain arbitrary typed meaning. In particular, dynamic `"2"` and `"2.0"` can
continue to satisfy the existing integer rule, dynamic `"1.5"` fails, and
static `StringValue("2")` fails.

Failures do not publish partial semantic output. Local child bindings remain
local; parent reassignment and document mutations already committed through a
shared mutable owner are not promised to roll back. Unverified upstream cases
remain compatibility gaps rather than invented transactions.

Provenance stays source-backed: argument/conversion failures point to the
argument expression, body failures to the original body, component properties
to their property source, and nested children retain their own spans. Typst
source-map entries are generated during lowering from those original spans.

## Intentionally deferred

This gate does not implement row, column, grid, any layout renderer or Typst
layout lowering, Size/Color consumers or layout enum consumers, String →
Markdown or String → component conversion, a parallel evaluator,
filesystem/network features, or an evaluator rewrite. No architecture
prototype or feature snapshot was
necessary: the existing Rust types and exhaustive backend consumer are enough
to select the representation at the document level.

The next implementation order is source component/content result construction,
typed semantic layout nodes, and finally pure Typst lowering. `.docauthor(s)`,
`.dockeywords`, `.doclang`, and `.theme` remain deferred.
