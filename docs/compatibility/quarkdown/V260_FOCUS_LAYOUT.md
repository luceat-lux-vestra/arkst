# Quarkdown v2.6 `focus` layout compatibility contract

Parent tracker: #311  
Implementation issue: #328

This document records the clean-room output contract used by Arkst for the
Quarkdown v2.6 `focus` layout theme. It intentionally does not describe or
translate Quarkdown implementation source.

## Oracle

Pinned Quarkdown release: `v2.6.0`, release commit
`22f3c1169d0b1356fb51d8f43e1833d3d815aab1`.

Official Linux x64 release artifact:

- `quarkdown-linux-x64.zip`
- SHA-256 `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`

The executable identified itself as `quarkdown version 2.6.0`.

## Independently-authored probes

The oracle was compiled with the same document body while changing only the
layout theme between `latex` and `focus`. The probe covered `plain`, `paged`,
and `slides`, plus `focus` with omitted color and explicit `paperwhite`.

Observed invariants:

1. The generated HTML body was byte-identical between `latex` and `focus` for
   each probed document type. Layout selection was instead observable through
   the emitted theme asset set and through browser-computed styles.
2. `focus` and `focus` with explicit `paperwhite` produced the same computed
   style/geometry observations in the probe.
3. `plain` and `slides` showed a distinct focus presentation: body text scaled
   from the control's 16px to 17.6px; level-1/level-2 headings became larger,
   high-spacing dark bands with white text under `paperwhite`; block code was
   approximately 0.75 of body text.
4. The probed `paged` document showed no difference from the `latex` control in
   the collected computed-style and geometry properties. Arkst therefore does
   not invent a paged `focus` delta from this evidence.
5. An unknown layout emitted a renderer warning (`'layout' theme not found`)
   but exited successfully and still produced HTML, even with `--strict`.
   Theme-name existence is therefore not promoted to an evaluator hard error.

Representative computed ratios used by the Typst adaptation:

| Property | `plain` focus observation |
|---|---:|
| body scale | `17.6 / 16 = 1.10` |
| H1 / focus body | `48.4 / 17.6 = 2.75` |
| H2 / focus body | `32.912 / 17.6 = 1.87` |
| H1 top margin / body | `112 / 17.6 ~= 6.36` |
| H1 top inset / body | `144 / 17.6 ~= 8.18` |
| H1 bottom margin / body | `64 / 17.6 ~= 3.64` |
| H2 top margin / body | `40 / 17.6 ~= 2.27` |
| H2 bottom margin / body | `32 / 17.6 ~= 1.82` |
| block-code / body | `13.2 / 17.6 = 0.75` |

For slides, the probe observed zero H1/H2 top margin and an H1 top inset of
`192 / 17.6 ~= 10.91`.

## Arkst mapping

The evaluator-owned `IrDocumentTheme` remains unchanged. `layout = "focus"`
is interpreted only by the Typst output layer.

The Typst adaptation currently applies only to `Plain` and `Slides` and
preserves the legacy lowering path for `Paged`, `Docs`, and all non-`focus`
layouts. It preserves source provenance by shifting generated source-map ranges
by the exact prelude byte length.

The adaptation implements the representable evidence-backed invariants:

- 1.10 body scale;
- distinct level-1 and level-2 heading scale and spacing;
- `paperwhite`/omitted-color dark heading band with white heading text;
- 0.75 body-relative block-code scale;
- slide-specific H1/H2 top-spacing observations.

## Explicit divergences / unpinned surfaces

Quarkdown's observed focus output used Source Sans Pro for body text, Fira Sans
for headings, and Noto Sans Mono for code. Arkst's in-process Typst world is
deterministic and does not discover host fonts. Typst's guaranteed embedded
font set does not include the two required sans families, so Arkst does not
pretend to reproduce that font identity. The current adaptation leaves font
family selection to Typst's deterministic available fonts while preserving the
measured scale/layout invariants.

Other color themes combined with `focus` were not pinned by the oracle. Arkst
therefore applies focus geometry/scale but does not invent `paperwhite` colors
for an explicitly different color theme.

`Docs` applicability was not part of the pinned black-box probe and receives no
focus-specific Typst prelude. HTML output remains owned by #320 and is not
implemented here.
