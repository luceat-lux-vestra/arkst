# Quarkdown v2.5.1 layout, pagination, style, and document-configuration audit

Status: complete strict audit artifact for Issue [#153](https://github.com/luceat-lux-vestra/arkst/issues/153). This is evidence and backlog work only. It does not implement a layout/configuration surface or promote the verified compatibility baseline.

## 1. Audit identity and evidence policy

| Item | Pinned value |
|---|---|
| Quarkdown target | v2.5.1 at [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e) |
| Arkst audit base | [`4a9112a9ee840374350dd9a90b65f58cce96eb08`](https://github.com/luceat-lux-vestra/arkst/tree/4a9112a9ee840374350dd9a90b65f58cce96eb08), the squash merge of reviewed PR #174 / completed Issue #152 |
| Parent tracker | [#147](https://github.com/luceat-lux-vestra/arkst/issues/147) |
| Canonical manifest | [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv) |
| Offline guard | [`layout_document_configuration_audit.rs`](../../../crates/arkst-core/tests/layout_document_configuration_audit.rs) |

The audit uses the repository's clean-room policy: public upstream source
declarations and models at the exact pinned commit, public documentation,
public tests as behavioral evidence, independent Arkst tests/fixtures, and
current Arkst source/tests. Upstream source, tests, and fixtures were not
copied or translated into Arkst. The checked-out upstream tree was detached
at the full target SHA; tag or `main` links are not used as canonical
provenance.

The manifest is the machine-checkable inventory. It contains 47 rows: 20
#153-owned names and 27 adjacent layout/content/output names explicitly handed
to #154. The 20 names handed from #152 were re-audited from pinned evidence;
their previous #152 `NOT_APPLICABLE` values are not used as #153 conclusions.

The canonical #147 vocabulary is used exactly:
`SUPPORTED_END_TO_END`, `SUPPORTED_SEMANTICS`, `PARSED_ONLY`, `PARTIAL`,
`UNSUPPORTED`, `DEFERRED`, `BLOCKED`, `NOT_APPLICABLE`, and `UNKNOWN`.

## 2. Enumeration and ownership result

The pinned public registration sweep covered the `Document.kt` and `Slides.kt`
`@QFunction` declarations, their `@Name` aliases and annotations, the
document/layout/numbering/TeX/slides models, and adjacent `Layout.kt`,
`Primitives.kt`, `Text.kt`, and `Html.kt` declarations. The 20 #153-owned
callables are exactly:

`numbering`, `nonumbering`, `font`, `paragraphstyle`, `captionposition`,
`texmacro`, `pageformat`, `pagemargin`, `footer`, `currentpage`, `totalpages`,
`formatpagenumber`, `resetpagenumber`, `lastheading`, `autopagebreak`,
`noautopagebreak`, `marker`, `navigation`, `tableofcontents`, and `slides`.

No additional #153-owned public callable was found. The full signatures,
source declaration names, aliases, annotations, exact source ranges, current
Arkst evidence, status, gap, and follow-up are in the manifest. The most
important ownership decisions are:

- `.pageformat` owns the genuinely document-scoped `columns` field. It is not
  the same semantic as component-local `.row`, `.column`, or `.grid`.
- `.numbered` and the `numbered` parameter of `.heading` consume global
  numbering policy but are content/component primitives owned by #154. The
  global policy setter is #153-owned.
- `.text` font size/weight/style/variant is inline node styling owned by #154;
  `.font` is the document-wide font configuration owned by #153.
- `.figure`, `.table`, and `.code` are #154 content producers. Their caption
  placement is consumed from the separate #153 `.captionposition` state.
- `.pagebreak` is an explicit content node owned by #154. Automatic heading
  page-break policy is `.autopagebreak`/`.noautopagebreak` and is #153-owned.
- `.fragment` and `.speakernote` are slide content primitives owned by #154;
  `.slides` is the document-type-gated global configuration initializer owned
  by #153.
- `.htmloptions` is target-specific HTML output configuration owned by #154,
  not document layout state.

The audit therefore does not move component IR into `DocumentState`, create a
global style abstraction, or assign #154 semantics ahead of its audit.

## 3. Canonical status result

For the 20 #153-owned rows:

| Status | Count |
|---|---:|
| `SUPPORTED_END_TO_END` | 0 |
| `SUPPORTED_SEMANTICS` | 0 |
| `PARSED_ONLY` | 11 |
| `PARTIAL` | 9 |
| `UNSUPPORTED` | 0 |
| `DEFERRED` | 0 |
| `BLOCKED` | 0 |
| `NOT_APPLICABLE` | 0 |
| `UNKNOWN` | 0 |

### #175 final reconciliation snapshot — 2026-10-04

The seven #175-owned configuration surfaces remain intentionally conservative
under the pinned v2.5.1 target:

| Surface | Canonical status | Current-main boundary |
|---|---|---|
| `.numbering` | `PARTIAL` | Typed mutation/IR semantics are present; numbering-aware producers/output and complete default consumption remain downstream consumer work. |
| `.nonumbering` | `PARTIAL` | Atomic reset semantics are present; observable numbering output remains downstream consumer work. |
| `.font` | `PARTIAL` | Size-only ordered state is present; family classification/resource registration, fallback/default resolution, and renderer lowering remain unsupported here. |
| `.paragraphstyle` | `PARTIAL` | Typed partial-merge state is present; renderer/locale defaults and output lowering remain open. |
| `.pageformat` | `PARTIAL` | The implemented global and bounded decoration slices are retained; selector-scoped pre-pagination layout and unresolved renderer defaults intentionally fail closed at the current Typst boundary. |
| `.autopagebreak` | `PARTIAL` | Explicit setter semantics are implemented, but current output defaults follow the later v2.6 document-type contract and are not equivalent to pinned v2.5.1's global initial threshold of 1. |
| `.noautopagebreak` | `PARTIAL` | Explicit zero-disable semantics are implemented, while the same implicit-default version drift prevents a pinned-v2.5.1 end-to-end claim. |

This reconciliation does not downgrade implemented safety or state semantics.
It prevents later v2.6 adaptation evidence from being misread as pinned-v2.5.1
equivalence, and it records which remaining gaps are downstream-owned versus
intentional backend fail-closed boundaries. No row is promoted merely because
its current implementation is richer than the original audit baseline.

`PARSED_ONLY` is used deliberately for the 11 rows that still have only
recognition/source-retention evidence. A preserved `IrNode::FunctionCall` or
inline directive is not a successful setter, typed node, state mutation, or
renderer claim. Nine rows are conservatively `PARTIAL`: `.captionposition`,
`.numbering`, `.nonumbering`, the bounded size-only `.font`, `.paragraphstyle`,
the bounded `.pageformat` alignment/geometry slice, `.autopagebreak`,
`.noautopagebreak`, and the bounded `.slides` configuration/PDF slice. `PARTIAL` does not claim complete v2.5.1 output
equivalence; each row retains the residual contract recorded below.

## 4. Pinned upstream semantic contracts

The following is the semantic reconstruction behind the manifest. `null` or
omission is described separately wherever the upstream contract distinguishes
inheritance, renderer defaults, reset, or no-op behavior. The `@LikelyNamed`
and `@LikelyBody` annotations are recorded metadata; they are not treated as
runtime named-only or body-only restrictions. Regular binding and conversion
remain the #149 boundary.

### Numbering and document layout state

#### `.numbering` and `.nonumbering`

`numbering(merge: Boolean = true, formats: Map<String, Value<String>>)` is a
document-wide setter with a required body-compatible formats dictionary. The
format keys are `headings`, `figures`, `tables`, `equations`, `code`, and
`footnotes`. The typed built-in fields and `DocumentNumbering.extra` are
populated from the same input map: a built-in key is parsed for its typed
field and every input pair is reparsed into `extra`. Therefore built-in keys
can be present in both the typed fields and `extra`; `extra` is not an
unknown-keys-only map. Whether that duplicate storage is observable at the
renderer boundary remains an explicit evidence/deferment question and must
not be normalized away by #175. `none` creates an empty/non-counting
`NumberingFormat` for that key. The
format string reserves `1`, `a`, `A`, `i`, and `I` for decimal, lower alpha,
upper alpha, lower Roman, and upper Roman counters; every other character is
a fixed symbol and backslash escapes the next character.

The initial stored value is unset. The effective value is derived from
`DocumentType.defaultNumbering`: plain defaults math numbering, paged defaults
heading/figure/table/math numbering, and slides/docs have their pinned model
defaults. With `merge:true`, each supplied key is merged over the current
effective defaults; omitted keys remain enabled/unchanged. With `merge:false`,
the supplied map is a complete replacement and omitted keys are disabled.
`nonumbering()` is the no-output reset shorthand: it invokes the equivalent of
`numbering(merge:false, formats:emptyMap())`.

There is no getter and no document content output. The mutation is
document-scoped and must be atomic: map conversion and all format parsing must
finish before publication. Arkst now implements a bounded evaluator/IR slice:
shared binding plus body-compatible Dictionary conversion, parsed
`IrNumberingFormat` tokens, typed built-in fields, the required duplicate
`extra` storage for every input key, merge/replace mutation, `.nonumbering`
as an empty replacement, failure rollback, serde-compatible state, and the
document type in effect at commit time as the downstream default-numbering
basis. It deliberately does not fabricate unresolved document-type default
formats. Heading, figure, table, math, code, footnote, and custom-numbered
output consumers remain separate renderer/AST boundaries. Status is
`PARTIAL`; numbering-aware output and complete default resolution remain open
under #175 and the applicable content owners.

#### `.font`

`font(main: String? = null, heading: String? = null, code: String? = null,
size: Size? = null)` appends a document-wide `FontInfo` layer and returns no
value/output. The initial list is empty and renderer defaults apply. `null` or
omission leaves that field absent in the new layer. Font families can be
system, file, URL, or Google Fonts; non-system resources are registered in
media storage. Later layers have higher fallback priority. Family fields stack
as fallback configurations, while `size` is taken from the last specified
layer. If `heading` is absent, `main` may apply to headings unless the active
layout theme supplies a heading font.

The parameter domain is typed `String`/`Size` and font-family resolution has a
resource/media boundary. There is no getter, no reset function, and no
document content output. A later implementation must validate all family and
size candidates before one state publication, preserve source-defined
precedence, keep resource access in the host/project boundary, and avoid JVM or
filesystem assumptions in WASM-capable core crates.

Arkst now implements a bounded **size-only** evaluator/IR state slice. Calls
whose family fields are omitted or semantic `None` append ordered
`IrFontLayer` entries carrying an optional typed `Size`; explicit non-None
`main`/`heading`/`code` values fail closed instead of being stored as
unclassified strings before the resource/media boundary exists. Size
conversion completes before publication, nested failures roll back, callable
scopes share the state, source-defined `.font` retains dispatch precedence,
and the state is serde/backward compatible. System/file/URL/Google-family
classification and registration, fallback resolution, renderer defaults, and
Typst/HTML lowering remain open. Status is conservatively `PARTIAL`.

#### `.paragraphstyle`

`paragraphstyle(lineheight: Number? = null, letterspacing: Number? = null,
spacing: Number? = null, indent: Number? = null)` updates a global
`ParagraphStyleInfo`. The initial fields are null and therefore use the
renderer default. Each call builds a partial style and merges it over the
current style; omitted/null fields preserve the current field rather than
resetting it. The values are relative multipliers of font size, and locale may
alter renderer defaults (for example, Chinese paragraph indentation).

The setter returns no output and has no getter. All four numeric conversions
must complete before the merged state is published. This is distinct from
inline `.text` styling and from component-local spacing. Arkst now implements
a bounded evaluator/IR state slice: all four values use the shared numeric
conversion boundary; omission and explicit `none` preserve the current field;
successful calls merge atomically into serializable backend-neutral
`IrParagraphStyleInfo`; nested failures roll back; callable scopes share the
document state; and source-defined `.paragraphstyle` retains dispatch
precedence. Locale-sensitive renderer defaults and Typst/HTML paragraph output
remain downstream work, so status is conservatively `PARTIAL`.

#### `.pageformat`

`pageformat(side: PageSide? = null, pages: Range? = null,
size: PageSizeFormat? = null, orientation: PageOrientation =
documentType.preferredOrientation, width: Size? = null, height: Size? = null,
margin: Sizes? = null, bordertop/right/bottom/left: Size? = null,
bordercolor: Color? = null, background: Color? = null, columns: Int? = null,
alignment: NodeStyle.TextAlignment? = null)` appends a page-format layer and
returns no output. The stored list starts empty. At rendering time,
`DocumentLayoutInfo.getPageFormatsWithDefault` prepends the final document
type's `defaultPageFormat` to the stored layers and then merges selector groups
with later non-null fields taking priority. The HTML document builder passes
`document.type.defaultPageFormat` at that final rendering boundary. The
effective initial format is therefore A4 portrait for final `paged`, none for
plain/docs, and slide-specific behavior. A null side/range is global; `side`
and a finite, 1-based inclusive `pages` range select subsets of paged pages.
An open range end fails before mutation.

When `size` is present, its standard closed `PageSizeFormat` bounds are
rotated to the selected orientation. Explicit width/height override those
bounds. Later layers with the same selector override only their non-null
fields; omitted fields inherit through the selector group. Positive
`columns` is document-wide multi-column configuration; values below one are
discarded. The pinned `docs/page-format.qd` applicability table limits both
`margin` and `columns` to `plain`, `paged`, and `slides`; `docs`
is explicitly included for `background` and `alignment`, but not for those
two layout fields. The public border-side arguments have a cross-field exception to
that simple omission rule: if any of `bordertop`, `borderright`,
`borderbottom`, or `borderleft` is supplied, `hasBorder` is true and the new
`contentBorderWidth` is a non-null `Sizes` whose omitted side fields are
explicitly `Size.ZERO` (the upstream expressions are `borderTop ?: Size.ZERO`
and their corresponding side forms). Thus a later layer with only
`bordertop: 1px` zeroes omitted left/right/bottom widths rather than inheriting
them. If no border side is supplied, `contentBorderWidth` is null and the
previous border-width structure can inherit through the layer merge.

`bordercolor` is independent from that `hasBorder` calculation. A color-only
call therefore leaves `contentBorderWidth` null while setting
`contentBorderColor`: for the same selector it can inherit a prior width while
changing only the color. Without an inherited width, the pinned HTML
stylesheet still publishes the color and `border-style: solid`, while
`--qd-page-content-border-width` remains at its renderer/CSS default (`unset`).
This is the actual v2.5.1 output boundary behind the public KDoc statement
that a color-only border uses a default width; evaluator/IR work must not
fabricate a concrete width that the upstream setter never stored. Margins,
borders, border color, background, and text alignment are typed layout
domains. Plain and slides documents have documented renderer limitations, and
page-format data is not itself a getter or output node.

The state is genuinely document-scoped and must remain backend-neutral.
Arkst now has bounded `.pageformat` slices for explicit width+height geometry,
document alignment, a global positive column count, selector-free global
border/background decoration state, selector-free global margin state, typed
selector-free standard size/orientation selection, and an ordered
page-format layer snapshot that preserves successful bounded mutations in
source order. The snapshot now carries a bounded selector identity for typed `left`/`right` page-side selectors and finite 1-based inclusive page ranges
when both endpoints are explicit; the two selector dimensions may be combined.
Selector-scoped positive columns are now retained in the ordered layer state as
bounded state-only evidence. Scoped selector layers are state-only and must not leak into the flattened global fields consumed by current renderers. Geometry/alignment,
selector-free global margins, global positive columns, selector-free global
background, and the bounded standard-size selection have current Typst/PDF
consumers. Final `docs` output now rejects selector-free global `margin`
and `columns` before page setup instead of applying fields outside their
pinned applicability domain; ordered state remains canonical and legacy
flattened margin/columns receive the same guard. Supported `docs`
background output remains unchanged, while row/column alignment inheritance
remains unchanged;
page-border decoration state now has one bounded Typst/PDF consumer when the
final document is `paged` and explicit margin, committed border widths, and
explicit border color are all present; selector-free implicit-margin,
width-only, and color-only cases are explicitly backend-rejected with a
generated panic before page setup instead of silently omitting the requested
border. The pinned public contract documents `side`/`pages` selectors as paged-only,
but the upstream setter stores selector layers without rejecting the document type in
effect at the call site. Arkst therefore retains selector state until the final document
type is known. The Typst lowerer now rejects any selector-bearing `.pageformat` when
the final document type is `plain`, `slides`, or `docs`, preventing scoped state
from silently disappearing without imposing a stricter call-time rule. A selector may
still be declared before a later `.doctype {paged}` mutation and participate in the
existing bounded paged side/range output. Border cases outside the bounded explicit-value
page-side/range subset remain fail-closed. The size slice
preserves the closed standard-format domain, named size binding, and an
explicit portrait/landscape orientation when supplied; when orientation is
omitted it records the document type in effect at commit time as the downstream
preferred-orientation basis. The selected Typst/PDF backend resolves a standard-size selection to
explicit physical millimeter bounds for every final document type, matching the
pinned setter which materializes `size` to concrete `pageWidth`/`pageHeight`
before the layer is stored and the renderer which consumes those dimensions
without a final document-type gate. Explicit orientation rotates those bounds.
When orientation was omitted, the call-time document-type snapshot remains the
preferred-orientation basis instead of a later `.doctype` mutation: pinned
`plain`, `paged`, and `docs` prefer portrait while `slides` prefers
landscape. Because a valid explicit standard-size selection is materialized to complete physical
geometry for every final document type, it does not depend on final `paged`/`slides`
applicability. The unresolved-dimension panic is reserved for genuinely unsupported or incomplete
explicit-axis state: final `plain`/`docs` width-only is preserved because the pinned public
applicability table explicitly supports `width` there, while height-only remains unsupported;
`paged`/`slides` still require complete geometry or a resolvable standard-size base. Ordered selector-free dimension layers now determine current Typst/PDF
size-versus-axis precedence. `IrPageFormatState::compose_global_page_dimensions`
folds only global dimension-bearing layers in source order: a later standard
size clears both earlier explicit axes, then explicit width/height in that same
layer override only their respective axes. Legacy flattened geometry/size remain
a backward-compatible fallback when ordered dimension state is absent. The margin slice expands the documented
`Sizes` shorthand into explicit top/right/bottom/left state: one value applies
to every side, two values map vertical/horizontal, and four values map TRBL.
Three-value or otherwise malformed groups fail closed, and semantic `None`
preserves the previously committed effective global margin. When ordered
page-format state exists, the current Typst/PDF margin consumer resolves only
the exact selector-free/global layer group in source order and treats that
merged margin as canonical. Flattened `page_margin` remains a legacy-IR
fallback only when ordered page-format state is absent, so stale compatibility
state cannot diverge content margin from the ordered margin already used by the
border consumer. The four explicit sides still lower through the shared size
conversion boundary, with real pinned-backend PDF integration coverage. Pinned v2.5.1
contains a slide-margin documentation/runtime divergence: the public page-format table
lists `margin` for slides, while the setter KDoc says it is unsupported there, and the
HTML stylesheet applies explicit global margin to plain/docs body layout but not to the
Reveal slide content frame. Arkst therefore retains the successfully authored slide margin
in backend-neutral ordered state but does not reinterpret it as a Typst page inset for final
`slides`; this matches the observable slide-frame consumer instead of inventing content
geometry from the public-table claim. Non-positive or
semantic-None column inputs are discarded without replacing an earlier
positive global value. When ordered page-format state exists, the current
Typst/PDF columns consumer resolves only the exact selector-free/global layer
group in source order and treats that merged positive count as canonical.
Flattened `page_columns` remains a legacy-IR fallback only when ordered
page-format state is absent, so stale compatibility state cannot override or
revive a global columns value. The committed positive count still maps directly
to the page column configuration, with real pinned-backend PDF integration
coverage. The decoration slice preserves the pinned cross-field
border rule: once any non-null border side is supplied, omitted/null sides
become explicit zero; color-only input updates border color without fabricating
a border width and therefore preserves any previously committed width
structure. Semantic-None border/color/background inputs preserve prior
effective state. When ordered page-format state exists, selector-free/global
background output resolves the exact global selector group in source order and
treats that merged background as canonical; flattened `page_background` is a
legacy-IR fallback only when ordered page-format state is absent. The current
Typst/PDF background consumer maps that committed typed RGB/alpha color directly
to page fill without changing border state.
The bounded Typst/PDF border consumer uses page foreground coordinates to draw
the explicit content-area rectangle. Final `plain` and `paged` still require
explicit margin, border widths, and border color because those values locate and paint
the content-area rectangle without fabricating renderer defaults. Final `slides`
follows the pinned runtime separately: the Reveal frame itself receives the content-area
border mixin, so explicit border widths + color draw a full-slide frame with zero inset and
do not require or consume page-format margin. This also means an authored slide margin
cannot accidentally shrink the Arkst border rectangle. The pinned v2.5.1 renderer emits
border variables for the global format and applies its border mixin directly to plain
`main`, the Reveal slide frame, and paged content areas. The consumer preserves four
independent side widths and does not fabricate unresolved renderer-default width or color. When ordered
page-format state exists, selector-free border output now resolves the exact
global selector group in source order and treats that ordered result as
canonical for margin, border widths, border color, non-paged rejection, and
unresolved-default rejection. Flattened border compatibility fields are used
only when ordered page-format state is absent, preventing stale flattened state
from overriding the canonical layer contract while preserving legacy IR.
The ordered layer snapshot remains a prerequisite for unsupported selector-aware
layout fields, but selector-free border output now consumes its exact-global
merge directly; recording other layer fields or selector identity alone still
does not widen renderer support. Because upstream records page
selectors before final renderer applicability is known, the bounded state slice retains
typed selector layers independently of the call-time document type. The current Typst
boundary then requires the final document type to be `paged`; final
`plain`/`slides`/`docs` selector output fails explicitly, while selectors declared
before a later `.doctype {paged}` remain usable. Within that final paged boundary, the
selector slice accepts typed `left`/`right`, explicit finite positive page ranges, and
left-open ranges with a finite positive end. A left-open `pages:{..N}` range
normalizes its omitted start to page 1, matching the public 1-based inclusive
page contract while preserving the existing finite `IrPageRange` boundary.
Ranges without a finite end, page zero, and invalid sides still fail before
publication. Selector-scoped positive columns publish only to
`IrPageFormatLayer::columns` and participate in
the existing applicable-field merge; they do not replace
`IrDocumentState::page_columns`, so current Typst/PDF column lowering remains
global-only. The IR now exposes
a bounded exact-selector resolver that folds only layers with identical selector
identity in source order: later non-null fields replace earlier values while
omitted fields inherit. Distinct selector groups remain uncombined, so this does
not claim cross-selector precedence, selector-aware output, or size-versus-geometry
resolution. A second bounded IR-only helper filters the source-ordered layer list
for one positive page number plus a caller-supplied typed page side. Global,
side-only, finite-range-only, and combined selectors are included only when they
admit that explicit page. The helper preserves original source order and does
not infer page side from parity, merge applicable layers, or choose precedence.
Non-positive page numbers fail closed. A bounded transient field resolver now
folds those applicable layers in original source order: later non-null fields
replace earlier values while omitted fields inherit, including when a later
global layer follows an earlier scoped layer. The merged value carries no selector identity. A second bounded IR-only
`compose_applicable_page_dimensions` helper consumes the same applicable source
order for page dimensions: when a layer supplies a standard size, it replaces
both previously composed page axes, then explicit width and height from that
same layer override only their respective axes. This closes the backend-neutral per-layer
size/width/height composition rule without resolving physical dimensions or widening
renderer support. A bounded `resolve_applicable_page_format` helper now folds the
same applicable source order once into a resolved explicit-page snapshot: dimensions
use that standard-size-reset/per-axis composition rule, while alignment, columns,
margin, border widths/color, and background use the existing later-non-null field
inheritance. The snapshot carries no selector identity, inferred page parity, eagerly
materialized physical dimensions, or renderer-specific state, so it remains backend-neutral
prerequisite evidence rather than selector-aware output support. Its composed dimensions can
now be passed to the bounded backend-neutral
`IrComposedPageDimensions::resolve_concrete_page_geometry` helper. A complete explicit
width+height pair remains concrete for any output document type. A standard-size base likewise
resolves to closed-domain physical millimeter geometry for every final document type; when
orientation was omitted, the captured call-time document type supplies the pinned preference
(`plain`/`paged`/`docs` portrait, `slides` landscape), after which explicit single-axis
overrides apply independently. The current Typst/PDF global dimension path
reuses this helper. Once any ordered page-format layer exists, that ordered state is
canonical for global dimensions even when it contains no dimension payload; flattened
`page_geometry` / `page_size` are legacy-IR fallbacks only when ordered page-format
state is absent, so effectless or non-dimension ordered layers cannot revive stale
flattened geometry or trigger stale unresolved-size failures. Selector-aware dimension
publication remains out of scope.
Typst page width/height and columns are page-setup parameters rather than contextual
background/foreground content, and changing a page set rule establishes a new conforming
page. Arkst therefore must not approximate selector-scoped size/width/height/columns by
consulting `here().page()` after pagination. Selector-scoped alignment has a separate
unsupported boundary. The current bounded selector-free alignment consumers are split by
the pinned `NodeStyle.TextAlignment` contract: `justify` is local-only and lowers to
Typst paragraph justification, while `start`/`center`/`end` determine compile-time
row/column inherited main-axis alignment and lower to Typst global alignment for every
final document type. Final `slides` composes that horizontal alignment with explicit
`.slides center:{true|false}` vertical `horizon`/`top` alignment in one Typst alignment
value, while nullable slide centering preserves the renderer-owned vertical default.
Scoped alignment remains retained only in ordered page-format state and has no
selector-aware content-alignment consumer. When ordered
page-format state exists, both bounded selector-free consumers resolve the exact global
layer group in source order and treat the merged alignment as canonical; flattened
`page_alignment` is a legacy-IR fallback only when ordered page-format state is absent.
The Typst lowerer therefore detects scoped alignment together with scoped
size/width/height/columns and emits an explicit `panic(...)` before output instead of
silently falling back to global layout state. The pinned Typst subprocess regressions
`integration_selector_scoped_page_layout_fails_closed_at_typst_boundary` and
`integration_selector_scoped_page_alignment_fails_closed_at_typst_boundary` prove
these boundaries fail closed. This is an explicit unsupported-output guard, not
selector-aware layout support. Selector-scoped content margin output remains unsupported. The Typst lowerer
now rejects every selector-scoped `paged` margin before output, including cases where
explicit border widths and color would otherwise make the foreground border path complete.
A scoped margin is part of page content layout, so consuming it only as border inset
geometry would still silently lose setter semantics. The bounded scoped-border path remains
available when margin is inherited from selector-free/global state and the selector scopes
only border/background decoration fields.
A bounded Typst/PDF background path now consumes ordered background layers for
global, side-only, finite-range-only, and combined side+range selectors. It uses
contextual page background content with physical 1-based `here().page()`; the
public page-side contract maps left/verso pages to even physical numbers and
right/recto pages to odd physical numbers. Overlapping selectors preserve
source-order last-wins precedence and a later global background remains the
fallback.
A bounded Typst/PDF scoped border path now consumes ordered margin,
border-width, and border-color layers for global, side-only, finite-range-only,
and combined side+range selectors. It resolves those three fields independently
in source order so a later color-only scoped layer inherits earlier widths and
margins, renders through contextual page foreground content keyed by physical
1-based `here().page()`, and applies the public left/even and right/odd page-side
mapping. A selector-aware border-request flag is resolved with the same page
conditions; when a border is requested on the current physical page but margin,
widths, or color remain unresolved after inheritance, the Typst context now
panics instead of silently returning `none`. Pages with no border request still
emit no foreground, while complete explicit border pages retain the existing
bounded drawing path.
The bounded evaluator now also admits selector-scoped mixed size+axis and
single-axis width/height layers as state-only evidence; those calls return
before the legacy flattened global renderer fields are mutated. Selector-free
mixed calls with a concrete standard-size base remain supported, and the current
Typst/PDF consumer resolves that base physically before applying same-layer
explicit width/height as per-axis overrides. Selector-free single-axis calls are
also admitted when prior ordered global dimension state already supplies the
opposite explicit axis or a concrete standard-size base; this preserves the
upstream later-non-null per-field merge for an earlier explicit width+height
pair instead of requiring every later axis override to originate from a paper
size. An explicit nullable `size:{.none}` contributes no new standard-size
base and therefore follows the same single-axis inheritance rule: it is accepted
only when the prior composition supplies the missing axis. Explicit nullable
`width`/`height` values likewise contribute no axis override, so the previously
composed value for that axis is inherited. A remaining non-null single-axis
override is retained even when no prior ordered layer supplies the opposite
axis, because the pinned setter stores the layer before renderer defaults are
applied. At the final rendering boundary, a final `paged` document prepends
its A4 portrait `defaultPageFormat`, allowing a selector-free width-only or
height-only layer to inherit the missing axis regardless of the document type
that was active when the layer was committed. The default is effective state
only and is not fabricated into the stored `IrPageFormatLayer.size`. Final
`plain`/`docs` have no page-format default to supply a missing opposite axis, but the
pinned public applicability table explicitly permits width while excluding height there. Arkst
therefore lowers a selector-free width-only layer directly as page width for final `plain`/`docs`
without fabricating a height; height-only remains fail-closed. Final `slides` still requires
complete geometry or a resolvable standard-size base. The pinned PageFormatInfo merge contract also makes
`alignment` nullable and applies later-non-null field precedence. Arkst therefore
treats semantic `.none` alignment as no new override: selector-free state keeps
the previously committed document alignment, and exact same-selector resolution
inherits the earlier alignment. The pinned setter also computes orientation only
through `format?.getBounds(orientation)`: if no standard `size` is present,
orientation has no page-format payload. Empty calls, orientation-only calls, and
selector-only calls therefore append effectless layers whose payload fields are all
null. Arkst retains those successful layers in ordered state after validating any
explicit orientation or finite page range, without mutating flattened effective
state. A selector on such an effectless layer is not an unsupported output request,
so the non-paged Typst selector guard ignores it; selector-bearing layers with any
effective payload remain fail-closed as before. This does not widen selector-aware
alignment rendering; non-null scoped alignment remains explicitly fail-closed at the
Typst boundary. Selector-aware output outside the bounded background/border side+range subsets
is deliberately unsupported at the current Typst boundary, with final non-paged selector
state explicitly backend-fail-closed. Selector-scoped size/width/height/columns/margin and
global/local alignment change page setup or content layout before pagination in pinned
v2.5.1; Arkst must not approximate those semantics with post-pagination
`here().page()` inspection. The existing scoped-layout, scoped-alignment, and scoped-margin
panics are therefore the terminal safety boundary for the current Typst backend, not a
pending #175 renderer-widening task. Selector-free explicit-value border requests are
supported for final `plain`, `paged`, and `slides`. Final `plain`/`paged` require
complete margin + widths + color; final `slides` requires complete widths + color and
draws the pinned full-slide frame without using margin. Missing slide width/color defaults
fail closed under a slide-specific unresolved-default guard, while plain/paged retain the
existing margin/width/color completeness guard. Final `docs` remains rejected because the
pinned docs viewport does not apply the content-area border mixin to its main content.
Color-only or otherwise implicit border-width output is also an intentional fail-closed
boundary: pinned v2.5.1 stores no concrete width for a color-only layer and leaves the HTML
renderer/CSS variable at `unset`, so Arkst cannot invent a Typst thickness without
fabricating state. Selector-scoped non-paged border state remains fail-closed under the
existing selector guard. No further bounded `.pageformat` output widening is authorized
under #175 without new backend capability or pinned evidence; complete cross-renderer
v2.5.1 output equivalence therefore remains conservatively `PARTIAL`, and Typst page
objects must not enter evaluator/IR state.

### Caption state

#### `.captionposition` — revalidated existing slice

The pinned signature is
`captionposition(default: CaptionPosition? = null,
figures: CaptionPosition? = null, tables: CaptionPosition? = null,
code: CaptionPosition? = null)`. `code` is the public alias of the
`codeBlocks` source parameter. `CaptionPosition` is a closed `TOP`/`BOTTOM`
domain. The initial effective default is `BOTTOM`; element-specific fields
are nullable inherited overrides.

The regular binder permits positional, named, and positional-then-named
forms. Omission and nullable `.none` preserve existing element-specific
overrides; a supplied default updates only the default, and supplied
figure/table/code values update only their own override. Each call constructs
a partial state, merges it with the current state, returns `VoidValue`, and
emits no document content. Binding is checked before candidate evaluation.
Arkst evaluates all candidates, converts through the existing closed-enum
conversion, uses the post-nested-evaluation state as the successful merge
base, and restores the whole pre-call state if a later conversion or nested
evaluation fails. Callable scopes share the state. Source-defined
`.captionposition` shadows the native dispatch in direct and chained calls.

The evaluator-only state is copied to an immutable `IrCaptionPositionInfo`
snapshot. `#[serde(default)]` preserves old IR without the field and the
closed IR enum preserves the distinction between inherited/null overrides and
explicit values. Typst/HTML do not consume the snapshot, so no caption
placement or rendered-output equivalence is claimed.

The permitted indented body is now retained as source-backed raw text beside
the parsed `CallBody`. Because `codeBlocks` is the final regular parameter,
Quarkdown maps the body to raw `DynamicValue` text, and #166 feeds that text to
the bounded setter without evaluating parsed body nodes as a substitute. This
remains a bounded #149/#148/#154 prerequisite slice, not a claim of complete
caption rendering or target coverage. The existing #145 / PR #146 slice
matches the pinned closed domain, bottom default, merge/preserve, nullable
behavior, candidate-before-commit, rollback, callable sharing,
source-defined precedence, immutable snapshot, serde defaults, and no-output
contract. No regression was found.

Canonical status: `PARTIAL`.

### TeX and pagination primitives

#### `.texmacro`

`texmacro(name: String, macro: String)` takes a required regular name and a
body-compatible raw TeX string. The initial macro map is empty. Each success
adds/replaces the map entry by name, returns no output, and is later consumed
by math typesetting. The body is not a Markdown body and nested-call execution
must not be substituted for the upstream raw-body conversion. Arkst has no
raw body binding, TeX state, math consumer, or renderer-neutral macro model;
status is `PARSED_ONLY`. Its distinct raw-string/document-map/math-renderer
boundary is assigned to #180, with shared binding/conversion prerequisites
remaining in #149/#165–#167 and math/content coordination in #154.

#### `.pagemargin` and `.footer`

`pagemargin(position: PageMarginPosition, content: MarkdownContent)` creates
an invisible page-margin initializer. `PageMarginPosition` is a closed enum
with 16 fixed corner/edge positions plus eight mirrored `inside`/`outside`
positions that resolve differently on left and right pages. The pinned
post-rendering handler processes physical pages in order: every initializer
first becomes active on the physical page containing it, remains active on
later pages, and a later initializer for the same authored position replaces
the earlier one. Same-page initializers are collected before margin content is
applied, so the last same-page initializer wins. `footer(content)` is exact
sugar for `pagemargin(bottomcenter, content)`; it is not separate document
state.

Arkst now has a bounded typed slice. The evaluator converts all 24 public
positions to `IrPageMarginPosition`, preserves the initializer as
`IrNode::PageMarginContent` in source order, evaluates an indented Markdown
block body structurally, preserves source-defined shadowing and serde, and
maps `.footer` exactly to `BottomCenter`. For final `paged` output,
`topcenter` and `bottomcenter` lower to locatable Typst metadata markers.
The page header/footer queries markers whose `location().page()` is less than
or equal to the current physical `here().page()`, then takes the last match.
That preserves the pinned start-page, persistence, and same-page last-wins
contract without reducing repeated content to one document-global static
header/footer value. Real Typst/PDF integration covers both central positions.

This is deliberately not complete page-margin equivalence. The other 22
positions, left/right mirror resolution, and final `plain`/`slides`/`docs`
renderer behavior remain unsupported and fail closed at the current Typst
boundary. The bounded evaluator also accepts the evidenced indented block-body
form only; complete explicit/inline `MarkdownContent` argument adaptation
remains part of the shared content-conversion boundary. Canonical status for
both rows is therefore `PARTIAL` under #176, not
`SUPPORTED_END_TO_END`.

#### `.currentpage`, `.totalpages`, `.formatpagenumber`, and `.resetpagenumber`

`.currentpage()` and `.totalpages()` create one typed `PageCounter` inline node with
closed `CURRENT` / `TOTAL` targets and accept no arguments or body. The pinned HTML
renderer always emits `-` as the initial placeholder. Its page-number handler is attached
to paged and slides document implementations and replaces those placeholders after
pagination; plain keeps the placeholder, and docs inherits the plain document path rather
than a page-number handler.

Arkst implements the bounded counter pair as backend-neutral
`IrInline::PageCounter` + `IrPageCounterTarget`, including no-argument/no-body
binding, source-defined shadowing, source-span preservation, serde, and source-order inline
materialization. Final `paged` and `slides` keep `TOTAL` tied to Typst's final
physical page counter, matching pinned `pages.length`. `CURRENT` now resolves the
bounded formatter/reset marker state described below from the physical page containing the
placeholder; with no applicable marker this is ordinary physical page numbering. Final
`plain` and `docs` preserve the pinned unresolved `-` placeholder instead of
fabricating pagination runtime semantics. These rows remain bounded `PARTIAL`, not a
complete page-numbering equivalence claim.

For `.formatpagenumber(format: String)`, the public `Document.kt` KDoc says
the format accepts the same syntax as `.numbering`, but pinned v2.5.1 HTML
output does not implement that full grammar. `page-numbers.ts` processes all
formatter markers contained in a page before assigning that page's displayed
number; the last marker on the page wins and its value persists to later
pages. Its `formatNumber` helper transforms only the exact strings `1`, `a`,
`A`, `i`, and `I`; any other string is returned literally. The audit therefore
records this documentation/output divergence instead of promoting the broader
`NumberingFormat` grammar to actual page-number renderer behavior.

`.resetpagenumber(start: Int = 1)` likewise creates an ordered initializer
without function-level positivity validation. The pinned HTML page-number
handler processes every reset marker on the containing page before assigning
that page's displayed number and applies a marker only when its parsed value is
finite and greater than zero. Zero or negative values are ignored at render
time rather than rejected by the function; when multiple valid resets occur on
a page, the last valid marker wins. The reset is therefore page-level for
observable HTML numbering rather than an intra-page source-position split.

Arkst now represents both initializers as typed invisible inline markers:
`IrInline::PageNumberFormat` stores the authored format string and
`IrInline::PageNumberReset` stores the signed Kotlin-`Int`-compatible reset value,
including zero and negatives. Binding/default conversion, source-defined shadowing,
source-span preservation, and serde are covered independently. For final `paged` and
`slides`, Typst lowers the markers to locatable `metadata` and every `CURRENT`
placeholder queries all markers whose physical `location().page()` is less than or equal
to its own `here().page()`. Taking the last matching formatter and the last positive reset
therefore reproduces the pinned page-level ordering: a marker later in source on the same
physical page affects counters earlier on that page, the last same-page marker wins, reset
positivity is filtered only at render time, and selected state persists to later pages.
`TOTAL` remains formatter/reset-independent.

The formatter consumer has explicit branches for `1`, `a`, `A`, `i`, and `I`
and returns every other format string literally. Lowercase/uppercase alphabetic output uses
the pinned one-codepoint offset model and Roman output uses Typst's corresponding numbering
systems. This is intentionally classified as bounded `PARTIAL`: exact extreme-value
equivalence with JavaScript `String.fromCharCode` UTF-16 wrapping and the external
`romans` package is not claimed. Final `plain` and `docs` emit neither marker runtime
and continue to expose the pinned unresolved `-` current/total placeholders. Both
`.formatpagenumber` and `.resetpagenumber` therefore move from `PARSED_ONLY` to
bounded `PARTIAL` under #176 without widening the remaining page-margin/footer or
last-heading residuals.

#### `.lastheading`

`lastheading(depth: Int)` is unavailable for `plain` documents and creates a
node that resolves the last heading of the requested depth on the current
page, searching backwards through pages and resetting when a shallower heading
is encountered. Upstream documentation describes heading depth as 1–6, but
the pinned `lastHeading` function performs no range check: it directly creates
`LastHeading(depth)`. The pinned HTML persistent-heading handler indexes its
heading history with `depth - 1` and falls back to empty content when no entry
exists, including out-of-range or non-positive depths. Therefore 1–6 is a
documented/intended heading range, not an upstream call-time validation rule
to reproduce. This behavior is derived from page/heading traversal rather than
a generic mutable document field. Arkst has no page-aware heading history
or node; status is `PARSED_ONLY` under #176.

#### `.autopagebreak` and `.noautopagebreak`

`autopagebreak(maxdepth: Int)` writes a global context option. At the pinned
v2.5.1 commit, `MutableContextOptions.autoPageBreakHeadingMaxDepth` is a
non-null `Int` initialized to `1`; `DocumentType` has no per-document-type
automatic-page-break default field. The pinned `Context.shouldAutoPageBreak`
therefore tests a heading against that one current threshold. Negative values
fail before mutation, zero disables automatic breaks, and
`noautopagebreak()` is exact shorthand for setting zero.

Arkst implements explicit threshold mutation, negative rollback,
source-defined shadowing, serde-compatible IR state, top-level-heading
consumption, weak Typst breaks, and real PDF evidence. However, later v2.6
adaptation changed the implicit default contract: when no explicit override is
stored, current Typst lowering derives `plain=0`, `paged=1`, `slides=2`,
and `docs=0` from the final document type. That behavior is intentionally
covered by the v2.6 compatibility evidence, but it is not pinned-v2.5.1
equivalence because v2.5.1 starts from global threshold `1` for every
document type.

Consequently explicit `.autopagebreak` and `.noautopagebreak` behavior is
implemented and safely consumed, but the rows remain canonical `PARTIAL`
rather than `SUPPORTED_END_TO_END`. The residual is an explicit
version-contract divergence in implicit defaults, not missing setter state or
a reason to reimplement the existing v2.6 path under #175.

### Navigation, outline, and table of contents

#### `.marker`

`marker(name: InlineMarkdownContent)` creates an invisible marker heading
that participates in location/reference and TOC behavior. It has no global
configuration state, but its semantic effect depends on heading traversal and
outline generation. Arkst has no marker node or location hook; status is
`PARSED_ONLY` under #177.

#### `.navigation`

`navigation(role: NavigationContainer.Role? = null,
content: MarkdownContent)` creates a navigable content container. The closed
role domain is `TABLE_OF_CONTENTS` or `PAGE_LIST`; null leaves the role
unspecified. It does not change layout by itself, but themes/renderers may use
it for navigation, styling, behavior, and accessibility. Arkst has no
typed node or output path; status is `PARSED_ONLY` under #177.

#### `.tableofcontents`

`tableofcontents(title: InlineMarkdownContent? = null, maxdepth: Int = 3,
breakpage: Boolean = true, headingdepth: Int? = null,
numberheading: Boolean = false, indexheading: Boolean = false,
focus: InlineMarkdownContent? = null)` creates a heading plus a TOC view.
Null title selects the localized default; blank title suppresses the title.
Depth filters headings. `breakpage` defaults true. Heading depth defaults to 3
for `docs` and 1 otherwise. `numberheading` tracks the heading location and
`indexheading` includes the heading in the TOC; indexing implies location
tracking. `focus` identifies one item by plain text and visually de-emphasizes
the others when a match exists.

This is derived AST/outline state, not a field to add to `DocumentState`.
Arkst has no title/focus/depth binding, TOC node, heading-location hook, or
renderer output. Status is `PARSED_ONLY` under #177.

### Slides document configuration

#### `.slides`

`slides(center: Boolean? = null, controls: Boolean? = null,
speakernotes: Boolean? = null, transition: Transition.Style? = null,
speed: Transition.Speed = DEFAULT)` is available only for `slides` documents.
It creates an invisible global configuration initializer. Null centering,
controls, and speaker-note fields preserve renderer defaults. Transition style
is a closed `NONE`/`FADE`/`SLIDE`/`ZOOM` domain; speed is closed
`DEFAULT`/`FAST`/`SLOW`. A speed matters only when a transition style is
specified, because the upstream node constructs a transition only then.

The configuration is document-wide presentation state, but upstream carries it
as an ordered AST initializer rather than `DocumentInfo` metadata. Arkst now
has a bounded slides state/PDF slice for the currently evidenced center/page
geometry behavior. Controls, speaker notes, transition style/speed, complete
v2.5.1 document-type gating/defaults, and the separate #154
`.fragment`/`.speakernote` content rows remain open. Status is `PARTIAL`
under #178.

## 5. Arkst pipeline and architecture boundary

The unresolved #153 rows no longer share one uniform pipeline. The 7
`PARSED_ONLY` rows still follow the unresolved-call path:

```text
source call with source span
  -> Markdown/Quarkdown frontend call representation
  -> IrNode::FunctionCall or IrInline::DirectiveCall
  -> evaluator lookup finds no completed native semantic owner
  -> unresolved call is structurally preserved
  -> no row-specific typed state/node
  -> no rendered output equivalence claim
```

The 13 `PARTIAL` rows instead have bounded typed semantics and/or output
evidence and must be judged individually against their recorded residual
contract. In particular, #175-owned numbering/font/paragraph/page-format and
automatic-page-break state plus the #176 bounded current/total counter and
page-number formatter/reset marker slices must not be described as absent merely
because complete pinned-v2.5.1 end-to-end equivalence is not claimed.

The existing evaluator explicitly preserves unresolved block and inline calls
with their arguments/body and spans. This is useful compatibility evidence for
`PARSED_ONLY`, not semantic support. Existing typed component paths are
separate: `IrComponent::Stacked`, `IrComponent::Container`, and
`IrComponent::Landscape` are already lowered by Typst and tested, but they do
not establish document-wide `.pageformat`, `.font`, `.paragraphstyle`, or
`.slides` state.

`DocumentState` remains evaluator-owned and shared by callable child contexts.
In addition to the #152 metadata families and bounded caption state, current
bounded implementations now carry numbering mutation state, ordered font
layers, paragraph style, automatic-page-break depth, ordered page-format
selectors/layers plus compatibility fields, and bounded slides configuration.
Successful evaluation snapshots those backend-neutral values into
`IrDocumentState`.

`IrDocumentState` remains immutable, backend-neutral, and serde-serializable.
The numbering representation stores parsed tokens, source-order mutation
intent, duplicate `extra` entries, and call-time document type without
renderer objects. Font-family/resource identity beyond the bounded size layer,
locale/renderer paragraph defaults, unsupported page-format renderer cases,
TeX macros, page counters, navigation/TOC derived state, and slide transition
domains remain outside the implemented boundary or fail closed as recorded by
their canonical rows.

Typst lowering consumes normalized IR/state, not unresolved evaluator calls.
No #153-owned row currently carries a `SUPPORTED_END_TO_END` v2.5.1 claim. Bounded output
evidence does exist for current page geometry/alignment, automatic page breaks,
and slides/PDF behavior; numbering and caption position intentionally have no
numbering/caption renderer consumer in this slice.

### Binding, conversion, atomicity, scope, and precedence

- Regular parameters are positionally and by name bindable according to the
  shared binder. `@LikelyNamed`/`@LikelyBody` do not establish named-only or
  body-only rules. Final-parameter body fallback is an upstream raw
  `DynamicValue` contract; #166 retains that lossless source text beside the
  parsed body for the bounded affected state setters.
- Closed domains identified in the sweep include page side, page orientation,
  page size format, page-margin position, caption position, numbering symbols,
  navigation role, slide transition style/speed, text alignment, and size/unit
  domains. Future implementation must use typed conversion rather than
  arbitrary strings.
- Stateful setters must validate/bind/convert all candidates before one
  commit. Nested evaluation that successfully mutates shared document state
  must use the post-evaluation state as its successful baseline; rollback must
  restore the whole pre-call state on later failure. This is evidenced for
  `.captionposition` and the bounded numbering mutation state and remains a
  requirement for later state work.
- Source-defined shadowing remains the established local native-dispatch rule.
  Native ownership is added only for bounded `PARTIAL` rows that have an actual
  evaluator implementation; unresolved rows remain structurally preserved.
- None/omission semantics are field-specific: they can preserve inherited
  values (`captionposition`, `paragraphstyle`, `pageformat` layers), select
  renderer defaults (`font`, slide fields), disable a numbering key (`none`),
  or be invalid for required parameters. They must not be collapsed into one
  generic reset convention.

### Serde and WASM implications

New bounded state remains serde-compatible and WASM-safe: numbering state is
backend-neutral, defaults when absent in old IR, and contains no filesystem,
process, network, JVM, media-store, or renderer handles. Existing caption,
page-geometry/alignment, automatic-page-break, and slides state follow the same
boundary. Future font/resource and remaining layout representation must keep
those host/backend concerns downstream.

## 6. Cross-audit reconciliation

- `.localization` and `.localize` remain canonical #151-owned surfaces. They
  are not in the #153-owned count and are not reclassified here.
- The eight #152 rows (`doctype`, `docname`, `docdescription`, `docauthor`,
  `docauthors`, `dockeywords`, `doclang`, `theme`) remain #152-owned. Their
  interactions with document type/defaults are described only where necessary
  to explain #153 defaults.
- `.doclang` continues to use the public parameter `locale`, not `language`.
- The #151 correction that stdlib registration loads
  `/lib/localization.qd` and seeds the `std` table remains represented in the
  #151 manifest and #152 audit handoff.
- #173 remains `.doclang` locale closure only.
- #154 rows are explicit `NOT_APPLICABLE` handoffs in the manifest, not
  canonical #153 statuses. #155 rows are not reopened or classified here; the
  existing #152 manifest remains their handoff record.
- #156 remains the cross-audit reconciliation, conformance, documentation,
  backlog, and dependency-order gate. This audit does not reconcile the final
  global matrix early.

## 7. Bounded follow-up backlog

Existing issues and evidence reused:

| Issue/PR | Reused boundary |
|---|---|
| #145 / PR #146 | Existing `.captionposition` evaluator/IR slice; no reimplementation |
| #149; #165–#167 | Shared binder, value conversion, diagnostics, provenance, and atomicity ownership |
| #150; #169 | Callable scope, lazy body, precedence, and programmable evaluation ownership |
| #152 / PR #174 | Metadata/document-state ownership and existing state base |
| #154 | Adjacent content/component/output ownership handoffs only |
| #156 | Required final reconciliation and dependency-aware implementation order |
| #158, #160 | Raw/structured content prerequisites relevant to body fallback |

The cohesive implementation follow-ups below remain authoritative; several
bounded slices have since started or landed and retain their residual scope:

| Issue | Exact scope | Owner/layer | Prerequisites and order |
|---|---|---|---|
| [#175](https://github.com/luceat-lux-vestra/arkst/issues/175) | `.numbering`, `.nonumbering`, `.font`, `.paragraphstyle`, `.pageformat`, `.autopagebreak`, `.noautopagebreak`; exact all-input-key `numbering.extra` storage plus border-side zeroing and color-only width inheritance | Engine + IR state; later Typst/output | #149/#165–#167; representation and renderer review; after #156 |
| [#176](https://github.com/luceat-lux-vestra/arkst/issues/176) | `.pagemargin`, `.footer`, `.currentpage`, `.totalpages`, `.formatpagenumber`, `.resetpagenumber`, `.lastheading`; page-level formatter/reset precedence, renderer-time reset filtering, and documented-vs-runtime heading depth | Engine/IR nodes + Typst/output | #149; raw/content boundary; after #156 |
| [#177](https://github.com/luceat-lux-vestra/arkst/issues/177) | `.marker`, `.navigation`, `.tableofcontents` | Engine/IR outline nodes + Typst/HTML output | heading/location/content evidence; #154 coordination; after #156 |
| [#178](https://github.com/luceat-lux-vestra/arkst/issues/178) | `.slides` global configuration and closed transition domains | Engine/IR only if needed + slide backend | `doctype`/#152 interaction and #154 slide content; after #156 |
| [#180](https://github.com/luceat-lux-vestra/arkst/issues/180) | `.texmacro` raw TeX body, document macro map, source-order replacement, and math-output consumption | Engine/IR only as backend-neutral state + math backend | #149/#166–#167; #154 math/content coordination; after #156 |

The `.captionposition` raw-body work is implemented in the bounded slice by
#166, while caption output remains separate. No issue is one-function-per-row
by default; #180 is split because raw
TeX body conversion, macro-map state, and math-renderer consumption form a
distinct semantic contract from #175 layout state. All implementation ordering
follows the dependency-aware order in [#156 reconciliation](RECONCILIATION.md).

## 8. Audit conclusion

The canonical #153 result remains a 20-row owned inventory. Current status is
15 conservative `PARTIAL` rows (`captionposition`,
`numbering`/`nonumbering`, bounded size-only `font`, `paragraphstyle`,
bounded `pageformat`, bounded `pagemargin`/`footer`, bounded
`currentpage`/`totalpages`, bounded `formatpagenumber`/`resetpagenumber`,
`autopagebreak`/`noautopagebreak`, and bounded `slides`) plus 5
`PARSED_ONLY` rows. This does not establish complete v2.5.1 output equivalence
or justify a generalized document-wide style system. Residual ownership remains
#175–#178 and the applicable #154 content/output consumers.
