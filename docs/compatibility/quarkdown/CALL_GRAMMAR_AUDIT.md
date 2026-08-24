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
v2.5.1 `GrammarUtils.kt`, `FunctionCallPatterns.kt`, `FunctionCallGrammar.kt`,
`FunctionCallRefiner.kt`, and `RegularArgumentsBinder.kt` records are used
only as permitted public behavioral and lexical evidence and are listed in
`SPEC_SOURCES.md`; no upstream code, test, or fixture is copied or translated.

The pinned grammar evidence establishes distinctions that matter to this
audit:

- the function-call walker owns context-free call/argument shape, including
  escaped delimiter recognition and balanced-brace scanning;
- `FunctionCallGrammar` places an optional `argumentSeparator` before every
  inline argument and before every chain `::`, and separately consumes a
  trailing line continuation even without a following argument;
- regular parameter existence, duplicate/unknown/excess binding, defaults,
  target conversion, and unnamed-after-named rejection are later binder
  concerns;
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
- after-argument backslash continuation accepts arbitrary leading spaces/tabs
  and LF/CRLF in the revalidated #60/#65 slice; separator placement before the
  first argument, before `::`, and at the trailing edge is the #164 gap;
- `::` chains remain structurally represented through frontend and IR;
- top-level tight calls preserve wrapper and inner spans;
- block/inline selection, malformed diagnostics, UTF-8/CRLF, `.md`/`.qd`
  isolation, and dynamic body indentation remain covered;
- chain value-flow was explicitly deferred to #61 in that PR and is not a
  #148 grammar claim.

