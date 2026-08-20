# Programmable Document Semantics

This document records the architecture-gate investigation for Quarkdown
v2.5.1 and Scribium issue #61. It is a compatibility record, not a feature
implementation plan that changes the current supported surface.

- Tracked upstream: Quarkdown v2.5.1
- Resolved upstream tag: [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e)
- Scribium comparison baseline: `054437ae08c805fcd1d897244ea1dc2aa38f6993`
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
layout node values. Scribium therefore chooses a future typed
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
parent, while a subdocument has an isolated copy. Scribium will represent this
as evaluator-owned working document state plus a final immutable state snapshot
in `IrDocument`; Typst lowering consumes the snapshot and does not replay calls.

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

## Compatibility matrix

| Feature | Quarkdown v2.5.1 behavior | Scribium current behavior | Architecture decision | Implementation status | Deferred work |
|---|---|---|---|---|---|
| Custom functions | `.function` defines a callable with a signature, lexical definition context, caller propagation, dynamic result, and separate output conversion | Bounded user functions with immutable capture, typed values, and isolated child maps; caller-overlay behavior is not complete | Keep `IrCallable` as evaluator value; separate invocation result from document node | Partial, existing slice | Differential scope compatibility and caller propagation |
| Lambda explicit parameters | Parameters bind by signature and shadow outer implicit parameters | Explicit lambda parameters and shadowing are supported in the bounded evaluator slice | Parameter installation is last in child scope | Bounded implemented | Broader upstream scope fixtures |
| Lambda implicit parameters | Headerless parameters are `.1`, `.2`, …; explicit parameters mask them | Headerless implicit scope exists for the current callable/callback slice | Preserve implicit scope as evaluator state, never backend state | Bounded implemented | Caller/definition overlay parity |
| Lexical/calling scope | Definition context is retained; mutable calling context can be propagated; nested lookup is nearest-first | Captures are immutable snapshots; captured calls do not yet overlay caller variables; child ordinary writes are local | Adopt hybrid target model; record current difference as a gap | Architecture decided; compatibility partial | Follow-up scope implementation |
| Lazy body evaluation | Body is a lazy `DynamicValue`; unreachable conditional/body paths do not execute it | Block bodies are evaluated on the existing callable path after binding; conversion failures precede body execution | Preserve eager inline vs lazy body timing | Bounded implemented | More differential lazy-body fixtures |
| DynamicValue result | Dynamic results may be scalar, node, iterable, collection, or Markdown/content and are converted at output boundary | Typed `IrValue` and `IrValue::Content`; unresolved calls are preserved; no general component carrier yet | Add future `IrValue::Component`; materialize to `IrNode` at output boundary | Architecture decided; implementation deferred | Component value and node materialization |
| Component/node result | `NodeValue` carries a semantic AST node; output visitors place it block/inline | Target-specific HTML has a closed semantic node slice; general components are not present | Keep node results backend-neutral and distinguish them from unresolved calls | Partial/closed slice | General component contract |
| Document-state mutation | Document APIs read with no argument, mutate shared mutable document info with an argument, and return void | Limited final `IrMetadata`; no evaluator working document state | Evaluator-owned shared working state plus final `IrDocument` snapshot | Architecture decided; implementation deferred | Metadata builtins and state tests |
| `row` | Stacked row with alignments, optional gap, and Markdown body | Not implemented | Future component value, then semantic node; no Typst representation in IR | Not implemented by design | Layout semantic slice |
| `column` | Stacked column with alignments, optional gap, and Markdown body | Not implemented | Same backend-neutral component boundary | Not implemented by design | Layout semantic slice |
| `grid` | Positive integer columns, alignments, general/vertical/horizontal gaps, Markdown body; non-positive columns fail | Not implemented | Validate in evaluator before component construction | Not implemented by design | Grid validation and lowering |
| `Size` conversion | `ValueFactory.size` parses typed/numeric/unit values with domain rules | No general Size target | Domain-specific origin-aware conversion adapter | Deferred | Size contract and tests |
| `Color` conversion | `ValueFactory.color` accepts typed colors or domain text decoding | No general Color target | Domain-specific origin-aware conversion adapter | Deferred | Color contract and tests |
| Enum conversion | Closed enum values are matched through the allowed value set and public names | No general enum target | Closed domain adapter; no reflective generic coercion | Deferred | Alignment/layout enum adapters |
| Markdown conversion | Markdown/content conversion parses a raw dynamic value in the frontend context; node output is semantic | Already-parsed `IrValue::Content` is supported; String → Markdown reparsing is not | Content remains structured; raw String conversion requires a future explicit frontend/provenance contract | Partial | Content conversion boundary |
| Component conversion | Dynamic result can become a node/layout value through typed output visitors | No general component conversion | Future `IrValue::Component`, origin-gated construction, typed output materialization | Deferred | Component property/child conversion |

## Normative evaluator rules

The observable order is: callee resolution; inline argument evaluation in
source order; named/positional binding; origin-aware conversion; child-scope
creation; parameter installation; lazy body decision; call execution; typed
result construction; document-state commit; result materialization.

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
layout lowering, full Size/Color/enum conversion, String → Markdown or String
→ component conversion, a parallel evaluator, filesystem/network features, or
an evaluator rewrite. No architecture prototype or feature snapshot was
necessary: the existing Rust types and exhaustive backend consumer are enough
to select the representation at the document level.

Recommended implementation order is document state, scope compatibility,
domain conversion adapters, component/content result construction, semantic
layout nodes, and finally pure Typst lowering.
