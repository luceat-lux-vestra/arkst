# ADR-0022 — Separate evaluator runtime from final document IR

- **Status:** Proposed
- **Date:** 2026-11-10
- **Owner:** [#719](https://github.com/luceat-lux-vestra/arkst/issues/719)
- **Companion source audit:** [G0 execution-surface ledger](../architecture/719-g0-audit.md)
- **Basis:** `main@54a7285ca7adce7482bc27df587f156a6162a3f9`

## Context

ADR-0020 and `docs/ARCHITECTURE.md` currently permit a single `IrDocument` to carry both executable pre-evaluation syntax and normalized backend output. #719 explicitly rejects this mixed lifecycle. The current normal compile starts from AST but still lowers unmatched blocks/chains, callable bodies, inline values and nested nodes into executable IR; `RuntimeValue` still bridges through `IrValue`, and the wire accepts executable payloads.

## Decision proposed for acceptance

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

**Wire and public interfaces:** use a separate versioned DTO/serialization contract with deterministic source-table normalization, source-span range validation, invalid payload rejection and explicit legacy decode/migrate/reject policy. Update `inspect --emit ir` and exported APIs. Public `arkst-core::ast_to_ir` and `evaluator::evaluate(IrDocument)` cannot remain backdoors for executable final IR; their deprecation/removal/migration follows a recorded semver compatibility decision and tests.

**Backend/host:** `arkst-typst` and native/in-process hosts accept final-only types with exhaustive pure lowering and source-map generation. Explicit VirtualProject/resource/library/env/logger/capabilities and WASM constraints remain; no new ambient filesystem, network, process, or global mutable state. No generalized HIR, MIR, bytecode VM or replacement renderer is introduced without separate measured justification.

## Exact supersession boundary

**On acceptance**, this decision supersedes the following *statements* (not unrelated features) of [ADR-0020](0020-programmable-document-semantic-model.md) and `docs/ARCHITECTURE.md `IR Model`:

- One `IrDocument` type containing both executable initial and final output.
- Universal `IrValue` as the language runtime value carrier.
- Unresolved executable `FunctionCall` as valid completed final IR, or durable `IrCallable` captures.
- Serialization of executable call/bodies as part of a final document.

Preserve ADR-0020's accepted typed backend-neutral semantic component decision, strict target-driven conversion, source provenance guarantees, output materialization, diagnostics and Typst source-map responsibility. ADR-0015 crate ownership remains applicable with the explicit separation of runtime versus final schema and wire. ADR-0016 clean-room and compatibility evidence policy is unchanged.

Before acceptance, status remains **Proposed**; historic accepted ADRs remain accurately labeled and contradictions are identified rather than retroactively hidden. After review, update `docs/ARCHITECTURE.md` and ADR-0020 with precise supersession annotations.

## Migration/verification (mandatory full #719 scope)

G0: exhaustive declared+dynamic path and consumer inventory, frozen clean-room semantic baseline and accepted ADR; G1: full typed RuntimeValue + conversion; G2: EvalSession/context/journal; G3: all AST calls/chains/controls; G4: all nested AST/callable/resource bodies; G5: immutable final DocumentConfig; G6: final-only IR and pure backend; G7: versioned wire, CLI, public facade and host migrations; G8: complete cross-platform, source-map, diagnostic, rollback, performance and security acceptance before closing #719. Each G0–G8 gate has mandatory exit proof; total PR count may vary. #718/#682 continue to own their features but contribute mandatory provenance/conversion integration proof.

**Proposal only:** creating this ADR is not evidence that G0 or migration is complete. No Phase 3J implementation follows automatically.
