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

1. Continue tracing every actual producer/consumer/call site of all ledger types, including nested/generative, extension/native precedence, legacy/core, WASM/host and test-only fixtures. Dynamic-route slices A and B below prove five concrete runtime routes with exact current-head execution evidence; they do **not** make the remaining routes exhaustive. The enum list is declaration-complete, not yet an exhaustive runtime branch proof.
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

### Dynamic producer/consumer reachability — bounded slice A

Companion [dynamic-route evidence ledger](719-g0-dynamic-route-evidence.tsv) closes a **bounded part** of G0 blocker 1 by tying three transitional runtime routes to exact production branches and exact current-head test execution. It does not promote the remaining inventory routes to proven status.

Production authority for this slice is unchanged from the current PR candidate: `crates/arkst-engine/src/evaluator.rs` blob `217d3b4c107119c9ffe81daa303bcac0883d2675` and `crates/arkst-engine/src/ast_to_ir.rs` blob `54802c362b3d3584fc25a1527f781dc0fc8f6377`. Exact execution authority is CI run **#42901128320** at `e11e6362c1ae58b33eb63dfa505647f3b81f0464`, with test jobs macOS `130127146005`, Windows `130127146163`, and Ubuntu ARM64 `130127146132`.

| Route | Production reachability established | Exact executable evidence | Current classification |
|---|---|---|---|
| **DYN01 direct source-defined call** | `evaluate_frontend_with_context` recognizes `FrontendInvocation`; the `DirectCall` branch attempts AST-backed `evaluate_frontend_block_call` before the explicit `lower_top_level_direct_call -> evaluate_block_call` fallback. | `g0_s04_source_defined_direct_calls_match_legacy_ir_entrypoint`; `phase3a_frontend_candidates_preserve_syntax_and_mixed_source_order` — both PASS on macOS/Windows/Ubuntu ARM64 at #42901128320. | **Branch-proven for the selected source-defined direct-call slice**, not all direct-call owners/shapes. |
| **DYN02 chain head + executable IR tail** | The `Chain` branch can keep the selected head in frontend syntax while `lower_top_level_chain_tail` materializes `IrCallSegment`; `continue_chain_from_value` consumes those segments and explicitly retains pre-lowered/native fallback ownership. | Phase 3E/3F/3G/3H/3I tests for shared continuation, terminal body parity, multi-tail parity, binding failure parity, predecessor ordering, flat-tail parity and native fallback — **7/7 PASS on all three platforms**. | **Mixed AST/IR transitional route dynamically proven**; its existence is a G3 migration obligation, not final architecture. |
| **DYN03 callable / selected body executable IR** | `FunctionBinding.body` is physically `Vec<IrNode>`; selected frontend block bodies cross `lower_frontend_block_body_for_runtime`; `evaluate_call_body` consumes block `IrNode` via `evaluate_node` and inline IR via `evaluate_inlines`. | First-class callable capture, two nested-body frontend tests, extension document rollback and lazy native raw-body tests — **5/5 PASS on all three platforms**. | **Executable IR body storage/consumption dynamically proven**; this directly demonstrates the remaining G4 body bridge. |

This slice upgrades those three routes from lexical/path anchors to **source-traced + executable-current-head evidence**. It intentionally leaves open: exhaustive direct/native/unresolved invocation ownership, every chain argument/body/extension/resource combination, all nested/generative AST owners, value/runtime provenance paths, context/transaction routes, host/WASM routes, public API/wire consumers and backend consumers.

### Dynamic producer/consumer reachability — bounded slice B

Slice B extends the same [dynamic-route evidence ledger](719-g0-dynamic-route-evidence.tsv) with two additional runtime routes: evaluator-owned runtime value/provenance and invocation-scoped context/transaction journaling. This is still a bounded proof slice, not exhaustive G0 reachability.

Production authority used by this slice:
- `crates/arkst-engine/src/value_conversion.rs` blob `c47164b1cc2524a3cf79c615e83058d8b6cd72a9`;
- `crates/arkst-engine/src/evaluator.rs` blob `217d3b4c107119c9ffe81daa303bcac0883d2675`;
- `crates/arkst-engine/src/builtins.rs` blob `35369890b3081c4e7359656f561f5369bb6d9333`.

Execution authority is exact-head CI run **#42906026861** at `04a7ee7a56cb656a29edc7f1f4e25a930a607767`, with macOS job `130144201000`, Windows job `130144200964`, and Ubuntu ARM64 job `130144200908`.

| Route | Production reachability established | Exact executable evidence | Current classification |
|---|---|---|---|
| **DYN04 runtime value / provenance** | `evaluate_argument_candidate` materializes evaluator-owned `RuntimeValue`; `RuntimeValue::collection` decomposes collection children so provenance is owned per element. Variables retain `RuntimeValue`; the direct logger route recovers it with `direct_variable_runtime_value` and `builtins::logger_string_conversion` consumes the bounded source-backed provenance. Operation-generated collections remain fail-closed. | RuntimeValue nested-collection provenance; logger dynamic-string boundary; direct Markdown-list source-origin conversion; generated-collection fail-closed behavior; provenance confinement; dynamic-vs-static conversion audit — **6/6 PASS on all three platforms** at #42906026861. | **Runtime provenance production/consumption dynamically proven for this bounded value/logger slice.** `SemanticLeaf(IrValue)` and `InvocationValue` remain transitional adapters, so G1 is not closed. |
| **DYN05 context / transaction savepoint** | `evaluate_call_value_with_input` begins an invocation and captures `InvocationCheckpoint`; failure calls `checkpoint.restore -> rollback_transaction -> restore_undo`, success calls `checkpoint.commit -> commit_savepoint`. `record_variable_before` and `record_document_state_undo` journal first writes, while cloned scope trees share the transaction with distinct scope identity. | Nested candidate rollback; caller-variable rollback; document-state rollback; prior-success preservation; nested-super outer rollback; local/parent function-binding undo; unresolved-callable non-publication; bounded document append undo metadata; nested conversion rollback — **9/9 PASS on all three platforms**. | **Invocation savepoint/journal route dynamically proven for the tested variable/function/document-state boundaries.** This preserves the reviewed S12 invariant but does not finish the target EvalSession/stable-identity transaction design. |

Across slice B, **15/15 exact tests pass on macOS, Windows and Ubuntu ARM64** at the cited exact execution HEAD.

This slice upgrades the runtime-value and transaction routes from structural anchors to **source-traced + executable evidence**. It intentionally leaves open: full recursive provenance across every semantic category and consumer, #682/#718 integration closure, every conversion target, all scope/resource/capture combinations, all document-state fields, host/WASM boundaries, public API/wire consumers and backend consumers. The new documentation commit that records this slice still requires its own exact-head CI before merge.

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

These fixtures establish **normal-vs-legacy differential assertions in executable test code**. They do **not** by themselves prove Quarkdown/reference correctness. Disposable clean-room PR #748 now supplies the independent official-v2.5.1 observations for S04, S12 and S16: S04 and S16 are semantically compatible at the tested boundaries, while S12 intentionally diverges because Arkst preserves its accepted transactional rollback invariant. These fixtures still do not close unimplemented S01–S32 cases or the G0 dynamic route census.

### S04 and S16 independent-reference disposition: semantically compatible

The same disposable clean-room PR #748 at exact HEAD `347b5a155fdb5ed11e6113e9aa29cc7bfd2ec803`, workflow run #42825801315, used the official Quarkdown v2.5.1 Linux x64 archive with SHA-256 `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`. The retained artifact is ID `13768992195`, digest `sha256:751d3386190fb2f5a33d702a2907a16321b87acb5eb1fcceddf96b8d785454a0`.

