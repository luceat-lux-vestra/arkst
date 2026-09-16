# Quarkdown v2.6 slides PDF export compatibility

Issue: #352. Parent migration checklist: #311. Disposable clean-room oracles: #353 and #355.

## Decision

The Quarkdown v2.6 `slides` PDF-export changes are **applicable to Arkst's current output surface**, but current Arkst is observably divergent from the v2.6 target and the migration row remains incomplete.

This is not the same ownership result as the v2.6 Chromium-export adapter classification in `V260_PDF_EXPORT_ADAPTER.md`. Arkst does not own Quarkdown's browser adapter, but Arkst **does** already own a `.doctype {slides}` → Typst → PDF path. A slides-PDF output invariant therefore cannot be classified current-surface N/A merely because the two projects use different renderers.

The relevant outstanding baseline subcontracts are already tracked by existing owners:

- #175 owns document-wide typed layout / `.pageformat` state, document-type defaults, and their Typst/PDF consumption;
- #178 owns document-type-gated `.slides` configuration, including nullable vertical centering and slide-capable renderer behavior;
- #185 owns explicit `.pagebreak` / `<<<` content breaks, which are needed to reproduce general interior headerless-slide boundaries in Arkst.

This classification does **not** require every unrelated part of #175/#178/#185 to land before work can proceed. It requires the applicable subcontracts to be defined rather than bypassed with a v2.6-only special case.

