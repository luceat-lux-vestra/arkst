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
returns no output. The stored list starts empty; the effective initial format
is supplied by the document type (A4 portrait for paged, none for plain/docs,
and slide-specific behavior). A null side/range is global; `side` and a
finite, 1-based inclusive `pages` range select subsets of paged pages. An open
range end fails before mutation.

When `size` is present, its standard closed `PageSizeFormat` bounds are
rotated to the selected orientation. Explicit width/height override those
bounds. Later layers with the same selector override only their non-null
fields; omitted fields inherit through the selector group. Positive
`columns` is document-wide multi-column configuration; values below one are
discarded. The public border-side arguments have a cross-field exception to
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
selector-free page-format layer snapshot that preserves successful bounded
mutations in source order without changing current renderer consumers. Geometry/alignment,
selector-free global margins, global positive columns, selector-free global
background, and the bounded standard-size selection have current Typst/PDF
consumers, while row/column alignment inheritance remains unchanged;
page-border decoration state now has one bounded Typst/PDF consumer when the
final document is `paged` and explicit margin, committed border widths, and
explicit border color are all present; implicit-margin, width-only,
color-only, slides/plain/docs, and selector-aware border output remain
fail-closed. The size slice
preserves the closed standard-format domain, named size binding, and an
explicit portrait/landscape orientation when supplied; when orientation is
omitted it records the document type in effect at commit time as the downstream
preferred-orientation basis. The selected Typst/PDF backend applies standard size only when the final output
document type supports it (`paged` or `slides`), resolves the closed format to
explicit physical millimeter bounds including B0, and rotates those bounds for
the effective orientation. When orientation was omitted, the call-time
document-type snapshot remains the default-orientation basis instead of a later
`.doctype` mutation. An omitted-orientation layer captured under `docs` remains
fail-closed because the pinned public contract does not define that cross-doctype
default. Explicit complete page geometry keeps its existing output precedence
while cross-layer size/width/height composition remains unresolved until
selector/layer ordering is represented. The margin slice expands the documented
`Sizes` shorthand into explicit top/right/bottom/left state: one value applies
to every side, two values map vertical/horizontal, and four values map TRBL.
Three-value or otherwise malformed groups fail closed, and semantic `None`
preserves the previously committed effective global margin. The current Typst
consumer lowers those four explicit sides through the shared size conversion
boundary, with real pinned-backend PDF integration coverage. Non-positive or
semantic-None column inputs are discarded without replacing an earlier
positive global value. The current Typst consumer maps the committed positive
column count directly to the page column configuration, with real
pinned-backend PDF integration coverage. The decoration slice preserves the pinned cross-field
border rule: once any non-null border side is supplied, omitted/null sides
become explicit zero; color-only input updates border color without fabricating
a border width and therefore preserves any previously committed width
structure. Semantic-None border/color/background inputs preserve prior
effective state. The current Typst/PDF background consumer maps the committed
typed RGB/alpha color directly to page fill without changing border state.
The bounded Typst/PDF border consumer uses page foreground coordinates to draw
the explicit content-area rectangle only when all of margin, border widths, and
border color are committed. It preserves four independent side widths and does
not fabricate a renderer-default margin, width, or color.
The ordered layer snapshot is a prerequisite only: current flattened fields
remain the bounded renderer compatibility surface and no new output claim is
made by recording layer order. Selector/range, width/height override
composition, selector-aware geometry/size/margin/decoration/column layering,
the remaining page-border output outside the explicit-margin/width/color paged
subset, and the remaining output consumption stay open, and Typst page objects
must not enter evaluator/IR state. Status is conservatively `PARTIAL`
under #175.

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
with fixed corner/edge positions plus mirrored `inside`/`outside` positions
that resolve differently on left and right pages. The content is lazy body
content and the initializer displays it on every page, with distinct
plain/paged/slides behavior. `footer(content)` is exact sugar for
`pagemargin(bottomcenter, content)`; it is not a separate state field.

These are AST/output primitives, not `DocumentInfo` fields. They require typed
body/content retention, closed position conversion, repeated-page semantics,
and renderer support. Arkst currently preserves unresolved calls only;
status for both is `PARSED_ONLY` and the grouped pagination follow-up is #176.

#### `.currentpage`, `.totalpages`, `.formatpagenumber`, and `.resetpagenumber`

`.currentpage()` and `.totalpages()` create typed page-counter nodes. Plain
documents display `-` at rendering time because they do not support page
counting.

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

All four return nodes/no direct output at evaluation time. They need
backend-neutral typed nodes or an equivalent event representation plus
backend-specific conformance that preserves these page-level precedence and
renderer rules. Arkst has no such representation or lowering; all four are
`PARSED_ONLY` under #176.

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

`autopagebreak(maxdepth: Int)` writes a global context option. The pinned
option field starts at `1`; effective behavior is document-type/renderer
dependent. A heading at depth less than or equal to the threshold can force a
break. Negative values fail before mutation, while zero disables automatic
breaks. `noautopagebreak()` is the zero-threshold shorthand. These are
document/pipeline configuration, not component-local layout. Arkst now has a
typed document-state threshold and bounded heading/page-break consumption in
the current Typst path, including zero-disable behavior. Complete pinned-v2.5.1
reconciliation across document types and all error/default cases remains open;
both rows are `PARTIAL` under #175.

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

The current path for the 13 still-unresolved rows is:

```text
source call with source span
  -> Markdown/Quarkdown frontend call representation
  -> IrNode::FunctionCall or IrInline::DirectiveCall
  -> evaluator lookup finds no #153 native owner
  -> unresolved call is structurally preserved
  -> no typed binding/conversion or DocumentState mutation
  -> no #153-specific IR snapshot
  -> Typst lowering sees no supported semantic node/state
  -> no rendered output equivalence claim
```

The existing evaluator explicitly preserves unresolved block and inline calls
with their arguments/body and spans. This is useful compatibility evidence for
`PARSED_ONLY`, not semantic support. Existing typed component paths are
separate: `IrComponent::Stacked`, `IrComponent::Container`, and
`IrComponent::Landscape` are already lowered by Typst and tested, but they do
not establish document-wide `.pageformat`, `.font`, `.paragraphstyle`, or
`.slides` state.

`DocumentState` remains evaluator-owned and shared by callable child contexts.
In addition to the #152 metadata families and bounded caption state, current
bounded implementations now carry numbering mutation state,
automatic-page-break depth, page alignment/geometry, and bounded slides
configuration. Successful evaluation snapshots those backend-neutral values
into `IrDocumentState`.

`IrDocumentState` remains immutable, backend-neutral, and serde-serializable.
The numbering representation stores parsed tokens, source-order mutation
intent, duplicate `extra` entries, and call-time document type without
renderer objects. Font layers, paragraph style, full page-format
selectors/layers, TeX macros, page counters, navigation/TOC derived state, and
slide transition domains remain absent until their separately owned contracts
are implemented.

Typst lowering consumes normalized IR/state, not unresolved evaluator calls.
No #153-owned row has complete v2.5.1 output equivalence. Bounded output
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
nine conservative `PARTIAL` rows (`captionposition`,
`numbering`/`nonumbering`, bounded size-only `font`, `paragraphstyle`,
bounded `pageformat`, `autopagebreak`/`noautopagebreak`, and bounded
`slides`) plus 11 `PARSED_ONLY` rows. This does not establish complete v2.5.1 output equivalence
or justify a generalized document-wide style system. Residual ownership remains
#175–#178 and the applicable #154 content/output consumers.
