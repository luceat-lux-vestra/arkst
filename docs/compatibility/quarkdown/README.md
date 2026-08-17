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
| Conditionals                   | `.if {cond}` / `.ifnot {cond}` | Semantically supported | Implemented      |
| User-defined functions         | `.function {name}`, explicit/implicit parameter modes, optional `parameter?`, positional/named calls, block-last binding | Semantically supported for the evidenced slice | Implemented (evidenced slice) |
| Scoped `.let` evaluation        | block explicit one-parameter or headerless `.1` lambda | Semantically supported for the evidenced slice | Implemented (block form; inline lambda values deferred) |
| Optional parameter values      | omitted `parameter?` → `None`, `.otherwise`, `.isnone` | Semantically supported for the evidenced slice | Implemented (evidenced slice) |
| Iteration                      | —                                | —                        | Planned          |
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
| User-defined functions         | `scribium-quarkdown/src/lib.rs::parses_contextual_lambda_headers_with_exact_spans`, `lambda_header_parser_is_contextual_and_rejects_malformed_headers`; `scribium-markdown/src/parser.rs::function_body_uses_contextual_source_backed_lambda_header`, `ordinary_non_lambda_body_with_colon_is_not_stripped`; `scribium-core/src/lib.rs::compile_user_functions_support_zero_and_required_parameters`, `compile_implicit_lambda_parameters_use_the_shared_callable_path`, `compile_implicit_parameters_preserve_typed_values`, `compile_implicit_lambda_scopes_are_nested_and_reusable`, `compile_user_functions_keep_scalar_values_for_nested_and_chain_calls`, `compile_user_function_rich_and_block_results_keep_markdown_structure`, `compile_user_functions_use_source_order_and_override_builtins`, `compile_user_functions_bind_block_last_and_isolate_child_scope`, `compile_user_function_argument_failures_are_single_and_body_is_not_run`, `compile_user_function_no_value_and_failed_nested_calls_keep_original_diagnostic`, `compile_optional_user_parameters_bind_missing_positional_and_named_values`, `compile_optional_final_parameter_accepts_missing_or_block_content_and_keeps_collision`, `optional_parameter_spans_survive_utf8_and_crlf_frontend_to_ir_conversion` |
| Scoped `.let`                | `scribium-markdown/src/parser.rs::let_explicit_lambda_header_is_source_backed_and_stripped`, `let_implicit_lambda_body_keeps_implicit_reference`, `let_header_utf8_span_is_exact_for_crlf_source`, `let_nested_container_span_keeps_original_body_ranges`; `scribium-core/src/ast_to_ir.rs::let_lambda_metadata_survives_ast_to_ir_with_original_spans`, `let_implicit_lambda_metadata_is_absent_in_ir`; `scribium-core/src/evaluator.rs::let_explicit_parameter_returns_scalar`, `let_implicit_parameter_returns_scalar`, `let_shadows_parent_and_local_variables_do_not_leak`, `nested_let_uses_nearest_implicit_scope`; `scribium-core/src/lib.rs::compile_let_supports_explicit_and_implicit_block_lambdas`, `compile_let_isolates_local_variables_and_functions` |
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
result propagation. Generic inline lambda values and other lambda-consuming
builtins remain deferred under Issue #61; standalone lambda values, iteration,
components, and complete DynamicValue compatibility remain compatibility debt.

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
evidenced `.sum`, `.multiply`, `.uppercase`, and `.lowercase` chain forms and
their documented nested-call equivalents are **Semantically supported** with
strict left-to-right value flow; an unimplemented chain callee reports a
source-backed `E3001` evaluation error. The case builtins' small scalar
adaptation contract is evidenced, not complete DynamicValue compatibility.
**User-defined functions are also semantically supported for the evidenced
slice**: headerless implicit and required/optional explicit-parameter
declarations, positional/named binding where applicable, block-last-parameter
binding, child scope, source-order redeclaration, builtin override, and
scalar/structured-content results. Missing optional parameters bind semantic
`None`, which is consumed by `.otherwise` and `.isnone` without string
conversion in value context. Headerless `.1`/`.2` references are 1-based,
invocation-local, and preserve typed `IrValue`s; missing indices produce a
source-backed `E3003` diagnostic. **Conditional evaluation (`.if` / `.ifnot`)
with boolean literals and variable references (`.if {.name}`) is implemented**.
Standalone lambda values, iteration, components, and complete
programmable-document compatibility remain unimplemented. A matrix row can
therefore represent only the evidenced
forms at its stated level; an input form that currently fails to parse (for
example with an `E2xxx` diagnostic) is a compatibility gap, not evidence of
support for that form. `Unsupported` is reserved for the explicit compatibility
diagnostic state.

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
debt against the complete target. Standalone lambda values, iteration, layout
semantics, resource/data loading, and other v2.5.0 built-ins remain additional
gaps. The evidenced function row does not promote those later semantic
surfaces.

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
