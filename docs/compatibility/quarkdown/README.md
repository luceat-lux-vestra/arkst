# Quarkdown Compatibility Specification

## Status

- **Specification version:** 0.5 (verified baseline v2.5.1)
- **Reference upstream:** Quarkdown v2.5.1
- **Compatibility target:** complete public-language/document-semantics compatibility
- **Current verified compatibility:** partial; only evidence-backed matrix rows are claims

## Scope

This document defines Scribium's Quarkdown-compatible syntax and semantics.
Each feature records its specification source, compatibility level, and known
divergences.

Scribium's long-term target is complete compatibility with the publicly
documented Quarkdown document language and document-observable semantics of the
tracked stable upstream release. The Feature Matrix records current verified
claims, not a permanent selected language scope: rows marked `Implemented` are
claims only at their stated compatibility level and only with the listed
conformance evidence. `SPEC_SOURCES.md` records upstream provenance. Rows
marked `Planned` or `Not implemented` are explicit compatibility gaps/debt and
must not be treated as supported.

The current implementation is partial. A feature being documented upstream is
not evidence that Scribium supports it, while a feature not yet implemented is
not thereby outside the long-term language target. The tracked target and
verified baseline are distinct; see [Upstream Evolution](#upstream-evolution).

“Full compatibility” means public document-language behavior and
document-observable semantics for the tracked release. It does not require
Quarkdown implementation identity, private APIs, undocumented bugs, internal
data structures, private plugin ABI, or internal compiler architecture.

The Quarkdown function-call grammar is implemented clean-room from the public
documentation, notably *"Syntax of a function call"* on the Quarkdown wiki.
No Quarkdown source code is copied or translated. See `SPEC_SOURCES.md` for
provenance records.

The remaining public-language surface is tracked in the
[`GAP_INVENTORY.md`](GAP_INVENTORY.md). It records upstream evidence, Scribium
status, semantic gaps, conformance evidence, and recommended order for
subsequent bounded slices; it replaces an opaque remaining-M2 list.

## Feature Matrix

| Feature                        | Syntax                           | Compatibility            | Status           |
|--------------------------------|----------------------------------|--------------------------|------------------|
| Dot-prefixed call              | `.note`                          | Parsed                   | Implemented      |
| Implicit positional refs       | `.1`, `.2`, ... in a headerless callable body | Semantically supported for the evidenced slice | Implemented (evidenced slice) |
| Positional arguments           | `.range {1} {10}`                | Parsed                   | Implemented      |
| Named arguments                | `.panel width:{320}`             | Parsed                   | Implemented      |
| Mixed positional/named         | `.panel {Intro} width:{320}`     | Parsed                   | Implemented      |
| Indented body argument         | `.panel {x}` + indent            | Parsed                   | Implemented      |
| Nested calls                   | `.outer {.inner {x}}`            | Parsed                   | Implemented      |
| Inline (mid-paragraph) call    | `see .note {x}`                  | Parsed                   | Implemented      |
| Tight-call boundaries          | word adjacency rejected          | Parsed                   | Implemented      |
| Malformed-call diagnostics     | `E2001`, `E2002`, `E2003`, `E2004` | Error                  | Implemented      |
| Variables                      | `.var {name} {value}`, `.name`, `.name {value}`, `.if {.name}` | Semantically supported | Implemented      |
| Conditionals                   | `.if {cond}` / `.ifnot {cond}`, including selected logical expressions | Semantically supported for literals, variables, and the logical/comparison slice | Implemented (evidenced slice) |
| Logical/comparison predicates  | `.islower`, `.isgreater`, `.equals`, `.not` | Typed boolean results, numeric ordering, plain-text equality fallback, lazy conditional use | Implemented (bounded v2.5.1 slice) |
| Mathematical/numeric operations | `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.truncate`, `.round`, `.iseven`, plus `.range` | Typed numeric/boolean results with shared binding, strict `decimals: Int` adaptation, upstream Float/Double/Float truncation, Kotlin ties-to-even rounding, and the evidenced scalar numeric boundary; `.logn`, `.pi`, `.sin`, `.cos`, and `.tan` remain deferred | Implemented (bounded v2.5.1 decimal post-processing slice) |
| String/text operations         | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, `.startswith`, `.plaintext` | Typed scalar string results and boolean predicates for the evidenced scalar/plain-text adaptation boundary; rich Markdown plain-text projection remains deferred | Implemented (bounded v2.5.1 slice) |
| User-defined functions         | `.function {name}`, explicit/implicit parameter modes, optional `parameter?`, positional/named calls, block-last binding | Semantically supported for the evidenced slice | Implemented (evidenced slice) |
| Scoped `.let` evaluation        | block explicit one-parameter or headerless `.1` lambda | Semantically supported for the evidenced slice | Implemented (block form) |
| Optional parameter values      | omitted `parameter?` → `None`, `.otherwise`, `.isnone` | Semantically supported for the evidenced slice | Implemented (evidenced slice) |
| Iteration                      | typed `Range` / `Collection` / `Pair` / ordered `Dictionary`; block `.foreach` and `.repeat` | Semantically supported for typed values, closed inclusive ranges, left-open ranges starting at 1, descending-empty behavior, ordered list adaptation, ordered dictionary entries, block explicit/implicit lambdas, Pair destructuring, typed collection results, parent visibility, and child isolation | Implemented (evidenced slice; right-open/fully-open iterable rejection and generalized patterns deferred) |
| Collection access              | `.size`, `.first`, `.second`, `.third`, `.last`, `.getat` | Typed access over `Collection`, `Pair`, ordered `Dictionary` entries, finite closed or left-open `Range`, and Markdown list values; one-based access with upstream absence/fallback behavior | Implemented (evidenced slice) |
| Collection operations          | `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, `.groupvalues` | Shared typed iterable materialization, upstream `asDouble()` aggregation, stable first-occurrence distinctness, reverse order, and nested first-seen groups | Implemented (evidenced v2.5.1 slice) |
| Generic callable and transforms | `@lambda ...`, contextual `by:{...}`, `.foreach`, `.map`, `.filter`, `.sorted` | Typed callable values, shared child-scope invocation, recursive results, and shared iterable adaptation; `.foreach` and `.sorted` are native compatibility evidence, while `.map`/`.filter` are Scribium extensions excluded from conformance counts | Implemented (bounded callable/native-transform slice) |
| Functions/components            | —                                | —                        | Planned          |
| Include/read                   | —                                | —                        | Planned          |
| Metadata                       | —                                | —                        | Planned          |
| Row/column/grid                | —                                | —                        | Planned          |
| Semantic evaluation            | `.if`/`.ifnot` + variables + user-defined functions + block `.let` + evidenced chain builtins | Partial / In progress | Implemented (partial) |
| Call chaining (`::`)           | `.a {x}::b {y}` and documented nested equivalent `.b {.a {x}} {y}` | Semantically supported for the evidenced scalar builtins, including `.otherwise` and `.isnone`; chain and nested forms share value-context invocation, with strict left-to-right flow and source-backed `E3001` failures for unimplemented callees | Implemented (evidenced slice) |
| Line continuation (`\`)        | `\` at end of line               | Parsed                   | Implemented      |
| Tight / brace-wrapped calls    | `H{.text {2}}O`                  | Parsed                   | Implemented      |
| Multi-line arguments           | `{.…}` parsing spans lines        | Parsed                   | Implemented      |
| `.json` data loading           | `.json {path}` (new in v2.5.0)   | Not implemented          | Planned          |
| `.markdown` / `.llmstxt`       | (new in v2.5.0)                  | Not implemented          | Planned          |

The v2.5.1 Markdown deltas are recorded in
[`V2_5_1_IMPACT.md`](V2_5_1_IMPACT.md). D2 link-parenthesis behavior and D3
deep four-space list behavior are tested at the Markdown frontend boundary;
the tests cover both ordinary Markdown and Quarkdown-mode inputs, including a
Quarkdown directive body. These rows are evidence for the tested frontend
behavior only and do not imply full Quarkdown compatibility.
D2 correction evidence additionally covers empty and whitespace-empty inline
destinations, exact angle/title/multiline-link spans, and link-kind isolation:
inline destinations receive the tested escape normalization while Auto,
Reference, and Image destination representations remain unchanged.

Issue #57 adds a separate end-to-end Markdown evidence slice for structures
already preserved by the frontend: blockquotes, single- and double-tilde
strikethrough, GFM task lists, and GFM tables. The slice carries recursive
content, task state, table
alignment, source spans, evaluator recursion, and deterministic Typst
lowering through `.md`, `.qd`, and an indented Quarkdown body. It is evidence
for the tested structures and forms only; it does not promote images, raw HTML,
or complete CommonMark/GFM support.

For an indented body, the minimum eligibility rule is at least two leading
spaces or one leading tab in the current Rushdown container context. The first
qualifying nonblank line establishes the actual body indentation; later lines
must meet that same container-relative indentation and a dedent ends the body.
The frontend preserves this parser decision for lazy paragraph normalization,
so body ownership is not re-inferred from absolute source columns or a fixed
indentation width. The evidence above covers 2/3/4/8-space bodies, one-space
rejection, single-tab and mixed indentation, UTF-8/CRLF provenance, nested
Markdown and Quarkdown, and list/blockquote-relative containers.

## Issue #61 structured iterable semantics

The evaluator now represents `Pair` and `Dictionary` as typed recursive
values. `.pair {first} {second}` creates a `Pair`; `.dictionary` consumes a
Markdown list body and creates an ordered deterministic `Dictionary` whose keys
are strings and whose values remain typed. Duplicate keys use the observed
Quarkdown v2.5.1 last-write-wins rule while retaining the first insertion slot.

Dictionary iteration uses the existing `.foreach` iteration engine and yields
`Pair(key, value)` items in dictionary order. The upstream block header syntax
is `key value:` (not a parenthesized pattern). One explicit parameter binds the
whole Pair; exactly two explicit parameters destructure a Pair into the two
same-child-scope bindings. Explicit lambda scope remains a hard boundary for
implicit `.1`, and each iteration gets a fresh `EvaluationContext::child()`, so
inner bindings mask and then restore outer bindings without leakage.

Pair and Dictionary construction/evaluation is atomic: a failed child or
dictionary entry produces a source-backed evaluator diagnostic, stops further
iteration/evaluation, and does not publish a partial structured value. Direct
document output is materialized by the evaluator as an ordered list for Pair
and a two-column table for Dictionary; Typst lowering does not implement these
language semantics.

This slice intentionally does not add nested or generalized destructuring,
rest/spread patterns, mutation, comparator-language syntax, descending sorting,
or transform forms beyond the shared first-class `by` callback. `.map`,
`.filter`, and `.sorted` now use the generic callable and iterable machinery
described below; `.filter` is Boolean-only and the requested `.map`/`.filter`
surface is not asserted as an upstream v2.5.1 compatibility claim because the
tracked public `Collection.kt` source does not define those functions.

## Collection and Iterable operations evidence

The v2.5.1 public [Iterable](https://quarkdown.com/wiki/iterable/) contract
defines `Collection` as an ordered list value, `Pair` as an iterable of two
values, `Dictionary` as an iterable of key-value pairs, and finite integer
`Range` as an ordered iterable. The public standard-library API and v2.5.1
source were consulted for the public Collection operations:

- [Collection API index](https://quarkdown.com/docs/quarkdown-stdlib/com.quarkdown.stdlib.module.Collection/)
  lists the operation signatures and chaining contract.
- [`Collection.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt)
  establishes `.size` as a non-negative count, one-based `.first`/`.second`/
  `.third`/`.last` access, `.getat` with optional `orelse`, `asDouble()`-based
  `.sumall`/`.average`, equality-based `.distinct`, reverse materialization,
  and nested `.groupvalues` results. It contains no public generic `.map` or
  `.filter` declaration.
