# Quarkdown v2.6 auto page-break compatibility

Issue: #332. Parent migration checklist: #311. Existing configuration owner: #175.

## Clean-room oracle

The compatibility contract was observed with the official Quarkdown v2.6.0 Linux x64 distribution (`quarkdown version 2.6.0`), SHA-256 `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`, using independently authored fixtures. The disposable probe PR is #333 and was closed unmerged after evidence capture.

Observed final-document semantics:

- implicit maximum heading depth is 0 for `plain`, 1 for `paged`, 2 for `slides`, and 0 for `docs`;
- explicit `.autopagebreak maxdepth:{0,1,2,3}` selects none / H1 / H1+H2 / H1+H2+H3 respectively;
- `.noautopagebreak` is the zero-depth shorthand;
- the final global setting applies to headings that occur both before and after the setter in source order;
- a final `.doctype {slides}` likewise applies the slides implicit default to preceding headings;
- negative depth fails before state mutation;
- source-defined `autopagebreak` and `noautopagebreak` functions shadow the native names;
- nested H2 elements receive the reference HTML marker but do not create a rendered Reveal `.slides > section`; top-level H2 does;
- heading-at-start and adjacent automatic/manual break observations do not create visible empty slides, motivating weak backend breaks.

## Arkst ownership

`IrDocumentState.auto_page_break_max_depth` stores only an explicit backend-neutral override. `None` means that the output path derives the implicit threshold from the final `IrDocumentType`; it does not mean zero.

The evaluator owns binding, integer conversion, non-negative validation, transactional publication, and source-function shadowing. It does not stamp heading-local history into IR because the oracle proves that the reference behavior uses final document configuration.

Typst lowering owns page-boundary emission. It emits `#pagebreak(weak: true)` only before qualifying top-level headings. Nested headings are deliberately not given Typst page breaks: the observed rendered slides do not split there, and Typst page breaks are not valid inside containers.

## Verification

The bounded regression suite covers final-state ordering, explicit thresholds, `.noautopagebreak`, negative rollback, source shadowing, backward-compatible serde omission, document-type defaults, top-level-only lowering, weak break emission, and real in-process Typst PDF page counts. Existing CI supplies the repository's pinned Typst 0.15.1 dependency and platform/WASM checks.

Manual `.pagebreak` / `<<<` syntax remains owned by the existing content/page primitive work; this slice does not broaden that primitive. The automatic break output is weak so an adjacent backend/manual page boundary can collapse without creating an empty page.
