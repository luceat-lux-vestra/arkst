# ADR-0020: Programmable Document Semantic Model

- Status: Accepted
- Date: 2026-08-20
- Decision scope: Quarkdown v2.5.1 programmable-document semantics
- Related issue: #61
- Supersedes: none
- Related ADRs: [ADR-0006](0006-source-map-and-diagnostic-model.md), [ADR-0015](0015-compiler-crate-boundaries.md), [ADR-0016](0016-full-quarkdown-compatibility-and-upstream-evolution.md)

## Context

Scribium is extending the evaluator from scalar and structured values toward
the programmable-document semantics needed by the tracked Quarkdown release.
The current implementation already contains typed values, callable captures,
source-backed diagnostics, and invocation-time dynamic conversion. It also
retains unresolved calls when a compatible builtin is not yet implemented.

This gate is architectural. It does not implement `.row`, `.column`, `.grid`,
document builtins, `Size`, `Color`, general enum conversion, or a new
component family. The baseline is the PR #105 squash merge:
`054437ae08c805fcd1d897244ea1dc2aa38f6993`.

## Problem

The existing names can be read more strongly than the current guarantees:

- `IrNode` is described as evaluated in some comments, but it can still carry
  an unresolved `FunctionCall`.
- `IrValue::Content` carries structured document content, while a future
  layout/component result needs typed semantic properties and children.
- `IrCallable` contains both the language-level callable value and evaluator
  capture machinery.
- child scopes isolate ordinary maps, but there is no document-state model.
- the PR #105 `InvocationValue`/`ValueOrigin` boundary must remain intact while
  future conversion targets are added.

Without an explicit model, a component can accidentally become a Typst string,
an unresolved invocation can be mistaken for an evaluated node, or static text
can acquire arbitrary typed meaning.

## Upstream v2.5.1 observations

The investigation used the immutable Quarkdown v2.5.1 tag at commit
[`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e).
Only public source and permitted black-box observations were used; no upstream
source or tests are copied or translated.

### Function-call pipeline

The relevant public source-level pipeline is:

`FunctionCallWalkerParser` → `FunctionCallRefiner` → `FunctionCallNode` →
`FunctionCallNodeExpander` → `FunctionCallExpansionStage`.

There is no class literally named `FunctionCallWalker` at this tag; the walker
parser and its `WalkedFunctionCall` result provide that stage.

- `FunctionCallRefiner` turns inline arguments into lazy raw expressions and a
  body argument into a lazy `DynamicValue`; a body is not evaluated merely by
  being present.
- `FunctionCallArgument.value` is lazy. `RegularArgumentsBinder` nevertheless
  evaluates ordinary inline arguments in source order while binding them.
- the binder performs positional/named binding and DynamicValue conversion
  before invoking the function. The body argument is reserved for the body
  parameter and is evaluated only when the binding is consumed.
- a chain resolves the previous node and passes its result as the first
  argument to the next call.
- `FunctionCallNodeExpander` invokes the call, selects a block or inline
  `NodeOutputValueVisitor`, and appends a node. `NodeValue` becomes its semantic
  node; collections become document structure; scalar values become block or
  inline text; `VoidValue` produces no observable content.
- a dynamic result is not necessarily text. It can be a node, iterable,
  collection, or raw Markdown value and is converted at the output boundary.

The relevant source records are [`FunctionCallRefiner.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt), [`RegularArgumentsBinder.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt), [`FunctionCallNodeExpander.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/FunctionCallNodeExpander.kt), [`FunctionCallExpansionStage.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/pipeline/stages/FunctionCallExpansionStage.kt), [`NodeOutputValueVisitor.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/output/node/NodeOutputValueVisitor.kt), and [`ValueFactory.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt).

### Lambda and lexical scope

`Lambda` stores a definition `parentContext`. At invocation it forks that
context, propagates the calling context's libraries when the caller is
mutable, and installs lambda parameters last. Consequently explicit
parameters shadow outer implicit parameters, and nested lookup is nearest
binding first. Headerless parameters are named `.1`, `.2`, and so on.

`Flow.function` declares a function from a lambda and passes the call context
when the function is invoked. Its body may therefore observe caller libraries
in addition to its definition context. `.var` resolves an existing variable's
owner through the scope chain; assigning an existing outer variable can mutate
that owner, while a new variable is local to the selected context.

The upstream source records are [`Lambda.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt), [`ScopeContext.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ScopeContext.kt), [`MutableContext.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/MutableContext.kt), [`SubdocumentContext.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/SubdocumentContext.kt), and [`Flow.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt).

