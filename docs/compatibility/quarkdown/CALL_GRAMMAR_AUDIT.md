# Quarkdown v2.5.1 Call Grammar Audit

## Review record

- **Audit issue:** [#148](https://github.com/luceat-lux-vestra/scribium/issues/148)
- **Parent tracker:** [#147](https://github.com/luceat-lux-vestra/scribium/issues/147)
- **Audit branch base:** `247d9458029e52a7bd18cc2026bf806c3e7499f7`
- **Scribium comparison:** current `origin/main` at the base above
- **Pinned target:** Quarkdown v2.5.1
- **Pinned upstream tag commit:** `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- **Review date:** 2026-08-24
- **Rushdown:** unchanged at `e5eb4e4446541ea0ed53111c1b37e779283ff57c`

This is a grammar/frontend audit. It does not promote parser acceptance to
semantic or output compatibility. Binding validity belongs to [#149],
programmable evaluation and lambda semantics belong to [#150], and broader
content conversion is tracked by [#154].

## Evidence and clean-room record

The public v2.5.1 [syntax-of-a-function-call specification](https://quarkdown.com/wiki/syntax-of-a-function-call/)
is the primary behavior source for dot calls, positional/named arguments,
multiline arguments, line continuation, nested calls, chains, tight calls,
inline/block placement, bodies, and nested calls in bodies. The pinned
v2.5.1 `FunctionCallPatterns.kt`, `FunctionCallGrammar.kt`, and
`FunctionCallRefiner.kt` records are used only as permitted public behavioral
and lexical evidence and are listed in `SPEC_SOURCES.md`; no upstream code,
test, or fixture is copied or translated.

The pinned grammar evidence establishes a distinction that matters to this
audit:

- the function-call walker owns context-free call/argument shape;
- regular parameter existence, duplicate/unknown/excess binding, defaults,
  and target conversion are later binder concerns;
- body text is a separate lazy argument boundary in upstream;
- chain value injection is a later refiner/evaluator transformation.

Scribium's accepted architecture preserves this separation: the
`scribium-quarkdown` crate owns call grammar, `scribium-markdown` owns
Rushdown lifecycle and source-backed frontend conversion, and evaluator/IR/
backend claims are recorded separately.

## Existing #60 revalidation

Issue [#60](https://github.com/luceat-lux-vestra/scribium/issues/60) was
closed by merged PR [#65](https://github.com/luceat-lux-vestra/scribium/pull/65)
at merge commit `3061b3cffb72decc26e9590761a47dbb71a1fbf1`. Its final head was
`3c66288d526d17eca2aa36d1ce8c4f8187dd4c37`, based on
`e6da6ee2e6da6b7ba144b5b607d6e4338844c947`. The implementation and tests were
rechecked at the current base rather than assumed from the closed state.

The #60/#65 slice is still valid for its stated boundary:

- multiline positional/named braced arguments preserve original spans;
- backslash continuation accepts arbitrary leading spaces/tabs and LF/CRLF;
- `::` chains remain structurally represented through frontend and IR;
- top-level tight calls preserve wrapper and inner spans;
- block/inline selection, malformed diagnostics, UTF-8/CRLF, `.md`/`.qd`
  isolation, and dynamic body indentation remain covered;
- chain value-flow was explicitly deferred to #61 in that PR and is not a
  #148 grammar claim.

The revalidation also found four current production gaps and one documentation
overclaim. They are recorded as bounded issues [#157], [#158], [#159], and
[#160], all native sub-issues of #148:

| Issue | Finding | Owner |
|---|---|---|
| [#157](https://github.com/luceat-lux-vestra/scribium/issues/157) | Current normal-name/implicit-reference and Unicode/ASCII boundary rules do not match the pinned v2.5.1 lexical evidence. | `scribium-quarkdown` grammar and `scribium-markdown` integration |
| [#158](https://github.com/luceat-lux-vestra/scribium/issues/158) | A tight call nested inside a braced content argument loses its wrapper structure and exposes wrapper braces as text. | `scribium-markdown` frontend conversion |
| [#159](https://github.com/luceat-lux-vestra/scribium/issues/159) | Malformed inline call recovery reports `E2003` but drops following source text from the AST. | `scribium-markdown` inline integration |
| [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) | Supported Markdown inline structure inside Quarkdown content arguments is currently flattened to text with `E3010`. | `scribium-markdown` content conversion; related to #154 |

No production fix is included in this audit.

## Inventory matrix

`PARSED_ONLY` means the frontend preserves the syntax at the stated boundary;
it is not evaluator, IR, or output support. `PARTIAL` means the audit found a
known mismatch or loss within the grammar/frontend surface. The status column
is the conservative canonical status used for #147.

| Surface | Upstream v2.5.1 evidence | Minimal source example | Scribium production path / frontend verdict | Source-span verdict | `.md` / `.qd` isolation | LF / CRLF | Malformed behavior | Existing tests | Conformance evidence | #147 status | Existing issue/PR | Remaining gap | Owner |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Call introducer and normal identifier | Public syntax page; pinned `FunctionCallGrammar.kt` identifier and `FunctionCallPatterns.kt` boundary records | `.foo`, `.foo-bar`, `.foo_bar` | `scribium-quarkdown::parse_segment`; `.foo` parses, while current underscore/hyphen/Unicode contract is not pinned-equivalent | Accepted spans are source-backed; divergent lexical acceptance is recorded | Quarkdown extension is absent in Markdown mode | Existing LF/CRLF span tests pass | `.` and invalid starts are rejected without panic | `public_name_validation_matches_call_name_grammar`, `parses_normal_call_names_and_spans` | No independent pinned-contract fixture yet | `PARTIAL` | #60/#65; #157 | Confirm upstream lexical contract, then correct grammar/docs/tests without binder changes | `scribium-quarkdown` |
| Implicit positional references | Public lambda documentation; pinned numeric identifier evidence | `.1`, `.2`, `.12`, `.0`, `.01`, `.1abc` | Current special case accepts 1-based digit tokens, does not consume arguments; `.0`/`.01` and some boundary cases are local policy, not verified upstream behavior | Accepted `.1` spans are exact byte spans | `.qd` body path is covered; Markdown mode remains literal | UTF-8/CRLF body span evidence passes | Invalid/word-adjacent forms are deterministic current behavior | `parses_implicit_positional_references_and_boundaries`, `implicit_references_do_not_consume_arguments` | No dedicated lexical conformance fixture | `PARTIAL` | #60/#65; #157; binding in #150 | Separate recognition from #150 lambda binding and align only confirmed syntax | `scribium-quarkdown` |
| Positional arguments | Public syntax page and pinned grammar | `.foo {a} {b}` | `parse_arguments` and `parse_braced` preserve empty, whitespace, nested, multiline, UTF-8 content as parser values/spans | Argument/content spans are source-backed | `.md` has no Quarkdown extension; `.qd` and body paths are covered | LF/CRLF covered in unit/frontend tests | Unclosed braces produce `E2003` | `parses_positional_named_and_mixed_arguments`, `parses_multiline_nested_arguments_with_original_spans` | `call-positional-basic` is `Parsed` | `PARSED_ONLY` | #60/#65 | No #148 semantic claim; nested Markdown conversion is #160/#154 | `scribium-quarkdown` |
| Named and mixed argument shape | Public syntax page; binder rules are explicitly #149 | `.foo {a} name:{b}` | Name token, name span, value span, and positional-then-named shape are preserved; positional after named is rejected as syntax shape `E2001` | Name/value/whole-argument spans are exact | Isolation covered by frontend tests | LF/CRLF covered | Missing braced named value is `E2002` | `parses_positional_named_and_mixed_arguments`, malformed argument tests | No semantic conformance claim | `PARSED_ONLY` | #60/#65; binding cases in #149 | Do not classify unknown/duplicate/excess parameter validity here | `scribium-quarkdown` |
| Multiline braced arguments | Public syntax page; pinned grammar balanced-delimiter evidence | `.foo {\n  content\n}` | Arbitrary indentation and nested braces are accepted; content remains source-backed | Physical newlines and nested ranges are preserved | `.qd` and body interaction tested; Markdown mode isolated | LF and CRLF tested | Unclosed delimiter is `E2003` at the opening brace | `parses_multiline_nested_arguments_with_original_spans`; frontend multiline regression | Existing fixture family is `Parsed`-level only | `PARSED_ONLY` | #60/#65 | No new production gap found at this boundary | `scribium-quarkdown` |
| Line continuation | Public syntax page; pinned grammar continuation token | `.foo {a} \\\nname:{b}` | Trailing `\\` + LF/CRLF consumes continuation and optional indentation; block and inline paths preserve the call range | Marker/newline are not included in argument content; source range retains original bytes | Both modes covered through frontend path | LF/CRLF focused tests pass | Missing/malformed following argument is deterministic `E2004`; upstream's tooling-only trailing continuation allowance is not claimed | `parses_line_continuations_without_fixed_indentation`, `qd_multiline_arguments_and_continuations_keep_header_body_boundary` | No separate fixture; #60 regression evidence | `PARSED_ONLY` | #60/#65 | Reassess trailing-continuation/tooling-only range behavior only if document contract requires it | `scribium-quarkdown` |
| Nested calls in arguments | Public syntax page and pinned refiner expression boundary | `.outer {.inner {value}}` | Ordinary nested calls are structurally found in source-backed content; evaluation is not this audit | Outer, inner, and argument spans are source-backed | `.qd` path covered; `.md` remains literal | UTF-8/CRLF surrounding spans covered | Inner malformed diagnostics retain original offsets | `parses_nested_content_and_scalar_classification`, `nested_content_calls_keep_prefix_suffix_and_original_spans` | No end-to-end claim in #148 | `PARTIAL` | #60/#65; #158/#160 | Tight nested calls and Markdown inline content require bounded follow-ups | `scribium-markdown` |
| `::` chaining grammar | Public syntax page; pinned walker/refiner chain records | `.a {x}::b {y}::c` | Head and each segment are structurally retained without grammar-level value injection | Head, segment, name, argument, and complete spans pass | `.md` isolation and `.qd` inline/block paths pass | LF/CRLF and UTF-8 surrounding evidence pass | Missing/invalid segment is `E2004` | `parses_chains_as_source_backed_segments_without_rewriting`, `rejects_malformed_chains_deterministically` | #65 structural evidence; semantic evidence is separate in #61 | `PARSED_ONLY` | #60/#65; #61 | Do not claim left-to-right semantic/output equivalence from this row | `scribium-quarkdown` / #150 |
| Tight / brace-wrapped calls | Public syntax page; pinned call-pattern wrapper grammar | `H{.text {2}}O` | Top-level tight calls are recognized by `QuarkdownTightInlineParser` and preserve wrapper span; nested tight content is not lossless | Top-level wrapper and inner spans pass; nested content wrapper loss is #158 | `.qd` only; Markdown mode remains ordinary text | Existing UTF-8/CRLF span coverage is present | Missing wrapper/inner close is rejected or remains text deterministically | `parses_tight_calls_and_preserves_inner_provenance`, `qd_inline_continuation_and_tight_calls_preserve_text_and_spans` | No nested-tight conformance case | `PARTIAL` | #60/#65; #158 | Preserve nested wrapper structure without synthetic fragment reparse | `scribium-markdown` |
| Inline vs block placement | Public syntax page; pinned `FunctionCallPatterns.kt` block/inline rule | `Text .foo {x} text` vs `.foo {x}` | Block parser accepts isolated calls; inline parser preserves calls inside paragraphs and trailing same-line text selects inline path | Call/paragraph spans remain source-backed | `.md` has no directive extension; `.qd` body/list/blockquote paths tested | LF/CRLF covered | Malformed inline recovery loses suffix (#159) | `qd_mode_parses_root_and_inline_calls_with_crlf_provenance`, body/container tests | Existing parser evidence only | `PARTIAL` | #60/#65; #159 | Keep placement heuristic independent of evaluator and retain all malformed source | `scribium-markdown` |
| Body argument and dynamic indentation | Public syntax page; pinned grammar body token; body nested-call example | `.foo\n  body\n    .inner` | `QuarkdownBlock` records first qualifying visual indentation, dedent termination, blank lines, nested Markdown/block/inline calls | Body line ranges and nested spans remain original source-backed | `.qd` body paths covered; Markdown mode isolated | LF/CRLF, UTF-8, tab, mixed/deeper indentation tested | One-space body is rejected; malformed header diagnostics are source-backed | `quarkdown_body_uses_first_body_line_indent_not_fixed_width` and body family tests | `call-indented-body-basic` is `Parsed` | `PARSED_ONLY` | PR #54; #60/#65 | Evaluator/body parameter semantics remain #149/#150; raw-body vs structured-body distinction remains documented | `scribium-markdown` |
| Call boundaries and protected contexts | Public call-pattern boundary; Markdown content/code behavior | `word.foo {x}`, `H{.foo {x}}O`, `` `.foo {x}` ``, fenced code, link/label contexts | Rushdown lifecycle plus Quarkdown extensions shields code/verbatim and Markdown mode; current Unicode/identifier boundary differs from pinned lexical evidence | Existing protected-context spans are source-backed | `.md`/`.qd` isolation explicitly passes for tested contexts | UTF-8/CRLF context tests pass | Incomplete cases follow current parser diagnostics/literal policy | `markdown_mode_keeps_quarkdown_as_text`, `code_shields_quarkdown_extension`, #60 boundary tests | No exhaustive link-label/emphasis matrix | `PARTIAL` | #60/#65; #157 | Add focused protected-context evidence and align confirmed Unicode/ASCII boundaries | `scribium-markdown` / #157 |
| Malformed input and recovery | Public grammar requires balanced syntax; Scribium Engineering requires no panic/input loss | `.foo {`, `.foo }`, `.foo {a`, `.foo {a}::` | Structured `E2001`–`E2004` diagnostics are deterministic, but malformed inline call recovery drops trailing source | Diagnostic spans are source-backed; AST source completeness fails for #159 | Code/verbatim shielding remains intact | UTF-8/CRLF diagnostic offsets pass | Root errors diagnose; inline suffix loss is a production gap | `malformed_*` parser tests cover diagnostics, not recovery completeness | No unsupported diagnostic golden for this grammar gap | `PARTIAL` | #60/#65; #159 | Retain all non-call source while preserving diagnostic span and policy | `scribium-markdown` |
| Markdown structure inside content arguments | Public Markdown-content page; public call syntax nested-content contract | `.outer {**bold** and [link](target)}` | Current frontend preserves opaque text and emits `E3010`; it does not preserve inline Markdown nodes | Original opaque span is exact, but structure is not retained | `.md` baseline works; `.qd` content-argument gap is separate | Existing UTF-8/CRLF source span coverage is partial | Delimiter text is retained as text with `E3010` | `content_argument_preserves_original_span_and_reports_markdown_gap` | No positive content-argument fixture | `PARTIAL` | #160; related #154 | Define bounded supported inline subset at existing Rushdown ownership boundary | `scribium-markdown` / #154 |

## Evidence-level separation

| Evidence layer | #148 result |
|---|---|
| Documented upstream behavior | The public syntax and Markdown-content pages define the listed call forms and boundaries. |
| Observed pinned upstream behavior | Pinned lexer/grammar/refiner records confirm identifier, argument, continuation, body, wrapper, and chain implementation boundaries; source evidence is not an implementation recipe. |
| Scribium parser acceptance | The rows marked `PARSED_ONLY` are covered by source-backed parser/frontend tests. |
| Source provenance | Accepted call, argument, segment, body, UTF-8, and CRLF spans are verified; malformed inline recovery and nested tight content are not lossless. |
| Evaluator semantics | Out of scope for #148. Existing #61/#147 evidence may be linked only at its stated bounded semantic level. |
| IR preservation | #65 structural chain evidence remains valid; #148 adds no IR schema or semantic claim. |
| Backend/output equivalence | Not claimed by this audit. |

## Documentation truthfulness changes

- `docs/SYNTAX.md` now distinguishes Scribium's current local name lexer from
  the pinned upstream contract and links #157–#160 for known gaps.
- `docs/compatibility/quarkdown/README.md` and `GAP_INVENTORY.md` link this
  audit and no longer present the affected parser rows as unqualified complete
  call-grammar support.
- `SPEC_SOURCES.md` records the pinned `FunctionCallGrammar.kt` evidence used
  for the lexical/argument audit without duplicating the existing syntax-page
  record.

## Explicit deferrals and non-goals

- No production parser, evaluator, binder, builtin, IR, Typst, or Rushdown code
  changed in this audit PR.
- No dependency, compatibility target, or upstream baseline changed.
- #149 owns unknown/duplicate/excess/default/optional binding behavior.
- #150 owns lambda binding, call evaluation order, chain value flow, and
  semantic body execution.
- #154 owns broader content/media/Markdown-extension surface follow-up.
- The four production gaps are tracked as #157–#160; they are not hidden with
  expected-failure tests or compatibility allowlists in this PR.

[from #147]: https://github.com/luceat-lux-vestra/scribium/issues/147
[from #148]: https://github.com/luceat-lux-vestra/scribium/issues/148
[from #149]: https://github.com/luceat-lux-vestra/scribium/issues/149
[from #150]: https://github.com/luceat-lux-vestra/scribium/issues/150
[from #154]: https://github.com/luceat-lux-vestra/scribium/issues/154
