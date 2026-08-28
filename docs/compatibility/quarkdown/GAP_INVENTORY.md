# Quarkdown v2.5.1 Public-Language Gap Inventory

## Review snapshot

The cross-audit canonical view, evidence map, and dependency-aware backlog are
maintained in [`RECONCILIATION.md`](RECONCILIATION.md). This document remains
the detailed gap index; its family summaries do not override the canonical
owner/status rows in the audit matrices and manifests.

- **Tracked target:** Quarkdown `v2.5.1`
- **Resolved tag commit:** `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- **Repository:** [`iamgio/quarkdown`](https://github.com/iamgio/quarkdown)
- **Review date:** 2026-08-25
- **Scribium comparison baseline:** `7144683346fd6e39c49ef0923733c856a6a55f42`
- **Rushdown:** unchanged at
  `e5eb4e4446541ea0ed53111c1b37e779283ff57c`

This is an evidence-backed inventory of the public language surface, not a
claim that parsing implies compatibility. A row is compatible only when the
observable behavior in the row is implemented through evaluation and the
relevant output boundary and is covered by Scribium tests. The inventory uses
the v2.5.1 tag sources and public documentation/API pages; upstream source and
tests were inspected as behavioral evidence under the clean-room policy, not
copied or translated.

For document metadata, the canonical per-surface #147 classification is now
[`DOCUMENT_STATE_AUDIT.md`](DOCUMENT_STATE_AUDIT.md): eight bounded #152 state
rows are `PARTIAL`. `.localization`/`.localize` remain canonical #151-owned
`UNSUPPORTED` general stdlib rows; the #152 manifest retains them only as
`NOT_APPLICABLE` ownership handoffs. Their bounded implementation owner is
[#196](https://github.com/luceat-lux-vestra/scribium/issues/196); `.get` is
owned by [#194](https://github.com/luceat-lux-vestra/scribium/issues/194),
library inspection by [#195](https://github.com/luceat-lux-vestra/scribium/issues/195),
and `.log`/`.debug`/`.error` by
[#197](https://github.com/luceat-lux-vestra/scribium/issues/197). The broader
classification index below remains a family implementation index; it does not
override the canonical #151/#152 rows or promote evaluator/IR state to
end-to-end renderer support.

For layout, pagination, style, and document-wide presentation state, the
canonical per-surface #147 classification is now
[`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md)
and its [manifest](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv). The
historical family rows below intentionally remain an index for cross-audit
navigation; they do not override the #153 result. In particular, unresolved
call retention is `PARSED_ONLY` in #153, while component-local `.row`,
`.column`, `.grid`, `.container`, `.align`, `.center`, `.landscape`, and
`.whitespace` remain #154 handoffs rather than document state.

Status meanings:

- **Compatible:** the reviewed surface has tested semantic support at the
  stated boundary.
- **Partially compatible:** a bounded subset works, while documented behavior
  remains missing or intentionally different.
- **Unsupported:** the surface is known and may parse/preserve, but Scribium
  does not provide its semantics.
- **Scribium extension:** Scribium behavior without a corresponding public
  v2.5.1 declaration; it is not counted as upstream conformance.
- **Intentionally deferred:** a known gap assigned to a later milestone or
  blocked on a host/architecture boundary.

Classification index:

| Classification | Inventory rows |
|---|---|
| Supported at the stated bounded boundary | Logical and comparison operations; optionality callbacks; Collection selector sorting; the VirtualProject/resource model as `SUPPORTED_SEMANTICS`; argumentless inline `.br`; bounded `.docauthor`/`.docauthors`; block-only Stacked row/column/grid consumers |
| Partially compatible | Function declarations/calls; variables; conditionals; lambdas/callables; iteration; collections; Dictionary/Pair/Range; type/value conversion; strings/text; metadata/document setup; complete public component/style/layout surface; complete error taxonomy |
| Compatible at the evidenced bounded boundary | Mathematics/numeric operations |
| Unsupported | Layout/document functions outside the reviewed Stacked, Container, and Landscape slices; unimplemented data-loading families such as `.csv`, `.listfiles`, and `.filename`; #151 `.get`, library inspection, localization, and logger families; #154 `.match`, `.loremipsum`, and `.subdocumentgraph` pending their explicit owner/blocker |
| Scribium extension | `.map` and `.filter` collection transforms; they are tested Scribium behavior but are not v2.5.1 upstream features |
| Intentionally deferred | Unimplemented data-loading families, `.llmstxt`, function-driven metadata, remaining container style/layout families, `.css`/`.cssproperties` until a target-specific HTML backend/product contract exists, arbitrary comparator syntax not present in v2.5.1, and generalized DynamicValue conversion |