### Custom functions and components

The v2.5.1 `Flow.kt` model separates function definition, parameter signature,
definition environment, calling environment, dynamic result, output conversion,
and mutation. `.if`/`.ifnot` invoke a body only on the selected branch;
`.foreach` and `.repeat` invoke a callback for each element; `.function`
registers a callable and returns `VoidValue`; `.var` can mutate an existing
owner. A custom function is therefore not just a body macro.

The upstream API has no single mandatory language type named component. The
observable categories are summarized below.

| Source call/result category | Evaluator result | Document observability | Backend responsibility |
|---|---|---|---|
| Native scalar function | typed scalar value | scalar output or parameter value | lower the already-validated scalar |
| Native function returning `NodeValue` | semantic node value | node is inserted at block/inline boundary | lower the backend-neutral node |
| Native Markdown/content result | structured content or node value | children are materialized in place | lower structured children |
| Native mutator returning `VoidValue` | no output plus context mutation | later reads observe committed state | consume final state only |
| Custom function returning `DynamicValue` | typed/dynamic result after conversion | output category depends on result | lower the materialized semantic result |
| Layout function | node value containing layout and children | semantic layout node | choose concrete rendering constructs |

`Layout.kt` models row, column, and grid as `Stacked` semantic nodes. It
validates grid columns before constructing the node. This is evidence for the
future representation, not an implementation slice in this ADR.

### Document state

`Document.kt` implements read/write dual semantics through
`modifyOrEchoDocumentInfo`: an argument-less call returns the current field; a
call with an argument mutates `MutableContext.documentInfo` and returns void.
`ScopeContext` delegates document info to its parent, while a subdocument gets
its own copy. The fields include type, name, description, authors, keywords,
locale, and theme.

The source records are [`Document.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt) and [`DocumentInfo.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentInfo.kt).

## Current Scribium constraints and reconnaissance

At the baseline, the relevant current types have these meanings:

| Type | Current observable role | Gate decision |
|---|---|---|
| `IrDocument` | document nodes plus limited `IrMetadata`; evaluator returns it after a pass | remains the final backend-neutral document, with a future explicit metadata/state snapshot |
| `IrNode` | structural document node; `FunctionCall` and chains may still be unresolved | evaluated semantic nodes and unresolved structural invocations must remain distinguishable |
| `IrValue` | typed scalar/collection/content/none/callable value | remains the language value carrier; future components are values until output materialization |
| `IrValue::Content` | structured `Vec<IrNode>` document content | remains content, not a component property bag |
| `IrCallable` | callable language value plus immutable captured variables/functions | remains the evaluator carrier; it is not a backend node |
| `FunctionBinding` | user-function signature/body/capture used by the evaluator | remains evaluator-owned |
| `EvaluationContext` | child-scope maps, lambda scope, project/source identity, active sources | gains an explicit evaluator-owned document working state in the implementation slice that needs it |
| `CallOutcome` | `Value`, `NoValue`, `Failed`, or `Unresolved` | remains the boundary distinguishing completed results, no output, failure, and unresolved calls |
| `InvocationValue` / `ValueOrigin` | invocation-time origin for bounded DynamicValue conversion | remains mandatory for every future conversion target |

