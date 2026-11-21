# #719 G1 — residual runtime / IR bridge inventory

- **Status:** refreshed for the G1-exit cleanup candidate; **not yet G1 exit proof**
- **Protected baseline:** `main@76a475d9676bf7133a003be2283e33a20adb48c0` (#818 / G1-L1t reconciled)
- **Authority:** [issue #719](https://github.com/luceat-lux-vestra/arkst/issues/719), [ADR-0022](../adr/0022-runtime-final-document-ir.md), [G0 surface ledger](719-g0-audit.md)
- **Purpose:** classify remaining runtime/IR boundaries by lifecycle. Raw token counts are evidence aids, not completion percentages.

The previous version of this file was a frozen snapshot from `main@68afd804ef0aa5376575f905fe7aa53f7ad0435b`. Its R01–R12 queue is historical: later G1-L1d through G1-L1t removed the live evaluator/builtin projection consumers it described. Historical checkpoint evidence remains preserved in [719-checkpoint-archive-2026-11-20.md](719-checkpoint-archive-2026-11-20.md).

## Current landed baseline before this candidate

At protected `main@76a475d9676bf7133a003be2283e33a20adb48c0`:

- `evaluator.rs` retains the reconciled zero-direct-projection baseline.
- production `builtins.rs` has zero direct `.semantic_value()`, `.to_ir_value()`, and `.into_ir_value()` consumers.
- `RuntimeValueKind` owns Unit/None/scalars, Range/Size/Color/Enum, Pair/Dictionary/Collection, Content/Component, Callable and InlineBody.
- `InvocationValue` and `SemanticLeaf(IrValue)` are absent from the active runtime carrier.
- the remaining generic reverse projection implementation is concentrated in `value_conversion.rs`.
- executable/mixed IR remains intentionally present for later G3/G4/G6/G7 migration.

## Lifecycle classification

### A — G1 blockers

These are evaluator-runtime values that are projected through `IrValue` and immediately consumed again as runtime state, or generic production escape hatches that allow that pattern to return.

The G1-exit cleanup candidate owns all of them together:

1. **Contextual InlineBody → Callable target roundtrip**
   - previous route: `RuntimeInlineBody -> RuntimeCallable -> IrCallable -> IrValue::Callable -> RuntimeValue`;
   - target route: build evaluator-owned `RuntimeCallable` directly and materialize `RuntimeValueKind::Callable` without crossing `IrValue`.

2. **Conversion-generated scalar/domain/content values**
   - previous route: construct a fresh `IrValue`, call `RuntimeValue::static_value`, then immediately decompose it;
   - target route: runtime-native constructors for Number/Boolean/String, Range/Size/Color/Enum, Content and Callable.

3. **Builtin generated-result factories**
   - previous route: `static_result(IrValue)` or `RuntimeValue::static_value(IrValue::...)`;
   - target route: generated builtin results are constructed directly as runtime values. Test compatibility wrappers may still project to IR under `#[cfg(test)]`.

4. **Generic reverse projection API**
   - `semantic_value`, `to_ir_value`, `into_ir_value`, Pair reverse projection, callable/capture reverse projection, `replace_semantic_value`, and `PartialEq<IrValue>` no longer have a production consumer after L1d–L1t;
   - retain them only under `#[cfg(test)]` while the existing roundtrip/regression tests still exercise exact legacy semantic parity.

**G1 exit condition for category A:** no production runtime→IR→runtime roundtrip and no production generic RuntimeValue→IrValue projection escape hatch.

## B — allowed at G1 exit

The crate boundary does not itself define runtime ownership. Immutable backend-neutral semantic leaf structures can remain shared where they carry no evaluator execution state.

Allowed examples:

- `IrRange`
- `IrSize`
- `IrColor`
- `IrEnumValue`
- `IrComponent`
- semantic `IrNode` content that is already evaluated and carried as runtime Content during this migration stage

G1 does **not** require inventing duplicate RuntimeRange/RuntimeSize/etc. types merely to remove the `arkst-ir` name from these leaf payloads.

## C — later-gate migration debt

These boundaries are intentionally not promoted into G1 blockers:

| Boundary | Current purpose | Required owner |
| --- | --- | --- |
| `RuntimeValue::new(IrValue, origin)` / `static_value(IrValue)` | one-way legacy IR ingress while old execution/facade paths still exist | G3/G4/G7 retirement with legacy routes |
| callable captured values reconstructed from `IrCallableCapture` | one-way legacy callable ingress | G4 |
| `RuntimeCallable.body: Vec<IrNode>` | callable bodies still execute through transitional mixed IR | G4 |
| `RuntimeInlineBody::{content,body}: Vec<IrNode>` | contextual body carrier until direct nested AST execution | G4 |
| `numbering::format_from_value(&IrValue)` | document metadata is still stored in legacy IR-shaped state; conversion uses the one-way runtime ingress only | G5/G6 |
| `ast_to_ir` and the core compatibility facade | pre-evaluation/executable IR route | G3/G4/G7 |
| executable `IrNode::{FunctionCall,ChainedFunctionCall,FunctionDeclaration}` | transitional execution model | G3/G4/G6 |
| `IrInline::{DirectiveCall,ChainedDirectiveCall}` | transitional nested executable model | G4/G6 |
| durable `IrValue::{Callable,InlineBody}` and executable wire variants | mixed final/runtime schema | G6/G7 |
| Typst handling of executable/mixed IR | negative baseline until final-only IR | G6 |

The G1 candidate must not remove these prematurely or introduce a parallel ad-hoc body/bytecode model.

## G1-exit cleanup candidate acceptance

The candidate is acceptable for PR creation only if authoritative source read-back proves:

- production `builtins.rs` has no generated `IrValue -> RuntimeValue` immediate wrapper;
- `materialized_target_value(IrValue)` is absent;
- the InlineBody→Callable target does not call `into_ir()` or construct `IrValue::Callable`;
- generic reverse-projection helpers are `#[cfg(test)]`-confined;
- one-way legacy ingress remains explicit and documented instead of being disguised as a generated-value factory;
- origin remains `Static` and provenance remains `Materialized` for newly materialized target values, matching the previous observable contract;
- existing source-backed identity conversions continue to preserve the original runtime carrier and provenance;
- no #682 structured-String widening, #718 table feature implementation, final-IR schema change, wire change, backend change, or G2+ implementation is introduced.

Focused regression ownership includes runtime-native Integer/Color conversion, InlineBody→Callable conversion, InlineBody→Content conversion, origin/provenance preservation, plus the existing scalar/domain/content/callable roundtrip and fail-closed suites.

## Exit sequencing

This candidate is an implementation candidate only. Do not declare G1 complete from source shape alone.

1. authoritative candidate read-back;
2. PR creation;
3. exact-HEAD required CI qualification;
4. guarded squash merge;
5. post-merge reconciliation;
6. **fresh G1 exit proof** against protected main.

If the fresh proof finds category A empty and the G1 integration evidence remains green, G1 may then be accepted and the next architectural gate becomes G2. Category C remains tracked by its later mandatory gate instead of being silently reclassified as completed work.
