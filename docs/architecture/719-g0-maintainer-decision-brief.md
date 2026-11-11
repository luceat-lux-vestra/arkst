# #719 G0 — Maintainer decision brief (proposed; NOT ACCEPTED)

**Status:** REVIEW REQUEST / recommendations only. **Owner:** #719. **Review baseline:** protected \`main@af312cab48fa037789b062d8a8ca8f1bcb51b88b\`; PR #746 exact candidate \`e538c835f80dfd0d1f7fc720743252334ae31096\`. **Date:** 2026-11-11.

This brief separates actual **maintainer architecture/product compatibility decisions** from G0 engineering evidence work. It does **not** change ADR-0022 from Proposed to Accepted; it does **not** approve breaking behavior, amend ADR-0020, implement a wire serializer/decoder, merge #746, or mark G0 complete.

## Executive recommendation (single review package)

| ID | Decision needed | Recommended choice | Human approval? |
| --- | --- | --- | --- |
| **D1** | Runtime vs final-document ownership | Accept proposed ADR-0022's direct-AST evaluator-owned runtime + final-only immutable document IR; supersede only ADR-0020's one-mixed-IR claims | **Yes: architecture authority** |
| **D2** | How to read old unversioned IR JSON | Introduce separately versioned final-only wire **v1**; explicit, read-only legacy import only for **provably evaluation-complete** content; reject executable, ambiguous and malformed legacy payloads; never auto-execute during decode | **Yes: compatibility contract** |
| **D3** | Public Rust facades and CLI transition | Migrate the default \`inspect --emit ir\` output to versioned final-only v1 at a documented pre-1.0 minor release; retain a bounded deprecated legacy Rust adapter during transition, with a predeclared removal milestone, not an indefinite public execution backdoor | **Yes: breaking-release policy** |
| **D4** | Unresolved / unsupported source calls | Emit explicit source-backed diagnostics and prevent publication as successful final-only IR; allow separate AST/source inspection, **not executable call nodes in final IR**; preserve independently evidenced Quarkdown behavior and intentional divergences | **Yes: user-visible compatibility/error behavior** |

The four items can be accepted as one package or revised separately. Engineering tests, source enumeration, CI and dynamic route traces do **not** require an additional human product decision unless they reveal a new public abstraction, semantic divergence or weakened safety invariant.

## Current evidence: why this is a real decision, not an abstract redesign

1. **Already-agreed direction.** #719's issue contract and proposed [ADR-0022](../adr/0022-runtime-final-document-ir.md) describe:
   \`frontend AST -> EvalSession / ArgumentCandidate / RuntimeValue -> immutable final Document IR -> pure backend lowering\`. Existing merged #720/#721/#722 foundations and later source-selective routes are deliberately transitional. The decision to **separate runtime and final IR** has already been made in #719 planning; D1 is the **formal ADR acceptance / supersession** authority, not a request to invent a new architecture.
2. **Contradictory accepted text.** Accepted [ADR-0020](../adr/0020-programmable-document-semantic-model.md) states a one-IR target, language-valued \`IrValue\`, and structurally unresolved calls that may survive evaluation. [ARCHITECTURE.md](../ARCHITECTURE.md) section \`IR Model\` still says one \`IrDocument\` may be initial executable or normalized backend output. Only those lifecycle claims should be superseded. Preserve typed backend-neutral components, target-driven conversion, provenance, diagnostics, source maps and the existing security model.
3. **Actual public Rust surface.** \`crates/arkst-core/src/lib.rs\` exports \`ast_to_ir::ast_to_ir_with_diagnostics\` and an \`evaluator::Evaluator\` compatibility facade accepting executable \`IrDocument\`; \`crates/arkst-core/src/ir.rs\` re-exports \`arkst_ir::*\`. The \`ir_facade\` test asserts core/physical IR type identity. \`crates/arkst-typst/src/lowering.rs\` takes \`&IrDocument\`, not a Rust type excluding calls. G6/G7 therefore require an intentional public interface transition.
4. **Current serialized contract is not versioned final-only.** \`crates/arkst-ir/src/lib.rs\` implements \`Serialize\`/\`Deserialize\` directly for mixed-lifecycle \`IrDocument\`; its private \`IrDocumentWire { nodes, metadata, sources? }\` has **no schema version**. The type contains \`IrNode::{FunctionCall, ChainedFunctionCall, FunctionDeclaration}\`, \`IrInline::{DirectiveCall, ChainedDirectiveCall}\` and \`IrValue::{Callable, InlineBody}\`. Existing raw-body decode validates source/source_ref exclusivity and bounds, but that narrow check does **not** prove whole-document final-only semantics, source registry validity, or malicious payload rejection.
5. **CLI coupling.** \`crates/arkst-cli/src/commands.rs::inspect_with_libraries\` implements \`"ir" => serde_json::to_string_pretty(&result.ir)\`. \`ast\`, \`semantic\`, and \`source-map\` emit modes are still placeholders. Changing final IR without an output-format policy changes a real CLI-visible JSON contract.
6. **Actual release posture.** \`arkst-core\`, \`arkst-engine\`, \`arkst-ir\`, and \`arkst-cli\` currently use \`version = "0.0.1"\`, \`publish = false\`. Accepted [ADR-0010](../adr/0010-versioning-and-compatibility-policy.md) already requires **pre-1.0 breaking changes to increment the minor version and be documented in CHANGELOG**. Unpublished crates reduce external registry compatibility pressure, **but do not prove that Git consumers or CLI JSON consumers do not exist**.
7. **Compatibility boundary.** Accepted [ADR-0016](../adr/0016-full-quarkdown-compatibility-and-upstream-evolution.md) requires public-language compatibility evidence and review of intentional divergences. S12's failure-atomic rollback is already an explicit accepted Arkst divergence; changing architecture must not silently discard it. PR #746 currently proves bounded S04/S05/S06/S07/S12/S13/S14/S15/S16 normal-vs-legacy cases and DYN01–DYN06 routes. These are **not exhaustive** G0 evidence.

## D1 — Architecture authority

| Choice | Benefits | Costs / risks | Assessment |
| --- | --- | --- | --- |
| **A. Accept ADR-0022 final-only separation** | Compile-time isolation of executable runtime from rendering/serialization; clear source provenance, evaluator and backend responsibilities | Multi-gate G1–G8 migration, compile breakage and tests to rewrite | **Recommended** |
| B. Keep one mixed \`IrDocument\` plus runtime guards/sidecars | Lowest immediate migration cost | Runtime execution state can remain structurally valid in output/wire; provenance bridges continue to grow; leaves #682/#718 architecture tension | Reject |
| C. Build HIR/MIR/bytecode VM before separating | Could enable future optimizations | No measured need; large new implementation/test burden unrelated to present boundary | Reject |

**Exact scope if accepted:** supersede ADR-0020's *one-IR lifecycle and runtime-carrier statements only*, not its component/type-conversion semantics. Keep ADR-0020's historical record with a scoped supersession annotation; do not rewrite its original historical observations as if the newer design had always existed.

## D2 — Versioned final-only wire and legacy import

| Choice | Impact | Assessment |
| --- | --- | --- |
| A. Reject every old unversioned JSON | Simplest implementation; all stored inspection JSON becomes unreadable | Safe but unnecessarily disruptive |
| **B. Explicit validated legacy import; final-only v1 for output** | Preserves benign historical document data where safe; refuses executable/ambiguous cases; bounded migration complexity | **Recommended** |
| C. Transparently deserialize all old JSON and evaluate on demand | Best apparent compatibility | Unsafe trust-boundary widening; could replay function bodies or depend on ambient host capabilities; violates final-only design |

**Proposed policy (not yet approved or implemented):**

- Version **the document wire format**, separately from tool SemVer and \`arkst.toml\` config-version. For example, a top-level format discriminator + integer wire version and a **document-only payload**; exact Rust DTO layout/field names are G7 implementation details.
- New CLI \`inspect --emit ir\` and documented serialization API publish **only** a versioned, evaluated, final-only form. No evaluator runtime values, callables, lazy arguments, body syntax, host handles or resources in the output schema.
- A legacy importer is an **explicit opt-in read path**, not implicit detection by raw JSON shape. It either (a) validates a legacy payload as completed document semantics and migrates it **without executing it**, or (b) rejects it with a deterministic, source-backed or wire-path-specific error.
- Reject unknown wire versions; executable node/value/inline variants anywhere recursively (including nested tables/components); missing/out-of-range \`source_ref\`; invalid UTF-8/byte ranges/source identities; duplicate or inconsistent source records; unsupported structural shapes; unbounded depth/size; unknown critical fields. Exact duplicate-key and unknown-field rejection require deliberate decoder handling rather than relying on Serde defaults.
- No filesystem/network/process access during import. Backends receive the typed final document, never the legacy wire object.
- Do not infer that present \`IrRawBody\` source slice validation is sufficient for whole-document safety.

### Minimum G7 decode matrix to freeze before implementation

| Input class | Proposed v1 behavior | Required proof |
| --- | --- | --- |
| Valid v1 final-only document | Accept and round-trip deterministically | Exact bytes/schema, source maps and both Typst backends |
| Unknown/future version | Reject | Deterministic version error, no fallback |
| Valid legacy document containing completed-only semantics | Accept **only via explicit legacy import** and normalize to v1 | No evaluation or host access, round-trip |
| Legacy root with unresolved \`FunctionCall\` / chained call / declaration | Reject | Recursively nested and top-level negative cases |
| Legacy callable / inline body / evaluator-only value | Reject | Recursion through lists, dictionaries, components, inlines |
| Missing / out-of-range / duplicate / ambiguous source reference | Reject | Explicit source table and span negative cases |
| Oversized/deeply nested or malformed fields | Reject with bounded failure | Resource limits, no panic, stable diagnostics |

## D3 — Rust public API and CLI migration window

| Choice | Trade-off | Assessment |
| --- | --- | --- |
| A. Immediately remove old methods and JSON output | Fast clean type boundary, maximum downstream disruption | Only if explicitly approved as an abrupt pre-1.0 break |
| **B. Explicit deprecation/migration window with removal milestone** | Lets consumers migrate; still reaches final-only type enforcement | **Recommended** |
| C. Keep \`ast_to_ir\` and \`evaluate(IrDocument)\` public forever | Lowest compatibility friction | Negates strong separation, permanent maintenance/security debt |

**Recommended rollout:**
1. G0 freezes API caller inventory, existing \`inspect --emit ir\` snapshots and wire negatives. Define what external usage can and cannot be substantiated.
2. G1–G6 add new runtime/final types and switch internal paths. Transitional APIs are quarantined from the final-only renderer and marked deprecated when replacement APIs are usable.
3. At the next documented **pre-1.0 minor breaking release**, \`inspect --emit ir\` switches to versioned final-only output; CHANGELOG includes before/after JSON and migration instructions. Do not silently call the unversioned wire “v1”.
4. During a declared bounded transition, explicitly named read-only legacy import and Rust adapter can remain for migration/tests, with a published removal milestone. They cannot be alternate execution backdoors into the final-only renderer. Do not promise indefinite backwards compatibility.
5. Before removal, CI runs migration fixtures for CLI consumers, normal \`compile\` facade, \`ast_to_ir\`/evaluator compatibility and both backend adapters.

**Residual uncertainty:** Cargo \`publish = false\` does not tell us whether a third party is using Arkst from Git or parsing old \`inspect\` output. No external adoption count is asserted here.

## D4 — Unresolved / unsupported source behavior

| Choice | Consequences | Assessment |
| --- | --- | --- |
| A. Continue outputting \`FunctionCall\` as a completed IR node | Keeps current structural representation | Violates final-only type contract |
| B. Drop the node silently | May seem convenient for partial output | Hides compatibility debt and loses source diagnostics |
| **C. Report explicit unsupported/unresolved diagnostics, keep syntax in AST inspection** | Consistent fail-closed final publication; preserves debugging and future migration | **Recommended** |

An unresolved language construct should remain visible in **diagnostics** and optional independent AST inspection, not in a serialized/backend-consumable final-document node. Define the publication gate: an ordinary compile may retain *only already-evaluated* partial semantic output alongside diagnostics for debugging, but must **not present it as a successful final-only published artifact** if unresolved executable source remains. \`inspect --emit ir\` must reject successful emission on such a failed compilation. This is a proposed future contract; today's output and existing Quarkdown compatibility cases still need a case-by-case regression matrix before change.

Preserve diagnostic code identity per ADR-0010, source spans, failure atomicity, explicit source/resource capability policy, and deliberate divergence review per ADR-0016. In particular, S12 rollback does **not** become an approval to emulate upstream non-atomic mutation persistence.

## What does *not* require maintainer choice now

- Enumerating all AST/IR producers and consumers; dynamic reachability and remaining G0 differential/reference matrix.
- Completing executable tests for source maps, nested content, provenance and rollback without weakening invariants.
- Fixing test/CI failures and recording exact-HEAD evidence; keeping #746 unmerged until its documented gate is met.
- Implementing an **approved** wire/API policy in bounded G7 slices after the release/version and legacy import decisions are recorded.
- Choosing an arbitrary number of internal PR slices: #719 already establishes G0–G8 gate sequence and allows the PR count to vary.

## Required confirmation and acceptance gates

A maintainer review should explicitly record **approve / revise** for D1–D4, with rationale for any divergence. **Approval of this brief is not a substitute for remaining G0 evidence.** After approval:
1. Change ADR-0022 from Proposed to Accepted and annotate exact scoped supersession in ADR-0020 and \`docs/ARCHITECTURE.md\`; preserve ADR-0010/0016 invariants.
2. Freeze versioned wire, explicit legacy importer, CLI and Rust API migration decisions as separate G7 testable contracts; add failing negative fixtures before implementation, not guessed PASS status.
3. Continue G0 runtime/wire/host producer-consumer proofs and all still-open S01–S32 evidence with exact-HEAD CI.
4. Reconcile #719 and require an explicit G0-exit review before G1. Do not merge #746 or close #719 merely because the proposal and current CI are green.

**Checkpoint:** This file is **decision support** only. No selection has been unilaterally accepted, and no production wire/API behavior is changed by adding it.