`IrNode` is therefore not currently “completely evaluated” in the strong
sense. A resolved call can materialize a value into a node, but an unsupported
call remains a structural unresolved call. `IrValue::Content` is the content
carrier; a semantic component is a typed value with properties and children
that is materialized only at the document output boundary.

Current child scopes clone visible variable/function maps, so local declarations
and reassignment stay in the child. Captures are immutable snapshots and a
captured callable does not currently overlay the caller's variables. The
current implementation has no document state beyond the final limited
`IrMetadata`. These are compatibility gaps recorded below, not reasons to
rewrite the evaluator in this gate.

## Decision

Scribium adopts a value-first, typed semantic component boundary:

1. A future `IrValue::Component` carries an `IrComponent` whose layout kind,
   validated properties, semantic children, and source provenance are all
   backend-neutral.
2. The evaluator constructs and validates the component. It never stores
   Typst function names, Typst source, or backend escape strings in the value.
3. A completed component is materialized into a typed `IrNode` at the same
   output boundary that materializes `IrValue::Content`, collections, and
   scalar values. The exact node variant is deferred until the first component
   implementation establishes its child/output invariants.
4. `IrNode::FunctionCall` and `IrNode::ChainedFunctionCall` continue to mean
   structural unresolved invocations when they survive evaluation. They are
   not evaluated components.
5. No general `SemanticValue`/HIR/MIR layer is introduced. The one-IR target
   architecture remains in force.

This is Option B from the alternatives below, with an explicit output
materialization boundary rather than a component-specific node-only variant.

### Semantic result categories

The existing `CallOutcome` is retained as the evaluator result envelope:

- `Value(IrValue)` — completed typed language value, including future component;
- `NoValue` — successful mutation/declaration with no document output;
- `Failed` — source-backed diagnostic and no published result;
- `Unresolved` — unsupported call preserved structurally for compatibility
  policy and later implementation.

No separate general semantic-result type is needed. The distinction between a
completed component and an unresolved invocation is represented by
`CallOutcome::Value(IrValue::Component(...))` versus
`CallOutcome::Unresolved`, and the distinction between content and a component
is represented by `IrValue::Content` versus `IrValue::Component`.

## Evaluation model

For a call with regular inline arguments and an optional body, the normative
observable order is:

1. resolve the callee and chain input;
2. evaluate inline positional and named expressions left-to-right in source
   order;
3. bind positional and named arguments to the parameter signature;
4. perform origin-aware DynamicValue conversion for each bound value;
5. create the callable child scope;
6. install parameters, with explicit parameters or implicit `.1`, `.2`, …
   bindings shadowing outer implicit bindings;
7. decide whether the lazy body is reachable and, only then, evaluate/bind it;
8. execute the native or user function body;
9. construct the typed semantic result or `NoValue`;
10. commit document-state mutations according to the state model;
11. materialize the result into block/inline document output.

Inline arguments are eager at the binding boundary. Body arguments are lazy;
an unreachable conditional branch or a failed earlier conversion must not
execute the body. The evaluator may fuse internal phases, but it must preserve
this observable order.

## Scope model

The target model is a hybrid of lexical definition capture and caller
propagation observed in v2.5.1:

- a callable remembers its definition environment;
- a call may expose the calling environment to the body for caller-visible
  libraries/variables;
- invocation parameters are installed last and therefore have highest
  precedence;
- explicit lambda parameters mask outer implicit `.1`, `.2`, … values;
- nested lambdas search their parameter scope, then caller-visible scope, then
  definition/captured scope, then outer scopes according to the evaluator's
  deterministic lookup order;
- a declaration made inside a child callable scope is local unless it is an
  explicit reassignment of an existing parent variable;
- reassignment of an existing parent variable may mutate that owner;
- document state is shared by ordinary callable child scopes and is committed
  through the evaluator state boundary.

