# Quarkdown v2.6 row/column omitted-alignment inheritance

Issue: #336. Prerequisite document-global alignment state: #338 / #339.

## Clean-room oracle

The compatibility contract was observed with the official Quarkdown v2.6.0 Linux x64 distribution, SHA-256 `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`, using independently authored fixtures only. Disposable probe PR #337 ended at exact HEAD `37dbb72e891ac1a4374cb80a3892d502b4e7c46f` and was closed without merge.

Observed invariants:

- omitted `.row` and `.column` do not publish an explicit main-axis override;
- final document-global `.pageformat alignment:{start|center|end}` controls omitted stacks, including stacks that occur before the final setter in source order;
- explicit stack `alignment` remains authoritative, including distributed values such as `spacebetween`;
- `.align` and `.center` wrappers do not become the inheritance source;
- plain/slides targeted defaults without a global override remain `start`;
- global `justify` is text/content justification and does not become a stacked main-axis value;
- source-defined `row` / `column` keep ownership and invalid explicit alignment remains fail-closed.

## Arkst ownership

`IrStackedComponent.main_axis_alignment` is optional. `None` means only that row/column source omitted the main-axis argument; explicit `start` is `Some(Start)`. Grid retains its existing explicit/defaulted `Center` semantics and does not inherit document page alignment.

The evaluator preserves omission rather than resolving it at call time because the oracle proves that a later `.pageformat` setter affects an earlier stack. Typst lowering resolves row/column omission from the final immutable `IrDocumentState.page_alignment`: `start -> Start`, `center -> Center`, `end -> End`, and unset/`justify -> Start`. The `justify` fallback is deliberate: the clean-room oracle proves no stacked `justify` value exists.

Document serialization keeps the existing concrete `main_axis_alignment` wire field for legacy readers and adds `main_axis_alignment_inherited: true` only when omission provenance must survive a round trip. New readers use that additive flag to restore `None`; legacy serde readers ignore it and continue to consume the concrete `Start` fallback for omitted rows/columns. Explicit alignments do not emit the flag.

This keeps the IR backend-neutral and preserves the observable distinction required by other output targets; no editor, host, CSS, browser, or Quarkdown renderer concept enters Arkst IR.

## Verification

Regression coverage pins omission versus explicit alignment, final-setter semantics, wrapper non-inheritance, nested override independence, source-function shadowing, backward-compatible serde, grid non-regression, target lowering, and a real pinned-Typst PDF integration path.

The v2.6 migration checklist is intentionally not changed in this implementation PR; #336 requires that bookkeeping update only after the implementation lands.