The revalidation now records seven current production gaps and one
documentation overclaim. They are recorded as bounded issues [#157], [#158],
[#159], [#160], [#162], [#163], and [#164], all native sub-issues of #148:

| Issue | Finding | Owner |
|---|---|---|
| [#157](https://github.com/luceat-lux-vestra/scribium/issues/157) | Current call-name, named-argument-name/delimiter-adjacency, implicit-reference, and Unicode/ASCII boundary rules do not all match the pinned v2.5.1 lexical evidence. | `scribium-quarkdown` grammar and `scribium-markdown` integration |
| [#158](https://github.com/luceat-lux-vestra/scribium/issues/158) | A tight call nested inside a braced content argument loses its wrapper structure and exposes wrapper braces as text. | `scribium-markdown` frontend conversion |
| [#159](https://github.com/luceat-lux-vestra/scribium/issues/159) | Malformed inline call recovery reports `E2003` but drops following source text from the AST. | `scribium-markdown` inline integration |
| [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) | Supported Markdown inline structure inside Quarkdown content arguments is currently flattened to text with `E3010`. | `scribium-markdown` content conversion; related to #154 |
| [#162](https://github.com/luceat-lux-vestra/scribium/issues/162) | Escaped call/argument delimiters are not handled with pinned v2.5.1 `unescapedMatch()` and balanced-brace semantics; escaped braces can truncate or unbalance current calls. | `scribium-quarkdown` grammar; `scribium-markdown` integration if affected |
| [#163](https://github.com/luceat-lux-vestra/scribium/issues/163) | Scribium rejects positional-after-named in the grammar with `E2001`, while pinned v2.5.1 preserves argument shape and assigns unnamed-after-named rejection to the binder. | `scribium-quarkdown` / `scribium-markdown` representation; semantic rejection remains #149 |
| [#164](https://github.com/luceat-lux-vestra/scribium/issues/164) | Current separator placement omits continuation before the first argument, whitespace/continuation before `::`, and pinned trailing-continuation consumption without a following argument. | `scribium-quarkdown` grammar; `scribium-markdown` block/inline integration |

No production fix is included in this audit.

## Inventory matrix

`PARSED_ONLY` means the frontend preserves the syntax at the stated boundary;
it is not evaluator, IR, or output support. `PARTIAL` means the audit found a
known mismatch or loss within the grammar/frontend surface. The status column
is the conservative canonical status used for #147.

| Surface | Upstream v2.5.1 evidence | Minimal source example | Scribium production path / frontend verdict | Source-span verdict | `.md` / `.qd` isolation | LF / CRLF | Malformed behavior | Existing tests | Conformance evidence | #147 status | Existing issue/PR | Remaining gap | Owner |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Call and argument-name identifiers | Public syntax page; pinned `FunctionCallGrammar.kt` shared `IDENTIFIER_PATTERN` for function and optional named-argument identifiers, plus `FunctionCallPatterns.kt` boundary records | `.foo`, `.foo name:{x}`, `.foo _:{x}`, `.foo -:{x}`, `.foo 1:{x}`, `.foo 10:{x}`, `.foo name-1:{x}`, `.1`, `.1abc`, `word.foo {x}` | `scribium-quarkdown::parse_segment` and `parse_arguments`; normal calls use local `[A-Za-z_][A-Za-z0-9_-]*`, named-argument scanning effectively accepts `[A-Za-z0-9_-]+`, implicit references are a separate `.1...` rule, and boundaries are local rather than pinned-equivalent | Call, named-name, and implicit-reference spans are source-backed; `_`, `-`, and numeric named-name byte spans are independently asserted, but lexical equivalence remains unresolved | Quarkdown extension is absent in Markdown mode; the audit probe confirms all named-name forms remain isolated there | Existing LF/CRLF boundary and UTF-8 span tests pass; named-name span probes are LF and current CRLF boundary evidence remains separate | Invalid normal/implicit starts and word-adjacent forms are deterministic; `_`, `-`, numeric, and hyphenated named names are currently accepted without diagnostics | `public_name_validation_matches_call_name_grammar`, `parses_normal_call_names_and_spans`, `parses_implicit_positional_references_and_boundaries`, `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_named_argument_identifier_lexical_contract` | No independent pinned-contract fixture yet; current behavior is audit evidence only | `PARTIAL` | #157; #60/#65 | Reconcile the shared call/argument-name lexical contract, implicit-reference recognition, and boundaries without changing binder/evaluator ownership | `scribium-quarkdown`; `scribium-markdown` integration |
| Implicit positional references | Public lambda documentation; pinned numeric identifier evidence | `.1`, `.2`, `.12`, `.0`, `.01`, `.1abc` | Current special case accepts 1-based digit tokens, does not consume arguments; `.0`/`.01` and some boundary cases are local policy, not verified upstream behavior | Accepted `.1` spans are exact byte spans | `.qd` body path is covered; Markdown mode remains literal | UTF-8/CRLF body span evidence passes | Invalid/word-adjacent forms are deterministic current behavior | `parses_implicit_positional_references_and_boundaries`, `implicit_references_do_not_consume_arguments` | No dedicated lexical conformance fixture | `PARTIAL` | #60/#65; #157; binding in #150 | Separate recognition from #150 lambda binding and align only confirmed syntax | `scribium-quarkdown` |
| Positional arguments | Public syntax page and pinned grammar | `.foo {a} {b}` | `parse_arguments` and `parse_braced` preserve empty, whitespace, nested, multiline, UTF-8 content as parser values/spans | Argument/content spans are source-backed | `.md` has no Quarkdown extension; `.qd` and body paths are covered | LF/CRLF covered in unit/frontend tests | Unclosed braces produce `E2003` | `parses_positional_named_and_mixed_arguments`, `parses_multiline_nested_arguments_with_original_spans` | `call-positional-basic` is `Parsed` | `PARSED_ONLY` | #60/#65 | No #148 semantic claim; nested Markdown conversion is #160/#154 | `scribium-quarkdown` |
| Escaped call/argument delimiters | Public syntax page; pinned `GrammarUtils.kt::unescapedMatch` / `balancedDelimitersMatch` and `FunctionCallGrammar.kt` | `\.foo {x}`; `.foo {a \} b}`; `.foo {a \{ b}`; nested, UTF-8, and CRLF variants | `scribium-quarkdown::parse_call`, `parse_arguments`, and `parse_braced`; escaped `.` remains literal, but escaped braces are counted as delimiters, causing early close, `E2003`, or source suffix separation | Literal introducer span is source-backed; brace probes retain byte offsets but do not preserve pinned delimiter boundaries; UTF-8/CRLF truncation is recorded | Quarkdown probes are covered; Markdown mode keeps all probes literal and isolated | LF probes plus UTF-8; CRLF truncation is independently asserted | Escaped closing delimiter silently truncates; escaped opening reports `E2003`; nested escaped pair currently closes by incorrect depth accounting | `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_escaped_delimiter_gap` | No positive pinned conformance fixture; audit evidence only | `PARTIAL` | #162; #60/#65 boundary evidence | Apply pinned escaped-delimiter recognition and depth semantics without synthetic reparsing | `scribium-quarkdown`; `scribium-markdown` integration if affected |
| Named and mixed argument shape | Public syntax page; pinned `FunctionCallGrammar.kt` optional-name sequence, shared identifier pattern, and adjacent `identifier ":" "{"` delimiter grammar, plus `RegularArgumentsBinder.kt` binder rule | `.foo named:{x} {y}`; `.foo name :{x}`; `.foo name: {x}`; `.foo name : {x}` | Scribium preserves current named tokens but its local name scanner accepts `_`, `-`, numeric, and hyphenated forms and its delimiter scanner accepts horizontal whitespace on both sides of `:`. It rejects positional-after-named in the grammar layer with `E2001`. Pinned v2.5.1 preserves the argument sequence, requires adjacent named delimiters, and lets the binder reject unnamed-after-named | Current named name/value/whole-argument spans and the `E2001` opening-brace span are source-backed; the named-name and delimiter-adjacency contract is not pinned-equivalent and is tracked by #157 | Existing frontend mode-isolation tests pass; Quarkdown probes are audited and Markdown mode remains isolated | LF and UTF-8/CRLF name/value span probes pass locally; no upstream raw-CRLF claim is made for the separate continuation token | Missing braced named value is `E2002`; `_`, `-`, numeric names, and all three whitespace variants are currently accepted; positional-after-named is early `E2001` | `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_named_argument_identifier_lexical_contract`, `audit_records_current_named_argument_delimiter_adjacency_gap`, `audit_records_current_early_rejection_of_positional_after_named`; existing malformed argument tests | No semantic conformance claim | `PARTIAL` | #157 lexical/delimiter gap; #163 representation ownership; #149 binder/value ownership; #60/#65 | Reconcile named-argument identifier and adjacent delimiter rules under #157, preserve positional/named order through grammar/frontend under #163, and leave semantic validity to #149 | `scribium-quarkdown`; `scribium-markdown` representation; #149 semantic owner |
| Multiline braced arguments | Public syntax page; pinned grammar balanced-delimiter evidence | `.foo {\n  content\n}` | Arbitrary indentation and nested braces are accepted; content remains source-backed | Physical newlines and nested ranges are preserved | `.qd` and body interaction tested; Markdown mode isolated | LF and CRLF tested | Unclosed delimiter is `E2003` at the opening brace | `parses_multiline_nested_arguments_with_original_spans`; frontend multiline regression | Existing fixture family is `Parsed`-level only | `PARSED_ONLY` | #60/#65 | No new production gap found at this boundary | `scribium-quarkdown` |
| Line continuation and argument-separator placement | Public syntax page; pinned `FunctionCallGrammar.kt` `argumentSeparator` before every inline argument and `trailingLineContinuation`; pinned `FunctionCallTest.kt` continuation-before-first-argument/body and trailing-range evidence | `.foo \\\nname:{x}`; `.foo {x} \\\n` | Current `parse_arguments` consumes continuation only after an argument; before the first argument it stops at `.foo`, and a trailing continuation reports `E2004` in block and inline paths | Current block first-continuation span includes `.foo \\\n`; current inline call stops at `.foo`; trailing and malformed continuation diagnostics retain current source offsets but do not preserve pinned grammar shape | Quarkdown probes are audited; Markdown mode keeps them literal and isolated | LF and current CRLF probes are source-backed; pinned token directly checks `\\` + LF, so Scribium CRLF acceptance is not raw-CRLF upstream conformance evidence | Trailing continuation and continuation-before-chain currently produce `E2004`; pinned trailing production consumes the former | `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_continuation_before_first_argument_gap`, `audit_records_current_trailing_continuation_gap`; existing continuation tests | No positive pinned conformance fixture; `FunctionCallTest.kt` is pinned integration evidence only | `PARTIAL` | #164; #60/#65 | Align separator placement before the first argument and before chain separators, consume pinned trailing continuation, preserve spans, and keep LF/CRLF evidence levels separate | `scribium-quarkdown`; `scribium-markdown` integration |
| Nested calls in arguments | Public syntax page and pinned refiner expression boundary | `.outer {.inner {value}}` | Ordinary nested calls are structurally found in source-backed content; evaluation is not this audit | Outer, inner, and argument spans are source-backed | `.qd` path covered; `.md` remains literal | UTF-8/CRLF surrounding spans covered | Inner malformed diagnostics retain original offsets | `parses_nested_content_and_scalar_classification`, `nested_content_calls_keep_prefix_suffix_and_original_spans` | No end-to-end claim in #148 | `PARTIAL` | #60/#65; #158/#160 | Tight nested calls and Markdown inline content require bounded follow-ups | `scribium-markdown` |
| `::` chaining grammar | Public syntax page; pinned `FunctionCallGrammar.kt` `chainCallParser` optional `argumentSeparator` before `::`, plus `FunctionCallChainingTest.kt` | `.a {x}::b {y}::c`; `.a {x} ::b {y}`; `.a {x} \\\n::b {y}` | Direct `::` is structurally retained, but current chain scanning starts only when `::` is immediately at the current call end; whitespace before `::` stops the call and continuation before `::` reaches the current `E2004` path | Direct head/segment spans pass; separator variants stop/truncate the current call or report a source-backed diagnostic rather than preserving the chain | `.md` isolation and `.qd` block/inline probes are covered by the audit test | LF and current UTF-8/CRLF surrounding evidence remain source-backed; pinned continuation LF normalization is not widened to a raw-CRLF claim | Whitespace-before-chain is treated as trailing text; continuation-before-chain reports `E2004` | `parses_chains_as_source_backed_segments_without_rewriting`, `rejects_malformed_chains_deterministically`, `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_chain_separator_placement_gap` | No positive pinned separator-placement fixture; `FunctionCallChainingTest.kt` is pinned chain integration evidence | `PARTIAL` | #164; #60/#65; #61 | Accept optional whitespace/continuation separators before `::` while preserving all segment spans; do not infer chain value-flow compatibility | `scribium-quarkdown`; `scribium-markdown` integration / #150 |
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
| Observed pinned upstream behavior | Pinned lexer/grammar/refiner records confirm identifier, argument, separator placement, continuation, body, wrapper, and chain implementation boundaries; source evidence is not an implementation recipe. |
| Scribium parser acceptance | The rows marked `PARSED_ONLY` are covered by source-backed parser/frontend tests. |
| Source provenance | Accepted call, argument, named-name, segment, body, UTF-8, and CRLF spans are verified; separator-placement truncation, escaped-delimiter truncation, malformed inline recovery, nested tight content, lexical identifier divergence, and early mixed-argument rejection are not lossless/pinned-equivalent. |
| Evaluator semantics | Out of scope for #148. Existing #61/#147 evidence may be linked only at its stated bounded semantic level. |
| IR preservation | #65 structural chain evidence remains valid; #148 adds no IR schema or semantic claim. |
| Backend/output equivalence | Not claimed by this audit. |

## Documentation truthfulness changes

- `docs/SYNTAX.md` now distinguishes Scribium's current local call,
  separator, and named-argument delimiter behavior and
  mixed-argument parser enforcement from the pinned upstream contract and
  links #157–#164 for known gaps.
- `docs/compatibility/quarkdown/README.md` and `GAP_INVENTORY.md` link this
  audit and no longer present the affected parser rows as unqualified complete
  call-grammar support.
- `SPEC_SOURCES.md` records the pinned `GrammarUtils.kt`,
  `FunctionCallGrammar.kt`, and `RegularArgumentsBinder.kt` evidence used for
  escaped-delimiter and argument-ownership findings without duplicating the
  existing syntax-page record.

## Explicit deferrals and non-goals

- No production parser, evaluator, binder, builtin, IR, Typst, or Rushdown code
  changed in this audit PR.
- No dependency, compatibility target, or upstream baseline changed.
- #149 owns unknown/duplicate/excess/default/optional binding behavior.
- #150 owns lambda binding, call evaluation order, chain value flow, and
  semantic body execution.
- #154 owns broader content/media/Markdown-extension surface follow-up.
- The seven production gaps are tracked as #157–#160 and #162–#164; they are not
  hidden with expected-failure tests or compatibility allowlists in this PR.

[from #147]: https://github.com/luceat-lux-vestra/scribium/issues/147
[from #148]: https://github.com/luceat-lux-vestra/scribium/issues/148
[from #149]: https://github.com/luceat-lux-vestra/scribium/issues/149
[from #150]: https://github.com/luceat-lux-vestra/scribium/issues/150
[from #154]: https://github.com/luceat-lux-vestra/scribium/issues/154
[from #162]: https://github.com/luceat-lux-vestra/scribium/issues/162
[from #163]: https://github.com/luceat-lux-vestra/scribium/issues/163