The current immutable capture implementation is narrower: a captured callable
uses the capture snapshot without caller-variable overlay, and ordinary child
map writes are isolated. That observable difference is a documented
compatibility gap. This ADR does not silently change it or introduce a parallel
evaluator; a later bounded scope-compatibility PR must add differential tests
and update this decision if the gap is closed.

## Document-state model

Document metadata and document-level mutable state are evaluator-owned. The
implementation model is an explicit working `DocumentState` held by the
evaluation context and shared by ordinary callable child scopes. A successful
evaluation returns its final immutable snapshot as part of `IrDocument`
metadata/state. A subdocument or future isolated compilation unit may clone
the state at its boundary.

Read/write dual APIs follow this rule:

- no argument: read the typed current field and return it as `IrValue`;
- argument present: validate/convert, mutate the working state, return
  `CallOutcome::NoValue`;
- backend lowering consumes the final snapshot; it never replays source calls
  or reconstructs metadata from call history.

The exact Rust field decomposition is deferred until the first document-state
implementation, but the ownership and read/write semantics are fixed here.

## Dynamic conversion boundary

PR #105 remains normative. `InvocationValue` and `ValueOrigin` are attached at
invocation time. `ScalarTarget` remains for bounded scalar conversion, while
future semantic targets are separate target families:

- scalar: number, boolean, string, and existing range conversion;
- domain values: `Size`, `Color`, and closed typed enums;
- content: already-structured Markdown/content values;
- component: typed semantic component construction.

The conversion dispatcher remains evaluator-owned and origin-aware. It may be
split into domain modules behind the existing `value_conversion.rs` boundary,
but no target may bypass `InvocationValue`.

- Dynamic textual values may use a bounded domain decoder.
- Existing typed values may pass through identity conversion where the target
  explicitly permits it.
- A static `StringValue("2")` may not become an integer merely because the
  target asks for one.
- `DynamicValue("2")` and `DynamicValue("2.0")` may continue to become an
  integer where the existing bounded integer rule permits them; `"1.5"`
  fails.
- String-to-Markdown and String-to-component reparsing/materialization are
  deferred and require an explicit provenance-preserving frontend contract.

`Size` and `Color` should use domain-specific adapters. Enums should use a
closed adapter supplied with the allowed values and their source-facing names,
not a reflective generic enum conversion. Conversion failures point first to
the offending argument span, with the parameter span as secondary context.

## Frontend / backend boundary

| Layer | Owns |
|---|---|
| Parser | syntax recognition and source spans |
| AST → IR | source-backed structural normalization; no source rewrite/reparse |
| Evaluator | callee execution, scopes, binding, DynamicValue adaptation, validation, component construction, document-state mutation, output materialization |
| Semantic IR | fully backend-neutral document meaning, typed values, semantic nodes, final state snapshot, provenance |
| Typst lowering | pure semantic IR → Typst constructs and generated source maps |

Typst lowering does not decide Quarkdown defaults, DynamicValue conversion,
enum parsing, lexical scope, function evaluation, grid column validity, or
document metadata semantics. Names such as `TypstGrid`, `TypstAlign`, and raw
`#grid(...)` are forbidden in semantic IR.

## Provenance model

Every semantic object keeps the most specific original source span available:

| Object | Primary provenance |
|---|---|
| invocation | full call span |
| argument | exact argument expression/body span |
| parameter-binding failure | offending argument and parameter declaration |
| converted DynamicValue | original argument span, not a synthesized target span |
| lambda | lambda header/body source span |
| lambda-produced content | each original child span |
| component | call/property construction span |
| component property | property argument span or declaration span |
| document mutation | document-call/property span |
| generated/default property | no invented source span; retain call/parameter context only |
| nested child content | original nested child span |

Synthetic values must not collapse all descendants to the call-site span.
Diagnostics choose the narrowest offending source. Typst lowering preserves
the source spans needed to emit source-map entries; generated ranges are not
stored in semantic IR.

