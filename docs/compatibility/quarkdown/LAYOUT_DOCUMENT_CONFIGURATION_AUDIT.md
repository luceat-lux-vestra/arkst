# Quarkdown v2.5.1 layout, pagination, style, and document-configuration audit

Status: complete strict audit artifact for Issue [#153](https://github.com/luceat-lux-vestra/scribium/issues/153). This is evidence and backlog work only. It does not implement a layout/configuration surface or promote the verified compatibility baseline.

## 1. Audit identity and evidence policy

| Item | Pinned value |
|---|---|
| Quarkdown target | v2.5.1 at [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e) |
| Scribium audit base | [`4a9112a9ee840374350dd9a90b65f58cce96eb08`](https://github.com/luceat-lux-vestra/scribium/tree/4a9112a9ee840374350dd9a90b65f58cce96eb08), the squash merge of reviewed PR #174 / completed Issue #152 |
| Parent tracker | [#147](https://github.com/luceat-lux-vestra/scribium/issues/147) |
| Canonical manifest | [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv) |
| Offline guard | [`layout_document_configuration_audit.rs`](../../../crates/scribium-core/tests/layout_document_configuration_audit.rs) |

The audit uses the repository's clean-room policy: public upstream source
declarations and models at the exact pinned commit, public documentation,
public tests as behavioral evidence, independent Scribium tests/fixtures, and
current Scribium source/tests. Upstream source, tests, and fixtures were not
copied or translated into Scribium. The checked-out upstream tree was detached
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
Scribium evidence, status, gap, and follow-up are in the manifest. The most
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
| `PARSED_ONLY` | 19 |
| `PARTIAL` | 1 |
| `UNSUPPORTED` | 0 |
| `DEFERRED` | 0 |
| `BLOCKED` | 0 |
| `NOT_APPLICABLE` | 0 |
| `UNKNOWN` | 0 |

`PARSED_ONLY` is used deliberately. Scribium's generic frontend/IR path
recognizes and source-preservingly retains unresolved calls, but no semantic
implementation exists for those 19 names. A preserved `IrNode::FunctionCall`
or inline directive is not a successful setter, typed node, state mutation,
or renderer claim. `.captionposition` is `PARTIAL` because its evaluator and
immutable IR semantics are implemented and independently evidenced, while
caption output remains outside the current boundary. #166 now covers the
source-backed raw-body fallback for the bounded `.captionposition` setter.

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
finish before `DocumentInfo.numbering` is replaced. Heading, figure, table,
math, code, footnote, and custom-numbered output consumers are separate
renderer/AST boundaries. Scribium has no binder, typed numbering model in
`IrDocumentState`, or numbering-aware backend path; status is `PARSED_ONLY`.

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
filesystem assumptions in WASM-capable core crates. Scribium currently has no
font state, font resource model, or backend lowering; status is `PARSED_ONLY`.

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
inline `.text` styling and from component-local spacing. Scribium has no
paragraph document state, locale-aware paragraph renderer, or IR/backend
consumer; status is `PARSED_ONLY`.

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

The state is genuinely document-scoped and would require backend-neutral
`PageFormatInfo`, selector, size, color, closed enum, and merge representation;
it must not contain Typst page objects. Scribium only has the existing
component-oriented `IrSize` conversion and no page-format state, layer merge,
or page backend; status is `PARSED_ONLY`.

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
Scribium evaluates all candidates, converts through the existing closed-enum
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
must not be substituted for the upstream raw-body conversion. Scribium has no
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
and renderer support. Scribium currently preserves unresolved calls only;
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
renderer rules. Scribium has no such representation or lowering; all four are
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
a generic mutable document field. Scribium has no page-aware heading history
or node; status is `PARSED_ONLY` under #176.

#### `.autopagebreak` and `.noautopagebreak`

`autopagebreak(maxdepth: Int)` writes a global context option. The pinned
option field starts at `1`; effective behavior is document-type/renderer
dependent. A heading at depth less than or equal to the threshold can force a
break. Negative values fail before mutation, while zero disables automatic
breaks. `noautopagebreak()` is the zero-threshold shorthand. These are
document/pipeline configuration, not component-local layout. Scribium has no
option mutation, heading interaction, or renderer page-break consumption;
status is `PARSED_ONLY` under #175.

### Navigation, outline, and table of contents

#### `.marker`

`marker(name: InlineMarkdownContent)` creates an invisible marker heading
that participates in location/reference and TOC behavior. It has no global
configuration state, but its semantic effect depends on heading traversal and
outline generation. Scribium has no marker node or location hook; status is
`PARSED_ONLY` under #177.

#### `.navigation`

`navigation(role: NavigationContainer.Role? = null,
content: MarkdownContent)` creates a navigable content container. The closed
role domain is `TABLE_OF_CONTENTS` or `PAGE_LIST`; null leaves the role
unspecified. It does not change layout by itself, but themes/renderers may use
it for navigation, styling, behavior, and accessibility. Scribium has no
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
Scribium has no title/focus/depth binding, TOC node, heading-location hook, or
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
as an ordered AST initializer rather than `DocumentInfo` metadata. Scribium
has no document-type gate, typed transition domains, initializer IR, or slide
backend. `.fragment` and `.speakernote` remain separate #154 content rows.
Status is `PARSED_ONLY` under #178.

## 5. Scribium pipeline and architecture boundary

The current path for the 19 unresolved rows is:

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

`DocumentState` at this base is evaluator-only and contains exactly the
metadata/state families already owned by #152 plus the bounded caption state:
name, description, document type, ordered authors, keywords, optional theme,
optional locale, and `IrCaptionPositionInfo`. Callable child contexts share a
reference-counted state handle; successful evaluation snapshots it into
`IrDocumentState`. No new field is added by this audit.

`IrDocumentState` is immutable, backend-neutral, serde-serializable data. Its
caption fields use a closed enum and nullable per-kind overrides. It currently
does not represent numbering, font layers, paragraph style, page-format
selectors, TeX macros, page counters, navigation/TOC derived state, automatic
page-break options, or slide transitions. None of those should be added merely
because the audit found a gap; each requires a separate semantic contract and
architecture review after #156.

Typst lowering consumes normalized IR nodes/components, not evaluator calls,
binding rules, runtime state, or unresolved calls. Existing Typst tests prove
component behavior only. No #153-owned row has current Typst/PDF/HTML output
equivalence, and `.captionposition` intentionally has no renderer consumer.

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
  restore the whole pre-call state on later failure. This is already evidenced
  for `.captionposition` and is a requirement for future state work, not a new
  production change here.
- Source-defined shadowing remains the established local native-dispatch rule.
  The audit does not add native names for #153 rows, so no precedence change is
  made.
- None/omission semantics are field-specific: they can preserve inherited
  values (`captionposition`, `paragraphstyle`, `pageformat` layers), select
  renderer defaults (`font`, slide fields), disable a numbering key (`none`),
  or be invalid for required parameters. They must not be collapsed into one
  generic reset convention.

### Serde and WASM implications

No serde format, IR field, production state structure, dependency, or
WASM-sensitive code changes in this audit. Existing caption serde tests cover
deterministic round trips and old IR defaulting. Future representation work
must add `#[serde(default)]` for new optional state without invalidating old
serialized documents, keep closed domains typed, and avoid font/resource/JVM,
filesystem, process, or network dependencies in core/IR/evaluator code. Page
geometry, typography, and slide configuration can be represented as
backend-neutral data only after their upstream inheritance and output
contracts are fixed; renderer handles and media storage must remain downstream
or host-owned.

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

New cohesive implementation follow-ups were created, but none was started:

| Issue | Exact scope | Owner/layer | Prerequisites and order |
|---|---|---|---|
| [#175](https://github.com/luceat-lux-vestra/scribium/issues/175) | `.numbering`, `.nonumbering`, `.font`, `.paragraphstyle`, `.pageformat`, `.autopagebreak`, `.noautopagebreak`; exact all-input-key `numbering.extra` storage plus border-side zeroing and color-only width inheritance | Engine + IR state; later Typst/output | #149/#165–#167; representation and renderer review; after #156 |
| [#176](https://github.com/luceat-lux-vestra/scribium/issues/176) | `.pagemargin`, `.footer`, `.currentpage`, `.totalpages`, `.formatpagenumber`, `.resetpagenumber`, `.lastheading`; page-level formatter/reset precedence, renderer-time reset filtering, and documented-vs-runtime heading depth | Engine/IR nodes + Typst/output | #149; raw/content boundary; after #156 |
| [#177](https://github.com/luceat-lux-vestra/scribium/issues/177) | `.marker`, `.navigation`, `.tableofcontents` | Engine/IR outline nodes + Typst/HTML output | heading/location/content evidence; #154 coordination; after #156 |
| [#178](https://github.com/luceat-lux-vestra/scribium/issues/178) | `.slides` global configuration and closed transition domains | Engine/IR only if needed + slide backend | `doctype`/#152 interaction and #154 slide content; after #156 |
| [#180](https://github.com/luceat-lux-vestra/scribium/issues/180) | `.texmacro` raw TeX body, document macro map, source-order replacement, and math-output consumption | Engine/IR only as backend-neutral state + math backend | #149/#166–#167; #154 math/content coordination; after #156 |

The `.captionposition` raw-body work is implemented in the bounded slice by
#166, while caption output remains separate. No issue is one-function-per-row
by default; #180 is split because raw
TeX body conversion, macro-map state, and math-renderer consumption form a
distinct semantic contract from #175 layout state. All implementation ordering
follows the dependency-aware order in [#156 reconciliation](RECONCILIATION.md).

## 8. Audit conclusion

The canonical #153 result is a 20-row owned inventory with one bounded
semantic/IR slice (`captionposition`) and 19 parser/structural-retention-only
rows. The audit found no evidence supporting a document-wide generalized style
system, no need to broaden `DocumentState` during this work, and no current
output-equivalence claim for any #153-owned surface. Production semantic/state
changes: **none**.
