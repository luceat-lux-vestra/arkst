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
- top-level and nested tight calls preserve wrapper and inner spans;
- block/inline selection, malformed diagnostics, UTF-8/CRLF, `.md`/`.qd`
  isolation, and dynamic body indentation remain covered;
- chain value-flow was explicitly deferred to #61 in that PR and is not a
  #148 grammar claim.

The revalidation originally recorded seven current production gaps and one
documentation overclaim. The lexical gaps in [#157] and [#158] are now
implemented at the grammar/frontend boundary below; the remaining bounded
issues are [#159], [#160], [#162], and [#164], all native sub-issues of #148:

| Issue | Finding | Owner |
|---|---|---|
| [#157](https://github.com/luceat-lux-vestra/scribium/issues/157) | Implemented: the shared call/named-argument identifier scanner, adjacent named delimiter, numeric-reference distinction, pinned call-start boundary, and source-backed frontend spans now match the recorded v2.5.1 lexical evidence. | `scribium-quarkdown` grammar and `scribium-markdown` integration |
| [#158](https://github.com/luceat-lux-vestra/scribium/issues/158) | Implemented: `scribium-markdown` delegates nested content `{` probes to `scribium_quarkdown::parse_tight_call` against the original source and preserves the complete wrapper, inner/head, name, argument, and surrounding-text spans. | `scribium-markdown` frontend conversion |
| [#159](https://github.com/luceat-lux-vestra/scribium/issues/159) | Malformed inline call recovery reports `E2003` but drops following source text from the AST. | `scribium-markdown` inline integration |
| [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) | Supported Markdown inline structure inside Quarkdown content arguments is currently flattened to text with `E3010`. | `scribium-markdown` content conversion; related to #154 |
| [#162](https://github.com/luceat-lux-vestra/scribium/issues/162) | Escaped call/argument delimiters are not handled with pinned v2.5.1 `unescapedMatch()` and balanced-brace semantics; escaped braces can truncate or unbalance current calls. | `scribium-quarkdown` grammar; `scribium-markdown` integration if affected |
| [#163](https://github.com/luceat-lux-vestra/scribium/issues/163) | Implemented: the grammar, frontend, and IR retain one source-ordered argument sequence, including positional-after-named transitions, with source-backed spans. The shared #165 engine binder consumes this representation for semantic validation. | `scribium-quarkdown` / `scribium-markdown` representation plus `scribium-engine` binder |
| [#164](https://github.com/luceat-lux-vestra/scribium/issues/164) | Current separator placement omits continuation before the first argument, whitespace/continuation before `::`, and pinned trailing-continuation consumption without a following argument. | `scribium-quarkdown` grammar; `scribium-markdown` block/inline integration |

The original #148 audit contained no production fix. Issue #157 is the bounded
production reconciliation of its identifier, delimiter, reference, and call
boundary row, and #163 is the bounded reconciliation of its ordered mixed
argument representation; the tests named below remain parser/provenance
evidence only.

## #157 implementation reconciliation

Based on the required base `af037698821f6978885e0799b11c6ea717eb5be0`, issue
#157 introduces one byte-oriented scanner for the pinned call-grammar alternatives
`[A-Za-z][A-Za-z0-9]*|[0-9]+`. Alphabetic identifiers do not consume `_` or `-`;
numeric identifiers include `0` and leading zeros, and the scanner returns the
accepted prefix, so `.1abc` is `.1` followed by the untouched `abc` remainder.
The same scanner is used for normal call segments and named-argument
candidates.

Named arguments are recognized only when the scanned identifier is immediately
followed by `:{`. Non-matching `_`, `-`, hyphenated, and whitespace-delimited
forms remain outside `named_args`; when the optional named-argument parser
cannot match its complete boundary, it stops without a fabricated diagnostic
and leaves the candidate as source remainder. A matched `name:{` with an
unclosed value remains a genuine malformed-brace diagnostic. Numeric
identifiers remain grammar identifiers, so `.1 {item}` follows the same
argument-shape path; implicit-reference binding remains #150-owned.

Call starts use the pinned ASCII boundary evidence: beginning of source or a
preceding byte other than ASCII alphanumeric, `.`, or `\\`. Non-ASCII
surroundings are therefore not classified as word barriers, and trailing
source after a parsed call remains available to the Markdown placement layer.
All spans continue to be byte offsets into the original LF/CRLF/UTF-8 source.

Independent evidence is in
`crates/scribium-quarkdown/src/lib.rs` tests
`call_and_named_identifiers_share_the_pinned_scanner`,
`numeric_identifiers_share_the_argument_grammar`,
`parses_implicit_positional_references_and_boundaries`, and
`tight_word_adjacency_and_symbol_boundaries_are_explicit`, plus
`crates/scribium-markdown/tests/call_grammar_audit.rs` tests
`audit_aligns_named_argument_identifier_lexing_and_spans`,
`audit_requires_adjacent_named_argument_delimiters_and_preserves_source`,
`audit_aligns_call_boundaries_across_utf8_crlf_and_modes`, and
`audit_numeric_named_arguments_cross_the_block_continuation_boundary`.

## #163 implementation reconciliation

At current `main` base `53f6abc8c614c05a9dcf0702378e0decab8ab9ff`, the
grammar/frontend boundary now stores each call head and chain segment as one
ordered `CallArgument` sequence. Each element retains its original argument
span; named elements additionally retain name, value, and complete spans. This
preserves `Named -> Positional` and repeated mixed transitions without
reordering or reconstructing source fragments.

The independently authored frontend evidence is
`crates/scribium-markdown/tests/call_grammar_audit.rs::audit_preserves_ordered_mixed_arguments_until_binder_validation`.
It covers block, inline, and chain calls, exact surrounding-source boundaries,
UTF-8, real CRLF bytes, and `.md` isolation. The parser no longer emits
`E2001` for positional-after-named. The current IR retains the ordered
lightweight `IrCallArgument` reference sequence alongside its legacy
positional/named value projections;
the engine-owned #165 binder consumes that sequence and rejects the invalid
shape with `E3003`, using the positional `CallArgument` span as the primary and
the preceding named argument span as a secondary location. This audit records
the representation prerequisite only: semantic binding remains an engine
concern, and no output compatibility claim is made.

## Inventory matrix

`PARSED_ONLY` means the frontend preserves the syntax at the stated boundary;
it is not evaluator, IR, or output support. `PARTIAL` means the audit found a
known mismatch or loss within the grammar/frontend surface. The status column
is the conservative canonical status used for #147.

| Surface | Upstream v2.5.1 evidence | Minimal source example | Scribium production path / frontend verdict | Source-span verdict | `.md` / `.qd` isolation | LF / CRLF | Malformed behavior | Existing tests | Conformance evidence | #147 status | Existing issue/PR | Remaining gap | Owner |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Call and argument-name identifiers | Public syntax page; pinned `FunctionCallGrammar.kt` shared `IDENTIFIER_PATTERN` for function and optional named-argument identifiers, plus `FunctionCallPatterns.kt` boundary records | `.foo`, `.foo name:{x}`, `.foo _:{x}`, `.foo -:{x}`, `.foo 1:{x}`, `.foo 10:{x}`, `.foo name-1:{x}`, `.1`, `.01`, `.1abc`, `word.foo {x}`, `.foo {x}한글` | `scribium-quarkdown::parse_segment` and `parse_arguments` share the pinned ASCII identifier scanner; nonmatching underscore/hyphen forms stop before the invalid suffix, numeric identifiers include leading zeros and return `.1` as the accepted prefix of `.1abc`, and call-start boundaries follow the pinned ASCII/non-ASCII distinction | Call, named-name, named-argument, and surrounding call spans are source-backed and asserted over LF, CRLF, and UTF-8; trailing source is not consumed | Quarkdown extension is absent in Markdown mode; valid and nonmatching forms are independently checked for `.qd`/`.md` isolation | LF/CRLF and UTF-8 name/value/boundary spans pass | Nonmatching named forms remain outside `named_args`; incomplete `name:` candidates stop at the call prefix without `E2002`; matched malformed braces and other recovery remain separately owned | `call_and_named_identifiers_share_the_pinned_scanner`, `parses_implicit_positional_references_and_boundaries`, `tight_word_adjacency_and_symbol_boundaries_are_explicit`, `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_aligns_named_argument_identifier_lexing_and_spans`, `audit_aligns_call_boundaries_across_utf8_crlf_and_modes` | Independent parser/frontend evidence now exists for the pinned lexical boundary | `PARSED_ONLY` | #60/#65; #157 implemented; #159/#162/#164 remain neighboring grammar gaps | Preserve lexical recognition/provenance only; binder/evaluator semantics remain separate | `scribium-quarkdown`; `scribium-markdown` integration |
| Implicit positional references | Public lambda documentation; pinned numeric identifier evidence | `.1`, `.2`, `.12`, `.0`, `.01`, `.1abc` | Numeric dot identifiers are recognized through the shared call scanner, including `0` and leading zeros; `.1abc` is parsed as the `.1` numeric prefix with `abc` left as remainder, and numeric identifiers use the ordinary grammar argument path. Binding/evaluation remains #150-owned | Accepted numeric identifier/reference spans, `.1abc` prefix/remainder, and braced `.01` content spans are exact byte spans | `.qd` body and `.md` literal isolation are covered | UTF-8/CRLF frontend span evidence passes | ASCII-alphanumeric suffixes remain source after the accepted numeric prefix; malformed braces and broader recovery remain separate | `parses_implicit_positional_references_and_boundaries`, `numeric_identifiers_share_the_argument_grammar`, `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_keeps_implicit_reference_structural_and_modes_isolated` | Independent lexical/provenance evidence now exists; no evaluation claim | `PARSED_ONLY` | #60/#65; #157 implemented; binding/evaluation in #150 | Keep numeric recognition separate from #150 implicit-reference binding/evaluation | `scribium-quarkdown`; `scribium-markdown` integration |
| Positional arguments | Public syntax page and pinned grammar | `.foo {a} {b}` | `parse_arguments` and `parse_braced` preserve empty, whitespace, nested, multiline, UTF-8 content as parser values/spans | Argument/content spans are source-backed | `.md` has no Quarkdown extension; `.qd` and body paths are covered | LF/CRLF covered in unit/frontend tests | Unclosed braces produce `E2003` | `parses_positional_named_and_mixed_arguments`, `parses_multiline_nested_arguments_with_original_spans` | `call-positional-basic` is `Parsed` | `PARSED_ONLY` | #60/#65 | No #148 semantic claim; nested Markdown conversion is #160/#154 | `scribium-quarkdown` |
| Escaped call/argument delimiters | Public syntax page; pinned `GrammarUtils.kt::unescapedMatch` / `balancedDelimitersMatch` and `FunctionCallGrammar.kt` | `\.foo {x}`; `.foo {a \} b}`; `.foo {a \{ b}`; nested, UTF-8, and CRLF variants | `scribium-quarkdown::parse_call`, `parse_arguments`, and `parse_braced`; escaped `.` remains literal, but escaped braces are counted as delimiters, causing early close, `E2003`, or source suffix separation | Literal introducer span is source-backed; brace probes retain byte offsets but do not preserve pinned delimiter boundaries; UTF-8/CRLF truncation is recorded | Quarkdown probes are covered; Markdown mode keeps all probes literal and isolated | LF probes plus UTF-8; CRLF truncation is independently asserted | Escaped closing delimiter silently truncates; escaped opening reports `E2003`; nested escaped pair currently closes by incorrect depth accounting | `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_escaped_delimiter_gap` | No positive pinned conformance fixture; audit evidence only | `PARTIAL` | #162; #60/#65 boundary evidence | Apply pinned escaped-delimiter recognition and depth semantics without synthetic reparsing | `scribium-quarkdown`; `scribium-markdown` integration if affected |
| Named and mixed argument shape | Public syntax page; pinned `FunctionCallGrammar.kt` optional-name sequence, shared identifier pattern, and adjacent `identifier ":" "{"` delimiter grammar, plus `RegularArgumentsBinder.kt` binder rule | `.foo named:{x} {y}`; `.foo name :{x}`; `.foo name: {x}`; `.foo name : {x}` | Scribium shares the pinned identifier scanner and requires adjacent `identifier:{` for named arguments. Nonmatching whitespace/invalid-identifier forms remain source remainder without a fabricated `E2002`; matched named values and all mixed transitions are retained in one source-ordered frontend/IR sequence. Positional-after-named is no longer rejected by the parser; the shared engine binder rejects it with source-backed `E3003` before target execution | Named name/value/whole-argument spans, positional argument spans, and nonmatching boundary spans are source-backed; the binder diagnostic uses the actual positional and named argument spans, with no source reconstruction | Frontend mode-isolation and `.qd`/`.md` probes pass for block, inline, and chained calls; shared binder coverage includes native head/chain paths and callable binding | LF and UTF-8/real-CRLF name/value/order span probes pass locally; no upstream raw-CRLF claim is made for the separate continuation token | The three whitespace variants remain source remainder without diagnostics; a matched `name:{` with an unclosed value remains `E2003`; positional-after-named reaches the frontend without parser `E2001` and is rejected by the shared binder with `E3003` | `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_preserves_ordered_mixed_arguments_until_binder_validation`; `crates/scribium-core/src/lib.rs::compile_rejects_positional_after_named_through_shared_binder`; `crates/scribium-engine/src/invocation_binder.rs` tests; named-identifier and malformed argument tests | Grammar/frontend representation plus bounded shared engine binder; no complete value/conversion conformance claim | `PARSED_ONLY` | #157 implemented; #163 implements the representation; #165 implements the shared binder; #60/#65 | Keep lexical adjacency and source-order/provenance here; target conversion/content and diagnostics remain with #166/#167 where applicable | `scribium-quarkdown`; `scribium-markdown`; `scribium-engine` binder; #149/#165 |
| Multiline braced arguments | Public syntax page; pinned grammar balanced-delimiter evidence | `.foo {\n  content\n}` | Arbitrary indentation and nested braces are accepted; content remains source-backed | Physical newlines and nested ranges are preserved | `.qd` and body interaction tested; Markdown mode isolated | LF and CRLF tested | Unclosed delimiter is `E2003` at the opening brace | `parses_multiline_nested_arguments_with_original_spans`; frontend multiline regression | Existing fixture family is `Parsed`-level only | `PARSED_ONLY` | #60/#65 | No new production gap found at this boundary | `scribium-quarkdown` |
| Line continuation and argument-separator placement | Public syntax page; pinned `FunctionCallGrammar.kt` `argumentSeparator` before every inline argument and `trailingLineContinuation`; pinned `FunctionCallTest.kt` continuation-before-first-argument/body and trailing-range evidence | `.foo \\\nname:{x}`; `.foo {x} \\\n` | Current `parse_arguments` consumes continuation only after an argument; before the first argument it stops at `.foo`, and a trailing continuation reports `E2004` in block and inline paths | Current block first-continuation span includes `.foo \\\n`; current inline call stops at `.foo`; trailing and malformed continuation diagnostics retain current source offsets but do not preserve pinned grammar shape | Quarkdown probes are audited; Markdown mode keeps them literal and isolated | LF and current CRLF probes are source-backed; pinned token directly checks `\\` + LF, so Scribium CRLF acceptance is not raw-CRLF upstream conformance evidence | Trailing continuation and continuation-before-chain currently produce `E2004`; pinned trailing production consumes the former | `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_continuation_before_first_argument_gap`, `audit_records_current_trailing_continuation_gap`; existing continuation tests | No positive pinned conformance fixture; `FunctionCallTest.kt` is pinned integration evidence only | `PARTIAL` | #164; #60/#65 | Align separator placement before the first argument and before chain separators, consume pinned trailing continuation, preserve spans, and keep LF/CRLF evidence levels separate | `scribium-quarkdown`; `scribium-markdown` integration |
| Nested calls in arguments | Public syntax page and pinned refiner expression boundary | `.outer {.inner {value}}` | Ordinary and tight nested calls are structurally found in source-backed content; evaluation is not this audit | Outer, inner, and argument spans are source-backed | `.qd` path covered; `.md` remains literal | UTF-8/CRLF surrounding spans covered | Inner malformed diagnostics retain original offsets | `parses_nested_content_and_scalar_classification`, `nested_content_calls_keep_prefix_suffix_and_original_spans`, `audit_preserves_nested_tight_call_wrapper_inside_content_argument`, `audit_preserves_nested_tight_utf8_and_crlf_provenance` | No end-to-end claim in #148 | `PARTIAL` | #60/#65; #160 | Markdown inline content requires the bounded #160 follow-up | `scribium-markdown` |
| `::` chaining grammar | Public syntax page; pinned `FunctionCallGrammar.kt` `chainCallParser` optional `argumentSeparator` before `::`, plus `FunctionCallChainingTest.kt` | `.a {x}::b {y}::c`; `.a {x} ::b {y}`; `.a {x} \\\n::b {y}` | Direct `::` is structurally retained, but current chain scanning starts only when `::` is immediately at the current call end; whitespace before `::` stops the call and continuation before `::` reaches the current `E2004` path | Direct head/segment spans pass; separator variants stop/truncate the current call or report a source-backed diagnostic rather than preserving the chain | `.md` isolation and `.qd` block/inline probes are covered by the audit test | LF and current UTF-8/CRLF surrounding evidence remain source-backed; pinned continuation LF normalization is not widened to a raw-CRLF claim | Whitespace-before-chain is treated as trailing text; continuation-before-chain reports `E2004` | `parses_chains_as_source_backed_segments_without_rewriting`, `rejects_malformed_chains_deterministically`, `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_records_current_chain_separator_placement_gap` | No positive pinned separator-placement fixture; `FunctionCallChainingTest.kt` is pinned chain integration evidence | `PARTIAL` | #164; #60/#65; #61 | Accept optional whitespace/continuation separators before `::` while preserving all segment spans; do not infer chain value-flow compatibility | `scribium-quarkdown`; `scribium-markdown` integration / #150 |
| Tight / brace-wrapped calls | Public syntax page; pinned call-pattern wrapper grammar | `H{.text {2}}O` | Top-level and nested tight calls are recognized by the existing grammar and preserve their wrapper and inner/head spans | Top-level and nested wrapper, inner/head, name, argument, and surrounding-text spans pass | `.qd` only; Markdown mode remains ordinary text | UTF-8/CRLF nested-content span coverage is independently asserted | Missing wrapper/inner close is rejected or remains text deterministically | `parses_tight_calls_and_preserves_inner_provenance`, `qd_inline_continuation_and_tight_calls_preserve_text_and_spans`, `audit_preserves_nested_tight_call_wrapper_inside_content_argument`, `audit_preserves_nested_tight_utf8_and_crlf_provenance` | No end-to-end claim; no independent nested-tight conformance fixture | `PARSED_ONLY` | #60/#65 | Preserve original-source wrapper structure without synthetic fragment reparse | `scribium-markdown` |
| Inline vs block placement | Public syntax page; pinned `FunctionCallPatterns.kt` block/inline rule | `Text .foo {x} text` vs `.foo {x}` | Block parser accepts isolated calls; inline parser preserves calls inside paragraphs and trailing same-line text selects inline path | Call/paragraph spans remain source-backed | `.md` has no directive extension; `.qd` body/list/blockquote paths tested | LF/CRLF covered | Malformed inline recovery loses suffix (#159) | `qd_mode_parses_root_and_inline_calls_with_crlf_provenance`, body/container tests | Existing parser evidence only | `PARTIAL` | #60/#65; #159 | Keep placement heuristic independent of evaluator and retain all malformed source | `scribium-markdown` |
| Body argument and dynamic indentation | Public syntax page; pinned grammar body token; body nested-call example | `.foo\n  body\n    .inner` | `QuarkdownBlock` records first qualifying visual indentation, dedent termination, blank lines, nested Markdown/block/inline calls | Body line ranges and nested spans remain original source-backed | `.qd` body paths covered; Markdown mode isolated | LF/CRLF, UTF-8, tab, mixed/deeper indentation tested | One-space body is rejected; malformed header diagnostics are source-backed | `quarkdown_body_uses_first_body_line_indent_not_fixed_width` and body family tests | `call-indented-body-basic` is `Parsed` | `PARSED_ONLY` | PR #54; #60/#65 | Evaluator/body parameter semantics remain #149/#150; raw-body vs structured-body distinction remains documented | `scribium-markdown` |
| Call boundaries and protected contexts | Public call-pattern boundary; Markdown content/code behavior | `word.foo {x}`, `H{.foo {x}}O`, `` `.foo {x}` ``, fenced code, link/label contexts | Rushdown lifecycle plus Quarkdown extensions shields code/verbatim and Markdown mode; the #157 lexer now applies the pinned preceding-byte rule (ASCII alphanumeric, `.`, and `\\` block; non-ASCII surroundings do not) | Existing protected-context spans are source-backed; the #157 call/name/trailing-text spans are exact | `.md`/`.qd` isolation explicitly passes for tested contexts | UTF-8/CRLF context tests pass | Incomplete cases follow current parser diagnostics/literal policy | `markdown_mode_keeps_quarkdown_as_text`, `code_shields_quarkdown_extension`, `crates/scribium-markdown/tests/call_grammar_audit.rs::audit_aligns_call_boundaries_across_utf8_crlf_and_modes`, #60 boundary tests | No exhaustive link-label/emphasis matrix | `PARTIAL` | #60/#65; #157 implemented | Keep protected-context coverage separate from the aligned call-start boundary | `scribium-markdown` |
| Malformed input and recovery | Public grammar requires balanced syntax; Scribium Engineering requires no panic/input loss | `.foo {`, `.foo }`, `.foo {a`, `.foo {a}::` | Structured `E2003`–`E2004` diagnostics are deterministic, but malformed inline call recovery drops trailing source | Diagnostic spans are source-backed; AST source completeness fails for #159 | Code/verbatim shielding remains intact | UTF-8/CRLF diagnostic offsets pass | Root errors diagnose; inline suffix loss is a production gap | `malformed_*` parser tests cover diagnostics, not recovery completeness | No unsupported diagnostic golden for this grammar gap | `PARTIAL` | #60/#65; #159 | Retain all non-call source while preserving diagnostic span and policy | `scribium-markdown` |
| Markdown structure inside content arguments | Public Markdown-content page; public call syntax nested-content contract | `.outer {**bold** and [link](target)}` | Current frontend preserves opaque text and emits `E3010`; it does not preserve inline Markdown nodes | Original opaque span is exact, but structure is not retained | `.md` baseline works; `.qd` content-argument gap is separate | Existing UTF-8/CRLF source span coverage is partial | Delimiter text is retained as text with `E3010` | `content_argument_preserves_original_span_and_reports_markdown_gap` | No positive content-argument fixture | `PARTIAL` | #160; related #154 | Define bounded supported inline subset at existing Rushdown ownership boundary | `scribium-markdown` / #154 |

## Evidence-level separation

| Evidence layer | #148 result |
|---|---|
| Documented upstream behavior | The public syntax and Markdown-content pages define the listed call forms and boundaries. |
| Observed pinned upstream behavior | Pinned lexer/grammar/refiner records confirm identifier, argument, separator placement, continuation, body, wrapper, and chain implementation boundaries; source evidence is not an implementation recipe. |
| Scribium parser acceptance | The rows marked `PARSED_ONLY` are covered by source-backed parser/frontend tests. |
| Source provenance | Accepted call, argument, named-name, segment, body, UTF-8, CRLF, and nested tight wrapper/inner spans are verified; separator-placement truncation, escaped-delimiter truncation, and malformed inline recovery remain non-lossless/pinned-equivalent gaps. Mixed argument order is retained at the grammar/frontend boundary, and the temporary engine guard reports from the actual argument spans before split IR projection; complete binder validation is not claimed. |
| Evaluator semantics | Out of scope for #148. Existing #61/#147 evidence may be linked only at its stated bounded semantic level. |
| IR preservation | #65 structural chain evidence remains valid; #148 adds no IR schema or semantic claim. |
| Backend/output equivalence | Not claimed by this audit. |

## Documentation truthfulness changes

- `docs/SYNTAX.md` now records the pinned call-identifier, numeric-reference,
  call-start boundary, adjacent named-delimiter, and ordered mixed-argument
  contract, while retaining separate notes for the remaining #159, #160, #162,
  and #164 gaps.
- `docs/compatibility/quarkdown/README.md` and `GAP_INVENTORY.md` link this
  audit and no longer present the affected parser rows as unqualified complete
  call-grammar support.
- `SPEC_SOURCES.md` records the pinned `GrammarUtils.kt`,
  `FunctionCallGrammar.kt`, and `RegularArgumentsBinder.kt` evidence used for
  escaped-delimiter and argument-ownership findings without duplicating the
  existing syntax-page record.

## Explicit deferrals and non-goals

- The #157 production change is limited to the `scribium-quarkdown` lexer and
  the necessary `scribium-markdown` lifecycle integration. No evaluator
  semantics or behavior changed; evaluator documentation was updated to
  describe the existing 1-based implicit-reference policy accurately. Binder,
  builtin, IR, Typst, and Rushdown behavior/code remain unchanged.
- No dependency, compatibility target, or upstream baseline changed.
- #149 owns unknown/duplicate/excess/default/optional binding behavior.
- #150 owns lambda binding, call evaluation order, chain value flow, and
  semantic body execution.
- #154 owns broader content/media/Markdown-extension surface follow-up.
- The remaining production gaps are tracked as #159, #160, #162, and #164; they
  are not hidden with expected-failure tests or compatibility allowlists in
  this PR.

[from #147]: https://github.com/luceat-lux-vestra/scribium/issues/147
[from #148]: https://github.com/luceat-lux-vestra/scribium/issues/148
[from #149]: https://github.com/luceat-lux-vestra/scribium/issues/149
[from #150]: https://github.com/luceat-lux-vestra/scribium/issues/150
[from #154]: https://github.com/luceat-lux-vestra/scribium/issues/154
[from #162]: https://github.com/luceat-lux-vestra/scribium/issues/162
[from #163]: https://github.com/luceat-lux-vestra/scribium/issues/163
