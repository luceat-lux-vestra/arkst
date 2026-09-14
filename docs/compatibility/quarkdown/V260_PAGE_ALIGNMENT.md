# Quarkdown v2.6 document page-alignment compatibility

Issue: #338. Required by omitted stack-alignment inheritance in #336. Existing broader page-format owner: #175.

## Clean-room oracle

The compatibility contract was observed with the official Quarkdown v2.6.0 Linux x64 distribution, SHA-256 `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`, using independently authored fixtures. The disposable probe PR is #337 at exact probe HEAD `37dbb72e891ac1a4374cb80a3892d502b4e7c46f`; it was closed without merge after evidence capture.

Observed final-document semantics relevant to this bounded slice:

- `.pageformat alignment:{start|center|end}` changes the document-global horizontal alignment;
- the final setter applies to omitted `.row` / `.column` alignment even when the setter occurs after those components in source order;
- explicit stack alignment remains an independent override;
- document-global `justify` belongs to the page/text alignment domain and must not be fabricated into stacked main-axis alignment;
- `.align` and `.center` wrappers are component-local layout and are not the source of document-global page alignment.

## Arkst ownership

`IrDocumentState.page_alignment` stores only an explicit backend-neutral page/content alignment override. `None` means no bounded alignment-only `.pageformat` call has committed state. `IrDocumentAlignment` is a separate closed domain with `Start`, `Center`, `End`, and `Justify`; it deliberately does not reuse `IrMainAxisAlignment`.

The evaluator claims only a fail-closed alignment-only `.pageformat` shape: one named `alignment` argument, no positional arguments, no body, and no source-defined `pageformat` shadow. Wider page-format calls such as selector/range, geometry, margin, border, background, or column forms remain outside this native slice and cannot mutate `page_alignment`.

Argument evaluation and state publication reuse the evaluator's existing invocation transaction. A failed alignment conversion rolls back nested document-state writes and does not publish a partial page alignment. Source-defined `pageformat` retains precedence over the bounded native path.

## Verification

The independently authored regression suite covers `start` / `center` / `end` / `justify`, final-setter-wins including a setter after content, unsupported/selective shapes preserving the previous bounded state, invalid closed-enum rollback, nested document-state rollback, source-function shadowing, and backward-compatible serde omission/round-trip behavior.

This slice does not lower the page alignment to Typst on its own. #336 consumes the final document-global state when resolving omitted row/column main-axis alignment. Full `.pageformat` geometry, selectors/ranges, margins, borders, background, columns, and other #175 surfaces remain out of scope.