- [`Types.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Types.kt)
  shows that `Value.asDouble()` parses String values when possible and falls
  back to `0.0` for other non-numeric values.
- [`IterableTest.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/IterableTest.kt)
  verifies sum, average, distinct, reversed, and group-values behavior,
  including first-seen group order and per-group input order.
- [`IterableValue.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/IterableValue.kt),
  [`DictionaryValue.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt),
  and [`Range.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt)
  establish Pair iteration, dictionary-entry adaptation, and finite Range
  iteration behavior.

The exact implemented policy is:

- `.size` accepts one iterable operand (`of:` in ordinary form) and returns a
  typed `Number`; empty values return `0`.
- `.first` and `.last` accept one iterable operand (`from:` in ordinary form),
  return the original typed element, and return semantic `None` for an empty
  value.
- `.getat` accepts an iterable, a finite integral numeric index, and optional
  `orelse:`. Indices are one-based. Zero, negative, very large, and
  out-of-range integral indices return semantic `None` or the typed fallback;
  fractional, non-finite, and non-numeric indices fail with a source-backed
  evaluator diagnostic. No truncating or saturating conversion is used.
- `.second` and `.third` return semantic `None` when the shared materialized
  sequence is too short, matching `.getat {2}` and `.getat {3}` without a
  fallback.
- `.sumall` sums every `asDouble()` conversion; invalid conversions contribute
  zero, while `.average` divides by the full input count and returns `NaN` for
  an empty input. `.distinct` preserves the first occurrence, `.reversed`
  returns a new collection, and `.groupvalues` returns groups in first-seen
  order with original order inside each group.
- Dictionary access observes deterministic ordered `Pair` entries. Pair
  access observes its two components. Closed Ranges are inclusive, descending
  Ranges are empty, and a left-open Range defaults its start to `1`. A
  right-open Range representation is supported, but standard Iterable
  consumption rejects it as endless; fully-open Ranges are rejected the same
  way. Strings and unsupported scalar values are not treated as iterables.

Scribium obtains ordered semantic elements through the same evaluator
adaptation used by `.foreach`, `.sorted`, `.map`, and `.filter`. Results remain
recursive `IrValue`s, so a
Dictionary access returns a Pair that can continue through Pair operations or
feed the existing `.foreach` destructuring path. Nested operand failures remain
atomic and propagate their original diagnostic without a duplicate. The
compile/evaluator evidence is listed in the conformance table below, including
UTF-8 and CRLF source-span coverage.

## Generic callable, native transforms, and Scribium extensions

The v2.5.1 lambda evidence identifies a lambda as a first-class typed value
with explicit named parameters or implicit `.1`, `.2`, and later positional
references. Invocation forks a child scope, preserves lexical parent bindings,
fills omitted optional parameters with `None`, and validates explicit arity.
Scribium stores the body and source spans in `IrValue::Callable`, snapshots
captured bindings deterministically, and routes `.foreach`, `.map`, `.filter`,
`.sorted` selectors, and first-class callback values through one invocation
path. The same path preserves nearest-scope implicit masking and child-scope
isolation.

The explicit first-class form is source-backed `@lambda`, for example:

```text
.var {identity} {@lambda .1}
.map {1..3} by:{.identity}
.map {1..3} by:{value: .value}
```

Transform callback arguments also accept the contextual upstream form without
the legacy marker when they occur as the `by` argument of `.map`, `.filter`, or
`.sorted`. Other content arguments remain ordinary content and are not
reclassified as lambdas.

All transforms first evaluate one value through `coerce_iterable`, then invoke
the callback against each typed element. `.map` returns a typed `Collection` in
source order. `.filter` requires a semantic Boolean predicate result and
returns the original typed elements in source order. `.sorted` accepts natural
keys or a `by` selector and returns a stable ascending typed `Collection`;
Scribium rejects heterogeneous, `None`, and unsupported keys with diagnostics.
There is no descending option or arbitrary comparator language.

The v2.5.1 `Collection.kt` source documents `.sorted(from, by?)` but does not
define public `.map` or `.filter` functions in the tracked tag. `.foreach`
does return an ordered collection with one result per input element, so its
block form has map-equivalent semantics and is included in the native evidence
slice. Consequently `.sorted` and `.foreach` are evidenced Quarkdown
v2.5.1-compatible operations, while `.map` and `.filter` remain Scribium
extensions and are excluded from conformance coverage. Unknown upstream
details remain deferred.
Pair, Dictionary, Range, and supported Markdown-list transforms reuse the exact
`.foreach` element sequence and Range policy. Callback failures, invalid
predicates, unsupported sort keys, and endless ranges publish no partial
result; no value is serialized or reparsed.

## Range construction and iterable semantics

Literal Range syntax is restricted to non-negative integer endpoints and keeps
the four endpoint shapes typed:

```text
A..B
..B
A..
..
```

Literal endpoints are syntax, not dynamically evaluated expressions. They are
converted to the signed core `IrRange` endpoint domain with checked conversion;
the v2.5.1 black-box boundary behavior for an oversized literal endpoint is
preserved as an open endpoint rather than a wrapped integer. The exact
`2147483647` endpoint remains representable, while `2147483648` does not wrap.

Dynamic `.range` constructs the same `IrValue::Range(IrRange)` value. Both
`from` and `to` are optional and accept normal positional, named, and valid
mixed argument binding:

```text
.range {A} {B}
.range from:{A} to:{B}
.range to:{B}
.range from:{A}
.range
```

Bounds are evaluated through the ordinary evaluator. A semantic `Number` is
converted with the observed upstream `Number.toInt()` behavior: truncation
toward zero, NaN to `0`, and clamping outside the signed `Int` domain to
`i32::MIN` or `i32::MAX`. Non-numeric bounds and invalid argument shapes fail
with one source-backed evaluator diagnostic; child failures are propagated
without an additional Range diagnostic.

The shared `coerce_iterable` path is used by `.foreach`, `.size`, `.first`,
`.last`, and `.getat`:

- closed `A..B` ranges iterate inclusively from `A` through `B`;
- descending `A..B` ranges are empty;
- left-open `..B` ranges use `1..B` and are empty when `B < 1`;
- right-open `A..` ranges remain valid typed values but standard Iterable
  consumption fails as endless; and
- fully-open `..` ranges remain valid typed values but standard Iterable
  consumption also fails as endless.

No arbitrary finite upper bound is introduced. Signed cardinality is checked
before `usize` conversion and fallible reservation, and a failed materialization
publishes no partial Collection or document output. Range values remain
backend-neutral; Typst does not interpret Range semantics.

`Implemented` rows are current claims only at their stated compatibility level
and are covered by the listed unit/golden/conformance evidence (see
[Conformance Evidence](#conformance-evidence)). `Planned` means the behavior is
not implemented yet, in whole or in part. It must not be assumed to work, and
its absence is tracked compatibility debt against the complete target.

## Conformance Evidence

Each `Implemented` row is backed by at least one Scribium conformance test.
The table maps every `Implemented` feature to the test(s) that verify it;
Quarkdown grammar evidence lives in
`crates/scribium-quarkdown/src/lib.rs`, while frontend integration evidence
lives in `crates/scribium-markdown/src/parser.rs` and its integration tests. A
single test may cover multiple features. This table is the
implementation-evidence counterpart of the upstream provenance recorded in
`SPEC_SOURCES.md`; the two are kept separate on purpose.

| Feature                         | Evidence (unit tests) |
|---------------------------------|------------------------|
| Dot-prefixed call               | `scribium-quarkdown/src/lib.rs::empty_and_plain_text_are_not_calls`, `scribium-quarkdown/src/lib.rs::parses_normal_call_names_and_spans`, `scribium-markdown/src/parser.rs::qd_mode_preserves_nested_body_and_utf8_spans` |
| Implicit positional refs        | `scribium-quarkdown/src/lib.rs::parses_implicit_positional_references_and_boundaries`, `implicit_references_do_not_consume_arguments`, `braced_implicit_reference_is_not_classified_as_a_decimal`; `scribium-core/src/lib.rs::compile_implicit_lambda_parameters_use_the_shared_callable_path`, `compile_implicit_parameters_preserve_typed_values`, `compile_implicit_parameter_content_keeps_markdown_structure`, `compile_implicit_lambda_scopes_are_nested_and_reusable`, `compile_implicit_parameter_missing_and_zero_argument_are_diagnostics`, `compile_implicit_parameter_diagnostic_preserves_utf8_and_crlf_span` |
| Positional arguments            | `scribium-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments`, `scribium-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification` |
| Named arguments                 | `scribium-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments` |
| Mixed positional/named          | `scribium-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments` |
| Indented body argument          | `scribium-markdown/src/parser.rs::quarkdown_body_uses_first_body_line_indent_not_fixed_width`, `quarkdown_body_rejects_one_space`, `quarkdown_body_tab_preserves_text_and_utf8_spans`, `quarkdown_body_dedent_terminates_body_and_shallower_lines_are_not_absorbed`, `quarkdown_body_preserves_nested_markdown`, `quarkdown_body_preserves_nested_quarkdown_blocks`, `quarkdown_body_is_container_relative_in_lists_and_blockquotes`, `quarkdown_body_blank_lines_preserve_body_lifecycle` |
| Nested calls                    | `scribium-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification`, `scribium-markdown/src/parser.rs::nested_content_calls_keep_prefix_suffix_and_original_spans` |
| Inline (mid-paragraph) call     | `scribium-markdown/src/parser.rs::nested_content_calls_keep_prefix_suffix_and_original_spans` |
| Tight-call boundaries           | `scribium-quarkdown/src/lib.rs::tight_word_adjacency_and_symbol_boundaries_are_explicit`, `scribium-quarkdown/src/lib.rs::parses_implicit_positional_references_and_boundaries` |
| Malformed-call diagnostics      | `scribium-quarkdown/src/lib.rs::rejects_malformed_and_ordered_arguments_without_panicking`, `scribium-markdown/src/parser.rs::malformed_root_block_reports_argument_span`, `scribium-markdown/src/parser.rs::malformed_inline_call_preserves_full_source_offset` |
| v2.5.1 link parentheses         | `scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_links_accept_balanced_escaped_and_nested_parentheses`, `qd251_unbalanced_plain_destination_stays_literal`, `qd251_trailing_parenthesis_and_surrounding_text_are_not_swallowed`, `qd251_links_preserve_utf8_and_crlf_source_boundaries`, `qd251_link_boundary_is_identical_in_md_qd_and_qd_body_modes`, `qd251_link_correction_empty_destinations_have_complete_spans`, `qd251_link_correction_preserves_angle_and_title_forms`, `qd251_link_correction_preserves_multiline_title_span`, `qd251_link_correction_preserves_autolink_backslashes_and_email_semantics`, `qd251_link_correction_preserves_reference_and_image_destinations`, `qd251_link_correction_preserves_utf8_and_crlf_edge_spans` |
| v2.5.1 deep four-space lists   | `scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_deep_four_space_lists_have_exact_depth_in_md_and_qd`, `qd251_deep_list_preserves_siblings_dedent_and_following_content`, `qd251_nested_paragraph_and_list_content_remain_in_their_items`, `qd251_deep_lists_preserve_utf8_and_crlf_spans`, `qd251_qd_body_uses_dynamic_indent_before_markdown_list_parsing` |
| M2 blockquotes / strikethrough / task lists / tables | `scribium-markdown/src/parser.rs::preserved_markdown_structures_keep_nested_semantics_and_source_spans`, `scribium-core/src/ast_to_ir.rs::convert_structures_preserves_task_table_and_nested_spans`, `scribium-core/src/evaluator.rs::structures_recurse_through_evaluator_without_losing_semantics`, `scribium-typst/src/lowering.rs::lower_structured_markdown_nodes_preserves_semantics_and_source_map`, `scribium-typst/tests/backend_integration.rs::integration_markdown_structures_compile_to_valid_pdf` |
| v2.5.1 call syntax slice | `scribium-quarkdown/src/lib.rs::parses_multiline_nested_arguments_with_original_spans`, `parses_line_continuations_without_fixed_indentation`, `parses_chains_as_source_backed_segments_without_rewriting`, `parses_tight_calls_and_preserves_inner_provenance`, `rejects_malformed_chains_deterministically`; `scribium-markdown/src/parser.rs::qd_multiline_arguments_and_continuations_keep_header_body_boundary`, `qd_inline_continuation_and_tight_calls_preserve_text_and_spans`; `scribium-core/src/ast_to_ir.rs::preserve_call_chain_segments_and_provenance_in_ir`, `scribium-core/src/lib.rs::compile_evaluates_block_and_inline_chain_value_flow`, `compile_evaluates_chain_inside_a_content_argument`, `compile_chain_and_nested_call_are_semantically_equivalent`, `compile_variable_values_keep_types_across_chain_and_nested_forms`, `compile_numeric_variable_reassignment_preserves_numeric_value_context`, `compile_chain_and_ordinary_conditional_are_equally_lazy`, `compile_reports_unimplemented_chain_callees_with_specific_spans`, `compile_reports_chain_failures_in_inline_and_content_paths`; `scribium-core/src/evaluator.rs::nested_call_and_chain_share_the_same_value_context`, `nested_and_chained_case_transforms_share_dynamic_scalar_adaptation`, `variable_values_remain_semantic_through_nested_and_chained_calls`, `chain_value_flow_is_left_to_right_and_injects_first`, `chain_preserves_explicit_positional_arguments_after_previous_value`, `chain_keeps_named_arguments_named_while_injecting_previous_value`, `false_final_conditional_chain_does_not_evaluate_its_body`, `false_final_inline_conditional_chain_does_not_evaluate_its_body`, `child_scope_inherits_parent_and_isolates_local_bindings`; `scribium-cli/src/commands.rs::unimplemented_chain_callee_fails_before_typst_or_pdf_output`; `scribium-typst/tests/backend_integration.rs::integration_chain_evaluation_reaches_typst_and_pdf`; `fixtures/markdown/quarkdown_v251_syntax.qd` syntax/provenance fixture |
| Conditionals                   | `evaluator.rs::if_true_keeps_block_body`, `evaluator.rs::if_false_drops_block_body`, `evaluator.rs::ifnot_true_drops_and_ifnot_false_keeps`, `evaluator.rs::boolean_identifiers_yes_no_true_false_case_insensitive`, `evaluator.rs::missing_condition_reports_e3001_and_drops`, `evaluator.rs::unresolvable_condition_reports_diagnostic`, `evaluator.rs::nested_if_inside_block_body_is_evaluated`, `evaluator.rs::content_value_second_argument_replaces_call`, `evaluator.rs::scalar_second_argument_becomes_text`, `evaluator.rs::inline_if_replaces_call_with_inline_body_or_content`, `evaluator.rs::inline_if_false_drops_call`, `evaluator.rs::inline_call_scalar_second_argument_becomes_text`, `evaluator.rs::non_conditional_calls_are_preserved_with_evaluated_bodies`, `evaluator.rs::named_condition_argument_works`, `evaluator.rs::named_condition_false_drops_body`, `evaluator.rs::named_condition_ifnot_inverts`, `evaluator.rs::named_condition_identifier_yes_no`, `evaluator.rs::named_body_argument_works`, `evaluator.rs::named_body_scalar_argument_works`, `evaluator.rs::block_body_priority_over_named_body`, `evaluator.rs::inline_named_condition_works`, `evaluator.rs::inline_named_body_works`, `evaluator.rs::named_condition_unresolvable_reports_e3001`, `lib.rs::compile_evaluates_if_true`, `lib.rs::compile_evaluates_if_false`, `lib.rs::compile_evaluates_ifnot`, `lib.rs::compile_evaluates_nested_if`, `lib.rs::compile_reports_e3001_for_unresolvable_condition`, `lib.rs::compile_evaluates_named_condition_true`, `lib.rs::compile_evaluates_named_condition_false`, `lib.rs::compile_evaluates_named_condition_yes_no`, `lib.rs::compile_evaluates_named_body`, `lib.rs::compile_evaluates_named_condition_and_body`, `lib.rs::compile_inline_named_condition`, `typst::conditional_evaluation_before_lowering` |
| String/text operations | `scribium-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification`; `scribium-core/src/builtins.rs::tests::string_surface_is_registered_and_returns_typed_values`, `string_operations_bind_named_arguments_and_defaults`, `string_case_and_empty_operations_cover_unicode_and_boundaries`, `string_operations_reject_unsupported_values_and_invalid_bindings`; `scribium-core/src/lib.rs::compile_v251_string_scalar_fixture_preserves_typed_value_flow`, `compile_string_predicates_feed_lazy_conditionals_without_text_materialization`, `compile_string_predicate_failure_is_atomic_and_source_backed`; `scribium-test-support/src/lib.rs::tests::test_verify_string_scalar_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/string-scalar-family/input.qd` | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, and `.startswith` preserve typed evaluator results, share positional/named binding and scalar string adaptation, support ordinary/nested/chained forms, and fail closed for unsupported values. `.plaintext` and general DynamicValue conversion remain deferred. | Implemented (bounded v2.5.1 slice) |
| Logical/comparison predicates | `scribium-core/src/builtins.rs::tests::logical_surface_is_registered_and_evaluates_typed_results`, `equality_preserves_types_and_uses_upstream_plain_text_fallback`, `logical_builtins_reject_invalid_values_and_duplicate_bindings`; `scribium-core/src/lib.rs::compile_logical_comparisons_drive_conditionals_and_nested_calls`, `compile_logical_comparisons_work_in_user_functions_and_chains`, `compile_logical_comparison_failure_is_atomic_and_source_backed`, `compile_logical_comparison_execution_is_deterministic_for_utf8_crlf`; `scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_logical_comparison_expression_remains_structural_and_source_backed`; `scribium-typst/tests/backend_integration.rs::integration_logical_comparison_evaluation_reaches_typst_and_pdf` | `.islower`, `.isgreater`, `.equals`, and `.not` return typed booleans, preserve the value boundary, support lazy conditional use, and fail closed on invalid input | Implemented (bounded v2.5.1 slice) |
| User-defined functions         | `scribium-quarkdown/src/lib.rs::parses_contextual_lambda_headers_with_exact_spans`, `lambda_header_parser_is_contextual_and_rejects_malformed_headers`; `scribium-markdown/src/parser.rs::function_body_uses_contextual_source_backed_lambda_header`, `ordinary_non_lambda_body_with_colon_is_not_stripped`; `scribium-core/src/lib.rs::compile_user_functions_support_zero_and_required_parameters`, `compile_implicit_lambda_parameters_use_the_shared_callable_path`, `compile_implicit_parameters_preserve_typed_values`, `compile_implicit_lambda_scopes_are_nested_and_reusable`, `compile_user_functions_keep_scalar_values_for_nested_and_chain_calls`, `compile_user_function_rich_and_block_results_keep_markdown_structure`, `compile_user_functions_use_source_order_and_override_builtins`, `compile_user_functions_bind_block_last_and_isolate_child_scope`, `compile_user_function_argument_failures_are_single_and_body_is_not_run`, `compile_user_function_no_value_and_failed_nested_calls_keep_original_diagnostic`, `compile_optional_user_parameters_bind_missing_positional_and_named_values`, `compile_optional_final_parameter_accepts_missing_or_block_content_and_keeps_collision`, `optional_parameter_spans_survive_utf8_and_crlf_frontend_to_ir_conversion` |
| Scoped `.let`                | `scribium-markdown/src/parser.rs::let_explicit_lambda_header_is_source_backed_and_stripped`, `let_implicit_lambda_body_keeps_implicit_reference`, `let_header_utf8_span_is_exact_for_crlf_source`, `let_nested_container_span_keeps_original_body_ranges`; `scribium-core/src/ast_to_ir.rs::let_lambda_metadata_survives_ast_to_ir_with_original_spans`, `let_implicit_lambda_metadata_is_absent_in_ir`; `scribium-core/src/evaluator.rs::let_explicit_parameter_returns_scalar`, `let_implicit_parameter_returns_scalar`, `let_shadows_parent_and_local_variables_do_not_leak`, `nested_let_uses_nearest_implicit_scope`; `scribium-core/src/lib.rs::compile_let_supports_explicit_and_implicit_block_lambdas`, `compile_let_isolates_local_variables_and_functions` |
| Iteration                    | `scribium-quarkdown/src/lib.rs::parses_typed_ranges_without_confusing_numbers_or_references`; `scribium-markdown/src/parser.rs::iteration_lambda_headers_are_contextual_and_source_backed`; `scribium-core/src/ast_to_ir.rs::range_survives_ast_to_ir_as_a_typed_source_backed_value`, `literal_range_endpoint_conversion_is_checked_at_the_signed_boundary`; `scribium-core/src/ir.rs::range_and_nested_collection_roundtrip_serde`, `pair_and_dictionary_roundtrip_serde_preserves_recursive_values`; `scribium-core/src/evaluator.rs::dynamic_range_returns_typed_signed_truncated_endpoints`, `dynamic_range_number_conversion_matches_upstream_edges`, `range_materialization_handles_signed_and_left_open_bounds_once`, `pair_evaluation_is_typed_recursive_and_atomic_on_child_failure`, `dictionary_iteration_reuses_pair_items_and_explicit_destructuring`, `pair_destructuring_rejects_non_pair_items_without_coercion`; `scribium-core/src/lib.rs::compile_foreach_closed_range_is_inclusive_and_preserves_numbers`, `compile_dynamic_range_converges_with_literal_and_supports_signed_bounds`, `compile_dynamic_range_supports_nested_bounds_and_typed_interoperability`, `compile_foreach_returns_a_typed_collection_that_can_be_stored_and_consumed`, `compile_foreach_reads_parent_values_and_functions_with_isolated_children`, `compile_foreach_adapts_only_list_values_and_preserves_nested_collections`, `compile_foreach_scopes_implicit_parameters_at_the_nearest_boundary`, `compile_dictionary_foreach_destructures_ordered_pairs`, `compile_dictionary_duplicate_keys_are_last_write_wins_in_first_slot`, `compile_dictionary_entry_failure_is_atomic_and_stops_before_output`, `compile_dictionary_implicit_scope_keeps_the_pair_typed`, `compile_dictionary_explicit_scope_masks_implicit_positional_references`, `compile_dictionary_destructuring_masks_and_restores_parent_bindings`, `compile_nested_dictionary_destructuring_restores_outer_scope`, `compile_pair_is_a_typed_recursive_value_at_the_output_boundary`, `compile_repeat_is_one_based_and_uses_the_shared_collection_result`, `compile_repeat_zero_and_descending_ranges_are_empty_per_upstream_evidence`, `compile_iteration_accepts_left_open_and_rejects_endless_ranges`, `compile_dynamic_range_rejects_invalid_shapes_and_preserves_atomic_failures`, `compile_dynamic_range_diagnostics_keep_utf8_crlf_and_nested_bound_spans`, `compile_iteration_body_no_value_and_failure_are_single_diagnostics` | Semantically supported for typed literal/dynamic values, signed endpoint conversion, closed and left-open iterable ranges, descending-empty behavior, ordered list adaptation, ordered dictionary entries, Pair destructuring, block explicit/implicit lambdas, typed collection results, parent visibility, and child isolation | Implemented (evidenced slice; endless right-open/fully-open consumption and generalized patterns deferred) |
| Collection and Iterable operations | `scribium-core/src/evaluator.rs::collection_second_and_third_share_one_based_iterable_access`, `collection_distinct_and_groupvalues_are_stable_and_typed`, `collection_reversed_uses_the_shared_materialized_sequence`, `collection_sumall_and_average_follow_as_double_and_kotlin_average`, `collection_access_reuses_failure_outcomes_and_checks_length_conversion`; `scribium-core/src/lib.rs::compile_collection_api_parity_uses_frontend_lists_and_shared_iterables`, `compile_collection_access_keeps_pair_dictionary_and_range_values_typed`, `compile_collection_access_diagnostics_keep_utf8_and_crlf_source_spans` | `.size`, `.first`, `.second`, `.third`, `.last`, `.getat`, `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, and `.groupvalues` over the shared typed `Collection`, Pair, ordered Dictionary entries, closed/left-open Range, and Markdown-list adaptation path; recursive typed results, stable ordering, aggregation conversion, and atomic failures | Implemented (evidenced v2.5.1 slice) |
| Generic callable, native transforms, and extensions | `scribium-quarkdown/src/lib.rs::parses_marked_inline_lambdas_without_rewriting_source`, `parses_marked_inline_implicit_lambdas`; `scribium-markdown/src/parser.rs::marked_inline_lambda_is_structural_and_source_backed`, `transform_callback_lambda_uses_contextual_unmarked_form`; `scribium-core/src/evaluator.rs::collection_transforms_share_typed_iterable_and_callable_paths`, `transforms_support_pair_dictionary_and_nested_typed_values`, `sorted_supports_typed_keys_and_fails_closed_for_unsupported_keys`, `transform_failures_are_atomic_and_predicates_are_boolean_only`, `first_class_callable_captures_definition_values_and_checks_arity`; `scribium-core/src/lib.rs::compile_collection_transforms_through_frontend_and_first_class_lambda_values` | First-class typed callable values, explicit/implicit callback binding, lexical capture, shared invocation and iterable adaptation, typed `.foreach`/`.sorted` results, and retained typed `.map`/`.filter` extensions. `.foreach` and `.sorted` are native evidence; `.map`/`.filter` are excluded from upstream v2.5.1 conformance counts | Implemented (bounded callable/native-transform evidence; extensions retained) |
| Optional parameter values      | `scribium-core/src/ir.rs::none_uses_the_stable_externally_tagged_serde_variant`, `scribium-core/src/lib.rs::compile_optional_parameters_support_otherwise_and_preserve_value_types`, `compile_optional_none_is_distinct_from_no_value`, `compile_optional_none_can_be_stored_locally_without_parent_scope_leak`, `compile_optional_none_direct_output_materializes_as_text`, `compile_isnone_returns_a_semantic_boolean_for_optional_values` |
| Variables                      | `evaluator.rs::var_scalar_definition_and_reference`, `evaluator.rs::var_boolean_reference_in_conditional`, `evaluator.rs::var_false_boolean_drops_conditional`, `evaluator.rs::var_ifnot_with_variable`, `evaluator.rs::var_explicit_reassignment`, `evaluator.rs::var_variable_name_reassignment`, `evaluator.rs::var_reassignment_produces_no_output`, `evaluator.rs::var_inline_use`, `evaluator.rs::var_block_variable`, `evaluator.rs::var_conditional_declaration_execution_order`, `evaluator.rs::var_unknown_call_preserved`, `evaluator.rs::var_malformed_declaration_reports_e3002`, `evaluator.rs::var_nested_evaluation_in_block_variable`, `evaluator.rs::var_evaluation_immutable_and_deterministic`, `lib.rs::compile_variable_declaration_and_reference`, `lib.rs::compile_variable_boolean_in_conditional`, `lib.rs::compile_variable_false_conditional`, `lib.rs::compile_variable_ifnot`, `lib.rs::compile_variable_explicit_reassignment`, `lib.rs::compile_variable_name_reassignment`, `lib.rs::compile_variable_inline_use`, `lib.rs::compile_variable_block_variable`, `lib.rs::compile_variable_conditional_declaration`, `lib.rs::compile_variable_unknown_preserved`, `lib.rs::compile_variable_malformed_reports_e3002`, `lib.rs::compile_variable_nested_in_block`, `lib.rs::compile_variable_immutable_and_deterministic` |

### v2.5.1 syntax-gap evidence

The v2.5.1 public function-call syntax review is backed by independently
authored fixtures in the grammar and frontend tests. The evidence covers
multiline nested positional/named arguments, line continuation with arbitrary
leading indentation, parser-preserved `::` chains, tight brace-wrapped calls,
normal boundary regressions, malformed recovery, UTF-8, CRLF, `.md`/`.qd`
isolation, and the existing dynamic body-indentation lifecycle.

The syntax adapter preserves the head, each chain segment, each name span,
argument spans, and the complete source span without synthetic reparsing. The
evaluator consumes those segments structurally and applies the documented
left-to-right value-flow transformation for the four evidenced builtin
callees. Their documented nested-call equivalents share the same evaluator
invocation contract and are covered by paired semantic and generated-Typst
tests. Successful terminal outputless calls (such as variable reassignment)
remain legal, but a no-value result in a nested value-required argument or
non-final chain segment reports source-backed `E3001`; an already-failed child
propagates its original diagnostic without a duplicate no-value error.
Value-context type preservation, the small documented scalar adaptation
surface, lazy conditional bodies, provenance, failure, and Typst/PDF tests
support this slice only; complete `DynamicValue` and general programmable
document compatibility are not claimed here.

The public source for this slice is the Quarkdown wiki's [Syntax of a
function call](https://quarkdown.com/wiki/syntax-of-a-function-call/) page,
which documents multiline arguments, line continuation, chaining, and tight
function calls. Fixtures are independently authored from that public contract;
no upstream implementation source, test, or fixture was used.

## Compatibility Levels

### User-defined function evidence

This slice is grounded in the public Quarkdown documentation for
[declaring functions](https://quarkdown.com/wiki/declaring-functions/),
[lambdas](https://quarkdown.com/wiki/lambda/),
[function-call syntax](https://quarkdown.com/wiki/syntax-of-a-function-call/),
[variables](https://quarkdown.com/wiki/variables/), and
[typing](https://quarkdown.com/wiki/typing/). Those pages document
`.function`, the `to from:` and `to from?:` parameter headers, positional and named calls,
block content as the final parameter, source-order redeclaration, and the
absence of an explicit return statement (reviewed 2026-08-15). Scribium
independently represents
the header and parameter spans, binds required parameters in a child scope,
and preserves scalar or structured-content results through the shared value
evaluator.

The v2.5.1 [lambda reference](https://quarkdown.com/wiki/lambda/) explicitly
defines a headerless lambda's positional parameters as `.1`, `.2`, `.3`, and
so on, and states that lambdas fork nested scopes. The official
[v2.5.1 release](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1)
was also probed as a black box: an out-of-range implicit reference and a
reference in a zero-argument lambda fail as unresolved references. Scribium
preserves that observable failure class as a deterministic source-backed
`E3003`; it never substitutes `None` or evaluator `NoValue`.

The claim is deliberately limited to required and optional explicit parameters,
headerless implicit positional references, the tested scalar/content shapes,
and the small `.otherwise`/`.isnone` builtin surface. Scoped `.let` is
supported for block-form explicit one-parameter and headerless `.1` lambdas,
including nested lexical scopes, parent lookup, child isolation, and semantic
result propagation. First-class `@lambda` values and contextual transform
callbacks now use the same invocation machinery. Components and complete
DynamicValue compatibility remain compatibility debt.

### Typed iteration evidence

This first iteration slice is grounded in the public
[Loops](https://quarkdown.com/wiki/loops/),
[Iterable](https://quarkdown.com/wiki/iterable/),
[Range](https://quarkdown.com/wiki/range/),
[Lambda](https://quarkdown.com/wiki/lambda/), and
[foreach API](https://quarkdown.com/docs/quarkdown-stdlib/com.quarkdown.stdlib.module.Flow/foreach.html)
references. They establish that `.foreach` maps an iterable through a scoped
lambda and returns an ordered collection, `.repeat` is the one-based
`.foreach {1..times}` shorthand, Markdown lists are iterable values, and Range
syntax preserves open endpoints.

The official v2.5.1 implementation was consulted only as public behavioral
evidence, never copied or translated: [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt)
delegates `.repeat` to `forEach(Range(1, times), body)`, and [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt)
iterates closed ranges inclusively, starts a left-open range at `1`, rejects a
right-open range as endless, and uses the host integer range behavior for
descending bounds. The public v2.5.1 [`FlowTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt)
also covers `..4` and rejects `1..`. [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
documents dynamic `.range` and floating-point truncation. Scribium follows this
policy through one shared iterable adaptation path.

Supported here are typed literal and dynamic Range values, signed dynamic
endpoints, recursive ordered Collections, closed and left-open Range iteration,
descending-empty behavior, Markdown ordered/unordered list adaptation at the
iterable boundary, block-form `.foreach` with one explicit parameter or
implicit `.1`, block-form `.repeat`, typed mapped results, parent lookup, fresh
per-iteration child scopes, and the four evidenced Collection access
operations. The Collection slice also covers `.second`, `.third`, `.sumall`,
`.average`, `.distinct`, `.reversed`, and `.groupvalues` through that same
typed materialization path. `.foreach` and `.sorted` are native v2.5.1
evidence; the retained `.map`/`.filter` surface is explicitly a Scribium
extension and is excluded from conformance claims. Deferred are generalized or
nested destructuring, comparator-language syntax, descending sorting, and
table-specific collection operations. Right-open and fully-open Range values
are represented but are rejected by the standard finite Iterable path as
endless.

- **Unsupported:** Syntax may be parsed and preserved, but normal compilation
  produces an explicit `E8xxx` error diagnostic for the unsupported semantics
  (see `compatibility/diagnostics.rs`)
- **Error:** Produces an explicit parse diagnostic (`E2xxx`) at the call site
- **Parsed:** Accepted syntactically; behavior may be undefined or rejected
- **Semantically supported:** Scribium semantics match documented behavior
- **Output-equivalent:** Typst output matches reference for tested inputs
- **Known divergence:** Deliberate behavioral difference with documented
  rationale

Function calls are currently **Parsed** except for the evidenced semantic
surfaces below: `.name`, positional arguments `{arg}`, named arguments
`name:{arg}`, nested calls, and indented block bodies are parsed into the
Scribium AST/IR. Multiline braced arguments, line continuations, and tight
brace-wrapped calls are syntax-supported with source-backed spans. The
evidenced `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`,
`.negate`, `.sqrt`, `.truncate`, `.round`, `.iseven`, `.string`, `.concatenate`, `.uppercase`,
`.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, `.startswith`,
`.islower`, `.isgreater`, `.equals`, and `.not` chain forms and their documented
nested-call equivalents are **Semantically supported** with strict
left-to-right value flow; an unimplemented chain callee reports a source-backed
`E3001` evaluation error. The string-family and comparison builtins' small
scalar adaptation contracts are evidenced, not complete DynamicValue
compatibility.
**User-defined functions are also semantically supported for the evidenced
slice**: headerless implicit and required/optional explicit-parameter
declarations, positional/named binding where applicable, block-last-parameter
binding, child scope, source-order redeclaration, builtin override, and
scalar/structured-content results. Missing optional parameters bind semantic
`None`, which is consumed by `.otherwise` and `.isnone` without string
conversion in value context. Headerless `.1`/`.2` references are 1-based,
invocation-local, and preserve typed `IrValue`s; missing indices produce a
source-backed `E3003` diagnostic. **Conditional evaluation (`.if` / `.ifnot`)
with boolean literals, variable references (`.if {.name}`), and the selected
logical/comparison expressions is implemented**. The comparison family is
documented in [`GAP_INVENTORY.md`](GAP_INVENTORY.md) and uses typed boolean
results rather than text reparsing.
Standalone lambda values, components, and complete programmable-document
compatibility remain unimplemented. Typed block iteration is limited to the
evidenced first slice above. A matrix row can
therefore represent only the evidenced
forms at its stated level; an input form that currently fails to parse (for
example with an `E2xxx` diagnostic) is a compatibility gap, not evidence of
support for that form. `Unsupported` is reserved for the explicit compatibility
diagnostic state.

### Logical and comparison evidence

The v2.5.1 public [`Logical.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt)
surface defines numeric `.islower` and `.isgreater` with `than` and optional
`orequals`, value `.equals` with `to`, and boolean `.not`. The public
[`Comparison.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Comparison.kt)
helper shows that equality first compares values and then compares a plain-text
projection for strings, numbers, and Markdown content. The v2.5.1 conditional
tests exercise `.islower` directly inside `.if`, including the lazy branch
behavior documented by the [conditional statements](https://quarkdown.com/wiki/conditional-statements/)
page.

Scribium implements this family as typed evaluator builtins. Numeric ordering
uses the upstream float comparison boundary and accepts the reviewed scalar
numeric text forms; `.equals` preserves typed equality and applies only the
documented plain-text fallback; `.not` accepts boolean values and boolean
literals. Invalid values, duplicate bindings, and unsupported bodies produce a
single source-backed `E3001`; the conditional body is not evaluated and no
partial result is published. The selected family is not a claim that all
DynamicValue conversions or other logical helpers are complete.

### Mathematical and numeric evidence

The v2.5.1 [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
source defines the arithmetic/unary functions plus `.truncate(x,
decimals: Int)` and `.round(x)` over `Number` values. The decimal slice has
the following observable boundaries:

- `.truncate` rejects negative `decimals` at runtime; fractional `decimals`
  fail the `Int` argument binding rather than being silently truncated.
- `decimals == 0` uses `x.toInt()`. Otherwise, non-integral `x` uses
  `x.toFloat() * 10.0.pow(decimals)` as Double arithmetic, `Double.toInt()`,
  and Float division by `multiplier.toFloat()` in that order.
- `.round` preserves an upstream Int, otherwise applies Kotlin
  `kotlin.math.round(x.toFloat())` (ties-to-even) and then `toInt()`.
- `NumberValue` normalizes integral Float values, including clamped
  non-finite conversions, to Int. This is why the final NaN/Infinity behavior
  must be checked after rounding or conversion, not from `round()` alone.

The existing arithmetic path retains the v2.5.1 `toFloat()` boundary;
`.pow` and `.iseven` apply `Number.toInt()`, division-by-zero results clamp to
the upstream Int boundaries when integral, `0/0` and negative square roots
remain `NaN`, and remainder keeps signed floating behavior.

[`NumberValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NumberValue.kt),
[`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt),
and [`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt)
establish the invocation-time numeric and normalization boundaries. Scribium
reuses its existing narrow scalar adaptation and adds only the local strict
integer-compatible `decimals` adapter; it does not introduce a general
DynamicValue conversion framework. All numeric functions use the existing
argument binder, preserve `IrValue::Number` or `IrValue::Boolean`, and reject
unsupported values, unknown/duplicate bindings, arity errors, and block bodies
without publishing partial nested output.

The independent unit and integration evidence is:
`scribium-core/src/builtins.rs::tests::decimal_numeric_surface_matches_upstream_boundaries`,
`scribium-core/src/lib.rs::tests::compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`,
`compile_numeric_decimal_forms_share_one_semantic_path`,
`compile_numeric_decimal_failure_is_atomic_and_source_backed`,
`scribium-test-support/src/lib.rs::tests::test_verify_numeric_decimal_family_is_semantically_supported`,
and `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`.
The arithmetic/unary regression remains covered by the existing numeric tests
and `numeric-arithmetic-family` fixture. The remaining `.logn`, `.pi`, `.sin`,
`.cos`, and `.tan` functions are deliberately outside this slice.

### String and text evidence

The v2.5.1 public [`Strings.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt)
surface defines scalar `.string`, `.concatenate`, case transforms,
emptiness predicates, and `.startswith`. Scribium implements the first eight
of those functions through one explicit invocation-boundary adapter for
strings, identifiers, numbers, booleans, and bounded plain-text content. The
results remain typed `IrValue::String` or `IrValue::Boolean`, so nested calls,
chains, variable bindings, and lazy conditionals share the ordinary evaluator
path.

Quoted scalar input is classified by the existing Quarkdown grammar, which
removes only its outer quotes and preserves inner whitespace before the typed
IR boundary. `None`, collections, and rich structured content are rejected
instead of being stringified. `.plaintext` is intentionally not included: it
is a separate rich Markdown-content projection boundary, and the private
`.equals` plain-text fallback is not promoted into a general conversion helper.

### Tight-call boundaries

A normal call requires a boundary before and after it: whitespace, a symbol
(including `-`), or the start/end of the line. A call directly adjacent to a
word character — any Unicode letter or digit, plus `_` — is not recognized and
the whole construct stays ordinary text. Examples:

- `.note {x}` is a call; `.note {x}B` and `한.note {x}` are not (both
  Unicode and ASCII letters count as word characters).
- `-.note` and `.note-` are valid calls: `-` is a symbol, not a word
  character.

The brace-wrapped form (`H{.text {2}}O`) lifts the boundary requirement. The
frontend accepts a complete wrapper, consumes the wrapper from output syntax,
and keeps both the wrapper and inner-call provenance. An incomplete wrapper
recovers as ordinary text.

### Existing public-language compatibility debt

Quarkdown has documented features represented in the v2.5.0/v2.5.1 evidence set that
Scribium has not implemented yet. They are listed in the Feature Matrix as
`Planned`, are **not** current compatibility claims, and remain compatibility
debt against the complete target. Standalone lambda values outside the
supported first-class/callback forms, layout semantics, resource/data loading,
and other v2.5.0 built-ins remain additional gaps; generalized or nested
destructuring, comparator-language syntax, descending sorting, and unrelated
collection operations remain deferred within the iteration slice. Right-open and
fully-open Range values are not globally unsupported: their representation is
supported, while standard Iterable consumption rejects them as endless. The
evidenced function row does not promote later consumer-specific Range surfaces.

## Specification Record Format

Each implemented compatibility feature records its public documentation
source, an independently authored input example, and the observed behavior.

```yaml
feature: dot-prefixed-call
specification_source: |
  Quarkdown wiki, "Syntax of a function call":
  https://quarkdown.com/wiki/syntax-of-a-function-call/ (v2.5.0 badge)
independently_authored_input: |
  .heading level:{1}
      Title
  .strong {bold text}
observed_reference_behavior: |
  Dot-prefixed names form function calls; each argument is wrapped in
  curly braces; named arguments use name:{value}; indented lines after
  a block call form its body. The current v2.5.0 documentation describes the
  same basic dot-prefixed, brace-argument model on which Scribium's existing
  parser subset is based.
scribium_behavior: |
  Parses dot calls, positional/named arguments, nested calls, and
  indented bodies into the shared DirectiveCall AST.
compatibility_level: Parsed
known_divergence: null
```

The `independently_authored_input` is written from the public syntax
specification only; it is not copied from Quarkdown sources, examples, or
tests (clean-room policy, see `docs/adr/0007-quarkdown-compatibility-scope-and-clean-room-process.md`).

## Provenance

The call grammar was derived from the public documentation *"Syntax of a
function call"* (wiki, badged `2.5.0`, accessed 2026-08-08). The current
v2.5.0 documentation describes the same basic dot-prefixed, brace-argument
model on which Scribium's currently evidenced parser behavior is based.
Scribium's previous
compatibility baseline was 0.9.x, but no claim is made that the upstream
grammar was verified to be identical across every version in between.
`SPEC_SOURCES.md` documents the source list, per-source version badges, and
accessed dates.

## Known Divergences

- (None yet for the currently implemented call-syntax rows)
- Scope note: the matrix is an evidence register, not a permanent language
  boundary. Rows marked **Planned** are *not* implemented and must not be
  claimed; any public Quarkdown behavior absent from the matrix is still a gap
  to investigate against the complete target.
- **Block variable evaluation timing:** Scribium evaluates block variable
  content at declaration time (source order). The cited Quarkdown public
  documentation does not explicitly specify evaluation timing for stored
  block content. This behavior may be refined if upstream semantics are
  clarified. See `docs/SYNTAX.md` for details.

## Upstream Evolution

Scribium tracks two distinct Quarkdown versions:

| Concept | Description | Authority |
|---------|-------------|-----------|
| **Tracked upstream target** | The latest stable Quarkdown release. It automatically becomes the release Scribium must investigate and adapt toward. | Stable-release observer |
| **Verified compatibility baseline** | The release for which permitted evidence, independent fixtures, implementation, regression/conformance tests, and known-divergence records are complete. The existing `supported_baseline` manifest field names this value. | Human-reviewed promotion PR |

The observer (`.github/workflows/upstream-quarkdown.yml`) runs daily, obtains
the latest stable release, and compares it with the verified baseline:

- If they match → no target/baseline lag is detected.
- If they differ → `drift` status and a deduplicated adaptation issue.

A new stable release is never an optional product-adoption question. The issue
asks what changed and what work is required to restore verified compatibility.
The current observer is only the early foundation of the intended pipeline:

```text
release detection
    -> permitted public evidence and release-note delta
    -> structured impact report
    -> independently authored conformance updates
    -> implementation/adaptation PR
    -> conformance and regression verification
    -> review gate
    -> verified baseline promotion
```

The observer does not yet implement this complete pipeline. Future automation
may prepare evidence, fixtures, impact reports, adaptation PRs, validation, and
baseline-promotion changes, but must stop for architecture review when a change
requires new ownership, dependency direction, public abstractions, semantic/IR
redesign, security capabilities, a permanent divergence, weakened invariants,
generic plugins, or backend escape hatches. Human review and merge remain the
authority boundary.

The verified baseline advances only after:

1. permitted public specification/release evidence is reviewed;
2. affected behavior is identified;
3. independent conformance cases exist;
4. required implementation changes are complete;
5. the relevant regression and conformance suites pass; and
6. known divergences are documented and reviewed.

See `docs/adr/0016-full-quarkdown-compatibility-and-upstream-evolution.md` and
`docs/adr/0013-upstream-compatibility-observation-and-baseline-promotion.md`.

## Outside the language-compatibility target

The complete target concerns the public document language and observable
document semantics. The following are implementation/product surfaces rather
than public language claims:

- Quarkdown interactive slide runtime
- Quarkdown internal plugin ABI
- Quarkdown-specific CSS themes
- Quarkdown HTML post-processing
- Quarkdown line click interactivity

These exclusions do not create a general escape hatch for publicly documented
language features. If a public-language behavior is deliberately divergent, it
requires the rationale, compatibility documentation, appropriate diagnostics,
and an ADR when substantial.

New v2.5.0 builtins (data loading via `.json`, `.markdown`, `.llmstxt`,
stdlib `foreach`/iterables) are tracked as `Planned` above; they do not belong
to the non-language exclusions above. As features are implemented, their matrix
status and evidence are promoted; until then they remain explicit gaps against
the complete target.