The implemented v2.6 H1/H2 automatic page-break slice (#332/#334) remains valid and bounded. It is orthogonal evidence and does not establish general slides layout or PDF parity.

## Official v2.6 claim

The official Quarkdown v2.6.0 release notes state that PDF artifacts generated from `slides` documents have a more polished layout and that blank/headerless slides are spaced correctly.

That language is intentionally treated as an output-level compatibility claim. It does not identify a backend-neutral semantic field or a concrete geometry that Arkst may infer without observation.

The exact complete set of changes covered by the phrase **"more polished layout" is UNKNOWN** under the allowed clean-room evidence. The migration must not reconstruct unspecified upstream layout rules by guesswork.

The release note says `Blank (headerless)`. The black-box evidence below pins **headerless/body-only slides**. It does not establish an invariant that adjacent breaks must synthesize an empty-content PDF page.

## Clean-room evidence identity

No Quarkdown implementation source was inspected for this slice. Evidence used only:

- official v2.6.0 release notes;
- official distributed v2.5.1 and v2.6.0 Linux x64 binaries as black boxes;
- independently authored fixtures;
- generated presentation artifacts, runtime layout observations, and produced PDFs.

Pinned distributions:

| Version | Linux x64 SHA-256 |
|---|---|
| v2.5.1 | `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9` |
| v2.6.0 | `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4` |

### Initial oracle (#353)

Final HEAD: `5674cb3efb923034b54f504baa48a0cd0f8a82fa`.

Reference-version run:

- workflow run `35037462289` — success;
- artifact `10423593387`;
- artifact digest `sha256:f462a5c8c9d2dbfdd6a0df18b9b86d1e96c7a4f8a1924e1412656708ea6bcd8f`.

Current-Arkst run:

- workflow run `35037462367` — success;
- artifact `10423303799`;
- artifact digest `sha256:f281d649080bb6408675f352e7f641ab1dd941fa2d43a01fcb52286e7af6fbd5`.

### Adversarial follow-up (#355)

Strict review found that the first oracle had not covered the full risk matrix from #352, so a second disposable probe tested automatic H1/H2 boundaries, consecutive interior headerless slides, adjacent explicit breaks, and `focus` as a separate layout control.

Final successful HEAD: `5ec5b46f0b788940eb1e19402ef3fc5ab1970c33`.

- workflow run `35044127244` — success;
- artifact `10426002679`;
- artifact digest `sha256:872d53115459c5018ce39eee618f8b55b6481d015d4f14b45e692362e4e8cdbe`.

Both #353 and #355 were closed unmerged after evidence capture.

## Observed v2.5.1 → v2.6.0 presentation delta

### Headerless/body-only runtime spacing

The generated presentation artifacts expose a repeatable change for the probed layouts:

- v2.5.1 body-only/headerless sections have computed `padding-top: 0px`;
- v2.6 body-only/headerless sections have computed `padding-top: 48px`;
- the independent `short-long` and `heading-only` controls reproduce the difference;
- the adversarial `consecutive-headerless` fixture creates two distinct interior body-only sections: both are `0px` in v2.5.1 and both independently become `48px` in v2.6;
- under v2.6 `focus`, the probed body-only section also has `padding-top: 48px`, while its final PDF marker placement differs slightly from the default layout.

The observed `48px` value is a **reference-renderer output fact for the bounded fixtures**, not an Arkst IR constant. Arkst must not copy it into backend-neutral state without separate semantic evidence.

### Adjacent breaks are not an empty-slide invariant

The adversarial fixture with two adjacent explicit `.pagebreak` calls produces only two rendered sections in both v2.5.1 and v2.6.0. The v2.6 native PDF likewise contains only two pages.

Therefore this evidence does not define “blank” as an empty-content page created by repeated break commands. The pinned compatibility concern is headerless/body-only slide layout. Explicit break semantics remain #185-owned because Arkst needs them to express interior/consecutive headerless cases, not because adjacent breaks must create empty pages.

### Automatic H1/H2 boundaries remain separate

The default `slides` fixture containing one H1 and two H2 headings is one rendered section in v2.5.1 and three sections in v2.6.0. The v2.6 native PDF has three pages.

That result agrees with the already-implemented #332/#334 contract. It is regression evidence that the PDF-layout investigation does not redefine automatic heading boundaries; it is not a second implementation requirement for this row.

## Official v2.6 native PDF observations

The official v2.6 native PDF exporter succeeds for the initial and adversarial fixtures. Representative results include:

| Fixture | Pages | Page size |
|---|---:|---|
| `headerless-body` | 3 | `749.04 x 546 pt` |
| `heading-only` | 2 | `749.04 x 546 pt` |
| `short-long` | 2 | `749.04 x 546 pt` |
| `consecutive-headerless` | 4 | `749.04 x 546 pt` |
| `empty-double-break` | 2 | `749.04 x 546 pt` |
| `auto-headings` | 3 | `749.04 x 546 pt` |
| `focus-headerless` | 1 | `749.04 x 546 pt` |

For the default-layout consecutive-headerless fixture, both body-only markers appear at approximately `y=36.779295 pt`; headed markers appear separately at approximately `y=40.889647 pt`. In the `focus` body-only fixture, the marker appears at approximately `y=37.261198 pt`.

These PDF coordinates and the runtime `48px` inset are **separate observations**. The oracle does not establish an exact CSS-pixel-to-PDF-coordinate causal or unit mapping. In particular, a usable v2.5.1 native-PDF control could not be produced in the runner, so the runtime 0→48px delta must not be restated as a measured native-PDF coordinate delta.

The v2.5.1 manual distribution's PDF command returned process status 0 in the runner but emitted a Puppeteer-missing diagnostic and produced no PDF artifact. Native v2.5.1 PDF page geometry and marker coordinates therefore remain **UNKNOWN** in this environment. Process exit status alone is not treated as successful artifact evidence.

The initial oracle also attempted direct headless-Chrome `--print-to-pdf` against generated HTML. That path did not reproduce Quarkdown/Reveal pagination, so its PDF page counts and marker results are excluded from the compatibility conclusion.

## Current Arkst artifact

A separate oracle compiled this independent current-surface fixture through `arkst-cli` and the in-process Typst backend:

```quarkdown
.doctype {slides}

QD-ARKST-HEADERLESS-FIRST

Body-only first slide before the first automatic heading boundary.

# QD-ARKST-HEADED-SECOND

Second slide body.
```

Current Arkst lowering emits materially:

```typst
QD-ARKST-HEADERLESS-FIRST

Body-only first slide before the first automatic heading boundary.

#pagebreak(weak: true)
= QD-ARKST-HEADED-SECOND

Second slide body.
```

There is no generic slide page-geometry or slide-layout prelude in this output. The subprocess backend also feeds the generated Typst source to the official Typst compiler without adding a slide-layout wrapper, so this is not an in-process-only omission.

The real Typst 0.15.1 PDF artifact is:

- 2 pages;
- `595.276 x 841.89 pt` (A4 portrait);
- headerless marker on page 1 at approximately `x=70.866`, `y=68.270`;
- headed marker on page 2 at approximately `x=70.866`, `y=67.032`.

This proves a **current Arkst → v2.6 target divergence** in the owned slides/PDF surface. It does **not** prove that Quarkdown's page dimensions themselves changed from v2.5.1 to v2.6.0; native v2.5.1 PDF geometry is unknown here. The A4 result therefore identifies a missing baseline slide-layout contract that must be reconciled when implementing target conformance, rather than a measured v2.6-specific page-size delta.

## Risk and ownership boundary

The evidence is specifically designed to prevent several incorrect implementations:

- **Renderer constant leakage:** `48px` must not become generic evaluator/IR state.
- **False N/A:** Arkst already owns slides→PDF output, so renderer choice does not remove the compatibility obligation.
- **Theme overgeneralization:** `focus` retains the runtime headerless inset but has different final PDF placement; exact coordinates are layout-specific evidence.
- **Baseline/delta conflation:** Arkst A4 versus v2.6 presentation geometry is a target divergence, not proof of a v2.6 page-size change.
- **Blank/empty conflation:** adjacent breaks do not create an empty slide in the observed reference behavior.
- **Automatic-break regression:** H1/H2 sectioning remains the separately owned #334 behavior.

The applicable outstanding subcontracts are:

1. #175: establish the document/page geometry and document-type default contract needed by the Typst/PDF path instead of relying accidentally on Typst's A4 default;
2. #178: establish slides-specific configuration and target-aware layout policy, including the relationship between default layout and vertical centering;
3. #185: provide explicit page-break content semantics needed to construct and test general interior/consecutive headerless slides;
4. the Typst/PDF output path: consume those contracts and implement the evidenced v2.6 output behavior without importing Reveal/CSS implementation details into generic compiler semantics.

Future whole-document HTML ownership under #320/#347 may need its own conformance tests, but it is not a reason to declare the current Typst/PDF behavior N/A.

## Completion criteria

The v2.6 migration-checklist row remains unchecked until the applicable baseline subcontracts are implemented and the Arkst-owned PDF path has target-specific conformance evidence.

At minimum, future regression/adversarial tests should cover:

- default `slides` page geometry rather than accidental Typst defaults;
- headed and body-only/headerless slides;
- multiple consecutive interior headerless/body-only slides using explicit boundaries once #185 supplies them;
- adjacent explicit breaks without fabricating an empty page unless separately evidenced;
- automatic H1/H2 heading boundaries without accidental extra pages, preserving #334;
- short and long body-only slides;
- default layout versus `focus` as separate renderer/layout evidence;
- interaction with the supported `.slides` centering/default-layout contract once the applicable #178 subcontract lands;
- real PDF page count, page dimensions, and content-position invariants on the Arkst backend.

Arkst needs behavioral equivalence at its owned PDF boundary, not Quarkdown's exact CSS classes, DOM structure, or browser implementation.

Any part of the release-note phrase "more polished layout" that is not pinned by clean-room evidence must remain `UNKNOWN`; it cannot be converted into guessed geometry during implementation.