The complete value taxonomy, invocation-time binding, target-driven conversion,
diagnostic/provenance, and state-atomicity review is canonical in
[`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md). The rows above remain the
family-level inventory; they do not duplicate that matrix.

## Issue #154 content/media/Markdown-extension reconciliation

The canonical pinned inventory for content, media, presentation components,
raw content, references, captions, and Quarkdown-specific Markdown extensions
is [CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md)
and its [machine-checkable manifest](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv).
It starts from Scribium commit 7144683346fd6e39c49ef0923733c856a6a55f42 and
Quarkdown v2.5.1 commit 107ec3a9482f10d6f90d7580f8409b46a719d18e.

The manifest contains 71 #154-owned rows and 12 explicit handoffs. Its owned
rows classify 13 bounded end-to-end surfaces, 3 semantic-only surfaces, 1
parser-only surface, 13 partial surfaces, 37 unsupported surfaces, 2
deferred surfaces, 1 blocked surface, and 1 unknown surface. The 12
NOT_APPLICABLE rows preserve #153 global configuration, #155 resource
environment, #149/#165–#167 binding/conversion/raw-body, #158 nested-call,
#160 inline-content, and #180 texmacro ownership. This is an audit-only
reconciliation; no newly discovered implementation is started by this audit.

The previously unresolved #154 rows now have explicit dispositions: `.match`
is owned by [#198](https://github.com/luceat-lux-vestra/scribium/issues/198),
`.loremipsum` and `.keybinding` are bounded under
[#184](https://github.com/luceat-lux-vestra/scribium/issues/184), and
`.subdocumentgraph` is blocked by [#188](https://github.com/luceat-lux-vestra/scribium/issues/188)
with producer/output ownership in
[#199](https://github.com/luceat-lux-vestra/scribium/issues/199). `.css` and
`.cssproperties` remain `UNSUPPORTED` with an explicit product/backend defer;
closed historical issue #58 is not their current owner.

## #148 call-grammar audit

The complete call-grammar/frontend inventory is maintained in
[`CALL_GRAMMAR_AUDIT.md`](CALL_GRAMMAR_AUDIT.md). It revalidates #60/#65 at
the current `origin/main` base and keeps parser evidence separate from
binding, evaluator, IR, and output claims.

The audit classifies ordinary calls, named-argument identifiers and delimiter
adjacency, implicit positional references,
positional/named and multiline arguments, continuation, nested calls, chains,
tight calls, inline/block placement, dynamic body indentation, protected
Markdown contexts, escaped delimiters, malformed recovery, argument-ownership
boundaries, and source provenance. The #157 lexical production slice is
implemented as recorded in the audit below. The remaining five bounded
production follow-ups are:

- [#158](https://github.com/luceat-lux-vestra/scribium/issues/158) — preserve
  nested tight-call wrappers inside content arguments;
- [#159](https://github.com/luceat-lux-vestra/scribium/issues/159) — retain
  source after malformed inline-call recovery; and
- [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) — preserve
  supported Markdown inline structure in Quarkdown content arguments.
- [#162](https://github.com/luceat-lux-vestra/scribium/issues/162) — align
  escaped call and argument delimiter recognition/depth handling with pinned
  `GrammarUtils.kt` and `FunctionCallGrammar.kt`; bounded to
  `scribium-quarkdown` with frontend integration only if required.
- [#164](https://github.com/luceat-lux-vestra/scribium/issues/164) — align
  optional argument-separator placement before the first argument and `::`,
  plus pinned trailing-continuation consumption; bounded to
  `scribium-quarkdown` separator scanning and `scribium-markdown` block/inline
  integration, with LF/CRLF evidence kept separate.

These remaining gaps are not hidden by expected-failure allowlists or by the
#157/#163 implementations. #157 is limited to the grammar/frontend identifier,
delimiter, boundary, and provenance contract; #163 is limited to the ordered
mixed-argument grammar/frontend handoff, which the #165 shared engine binder
now consumes as the single semantic binding source.

## #149 value-model, binding, and conversion audit

The canonical value/binding/conversion matrix is
[`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md). Its primary statuses are
conservative: bounded numeric, Boolean, range, dictionary, dynamic-origin,
nested-evaluation, and document-state semantics are evidenced at the semantic
boundary; strings/content, None/defaults, enums, collections, binding, body
values, diagnostics, and general atomicity remain `PARTIAL` where the public
v2.5.1 contract is broader than the current slice. The audit does not infer
generalized coercion.