For **S04**, official v2.5.1 default and strict modes both exit 0 with empty stderr and the same visible output: `Quarkdown Hello, world from None! Hello, world from Jane!`. The rendered HTML SHA-256 is `3f4c6d0db35eccc030dc107e0ded16fe6004e4b5aeb56c02c6aa37dc8cd010c8` in both modes. This independently supports the required/optional/named source-defined direct-call behavior exercised by Arkst's S04 differential fixture.

For **S16 dynamic String**, official default and strict modes both exit 0, produce visible `Quarkdown 3.5`, and share HTML SHA-256 `97cbf5ab43bce37796fb200cab5635d83597fadd92b9df46a5a7943001510645`. For **S16 static String**, official non-strict mode reports that `abs(Number x)` received a `StringValue` instead of Number, renders an error document with SHA-256 `f2a72851f31038b0c44c786fd653571ec4a9120d6e36bc2a9014e8f42574dda6`, and strict mode exits 66 with no HTML. This independently supports the semantic conversion boundary: dynamically produced numeric text converts, while a statically typed String remains rejected by a Number target.

The S16 compatibility claim is deliberately **semantic**, not byte-identical diagnostic-presentation parity. Arkst's diagnostic code/message/rendering contract remains its own tested interface; the official CLI presentation and JVM exception naming are not imported as Arkst's byte-for-byte oracle.

Accordingly:
- **S04:** upstream reference frozen as semantically compatible at the required/optional/named direct-call boundary.
- **S16:** upstream reference frozen as semantically compatible at the dynamic-vs-static numeric conversion boundary.
- Their independent-reference plus Arkst migration evidence was validated at exact HEAD `4ffe231011688997788c9fed751695201c0983bb` by CI run #42833994417: required Merge Gate succeeded and S04/S16 passed on macOS, Windows and Ubuntu ARM64. This freezes the evidence disposition; it does not close wider G0 coverage. Any later documentation candidate, including the commit that records this wording, still requires its own exact-HEAD CI before merge.

### S12 independent-reference disposition: intentional rollback divergence

Disposable clean-room PR #748 at exact HEAD `347b5a155fdb5ed11e6113e9aa29cc7bfd2ec803` ran the official Quarkdown v2.5.1 Linux x64 release artifact (archive SHA-256 `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`) as a black-box oracle. Workflow run #42825801315 completed successfully and retained artifact ID `13768992195` with digest `sha256:751d3386190fb2f5a33d702a2907a16321b87acb5eb1fcceddf96b8d785454a0`.

For the S12 fixture, official v2.5.1 **non-strict** reports the failing `.sum {true} {2}` but leaves the preceding `.docname {changed}` mutation observable; the final document-name observation is `changed`. Strict mode exits with code 66 before a post-failure state can be observed. This contradicts Arkst's established rollback behavior, where a failed invocation/body restores evaluator/document mutations to its savepoint while previously committed successful outer mutations remain committed.

The divergence is **intentional and reviewed**, not a compatibility bug to repair by weakening Arkst's transaction semantics. The decision is anchored by completed programmable-language audit #150, shared atomicity owner #167, the extension regression family in `quarkdown_function_extension.rs`, #719's retained first-write/savepoint transaction direction, and accepted ADR-0016's requirement to document/review intentional divergences and invariant weakening.

Accordingly, S12 has two distinct authorities:

- **Arkst migration oracle:** normal/direct-AST and legacy evaluator paths must preserve Arkst's failure-atomic rollback result (`before`) and the boundary that earlier successful commits survive.
- **Upstream compatibility reference:** official Quarkdown v2.5.1 non-strict preserves the failed invocation's prior document mutation (`changed`).
- **Compatibility classification:** explicit intentional divergence. Upstream-output equality is not the S12 acceptance predicate.

The S12 row in the differential-oracle contract is therefore frozen as an Arkst migration oracle with an explicit upstream divergence. Exact HEAD `4ffe231011688997788c9fed751695201c0983bb` was validated by CI run #42833994417: required Merge Gate succeeded and S12 passed on macOS, Windows and Ubuntu ARM64. This freezes the S12 reference/migration evidence disposition; it does not complete G0 or relax any remaining G0 proof requirement. Any later documentation candidate, including the commit that records this wording, still requires its own exact-HEAD CI before merge.

**STOP boundary:** S04/S12/S16 evidence-status reconciliation only. CI for the replacement documentation HEAD, additional oracle fixtures, ADR acceptance, merge and later G0 work remain separate work units.

## S13/S14/S15 executable migration differential — bounded G0 evidence

Three additional executable migration oracles were added to `crates/arkst-core/tests/g0_runtime_differential.rs` at candidate `802d92bae540d23b17772f36031aafbb6992f97f`. As for S04/S12/S16, the helper compiles exactly the same parser-clean source through normal public `arkst_core::compile` and the legacy `parse_with_mode -> ast_to_ir_with_diagnostics_for_mode -> Evaluator::evaluate_project(IrDocument)` entrypoint, requiring exact final `IrDocument` equality and normalized diagnostic equality including code, severity, message, primary/secondary spans and hints.

| Scenario | New differential test | Bounded observed assertion |
| --- | --- | --- |
| **S13** nested transaction rollback | `g0_s13_nested_super_success_rolls_back_after_outer_failure` | A nested successful `.super` side effect is undone after its outer invocation fails, with exactly one numeric-argument diagnostic and final visible state `before`. |
| **S14** native raw body laziness | `g0_s14_extended_native_raw_body_stays_lazy_and_matches_legacy` | Apparent `.state {changed}` source inside an extended native `.lowercase` raw body stays opaque text, and the later state remains `before`, without diagnostics. |
| **S15** nested source-defined body | `g0_s15_source_defined_nested_body_matches_legacy` | A nested indented body passed to a source-defined `.echo` inside `.text` yields exactly `nested`, without diagnostics. |

