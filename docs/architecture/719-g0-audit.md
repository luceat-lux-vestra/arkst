# #719 G0 — executable surface audit and evidence ledger

**Status: unverified candidate, G0 exit NOT YET MET.** Baseline: `main@54a7285ca7adce7482bc27df587f156a6162a3f9`; Git tree `5213795ebd154b25d2e10b199dcd6fcebb625eff`. Parent [#719](https://github.com/luceat-lux-vestra/arkst/issues/719) owns G0–G8. This document inventories the complete **declared** AST/IR variant surface and identified production/host routes; it does **not** claim every dynamic dispatch branch or test path has already been proven.

## Machine-readable authority

[719-g0-execution-surface-inventory.tsv](719-g0-execution-surface-inventory.tsv) has **117 data rows**: **97 enum variants** and **20 cross-layer routes**. Each row gives the declaration path/line, observed current path, state, mandatory migration gate, test/source anchor and outstanding obligation. Test/source anchors indicate where evidence must be established; they do *not* mean the case passed.

| Type | Declared count | Names |
|---|---:|---|
| `ast.Block` | 13 | Heading, Paragraph, Blockquote, UnorderedList, OrderedList, Table, CodeBlock, ThematicBreak, PageBreak, DirectiveCall, Metadata, RawHtml, Unsupported |
| `ast.Inline` | 12 | Text, Emphasis, Strong, DirectiveCall, Link, Image, Code, RawHtml, Strikethrough, HardBreak, SoftBreak, Unsupported |
| `ast.Value` | 8 | String, Number, Boolean, Identifier, Range, Content, InlineBody, Lambda |
| `ast.CallArgument` | 2 | Positional, Named |
| `ir.IrNode` | 17 | Heading, Paragraph, Blockquote, UnorderedList, OrderedList, Table, CodeBlock, RawHtml, TargetSpecificContent, Component, PageMarginContent, FunctionCall, ChainedFunctionCall, FunctionDeclaration, ThematicBreak, PageBreak, Math |
| `ir.IrInline` | 28 | Text, Keybinding, Whitespace, PageCounter, PageNumberFormat, PageNumberReset, LastHeading, Emphasis, Strong, TextSize, TextCase, TextVariant, TextScript, TextColor, Underline, Overline, Strikethrough, InlineCollapse, DirectiveCall, ChainedDirectiveCall, Link, Image, ExplicitError, Code, SoftBreak, HardBreak, RawHtml, TargetSpecificContent |
| `ir.IrValue` | 17 | String, Number, Boolean, Identifier, Size, Color, Enum, Range, Collection, Pair, Dictionary, Content, Component, Unit, None, Callable, InlineBody |
| **Total variants** | **97** | Mechanically extracted from exact pinned AST/IR source |

The inventory was generated from the declarations of `arkst-markdown/src/ast.rs` and `arkst-ir/src/lib.rs`, verified against counts `13/12/8/2/17/28/17`. On any code change affecting these enums, regenerate and diff the ledger; a zero-drift claim without this check is invalid.

## Observed execution-to-output path

```
VirtualProject / SourceStore
  -> parse_with_mode -> frontend AST
  -> core::compile_with_inputs -> Evaluator::evaluate_frontend_*
    -> selected top-level frontend invocation candidates (Phase 3A–3I)
    OR lower_top_level_block / lower_top_level_chain_tail
    -> evaluate_node / evaluate_inlines / evaluate_call_body
    -> callable body Vec<IrNode> / IrCallSegment / IR value conversion
    -> RuntimeValue::SemanticLeaf(IrValue) / InvocationValue
    -> EvaluationContext cloned scope tree + savepoint/document state
  -> IrDocument with completed nodes and state snapshot, but executable variants remain legal
  -> Typst lowering + source maps / inspect JSON via IrDocumentWire
```

Source anchors:
- `crates/arkst-core/src/lib.rs:326–437`: normal compile facade and final `CompileResult.ir`.
- `crates/arkst-engine/src/evaluator.rs:3838–4092`: frontend select/fallback; `:145` callable body; `:20245` IR body; `:1641–1695,2188–2221` cloned scopes/journal; `:985–1010` document state.
- `crates/arkst-engine/src/ast_to_ir.rs:473–625,1034–1160,1195–1250`: structure/inline/value IR bridges including executable lambda and rich content.
- `crates/arkst-engine/src/value_conversion.rs:106–310`: runtime provenance/origin and `SemanticLeaf(IrValue)` / `InvocationValue`.
- `crates/arkst-ir/src/lib.rs:17–79,3757–3810,4019–4040,4260–4300`: executable block/inline/callable and unversioned wire.
- `crates/arkst-typst/src/lowering.rs:19–32`: backend accepts mixed-lifecycle `&IrDocument`; no final-only type boundary.
- `docs/ARCHITECTURE.md:595–622` / ADR-0020: earlier executable one-IR statement conflicts with #719's target.

## Ownership and dependency closure

| Work area | Paths audited | Target gate | Required evidence before acceptance |
|---|---|---|---|
| All top-level block calls, named/positional/mixed args and chains | FrontendInvocation, ArgumentCandidate, IrCallSegment | G1/G3 | Source order, owner precedence, structural preflight, once-only side effects, multi-tail |
| All nested block/inline/link/heading/list/table/content nodes | block_to_ir, inline_to_ir, evaluate_evidenced_structural_nodes | G4 | Direct AST at every depth, source spans and diagnostics, HTML fail-closed |
| Lambda bodies, user function, extension and conditionals | FunctionBinding Vec<IrNode>, evaluate_call_body | G2–G4 | Captured vs caller scope/resource/mode, laziness, atomic rollback |
| Repeated/generated content, #718 tables, #682 structured strings | RuntimeValue, ValueProvenance, target converters | G1/G4 | Per-child provenance, materialized vs source-backed, no producer heuristic |
| Includes/libraries/subdocuments/resources, environment/logger | ResourceProvider/LoadableLibraryProvider, source mode/context | G2/G4 | Explicit capabilities, no project-root escape, independent compiles |
| Document metadata/config | mutable DocumentState -> IrDocumentState | G5 | One final immutable config; defaults and rollback parity |
| Final IR, pure Typst, native/in-process | IrNode/IrInline/IrValue, lower_to_typst | G6/G8 | No executable syntax allowed by type, output/source-map parity |
| Wire, CLI inspect, public AST-to-IR/evaluate facade | IrDocumentWire, CLI, core legacy wrappers | G7 | Versioned DTO, legacy accept/reject matrix, malicious data tests, deprecation |
| CI + reference | Merge Gate, Reference JVM, platform tests | G0/G8 | Exact-head CI, Linux ARM/macOS/Windows, WASM, clean-room oracle |

## Baseline test/source anchors to expand, not falsely treat as PASS

- `arkst-markdown/tests/call_grammar_audit.rs`; `content_inline_markdown.rs`
- `arkst-core/tests/quarkdown_nested_call_body_690.rs`, `quarkdown_function_extension.rs`, `quarkdown_value_model_audit.rs`, `ir_facade.rs`
- `arkst-engine/tests/resource_contract.rs`, `global_resource_boundary.rs`, `environment_inputs.rs`; `arkst-core/tests/resource_permission_policy_296.rs`
- `arkst-ir/tests/code_callouts_serde.rs`; `arkst-typst-subprocess/tests/lowering_integration.rs`, `backend_integration.rs`; `arkst-typst-inprocess/tests/backend_parity.rs`
- `.github/workflows/ci.yml` required Merge Gate and `.github/workflows/reference-jvm.yml`. Latest Phase 3I proof: [CI 42751293944](https://github.com/luceat-lux-vestra/arkst/actions/runs/42751293944), **not** a proof of this candidate.

## Open blockers for the **G0 gate** (no deferral or scope reduction)

1. Trace every actual producer/consumer/call site of all ledger types, including nested/generative, extension/native precedence, legacy/core, WASM/host and test-only fixtures. Attach specific executable test IDs/results rather than treating path anchors as proof. The enum list is declaration-complete, not yet an exhaustive runtime branch proof.
2. Freeze independent clean-room representative inputs and byte-exact expected diagnostic code/message/span/order, effects, lazy branches/failed mutations, resource modes, source provenance, output and source-map behavior. Preserve known PARTIAL/unsupported/fail-closed cases.
3. Decide public API and CLI/wire compatibility/version policy from consumer inventory; test legacy decoder behavior and invalid source refs rather than assuming compatibility.
4. Review and accept [proposed ADR-0022](../adr/0022-runtime-final-document-ir.md); explicitly amend historical ADR-0020 and `docs/ARCHITECTURE.md` after acceptance, without silently rewriting their prior claims.
5. Submit the documentation candidate as a PR and verify exact-HEAD required CI/docs policy. **No G0 exit before this is green and merged/reconciled.**

**Durable STOP:** documentation candidate HEAD only. No source implementation, PR, CI, merge, or claim that G0 is completed. #719 remains OPEN.

## Exact runtime caller/test evidence (G0 proof slice; NOT G0 complete)

Companion [runtime test evidence matrix](719-g0-runtime-test-evidence.tsv) indexes **44 exact `#[test]` functions from 16 verified source files** at original PR HEAD `0b6143a9e95f76ac3913a1d7acea171670d1b880`. Names and line numbers were read from exact source (each has a `#[test]` annotation and assertion). Each matrix entry records the entrypoint and **specific asserted subject**, not merely a test filename. These anchors are a selected baseline, **not** coverage proof for every dynamic runtime path or all 97 declared variants.

### Call chain and bridge sites established from code

1. Normal frontend: `arkst-markdown/src/parser.rs:756` -> `arkst-core/src/lib.rs:326-422` -> `Evaluator::evaluate_frontend_*` -> `evaluator.rs:3838-3892`. Selected top-level calls enter `FrontendInvocation`; unselected blocks still enter `ast_to_ir::lower_top_level_block` at `evaluator.rs:3867`, then `evaluate_evidenced_structural_nodes` at 3875.
2. Direct-call selection: `evaluator.rs:3935-3996` uses Phase-3A source candidates only for selected owners; otherwise `ast_to_ir::lower_top_level_direct_call` feeds executable IR arguments to `evaluate_block_call`. Chain route `evaluator.rs:3999-4090` still lowers tail IR at 4021 and can fall back to `evaluate_block_chain`; `continue_chain_from_value` at 19904 operates on `IrCallSegment`. Unit test `phase3i_native_tail_falls_back_to_pre_lowered_chain` specifically verifies this remaining fallback.
3. Nested content: `ast_to_ir.rs:473` block conversion, `:1034` inline conversion, `:1195` value conversion, `evaluator.rs:4521` structural evaluation, `:4765-4779` inline execution. `FunctionBinding.body: Vec<IrNode>` at `evaluator.rs:145` and `evaluate_call_body` at 20245 continue executing IR bodies. This is **not** direct AST at every depth.
4. Values and state: `value_conversion.rs:166-310` has `RuntimeValueKind::SemanticLeaf(IrValue)` and `InvocationValue`; `evaluator.rs:1641-1695,2188-2221` maintains cloned parent-scope/context and journal state. `final_document_guard_detects_nested_block_and_inline_executable_state` tests a **runtime guard**, not a final-only Rust type.
5. Final consumers: `arkst-ir/src/lib.rs:33-79` serializes via unversioned `IrDocumentWire`; `arkst-cli/src/commands.rs:1599` writes `result.ir` JSON; `arkst-typst/src/lowering.rs:23` still accepts unqualified `&IrDocument`. No type-enforced final-only or versioned-wire contract exists.
6. **Fixture entrypoint matters:** `arkst-engine/tests/{logger_diagnostics,loadable_library_dispatch,environment_inputs}.rs` call `ast_to_ir_with_diagnostics_for_mode` and `Evaluator::evaluate*(IrDocument)`. They test the **legacy executable IR entrypoint**, not normal frontend-AST execution. Other rows are separately marked parser-only, evaluator-private unit, normal `arkst-core::compile`, wire or backend.

### CI provenance and interpretation

[PR #746 initial exact-HEAD CI #42770631592](https://github.com/luceat-lux-vestra/arkst/actions/runs/42770631592) passed including `Merge Gate`. Linux ARM, Windows and macOS job-step readback confirmed **`Test (Typst required)` ran and completed successfully** and `Documentation-only fast path` was skipped. The pinned workflow `ci.yml:395` invokes `cargo test --locked --workspace --all-targets --all-features`. Thus the baseline includes a genuine workspace test suite on the **previous HEAD**, but the matrix does not supply per-test job logs or an independent clean-room oracle. A new docs candidate HEAD must receive its own CI check.

### Unresolved G0 proof requirements retained without scope reduction

- Trace and classify **every producer/consumer** of all AST and IR variants, including all nested/generated sites and legacy API consumers; selected 44 fixtures are not exhaustive proof.
- Build independent clean-room parity fixtures for rich/multi-tail/named/terminal-body AST routes, all function/extension/conditional owners, diagnostic codes/messages/source spans and state side effects versus the old IR path.
- Preserve source vs materialized recursive per-element provenance for #682 structured String and #718 source-backed table; existing conversion tests cannot close these integration gaps.
- Freeze cross-provider/subdocument/closure and nested transaction oracle cases under normal frontend and legacy entrypoints, plus source-map/backend parity and size/recursion limits.
- Define legacy IR public API/CLI/Wire compatibility, versioning, decode rejection and malicious payload tests; review ADR-0022 and update historical ADR-0020/architecture text before claiming G0 accepted.

**STOP:** this entrypoint-aware evidence slice is a documentation candidate only. It does not create new tests, finish G0, change code, or justify G1 start. The next checkpoint must check the replacement PR HEAD and authoritative CI.

## Full Rust production-source lexical variant census and differential-oracle contract (G0 candidate)

The [production variant cross-reference ledger](719-g0-production-variant-crossrefs.tsv) read **all 48 `crates/*/src/**/*.rs` files** at exact frozen source HEAD `c24ae20d9fb6898ef82faf5ddc0f4aa40ae03e5a` (each source blob SHA recorded in 48 `FILE_SCAN` rows, even when no explicit variant references appear). It records **454 variant/file pairs with 4,068 matching source-line references** across all **97 declared AST/IR variants**. All 97 types have at least one matching explicit `Enum::Variant` spelling; `FILE_SCAN` + `ENUM_REF` rows total **502** (plus TSV header). The matcher covers direct `IrNode/IrInline/IrValue`, frontend `Block/Inline/Value/CallArgument`, and recognized `Frontend*` aliases, with exact line locations. This is a reproducible, code-wide **lexical census**, **not** exhaustive semantic reachability: comments, in-module tests and dead code may contribute matches; aliases not recognized by the matcher, destructuring without enum prefixes, type-erased factories, and dynamic dispatch need independent review.

**Stage ownership cross-check:** `arkst-markdown/src/parser.rs` produces parsed AST; `arkst-engine/src/ast_to_ir.rs` consumes AST and creates executable IR; `arkst-engine/src/evaluator.rs` has partial direct-AST calls, remaining pre-lowered body/chain execution and final semantic results; `arkst-ir/src/lib.rs` contains in-memory executable variants plus wire conversions; `arkst-typst/src/lowering.rs`, `focus_lowering.rs` and `arkst-cli/src/commands.rs` consume IR. Neither file-reference density nor an empty `ENUM_REF` row establishes whether a source file is a runtime consumer via traits, accessors or type-erased helpers. This distinction is part of G0's required review, not a reason to remove those routes.

The [differential oracle contract](719-g0-differential-oracle-contract.tsv) adds **32 separately numbered proposed oracle probes** with explicit input/setup, comparison paths, observable dimensions (diagnostics code/message/span/order; emitted nodes; mutation journal; source identities; source maps; host permissions; wire shape), provisional compatibility invariants, existing test anchors and G0/G1–G8 ownership. Every row is marked **`PENDING_REFERENCE_FREEZE_AND_NEW_EXECUTION`**. Existing tests are included only as anchors; they are **not** an independent oracle. For upstream-supported Quarkdown constructs, freeze independently recorded outputs from a pinned clean-room reference; for Arkst-specific policy, freeze independently reviewed current behavioral contracts before allowing new AST routes to supersede legacy ones. Never fabricate a PASS from the fixture design.

**Acceptance limitations:** the 48/48 source census closes a *lexical source enumeration* item, not the entire G0 runtime producer/consumer proof; complete each `role_tier` with branch reachability, actual producer/consumer direction and test IDs. The 32-oracle design closes a *test-plan specification* item, not observed differential parity. G0 remains blocked on executable new independent oracle proofs, public API/CLI/legacy wire compatibility decision, ADR-0022 acceptance and historical architecture reconciliation. #682/#718 provenance/conversion boundaries remain mandatory. The prior 44 anchored tests and their previous green CI remain a separate baseline, not validation of this candidate.

**Checkpoint STOP:** documentation candidate only. Do not declare G0 accepted, begin G1, merge #746, or close #719 until explicit gate exit evidence exists.

## First executable normal-frontend vs legacy-IR differential bundle (G0 candidate)

A first **executable differential fixture bundle** now lives in `crates/arkst-core/tests/g0_runtime_differential.rs`. It runs the same parser-clean Quarkdown source through (a) the normal public `arkst_core::compile` frontend path and (b) an explicit legacy pipeline `parse_with_mode -> ast_to_ir_with_diagnostics_for_mode -> core evaluator::evaluate_project(IrDocument)`, then requires exact final `IrDocument` equality and a normalized exact diagnostic signature (code, severity, message, primary/secondary spans and hints).

The bounded bundle implements three previously planned oracle rows:

- **S04** source-defined direct calls with required/optional/named binding.
- **S12** failed extension mutation of document state, requiring rollback and identical diagnostic/output.
- **S16** dynamic String-to-number conversion versus source-authored static String rejection.

These fixtures establish **normal-vs-legacy differential assertions in executable test code**. They do **not** independently prove Quarkdown/reference correctness. S04 and S16 still require their independent-reference observations to be reconciled into this G0 contract before their final reference disposition is claimed. S12 is intentionally different: the independently observed upstream behavior conflicts with Arkst's accepted transactional invariant and is recorded below as an explicit compatibility divergence. These fixtures also do not close unimplemented S01–S32 cases or the G0 dynamic route census.

### S12 independent-reference disposition: intentional rollback divergence

Disposable clean-room PR #748 at exact HEAD `347b5a155fdb5ed11e6113e9aa29cc7bfd2ec803` ran the official Quarkdown v2.5.1 Linux x64 release artifact (archive SHA-256 `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`) as a black-box oracle. Workflow run #42825801315 completed successfully and retained artifact ID `13768992195` with digest `sha256:751d3386190fb2f5a33d702a2907a16321b87acb5eb1fcceddf96b8d785454a0`.

For the S12 fixture, official v2.5.1 **non-strict** reports the failing `.sum {true} {2}` but leaves the preceding `.docname {changed}` mutation observable; the final document-name observation is `changed`. Strict mode exits with code 66 before a post-failure state can be observed. This contradicts Arkst's established rollback behavior, where a failed invocation/body restores evaluator/document mutations to its savepoint while previously committed successful outer mutations remain committed.

The divergence is **intentional and reviewed**, not a compatibility bug to repair by weakening Arkst's transaction semantics. The decision is anchored by completed programmable-language audit #150, shared atomicity owner #167, the extension regression family in `quarkdown_function_extension.rs`, #719's retained first-write/savepoint transaction direction, and accepted ADR-0016's requirement to document/review intentional divergences and invariant weakening.

Accordingly, S12 has two distinct authorities:

- **Arkst migration oracle:** normal/direct-AST and legacy evaluator paths must preserve Arkst's failure-atomic rollback result (`before`) and the boundary that earlier successful commits survive.
- **Upstream compatibility reference:** official Quarkdown v2.5.1 non-strict preserves the failed invocation's prior document mutation (`changed`).
- **Compatibility classification:** explicit intentional divergence. Upstream-output equality is not the S12 acceptance predicate.

The S12 row in the differential-oracle contract is therefore frozen as an Arkst migration oracle with an explicit upstream divergence, pending exact-HEAD CI for this documentation candidate. This does not complete G0 or relax any remaining G0 proof requirement.

**STOP boundary:** S12 evidence-document reconciliation only. Exact-head CI, additional oracle fixtures, ADR acceptance, merge and later G0 work remain separate work units.
