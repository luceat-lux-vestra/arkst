# Issue #154 — Content, media, and Markdown-extension audit

## Audit identity

- **Arkst audit base:** '7144683346fd6e39c49ef0923733c856a6a55f42'
- **Quarkdown target:** v2.5.1 at '107ec3a9482f10d6f90d7580f8409b46a719d18e'
- **Scope:** Quarkdown document-content, media, presentation-component, raw-content, reference, and Quarkdown-specific Markdown-extension primitives
- **Authority:** the machine-checkable [83-row manifest](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv)
- **Parent tracker:** [#147](https://github.com/luceat-lux-vestra/scribium/issues/147)
- **Audit issue:** [#154](https://github.com/luceat-lux-vestra/scribium/issues/154)

This is an evidence and backlog-reconciliation record. It does not implement
newly discovered Quarkdown semantics. Production semantic, state, parser,
resource, IR, and backend changes are **none**; post-audit implementation
ordering is defined by [#156](https://github.com/luceat-lux-vestra/scribium/issues/156)
and its dependency graph.

The manifest is authoritative for one canonical #147 status per enumerated
surface. It contains 71 #154-owned rows and 12 explicit handoff rows. The
owned status counts are:

| Status | Rows |
|---|---:|
| 'SUPPORTED_END_TO_END' | 13 |
| 'SUPPORTED_SEMANTICS' | 3 |
| 'PARSED_ONLY' | 1 |
| 'PARTIAL' | 13 |
| 'UNSUPPORTED' | 37 |
| 'DEFERRED' | 2 |
| 'BLOCKED' | 1 |
| 'UNKNOWN' | 1 |
| **Owned total** | **71** |
| 'NOT_APPLICABLE' handoffs | 12 |
| **Manifest total** | **83** |

The 'NOT_APPLICABLE' rows are not hidden support claims. They identify
adjacent public surfaces whose producer or prerequisite is owned by #153,
#155, #149, #158, #160, or #166. The #160 frontend/content slice is now
implemented and independently evidenced; this audit still records it only as
an ownership handoff and makes no producer, evaluator, IR, or output claim.
The guard requires those ownership edges to remain explicit.

## Enumeration method

The pinned public sweep covered the complete applicable surface in the
following source families:

- Markdown/CommonMark/GFM blocks, inlines, tables, code fences, links,
  images, footnotes, raw HTML, and metadata/attribute boundaries;
- Quarkdown lexer/parser extensions for sized images, inline/display math,
  explicit page breaks, custom IDs, captions, compact footnotes,
  cross-references, and callout/box forms;
- Primitives.kt: .heading, .paragraph, .link, .image, .pagebreak, .code,
  .math, and .figure;
- Layout.kt: .container, .align, .center, .float, .row, .column, .grid,
  .landscape, .fullspan, .whitespace, .clip, .box, .todo, .collapse,
  .textcollapse, .numbered, and .table;
- TableComputation.kt: .tablesort, .tablefilter, .tablecompute,
  .tablecolumn, .tablecolumns, and .tablebyrows;
- Text.kt, Markdown.kt, Html.kt, and Reference.kt: text, breaks, code spans,
  text matching, packaged text, raw Markdown, HTML options, HTML/CSS, llms.txt,
  and references;
- Slides.kt, Icon.kt, Emoji.kt, Mermaid.kt, and MiscElements.kt: fragments,
  speaker notes, icons, emoji, diagrams, charts, subdocument graphs, and
  keybindings; and
- resource/data consumers and global-state producers that are relevant
  dependencies but remain explicitly handed off.

The sweep uses immutable v2.5.1 source links containing the target SHA. Moving
main, latest documentation, and unpinned tag contents are not canonical
evidence for these rows. Public source was used only to independently
describe observable contracts; no upstream implementation, test, or fixture
was copied or translated.

## Markdown baseline versus Quarkdown semantics

The audit keeps six layers distinct:

1. ordinary Markdown/CommonMark/GFM syntax;
2. Rushdown parser retention and source provenance;
3. Quarkdown-specific syntax extensions;
4. Quarkdown semantic/evaluator behavior, including callable body and option
   conversion;
5. Arkst's backend-neutral AST/IR representation; and
6. Typst lowering and rendered-output fidelity.

A Markdown image is not a Quarkdown-sized image. A Markdown fence is not
.code with an evaluable body, caption, focus, line numbers, and reference.
An ordinary Markdown table is not a generated, computed, captioned, numbered,
or cross-referenceable Quarkdown table. Raw HTML preservation is not a
generic backend escape hatch. These distinctions are recorded separately in
the manifest fields rather than collapsed into one parser status.

The five ordinary Markdown block/inline/table/code baseline rows are
SUPPORTED_END_TO_END only for their bounded existing contract. Markdown
images and raw HTML are PARTIAL because their resource and target policy
is bounded. Markdown footnotes and generic attributes have no current
lossless semantic model. The Quarkdown-specific delimiter rows remain
UNSUPPORTED, PARTIAL, or PARSED_ONLY according to the actual current
layer; no parser-only observation is promoted.

## Content and component inventory

The public content producers are separated by their representation and
dependency:

- Headings, paragraphs, links, text, code spans, breaks, and callable code
  are distinct from their ordinary Markdown counterparts.
- Images, figures, Mermaid, charts, icons, and packaged emoji are media or
  content producers. Their resource, media-type, missing-resource, and
  renderer contracts are not inferred from an existing IrInline::Image.
- Tables are split into ordinary Markdown tables, callable .table,
  generated .tablebyrows, and the six table traversal/computation
  functions. They do not share a single unverified “table supported” claim.
- .row, .column, .grid, .align, .center, .landscape, and .whitespace are
  local component consumers. The bounded row/column/grid/center/alignment/
  landscape/whitespace rows have semantic and output evidence; the remaining
  component functions do not.
- .container is PARTIAL: bounded width/height/fullwidth behavior exists,
  while float, full-span, class, style, and complete body conversion remain
  open.
- .box, .todo, .collapse, .textcollapse, .float, .clip, and .numbered remain
  explicit gaps. A blockquote or generic function-call node is not their
  semantic implementation.

No generic style framework, layout framework, or new semantic layer is
introduced by this inventory.

The canonical implementation ownership is intentionally split at the
producer and shared-index boundaries:

- [#181](https://github.com/luceat-lux-vestra/scribium/issues/181) owns
  structural content plus shared caption, identifier, reference, and index
  infrastructure. It is not the producer owner for `.code`, `.math`, or the
  generated/computed table family.
- [#182](https://github.com/luceat-lux-vestra/scribium/issues/182) owns image,
  media, named icon/emoji catalog (`.icon`, `.emoji`, `.allemojis`), diagram
  (`.mermaid`), and chart (`.xychart`) producers; [#155](https://github.com/luceat-lux-vestra/scribium/issues/155)
  remains the resource/environment owner.
- [#183](https://github.com/luceat-lux-vestra/scribium/issues/183) owns table
  producers and table computation. It consumes #181 shared caption,
  identifier, reference, and index infrastructure.
- [#184](https://github.com/luceat-lux-vestra/scribium/issues/184) owns the
  component-local producer family `.text`, `.box`, `.todo`, `.collapse`,
  `.textcollapse`, `.clip`, `.float`, `.fullspan`, `.fragment`, and
  `.speakernote`, plus the bounded `.keybinding` and `.loremipsum` content
  producer review. The remaining component-local `.container` subcontracts
  (`float`, `fullspan`, `classname`, `style`, and complete body conversion)
  are linked there without transferring document-wide state ownership.
- [#185](https://github.com/luceat-lux-vestra/scribium/issues/185) owns inline
  and display math, `.math`, code presentation/captions, `.codespan`, and
  explicit `.pagebreak`/`<<<` breaks. It consumes #181 shared caption,
  identifier, reference, and index infrastructure; `.texmacro` remains #180.

The bounded `.align`, `.center`, `.row`, `.column`, `.grid`, `.landscape`,
`.whitespace`, and `.br` rows have no remaining #154 implementation gap in
the evidenced contract. They are not assigned to [#175](https://github.com/luceat-lux-vestra/scribium/issues/175):
#175 remains document-wide only, including `.pageformat.columns` and global
automatic page-break/numbering policy.

## Captions, numbering, labels, and references

The audit reconstructs the producer/consumer edges without combining their
state models:

- .captionposition is #153-owned global state. Figure, table, code, and
  related caption-producing content is #154-owned. The manifest records the
  handoff and each producer separately.
- #181 owns the shared caption/identifier/reference/index infrastructure;
  figure and structural content producers use it, #183 owns table producers,
  and #185 owns code/math producers. This is a dependency edge, not a second
  producer implementation in #181.
- .numbering and .nonumbering are #153-owned policy. Headings, figures,
  tables, equations, code, footnotes, and .numbered are #154 consumers whose
  derived traversal/index behavior is not yet implemented.
- Custom IDs and .ref require a producer/consumer index, uniqueness and
  unresolved-reference rules, generated reference text, and output anchors.
  Existing Markdown links do not supply that contract.
- Table-caption adjacency, code-caption syntax, math identifiers, and compact
  footnotes are separately inventoried. None is promoted merely because
  caption text or a source string can be retained.
- Page/location behavior is split from local content. Explicit .pagebreak is
  #154; automatic page-break policy and page configuration remain #153.

The relevant bounded follow-up groups are [#181](https://github.com/luceat-lux-vestra/scribium/issues/181)
for structural content/captions/identifiers/references and
[#185](https://github.com/luceat-lux-vestra/scribium/issues/185) for math,
code presentation, and explicit breaks. [#145](https://github.com/luceat-lux-vestra/scribium/issues/145)
and its completed correction remain the source of the global caption-position
boundary; they are not reopened by this audit.

## Resource-backed primitives

Images, media, icons, diagrams, packaged text, and content-loading consumers
were checked for:

- local/project-resource versus URL/network behavior;
- logical project paths and host filesystem boundaries;
- media type and renderer dependencies;
- missing and invalid resource behavior;
- frontend, evaluator, project, and host ownership; and
- WASM/platform-neutral constraints.

The current Arkst project abstraction can represent bounded logical
project references, and existing Markdown image/read/json/include slices use
that boundary. It does not establish Quarkdown media storage, URL/network
loading, icon catalogs, Mermaid/chart renderers, packaged-resource parity, or
subdocument graphs. Those dependencies are assigned to [#155](https://github.com/luceat-lux-vestra/scribium/issues/155)
where they are resource/environment concerns. No #154 row adds filesystem,
network, process, or resource access to a platform-neutral crate.

.read, .json, .csv, .include, .includeall, and .subdocument are explicit #155
handoffs. #154 records the content/table consumer edge only. It does not
duplicate a project/resource model. The `.subdocumentgraph` row is separately
blocked by the completed resolver handoff now owned by #188 and has producer/
output ownership in #199.

## Raw content and escape hatches

.html, .markdown, .htmloptions, .css, .cssproperties, and .llmstxt are not
interchangeable:

- .html and .markdown have bounded target-specific semantic/native content
  behavior already represented in Arkst. Typst/PDF omission is intentional
  and is not rendered-output compatibility.
- .htmloptions is a #154 target-specific content/output configuration consumer,
  not #153 document layout state. Its bounded semantic state does not establish
  an HTML backend.
- .css and .cssproperties remain unsupported and are explicitly deferred to a
  future target-specific HTML backend/product contract. They do not justify a
  generic style system or a raw backend escape hatch; closed historical issue
  #58 is evidence/history only, not their current owner.
- .llmstxt is deferred on the base-URL, page-environment, and output boundary.

Unsupported raw or structured content is fail-closed. This audit does not
lower unsupported structures to plain text, generic Markdown, a semantically
supported-looking function-call node, raw Typst, or backend-specific escape
code. Arkst's architecture continues to forbid generic RawTypst,
BackendRaw, and equivalent backend-code escape hatches.

## Math and slide boundaries

The math rows cover inline/display syntax and the .math producer/consumer;
their producer implementation is assigned to #185, with #181 retained only as
the shared caption/identifier/reference/index dependency.
.texmacro remains the distinct #153/#180 boundary: raw TeX body, document macro
map, source-order replacement, and math-renderer consumption belong to
[#180](https://github.com/luceat-lux-vestra/scribium/issues/180). No .texmacro
implementation is reassigned or added here.

.fragment and .speakernote are #154 slide content producers. Global .slides
configuration, document type, controls, transitions, and speaker-note options
remain #153. The content rows remain `UNSUPPORTED` until a bounded slide
representation and output contract is reviewed in [#184](https://github.com/luceat-lux-vestra/scribium/issues/184),
following the order in [`RECONCILIATION.md`](RECONCILIATION.md).

## Follow-up reconciliation

No one-issue-per-surface fragmentation was created. Existing ownership was
reused where it already describes the boundary:

| Follow-up | Cohesive boundary |
|---|---|
| [#181](https://github.com/luceat-lux-vestra/scribium/issues/181) | Structural content plus shared caption/identifier/reference/index infrastructure; no `.code`/`.math` producer implementation |
| [#182](https://github.com/luceat-lux-vestra/scribium/issues/182) | Image/media sizing, media storage, icons, diagrams, chart/figure resource and renderer contract |
| [#183](https://github.com/luceat-lux-vestra/scribium/issues/183) | Callable/generated/computed table producers, traversal, conversion, and output; consumes #181 shared infrastructure |
| [#184](https://github.com/luceat-lux-vestra/scribium/issues/184) | `.text`, `.box`, `.todo`, `.collapse`, `.textcollapse`, `.clip`, `.float`, `.fullspan`, `.fragment`, `.speakernote`, `.keybinding`, and `.loremipsum`; remaining `.container` subcontracts are linked here |
| [#185](https://github.com/luceat-lux-vestra/scribium/issues/185) | Inline/display math, `.math`, code presentation/captions, `.codespan`, and explicit breaks; consumes #181 shared infrastructure |
| [#198](https://github.com/luceat-lux-vestra/scribium/issues/198) | `.match` pattern/callback traversal and inline-content replacement semantics; #181 remains shared infrastructure only |
| [#199](https://github.com/luceat-lux-vestra/scribium/issues/199) | `.subdocumentgraph` producer and output contract after #188 logical resource resolution; #181 is coordinated only for shared identifiers/indexing |
| [#155](https://github.com/luceat-lux-vestra/scribium/issues/155) | Resource/environment ownership used by media and content consumers |
| [#149](https://github.com/luceat-lux-vestra/scribium/issues/149), [#165](https://github.com/luceat-lux-vestra/scribium/issues/165), [#166](https://github.com/luceat-lux-vestra/scribium/issues/166), [#167](https://github.com/luceat-lux-vestra/scribium/issues/167) | Binding, conversion, raw-body, and atomicity prerequisites |
| [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) | Implemented bounded `arkst-markdown` frontend slice for source-backed inline Markdown in static content arguments; the #158 nested tight-call representation is retained. Dynamic/content conversion and producer/output semantics remain outside this handoff. |
| [#180](https://github.com/luceat-lux-vestra/scribium/issues/180) | .texmacro and math-renderer dependency |

The #154 manifest keeps `.keybinding` as actionable `UNKNOWN` with #184 as
its bounded review owner, and keeps `.loremipsum` as `UNSUPPORTED` with #184
as its content-producer owner. `.css` and `.cssproperties` remain
`UNSUPPORTED` with an explicit `DEFERRED_PRODUCT_SURFACE:html-backend`
disposition; no closed issue is treated as an implementation owner. The
`.subdocumentgraph` row remains `BLOCKED` on #188 and follows #199 for graph
and output semantics. These are canonical issue/defer/blocker links, not
implementation performed by this audit.

These follow-ups were created for bounded implementation review only. None
is started by this audit; implementation ordering is defined by
[`RECONCILIATION.md`](RECONCILIATION.md).

## Validation and guard obligations

The offline guard in
[content_media_markdown_extensions_audit.rs](../../../crates/arkst-core/tests/content_media_markdown_extensions_audit.rs)
checks, without network access:

- exact target and audit-base pins;
- 30-column rows, declared totals, unique canonical names, and unique aliases;
- valid statuses and the distinction between #154-owned rows and
  NOT_APPLICABLE handoffs;
- pinned provenance in every row;
- required Markdown-versus-Quarkdown boundary language;
- required #153/#155/#180 dependency edges;
- status-specific evidence and exact status-to-gap consistency;
- representative #181–#185 producer/shared-infrastructure ownership, with no
  #175 attribution for component-local rows; and
- no accidental production semantic/state/backend scope in the audit record.

Repository-wide checks are reported separately from this document. A passing
offline guard is evidence that the inventory is internally reconciled; it is
not remote CI, a merge, or a claim of complete Quarkdown compatibility.
