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

## S08 mixed-owner multitail + terminal body — bounded cross-platform proof (not G0 exit)

- **New full-frontend fixture:** `g0_runtime_differential::g0_s08_multitail_terminal_body_source_native_source_matches_legacy` compiles parser-clean source containing a source-defined `identity` head, native `multiply` middle tail, and a source-defined `tail` owning the terminal indented body.
- **Oracle comparison implemented:** `compile_and_assert_legacy_parity` checks exact `IrDocument` plus ordered diagnostics (`code`, `severity`, `message`, primary/secondary source spans, `hints`) against the separate legacy AST-to-executable-IR evaluator path. The tested visible terminal content is `terminal body`.
- **Precise dynamic claim if CI passes:** normal public source compilation traverses a multi-tail, mixed-owner continuation and binds the terminal block body at the final source-defined owner without a frontend/legacy output difference. This extends the already known **DYN02** path; it is **not** a new architecture route ID.
- **Authority limit:** the engine-internal `phase3g_multitail_final_body_matches_legacy_chain_result` remains a bounded unit anchor, not an independent upstream oracle. The new public full-frontend S08 path passed [CI #43039010366](https://github.com/luceat-lux-vestra/arkst/actions/runs/43039010366) at exact HEAD `6b945dcd45266ab9d80b6e796edcca603417fdd9`: all three OS full test jobs and required Merge Gate **SUCCESS**. No pinned official Quarkdown S08 default/strict observation, error-first-tail permutation, once-only-effect proof, rich body or resource-context matrix is asserted.
- **G0 state:** *bounded Arkst migration proof PASS at the exact S08 test HEAD*, independent upstream/reference and adversarial/once-only-effect matrix still PENDING. This follow-up **evidence-status commit requires its own exact-HEAD CI**, and the earlier successful run alone is not authority for the documentation commit. #719 G0–G8 and final-only target unchanged; do not merge #746 or start G1.

