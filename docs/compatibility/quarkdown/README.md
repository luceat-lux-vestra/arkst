# Quarkdown Compatibility Specification

## Status

- **Specification version:** 0.4 (verified baseline v2.5.0)
- **Reference upstream:** Quarkdown v2.5.0
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
| Implicit positional refs       | `.1`, `.2`, ...                  | Parsed                   | Implemented      |
| Positional arguments           | `.range {1} {10}`                | Parsed                   | Implemented      |
| Named arguments                | `.panel width:{320}`             | Parsed                   | Implemented      |
| Mixed positional/named         | `.panel {Intro} width:{320}`     | Parsed                   | Implemented      |
| Indented body argument         | `.panel {x}` + indent            | Parsed                   | Implemented      |
| Nested calls                   | `.outer {.inner {x}}`            | Parsed                   | Implemented      |
| Inline (mid-paragraph) call    | `see .note {x}`                  | Parsed                   | Implemented      |
| Tight-call boundaries          | word adjacency rejected          | Parsed                   | Implemented      |
| Malformed-call diagnostics     | `E2001`, `E2002`, `E2003`        | Error                    | Implemented      |
| Variables                      | `.var {name} {value}`, `.name`, `.name {value}`, `.if {.name}` | Semantically supported | Implemented      |
| Conditionals                   | `.if {cond}` / `.ifnot {cond}` | Semantically supported | Implemented      |
| Iteration                      | —                                | —                        | Planned          |
| Functions/components            | —                                | —                        | Planned          |
| Include/read                   | —                                | —                        | Planned          |
| Metadata                       | —                                | —                        | Planned          |
| Row/column/grid                | —                                | —                        | Planned          |
| Semantic evaluation            | `.if`/`.ifnot` + variables       | Partial / In progress    | Planned          |
| Call chaining (`::`)           | `.a {x}::b {y}`                  | Not implemented          | Planned          |
| Line continuation (`\`)        | `\` at end of line               | Not implemented          | Planned          |
| Tight / brace-wrapped calls    | `.x` wrapped in braces at adjacency | Not implemented       | Planned          |
| Multi-line arguments           | `{.…}` parsing spans lines        | Not implemented (E2xxx today) | Planned          |
| `.json` data loading           | `.json {path}` (new in v2.5.0)   | Not implemented          | Planned          |
| `.markdown` / `.llmstxt`       | (new in v2.5.0)                  | Not implemented          | Planned          |

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
| Implicit positional refs        | `scribium-quarkdown/src/lib.rs::parses_implicit_positional_references_and_boundaries`, `scribium-quarkdown/src/lib.rs::implicit_references_do_not_consume_arguments` |
| Positional arguments            | `scribium-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments`, `scribium-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification` |
| Named arguments                 | `scribium-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments` |
| Mixed positional/named          | `scribium-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments` |
| Indented body argument          | `scribium-markdown/src/parser.rs::quarkdown_body_uses_first_body_line_indent_not_fixed_width`, `quarkdown_body_rejects_one_space`, `quarkdown_body_tab_preserves_text_and_utf8_spans`, `quarkdown_body_dedent_terminates_body_and_shallower_lines_are_not_absorbed`, `quarkdown_body_preserves_nested_markdown`, `quarkdown_body_preserves_nested_quarkdown_blocks`, `quarkdown_body_is_container_relative_in_lists_and_blockquotes`, `quarkdown_body_blank_lines_preserve_body_lifecycle` |
| Nested calls                    | `scribium-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification`, `scribium-markdown/src/parser.rs::nested_content_calls_keep_prefix_suffix_and_original_spans` |
| Inline (mid-paragraph) call     | `scribium-markdown/src/parser.rs::nested_content_calls_keep_prefix_suffix_and_original_spans` |
| Tight-call boundaries           | `scribium-quarkdown/src/lib.rs::tight_word_adjacency_and_symbol_boundaries_are_explicit`, `scribium-quarkdown/src/lib.rs::parses_implicit_positional_references_and_boundaries` |
| Malformed-call diagnostics      | `scribium-quarkdown/src/lib.rs::rejects_malformed_and_ordered_arguments_without_panicking`, `scribium-markdown/src/parser.rs::malformed_root_block_reports_argument_span`, `scribium-markdown/src/parser.rs::malformed_inline_call_preserves_full_source_offset` |
| Conditionals                   | `evaluator.rs::if_true_keeps_block_body`, `evaluator.rs::if_false_drops_block_body`, `evaluator.rs::ifnot_true_drops_and_ifnot_false_keeps`, `evaluator.rs::boolean_identifiers_yes_no_true_false_case_insensitive`, `evaluator.rs::missing_condition_reports_e3001_and_drops`, `evaluator.rs::unresolvable_condition_reports_diagnostic`, `evaluator.rs::nested_if_inside_block_body_is_evaluated`, `evaluator.rs::content_value_second_argument_replaces_call`, `evaluator.rs::scalar_second_argument_becomes_text`, `evaluator.rs::inline_if_replaces_call_with_inline_body_or_content`, `evaluator.rs::inline_if_false_drops_call`, `evaluator.rs::inline_call_scalar_second_argument_becomes_text`, `evaluator.rs::non_conditional_calls_are_preserved_with_evaluated_bodies`, `evaluator.rs::named_condition_argument_works`, `evaluator.rs::named_condition_false_drops_body`, `evaluator.rs::named_condition_ifnot_inverts`, `evaluator.rs::named_condition_identifier_yes_no`, `evaluator.rs::named_body_argument_works`, `evaluator.rs::named_body_scalar_argument_works`, `evaluator.rs::block_body_priority_over_named_body`, `evaluator.rs::inline_named_condition_works`, `evaluator.rs::inline_named_body_works`, `evaluator.rs::named_condition_unresolvable_reports_e3001`, `lib.rs::compile_evaluates_if_true`, `lib.rs::compile_evaluates_if_false`, `lib.rs::compile_evaluates_ifnot`, `lib.rs::compile_evaluates_nested_if`, `lib.rs::compile_reports_e3001_for_unresolvable_condition`, `lib.rs::compile_evaluates_named_condition_true`, `lib.rs::compile_evaluates_named_condition_false`, `lib.rs::compile_evaluates_named_condition_yes_no`, `lib.rs::compile_evaluates_named_body`, `lib.rs::compile_evaluates_named_condition_and_body`, `lib.rs::compile_inline_named_condition`, `typst::conditional_evaluation_before_lowering` |
| Variables                      | `evaluator.rs::var_scalar_definition_and_reference`, `evaluator.rs::var_boolean_reference_in_conditional`, `evaluator.rs::var_false_boolean_drops_conditional`, `evaluator.rs::var_ifnot_with_variable`, `evaluator.rs::var_explicit_reassignment`, `evaluator.rs::var_variable_name_reassignment`, `evaluator.rs::var_reassignment_produces_no_output`, `evaluator.rs::var_inline_use`, `evaluator.rs::var_block_variable`, `evaluator.rs::var_conditional_declaration_execution_order`, `evaluator.rs::var_unknown_call_preserved`, `evaluator.rs::var_malformed_declaration_reports_e3002`, `evaluator.rs::var_nested_evaluation_in_block_variable`, `evaluator.rs::var_evaluation_immutable_and_deterministic`, `lib.rs::compile_variable_declaration_and_reference`, `lib.rs::compile_variable_boolean_in_conditional`, `lib.rs::compile_variable_false_conditional`, `lib.rs::compile_variable_ifnot`, `lib.rs::compile_variable_explicit_reassignment`, `lib.rs::compile_variable_name_reassignment`, `lib.rs::compile_variable_inline_use`, `lib.rs::compile_variable_block_variable`, `lib.rs::compile_variable_conditional_declaration`, `lib.rs::compile_variable_unknown_preserved`, `lib.rs::compile_variable_malformed_reports_e3002`, `lib.rs::compile_variable_nested_in_block`, `lib.rs::compile_variable_immutable_and_deterministic` |

## Compatibility Levels

- **Unsupported:** Produces explicit `E8xxx` diagnostic (used only by the
  compatibility-profile diagnostics; see `compatibility/diagnostics.rs`)
- **Error:** Produces an explicit parse diagnostic (`E2xxx`) at the call site
- **Parsed:** Accepted syntactically; behavior may be undefined or rejected
- **Semantically supported:** Scribium semantics match documented behavior
- **Output-equivalent:** Typst output matches reference for tested inputs
- **Known divergence:** Deliberate behavioral difference with documented
  rationale

Function calls are currently **Parsed**: `.name`, positional arguments
`{arg}`, named arguments `name:{arg}`, nested calls, and indented block
bodies are parsed into the Scribium AST/IR. **Conditional evaluation
(`.if` / `.ifnot`) with boolean literals and variable references
(`.if {.name}`) is implemented**. Full semantic evaluation (functions,
iteration, components) remains the next milestone (see `docs/SYNTAX.md` and
`docs/ROADMAP.md`). A matrix row can therefore represent only the evidenced
forms at its stated level; an input form that currently fails to parse (for
example with an `E2xxx` diagnostic) is a compatibility gap, not evidence of
support for that form. `Unsupported` is reserved for the explicit compatibility
diagnostic state.

### Tight-call boundaries

A call requires a boundary before and after it: whitespace, a symbol
(including `-`), or the start/end of the line. A call directly adjacent to a
word character — any Unicode letter or digit, plus `_` — is not recognized and
the whole construct stays ordinary text. Examples:

- `.note {x}` is a call; `.note {x}B` and `한.note {x}` are not (both
  Unicode and ASCII letters count as word characters).
- `-.note` and `.note-` are valid calls: `-` is a symbol, not a word
  character.

The new-in-Quarkdown brace-wrapped form (`H{.text {2}}O`), which lifts the
boundary requirement, is a documented v2.5.0 behavior but is **not
implemented** here; the inner call parses, but the wrapping braces are kept
as literal text.

### v2.5.0 public-language compatibility debt

Quarkdown has documented features represented in the v2.5.0 evidence set that
Scribium has not implemented yet. They are listed in the Feature Matrix as
`Planned`, are **not** current compatibility claims, and remain compatibility
debt against the complete target. They do not produce `E8xxx` diagnostics today
and their current behavior is undefined for the purposes of a claim; examples
include line continuation (`\` at EOL), `::` chaining, tight brace-wrapped
calls, multi-line arguments, and v2.5.0 built-ins such as data loading and
`.markdown`.

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