**Execution evidence:** [CI #42952023697](https://github.com/luceat-lux-vestra/arkst/actions/runs/42952023697), exact `head_sha=802d92bae540d23b17772f36031aafbb6992f97f`, completed **SUCCESS**. The macOS, Windows and Ubuntu ARM64 `test` jobs each completed successfully, including `Test (Typst required)` (full workspace `cargo test --locked --workspace --all-targets --all-features`). Required `Merge Gate` also succeeded. These three newly added, non-ignored integration tests were included in that full-workspace test run.

**Classification:** `ARKST_MIGRATION_CROSS_PLATFORM_PROVEN_UPSTREAM_REFERENCE_PENDING` for S13/S14/S15 only. These test inputs are adapted from already-established Arkst regression cases; there is **no independent official Quarkdown reference observation** for these three scenarios, and no new upstream compatibility claim. S13's accepted Arkst failure-atomic policy remains distinct from the upstream divergence already documented for S12.

**G0 remains incomplete:** this slice does not resolve remaining S01–S32 oracle rows, exhaustive dynamic producer/consumer reachability, public API / versioned wire / CLI policy, proposed ADR-0022 acceptance, or ADR-0020 / architecture reconciliation. No G1 or merge authorization follows from the green CI run.

## S05/S06/S07 executable normal-vs-legacy migration proof

At test candidate `ecf486cf27b575b08f0c04d0f454ad8c8c5a8919`, three additional parser-clean sources were added to `crates/arkst-core/tests/g0_runtime_differential.rs`. Their common oracle requires complete `IrDocument` equality and exact normalized diagnostic equality (code, severity, message, primary/secondary source spans and hints) between the public frontend `compile` entrypoint and the legacy AST→IR→Evaluator entrypoint.

- **S05 native direct fallback:** `g0_s05_native_direct_fallback_matches_legacy_ir_entrypoint` covers successful native `.sum {1} {2}` (visible `3`) and a native `.sum` call with a raw block body that produces one numeric-argument diagnostic. The native-owner selector denies the Phase 3A source-defined candidate path and retains the `lower_top_level_direct_call -> evaluate_block_call` executable-IR bridge.
- **S06 zero-argument source-defined chain tail:** `g0_s06_zero_argument_chain_tail_matches_legacy_ir_entrypoint` covers a source-defined head with one bodyless terminal tail and no authored tail arguments, returning the predecessor `7`. This is a bounded Phase 3H parser-to-runtime parity check; the existing evaluator unit tests separately inspect the evaluated predecessor candidate's slot/origin.
- **S07 one-flat-argument source-defined chain tail:** `g0_s07_flat_argument_chain_tail_matches_legacy_ir_entrypoint` covers the same head followed by one flat positional authored tail argument, returning `9`. This is a bounded Phase 3I parity check; the existing evaluator unit tests separately inspect predecessor slot 0 and authored slot 1 including source order and spans.

**Execution:** [CI #42953638203](https://github.com/luceat-lux-vestra/arkst/actions/runs/42953638203) at exact HEAD `ecf486cf27b575b08f0c04d0f454ad8c8c5a8919` completed **SUCCESS**. All three native platform `test (Typst required)` suites (macOS, Windows, Ubuntu ARM64) completed **SUCCESS**, as did the required `Merge Gate`, MSRV, Clippy, WASM, docs, compatibility and pinned Reference JVM verification.

**Classification:** S05–S07 = `ARKST_MIGRATION_CROSS_PLATFORM_PROVEN_UPSTREAM_REFERENCE_PENDING`. No fresh independent official Quarkdown black-box reference has been captured for these three test cases, so they do not establish new upstream semantic compatibility. S05 is not an exhaustive native-call matrix; S06/S07 do not exhaust all chain owners, side effects, rich bodies, named/multiple authored arguments, source modes or resource contexts. DYN06 in the route ledger records only the native-direct fallback; S06/S07 extend the already-recognized DYN02 chain route rather than inventing a new architectural owner.

**G0 exit remains blocked:** additional dynamic route closure, remaining S01–S32 oracles, public API/CLI/versioned wire decision, proposed ADR-0022 acceptance, ADR-0020/architecture reconciliation and final CI/merge acceptance are still outstanding. No G1 authorization or merge claim is made by this evidence slice.

## Maintainer decision approved — 2026-11-11

The [G0 maintainer decision record](719-g0-maintainer-decision-brief.md)
replaces the earlier compatibility-preserving proposal. ADR-0022 is
accepted as the target: **do not maintain historical Arkst Rust API/IR JSON
backward compatibility**; remove old public executable-IR interfaces,
reject old unversioned JSON, emit only the new strictly validated wire v1,
and carry out all G0–G8 work without cost-based scope reduction.

This is an **architecture decision, not a completed implementation or G0
gate exit**. Do not confuse Arkst historical API breakage with permitted
degradation of Quarkdown language compatibility, source maps, diagnostics,
rollback semantics or security. Keep PR #746 unmerged until its G0
evidence and CI obligations are complete.

## S01 source-order and later binding failures — pinned official reference

Disposable clean-room PR #754 is **CLOSED/UNMERGED**. Final probe HEAD `5866a0d6f538029de71c080afd7b4e11b2433953`; authoritative run #43057680381 completed **SUCCESS** using the exact official Quarkdown v2.5.1 Linux x64 archive SHA-256 `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`. Retained artifact `g0-s01-quarkdown-v2.5.1`, ID `13872356098`, digest `sha256:69db66203158e2f5b96172b071557084258df8b8ccfe54452152bf190e4a34f4`.

The fixtures are independently authored and intentionally make argument ordering observable rather than relying on the names of existing Arkst tests.

| Fixture | Official default | Official strict | Bounded semantic observation |
|---|---|---|---|
| valid positional → named mixed call with two stateful nested `tick` calls | rc 0; stderr empty; visible target result `12`, then final `count=2`; HTML SHA-256 `73f3849d26bddb9e0d62261820b7514b02e2312da75a8d1fea715be4812d22a1` | rc 0; same HTML hash | ordinary eager candidates evaluate left-to-right in source order; first nested call returns 1, second returns 2 |
| `first:{.sum {true} {2}} {.tick}` | rc 0; failure owned by nested `sum`; recoverable error HTML plus later visible `0`; SHA-256 `8fd250442b361d59715fc5d26e7ad6f790a5b01d59238e05a08ad28337f17d14` | rc 66; owner `sum`; no HTML | the earlier named candidate is evaluated and fails before the later positional-after-named defect becomes the first observable error |
| `first:{.sum {true} {2}} first:{.tick}` | rc 0; same nested `sum` failure and later visible `0`; same bounded failure HTML hash | rc 66; owner `sum`; no HTML | the earlier candidate failure likewise precedes discovery of the later duplicate-name binding defect |

### S01 Arkst reconciliation

The prior ledger sentence **“preflight validation precedes argument effects” is not a valid universal Quarkdown-compatibility invariant**. It described current Arkst's shared structural-plan architecture, not the complete official v2.5.1 ordering contract.

Current Arkst intentionally constructs a structural binding plan before eager candidate evaluation. The new #746 public differential candidates therefore separate two claims:

- `g0_s01_mixed_arguments_evaluate_left_to_right_matches_legacy` expects the compatible valid-mixed observation: target `12`, final count `2`;
- `g0_s01_positional_after_named_preflights_before_nested_failure_in_arkst` and `g0_s01_duplicate_named_preflights_before_nested_failure_in_arkst` freeze current Arkst's different failure ordering, exact source provenance and normal-frontend vs legacy-evaluator parity.

Those tests were added at `b6cf594b46dfb4d529fb3020d070b32adf28c489`; duplicate diagnostic wording/provenance was pinned at `313331d4a6d616ad2ec9bdebba836a7689800204`. Exact execution HEAD `313331d4a6d616ad2ec9bdebba836a7689800204` completed CI #43058559999 **SUCCESS**: Ubuntu ARM64, macOS, and Windows Typst-required jobs plus the required Merge Gate all succeeded. The valid mixed slice is therefore bounded-compatible evidence; the two preflight-first failure cases are cross-platform-frozen Arkst migration evidence for the open #755 compatibility debt.

**Compatibility owner:** #755, `compat(runtime): match source-ordered argument evaluation before later binding failures`. This is **compatibility debt, not an accepted divergence**. The required future change must preserve source-ordered/incremental regular argument processing without replacing it with a blanket “evaluate everything before validation” rule, and it must preserve #167 rollback/commit guarantees. Production semantics are intentionally not changed in evidence-only PR #746.

## S08 mixed-owner multitail + terminal body — bounded cross-platform proof (not G0 exit)

- **New full-frontend fixture:** `g0_runtime_differential::g0_s08_multitail_terminal_body_source_native_source_matches_legacy` compiles parser-clean source containing a source-defined `identity` head, native `multiply` middle tail, and a source-defined `tail` owning the terminal indented body.
- **Oracle comparison implemented:** `compile_and_assert_legacy_parity` checks exact `IrDocument` plus ordered diagnostics (`code`, `severity`, `message`, primary/secondary source spans, `hints`) against the separate legacy AST-to-executable-IR evaluator path. The tested visible terminal content is `terminal body`.
- **Precise dynamic claim if CI passes:** normal public source compilation traverses a multi-tail, mixed-owner continuation and binds the terminal block body at the final source-defined owner without a frontend/legacy output difference. This extends the already known **DYN02** path; it is **not** a new architecture route ID.
- **Authority limit:** the engine-internal `phase3g_multitail_final_body_matches_legacy_chain_result` remains a bounded unit anchor, not an independent upstream oracle. The new public full-frontend S08 path passed [CI #43039010366](https://github.com/luceat-lux-vestra/arkst/actions/runs/43039010366) at exact HEAD `6b945dcd45266ab9d80b6e796edcca603417fdd9`: all three OS full test jobs and required Merge Gate **SUCCESS**. At that earlier test HEAD no independent Quarkdown observation had been captured. The pinned official S08 run is now recorded below; error-first-tail variants and rich body/resource-context matrix remain open. The true counter-based once-only fixture added at `ad7c61d4063f5f1850c09dfc2a463a48094d5fd6` is cross-platform frozen: execution HEAD `71b525e696d63eb48e9bb7e837fc5e87bc69cbee` passed CI #43053491597 on Ubuntu ARM64/macOS/Windows plus Merge Gate, and the reconciled final status HEAD `13291822c75411ef387e2592ce49a8df7f51ee63` passed CI #43055269794.
- **G0 state:** *bounded Arkst migration proof PASS at the exact S08 test HEAD*, the pinned official upstream S08 observations below now exist, while the default-mode failure recovery-content difference and the remaining adversarial matrix remain OPEN. The once-only numerical counter evidence is frozen; only the separate default-mode recovery-content debt #752 and remaining adversarial matrix stay open. This follow-up **evidence-status commit requires its own exact-HEAD CI**, and the earlier successful run alone is not authority for the documentation commit. #719 G0–G8 and final-only target unchanged; do not merge #746 or start G1.

## S08 middle-tail failure / stateful terminal-owner control — bounded cross-platform proof

- **Source-backed test (qualified at exact test HEAD):** `g0_runtime_differential::g0_s08_middle_tail_failure_does_not_dispatch_stateful_terminal_tail` compares a successful native `multiply {3}` middle segment with a failing native `multiply {true}` segment in the same source-defined-head → native-middle-tail → source-defined-terminal-tail chain.
- The terminal `tail` mutates a root-visible `state` to `changed` before returning the indented terminal body. The success control requires `terminal body` followed by `changed`; the failure requires exactly one numeric-arguments diagnostic and only the unchanged `before` state output.
- The fixture calls `compile_and_assert_legacy_parity` for **both** variants, comparing exact final `IrDocument` and ordered diagnostic signatures (including spans). This is a **DYN02** continuation/failure-path evidence candidate, not a new route ID.
- **Critical limits:** the `multiply {true}` failure is a native middle-tail numeric error; the official-v2.5.1 observation has now been captured as detailed below, but alternate error ownership, nested resources and full G0 dynamic reachability are not proven. The new counted side-effect fixture is present on #746 but remains CI-pending at this documentation point. The prior S08 success-case cross-platform proof remains authoritative only for its exact test HEAD.
- **Execution authority:** [CI #43042015522](https://github.com/luceat-lux-vestra/arkst/actions/runs/43042015522) at exact test HEAD `cb620b24a3c567366047423ec64de967a59c533d` **SUCCESS**, including Linux ARM64/macOS/Windows test (Typst required) jobs, Reference JVM and required Merge Gate. The bounded successful-control / failing-native-middle-tail test is now cross-platform proven. This evidence-reconciliation documentation commit requires its own exact-HEAD CI before declaring that new documentation HEAD qualified. G0 remains open; #746 unmerged.

## S08 pinned official Quarkdown v2.5.1 black-box oracle — six observed executions

**Evidence authority:** disposable clean-room [PR #751](https://github.com/luceat-lux-vestra/arkst/pull/751), exact probe HEAD `cc8162b8d9d4c9098f0593e39b786710c12d4083`, workflow `.github/workflows/probe-719-g0-s08-v251.yml`, completed [run #43045759202](https://github.com/luceat-lux-vestra/arkst/actions/runs/43045759202) **SUCCESS** with official binary download/checksum/execution and artifact upload all successful. The three fixture sources are independently authored and committed in that workflow, **not copied from upstream implementation/tests**.

- Official release URL: `https://github.com/iamgio/quarkdown/releases/download/v2.5.1/quarkdown-linux-x64.zip`.
- Verified archive SHA-256: `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`.
- Retained run artifact: `g0-s08-quarkdown-v2.5.1`, ID `13867202016`, digest `sha256:314071c54e248bd682dc0cb7b92ee18e73c6282dab84ad5ac61275b80eb12657`.
- Invocation: official binary `compile <fixture>.qd --clean --pretty --out <path> --out-name probe`, adding `--strict` for strict run. Each observation records exit code, stdout, stderr, visible text of emitted HTML, and complete HTML SHA-256.

| Fixture | Default official v2.5.1 | Strict official v2.5.1 | Interpretation |
| --- | --- | --- | --- |
| S08-terminal: `identity {2}::multiply {3}::tail` + terminal indented body | rc 0; stderr empty; one HTML page, visible `Quarkdown terminal body` | rc 0; stderr empty; one HTML page with same hash | Successful mixed-owner terminal body is semantically compatible with Arkst's bounded `terminal body` observation; ignore Quarkdown page chrome only |
| S08-state-success: terminal `tail` writes `state=changed`, then `.state` | rc 0; stderr empty; one HTML page, visible `Quarkdown terminal body changed` | rc 0; stderr empty; one HTML page with same hash | Final-tail dispatch and state observation agree with Arkst success control at the tested boundary |
| S08-state-failure: only `multiply {3}` changed to `multiply {true}` | rc 0; stderr `Cannot call function multiply(Number a, Number by) with arguments (2, true): Not a numeric value: true`; one HTML page shows an `Error: multiply` panel, the literal failing `identity` chain and terminal body, plus `before` | rc 66; no HTML page; stderr starts `An error occurred while in strict mode (error code 66)` and names `multiply` | Middle-tail numeric failure prevents final owner state mutation in non-strict mode, as Arkst does. **But error recovery/visible rendered output differs:** upstream retains error panel + original source/body; Arkst bounded IR emits only the remaining `before` paragraph. Strict abort has no comparable post-failure state observation. |

**Exact successful HTML hashes:** `s08-terminal` default/strict = `e13cdbb4bb058c65c7945263ad84c454228bf8dbd84f473c33385d905a675561`; `s08-state-success` default/strict = `1c74f2cb07a9e38ca97cbc107aba60bc30c958dfcb41346b67508da613d72555`. Default failure HTML hash = `6c21f0265c0df0b3f3de9dacad9f6cea26943812c78399083dba1213e4d09bff`; strict failure has no HTML hash because no file is generated.

### S08 once-only counter candidate

- **Candidate test:** `g0_runtime_differential::g0_s08_side_effectful_head_executes_once_on_success_and_middle_failure`, added on #746 at commit `ad7c61d4063f5f1850c09dfc2a463a48094d5fd6`.
- The source-defined chain head increments a caller-visible numeric `count` and returns a numeric value. The success control continues through native `multiply {3}` and the terminal source-defined tail; the failure variant changes only the middle input to `multiply {true}`.
- Required observation in both variants is final `count=1`. On success the visible result is `terminal body\n1`; on middle-tail failure the visible result is `1`, proving the already-successful head committed once while the terminal owner did not execute.
- The test still performs exact normal-frontend vs legacy executable-IR equality and complete ordered diagnostic-signature equality, so a final count of 1 constrains both entrypoints.
- **Execution evidence frozen:** exact execution HEAD `71b525e696d63eb48e9bb7e837fc5e87bc69cbee` completed CI #43053491597 **SUCCESS**. Ubuntu ARM64, macOS, and Windows Typst-required test jobs all passed, as did the required Merge Gate. The S08 once-only fixture is therefore frozen for this bounded migration slice; #752 recovery-content debt remains separate.

### S08 default-mode recovery-content compatibility debt

- **Owner:** #752, `compat(runtime): preserve default-mode failed-chain recovery content`.
- **Required semantic test:** `g0_s08_default_mode_failed_chain_preserves_recoverable_error_content`.
- #752 owns the backend-neutral failed-call recovery contract: source-backed recovery/error content at the failed call site, preserved failed chain/body provenance, continued later document content, first-failing-tail ownership and composition with the accepted invocation-scoped rollback rule.
- Whole-document HTML implementation remains downstream of #320; #752 must not select a second renderer merely to match the reference artifact.
- This gap is **compatibility debt, not an accepted divergence**. #746 records evidence/classification only and must not change production behavior to make the ledger green.


## S09 chain-tail binding errors — pinned official ordering evidence

Disposable clean-room PR #753 is **CLOSED/UNMERGED**. Its final probe HEAD is `4266e88f98da26aff75a089bea64acce41d16cb8`; authoritative run #43052756497 completed **SUCCESS** against the exact official Quarkdown v2.5.1 Linux x64 archive SHA-256 `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`. Retained artifact ID `13870690958`, digest `sha256:57c9b696dc9b3affe2aeb4410c2eb9e5dd338e099d591fb8b39f494141dab160`.

The first probe run #43052656501 is explicitly **NON-EVIDENCE**: the generated workflow accidentally escaped the shell array expansion and passed literal `${args[@]}` to the Quarkdown CLI. Commit `4266e88f98da26aff75a089bea64acce41d16cb8` fixes only that harness defect; only run #43052756497 is authoritative.

| Fixture | Official default | Official strict | Bounded semantic observation |
|---|---|---|---|
| terminal body collides with the terminal `tail(previous)` predecessor slot | rc 0; stderr owned by `tail` with ParameterAlreadyBound; one HTML page contains error recovery, literal failed chain/body, and final visible `1`; SHA-256 `78e5105e25a173706d83e214665f80d09708f1dc81b468f5ae7447f629f80a8f` | rc 66; `tail` failure; no HTML | source-defined head already executed once and committed `count=1` before terminal-tail binding failure |
| excess authored tail positional argument after implicit predecessor | rc 0; stderr owned by `tail`, 3 arguments for 2 parameters; one HTML page contains error recovery, literal failed chain, and final visible `1`; SHA-256 `e14fec6e8102f3b9ed56a80463626a42a42d6afd8f2d70bd9fccdb073fde13f2` | rc 66; `tail` failure; no HTML | same ordering: head commits once, then terminal tail rejects excess binding; the head effect is neither duplicated nor rolled back |

### Arkst reconciliation

The initial public S09 candidate incorrectly encoded an old **preflight-before-head** interpretation and expected final `count=0`. CI #43052527482 disproved that interpretation on current Arkst: both Ubuntu ARM64 and macOS reached the new test after all preceding G0 differential fixtures passed, and the first S09 assertion reported actual `"1"` versus expected `"0"`.

That failure is evidence about the test expectation, **not a production regression**. The normal frontend and legacy evaluator had already passed exact IR and normalized diagnostic equality before the assertion. Current Arkst therefore matches the official reference ordering for the first bounded case.

Commit `feb1537c58cb1160dd2e18bde2b690cf95cd3631` renames the fixture to `g0_s09_chain_tail_shape_errors_preserve_once_only_head_effect`, changes both bounded expected counters to `1`, and documents the correct order.

**Execution evidence frozen:** exact execution HEAD `71b525e696d63eb48e9bb7e837fc5e87bc69cbee` completed CI #43053491597 **SUCCESS**. The corrected two-case S09 public differential passed Ubuntu ARM64, macOS, and Windows Typst-required tests and the required Merge Gate. Both bounded cases therefore preserve exact normal-frontend vs legacy-evaluator IR/diagnostic parity while observing the same once-only committed head effect (`count=1`) as the official v2.5.1 default-mode reference.

**Classification:** official S09 reference and Arkst cross-platform migration evidence are frozen as **bounded semantic-order compatible**. This does not claim byte-for-byte error rendering parity; default-mode recovery-content rendering remains governed by the separate S08/#752 compatibility debt.


## S02 UTF-8 / CRLF source provenance — bounded official semantics + Arkst span freeze

Disposable clean-room PR #756 is **evidence-only and must remain unmerged**. The final bounded probe HEAD is `64a2b784fef393705752a02602d3149a56f74ab0`; authoritative run #43103889446 completed **SUCCESS** against the exact official Quarkdown v2.5.1 Linux x64 archive SHA-256 `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`. Retained artifact `g0-s02-quarkdown-v2.5.1` has ID `13893545643` and digest `sha256:cd97005a95d63a8b7ada89c7532302a7c4d00ac3132ea063d57c7a1cb3f6a4ea`.

The final probe intentionally uses only native `.text` calls so it measures the S02 grammar surface itself rather than user-defined-function implementation details. Earlier #756 runs #43103559415 and #43103738073 are **NON-EVIDENCE**: those revisions included a broader user-defined-function CRLF fixture and were replaced before freezing S02.

| Final fixture pair | Official v2.5.1 default + strict | Bounded conclusion |
| --- | --- | --- |
| tight nested UTF-8 native call, LF vs CRLF | all rc 0; stderr empty; visible text `앞 값 뒤` | LF and CRLF are semantically equivalent at this native nested-call boundary. LF HTML SHA-256 `a76536e4dcaf8adfddb1df3454c8688727603581c8f3ec533617809e575d78c8`; CRLF HTML SHA-256 `601075a1830a7bad6d023a6b86a72bc13a71d64a690a27a5613cdeca3102601b`. Different HTML bytes mean **do not claim byte-identical renderer output**. |
| escaped closing delimiter spanning a line break, LF vs CRLF | all rc 0; stderr empty; visible text `prefix 한글 } text suffix` | LF and CRLF semantics match at this escaped-delimiter boundary. All four outputs share HTML SHA-256 `64c5d98fa611ecc673fae43fcf73203191e36efcc439c467842f94494ea81636`. |

### Arkst source-provenance evidence

PR #746 adds three public S02 tests:

- `g0_s02_utf8_tight_nested_call_keeps_exact_source_spans_and_mode_isolation`: Quarkdown mode preserves the exact source-backed spans for the outer call, nested call, UTF-8 argument and surrounding text while all offsets remain UTF-8 character boundaries. The same source parsed in Markdown mode must not fabricate block or inline directive calls.
- `g0_s02_crlf_escaped_delimiter_keeps_source_backing_and_markdown_literal`: the raw CRLF and escaped `\}` remain source-backed through the parsed content; Markdown mode again remains literal/non-directive.
- `g0_s02_normal_compile_keeps_utf8_crlf_nested_failure_source_identity`: normal compilation of a nested failing call pins the diagnostic primary span to the exact `{true}` bytes in the original CRLF source and preserves the correct `SourceId`.

The exact Arkst execution HEAD `67b192781620a03c8264a4d76491c67e377f2c9a` completed CI #43102574849 **SUCCESS**. Ubuntu ARM64, macOS and Windows Typst-required tests, compatibility/reference jobs, fmt/clippy/MSRV/WASM/docs/security-related jobs and the required Merge Gate all passed.

**Classification:** official v2.5.1 is authority only for the bounded native LF/CRLF semantic observations above. Arkst's exact byte offsets, raw-CRLF provenance, SourceId mapping and Markdown-mode isolation are internal parser/compiler evidence and are **not inferred from black-box HTML output**. S02 is therefore frozen as `UPSTREAM_V251_NATIVE_LF_CRLF_SEMANTICS_AND_ARKST_SOURCE_PROVENANCE_CROSS_PLATFORM_FROZEN`; no compatibility debt was discovered in the final bounded S02 grammar.



## S03 nested rich inlines — official cross-version reference + Arkst provenance debt

Disposable clean-room PR #757 is **CLOSED/UNMERGED**. Its final probe HEAD is \`ba44fe89d7c97920635c59b21bdf74fb67f07734\`. The independently authored fixture nests Strong text, a Markdown link and an inline \`.uppercase {world}\` call, then exercises the same authored content as tight/indented LF/CRLF in default and strict modes.

### Official Quarkdown reference

Two release binaries were pinned because the repository's verified compatibility baseline is v2.6.0 while historical G0 slices also use v2.5.1:

- **v2.5.1:** run #43115604496 **SUCCESS**; artifact \`g0-s03-quarkdown-v2.5.1\` ID \`13899065329\`, digest \`sha256:8ed9f11543f0da38ce2e764035a64e284832e0e91f1a99b1a7494f07ce2109d8\`; Linux x64 archive SHA-256 \`5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9\`.
- **v2.6.0 verified baseline:** run #43115604769 **SUCCESS**; artifact \`g0-s03-quarkdown-v2.6.0\` ID \`13898221999\`, digest \`sha256:ce0692de74edfeb67fe90cc63e20d6e3a1e3584b1bce4e5b97240d2249a42c6e\`; Linux x64 archive SHA-256 \`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4\`.

Both releases show the same bounded behavior in default and strict modes:

| Source shape | rc / stderr | Rich-inline observation |
| --- | --- | --- |
| tight LF | rc 0 / empty | one Strong element; link href \`https://example.com\`; inline call renders \`WORLD\`; visible \`앞 WORLD 뒤\` |
| tight CRLF | rc 0 / empty | same semantics |
| indented LF | rc 0 / empty | same semantics |
| indented CRLF | rc 0 / empty | link and inline call still work, but Strong count is 0 and the authored \`**\` markers remain visible as literal text |

The indented-CRLF Strong loss is reproducible in both binaries, but it is **not automatically a required Arkst behavior**. ADR-0016 targets public document-language semantics and explicitly excludes undocumented upstream bugs; Arkst's documented syntax contract states that CRLF is accepted with LF-normalized semantics. Reproducing the binary quirk would therefore weaken Arkst's documented CRLF contract without a compatibility decision.

### Arkst migration/source-map evidence

PR #746 adds bounded public evidence across parser/evaluator/backend ownership without introducing a forbidden core → Typst dependency:

- \`g0_s03_nested_markdown_and_inline_call_preserve_order_spans_and_legacy_parity\` proves tight-CRLF parser ordering/spans, Markdown-mode isolation, exact normal-frontend vs legacy final-IR/diagnostic parity, and final Strong → Text/Link(\`WORLD\`)/Text ordering.
- \`g0_s03_indented_crlf_rich_body_preserves_arkst_markdown_structure\` proves current Arkst preserves the intended Strong/link/inline-call semantics for the indented CRLF source and that normal frontend vs legacy evaluator remain exactly aligned.
- \`crates/arkst-cli/tests/g0_s03_source_map.rs\` exercises the real normal compile → final IR → Typst lowering boundary for tight and indented CRLF without adding any new core dependency edge. It requires the generated Typst to retain Strong + link + \`WORLD\` and requires the source map to carry the final IR Strong/link spans.

The audit found one real Arkst provenance defect. Tight CRLF retains the full authored Strong span. For the indented CRLF source, current final \`IrInline::Strong.span\` slices only:

\`**앞 [.uppercase {world}](https://example.com) 뒤\`

and omits the authored closing \`**\`. The Typst source map correctly reflects that current final-IR span, so the defect belongs to frontend/final-IR provenance rather than the lowering mapper itself. **#758** owns restoring the closing delimiter while preserving current semantic structure, UTF-8/CRLF source backing and mode isolation.

This is **compatibility/provenance debt, not an accepted divergence**. Evidence-only #746 freezes current behavior and must not implement the production fix.

**Execution authority:** exact evidence HEAD \`329935ada00ca416cdc96a0aa321d53d1c17642e\` completed CI #43116791006 **SUCCESS**. Ubuntu ARM64, macOS and Windows Typst-required tests, compatibility/reference checks, fmt/clippy/MSRV/WASM/docs/security-related jobs and required Merge Gate all succeeded.

**Classification:** \`UPSTREAM_V251_V260_RICH_INLINE_REFERENCE_FROZEN_ARKST_CRLF_SEMANTICS_PRESERVED_SPAN_DEBT_758_OPEN\`. S03 is evidenced/classified, but source-provenance completion remains blocked by #758.



## S05 native direct fallback — cross-version binding/conversion reference

Disposable clean-room PR #759 is **CLOSED/UNMERGED**. Its final probe HEAD is \`f7c8f3d774d9c1160d91e5f2a387825b1dc08448\`; authoritative matrix run #43146982448 completed **SUCCESS**.

The probe pins both official binaries used by this repository's compatibility evidence:

- **v2.5.1:** artifact \`g0-s05-quarkdown-v2.5.1\`, ID \`13912021800\`, digest \`sha256:cde05d6cb692aaa90491606edc5f26d3420d2be9d9aca042d9a3548f19a936f9\`; Linux x64 archive SHA-256 \`5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9\`.
- **v2.6.0 verified baseline:** artifact \`g0-s05-quarkdown-v2.6.0\`, ID \`13912316262\`, digest \`sha256:0bea018cc182142f22e4b53c3f03430f8eaef5bd8c6aade2c88e849585781194\`; Linux x64 archive SHA-256 \`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4\`.

The two releases behave identically for the bounded direct-native cases in both default and strict modes.

| Fixture | Official observation | Bounded conclusion |
| --- | --- | --- |
| \`.sum {1} {2}\` | rc 0, empty stderr, visible result \`3\`; HTML SHA-256 \`8cc1ff7a706fd342dbb6961a9d7a151b567635d98269dafa82108f80639e605b\` | ordinary direct native binding/conversion is stable across v2.5.1/v2.6.0 |
| \`.sum {1}\\n    invalid\` default | rc 0; stderr says \`sum(Number a, Number b)\` received \`(1, invalid)\` and \`invalid\` is not numeric; one recoverable error HTML artifact, SHA-256 \`fbde998724e0638b00b0461a5ba3cd98dbb61cfc3e0254792b8b829ea5b98141\` | the indented body is the second argument, not an ignored body; numeric coercion owns the failure |
| same source, strict | rc 66; stderr identifies originating function \`sum\`; no HTML artifact | strict failure owner is the same direct native call |

### Arkst reconciliation

The S05 route exists because the current direct-AST evaluator does not yet own every native-call shape and can fall back to the transitional pre-lowered chain path. That route is **migration architecture**, not a compatibility contract and not an end-state endorsement.

Public differential \`g0_s05_native_direct_fallback_matches_legacy_ir_entrypoint\` already proves exact normal-frontend vs legacy-evaluator parity for the same two authored cases. The S05 evidence update additionally pins:

- success has no diagnostics and final visible semantic output \`3\`;
- the indented-body failure emits exactly one \`E3001\` numeric-argument diagnostic;
- the failed invocation produces no ordinary final content node, preventing silent success;
- existing \`direct_and_extended_native_raw_body_calls_keep_call_span_provenance\` evidence keeps the direct failed call source-backed through the body text.

Exact Arkst evidence HEAD \`e57bc32241baf36e2bc670313a97bc154ad9c649\` completed CI #43147132829 **SUCCESS** across Ubuntu ARM64, macOS and Windows Typst-required tests plus required Merge Gate.

This yields a bounded compatibility conclusion for **binding, conversion ownership, success output and failure ownership**. It does **not** claim default-mode recovery-content parity: official Quarkdown materializes recoverable failed-call content while current Arkst does not yet have the equivalent backend-neutral representation. #752 already owns that failed-call recovery-content contract and has been updated with this direct-call corroboration; no duplicate debt issue is created.

**Classification:** \`UPSTREAM_V251_V260_DIRECT_NATIVE_BINDING_CONVERSION_FROZEN_ARKST_FALLBACK_CROSS_PLATFORM_PROVEN_RECOVERY_DEBT_752_OPEN\`. S05's public call semantics are evidenced; the fallback implementation remains transitional and G3 must remove it without changing these observations.



## S06 zero-authored tail — predecessor slot + once-only head freeze

Disposable clean-room PR #761 is **CLOSED/UNMERGED**. Final probe HEAD \`b82872891ce93f55a771b60373705159778cd17f\`; authoritative matrix run #43216405262 completed **SUCCESS**.

Pinned official distributions:

- **Quarkdown v2.5.1:** artifact \`g0-s06-quarkdown-v2.5.1\`, ID \`13942668602\`, digest \`sha256:feb8beb0852f61572347931141d6fd06ef6647db7e8f0ab83a50b95a41c77be8\`; Linux x64 archive SHA-256 \`5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9\`.
- **Quarkdown v2.6.0 verified baseline:** artifact \`g0-s06-quarkdown-v2.6.0\`, ID \`13944036277\`, digest \`sha256:cb0e09c5ff384f0b45c47813ea89fd5f951dd12f86b6b3e80cae118539d78444\`; Linux x64 archive SHA-256 \`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4\`.

Both official releases behave identically in default and strict modes:

| Fixture | Official observation | Bounded conclusion |
| --- | --- | --- |
| source-defined \`identity {7}::tail\`, where \`tail\` declares only \`previous:\` | rc 0, empty stderr, visible \`7\`; HTML SHA-256 \`8ce36f5ab06821d0ab99b883925c6865b3ca5321a37e88bda0d6061c6db982e4\` | evaluated head result is available to the zero-authored tail as its implicit predecessor |
| side-effectful \`counted::tail\` control | head increments caller-visible \`count\`, returns numeric \`7\`; tail returns predecessor; later count is \`1\`; visible \`7 1\`; HTML SHA-256 \`3b7ef4df402f04618e144e20363d062190ed825c026771e9204fd1d84ba93376\` | successful head executes exactly once before zero-authored tail binding/dispatch |

### Arkst reconciliation

The existing S06 public differential already compares the same baseline source through normal frontend evaluation and the legacy whole-call IR evaluator with exact final \`IrDocument\` and normalized diagnostic equality.

The evidence update extends \`g0_s06_zero_argument_chain_tail_matches_legacy_ir_entrypoint\` with the independently authored once-only control used by the official probe. It therefore freezes all of the following in both Arkst entrypoints:

- head-side mutation commits exactly once;
- the head result \`7\` becomes the zero-authored tail's predecessor;
- the tail returns that predecessor without requiring an authored argument;
- the later caller-visible counter remains \`1\`;
- final visible semantic output is \`7\\n1\`.

Exact S06 execution HEAD \`4c1ae8aee02f2e4c803135d99f322e5ade370e82\` completed CI #43216634805 **SUCCESS**. Ubuntu ARM64, macOS and Windows Typst-required tests, Reference JVM, compatibility, fmt/clippy/MSRV/WASM/docs/security-related jobs and required Merge Gate all succeeded.

The official black-box HTML is authority only for the bounded semantic ordering/once-only observations. Exact Arkst source spans, value origin metadata and transitional evaluator representation remain internal implementation evidence and are not inferred from rendered HTML.

### Protected-main synchronization

While S06 was being frozen, protected \`main\` advanced from \`40b5659053dd61567646bed8daeff6259e2a8e93\` to \`8b9d347e82001a790baf1dfcf83267c378abd5f3\` via merged #760, which only adopts mold for the Ubuntu ARM64 CI/cache path.

PR #746 was explicitly synchronized after the S06 executable run:

- synchronization merge commit: \`aa252bc5a0accfd67af9f38d8d3c4bb6308a8316\`;
- parents: S06 evidence HEAD \`4c1ae8aee02f2e4c803135d99f322e5ade370e82\` + protected main \`8b9d347e82001a790baf1dfcf83267c378abd5f3\`;
- the merged tree preserves #746 evidence plus the two #760 workflow files;
- compare against protected main reports \`ahead_by=61\`, \`behind_by=0\`, merge base = current protected main.

This removes the prior base drift before the S06 docs are qualified.

**Architecture disposition:** current chain execution still continues through a transitional executable-IR tail path. S06 freezes the public predecessor/once-only contract that G3 must preserve while removing that bridge; the bridge itself is not endorsed as final architecture.

**Classification:** \`UPSTREAM_V251_V260_ZERO_AUTHORED_TAIL_PREDECESSOR_ONCE_ONLY_FROZEN_ARKST_CROSS_PLATFORM_PROVEN\`. No new S06 compatibility debt was discovered in this bounded semantic surface.



## S07 flat tail — predecessor/authored slot order + evaluation-order freeze

Disposable clean-room PR #762 is **CLOSED/UNMERGED**. Final probe HEAD \`0446da6c3015d8f1e5346c61cbaa2bb15b4788e8\`; authoritative matrix run #43233514212 completed **SUCCESS**.

Pinned official distributions:

- **Quarkdown v2.5.1:** artifact \`g0-s07-quarkdown-v2.5.1\`, ID \`13950044069\`, digest \`sha256:ce5ac56c445da30b820e0e87ab74d8f0cf0cecde0a1a31c2182506c14f1c90d3\`; Linux x64 archive SHA-256 \`5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9\`.
- **Quarkdown v2.6.0 verified baseline:** artifact \`g0-s07-quarkdown-v2.6.0\`, ID \`13951237487\`, digest \`sha256:d3822acd6464945f29cf9faa85c114f59d5ed4d7ed694e53bd1b7f11f508dbd9\`; Linux x64 archive SHA-256 \`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4\`.

Both official releases behave identically in default and strict modes:

| Fixture | Official observation | Bounded conclusion |
| --- | --- | --- |
| \`.identity {7}::tail {9}\`, where \`tail(previous, second)\` returns \`second\` | rc 0, empty stderr, visible \`9\`; HTML SHA-256 \`42904ad40d1e7212944e515c9997d297cb35b037d73951bdc3eb39aab66cc884\` | one authored positional tail candidate follows the implicit predecessor and binds to the second parameter |
| same call with tail returning \`.sum {.previous} {.second}\` | visible \`16\`; HTML SHA-256 \`d69b14097f932ac0f271302fe7d27ae7df48fe0f6afeead3d12d2098823d69be\` | predecessor \`7\` and authored positional \`9\` occupy distinct ordered slots 0 and 1 |
| side-effect/read order control \`.counted::tail {.count}\` | head increments caller-visible count from 0 to 1 and returns 7; authored argument reads count; tail returns authored value; later count remains 1; visible \`1 1\`; HTML SHA-256 \`bdee750b99434d4f8df2f3db6f4a7567479c154a75874e0b098287fcf853c5f3\` | head executes exactly once **before** authored tail-argument evaluation |

### Arkst reconciliation

The existing S07 public differential compared the baseline flat-tail source through normal frontend evaluation and the legacy whole-call IR evaluator with exact final \`IrDocument\` and normalized diagnostic equality.

The S07 evidence update extends that same differential with two independent controls matching the official probe:

- a slot-identity control requiring \`sum(previous, second) == 16\`;
- an evaluation-order control requiring final visible \`1\\n1\`, which would become \`0\\n1\` if the authored \`.count\` candidate were evaluated before the side-effectful head.

Exact S07 execution HEAD \`1e0f2e48a173b9d0e951345e7170fec14e7970bf\` completed CI #43233772000 **SUCCESS**. Ubuntu ARM64, macOS and Windows Typst-required tests, Reference JVM, compatibility, fmt/clippy/MSRV/WASM/docs/security-related jobs and required Merge Gate all succeeded.

The bounded public contract is therefore:

1. execute the chain head exactly once;
2. commit/evaluate that head before evaluating the authored tail candidate;
3. bind the evaluated predecessor to slot 0;
4. bind the authored positional candidate to the next positional slot;
5. dispatch the tail with those ordered bindings.

The official black-box output is semantic authority for those observations only. Exact Arkst source spans/origin metadata remain internal evaluator evidence and are not inferred from rendered HTML.

**Architecture disposition:** Arkst still represents this route through the transitional executable-IR chain-tail bridge. S07 freezes behavior that G3 must preserve while removing that bridge; the bridge itself is not part of the ADR-0022 target.

**Classification:** \`UPSTREAM_V251_V260_FLAT_TAIL_PREDECESSOR_THEN_AUTHORED_ORDER_ONCE_ONLY_FROZEN_ARKST_CROSS_PLATFORM_PROVEN\`. No new S07 compatibility debt was found in this bounded semantic surface.



## S10 callable identity — definition capture, caller overlay, parameter precedence

Disposable clean-room PR #763 is **CLOSED/UNMERGED**. Final probe HEAD \`4980f28fc7ef90c4bc341d52ddf347fa59cfb15e\`; authoritative matrix run #43238226209 completed **SUCCESS**.

Pinned official distributions:

- **Quarkdown v2.5.1:** artifact \`g0-s10-quarkdown-v2.5.1\`, ID \`13953472202\`, digest \`sha256:f892286616678d23b6e6a35a708413af22c28a8a1e0b92df7bc9d111c0465a0a\`; Linux x64 archive SHA-256 \`5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9\`.
- **Quarkdown v2.6.0 verified baseline:** artifact \`g0-s10-quarkdown-v2.6.0\`, ID \`13954161200\`, digest \`sha256:4320dceebf7d77ca091bfc8a6defc450028b7e1e4ecfec5798ae8cd449d7ca63\`; Linux x64 archive SHA-256 \`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4\`.

Both releases behave identically in default and strict modes:

| Fixture | Official observation | Bounded conclusion |
| --- | --- | --- |
| definition value \`definition\`; callable returns \`.value\`; invoke before and after caller sets \`value=caller\` | rc 0, empty stderr, visible \`definition caller\`; HTML SHA-256 \`1148644bf3dcc34ea87ad8309bec82261fc1596575347108e3532a4f8513a3da\` | definition capture supplies fallback when no caller binding exists; current caller-visible value overlays it at invocation |
| definition \`value=definition\`, caller \`value=caller\`, callable parameter also named \`value\`, invoke with \`parameter\` | rc 0, empty stderr, visible \`parameter\`; HTML SHA-256 \`3861de905485dd8eb6e2cd28469310e9f70c24e776f7d24c9c2645fe761cd0de\` | invocation parameter shadows caller overlay and definition capture |
| outer callable invokes inner callable reading \`.value\`; caller changes \`value\` between invocations | rc 0, empty stderr, visible \`caller next caller\`; HTML SHA-256 \`f25cd12c7a8ce17b627f28b57dd3fdaeda24cd8b230541bfc3234373f09d615b\` | current caller overlay propagates through nested callable invocation and is refreshed for each call |

### Arkst reconciliation

The existing product tests already covered the individual callable scope behaviors. S10 adds a single G0 public differential, \`g0_s10_callable_capture_and_caller_overlay_match_legacy_ir_entrypoint\`, containing the same three independently authored controls. Each source is compiled through both the normal frontend and the legacy whole-call evaluator, requiring exact final \`IrDocument\` and normalized diagnostic equality before its visible result is asserted.

The frozen public lookup layering is therefore:

1. immutable **definition capture** as fallback;
2. current **caller-visible overlay** for permitted caller context;
3. invocation child scope with **parameters installed last**, so parameters shadow both outer layers;
4. nested callable invocation receives the current caller overlay rather than a stale invocation snapshot.

This does not imply that caller overlay becomes a mutation owner or that definition capture is rewritten. Those ownership rules remain internal evaluator-state semantics and broader mutable-scope parity remains outside this bounded S10 observation.

Initial execution HEAD \`b5d6384dd811d2f0265797321a84e60d6d156987\` passed the Ubuntu ARM64/macOS/Windows semantic test jobs but failed only the repository rustfmt gate. No semantic assertion failed. The formatting-only correction is HEAD \`2b75929942b1de5f3db7942f9cf2df27692d756a\`; final execution/final-head authority is recorded only after its CI completes.

**Architecture disposition:** \`IrCallable\` / callable capture and runtime scope composition remain G2/G4 migration ownership. The public definition/caller/parameter lookup semantics must survive clean-break removal of legacy executable/runtime bridges; rendered HTML is not authority for Arkst's capture representation, owner identity or source provenance.



### Compatibility disposition and remaining G0 blockers

- **Observed supported-success parity (bounded):** upstream compiled terminal body and state mutation align with the local Arkst normal-vs-legacy migration fixtures. The independent HTML hashes apply **only** to Quarkdown's own default-vs-strict output, not Arkst/Quarkdown byte-for-byte renderer equality.
- **Observed error-recovery gap (OPEN, not accepted divergence):** official non-strict recovery emits an error panel and source/body in HTML while Arkst's bounded failure `IrDocument` path returns `before` and its own diagnostic. Classify separately as a compatibility behavior to investigate at G0; neither mask it by claiming full S08 compatibility nor change Arkst's accepted invocation-scoped rollback invariant without architecture review.
- **Still open beyond the frozen S08/S09 slices:** other middle-tail ownership/failure forms, multi-source/nested/host paths, complete G0 producer/consumer reachability, and cross-target output/diagnostic equivalence. S08 once-only and corrected S09 two-case ordering are cross-platform frozen at execution HEAD `71b525e696d63eb48e9bb7e837fc5e87bc69cbee` / CI #43053491597. Strict upstream exit `66` is a reference observation, not an automatic Arkst CLI-exit matching requirement.
- **Checkpoint discipline:** this is an evidence-only update on PR #746 and requires its **own exact-HEAD CI** before qualification. Disposable PR #751 must be **closed unmerged** after these observations are durable. #719 remains OPEN; no G1 or #746 merge authorization.