The actionable engine-owned follow-ups are [#165](https://github.com/luceat-lux-vestra/scribium/issues/165)
(central binder validation/parameter metadata),
[#166](https://github.com/luceat-lux-vestra/scribium/issues/166)
(dynamic/content target conversion), and
[#167](https://github.com/luceat-lux-vestra/scribium/issues/167)
(conversion diagnostics and validate-then-commit atomicity). They are native
sub-issues of #149. #163 provides the parser/frontend and IR prerequisite for
the ordered argument shape; #165 implements the shared engine binding contract
for positional-after-named and the related slot rules. #150–#155 remain the broader programmable,
general-builtin, metadata, layout, content, and resource audits; #156 remains
the reconciliation gate.

No production fix is selected by #149, and implementation ordering follows
the dependency bands in [#156 reconciliation](RECONCILIATION.md).

## #150 programmable-language semantics audit

The canonical programmable-language inventory is
[`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md). It
reconciles the v2.5.1 observable contracts for variables and mutation owners,
temporary `.let` bindings, definition/caller/parameter/iteration scopes,
callable definitions and recursion, evaluation order and laziness, optionality
callback invocation, chain outcomes, conditionals, iteration, collection
evaluation boundaries, the `.node` value-to-content boundary, definition
precedence, failure effects, and source-backed deterministic diagnostics.

The primary #147 statuses are conservative: the bounded callable, scope,
parameter, `.let`, evaluation, chain, conditional, iteration,
optionality/callback, collection, precedence, failure, and diagnostic rows are
`PARTIAL`; `.node` and `.extend`/`.super` are `UNSUPPORTED`; document-state
separation is `NOT_APPLICABLE` to #150. No row is promoted to
`SUPPORTED_END_TO_END` by this audit. The existing #61/#131 evidence remains
valid for its bounded foundation but does not establish full v2.5.1
compatibility.

The audit reuses #148/#158–#164 for grammar/frontend boundaries and
#149/#165–#167 for binding, conversion, diagnostics, and commit atomicity. It
does not create duplicate builtin or test issues. The only new cohesive
semantic follow-up is [#169](https://github.com/luceat-lux-vestra/scribium/issues/169)
for engine-owned `.extend`/`.super` semantics; implementation ordering follows
the dependency bands in [#156 reconciliation](RECONCILIATION.md).

## v2.5.1 stdlib surface classification

Issue #151 now has a dedicated canonical inventory in
[`STDLIB_BUILTINS_AUDIT.md`](STDLIB_BUILTINS_AUDIT.md), with the exact
162-name pinned manifest in
[`STDLIB_BUILTINS_AUDIT_MANIFEST.tsv`](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv).
The classification below remains a historical family-level seed and is not
the canonical #151 ownership/status matrix; it intentionally retains the
broader cross-audit rows for navigation.

The pinned v2.5.1 stdlib tree at commit
`107ec3a9482f10d6f90d7580f8409b46a719d18e` was re-enumerated on the current
Scribium head. Names below are the public `@QFunction` names, including
`@Name` aliases. “Implemented” always means the bounded semantic boundary
stated in the corresponding family row; it does not promote the whole family
to complete compatibility.

| Classification | v2.5.1 functions | Boundary / reason |
|---|---|---|
| Bounded semantic evidence (canonical status varies) | `.read`, `.json`, and `.include` | These are `PARTIAL` resource rows; the exact supported subset and missing project/global behavior are in #155 and #188. The remaining names in this historical family index must be read with the #151/#154/#155 manifests, not as a complete implemented list. |
| Partially implemented (Unicode string semantics) | `.capitalize`, `.startswith` | The pinned `StringCase.Capitalize` uses `Char::titlecase`, while the current engine uses `char::to_uppercase`; pinned Kotlin `String.startsWith(prefix, ignoreCase)` uses character-wise case-insensitive matching, while current Scribium lowercases complete strings. These gaps are independently evidenced with `ǳ`/`ǲ`/`Ǳ` and Greek `ς`/`Σ`; follow-up #172 is bounded to this string-semantics slice. |
| Partially implemented (bounded) | `.container` | Empty/body-only containers plus `width`, `height`, and `fullwidth` sizing use the typed Container component and Typst block lowering; deferred style/layout parameters remain explicitly unsupported. |
| Partially implemented | `.autopagebreak`, `.currentpage`, `.font`, `.footer`, `.formatpagenumber`, `.lastheading`, `.marker`, `.navigation`, `.noautopagebreak`, `.nonumbering`, `.numbering`, `.pageformat`, `.pagemargin`, `.paragraphstyle`, `.resetpagenumber`, `.tableofcontents`, `.texmacro`, `.totalpages` | Project/front-matter or IR metadata provides only a different, partial boundary; remaining function-driven document context and observable layout state are not implemented. |
| Unsupported | `.allemojis`, `.bibliography`, `.box`, `.cite`, `.clip`, `.code`, `.codespan`, `.collapse`, `.debug`, `.emoji`, `.error`, `.extend`, `.figure`, `.filetree`, `.float`, `.fragment`, `.fullspan`, `.functionexists`, `.get`, `.heading`, `.icon`, `.image`, `.keybinding`, `.libraries`, `.libexists`, `.libfunctions`, `.link`, `.localization`, `.localize`, `.log`, `.loremipsum`, `.math`, `.match`, `.mermaid`, `.numbered`, `.pagebreak`, `.paragraph`, `.ref`, `.slides`, `.speakernote`, `.subdocumentgraph`, `.table`, `.tablebyrows`, `.tablecolumn`, `.tablecolumns`, `.tablecompute`, `.tablefilter`, `.tablesort`, `.text`, `.textcollapse`, `.todo`, `.xychart` | The call may be preserved or parsed, but no approved evaluator/backend-neutral semantic implementation exists. `.text`, `.codespan`, `.clip`, `.float`, `.fullspan`, and the remaining component/layout families remain separate compatibility work; the bounded `.row`/`.column`/`.grid`, `.center`, `.align`, `.landscape`, and `.container` slices are classified above. |
| Unsupported (explicit ownership) | `.node` | Public `Flow.kt` `@QFunction`; #150 records only its programmable value-to-content and lazy-body boundary, while Node/Markdown representation, materialization, and backend/output fidelity belong to #154. No Scribium `.node` evaluator path or conformance claim exists. |
| Intentionally deferred | `.csv`, `.css`, `.cssproperties`, `.env`, `.filename`, `.htmloptions`, `.includeall`, `.listfiles`, `.llmstxt`, `.pathtoroot`, `.subdocument` | Resource families, target-specific options, and process/environment access are explicitly outside this slice or require a separate host/security decision. `.llmstxt` remains deferred per task scope. |
| Scribium extension | `.map`, `.filter` | Existing typed collection transforms have no corresponding public v2.5.1 `Collection.kt` declarations and are excluded from upstream compatibility counts. |

The pinned source sweep corrected one seed statement: `.llmstxt` is a public
`@QFunction` in `Html.kt`, with `content: String` and
`markdownavailable: Boolean`. It remains deferred and #155-owned; see the
canonical #151 audit for the full source link and ownership disposition.

### `.captionposition` block-body fallback gap

Quarkdown v2.5.1's `RegularArgumentsBinder` permits an indented body after
regular arguments and, because `codeBlocks` is the final bindable
`.captionposition` parameter, falls back to that parameter as raw
`DynamicValue` text. Scribium's current frontend/IR boundary exposes a parsed
`CallBody` rather than lossless raw body text. Therefore the bounded native
setter rejects a body before evaluation, exactly as the bounded `.theme` slice
does. This is an explicit compatibility gap, not an upstream semantic claim
that bodies are invalid; implementing it requires an accepted frontend/IR raw
body representation and is outside this PR.

## R11/R12 current semantic evidence

The R11 and R12 reviews rechecked the three issue-#61 areas against the pinned
v2.5.1 source and public documentation instead of carrying forward prior gap
labels. R11 owner writeback and R12 inline iteration are completed evidence at
the current comparison baseline below. The broader public component, style,
conversion, and context surfaces remain explicit gaps.

| Area | Upstream behavior and evidence | Current Scribium behavior at `3829d847` | Classification | Current evidence / remaining gap |
|---|---|---|---|---|
| Iteration | `Flow.kt` defines `.foreach` as an ordered map over any `Iterable` and `.repeat` as `.foreach {1..N}`. The v2.5.1 loops documentation also shows an inline lambda body and states that Markdown lists are accepted. Evidence: [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt), [loops](https://quarkdown.com/wiki/loops/), [`LoopTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/LoopTest.kt). | Block and inline `.foreach`/`.repeat` callable-body forms are implemented with explicit/implicit parameters, Pair destructuring, finite/left-open range materialization, ordered results, child isolation, semantic-owner writeback, failure atomicity, source-defined native shadowing in direct and chained calls, and R10 materialization/depth limits. The inline carrier remains `Value::InlineBody`/`IrValue::InlineBody` until native resolution, then converges on `IrCallable`; `.map`/`.filter` remain Scribium extensions. | Partially compatible; this documented iteration slice is evidenced, while the broader public consumer set, endless-range consumption, generalized patterns, and arbitrary inline component/callback bodies remain absent. | Completed R12 evidence: `compile_inline_foreach_and_repeat_use_the_shared_callable_path`, `compile_inline_foreach_preserves_pair_destructuring`, `compile_inline_foreach_reuses_materialization_budget`, `compile_inline_foreach_preserves_owner_reassignment_and_parameter_shadowing`, `compile_inline_foreach_rhs_sees_outer_owner_with_different_parameter_name`, `compile_inline_foreach_keeps_new_variables_invocation_local`, `compile_source_defined_foreach_and_repeat_shadow_native_direct_and_chain`, and `compile_inline_foreach_failure_is_atomic_and_source_backed`. |
| Functions/components | v2.5.1 `Lambda.kt` keeps a definition context, propagates caller libraries for dynamic references, and installs parameters last. `Flow.kt` returns custom-function results dynamically; `Layout.kt` models row/column/grid as semantic nodes. Evidence: [`Lambda.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt), [`Layout.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Layout.kt). | Source-defined functions, explicit/implicit callable parameters, typed callable values/captures, caller lookup overlay, callback invocation, nested callable execution, and typed row/column/grid plus existing component paths are implemented. The complete public stdlib/component surface, generalized DynamicValue output conversion, and deferred style/layout families remain absent. | Partially compatible; bounded consumer slices are implemented, while the public family remains partial. | Current evidence is in the Stacked layout and callable rows below; no additional #61 foundation blocker was identified. Broader public component/function expansion remains separate work. |
| Scoping/evaluation | v2.5.1 `Flow.kt` searches parent contexts for the existing variable owner before replacing a variable; `RegularArgumentsBinder` evaluates `DynamicValue` arguments in the normal call context before `.var` receives them, and the pinned `VariableTest.kt` verifies that `.var {total} ...` inside `.foreach` updates the outer `total`. `Lambda.kt` forks invocation context and installs parameters last. Evidence: [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt), [`VariableTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/VariableTest.kt). | Callable invocation evaluates `.var` RHS values in normal invocation lookup, resolves the assignment target to its semantic owner, and writes back successful existing caller-visible owners while preserving parameter shadowing and new-local isolation. Failed callable/iteration evaluation publishes no partial variable mutation. | Partially compatible; the targeted scope/evaluation-order/writeback contract is evidenced while broader context/library parity remains absent. | Current tests cover `compile_callable_var_updates_owner_without_overwriting_shadowing_parameter`, `compile_callable_var_does_not_treat_parameter_as_an_owner`, `compile_inline_foreach_rhs_sees_outer_owner_with_different_parameter_name`, `foreach_reassignment_updates_existing_caller_variable_but_new_locals_stay_local`, and `failed_callable_reassignment_is_atomic_and_keeps_the_inner_span`. Definition-capture mutation and broader context/library sharing remain deferred. |

The inventory deliberately keeps native bounded support separate from
partially implemented family claims. In particular, `.ifpresent` and
`.takeif` use first-class `@lambda` values or headerless indented callback
bodies through the existing callable path; ordinary content is not reparsed or
silently reclassified as a lambda.

The `.container` entry is a partial bounded consumer rather than complete
upstream support: empty/body-only containers, `width`, `height`, `fullwidth`,
structured grouping, and the existing origin-aware Size/Boolean conversion
boundary are implemented. The remaining style/layout parameters are deferred.

## Inventory

| Family | v2.5.1 public surface and evidence | Scribium status and semantic gap | Test/conformance evidence | Milestone and order |
|---|---|---|---|---|
| Function declarations and calls | Dot-prefixed calls with positional, named, mixed, nested, block, inline, and chained forms; `.function` declares document functions. Public evidence: [function-call syntax](https://quarkdown.com/wiki/syntax-of-a-function-call/), [declaring functions](https://quarkdown.com/wiki/declaring-functions/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt). | **Partially compatible.** The grammar, source-backed spans, user-function binding, value-context calls, caller-visible variable/function overlay, and the reviewed scalar/collection slices work. The complete v2.5.1 stdlib/component call surface is not implemented; unresolved ordinary calls are preserved and unresolved chain segments fail closed with `E3001`. | `compile_user_functions_*`, `compile_captured_callable_*`, `compile_nested_callable_*`, and existing CLI chain failure tests. | M2 foundation; maintain as the dependency of each later bounded family. |
| Variables and assignment | `.var {name} {value}`, parameterless access, `.name {newvalue}`, block variables, and scoped variables through `.let`. Evidence: [variables](https://quarkdown.com/wiki/variables/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt). | **Partially compatible.** Document-scope scalar/block values, source-order reassignment, typed value flow, scoped `.let`, caller-visible lookup shadowing, and R11 caller-visible owner reassignment through callable/iteration scopes are implemented. Complete DynamicValue conversion, definition-capture mutation, and all host/context sharing behavior are not. | `compile_variable_*`, `compile_let_*`, `compile_captured_callable_uses_definition_fallback_and_caller_shadowing`, and R11 evaluator owner/atomicity tests. | M2 bounded scope slice; broader mutable-scope and context parity remain deferred. |
| Conditionals | `.if` and `.ifnot` lazily evaluate a boolean condition and body; condition results can be composed from logical functions. Evidence: [conditional statements](https://quarkdown.com/wiki/conditional-statements/), v2.5.1 [`ConditionalTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/ConditionalTest.kt). | **Partially compatible.** Literal/variable conditions, lazy nested calls, and the selected logical/comparison expressions are implemented. Truthiness, arbitrary conversions, and all upstream function-produced condition values are not universally supported. | `compile_logical_comparisons_*`; `qd251_logical_comparison_expression_remains_structural_and_source_backed`; existing conditional laziness tests. | M2; the bounded logical/comparison predicate slice is complete, while other condition-producing families remain separately evidenced. |
| Lambdas and callables | First-class typed lambdas, explicit or implicit `.1`, `.2`, … parameters, optional parameters, lexical parent context, `.let`, callback arguments, and lambda invocation. Evidence: [lambda](https://quarkdown.com/wiki/lambda/), [`Lambda.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt), [`LambdaTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/LambdaTest.kt). | **Partially compatible.** Typed `IrValue::Callable`, immutable definition capture, caller-visible variable/function/lambda overlay, explicit/implicit parameter precedence, child scopes, and the reviewed callback forms work. General lambda consumers, component semantics, and all callback-typed stdlib functions remain unsupported. | `compile_implicit_lambda_*`, `compile_nested_implicit_parameters_use_the_nearest_available_binding`, `compile_explicit_lambda_parameters_mask_outer_implicit_parameters`, `first_class_callable_*`, and transform tests; frontend `@lambda` span tests. | M2 bounded callable foundation and caller overlay; component and generalized callback work is M3+. |
| Iteration | `.foreach` maps an iterable into an ordered collection; `.repeat` is one-based `.foreach {1..N}`. Upstream also accepts positional inline callable bodies with explicit or implicit parameters. Evidence: [loops](https://quarkdown.com/wiki/loops/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt), [`FlowTest.kt`](https://raw.githubusercontent.com/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt), [`LambdaGrammar.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/walker/lambda/LambdaGrammar.kt). | **Partially compatible.** Typed block and inline forms cover explicit/implicit parameters, closed/left-open ranges, ordered results, Pair destructuring, child isolation, semantic-owner writeback, source-defined native shadowing in direct and chained calls, failure atomicity, and explicit evaluator materialization/depth limits. Inline support is restricted to `.foreach {iterable} {body}` and `.repeat {count} {body}` callable positions; the `Value::InlineBody`/`IrValue::InlineBody` carrier is preserved until native resolution and then uses the shared `IrCallable` path. Endless-range consumers, generalized patterns, and the full stdlib consumer set remain gaps. | `compile_foreach_*`, `compile_repeat_*`, `compile_inline_foreach_*`, `compile_source_defined_foreach_and_repeat_shadow_native_direct_and_chain`, `compile_inline_foreach_failure_is_atomic_and_source_backed`, contextual parser tests, evaluator resource-boundary tests, and iterable conformance tests. | M2 reviewed block/inline slice completed at `3829d847`; generalized patterns, unrelated inline bodies, and generalized inline component/callback bodies remain deferred. |
| Collection and Iterable operations | `.getat`, `.first`, `.second`, `.third`, `.last`, `.size`, `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, `.groupvalues`; Pair, Dictionary, and Range adapt as iterables. Evidence: [`Collection.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt), [`IterableTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/IterableTest.kt). | **Partially compatible.** The reviewed typed operations and ordering/absence behavior are implemented. Upstream `.sorted` accepts an optional one-element `by` selector and uses stable natural-key ordering; Scribium covers that selector boundary. Table-specific operations and unrelated collection APIs remain missing. Scribium also exposes `.map` and `.filter` as explicit extensions; they are not counted as v2.5.1 compatibility. | `collection_*`, `compile_collection_*`, and transform tests in `crates/scribium-core`; extension behavior is documented separately in `docs/SYNTAX.md`. | M2 bounded selector/collection slice; table operations and generalized consumers remain deferred. |
| Dictionary, Pair, and Range values | `.pair`, `.dictionary`, one-based Pair/Dictionary entry access, ordered dictionary entries, last-write-wins keys, literal `A..B`/`..B`/`A..`/`..`, and dynamic `.range`. Evidence: [dictionary](https://quarkdown.com/wiki/dictionary/), [range](https://quarkdown.com/wiki/range/), [`DictionaryValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt), [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt). | **Partially compatible.** Recursive typed values, ordered iteration, access, finite range behavior, and atomic construction work. Nested/general destructuring, mutation, and direct materialization of every value shape are intentionally limited. | Pair/dictionary/range tests in `crates/scribium-core/src/lib.rs` and `evaluator.rs`; frontend range-span tests. | M2 reviewed slice; do not expand into generalized patterns without architecture review. |
| Type and value conversion | Dynamic typing adapts a **DynamicValue-origin argument at invocation time** to `String`, `Number`, `Boolean`, `Range`, `Size`, `Color`, closed enums, Markdown content, collections, and other public value types. A statically materialized `StringValue` does not enter that converter for unrelated targets; it only participates in its own String/InlineMarkdownContent boundaries. Evidence: [typing](https://quarkdown.com/wiki/typing/), [`RegularArgumentsBinder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt), [`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt), [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [`ColorDecoder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/misc/color/decoder/ColorDecoder.kt), [`ColorDecoderUtils.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/misc/color/decoder/ColorDecoderUtils.kt). | **Partially compatible.** The scalar slice remains bounded and the production domain dispatcher implements `IrSize` (exact `px`/`pt`/`cm`/`mm`/`in`/`em`/`%` decimal grammar), `IrColor` (ordered Hex/RGB/RGBA/HSV-HSL/Named decoding), and closed enum conversion consumed by `.doctype` and Stacked alignment binders. Typed domain values use identity; dynamic text is parsed only within the reviewed bounds; static String results do not gain unrelated Size/Int/enum meaning. Context-sensitive Markdown conversion and a universal DynamicValue framework remain gaps. | `crates/scribium-engine/src/value_conversion.rs` domain and Stacked conversion fixtures; `.doctype` tests; `crates/scribium-core/tests/quarkdown_stacked_layout.rs`; evaluator value/materialization tests. | M2 domain adapters and the reviewed Stacked consumer implemented; other Size/Color consumers and remaining conversion surfaces remain deferred. |
| String and text operations | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, `.startswith`, `.plaintext`. Evidence: [`Strings.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt), [`NodeUtils.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/node/NodeUtils.kt), [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [typing](https://quarkdown.com/wiki/typing/). | **Partially compatible.** The scalar string family remains bounded. `.plaintext` now performs the evidenced structural projection for already-parsed `IrValue::Content` inline trees and the oracle-equivalent Identifier/Number/Boolean boundary. Emphasis, strong, strikethrough, code, and link labels recurse; soft breaks emit a newline; hard breaks and images emit nothing. Unresolved inline calls, `String`, `None`, collections, ranges, pairs, dictionaries, and callables fail closed. Dynamic String → InlineMarkdownContent conversion that would require upstream reparsing remains unsupported by design. | `scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_plaintext_keeps_existing_scalar_and_content_argument_classification`; `scribium-engine/src/builtins.rs::tests::plaintext_*`; `scribium-engine/src/evaluator.rs` shared value-context evaluation; `scribium-core/src/lib.rs::compile_v251_plaintext_fixture_projects_evaluated_inline_content`, `compile_plaintext_rejects_unsupported_values_atomically`; `scribium-test-support/src/lib.rs::tests::test_verify_plaintext_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/plaintext-family/input.qd`. | M2; bounded already-parsed inline projection implemented. General DynamicValue conversion and String → InlineMarkdownContent reparsing remain compatibility debt. |
| Target-specific HTML content | `.html` accepts one `content: String`; inline and isolated block forms are documented. v2.5.1 evidence: [`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L59-L90), [`Html` node](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/base/block/Html.kt#L1-L14), [HTML documentation](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/docs/html.qd#L43-L58). | **Implemented for the closed Html semantic slice.** The evaluator checks explicit `NativeContent` (default granted; host/API denial emits one source-backed `E3004`), evaluates the single String boundary, preserves source-backed block/inline `TargetSpecificContent`, and Typst/PDF omit it silently. A future HTML output backend consumes the payload; `scribium-html` remains only the Markdown/foreign-HTML normalization boundary. `.css` and `.htmloptions` remain unsupported, and ordinary mixed `.qd`/`.scrib` raw HTML remains `E8001`. | `crates/scribium-markdown/tests/quarkdown_html_contract.rs`; `crates/scribium-core/tests/quarkdown_html_contract.rs`; `crates/scribium-typst-subprocess/tests/backend_integration.rs`; [ADR-0018](../../adr/0018-quarkdown-target-specific-native-content.md). | M2 bounded semantic slice implemented; HTML backend remains future work. |
| Mathematical and numeric operations | `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, `.round`, `.iseven`, and `.range`. Evidence: [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt), [`MathFunctionsTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt), [math](https://quarkdown.com/wiki/math/). | **Compatible at the evidenced numeric boundary.** Typed evaluator paths implement the arithmetic, unary, decimal, and transcendental functions; dynamic/literal `.range` remains covered separately. `.logn`, `.sin`, `.cos`, and `.tan` adapt through the shared `numeric_argument()` Float boundary, then use pinned pure-Rust `libm` binary64 software functions on the adapted Float and narrow to Float. `.pi` preserves the upstream binary64 `PI` constant and does not pass through Float normalization. `.truncate` reproduces the upstream Float/Double/toInt/Float boundary, requires strict integral `decimals: Int`, and `.round` reproduces Kotlin ties-to-even followed by Int conversion. `NumberValue`-style integral Float normalization remains the evaluator output boundary. | Arithmetic regression: `crates/scribium-engine/src/builtins.rs::tests::numeric_*`, `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_arithmetic_fixture_preserves_typed_value_flow`. Decimal slice: `decimal_numeric_surface_matches_upstream_boundaries`, `compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`, `compile_numeric_decimal_forms_share_one_semantic_path`, `compile_numeric_decimal_failure_is_atomic_and_source_backed`, `crates/scribium-test-support/src/lib.rs::tests::test_verify_numeric_decimal_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`. Transcendental slice: `transcendental_numeric_surface_matches_upstream_boundaries`, `deterministic_transcendental_math_has_stable_representative_bits`, `compile_v251_numeric_transcendental_fixture_preserves_typed_value_flow`, `compile_numeric_transcendental_failure_is_atomic_and_source_backed`, `crates/scribium-test-support/src/lib.rs::tests::test_verify_numeric_transcendental_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/numeric-transcendental-family/input.qd`. | M2 numeric family slice; `.range` remains a separately evidenced existing semantic path. |
| Logical and comparison operations | `.islower {a} than:{b} orequals:{bool}`, `.isgreater`, `.equals {a} to:{b}`, and `.not {value}`. Evidence: [`Logical.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt), [`Comparison.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Comparison.kt), conditional examples/tests. | **Compatible for the bounded slice implemented here.** Numeric ordering uses upstream `toFloat` comparison and accepts the reviewed numeric scalar text forms; equality preserves typed values with the documented plain-text fallback; negation requires a boolean. Unsupported conversion inputs fail with one source-backed `E3001` and no partial branch output. | `builtins::tests::logical_*`; `compile_logical_comparisons_*`; frontend structural/span test; CLI verification below. | M2 completed bounded logical/comparison slice; future logical expansion remains separately evidenced. |
| Include, read, and data loading | `.include`, `.includeall`, `.read`, `.json`, `.csv`, `.listfiles`, `.filename`, and context sandbox modes. Evidence: [including other files](https://quarkdown.com/wiki/including-other-quarkdown-files/), [`Ecosystem.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Ecosystem.kt), [`Data.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Data.kt). | **Partially compatible.** `.read`, `.json`, and `.include` are implemented over `VirtualProject` only. Paths resolve relative to the current logical source, nested includes change that base to the included source, active include stacks detect cycles, and repeated includes remain valid. Local paths that leave the project, absolute paths, URI schemes, missing resources, and invalid UTF-8 produce structured diagnostics; the evaluator has no host filesystem or network capability. `.includeall`, `.csv`, `.listfiles`, and `.filename` remain deferred. | `crates/scribium-core/tests/quarkdown_resource_builtins.rs` covers local/parent-relative reads, UTF-8 and JSON failures, nested source context, cycles, repeated includes, and in-memory projects. CLI loading is the native filesystem boundary. | M2 bounded resource-backed slice; additional data families require separate upstream evidence and semantic tests. |
| Metadata and document setup | `.doctype`, `.docname`, `.docdescription`, `.docauthor(s)`, `.dockeywords`, `.doclang`, `.theme`, page/paragraph metadata, numbering, and related document state. Evidence: [document metadata](https://quarkdown.com/wiki/document-metadata/), [localization](https://quarkdown.com/wiki/localization/), [`Document.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt), [`DocumentInfo.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentInfo.kt), [`LocaleLoader.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/LocaleLoader.kt), [`RegularArgumentsBinder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt), [`FunctionCallRefiner.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt). | **Partially compatible with an implemented bounded foundation.** `.docname`, `.docdescription`, `.doctype`, `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, and `.theme` implement bounded document-state semantics through evaluator-owned shared state and a final immutable IR snapshot. `.doclang` follows the upstream general case-insensitive English full-name or IETF BCP 47 tag lookup contract; its checked-in deterministic table covers the public ten locales plus pinned `ko`, `en-US`, and `fr-CA`, returns localized names, and stores canonical tag/name data. Valid identifiers outside that table are an explicit bounded compatibility gap; unsupported or malformed identifiers fail. `.theme` accepts two positional or named nullable `String` parameters, maps `.none` to null, lowercases supplied strings, and replaces the complete theme on each call; an empty call commits an explicit empty theme. Upstream regular block bodies fall back to final `layout` as raw `DynamicValue` text, but Scribium defers that fallback for `.theme` and `.doclang` because the current frontend/IR exposes parsed nodes rather than lossless raw body text and rejects those bodies before evaluation. The stdlib registration hook loads `/lib/localization.qd` and invokes `.localization name:{std}` before user calls, so the stdlib-ready `std` localization table is seeded; localization table mutation and `.localize` lookup remain canonical #151-owned `UNSUPPORTED` rows, not #152 semantics. Hyphenation, locale-aware rendering, theme resolution/defaults, rendering, front-matter merging, numbering, and observable layout metadata remain deferred. | `crates/scribium-core/src/lib.rs::tests::document_state_*`, `doclang_*`, `theme_*`; `crates/scribium-engine/src/locale.rs`; `crates/scribium-ir/src/lib.rs::tests::document_state_roundtrips_deterministically_and_defaults_for_old_ir`; independent `docauthor-family`, `docauthors-family`, `dockeywords-family`, `doclang-family`, and `theme-document-state` fixtures. | DocumentState foundation: implemented; `.docname`, `.docdescription`, `.doctype`, `.docauthor`, `.docauthors`, `.dockeywords`, bounded `.doclang`, and bounded `.theme` at evidenced evaluator/IR boundaries: implemented. Valid BCP 47/name identifiers outside the checked-in locale table, upstream block-body fallback, localization/translation, hyphenation, locale-aware rendering, remaining document fields, theme resolution, and rendering policy are deferred. |
| Layout and document functions | `.row`, `.column`, `.grid`, `.center`, `.container`, `.align`, `.landscape`, `.box`, `.figure`, page breaks, tables, and related layout primitives. Evidence: [`Layout.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Layout.kt), [`Primitives.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Primitives.kt). | **Partially compatible for bounded block-body consumers.** `.row`, `.column`, and `.grid` bind typed arguments; `.center` and `.align` preserve their existing bounded Container behavior; `.container` supports an optional body, `width`, `height`, and `fullwidth`, with structured grouping and origin-aware Size/Boolean conversion; `.landscape` requires a Markdown block body and produces a typed Landscape component with a 90-degree counter-clockwise semantic. All implemented consumers validate before lazy body evaluation, materialize typed nodes, and lower through the Typst backend. The remaining Container style/layout parameters remain deferred. | `crates/scribium-core/tests/quarkdown_stacked_layout.rs`; `crates/scribium-core/tests/quarkdown_center.rs`; `crates/scribium-core/tests/quarkdown_align.rs`; `crates/scribium-core/tests/quarkdown_container.rs`; `crates/scribium-core/tests/quarkdown_landscape.rs`; `crates/scribium-typst-subprocess/tests/backend_integration.rs`; `examples/stacked/main.qd`; `examples/center/main.qd`; `examples/align/main.qd`; `examples/container/main.qd`. | Stacked layout, bounded Container sizing/alignment, and `.landscape` are implemented only for the reviewed block-body boundary. Deferred: `float`, `fullspan`, `classname`, `StyleOptions`, `alignment`/`textalignment` on `.container`, colors, borders, margin/padding/radius, font and text-style properties, general String → Markdown conversion, inline component insertion, and other layout primitives. |
| Inline whitespace | `.whitespace width? height?` is an inline `Whitespace` node. `width` and `height` are optional `Size` values, bind positionally in that order or by name, and do not accept a body. With neither dimension it is NBSP-equivalent; with one or both dimensions it is an empty fixed-size rectangle and an omitted axis is zero. Evidence: v2.5.1 public layout/primitives contract at the pinned upstream target. | **Implemented for the bounded slice.** `IrInline::Whitespace` is backend-neutral, preserves source span and inline order, reuses the existing `InvocationValue`/`ValueOrigin` Size adapter, rejects invalid/duplicate/unknown/body bindings atomically, and lowers to NBSP or an explicit zero-filled Typst `#box`. | `crates/scribium-core/tests/quarkdown_whitespace.rs`; `crates/scribium-typst-subprocess/tests/backend_integration.rs`; `IrInline` serde roundtrip and source-provenance assertions. | M2 bounded inline semantic slice implemented. General inline node/value composition and other inline layout functions remain deferred. |
| Inline hard line break | `.br` is an argumentless inline `LineBreak` producer. It accepts no positional or named arguments and no block or lambda body. v2.5.1 `Text.kt` returns `LineBreak.wrappedAsValue()`, and `NodeUtils.toPlainText()` omits `LineBreak`. | **Implemented for the bounded slice.** The evaluator validates the signature before any invalid body/argument evaluation and materializes one existing backend-neutral `IrInline::HardBreak` with the call span. Surrounding inline order, source-defined `br` precedence, atomic failure, serde, plaintext, and existing Typst hard-break lowering are covered. No generalized inline component/value family is implied. | `crates/scribium-core/tests/quarkdown_br.rs`; `crates/scribium-typst-subprocess/tests/backend_integration.rs`; `fixtures/quarkdown-conformance/cases/br-line-break-family/`; pinned/current-main `Text.kt` and pinned `LineBreak.kt`/`NodeUtils.kt` evidence in `SPEC_SOURCES.md`. | M2 bounded inline semantic slice implemented. `.text`, `.codespan`, `.clip`, `.float`, and `.fullspan` remain separate work. |
| Error and absence behavior | `none`, `.isnone`, `.otherwise`, `.ifpresent`, `.takeif`, invalid argument/type errors, and lazy failure behavior. Evidence: [`Optionality.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt), [conditional statements](https://quarkdown.com/wiki/conditional-statements/). | **Partially compatible; optionality callbacks are implemented at a bounded boundary.** `None`, `.isnone`, `.otherwise`, the `.ifpresent` absence short-circuit, Boolean-only `.takeif` callback execution, source-backed evaluator errors, nested diagnostic de-duplication, atomic results, and contextual unmarked iteration callbacks are covered. General callback-typed stdlib coverage remains outside this slice. | `compile_optionality_*`, `compile_isnone_returns_a_semantic_boolean_for_optional_values`, inline iteration tests, and `fixtures/quarkdown-conformance/cases/optionality-callback-family/input.qd`. | M2 bounded callback slices; retain the family as partial until the remaining error and conversion surfaces are evidenced. |

## Stacked layout and Container consumers

Implemented for this selected slice:

- typed `IrValue::Component` and closed backend-neutral `IrComponent::Stacked`;
- typed row/column/grid layout, alignment, `IrSize` gap, and `Vec<IrNode>`
  children with source provenance;
- deterministic serde roundtrip, including `Grid { columns: NonZeroU32 }`;
- value-context preservation through variables and callable results; and
- `.row`, `.column`, and `.grid` source construction with exact defaults,
  positional/named binding, duplicate/unknown argument diagnostics, origin-aware
  enum/Size/Int conversion, and grid gap precedence;
- lazy block-body evaluation into typed children, nested component composition,
  callable return flow, and failure atomicity;
- typed block materialization as `IrNode::Component` without flattening,
  stringification, silent drop, or partial publication; and
- pure Typst lowering with alignment/gap structure, source maps, and actual
  Typst/PDF integration coverage.

The bounded `.landscape` consumer adds:

- required Markdown block-body validation with no positional or named
  arguments, lazy body evaluation, and lambda/inline fail-closed diagnostics;
- typed `IrComponent::Landscape` children with preserved structure, call/child
  provenance, nested and callable composition, and block materialization; and
- pure Typst `#rotate(-90deg, reflow: true)` lowering. The language semantic is
  counter-clockwise content rotation inside the document flow, not page
  orientation or page-size mutation. Upstream documents this feature as
  experimental.

The bounded direct `.container` consumer adds:

- optional `width` and `height` `IrSize` properties, `full_width`, optional
  logical alignment, and structured children in the existing
  `IrContainerComponent`;
- empty or body-only construction, with body evaluation after argument
  binding, conversion, and validation;
- positional `width`, `height`, `fullwidth` binding plus named equivalents,
  duplicate/unknown/deferred-parameter diagnostics, and origin-aware Size and
  Boolean conversion; and
- deterministic Typst `#block` sizing with explicit width taking precedence
  over `fullwidth`, while `alignment: None` emits no `#align` wrapper.

The bounded `.center` consumer adds a full-width Container with logical
`Center` alignment. The bounded `.align` consumer reuses that Container family
with `start`/`center`/`end` alignment, a dedicated origin-aware closed-enum
carrier/converter, exact positional/named binding, required lazy Markdown block
body evaluation, typed nested composition, and source-backed failure atomicity.
These consumers do not add `StyleOptions`, `.float`, `.fullspan`, general
String → Markdown conversion, or inline Container insertion.

Deferred from this slice:

- general DynamicValue String → Markdown body conversion;
- inline Stacked insertion (Stacked is block-only); and
- the remaining layout/component families and deferred `.container` style
  parameters.

## Selection record

The selected slices are the Stacked layout family (`.row`, `.column`, and
`.grid`), the bounded `.center`/`.align`/`.container` Container consumers, and
the bounded `.landscape` component consumer, integrated with the existing typed
component value foundation and evaluator-owned value/materialization boundary.

This is a bounded public semantic family with direct v2.5.1 source/test
evidence. It fits the existing evaluator/value-flow boundary and needs no new
parser, filesystem capability, IR tier, or backend escape hatch. Direct
`.container` support is intentionally limited to the sizing subset;
`StyleOptions`, `.float`, `.fullspan`, and the remaining style/layout families
remain deferred. `.float` and `.fullspan` are not inferred from
`.landscape`: they have distinct floating and multi-column semantics requiring
separate architecture/backend decisions.
The earlier decimal family remains implemented under the same boundary:
`decimals` uses the same invocation-time DynamicValue Number conversion as
other numeric targets, then accepts only integral NumberValue-compatible
results. Dynamic text `2` and `2.0` are accepted, dynamic `1.5` and static
StringValue text are rejected, and negative accepted Int values fail at
runtime. The evaluator preserves `IrValue::Number` and materializes only
through the existing normal IR-to-Typst path.

The current `.plaintext` slice is a separate bounded builtin adaptation. It
consumes already-parsed inline content directly from `IrValue::Content` after
the shared evaluator has resolved nested calls. It does not create a
plain-text backend, invoke Typst, rewrite source, or reparse `IrValue::String`.
The upstream v2.5.1 runtime probe observed the following projection contract:

- text, code, emphasis, strong, strikethrough, and link labels preserve their
  displayed text;
- soft breaks emit a newline;
- hard breaks and images emit no text; and
- empty inline content emits the empty string.

The unsupported Dynamic String → InlineMarkdownContent conversion remains a
documented gap because upstream may reparse Markdown-bearing strings. The
remaining resource/data loading, metadata functions, and deferred
layout/document primitives are also outside this slice. The reviewed Stacked
consumer is covered separately above. `.sorted` is covered
at the upstream `by` selector boundary; v2.5.1 does not expose an arbitrary
negative/zero/positive comparator parameter.

## Optionality callback evidence

The v2.5.1 `Optionality.kt` contract defines `.none`, `.isnone`, `.otherwise`,
`.ifpresent(value, mapping)`, and `.takeif(value, condition)`. `.ifpresent`
returns `None` without invoking its mapping callback for a `None` value and
otherwise returns the callback's typed result. `.takeif` invokes its condition
for every value, including `None`, requires a Boolean callback result, and
returns the original value or `None`.

Scribium implements this bounded family through `IrValue::None` and the
existing `IrValue::Callable` invocation path. First-class `@lambda` callbacks
and headerless indented callback bodies use fresh child scopes, immutable
captures, nearest-scope implicit parameters, and source-backed diagnostics.
Callback failure is atomic: no optionality result is published, and nested
failure produces one E3001/E3003 diagnostic. The independently authored
fixture is `fixtures/quarkdown-conformance/cases/optionality-callback-family/input.qd`;
compile, lexical capture/shadowing, lazy-none, UTF-8/CRLF provenance, and
failure-atomicity coverage is in `crates/scribium-core/src/lib.rs::tests::compile_optionality_*`.

## Conformance evidence for the current numeric slice

- Frontend preserves `.if {.islower {2} than:{3}}` as a nested directive with
  original UTF-8/CRLF-safe spans:
  `crates/scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_logical_comparison_expression_remains_structural_and_source_backed`.
- Unit evaluation covers strict/inclusive ordering, boolean negation, typed
  equality, plain-text fallback, invalid values, duplicate bindings, and body
  rejection: `crates/scribium-engine/src/builtins.rs::tests::*`.
- Unit evaluation covers the decimal surface, strict integral `decimals`,
  negative/fractional failure, Float/Double/Float truncation order, zero and
  negative values, Kotlin ties-to-even, NaN/Infinity/clamping boundaries,
  named/mixed binding, nested values, invalid conversion, and block-body
  rejection: `crates/scribium-engine/src/builtins.rs::tests::decimal_numeric_surface_matches_upstream_boundaries`.
- The independently authored decimal conformance fixture covers direct,
  positional/named, chained, half-even, negative, zero-decimal, and nested
  arithmetic composition:
  `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`.
- Integration coverage verifies ordinary/named/chain equivalence, typed
  `IrValue::Number` flow, source-backed nested failure, one diagnostic, and no
  partial enclosing output: `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`,
  `compile_numeric_decimal_forms_share_one_semantic_path`, and
  `compile_numeric_decimal_failure_is_atomic_and_source_backed`.
- The independently authored transcendental fixture covers binary64 `.pi`,
  Float-normalized `.logn`/trigonometry, zero and pi composition, nested
  typed values, and a chain into `.logn`:
  `fixtures/quarkdown-conformance/cases/numeric-transcendental-family/input.qd`.
- Transcendental unit coverage fixes representative `to_bits()` results for
  logarithm, sine, cosine, and tangent; checks `ln(0)`, negative-domain NaN,
  infinities, and `sin(-0)`, `cos(-0)`, and `tan(-0)`; and exercises named,
  textual, nested, chained, arity, body, and invalid-structured-input paths:
  `crates/scribium-engine/src/builtins.rs::tests::transcendental_numeric_surface_matches_upstream_boundaries`
  and `deterministic_transcendental_math_has_stable_representative_bits`.
- Integration coverage verifies the rendered oracle cases, typed flow, one
  source-backed `E3001`, and no partial output on nested transcendental
  failure: `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_transcendental_fixture_preserves_typed_value_flow`
  and `compile_numeric_transcendental_failure_is_atomic_and_source_backed`.
- The prior arithmetic/unary regression remains covered by
  `crates/scribium-engine/src/builtins.rs::tests::numeric_*`,
  `compile_v251_numeric_arithmetic_fixture_preserves_typed_value_flow`, and
  `fixtures/quarkdown-conformance/cases/numeric-arithmetic-family/input.qd`.
- Nested numeric failure is source-backed, emits one diagnostic, and publishes
  no partial output: `crates/scribium-core/src/lib.rs::tests::compile_numeric_nested_failure_is_atomic_and_source_backed`.
- Full compile coverage covers nested calls, variables, user functions,
  chains, lazy branches, failure atomicity, source spans, UTF-8, CRLF, and
  deterministic repeated execution: `crates/scribium-core/src/lib.rs::tests::compile_logical_*`.
- The focused backend integration test verifies the output boundary and real
  Typst/PDF compilation; no Typst-specific implementation was added.
- The repository's required `examples/hello` CLI smoke commands remain a
  pre-existing failure because that example references unresolved `show_code`;
  this slice does not alter the example or invent configuration semantics.

The scalar string conformance fixture covers quoted whitespace preservation,
positional/named/mixed binding, conditional concatenation, Unicode and boundary
case behavior, typed chain results, lazy predicate conditionals, and the
source-backed invalid-boolean failure path. The separate `plaintext-family`
fixture covers rich formatting, nested formatting, code spans, link labels,
soft/hard breaks, and evaluated nested calls through the already-parsed IR
boundary. Dynamic String reparsing remains a compatibility gap; the `.equals`
plain-text fallback remains private to equality.
