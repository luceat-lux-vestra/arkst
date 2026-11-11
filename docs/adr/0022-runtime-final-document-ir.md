# ADR-0022 — Separate evaluator runtime from final document IR

- **Status:** Accepted (target design; G0–G8 implementation remains incomplete)
- **Date:** 2026-11-10
- **Maintainer acceptance:** 2026-11-11 (development-stage, no historical Arkst API/wire compatibility obligation)
- **Owner:** [#719](https://github.com/luceat-lux-vestra/arkst/issues/719)
- **Companion source audit:** [G0 execution-surface ledger](../architecture/719-g0-audit.md)
- **Basis:** `main@54a7285ca7adce7482bc27df587f156a6162a3f9`

## Context

ADR-0020 and `docs/ARCHITECTURE.md` currently permit a single `IrDocument` to carry both executable pre-evaluation syntax and normalized backend output. #719 explicitly rejects this mixed lifecycle. The current normal compile starts from AST but still lowers unmatched blocks/chains, callable bodies, inline values and nested nodes into executable IR; `RuntimeValue` still bridges through `IrValue`, and the wire accepts executable payloads.

## Accepted target architecture

```
VirtualProject/SourceStore -> source-backed parser AST
  -> arkst-engine EvalSession / context frames / binding+callable store
       -> ArgumentCandidate (unevaluated source, binding-time selection)
       -> RuntimeValue (all kinds; child-wise ValueOrigin + provenance)
       -> direct AST evaluation for all calls/controls/bodies/nested content
       -> source+resource modes, atomic journal, mutable working DocumentState
  -> immutable, evaluation-complete backend-neutral Document IR + DocumentConfig
       -> pure Typst lowering and deterministic source maps (native/in-process)
       -> separately versioned, validated final-document wire DTO (inspect/API)
```

**Runtime ownership:** `arkst-engine` exclusively owns parser-side execution, mutable frame/binding/capture/extension identity, all scalar/collection/pair/dictionary/content/component/callable runtime variants, origin/provenance (per nested element), target conversions, lazy body and ordered argument binding, explicit host capability and source mode, diagnostic state, document writes and nested savepoint/first-write rollback. There must be no `SemanticLeaf(IrValue)` or `InvocationValue` in final production runtime and no recursive context-tree copy needed per normal invocation.

**Final document:** `arkst-ir` contains *only completed document semantics* and immutable backend-neutral `DocumentConfig`; no parser syntax, executable IR, callable captures, context handles, host resources, raw unevaluated argument/body, or runtime conversion data. Specifically `IrNode::{FunctionCall,ChainedFunctionCall,FunctionDeclaration}`, `IrInline::{DirectiveCall,ChainedDirectiveCall}` and `IrValue::{Callable,InlineBody}` are execution-stage carriers **to retire from final IR and wire**. Preserve valid semantic nodes/components. Unimplemented/unresolved source input remains explicitly classified by evaluator diagnostics and optional separate frontend inspection, never silently accepted, discarded, serialized as an executable final document, or replayed by the renderer. Source-span, error semantics, compatibility fail-closed behavior remain binding.

**Document state:** `EvalSession's DocumentState` is mutable and journaled. Its accepted publication is an immutable typed final configuration; preserve default/explicit-empty semantics, source order, inherited configuration, subdocument isolation, partial transactional rollback and native/backend neutrality. Evaluator-only localization tables, providers and mutable handles do not escape.

**Wire and public interfaces:** adopt a separately versioned, strictly validated, final-document-only wire v1 DTO. Reject the historical unversioned/mixed/executable Arkst JSON outright: **no legacy importer, compatibility decoder, automatic migration or dual-format output**. Require deterministic source-table normalization, source/span validation, recursive runtime/executable-variant rejection and resource limits. Change `inspect --emit ir` to the new schema and remove the old public `arkst-core::ast_to_ir`, `evaluator::evaluate(IrDocument)` executable-entry compatibility facades and mixed-IR serialization surface when the replacement routes are ready. **No historical Arkst API deprecation interval or adapter is required**. Internal transitional test/evaluation bridges may exist only while migrating and must be absent at G8.

**Backend/host:** `arkst-typst` and native/in-process hosts accept final-only types with exhaustive pure lowering and source-map generation. Explicit VirtualProject/resource/library/env/logger/capabilities and WASM constraints remain; no new ambient filesystem, network, process, or global mutable state. No generalized HIR, MIR, bytecode VM or replacement renderer is introduced without separate measured justification.

## Development-stage redesign policy — 2026-11-11

The maintainer explicitly prioritized the best end-state design over historical Arkst API or JSON backward compatibility. Prior architecture decisions can be corrected or fully replaced when wrong; do not limit the G0–G8 migration because implementation work is large. Remove obsolete Rust public APIs, manually constructed executable IR support, old `inspect --emit ir` JSON, legacy serializers and dual-runtime bridges rather than maintaining them for compatibility. During migration, internal scaffolding and tests may exist, but G8 must remove production and public execution backdoors.

**Distinct invariants remain:** Arkst-to-Arkst backward compatibility is *not* required, but Quarkdown public-language compatibility and independently verified intentional divergences (ADR-0016), security/resource capability boundaries, diagnostic/source-span precision, transaction rollback, deterministic wire safety, Typst lowering and G0–G8 acceptance **are** required. A generalized HIR/MIR/VM is not mandated without separate technical justification; that is an architecture choice, not an effort-based reduction in migration scope.

The new wire is **v1 only**. Missing/unversioned or unknown versions are errors; there is no legacy decode path or legacy migration acceptance matrix. Version bookkeeping when actually publishing remains governed by ADR-0010.

## Exact supersession boundary

**This accepted decision** supersedes the following *statements* (not unrelated features) of [ADR-0020](0020-programmable-document-semantic-model.md) and `docs/ARCHITECTURE.md `IR Model`:

- One `IrDocument` type containing both executable initial and final output.
- Universal `IrValue` as the language runtime value carrier.
- Unresolved executable `FunctionCall` as valid completed final IR, or durable `IrCallable` captures.
- Serialization of executable call/bodies as part of a final document.

Preserve ADR-0020's accepted typed backend-neutral semantic component decision, strict target-driven conversion, source provenance guarantees, output materialization, diagnostics and Typst source-map responsibility. ADR-0015 crate ownership remains applicable with the explicit separation of runtime versus final schema and wire. ADR-0016 clean-room and compatibility evidence policy is unchanged.

Approval establishes **target architectural authority only**; G0 evidence and G1–G8 migration remain incomplete. Historic accepted ADRs keep their historical record with scoped supersession notes. The physical implementation may continue to contain transitional executable IR until it is removed and fully verified.

## Migration/verification (mandatory full #719 scope)

G0: exhaustive declared+dynamic path and consumer inventory, frozen clean-room semantic baseline and accepted ADR; G1: full typed RuntimeValue + conversion; G2: EvalSession/context/journal; G3: all AST calls/chains/controls; G4: all nested AST/callable/resource bodies; G5: immutable final DocumentConfig; G6: final-only IR and pure backend; G7: versioned wire, CLI, public facade and host migrations; G8: complete cross-platform, source-map, diagnostic, rollback, performance and security acceptance before closing #719. Each G0–G8 gate has mandatory exit proof; total PR count may vary. #718/#682 continue to own their features but contribute mandatory provenance/conversion integration proof.

**Accepted target only:** this ADR does not itself complete G0, authorize premature G1 work, change production code or public wire, merge PR #746 or close #719. All G0–G8 evidence and acceptance gates remain mandatory.