## Failure model

Binding and conversion failures occur before body execution. Semantic output is
published atomically at the call output boundary: a failed call does not append
partially materialized children or a partially built component.

There is no blanket transaction/rollback promise for all evaluator state:

- local parameter bindings and local declarations disappear with a failed
  child call;
- earlier top-level output and diagnostics remain;
- parent-variable reassignment that already happened may remain, matching the
  observed mutable-owner model;
- document-state mutations already committed through the shared working state
  may remain after a later failure;
- a function declaration is visible only after its declaration step succeeds;
- a failed result is not published as output and is not duplicated as a second
  generic error.

Where v2.5.1 behavior is not yet independently established, the compatibility
matrix marks the case as a gap rather than inventing rollback semantics.

## Row, column, and grid classification only

No row/column/grid builtin or Typst lowering is implemented by this ADR. The
v2.5.1 observable contract is recorded for future work:

| Function | Semantic inputs | Result/validation |
|---|---|---|
| `row` | layout=row, main-axis alignment, cross-axis alignment, optional gap, Markdown body | semantic stacked node |
| `column` | layout=column, main-axis alignment, cross-axis alignment, optional gap, Markdown body | semantic stacked node |
| `grid` | positive integer columns, both alignments, general gap, vertical gap, horizontal gap, Markdown body | semantic stacked node; non-positive columns fail before construction |

These contracts identify future `Size`, alignment enums, validated integer,
content, and component/node conversion targets. They do not authorize their
implementation in this gate.

## Rejected alternatives

### Option A — component-specific `IrNode` variants first

Rejected as the primary representation. It makes a callable result and a
document-placement result inseparable, complicates scalar/content return
paths, and encourages unresolved calls and evaluated components to share one
node shape. A typed node may still be the materialized output of a component.

### Option B — `IrValue::Component` plus output materialization

Accepted. It matches the existing value-first callable pipeline, keeps result
conversion in the evaluator, and permits the same component to be returned,
passed, or nested before it becomes document output.

### Option C — a separate `SemanticValue`/`EvaluationResult` hierarchy

Rejected. `CallOutcome` already supplies the needed success/no-value/failure/
unresolved envelope and the one-IR architecture rejects a parallel semantic
IR tier. A new general hierarchy would duplicate ownership and increase
recursion/serialization risk without resolving the component boundary.

### Option D — extend `FunctionCall` for evaluated components

Rejected. `FunctionCall` is structural/unresolved state. Overloading it with a
completed component would make “not yet evaluated” and “evaluated semantic
result” observationally ambiguous and would make backend lowering responsible
for evaluation state.

### Broad `IrValue` coercion

Rejected. It would undo PR #105's invocation-time origin policy and allow static
text to acquire arbitrary typed meaning.

## Consequences

Positive consequences:

- components can remain backend-neutral values until a typed output boundary;
- unresolved calls are distinguishable from completed semantic results;
- future layout and document slices have explicit ownership and provenance;
- the PR #105 conversion safety invariant remains intact;
- Typst lowering remains pure and replay-free.

Costs and compatibility gaps:

- current `IrMetadata` and immutable captures do not yet implement all of this
  target model;
- a later implementation must add exhaustive IR/lowering handling for the
  chosen component node materialization;
- scope overlay and failure-state behavior need differential fixtures before
  claiming full v2.5.1 compatibility.

## Deferred work

Follow-up slices should be independently scoped and tested:

1. document working-state snapshot and the read/write metadata builtins;
2. scope/caller-overlay compatibility fixtures and any bounded evaluator fix;
3. domain conversion adapters for `Size`, `Color`, and closed enums;
4. content/component value construction with exact child provenance;
5. semantic row/column/grid nodes and validation;
6. pure Typst lowering for the approved semantic layout nodes.

This ADR intentionally does not implement any of those features. In
particular, row, column, and grid remain unimplemented.
