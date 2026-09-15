# Quarkdown v2.6 slides PDF export compatibility

Issue: #352. Parent migration checklist: #311. Disposable clean-room oracle: #353.

## Decision

The Quarkdown v2.6 `slides` PDF-export changes are **applicable to Arkst's current output surface**, but current Arkst is observably divergent and the migration row remains incomplete.

This is not the same ownership result as the v2.6 Chromium-export adapter classification in `V260_PDF_EXPORT_ADAPTER.md`. Arkst does not own Quarkdown's browser adapter, but Arkst **does** already own a `.doctype {slides}` → Typst → PDF path. Therefore a slides-PDF layout invariant cannot be classified current-surface N/A merely because the two projects use different renderers.

The missing baseline ownership is already tracked by existing issues:

- #175 owns document-wide typed layout / `.pageformat` state and page geometry;
- #178 owns document-type-gated `.slides` configuration, including nullable vertical centering and slide-capable renderer behavior;
- #185 owns explicit `.pagebreak` / `<<<` content breaks, which are required to represent the general blank/headerless-slide cases exercised by the reference fixtures.

The implemented v2.6 H1/H2 automatic page-break slice (#332/#334) remains valid and bounded. It does not establish general slides layout or PDF parity.

## Official v2.6 claim

The official Quarkdown v2.6.0 release notes state that PDF artifacts generated from `slides` documents have a more polished layout and that blank/headerless slides are spaced correctly.

That language is intentionally treated as an output-level compatibility claim. It does not identify a backend-neutral semantic field or a concrete geometry that Arkst may infer without observation.

The exact complete set of changes covered by the phrase **"more polished layout" is UNKNOWN** under the allowed clean-room evidence. The migration must not reconstruct unspecified upstream layout rules by guesswork.

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

Final disposable-oracle HEAD: `5674cb3efb923034b54f504baa48a0cd0f8a82fa`.

Reference-version run:

- workflow run `35037462289` — success;
- artifact `10423593387`;
- artifact digest `sha256:f462a5c8c9d2dbfdd6a0df18b9b86d1e96c7a4f8a1924e1412656708ea6bcd8f`.

Current-Arkst run:

- workflow run `35037462367` — success;
- artifact `10423303799`;
- artifact digest `sha256:f281d649080bb6408675f352e7f641ab1dd941fa2d43a01fcb52286e7af6fbd5`.

PR #353 was closed unmerged after evidence capture.

## Observed reference delta

The fixtures deliberately used the default slides layout and did not set `.slides` layout options. Therefore the measurements below are evidence for that bounded default configuration, not universal constants for all themes or future slide-layout options.

### Headerless/body-only spacing

The generated presentation artifacts expose a repeatable v2.5.1 → v2.6.0 change:

- v2.5.1 body-only/headerless sections have computed `padding-top: 0px`;
- v2.6 body-only/headerless sections have computed `padding-top: 48px`;
- in the independent `short-long` fixture, both v2.6 body-only sections have a 48 px first-content offset and section boxes 48 px taller than their v2.5.1 counterparts;
- the independent `heading-only` fixture likewise places the v2.6 body-only second slide at a 48 px top inset while v2.5.1 has no such inset.

The observed `48px` value is a **reference-renderer output fact for the bounded default fixture**, not an Arkst IR constant. Arkst must not copy it into backend-neutral state without separate semantic evidence.

### Official v2.6 native PDF

The official v2.6 native PDF exporter succeeded for all oracle fixtures:

| Fixture | Pages | Page size |
|---|---:|---|
| `double-break` | 2 | `749.04 x 546 pt` |
| `headerless-body` | 3 | `749.04 x 546 pt` |
| `heading-only` | 2 | `749.04 x 546 pt` |
| `short-long` | 2 | `749.04 x 546 pt` |

In the native PDF, a body-only marker starts at approximately `y=36.779 pt`; headed markers are separately positioned, for example `QD-HEAD-ONE` at approximately `y=40.890 pt`. This confirms that the headerless spacing is observable in the final PDF rather than being inert DOM metadata.

The oracle also attempted direct headless-Chrome `--print-to-pdf` against generated HTML from both versions. That path produced one page for every fixture and did not reproduce Quarkdown/Reveal PDF pagination. Those direct-print page counts and marker results are **excluded** from the compatibility conclusion. The relied-upon observations are the runtime presentation geometry plus the official v2.6 native PDF exporter.

## Current Arkst artifact

A separate oracle compiled this independent current-surface fixture through `arkst-cli` and the in-process Typst backend:

```quarkdown
.doctype {slides}

QD-ARKST-HEADERLESS-FIRST

Body-only first slide before the first automatic heading boundary.

# QD-ARKST-HEADED-SECOND

Second slide body.
```

Current Arkst lowering emits:

```typst
QD-ARKST-HEADERLESS-FIRST

Body-only first slide before the first automatic heading boundary.

#pagebreak(weak: true)
= QD-ARKST-HEADED-SECOND

Second slide body.
```

There is no generic slide page-geometry or slide-layout prelude in this output.

The real Typst 0.15.1 PDF artifact is:

- 2 pages;
- `595.276 x 841.89 pt` (A4 portrait);
- headerless marker on page 1 at approximately `x=70.866`, `y=68.270`;
- headed marker on page 2 at approximately `x=70.866`, `y=67.032`.

This is a concrete current-output divergence from the observed v2.6 slides PDF surface. The difference is broader than the Chromium executable-selection adapter handled by #342/#344.

## Ownership boundary

The evidence does **not** justify adding a `48px` field to evaluator state or backend-neutral IR, and it does not justify a one-off Typst inset special case.

Before this migration row can be completed, Arkst needs the baseline owners to define the actual contract:

1. #175 must establish the relevant document/page geometry instead of leaving slides on Typst's default A4 page;
2. #178 must establish slides-specific configuration and the target-aware layout policy, including the relationship between default layout and vertical centering;
3. #185 must provide explicit page-break content semantics so general headerless/blank slides can be represented and tested rather than only the special first-page-before-heading case;
4. the Typst/PDF output path must then implement and test the v2.6-observable behavior without importing Reveal/CSS implementation details into generic compiler semantics.

Future whole-document HTML ownership under #320/#347 may need its own conformance tests, but it is not a reason to declare the current Typst/PDF behavior N/A.

## Completion criteria

The v2.6 migration-checklist row remains unchecked until all applicable prerequisites are implemented and the Arkst-owned PDF path has target-specific conformance evidence.

At minimum, future tests should cover:

- default `slides` page geometry rather than accidental Typst defaults;
- headed and body-only/headerless slides;
- explicit blank/headerless slides once #185 lands;
- automatic heading boundaries without accidental empty pages;
- short and long body-only slides;
- interaction with the supported `.slides` centering/default-layout contract once #178 lands;
- real PDF page count, page dimensions, and content-position invariants on the Arkst backend.

Arkst needs behavioral equivalence at its owned PDF boundary, not Quarkdown's exact CSS classes, DOM structure, or browser implementation.

Any part of the release-note phrase "more polished layout" that is not pinned by later clean-room evidence must remain `UNKNOWN`; it cannot be converted into guessed geometry during implementation.
