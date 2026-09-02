# Quarkdown Compatibility Specification

## Status

- **Specification version:** 0.5 (verified baseline v2.5.1)
- **Reference upstream:** Quarkdown v2.5.1
- **Compatibility target:** complete public-language/document-semantics compatibility
- **Current verified compatibility:** partial; only evidence-backed matrix rows are claims

## Scope

This document defines Arkst's Quarkdown-compatible syntax and semantics.
Each feature records its specification source, compatibility level, and known
divergences.

Arkst's long-term target is complete compatibility with the publicly
documented Quarkdown document language and document-observable semantics of the
tracked stable upstream release. The Feature Matrix records current verified
claims, not a permanent selected language scope: rows marked `Implemented` are
claims only at their stated compatibility level and only with the listed
conformance evidence. `SPEC_SOURCES.md` records upstream provenance. Rows
marked `Planned` or `Not implemented` are explicit compatibility gaps/debt and
must not be treated as supported.

The current implementation is partial. A feature being documented upstream is
not evidence that Arkst supports it, while a feature not yet implemented is
not thereby outside the long-term language target. The tracked target and
verified baseline are distinct; see [Upstream Evolution](#upstream-evolution).

Issue #156 is the cross-audit canonical view for the v2.5.1 status, evidence
layers, ownership handoffs, backlog dependencies, and implementation order:
[`RECONCILIATION.md`](RECONCILIATION.md). The detailed audit matrices and
manifests linked below remain the row-level evidence authorities.

The remaining #151 `UNSUPPORTED` families have cohesive implementation owners:
[#194](https://github.com/luceat-lux-vestra/scribium/issues/194) for `.get`,
[#195](https://github.com/luceat-lux-vestra/scribium/issues/195) for library
inspection, [#196](https://github.com/luceat-lux-vestra/scribium/issues/196) for
localization table/lookup, and
[#197](https://github.com/luceat-lux-vestra/scribium/issues/197) for logger /
diagnostic builtins. For #154, `.match` is #198-owned,
`.keybinding`/`.loremipsum` are explicitly within #184, and
`.subdocumentgraph` is blocked by #188 with producer/output ownership in #199.
`.css` and `.cssproperties` remain unsupported with an explicit defer until a
target-specific HTML backend/product contract exists; closed #58 is historical
evidence only.

“Full compatibility” means public document-language behavior and
document-observable semantics for the tracked release. It does not require
Quarkdown implementation identity, private APIs, undocumented bugs, internal
data structures, private plugin ABI, or internal compiler architecture.

The Quarkdown function-call grammar is audited and implemented at the bounded
parser/frontend boundary recorded by #148. The #157 identifier, named-delimiter,
numeric-reference, and call-start lexical slice is aligned with the pinned
evidence, including prefix parsing such as `.1abc` as `.1` plus source
remainder and non-diagnostic incomplete named candidates; the remaining
call-grammar gaps are still partial. The work is
clean-room from public documentation, notably *"Syntax of a function call"*
on the Quarkdown wiki. No Quarkdown source code is copied or translated. See
`SPEC_SOURCES.md` and [`RECONCILIATION.md`](RECONCILIATION.md) for provenance
and canonical layer status.

The remaining public-language surface is tracked in the
[`GAP_INVENTORY.md`](GAP_INVENTORY.md). It records upstream evidence, Arkst
status, semantic gaps, conformance evidence, and recommended order for
subsequent bounded slices; it replaces an opaque remaining-M2 list.

The function/lambda/component/document architecture gate is recorded in
[`PROGRAMMABLE_DOCUMENT_SEMANTICS.md`](PROGRAMMABLE_DOCUMENT_SEMANTICS.md),
with the normative representation and evaluator/backend boundary in
[`ADR-0020`](../../adr/0020-programmable-document-semantic-model.md). That
record preserves the original architecture decision separately from current
implementation status. The bounded row/column/grid and other component slices
are documented below; they do not promote the complete public component,
style, or layout surface to compatibility.

The canonical value, invocation-binding, and target-conversion inventory is
[`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md). It is the authority for the
dynamic/static origin boundary, optionality, collection ordering, binding
ownership, conversion failures, and state-commit findings; this README keeps
only the bounded feature-family summary.

The canonical programmable-language semantics inventory is
[`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md). Its
`#150` rows are the authoritative reconciliation for variables, scope,
callables, evaluation order, chaining, conditionals, iteration, collection
evaluation boundaries, precedence, extension semantics, failure effects, and
diagnostic/provenance claims. The older bounded family rows below remain useful
evidence links but must not be read as a complete v2.5.1 support claim.

The canonical standard-library/general-builtin inventory is
[`STDLIB_BUILTINS_AUDIT.md`](STDLIB_BUILTINS_AUDIT.md). Its pinned manifest
contains the complete 162-name sweep and separates the 60 #151-owned names
from the 102 names owned by #150 and #152–#155.

The canonical layout, pagination, style, and document-configuration inventory
is [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md)
with its machine-checkable
[`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv).
It re-audits the 20 #152→#153 handoffs and explicitly separates document-wide
configuration from component-local layout and renderer/output behavior. Its
per-surface statuses supersede the historical family-level layout rows below.

The canonical content, media, presentation-component, and
Quarkdown-Markdown-extension inventory is
[CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md)
with its machine-checkable
[CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv).
It separates ordinary Markdown/CommonMark/GFM behavior from Quarkdown
extensions, callable content semantics, resource/project ownership,
caption/numbering/reference consumers, and Typst/output fidelity. Parser
retention is not treated as semantic or end-to-end support. The audit is
documentation/guard-only; implementation sequencing is now defined by
[`RECONCILIATION.md`](RECONCILIATION.md).

The canonical filesystem, project, data, and resource-backed inventory is
[`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md)
with its machine-checkable
[`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv).
It audits the pinned v2.5.1 resource surface, nested/source-relative bases,
project boundaries, remote/network behavior, deterministic external inputs,
native/WASM implications, and the current Typst source-context coupling. The
audit is documentation/guard-only and preserves the #155 → #156 → #187 order.

## Feature Matrix

| Feature                        | Syntax                           | Compatibility            | Status           |
|--------------------------------|----------------------------------|--------------------------|------------------|
| Dot-prefixed call              | `.note`                          | Parsed                   | Parsed-only: pinned identifier and call-start lexical boundaries are aligned by #157; separator and recovery gaps remain |
| Implicit positional refs       | `.1`, `.2`, ... in a headerless callable body | Parser-supported numeric identifiers; binding is separate | Parsed-only lexical recognition, including `.0`/`.01` and `.1abc` as `.1` plus remainder; implicit-reference evaluation remains #150-owned |
| Positional arguments           | `.range {1} {10}`                | Parsed                   | Parsed-only grammar evidence; semantic support is separate |
| Named arguments                | `.panel width:{320}`             | Parsed                   | Parsed-only: identifier and `:{` adjacency are aligned by #157; source-ordered mixed shape is retained by #163 and consumed by the shared engine binder in #165 |
| Mixed positional/named         | `.panel {Intro} width:{320}`     | Parsed                   | Parsed-only: frontend/IR retains the complete source-ordered sequence, including positional-after-named; the shared engine binder rejects invalid ordering with source-backed `E3003`; no complete value/conversion compatibility claim is made (#163, #165) |
| Escaped call/argument delimiters | `\.foo {x}`, `.foo {a \} b}` | Parsed                   | Partial: escaped argument delimiters are counted by the current parser; UTF-8/CRLF truncation and `E2003` behavior are tracked by #162 |
| Indented body argument         | `.panel {x}` + indent            | Parsed                   | Parsed-only grammar evidence; body semantics are separate |
| Nested calls                   | `.outer {.inner {x}}`            | Parsed                   | Partial: ordinary and tight nested calls plus supported Markdown inline content preserve source structure; semantic/output support remains separate |
| Inline (mid-paragraph) call    | `see .note {x}`                  | Parsed                   | Parsed-only placement evidence; semantic/output support is separate |
| Tight-call boundaries          | word adjacency rejected          | Parsed                   | Parsed-only: top-level and nested tight calls preserve wrapper and inner provenance; malformed recovery remains #159 |
| Malformed-call diagnostics     | `E2003`, `E2004` | Error                  | Partial: incomplete optional named candidates remain source remainder; inline recovery suffix loss is #159 |
| Variables                      | `.var {name} {value}`, `.name`, `.name {value}`, `.if {.name}` | Semantically supported | Implemented      |
| Conditionals                   | `.if {cond}` / `.ifnot {cond}`, including selected logical expressions | Semantically supported for literals, variables, and the logical/comparison slice | Implemented (evidenced slice) |
| Logical/comparison predicates  | `.islower`, `.isgreater`, `.equals`, `.not` | Typed boolean results, numeric ordering, plain-text equality fallback, lazy conditional use | Implemented (bounded v2.5.1 slice) |
| Mathematical/numeric operations | `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, `.round`, `.iseven`, plus `.range` | Typed numeric/boolean results with shared binding, upstream Float/Double/Float operation boundaries, binary64 `.pi`, deterministic software transcendental evaluation, DynamicValue Number conversion for textual `decimals` followed by Int-only normalization, and Kotlin ties-to-even rounding | Implemented (bounded v2.5.1 numeric family) |
| String/text operations         | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, `.startswith`, `.plaintext` | Typed scalar string results and boolean predicates plus bounded `.plaintext` projection from already-parsed inline IR; `.capitalize` uses the pinned Temurin 25/Unicode 16 oracle and `.startswith(ignorecase:true)` uses Kotlin/JVM-compatible character-wise case matching, with no normalization or locale/global state; Dynamic String → InlineMarkdownContent conversion is implemented only at the explicit `.plaintext` target | Partial (bounded v2.5.1 slice; `.capitalize` and `.startswith` are `SUPPORTED_SEMANTICS`) |
| Inline hard line break          | `.br` | Argumentless inline `LineBreak` producer represented as the existing `IrInline::HardBreak`; surrounding order, call provenance, source-defined shadowing, atomic invalid forms, `.plaintext`, serde, and existing Typst lowering are covered | Implemented (bounded v2.5.1 slice) |
| Target-specific HTML content  | `.html {<em>world</em>}` or isolated `.html` with an indented body | Closed `Html` target-specific semantic node, explicit `NativeContent` capability, verbatim payload retained for a future HTML output backend, silent Typst/PDF omission | Implemented (bounded semantic slice; no HTML backend) |
| User-defined functions         | `.function {name}`, explicit/implicit parameter modes, optional `parameter?`, positional/named calls, block-last binding | Semantically supported for the evidenced slice | Implemented (evidenced slice) |
| Scoped `.let` evaluation        | block explicit one-parameter or headerless `.1` lambda | Semantically supported for the evidenced slice | Implemented (block form) |
| Optionality and callback values | `.none`, `.isnone`, `.otherwise`, `.ifpresent`, `.takeif` | Typed `IrValue::None`; `.ifpresent` absence short-circuit; first-class `@lambda` or headerless indented callbacks; Boolean-only `.takeif`; source-backed atomic failures | Implemented (bounded v2.5.1 slice) |
| Iteration                      | typed `Range` / `Collection` / `Pair` / ordered `Dictionary`; block and inline `.foreach` / `.repeat` | Block bodies and native contextual inline callable bodies are semantically supported for typed values, closed inclusive ranges, left-open ranges starting at 1, descending-empty behavior, ordered list adaptation, ordered dictionary entries, explicit/implicit parameters, Pair destructuring, typed collection results, parent visibility, child isolation, owner writeback, atomic failure, and R10 materialization/depth limits | Implemented (evidenced block/inline slice; generalized inline component/callback bodies, right-open/fully-open iterable rejection, and broader patterns deferred) |
| Collection access              | `.size`, `.first`, `.second`, `.third`, `.last`, `.getat` | Typed access over `Collection`, `Pair`, ordered `Dictionary` entries, finite closed or left-open `Range`, and Markdown list values; one-based access with upstream absence/fallback behavior | Implemented (evidenced slice) |
| Collection operations          | `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, `.groupvalues` | Shared typed iterable materialization, upstream `asDouble()` aggregation, stable first-occurrence distinctness, reverse order, nested first-seen groups, and stable `by` selector sorting | Implemented (evidenced v2.5.1 slice; table operations remain deferred) |
| Generic callable and transforms | `@lambda ...`, contextual `by:{...}`, `.foreach`, `.map`, `.filter`, `.sorted` | Typed callable values, shared child-scope invocation, recursive results, and shared iterable adaptation; `.foreach` and `.sorted` are native compatibility evidence, while `.map`/`.filter` are Arkst extensions excluded from conformance counts | Implemented (bounded callable/native-transform slice) |
| Functions/components            | —                                | Complete public component/layout semantics remain partial; bounded typed Stacked, Container, and Landscape consumers are implemented and tracked separately | Partial (bounded) |
| Include/read                   | `.include {path}`, `.read {path}` with optional `lines` range | Source-relative logical `VirtualProject` resources; included sources retain their own source identity and working directory; active-stack cycle detection; no host filesystem or network access | Partial (bounded subset; #188) |
| Metadata                       | `.doctype`, `.docname`, `.docdescription`, `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, `.theme`, and related document metadata | Canonical Issue #152 classification is eight `PARTIAL` evaluator/IR slices (`.doctype`, `.docname`, `.docdescription`, `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, `.theme`). `.localization`/`.localize` remain #151-owned `UNSUPPORTED` general stdlib functions; renderer, front-matter, and cross-owned layout/content/resource state remain separate | [#152 audit](DOCUMENT_STATE_AUDIT.md) and [#151 manifest](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv) |
| Layout/document configuration | `.numbering`, `.nonumbering`, `.font`, `.paragraphstyle`, `.captionposition`, `.texmacro`, `.pageformat`, `.pagemargin`, `.footer`, page counters/format/reset, `.lastheading`, automatic page breaks, `.marker`, `.navigation`, `.tableofcontents`, `.slides` | Canonical #153 result: one `PARTIAL` bounded evaluator/IR `.captionposition` slice and 19 `PARSED_ONLY` unresolved-call rows; no #153 output-equivalence claim | [#153 audit](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md) |
| Row/column/grid                | `.row`, `.column`, `.grid columns:{2}` with a Markdown block body | Block-only native consumers with typed `IrComponent::Stacked`: Row, Column, and positive-column Grid; typed main/cross alignment and Size gaps; structured children and source provenance; argument validation before lazy body evaluation; pure Typst lowering and real backend integration evidence | Implemented (bounded Stacked layout slice) |
| Container sizing               | `.container`, optional `width`, `height`, `fullwidth`, and Markdown body | Empty/body-only structured Container; origin-aware Size/Boolean conversion; deterministic Typst block sizing | Partial (bounded) |
| Semantic evaluation            | `.if`/`.ifnot` + variables + user-defined functions + block `.let` + evidenced chain builtins | Partial / In progress | Implemented (partial) |
| Call chaining (`::`)           | `.a {x}::b {y}` and documented nested equivalent `.b {.a {x}} {y}` | Semantically supported for the evidenced scalar builtins, including `.otherwise` and `.isnone`; direct chain and nested forms share value-context invocation, with strict left-to-right flow and source-backed `E3001` failures for unimplemented callees | Partial: direct-chain semantic slice is evidenced; optional whitespace/continuation before `::` remains the grammar gap in #164 |
| Line continuation (`\`)        | `\` at end of line               | Parsed                   | Partial: after-argument continuation is evidenced, but first-argument, trailing, and chain-separator placement are tracked by #164 |
| Tight / brace-wrapped calls    | `H{.text {2}}O`                  | Parsed                   | Parsed-only: top-level and nested tight wrapper/inner spans are preserved |
| Multi-line arguments           | `{.…}` parsing spans lines        | Parsed                   | Parsed-only grammar evidence |
| `.json` data loading           | `.json {path}` (new in v2.5.0)   | UTF-8 JSON mapped to recursive typed `IrValue` collections/dictionaries/scalars; exact binary64 integer boundary; logical resource diagnostics | Partial (bounded source-relative resource slice; #155/#188) |
| `.markdown`                    | `.markdown {content}` (new in v2.5.0) | Raw `NativeContent` Markdown node retained for a future Markdown output target; this is not a file loader | Implemented (bounded native-content slice) |
| `.llmstxt`                     | `content: String`, `markdownavailable: Boolean` | Pinned `Html.kt` declares public `@QFunction llmstxt`; target-specific output/configuration remains #155-owned and Arkst keeps the explicit deferred boundary | Intentionally deferred |

The v2.5.1 Markdown deltas are recorded in
[`V2_5_1_IMPACT.md`](V2_5_1_IMPACT.md). D2 link-parenthesis behavior and D3
deep four-space list behavior are tested at the Markdown frontend boundary;
the tests cover both ordinary Markdown and Quarkdown-mode inputs, including a
Quarkdown directive body. These rows are evidence for the tested frontend
behavior only and do not imply full Quarkdown compatibility.
D2 correction evidence additionally covers empty and whitespace-empty inline
destinations, exact angle/title/multiline-link spans, and link-kind isolation:
inline destinations receive the tested escape normalization while Auto,
Reference, and Image destination representations remain unchanged.

### Bounded `.docauthor` document-state contract

The pinned Quarkdown v2.5.1 document-metadata contract gives `.docauthor` a
read/write dual behavior. With no argument it returns an empty string when no
author exists, or the first author name otherwise. A positional `String` or
named `author:` setter appends one author and returns no document value; it
does not replace earlier authors. Arkst preserves this order in the shared
evaluator-owned `DocumentState` and the final immutable
`IrDocument.metadata.document_state` snapshot.

The bounded implementation reuses the existing invocation-time String
conversion boundary, validates arity and named-argument shape before the
author commit, and reports invalid calls through source-backed structured
diagnostics. `IrDocumentAuthor` stores only bounded ordered string info for
the separate `.docauthors` slice. Front-matter
`IrMetadata.author` is not synchronized with document-state authors, and no
Typst author rendering policy is introduced here.

Evidence is in `crates/arkst-core/src/lib.rs::tests::docauthor_*`, the IR
serde test, and the independently authored
`fixtures/quarkdown-conformance/cases/docauthor-family/` case.

### Bounded `.docauthors` document-state contract

The pinned Quarkdown v2.5.1 contract gives `.docauthors` the same read/write
dual behavior as `.docauthor`. A setter accepts a
`Dictionary<String, Dictionary<String, String>>` through the documented
nested Markdown-list body, an already-evaluated typed dictionary, or the
normal `authors:` binding. It appends each dictionary entry to the shared
ordered author state and returns no document value. The argumentless getter
returns an ordinary typed dictionary whose values are nested typed
dictionaries.

Arkst stores each author as a backend-neutral name plus ordered string info
pairs. Dictionary construction preserves the first insertion slot and uses
the last value for duplicate names or info keys, matching the pinned
`mutableMapOf`/`dictionaryOf` behavior. Repeated `.docauthor` calls remain
duplicate ordered state, while the getter presents the resulting dictionary
view. Validation is bounded to scalar/plain-text-safe strings; rich
components, callables, ranges, collections, and arbitrary recursive metadata
are rejected. Setter evaluation is validate-then-commit and restores the
previous document state on failure.

Source-defined `.docauthors` functions shadow the native only in the same
model as `.docauthor`; `.docname`, `.docdescription`, and `.doctype` retain
native-first precedence. No author rendering or front-matter merging is
introduced.

Evidence is in `crates/arkst-core/src/lib.rs::tests::docauthors_*`, the
ordered author serde tests, and
`fixtures/quarkdown-conformance/cases/docauthors-family/`.

### Bounded `.dockeywords` document-state contract

The pinned Quarkdown v2.5.1 contract gives `.dockeywords` an argumentless
getter returning the current keywords as an iterable and a setter accepting an
iterable. Arkst supports the documented Markdown list body, an already
evaluated typed iterable through normal positional binding, and the named
`keywords:` binding. For a source-backed body, the shared iterable converter
evaluates one dynamic expression in the current context and prefers its typed
Iterable or Dictionary result before using the Markdown-list fallback. Getter
order is preserved, the default is an empty list, and every successful setter
**replaces** the complete prior keyword list.

The evaluator materializes and validates the complete candidate before one
state commit. Only the existing bounded String-like scalar families
(String/Identifier/Number/Boolean) are accepted as keyword elements; ranges,
nested collections, rich content, callables, components, and unresolved
values remain rejected. The shared evaluator state is visible in ordinary
callable child scopes, while `IrDocument.metadata.document_state.keywords` is
an immutable backend-neutral snapshot with serde defaults for older IR.
Source-defined `.dockeywords` functions shadow this native builtin under the
same model as `.docauthor(s)`; `.docname`, `.docdescription`, and `.doctype`
retain native-first precedence. No HTML SEO rendering, Typst metadata,
front-matter merging, or generalized conversion is introduced.

Evidence is in `crates/arkst-core/src/lib.rs::tests::dockeywords_*`, the IR
serde test, and the independently authored
`fixtures/quarkdown-conformance/cases/dockeywords-family/` case.

### Bounded `.doclang` document-state contract

The pinned Quarkdown v2.5.1 contract gives `.doclang` read/write dual
behavior. With no argument it returns the current locale's `localizedName`,
or an empty String when no locale is set. A positional `locale` String or
named `locale:` setter resolves an English locale name case-insensitively
before trying a locale tag, replaces the current locale, and returns no
document value. The bounded IR snapshot stores only the canonical tag and the
localized getter name.

The upstream `.doclang` input contract is broader than the built-in
localization table: it accepts a case-insensitive English full name or an IETF
BCP 47 language tag. Arkst uses a checked-in, immutable pure-Rust snapshot
of the observable locale data returned by the pinned Temurin `25.0.4.1+1`
oracle, with CLDR provider routing and active FALLBACK-root values captured at
generation time. This avoids OS/JVM locale databases, hidden environment
state, and host-dependent results while keeping the evaluator WASM-compatible.
The snapshot is a compatibility representation of the pinned JDK behavior,
not a generic JVM, CLDR, or ResourceBundle implementation; localization-table
and rendering semantics remain separately owned.

`.none` follows the pinned nullable `String? = null` path: it invokes the
getter and does not clear the existing locale. The setter validates argument
shape, evaluates and converts the candidate, resolves it, and commits once;
failed resolution restores the previous locale, including state mutated by a
nested candidate evaluation. #166 retains a source-backed raw block body beside
its parsed nodes and converts the bounded `.doclang` fallback without executing
those nodes. Ordinary callable child scopes share the
locale state. Source-defined `.doclang` shadows the native builtin, while the
historical `.docname`, `.docdescription`, and `.doctype` native-first behavior
is unchanged. Arkst introduces no localization tables, `.localize`
implementation, hyphenation, Typst/HTML language output, or locale-aware
rendering. The standard-library
registration hook separately loads `/lib/localization.qd` before user calls;
that pinned resource invokes `.localization name:{std}`, so the stdlib-ready
initial localization table contains the seeded `std` table. Localization table
mutation and lookup remain canonical #151-owned behavior, not #152 semantics.

Evidence is in `crates/arkst-core/src/lib.rs::tests::doclang_*`,
`crates/arkst-engine/src/locale.rs`, the IR serde test, the pinned
`Document.kt`/locale/binder/refiner sources recorded in `SPEC_SOURCES.md`, and
the independently authored
`fixtures/quarkdown-conformance/cases/doclang-family/` case.

### Bounded `.theme` document-state contract

The pinned Quarkdown v2.5.1 implementation exposes `.theme(color: String? =
null, layout: String? = null)` as a setter returning no document value. Both
regular parameters can bind positionally or by name; `@LikelyNamed` on
`layout` is metadata and is not a runtime positional restriction. An indented
body falls back to the final bindable parameter, so it binds `layout` for this
signature. Arkst retains the lossless source-backed body beside the parsed
body and derives the target-conversion `DynamicValue` as
`trimIndent().trimEnd()`; nested body calls are not executed as a substitute
for the raw value. Supplied string components are lowercased and theme
existence is left to the rendering boundary. Arkst stores the result as an explicit
backend-neutral `IrDocumentTheme` in the shared evaluator-owned document state.

Each successful call replaces the complete theme. Therefore a later
`.theme {Light}` stores `color = "light"` and `layout = null` after an earlier
call supplied both components; it does not preserve the previous layout. A
supplied `.none` is accepted by the nullable `String?` parameters and stores a
null component. An argumentless `.theme` is still a setter and stores an
explicit empty theme `Some({ color: null, layout: null })`, distinct from the
absence of any `.theme` call. This follows the pinned source implementation
even though the public KDoc describes omitted components as being kept/defaulted;
resolution and defaults remain outside this bounded contract.

The existing invocation-time scalar String boundary accepts String,
Identifier, Number, and Boolean values for each component; `.none` maps to a
null component. Collections, Dictionary/Pair, Range, Callable, Component,
rich content, and unresolved values remain outside this bounded component
 conversion. A block body is retained as source-backed raw text and selected
target conversion happens before its parsed nodes can be evaluated; this
prevents nested calls or document-state mutations from running accidentally.
Binding,
evaluation, conversion, normalization, and shape validation complete before
one state commit; failures retain the old full theme and keep the original
argument span in the structured diagnostic. Callable child scopes share the
state. Source-defined
`.theme` shadows the native setter under the same bounded policy as the newer
document-state setters, while `.docname`, `.docdescription`, and `.doctype`
retain native-first precedence. Front matter remains separate and no
renderer, theme registry, filesystem lookup, or CSS/Typst integration is
introduced.

Evidence is in `crates/arkst-core/src/lib.rs::tests::theme_*`, the IR serde
tests, the pinned [`RegularArgumentsBinder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt), the pinned [`FunctionCallRefiner.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt), and the independently authored
`fixtures/quarkdown-conformance/cases/theme-document-state/` case.

### Bounded `.captionposition` document-state contract

The pinned Quarkdown v2.5.1 `.captionposition` setter accepts nullable regular
parameters `default`, `figures`, `tables`, and `codeBlocks`, exposed to source
as the named alias `code:`. The closed `CaptionPosition` enum accepts `top` and
`bottom` case-insensitively, and the initial default is `bottom`. The regular
binder permits positional, named, and positional-then-named forms; its
`@LikelyNamed` annotation is documentation metadata rather than a named-only
restriction. Duplicate, unknown, excess, and unnamed-after-named arguments
remain binding failures.

The upstream regular binder also accepts an indented block body. Because
`codeBlocks` is the final bindable parameter, upstream falls back from that
body to `codeBlocks` as a raw `DynamicValue`. #166 retains the lossless
source-backed body beside the parsed `CallBody` and derives its target value
with `trimIndent().trimEnd()` before routing it through the shared conversion
boundary. `.captionposition` therefore does not execute parsed body nodes as
a substitute; caption rendering and broader target coverage remain explicit
gaps.

Each invocation contributes a partial `CaptionPositionInfo` and merges it into
the current state. A supplied `default` replaces only the default; supplied
element-specific values replace their own override; omitted or nullable
`.none` fields preserve the previous value. An explicit override remains
distinct from inheriting the effective default. Arkst validates binding,
evaluates every candidate, uses the post-evaluation shared state as the merge
base, converts through the existing closed-enum boundary, and commits one
complete `IrCaptionPositionInfo` snapshot. This preserves successful nested
caption-state mutations while failures still restore the pre-invocation
state. The source-backed body fallback is now covered by #166; caption
rendering and broader target coverage remain explicit gaps.

The evaluator-owned state is shared by callable child scopes, while a
source-defined `.captionposition` shadows the native setter in direct and
chained calls. Successful calls return no document content. The immutable IR
contains only `IrCaptionPosition` and `IrCaptionPositionInfo`; Typst/HTML
caption placement, `.figure`, `.table`, and `.code` rendering remain deferred.

Evidence is in `crates/arkst-core/src/lib.rs::tests::captionposition_*`,
`crates/arkst-engine/src/value_conversion.rs`, the IR serde tests, the
pinned `CaptionPosition.kt`, `CaptionPositionInfo.kt`, `Document.kt`,
`RegularArgumentsBinder.kt`, Amber merge generator, and the independently
authored `fixtures/quarkdown-conformance/cases/captionposition-document-state/`
case.

### Bounded `.container` sizing contract

The direct `.container` consumer is **partial/bounded**, not complete upstream
Container support. The implemented subset is:

- an empty or optional Markdown block body with structured `Vec<IrNode>`
  children;
- `width: Size?`, `height: Size?`, and `fullwidth: Boolean` through the
  existing origin-aware invocation conversion boundary;
- positional prefix binding in `width`, `height`, `fullwidth` order and named
  equivalents, with duplicate, unknown, and deferred-known parameters
  rejected by source-backed diagnostics; and
- backend-neutral `IrComponent::Container` materialization and Typst
  `#block` sizing, where explicit width overrides `fullwidth` and an
  unaligned container emits no `#align` wrapper.

The deferred surface includes `float`, `fullspan`, `classname`, StyleOptions,
container alignment/text alignment, colors, borders, margin/padding/radius,
font/text-style properties, and inline Container insertion. Evidence is in
`crates/arkst-core/tests/quarkdown_container.rs`, the IR/lowering unit
tests, and `crates/arkst-typst-subprocess/tests/backend_integration.rs`.

### Bounded row/column/grid (Stacked) layout contract

The `.row`, `.column`, and `.grid` rows are **Implemented (bounded Stacked
layout slice)**. They are block-only native consumers, not a claim that the
complete public layout family is compatible. The reviewed implementation:

- preserves a typed backend-neutral `IrComponent::Stacked` value until the
  block output boundary;
- represents Row and Column distinctly, and Grid with a positive typed column
  count;
- converts main-axis and cross-axis alignment, plus typed `Size` gaps, through
  the existing origin-aware invocation boundary;
- retains structured `Vec<IrNode>` children and source provenance for the call,
  arguments, and nested children;
- validates arity, duplicate/unknown arguments, alignment, gap, and positive
  grid-column constraints before evaluating the lazy Markdown body; and
- lowers the typed component through pure Typst code generation with source
  maps, backed by real Typst/PDF integration tests.

The public component/style/layout surface remains partial: inline Stacked
insertion, general String-to-Markdown conversion, broader Container styles,
and unrelated layout families remain deferred.

### Target-specific `.html` contract

The fixed upstream reference for this contract is Quarkdown v2.5.1 at tag
commit [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e).
[`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L59-L90)
declares one regular `content: String` argument, checks
`Permission.NativeContent`, and returns `Html(content)` as a node. The
function-call grammar permits both isolated block calls and inline calls; the
same generic node can occur between paragraph children. The HTML renderer
returns the content verbatim, while the v2.5.1 plaintext and GFM visitors
return empty output. The normal CLI permission default includes
`native-content`; denial is an evaluator-time missing-permission failure.

Arkst's implementation status is **implemented for the closed `Html` target
semantic slice**. The evaluator accepts one regular `content: String` through
positional, named, inline, and indented-body forms. The ordinary `compile(...)`
entry point uses `Capabilities::compatibility_default()`, which grants
`NativeContent`; hosts using `compile_with_capabilities(...)` may supply an
explicit capability set, including one that denies `NativeContent`. Denial
emits one source-backed `E3004` diagnostic before node creation.

- `.html {<em>world</em>}` evaluates to a block `TargetSpecificContent` node;
- `**Hello** .html {<em>world</em>}!` keeps the target node inline between
  surrounding text;
- an indented body is retained as an opaque function-body String boundary;
- Typst/PDF omit the target-specific payload without warnings or source-map
  entries.

The implemented representation is a closed backend-neutral target-specific
content payload carrying `NativeTarget::Html`, the evaluated String, and its
`SourceSpan`, with placement-preserving block and inline carriers. A future
HTML output backend, whose physical crate/name is not frozen here, will emit the
string verbatim. `arkst-html` continues to normalize Markdown/foreign HTML
only and does not consume this payload. This is not a generic raw backend/MIME
mechanism.
Ordinary `<em>x</em>` or `<!-- comment -->` in `.qd`/`.scrib` remains the
separate source-language raw-HTML case and continues to fail closed with
`E8001`. See [ADR-0018](../../adr/0018-quarkdown-target-specific-native-content.md)
and [RAW_HTML_POLICY.md](../RAW_HTML_POLICY.md).

### Project-backed resource builtins

The reviewed v2.5.1 [`Data.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Data.kt)
contract makes `.read` a UTF-8 text operation with normalized line separators
and an optional one-based inclusive `lines` range. `.json` parses a resource
into recursive object/array/scalar values. The reviewed
[`Ecosystem.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Ecosystem.kt)
contract evaluates `.include` as a separate source with a working directory
at that source's parent and supports `share`, `scope`, and `subdocument`
sandbox modes. Arkst implements this evidenced subset over the
host-supplied `VirtualProject`.

| Builtin | Arguments / result | Resource semantics | Missing/unsupported behavior | Status |
|---|---|---|---|---|
| `.read` | `path`, optional `lines`; `String` | UTF-8 text, normalized line separators, source-relative | structured missing, boundary, URI, and invalid-UTF-8 diagnostics | Partial |
| `.json` | `path`; recursive `IrValue` | UTF-8 JSON object/array/scalar mapping; deterministic insertion order | structured parse and numeric-precision diagnostics | Partial |
| `.include` | `path`, optional `sandbox`; evaluated content | parses/evaluates a separate source with its own `SourceId` and working directory | structured missing/boundary diagnostics and active-stack cycle detection | Partial |
| `.markdown` | raw `content`; `NativeContent` | preserves content; it does not load a file | native-content capability diagnostic when denied | Implemented |
| `.llmstxt` | `content: String`, `markdownavailable: Boolean` | pinned `Html.kt` public declaration; target-specific output/configuration remains outside this slice | explicit deferred diagnostic | Deferred |

Resource paths are logical and source-relative. A nested include changes the
base for subsequent `.read`/`.json`/`.include` calls to the included source;
the active source stack detects cycles while repeated, completed includes are
allowed. `VirtualPathBuf` enforces the project boundary. Absolute paths,
Windows paths, URI schemes, missing resources, and invalid UTF-8 are reported
as structured diagnostics without exposing a physical host root. The same
evaluator path works with an in-memory `VirtualProject`, which is the WASM
model; the CLI is responsible only for bounded native project loading.

`.markdown` follows the reviewed v2.5.1 [`Markdown.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Markdown.kt)
contract: it preserves raw Markdown content as a `NativeContent` node and is
not a file-loading alias. A Markdown source included with `.include` is parsed
with its own source identity, so relative Markdown images retain the included
document's resource base. Typst currently omits native Markdown content;
image alt text and titles remain preserved in the Markdown AST/IR but are not
emitted as PDF accessibility metadata. `.llmstxt` is present as a pinned
standard builtin; its target-specific output/configuration contract remains
explicitly deferred to #155.

Issue #57 adds a separate end-to-end Markdown evidence slice for structures
already preserved by the frontend: blockquotes, single- and double-tilde
strikethrough, GFM task lists, and GFM tables. The slice carries recursive
content, task state, table
alignment, source spans, evaluator recursion, and deterministic Typst
lowering through `.md`, `.qd`, and an indented Quarkdown body. It is evidence
for the tested structures and forms only; it does not promote raw HTML or
complete CommonMark/GFM support.

For an indented body, the minimum eligibility rule is at least two leading
spaces or one leading tab in the current Rushdown container context. The first
qualifying nonblank line establishes the actual body indentation; later lines
must meet that same container-relative indentation and a dedent ends the body.
The frontend preserves this parser decision for lazy paragraph normalization,
so body ownership is not re-inferred from absolute source columns or a fixed
indentation width. The evidence above covers 2/3/4/8-space bodies, one-space
rejection, single-tab and mixed indentation, UTF-8/CRLF provenance, nested
Markdown and Quarkdown, and list/blockquote-relative containers.

## Issue #61 bounded DynamicValue conversion

The v2.5.1 review distinguishes invocation-time conversion from the upstream
value class hierarchy. `DynamicValueConverter.convertTo()` is consumed at the
argument-binding boundary (`RegularArgumentsBinder.kt` and `Lambda.kt`), and
dispatches to `ValueFactory` conversion methods. Arkst therefore keeps the
conversion policy inside the existing evaluator/value-resolution path instead
of reproducing the Kotlin class model or introducing a universal conversion
engine.

The gate is the argument's origin, not the final `IrValue` shape. Only a value
that reaches the call as a DynamicValue-origin argument receives the textual
Number, Boolean, or Range/Iterable conversion below. A statically materialized
`StringValue`—for example the direct result of nested `.string`—remains a
String and does not acquire those target types. Variable and custom-function
references retain the dynamic invocation boundary when they are passed to a
consumer. This distinction is evaluator-internal and is not emitted into the
backend-neutral IR.

The implemented boundary is deliberately consumer-driven:

| Target | Status | Existing Arkst consumer | v2.5.1 evidence |
|---|---|---|---|
| `Number` | bounded scalar conversion implemented | arithmetic/numeric arguments and dynamic range endpoints | [`ValueFactory.number`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt), [`Logical.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt) |
| `Boolean` | bounded scalar conversion implemented | conditions, predicates, and boolean argument flags | [`ValueFactory.boolean`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [`Optionality.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt) |
| `Range` | bounded conversion implemented for iterable consumers | `.foreach`, collection access, and dynamic range materialization | [`ValueFactory.range`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt), [`Collection.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt) |
| `String` | bounded scalar conversion implemented | scalar string builtins and the typed Range-to-text boundary; static StringValue remains String | [`ValueFactory.string`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [`StringValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/StringValue.kt), [`Strings.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt), [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt) |
| `EvaluableString`, `MarkdownContent`, `InlineMarkdownContent` | bounded context-sensitive conversion implemented; complete coverage deferred | parser/evaluation context is required; source-backed raw bodies remain outside `IrValue` | [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt) |
| `Size` | bounded domain conversion implemented for reviewed consumers | `.container` width/height and Stacked gaps; typed Size identity and DynamicValue-origin text | v2.5.1 `Size` value family and existing evaluator conversion tests |
| `Color`, `Enum`, and remaining layout/document values | component/layout conversion remains partial or deferred | closed enum alignment consumers are implemented; colors, styles, and remaining layout fields are deferred | v2.5.1 value families; bounded consumers only |
| collections, callables, and generic document/content stringification | unsupported conversion; existing typed operations remain separate | typed collection/callable paths only | `DynamicValueConverter.kt`, `ValueFactory.kt`, and consumer signatures |

The scalar rules are evidence-backed: Number keeps an already typed value and
parses **DynamicValue-origin** text as integer first, then Float, without
trimming; Boolean accepts only case-insensitive `true`, `yes`, `false`, and
`no` from that same dynamic boundary; Range accepts the reviewed textual
`x..y`, `..y`, `x..`, and `..` forms without whitespace or signed endpoints
only for a dynamic iterable consumer; and String handles its own typed value,
scalar values, and typed Range text. A static StringValue does not become a
Number, Boolean, or Iterable merely because its text happens to parse. None is
not converted. `.ifpresent(None)` skips its callback while `.takeif(None)`
still invokes its predicate; these are separate from conversion failure and
optional argument omission.

Invalid conversions use the existing source-backed `E3001` path and do not
publish partial IR, collection, or callback results. Conversion is a pure
semantic transformation over the invocation value, its dynamic-origin bit,
and explicit target; it does not access files, processes, the network, or a
backend. A dynamic String is reparsed only by the explicit `.plaintext`
InlineMarkdownContent target path; source-backed bodies use retained raw text,
and all other generic String → Markdown paths remain deferred. The independently authored
`fixtures/quarkdown-conformance/cases/dynamic-value-scalar-family` fixture and
the evaluator/unit tests cover the implemented consumer paths. This is
**bounded scalar conversion implemented**, not broad DynamicValue
compatibility or conversion completion. Resource I/O semantics (`.read`,
`.json`, `.include`, and the deferred data-loading family) are unchanged, and
component/layout semantics remain outside this slice.

## Issue #61 structured iterable semantics

The evaluator now represents `Pair` and `Dictionary` as typed recursive
values. `.pair {first} {second}` creates a `Pair`; `.dictionary` consumes a
Markdown list body and creates an ordered deterministic `Dictionary` whose keys
are strings and whose values remain typed. Duplicate keys use the observed
Quarkdown v2.5.1 last-write-wins rule while retaining the first insertion slot.

Dictionary iteration uses the existing `.foreach` iteration engine and yields
`Pair(key, value)` items in dictionary order. The upstream block header syntax
is `key value:` (not a parenthesized pattern). One explicit parameter binds the
whole Pair; exactly two explicit parameters destructure a Pair into the two
same-child-scope bindings. Explicit lambda scope remains a hard boundary for
implicit `.1`, and each iteration gets a fresh `EvaluationContext::child()`, so
inner bindings mask and then restore outer bindings without leakage.

Pair and Dictionary construction/evaluation is atomic: a failed child or
dictionary entry produces a source-backed evaluator diagnostic, stops further
iteration/evaluation, and does not publish a partial structured value. Direct
document output is materialized by the evaluator as an ordered list for Pair
and a two-column table for Dictionary; Typst lowering does not implement these
language semantics.

This slice intentionally does not add nested or generalized destructuring,
rest/spread patterns, mutation, arbitrary comparator syntax, descending
sorting, or transform forms beyond the shared first-class `by` callback. `.map`,
`.filter`, and `.sorted` now use the generic callable and iterable machinery
described below; `.filter` is Boolean-only and the requested `.map`/`.filter`
surface is not asserted as an upstream v2.5.1 compatibility claim because the
tracked public `Collection.kt` source does not define those functions.

## Collection and Iterable operations evidence

The v2.5.1 public [Iterable](https://quarkdown.com/wiki/iterable/) contract
defines `Collection` as an ordered list value, `Pair` as an iterable of two
values, `Dictionary` as an iterable of key-value pairs, and finite integer
`Range` as an ordered iterable. The public standard-library API and v2.5.1
source were consulted for the public Collection operations:

- [Collection API index](https://quarkdown.com/docs/quarkdown-stdlib/com.quarkdown.stdlib.module.Collection/)
  lists the operation signatures and chaining contract.
- [`Collection.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt)
  establishes `.size` as a non-negative count, one-based `.first`/`.second`/
  `.third`/`.last` access, `.getat` with optional `orelse`, `asDouble()`-based
  `.sumall`/`.average`, equality-based `.distinct`, reverse materialization,
  and nested `.groupvalues` results. It contains no public generic `.map` or
  `.filter` declaration.
- [`Types.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Types.kt)
  shows that `Value.asDouble()` parses String values when possible and falls
  back to `0.0` for other non-numeric values.
- [`IterableTest.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/IterableTest.kt)
  verifies sum, average, distinct, reversed, and group-values behavior,
  including first-seen group order and per-group input order.
- [`IterableValue.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/IterableValue.kt),
  [`DictionaryValue.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt),
  and [`Range.kt` at v2.5.1](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt)
  establish Pair iteration, dictionary-entry adaptation, and finite Range
  iteration behavior.

The exact implemented policy is:

- `.size` accepts one iterable operand (`of:` in ordinary form) and returns a
  typed `Number`; empty values return `0`.
- `.first` and `.last` accept one iterable operand (`from:` in ordinary form),
  return the original typed element, and return semantic `None` for an empty
  value.
- `.getat` accepts an iterable, a finite integral numeric index, and optional
  `orelse:`. Indices are one-based. Zero, negative, very large, and
  out-of-range integral indices return semantic `None` or the typed fallback;
  fractional, non-finite, and non-numeric indices fail with a source-backed
  evaluator diagnostic. No truncating or saturating conversion is used.
- `.second` and `.third` return semantic `None` when the shared materialized
  sequence is too short, matching `.getat {2}` and `.getat {3}` without a
  fallback.
- `.sumall` sums every `asDouble()` conversion; invalid conversions contribute
  zero, while `.average` divides by the full input count and returns `NaN` for
  an empty input. `.distinct` preserves the first occurrence, `.reversed`
  returns a new collection, and `.groupvalues` returns groups in first-seen
  order with original order inside each group.
- Dictionary access observes deterministic ordered `Pair` entries. Pair
  access observes its two components. Closed Ranges are inclusive, descending
  Ranges are empty, and a left-open Range defaults its start to `1`. A
  right-open Range representation is supported, but standard Iterable
  consumption rejects it as endless; fully-open Ranges are rejected the same
  way. Strings and unsupported scalar values are not treated as iterables.

Arkst obtains ordered semantic elements through the same evaluator
adaptation used by `.foreach`, `.sorted`, `.map`, and `.filter`. Results remain
recursive `IrValue`s, so a
Dictionary access returns a Pair that can continue through Pair operations or
feed the existing `.foreach` destructuring path. Nested operand failures remain
atomic and propagate their original diagnostic without a duplicate. The
compile/evaluator evidence is listed in the conformance table below, including
UTF-8 and CRLF source-span coverage.

## Generic callable, native transforms, and Arkst extensions

The v2.5.1 lambda evidence identifies a lambda as a first-class typed value
with explicit named parameters or implicit `.1`, `.2`, and later positional
references. Invocation forks a child scope, preserves lexical parent bindings,
fills omitted optional parameters with `None`, and validates explicit arity.
Arkst stores the body and source spans in `IrValue::Callable`, snapshots
captured bindings deterministically, and routes `.foreach`, `.map`, `.filter`,
`.sorted` selectors, and first-class callback values through one invocation
path. The same path preserves nearest-scope implicit masking and child-scope
isolation.

The explicit first-class form is source-backed `@lambda`, for example:

```text
.var {identity} {@lambda .1}
.map {1..3} by:{.identity}
.map {1..3} by:{value: .value}
```

Transform callback arguments also accept the contextual upstream form without
the legacy marker when they occur as the `by` argument of `.map`, `.filter`, or
`.sorted`. Other content arguments remain ordinary content and are not
reclassified as lambdas.

All transforms first evaluate one value through `coerce_iterable`, then invoke
the callback against each typed element. `.map` returns a typed `Collection` in
source order. `.filter` requires a semantic Boolean predicate result and
returns the original typed elements in source order. `.sorted` accepts natural
keys or a `by` selector and returns a stable ascending typed `Collection`;
Arkst rejects heterogeneous, `None`, and unsupported keys with diagnostics.
There is no descending option or arbitrary comparator language.

The v2.5.1 `Collection.kt` source documents `.sorted(from, by?)` but does not
define public `.map` or `.filter` functions in the tracked tag. `.foreach`
does return an ordered collection with one result per input element, so its
block form has map-equivalent semantics and is included in the native evidence
slice. Consequently `.sorted` and `.foreach` are evidenced Quarkdown
v2.5.1-compatible operations, while `.map` and `.filter` remain Arkst
extensions and are excluded from conformance coverage. Unknown upstream
details remain deferred.
Pair, Dictionary, Range, and supported Markdown-list transforms reuse the exact
`.foreach` element sequence and Range policy. Callback failures, invalid
predicates, unsupported sort keys, and endless ranges publish no partial
result; no value is serialized or reparsed.

## Optionality and callback semantics

The v2.5.1 [`Optionality.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt)
surface defines `.none`, `.isnone`, `.otherwise`, `.ifpresent`, and `.takeif`.
`.ifpresent` maps a non-`None` value through a one-argument lambda and returns
`None` without invoking that lambda for an absent value. `.takeif` invokes its
one-argument condition for every value, including `None`, requires a Boolean
result, and returns the original value or semantic `None`.

Arkst implements this bounded semantic slice in the evaluator. `IrValue::None`
is distinct from terminal `NoValue`; direct output retains the existing text
`None` boundary, while value-context composition retains typed `None`.
First-class `@lambda` callbacks and headerless indented callback bodies reuse
`IrValue::Callable`, immutable capture snapshots, `EvaluationContext::child()`,
nearest-scope implicit parameters, and the existing failure-atomic invocation
path. No source text is generated or reparsed. Ordinary content is not
silently classified as a lambda, so unmarked explicit inline callback headers
remain outside this bounded parser-independent slice.

Evidence is independently authored in
`fixtures/quarkdown-conformance/cases/optionality-callback-family/input.qd` and
covered by `compile_optionality_*` tests for `.ifpresent` lazy absence, named/mixed callback
binding, capture, shadowing, callback failure, UTF-8/CRLF spans, and atomic
results.

## Range construction and iterable semantics

Literal Range syntax is restricted to non-negative integer endpoints and keeps
the four endpoint shapes typed:

```text
A..B
..B
A..
..
```

Literal endpoints are syntax, not dynamically evaluated expressions. They are
converted to the signed core `IrRange` endpoint domain with checked conversion;
the v2.5.1 black-box boundary behavior for an oversized literal endpoint is
preserved as an open endpoint rather than a wrapped integer. The exact
`2147483647` endpoint remains representable, while `2147483648` does not wrap.

Dynamic `.range` constructs the same `IrValue::Range(IrRange)` value. Both
`from` and `to` are optional and accept normal positional, named, and valid
mixed argument binding:

```text
.range {A} {B}
.range from:{A} to:{B}
.range to:{B}
.range from:{A}
.range
```

Bounds are evaluated through the ordinary evaluator. A semantic `Number` is
converted with the observed upstream `Number.toInt()` behavior: truncation
toward zero, NaN to `0`, and clamping outside the signed `Int` domain to
`i32::MIN` or `i32::MAX`. Non-numeric bounds and invalid argument shapes fail
with one source-backed evaluator diagnostic; child failures are propagated
without an additional Range diagnostic.

The shared `coerce_iterable` path is used by `.foreach`, `.size`, `.first`,
`.last`, and `.getat`:

- closed `A..B` ranges iterate inclusively from `A` through `B`;
- descending `A..B` ranges are empty;
- left-open `..B` ranges use `1..B` and are empty when `B < 1`;
- right-open `A..` ranges remain valid typed values but standard Iterable
  consumption fails as endless; and
- fully-open `..` ranges remain valid typed values but standard Iterable
  consumption also fails as endless.

No arbitrary finite upper bound is introduced. Signed cardinality is checked
before `usize` conversion and fallible reservation, and a failed materialization
publishes no partial Collection or document output. Range values remain
backend-neutral; Typst does not interpret Range semantics.

`Implemented` rows are current claims only at their stated compatibility level
and are covered by the listed unit/golden/conformance evidence (see
[Conformance Evidence](#conformance-evidence)). `Planned` means the behavior is
not implemented yet, in whole or in part. It must not be assumed to work, and
its absence is tracked compatibility debt against the complete target. The
cross-audit status and dependency order are canonical in
[`RECONCILIATION.md`](RECONCILIATION.md).

## Conformance Evidence

Each supported/implemented row is backed by evidence at its stated layer. The
17-case independent corpus is intentionally bounded and does not imply that
every public surface is output-equivalent. The table maps the supported
features to the test(s) that verify them;
Quarkdown grammar evidence lives in
`crates/arkst-quarkdown/src/lib.rs`, while frontend integration evidence
lives in `crates/arkst-markdown/src/parser.rs` and its integration tests. A
single test may cover multiple features. This table is the
implementation-evidence counterpart of the upstream provenance recorded in
`SPEC_SOURCES.md`; the two are kept separate on purpose.

| Feature                         | Evidence (unit tests) |
|---------------------------------|------------------------|
| Dot-prefixed call               | `arkst-quarkdown/src/lib.rs::empty_and_plain_text_are_not_calls`, `call_and_named_identifiers_share_the_pinned_scanner`, `parses_normal_call_names_and_spans`, `arkst-markdown/tests/call_grammar_audit.rs::audit_aligns_call_boundaries_across_utf8_crlf_and_modes` |
| Implicit positional refs        | `arkst-quarkdown/src/lib.rs::parses_implicit_positional_references_and_boundaries`, `numeric_identifiers_share_the_argument_grammar`, `braced_implicit_reference_is_not_classified_as_a_decimal`; `arkst-core/src/lib.rs::compile_implicit_lambda_parameters_use_the_shared_callable_path`, `compile_implicit_parameters_preserve_typed_values`, `compile_implicit_parameter_content_keeps_markdown_structure`, `compile_implicit_lambda_scopes_are_nested_and_reusable`, `compile_implicit_parameter_missing_and_zero_argument_are_diagnostics`, `compile_implicit_parameter_diagnostic_preserves_utf8_and_crlf_span` |
| Positional arguments            | `arkst-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments`, `arkst-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification` |
| Named arguments                 | `arkst-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments`, `call_and_named_identifiers_share_the_pinned_scanner`; `arkst-markdown/tests/call_grammar_audit.rs::audit_aligns_named_argument_identifier_lexing_and_spans`, `audit_requires_adjacent_named_argument_delimiters_and_preserves_source` |
| Mixed positional/named          | `arkst-quarkdown/src/lib.rs::parses_positional_named_and_mixed_arguments`; `arkst-markdown/tests/call_grammar_audit.rs::audit_preserves_ordered_mixed_arguments_until_binder_validation`, `audit_aligns_named_argument_identifier_lexing_and_spans` |
| Escaped call/argument delimiters | `arkst-markdown/tests/call_grammar_audit.rs::audit_records_current_escaped_delimiter_gap` |
| Indented body argument          | `arkst-markdown/src/parser.rs::quarkdown_body_uses_first_body_line_indent_not_fixed_width`, `quarkdown_body_rejects_one_space`, `quarkdown_body_tab_preserves_text_and_utf8_spans`, `quarkdown_body_dedent_terminates_body_and_shallower_lines_are_not_absorbed`, `quarkdown_body_preserves_nested_markdown`, `quarkdown_body_preserves_nested_quarkdown_blocks`, `quarkdown_body_is_container_relative_in_lists_and_blockquotes`, `quarkdown_body_blank_lines_preserve_body_lifecycle` |
| Nested calls                    | `arkst-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification`, `arkst-markdown/src/parser.rs::nested_content_calls_keep_prefix_suffix_and_original_spans`, `arkst-markdown/tests/call_grammar_audit.rs::audit_preserves_nested_tight_call_wrapper_inside_content_argument`, `audit_preserves_nested_tight_utf8_and_crlf_provenance` |
| Inline (mid-paragraph) call     | `arkst-markdown/src/parser.rs::nested_content_calls_keep_prefix_suffix_and_original_spans`, `arkst-markdown/tests/call_grammar_audit.rs::audit_preserves_nested_tight_call_wrapper_inside_content_argument` |
| Tight-call boundaries           | `arkst-quarkdown/src/lib.rs::tight_word_adjacency_and_symbol_boundaries_are_explicit`, `arkst-quarkdown/src/lib.rs::parses_implicit_positional_references_and_boundaries`, `arkst-markdown/tests/call_grammar_audit.rs::audit_preserves_nested_tight_call_wrapper_inside_content_argument`, `audit_preserves_nested_tight_utf8_and_crlf_provenance` |
| Malformed-call diagnostics      | `arkst-quarkdown/src/lib.rs::rejects_malformed_arguments_and_preserves_unmatched_named_candidates`, `arkst-markdown/src/parser.rs::malformed_root_block_reports_argument_span`, `arkst-markdown/src/parser.rs::malformed_inline_call_preserves_full_source_offset` |
| v2.5.1 link parentheses         | `arkst-markdown/tests/quarkdown_v2_5_1.rs::qd251_links_accept_balanced_escaped_and_nested_parentheses`, `qd251_unbalanced_plain_destination_stays_literal`, `qd251_trailing_parenthesis_and_surrounding_text_are_not_swallowed`, `qd251_links_preserve_utf8_and_crlf_source_boundaries`, `qd251_link_boundary_is_identical_in_md_qd_and_qd_body_modes`, `qd251_link_correction_empty_destinations_have_complete_spans`, `qd251_link_correction_preserves_angle_and_title_forms`, `qd251_link_correction_preserves_multiline_title_span`, `qd251_link_correction_preserves_autolink_backslashes_and_email_semantics`, `qd251_link_correction_preserves_reference_and_image_destinations`, `qd251_link_correction_preserves_utf8_and_crlf_edge_spans` |
| v2.5.1 deep four-space lists   | `arkst-markdown/tests/quarkdown_v2_5_1.rs::qd251_deep_four_space_lists_have_exact_depth_in_md_and_qd`, `qd251_deep_list_preserves_siblings_dedent_and_following_content`, `qd251_nested_paragraph_and_list_content_remain_in_their_items`, `qd251_deep_lists_preserve_utf8_and_crlf_spans`, `qd251_qd_body_uses_dynamic_indent_before_markdown_list_parsing` |
| M2 blockquotes / strikethrough / task lists / tables | `arkst-markdown/src/parser.rs::preserved_markdown_structures_keep_nested_semantics_and_source_spans`, `arkst-engine/src/ast_to_ir.rs::convert_structures_preserves_task_table_and_nested_spans`, `arkst-engine/src/evaluator.rs::structures_recurse_through_evaluator_without_losing_semantics`, `arkst-typst/src/lowering.rs::lower_structured_markdown_nodes_preserves_semantics_and_source_map`, `arkst-typst/tests/backend_integration.rs::integration_markdown_structures_compile_to_valid_pdf` |
| v2.5.1 call syntax slice | `arkst-quarkdown/src/lib.rs::parses_multiline_nested_arguments_with_original_spans`, `parses_line_continuations_without_fixed_indentation`, `parses_chains_as_source_backed_segments_without_rewriting`, `parses_tight_calls_and_preserves_inner_provenance`, `rejects_malformed_chains_deterministically`; `arkst-markdown/src/parser.rs::qd_multiline_arguments_and_continuations_keep_header_body_boundary`, `qd_inline_continuation_and_tight_calls_preserve_text_and_spans`; `arkst-engine/src/ast_to_ir.rs::preserve_call_chain_segments_and_provenance_in_ir`, `arkst-core/src/lib.rs::compile_evaluates_block_and_inline_chain_value_flow`, `compile_evaluates_chain_inside_a_content_argument`, `compile_chain_and_nested_call_are_semantically_equivalent`, `compile_variable_values_keep_types_across_chain_and_nested_forms`, `compile_numeric_variable_reassignment_preserves_numeric_value_context`, `compile_chain_and_ordinary_conditional_are_equally_lazy`, `compile_reports_unimplemented_chain_callees_with_specific_spans`, `compile_reports_chain_failures_in_inline_and_content_paths`; `arkst-engine/src/evaluator.rs::nested_call_and_chain_share_the_same_value_context`, `nested_and_chained_case_transforms_share_dynamic_scalar_adaptation`, `variable_values_remain_semantic_through_nested_and_chained_calls`, `chain_value_flow_is_left_to_right_and_injects_first`, `chain_preserves_explicit_positional_arguments_after_previous_value`, `chain_keeps_named_arguments_named_while_injecting_previous_value`, `false_final_conditional_chain_does_not_evaluate_its_body`, `false_final_inline_conditional_chain_does_not_evaluate_its_body`, `child_scope_inherits_parent_and_isolates_local_bindings`; `arkst-cli/src/commands.rs::unimplemented_chain_callee_fails_before_typst_or_pdf_output`; `arkst-typst/tests/backend_integration.rs::integration_chain_evaluation_reaches_typst_and_pdf`; `fixtures/markdown/quarkdown_v251_syntax.qd` syntax/provenance fixture |
| Conditionals                   | `evaluator.rs::if_true_keeps_block_body`, `evaluator.rs::if_false_drops_block_body`, `evaluator.rs::ifnot_true_drops_and_ifnot_false_keeps`, `evaluator.rs::boolean_identifiers_yes_no_true_false_case_insensitive`, `evaluator.rs::missing_condition_reports_e3001_and_drops`, `evaluator.rs::unresolvable_condition_reports_diagnostic`, `evaluator.rs::nested_if_inside_block_body_is_evaluated`, `evaluator.rs::content_value_second_argument_replaces_call`, `evaluator.rs::scalar_second_argument_becomes_text`, `evaluator.rs::inline_if_replaces_call_with_inline_body_or_content`, `evaluator.rs::inline_if_false_drops_call`, `evaluator.rs::inline_call_scalar_second_argument_becomes_text`, `evaluator.rs::non_conditional_calls_are_preserved_with_evaluated_bodies`, `evaluator.rs::named_condition_argument_works`, `evaluator.rs::named_condition_false_drops_body`, `evaluator.rs::named_condition_ifnot_inverts`, `evaluator.rs::named_condition_identifier_yes_no`, `evaluator.rs::named_body_argument_works`, `evaluator.rs::named_body_scalar_argument_works`, `evaluator.rs::block_body_priority_over_named_body`, `evaluator.rs::inline_named_condition_works`, `evaluator.rs::inline_named_body_works`, `evaluator.rs::named_condition_unresolvable_reports_e3001`, `lib.rs::compile_evaluates_if_true`, `lib.rs::compile_evaluates_if_false`, `lib.rs::compile_evaluates_ifnot`, `lib.rs::compile_evaluates_nested_if`, `lib.rs::compile_reports_e3001_for_unresolvable_condition`, `lib.rs::compile_evaluates_named_condition_true`, `lib.rs::compile_evaluates_named_condition_false`, `lib.rs::compile_evaluates_named_condition_yes_no`, `lib.rs::compile_evaluates_named_body`, `lib.rs::compile_evaluates_named_condition_and_body`, `lib.rs::compile_inline_named_condition`, `typst::conditional_evaluation_before_lowering` |
| String/text operations | `arkst-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification`; `arkst-engine/src/builtins.rs::tests::string_surface_is_registered_and_returns_typed_values`, `string_operations_bind_named_arguments_and_defaults`, `string_case_and_empty_operations_cover_unicode_and_boundaries`, `string_operations_reject_unsupported_values_and_invalid_bindings`, `plaintext_projects_evaluated_inline_structure`, `plaintext_rejects_reparse_and_unsupported_values`, `plaintext_reuses_single_content_argument_binding`; `arkst-core/src/lib.rs::compile_v251_string_scalar_fixture_preserves_typed_value_flow`, `compile_v251_plaintext_fixture_projects_evaluated_inline_content`, `compile_plaintext_rejects_unsupported_values_atomically`; `arkst-test-support/src/lib.rs::tests::quarkdown_conformance_corpus_obeys_declared_levels`; `fixtures/quarkdown-conformance/cases/string-scalar-family/input.qd`, `fixtures/quarkdown-conformance/cases/plaintext-family/input.qd`, and their `expected/ir.json` goldens | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, and `.startswith` preserve typed evaluator results, share positional/named binding and scalar string adaptation, support ordinary/nested/chained forms, and fail closed for unsupported values. `.plaintext` projects already-parsed inline IR after nested evaluation: text, code, emphasis, strong, strikethrough, and link labels recurse; soft breaks emit a newline; hard breaks and images emit nothing; empty content emits an empty string. Unresolved calls and unsupported structured values fail closed with source-backed `E3001`. Dynamic String → InlineMarkdownContent is parsed only at the explicit `.plaintext` target; generic String → Markdown conversion remains deferred. | Implemented (bounded v2.5.1 slice) |
| Logical/comparison predicates | `arkst-engine/src/builtins.rs::tests::logical_surface_is_registered_and_evaluates_typed_results`, `equality_preserves_types_and_uses_upstream_plain_text_fallback`, `logical_builtins_reject_invalid_values_and_duplicate_bindings`; `arkst-core/src/lib.rs::compile_logical_comparisons_drive_conditionals_and_nested_calls`, `compile_logical_comparisons_work_in_user_functions_and_chains`, `compile_logical_comparison_failure_is_atomic_and_source_backed`, `compile_logical_comparison_execution_is_deterministic_for_utf8_crlf`; `arkst-markdown/tests/quarkdown_v2_5_1.rs::qd251_logical_comparison_expression_remains_structural_and_source_backed`; `arkst-typst/tests/backend_integration.rs::integration_logical_comparison_evaluation_reaches_typst_and_pdf` | `.islower`, `.isgreater`, `.equals`, and `.not` return typed booleans, preserve the value boundary, support lazy conditional use, and fail closed on invalid input | Implemented (bounded v2.5.1 slice) |
| User-defined functions         | `arkst-quarkdown/src/lib.rs::parses_contextual_lambda_headers_with_exact_spans`, `lambda_header_parser_is_contextual_and_rejects_malformed_headers`; `arkst-markdown/src/parser.rs::function_body_uses_contextual_source_backed_lambda_header`, `ordinary_non_lambda_body_with_colon_is_not_stripped`; `arkst-core/src/lib.rs::compile_user_functions_support_zero_and_required_parameters`, `compile_implicit_lambda_parameters_use_the_shared_callable_path`, `compile_implicit_parameters_preserve_typed_values`, `compile_implicit_lambda_scopes_are_nested_and_reusable`, `compile_user_functions_keep_scalar_values_for_nested_and_chain_calls`, `compile_user_function_rich_and_block_results_keep_markdown_structure`, `compile_user_functions_use_source_order_and_override_builtins`, `compile_user_functions_bind_block_last_and_isolate_child_scope`, `compile_user_function_argument_failures_are_single_and_body_is_not_run`, `compile_user_function_no_value_and_failed_nested_calls_keep_original_diagnostic`, `compile_optional_user_parameters_bind_missing_positional_and_named_values`, `compile_optional_final_parameter_accepts_missing_or_block_content_and_keeps_collision`, `optional_parameter_spans_survive_utf8_and_crlf_frontend_to_ir_conversion` |
| Scoped `.let`                | `arkst-markdown/src/parser.rs::let_explicit_lambda_header_is_source_backed_and_stripped`, `let_implicit_lambda_body_keeps_implicit_reference`, `let_header_utf8_span_is_exact_for_crlf_source`, `let_nested_container_span_keeps_original_body_ranges`; `arkst-engine/src/ast_to_ir.rs::let_lambda_metadata_survives_ast_to_ir_with_original_spans`, `let_implicit_lambda_metadata_is_absent_in_ir`; `arkst-engine/src/evaluator.rs::let_explicit_parameter_returns_scalar`, `let_implicit_parameter_returns_scalar`, `let_shadows_parent_and_local_variables_do_not_leak`, `nested_let_uses_nearest_implicit_scope`; `arkst-core/src/lib.rs::compile_let_supports_explicit_and_implicit_block_lambdas`, `compile_let_isolates_local_variables_and_functions` |
| Iteration                    | `arkst-quarkdown/src/lib.rs::parses_typed_ranges_without_confusing_numbers_or_references`; `arkst-markdown/src/parser.rs::iteration_lambda_headers_are_contextual_and_source_backed`, `iteration_inline_body_preserves_contextual_metadata_without_eager_lambda_coercion`, `implicit_iteration_body_keeps_nested_named_arguments_in_the_body`; `arkst-engine/src/ast_to_ir.rs::range_survives_ast_to_ir_as_a_typed_source_backed_value`, `literal_range_endpoint_conversion_is_checked_at_the_signed_boundary`; `arkst-ir/src/lib.rs::range_and_nested_collection_roundtrip_serde`, `pair_and_dictionary_roundtrip_serde_preserves_recursive_values`; `arkst-engine/src/evaluator.rs::dynamic_range_returns_typed_signed_truncated_endpoints`, `dynamic_range_number_conversion_matches_upstream_edges`, `range_materialization_handles_signed_and_left_open_bounds_once`, `pair_evaluation_is_typed_recursive_and_atomic_on_child_failure`, `dictionary_iteration_reuses_pair_items_and_explicit_destructuring`, `pair_destructuring_rejects_non_pair_items_without_coercion`, `foreach_reassignment_updates_existing_caller_variable_but_new_locals_stay_local`, `failed_callable_reassignment_is_atomic_and_keeps_the_inner_span`; `arkst-core/src/lib.rs::compile_foreach_closed_range_is_inclusive_and_preserves_numbers`, `compile_dynamic_range_converges_with_literal_and_supports_signed_bounds`, `compile_dynamic_range_supports_nested_bounds_and_typed_interoperability`, `compile_foreach_returns_a_typed_collection_that_can_be_stored_and_consumed`, `compile_foreach_reads_parent_values_and_functions_with_isolated_children`, `compile_foreach_adapts_only_list_values_and_preserves_nested_collections`, `compile_foreach_scopes_implicit_parameters_at_the_nearest_boundary`, `compile_dictionary_foreach_destructures_ordered_pairs`, `compile_dictionary_duplicate_keys_are_last_write_wins_in_first_slot`, `compile_dictionary_entry_failure_is_atomic_and_stops_before_output`, `compile_dictionary_implicit_scope_keeps_the_pair_typed`, `compile_dictionary_explicit_scope_masks_implicit_positional_references`, `compile_dictionary_destructuring_masks_and_restores_parent_bindings`, `compile_nested_dictionary_destructuring_restores_outer_scope`, `compile_pair_is_a_typed_recursive_value_at_the_output_boundary`, `compile_repeat_is_one_based_and_uses_the_shared_collection_result`, `compile_repeat_zero_and_descending_ranges_are_empty_per_upstream_evidence`, `compile_iteration_accepts_left_open_and_rejects_endless_ranges`, `compile_dynamic_range_rejects_invalid_shapes_and_preserves_atomic_failures`, `compile_dynamic_range_diagnostics_keep_utf8_crlf_and_nested_bound_spans`, `compile_iteration_body_no_value_and_failure_are_single_diagnostics`, `compile_inline_foreach_and_repeat_use_the_shared_callable_path`, `compile_inline_foreach_preserves_pair_destructuring`, `compile_inline_foreach_reuses_materialization_budget`, `compile_inline_foreach_preserves_owner_reassignment_and_parameter_shadowing`, `compile_inline_foreach_rhs_sees_outer_owner_with_different_parameter_name`, `compile_inline_foreach_keeps_new_variables_invocation_local`, `compile_source_defined_foreach_and_repeat_shadow_native_direct_and_chain`, `compile_inline_foreach_failure_is_atomic_and_source_backed` | Semantically supported for typed literal/dynamic values, signed endpoint conversion, closed and left-open iterable ranges, descending-empty behavior, ordered list adaptation, ordered dictionary entries, Pair destructuring, block and inline explicit/implicit callable bodies, typed collection results, parent visibility, semantic-owner writeback, source-defined native shadowing, child isolation, materialization/depth limits, and failure atomicity | Implemented (evidenced block/inline slice; endless right-open/fully-open consumption, generalized patterns, and generalized inline component/callback bodies deferred) |
| Row/column/grid (Stacked layout) | `crates/arkst-core/tests/quarkdown_stacked_layout.rs::row_column_and_grid_defaults_are_distinct_and_typed`, `row_column_and_grid_bind_typed_arguments_and_preserve_children`, `alignments_are_closed_case_insensitive_domains_without_underscore_normalization`, `integer_boundary_is_integral_positive_and_origin_aware`, `duplicate_unknown_missing_and_invalid_arguments_fail_before_body`, `body_is_lazy_and_nested_failures_do_not_publish_outer_components`, `component_values_compose_in_functions_and_custom_row_shadows_native_row`; `arkst-ir/src/lib.rs::stacked_components_roundtrip_deterministically_for_row_column_and_grid`, `grid_layout_rejects_zero_columns_during_deserialization`; `crates/arkst-typst-subprocess/tests/backend_integration.rs::integration_stacked_layouts_lower_to_valid_typst_and_pdf` | Block-only native consumers preserve typed Row, Column, and positive-column Grid components with typed alignments/gaps, structured children, source provenance, validate-before-lazy-body behavior, deterministic serde, and real Typst/PDF lowering | Implemented (bounded Stacked layout slice; complete component/style/layout compatibility remains partial) |
| Collection and Iterable operations | `arkst-engine/src/evaluator.rs::collection_second_and_third_share_one_based_iterable_access`, `collection_distinct_and_groupvalues_are_stable_and_typed`, `collection_reversed_uses_the_shared_materialized_sequence`, `collection_sumall_and_average_follow_as_double_and_kotlin_average`, `collection_access_reuses_failure_outcomes_and_checks_length_conversion`; `arkst-core/src/lib.rs::compile_collection_api_parity_uses_frontend_lists_and_shared_iterables`, `compile_collection_access_keeps_pair_dictionary_and_range_values_typed`, `compile_collection_access_diagnostics_keep_utf8_and_crlf_source_spans` | `.size`, `.first`, `.second`, `.third`, `.last`, `.getat`, `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, and `.groupvalues` over the shared typed `Collection`, Pair, ordered Dictionary entries, closed/left-open Range, and Markdown-list adaptation path; recursive typed results, stable ordering, aggregation conversion, and atomic failures | Implemented (evidenced v2.5.1 slice) |
| Generic callable, native transforms, and extensions | `arkst-quarkdown/src/lib.rs::parses_marked_inline_lambdas_without_rewriting_source`, `parses_marked_inline_implicit_lambdas`; `arkst-markdown/src/parser.rs::marked_inline_lambda_is_structural_and_source_backed`, `transform_callback_lambda_uses_contextual_unmarked_form`; `arkst-engine/src/evaluator.rs::collection_transforms_share_typed_iterable_and_callable_paths`, `transforms_support_pair_dictionary_and_nested_typed_values`, `sorted_supports_typed_keys_and_fails_closed_for_unsupported_keys`, `transform_failures_are_atomic_and_predicates_are_boolean_only`, `first_class_callable_captures_definition_values_and_checks_arity`; `arkst-core/src/lib.rs::compile_collection_transforms_through_frontend_and_first_class_lambda_values` | First-class typed callable values, explicit/implicit callback binding, lexical capture, shared invocation and iterable adaptation, typed `.foreach`/`.sorted` results, and retained typed `.map`/`.filter` extensions. `.foreach` and `.sorted` are native evidence; `.map`/`.filter` are excluded from upstream v2.5.1 conformance counts | Implemented (bounded callable/native-transform evidence; extensions retained) |
| Optional parameter values      | `arkst-ir/src/lib.rs::none_uses_the_stable_externally_tagged_serde_variant`, `arkst-core/src/lib.rs::compile_optional_parameters_support_otherwise_and_preserve_value_types`, `compile_optional_none_is_distinct_from_no_value`, `compile_optional_none_can_be_stored_locally_without_parent_scope_leak`, `compile_optional_none_direct_output_materializes_as_text`, `compile_isnone_returns_a_semantic_boolean_for_optional_values` |
| Variables                      | `evaluator.rs::var_scalar_definition_and_reference`, `evaluator.rs::var_boolean_reference_in_conditional`, `evaluator.rs::var_false_boolean_drops_conditional`, `evaluator.rs::var_ifnot_with_variable`, `evaluator.rs::var_explicit_reassignment`, `evaluator.rs::var_variable_name_reassignment`, `evaluator.rs::var_reassignment_produces_no_output`, `evaluator.rs::var_inline_use`, `evaluator.rs::var_block_variable`, `evaluator.rs::var_conditional_declaration_execution_order`, `evaluator.rs::var_unknown_call_preserved`, `evaluator.rs::var_malformed_declaration_reports_e3002`, `evaluator.rs::var_nested_evaluation_in_block_variable`, `evaluator.rs::var_evaluation_immutable_and_deterministic`, `lib.rs::compile_variable_declaration_and_reference`, `lib.rs::compile_variable_boolean_in_conditional`, `lib.rs::compile_variable_false_conditional`, `lib.rs::compile_variable_ifnot`, `lib.rs::compile_variable_explicit_reassignment`, `lib.rs::compile_variable_name_reassignment`, `lib.rs::compile_variable_inline_use`, `lib.rs::compile_variable_block_variable`, `lib.rs::compile_variable_conditional_declaration`, `lib.rs::compile_variable_unknown_preserved`, `lib.rs::compile_variable_malformed_reports_e3002`, `lib.rs::compile_variable_nested_in_block`, `lib.rs::compile_variable_immutable_and_deterministic` |

### v2.5.1 syntax-gap evidence

The v2.5.1 public function-call syntax review is backed by independently
authored fixtures in the grammar and frontend tests. The evidence covers the
#157 shared identifier and adjacent named-delimiter contract, numeric
identifier/reference boundaries, multiline nested positional/named arguments,
source-ordered mixed arguments, line continuation with arbitrary leading
indentation, parser-preserved `::` chains, tight brace-wrapped calls, normal
boundary regressions, malformed recovery, UTF-8, CRLF, `.md`/`.qd` isolation,
and the existing dynamic body-indentation lifecycle. Malformed recovery,
separator placement, and escaped delimiters remain their separately owned
gaps. The AST-to-IR handoff retains the ordered argument representation needed
by the engine binder. The shared binder's `E3003` positional-after-named
diagnostic is semantic evidence owned by #165, not a grammar/frontend claim.

The complete grammar/frontend re-audit, including its conservative status
matrix and bounded follow-up issues, is recorded in
[`CALL_GRAMMAR_AUDIT.md`](CALL_GRAMMAR_AUDIT.md).

The syntax adapter preserves the head, each chain segment, each name span,
argument spans, and the complete source span without synthetic reparsing. The
evaluator consumes those segments structurally and applies the documented
left-to-right value-flow transformation for the four evidenced builtin
callees. Their documented nested-call equivalents share the same evaluator
invocation contract and are covered by paired semantic and generated-Typst
tests. Successful terminal outputless calls (such as variable reassignment)
remain legal, but a no-value result in a nested value-required argument or
non-final chain segment reports source-backed `E3001`; an already-failed child
propagates its original diagnostic without a duplicate no-value error.
Value-context type preservation, the bounded scalar conversion slice, lazy
conditional bodies, provenance, failure, and Typst/PDF tests support this
slice only; complete `DynamicValue` and general programmable document
compatibility are not claimed here.

The primary public source for this slice is the Quarkdown wiki's [Syntax of a
function call](https://quarkdown.com/wiki/syntax-of-a-function-call/) page,
which documents multiline arguments, line continuation, chaining, and tight
function calls. Pinned v2.5.1 lexer/grammar/refiner records are listed in
`SPEC_SOURCES.md` as additional lexical and ownership evidence. Fixtures remain
independently authored; no upstream implementation code, test, or fixture was
copied or translated.

The String/text evidence row includes the currently observable bounded
implementation. The canonical #151 classification is now
`SUPPORTED_SEMANTICS` for `.capitalize` and `.startswith`: Kotlin
`Char.titlecase()` and character-wise case-insensitive matching are reproduced
from the pinned Eclipse Temurin `25.0.4.1+1` oracle (Unicode 16.0) without
normalization or locale/global state. The active contract and regeneration
fingerprints are recorded in [`reference-jvm.md`](reference-jvm.md).
Independent evidence is in `crates/arkst-core/tests/stdlib_builtin_audit.rs`;
#172 closes this bounded string-semantics slice. The row remains partially
compatible overall because `.plaintext` and broader DynamicValue/output
contracts remain bounded elsewhere.

## Compatibility Levels

### User-defined function evidence

This slice is grounded in the public Quarkdown documentation for
[declaring functions](https://quarkdown.com/wiki/declaring-functions/),
[lambdas](https://quarkdown.com/wiki/lambda/),
[function-call syntax](https://quarkdown.com/wiki/syntax-of-a-function-call/),
[variables](https://quarkdown.com/wiki/variables/), and
[typing](https://quarkdown.com/wiki/typing/). Those pages document
`.function`, the `to from:` and `to from?:` parameter headers, positional and named calls,
block content as the final parameter, source-order redeclaration, and the
absence of an explicit return statement (reviewed 2026-08-15). Arkst
independently represents
the header and parameter spans, binds required parameters in a child scope,
and preserves scalar or structured-content results through the shared value
evaluator.

The v2.5.1 [lambda reference](https://quarkdown.com/wiki/lambda/) explicitly
defines a headerless lambda's positional parameters as `.1`, `.2`, `.3`, and
so on, and states that lambdas fork nested scopes. The official
[v2.5.1 release](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1)
was also probed as a black box: an out-of-range implicit reference and a
reference in a zero-argument lambda fail as unresolved references. Arkst
preserves that observable failure class as a deterministic source-backed
`E3003`; it never substitutes `None` or evaluator `NoValue`.

The claim is deliberately limited to required and optional explicit parameters,
headerless implicit positional references, the tested scalar/content shapes,
and the small `.otherwise`/`.isnone` builtin surface. Scoped `.let` is
supported for block-form explicit one-parameter and headerless `.1` lambdas,
including nested lexical scopes, parent lookup, child isolation, and semantic
result propagation. First-class `@lambda` values and contextual transform
callbacks now use the same invocation machinery. Components and complete
DynamicValue compatibility remain compatibility debt.

### Typed iteration evidence

This first iteration slice is grounded in the public
[Loops](https://quarkdown.com/wiki/loops/),
[Iterable](https://quarkdown.com/wiki/iterable/),
[Range](https://quarkdown.com/wiki/range/),
[Lambda](https://quarkdown.com/wiki/lambda/), and
[foreach API](https://quarkdown.com/docs/quarkdown-stdlib/com.quarkdown.stdlib.module.Flow/foreach.html)
references. They establish that `.foreach` maps an iterable through a scoped
lambda and returns an ordered collection, `.repeat` is the one-based
`.foreach {1..times}` shorthand, Markdown lists are iterable values, and Range
syntax preserves open endpoints.

The official v2.5.1 implementation was consulted only as public behavioral
evidence, never copied or translated: [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt)
delegates `.repeat` to `forEach(Range(1, times), body)`, and [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt)
iterates closed ranges inclusively, starts a left-open range at `1`, rejects a
right-open range as endless, and uses the host integer range behavior for
descending bounds. The public v2.5.1 [`FlowTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt)
also covers `..4` and rejects `1..`. [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
documents dynamic `.range` and floating-point truncation. Arkst follows this
policy through one shared iterable adaptation path.

Supported here are typed literal and dynamic Range values, signed dynamic
endpoints, recursive ordered Collections, closed and left-open Range iteration,
descending-empty behavior, Markdown ordered/unordered list adaptation at the
iterable boundary, block-form `.foreach` and `.repeat`, inline contextual
`.foreach {iterable} {item: body}` and `.foreach {iterable} {body}`, inline
`.repeat {count} {item: body}` and `.repeat {count} {body}`, explicit and
implicit parameters, Pair destructuring, typed mapped results, parent lookup,
fresh per-iteration child scopes, owner writeback, failure atomicity, and the
R10 materialization/depth limits. The inline forms preserve a source-backed
`Value::InlineBody`/`IrValue::InlineBody` carrier until native `.foreach` or
`.repeat` resolution and then reuse the existing `IrCallable` evaluator path;
source-defined `foreach`/`repeat` bindings retain precedence in direct and
chained calls. The Collection slice also covers `.second`, `.third`, `.sumall`,
`.average`, `.distinct`, `.reversed`, and `.groupvalues` through that same
typed materialization path. `.foreach` and `.sorted` are native v2.5.1
evidence; the retained `.map`/`.filter` surface is explicitly a Arkst
extension and is excluded from conformance claims. Deferred are generalized or
nested destructuring, arbitrary comparator syntax, descending sorting,
table-specific collection operations, and generalized inline component or
callback bodies. Right-open and fully-open Range values are represented but are
rejected by the standard finite Iterable path as endless.

- **Unsupported:** Syntax may be parsed and preserved, but normal compilation
  produces an explicit `E8xxx` error diagnostic for the unsupported semantics
  (see `crates/arkst-compat/src/diagnostics.rs`)
- **Error:** Produces an explicit parse diagnostic (`E2xxx`) at the call site
- **Parsed:** Accepted syntactically; behavior may be undefined or rejected
- **Semantically supported:** Arkst semantics match documented behavior
- **Output-equivalent:** Typst output matches reference for tested inputs
- **Known divergence:** Deliberate behavioral difference with documented
  rationale

`Parsed` is parser acceptance only. It is not evidence of evaluator, IR, or
output compatibility; semantic claims require executable evaluation and
output-boundary evidence at the stated level.

Function calls are currently **Parsed** except for the evidenced semantic
surfaces below: `.name`, positional arguments `{arg}`, named arguments
`name:{arg}`, nested calls, and indented block bodies are parsed into the
Arkst AST/IR. Multiline braced arguments, line continuations, and tight
brace-wrapped calls are syntax-supported with source-backed spans. The
evidenced `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`,
`.negate`, `.sqrt`, `.truncate`, `.round`, `.iseven`, `.string`, `.concatenate`, `.uppercase`,
`.lowercase`, `.isempty`, `.isnotempty`,
`.islower`, `.isgreater`, `.equals`, and `.not` chain forms and their documented
nested-call equivalents are **Semantically supported** with strict
left-to-right value flow; an unimplemented chain callee reports a source-backed
`E3001` evaluation error. The string-family, comparison, and bounded scalar
conversion contracts are evidenced, not complete DynamicValue compatibility.
`.capitalize` and `.startswith(ignorecase:true)` are **Semantically supported**
at the bounded scalar boundary after #172: the engine uses Kotlin/JVM
semantics captured from the pinned Eclipse Temurin `25.0.4.1+1` runtime and
Unicode 16.0 full/simple mapping data. The
broader string family remains bounded because `.plaintext` and other
DynamicValue/output contracts are not promoted by this slice.
**User-defined functions are also semantically supported for the evidenced
slice**: headerless implicit and required/optional explicit-parameter
declarations, positional/named binding where applicable, block-last-parameter
binding, child scope, source-order redeclaration, builtin override, and
scalar/structured-content results. Missing optional parameters bind semantic
`None`, which is consumed by `.otherwise` and `.isnone` without string
conversion in value context. Headerless `.1`/`.2` references are 1-based,
invocation-local, and preserve typed `IrValue`s; missing indices produce a
source-backed `E3003` diagnostic. **Conditional evaluation (`.if` / `.ifnot`)
with boolean literals, variable references (`.if {.name}`), and the selected
logical/comparison expressions is implemented**. The comparison family is
documented in [`GAP_INVENTORY.md`](GAP_INVENTORY.md) and uses typed boolean
results rather than text reparsing.
Standalone lambda values, components, and complete programmable-document
compatibility remain unimplemented. Typed block iteration is limited to the
evidenced first slice above. A matrix row can
therefore represent only the evidenced
forms at its stated level; an input form that currently fails to parse (for
example with an `E2xxx` diagnostic) is a compatibility gap, not evidence of
support for that form. `Unsupported` is reserved for the explicit compatibility
diagnostic state.

### Logical and comparison evidence

The v2.5.1 public [`Logical.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt)
surface defines numeric `.islower` and `.isgreater` with `than` and optional
`orequals`, value `.equals` with `to`, and boolean `.not`. The public
[`Comparison.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Comparison.kt)
helper shows that equality first compares values and then compares a plain-text
projection for strings, numbers, and Markdown content. The v2.5.1 conditional
tests exercise `.islower` directly inside `.if`, including the lazy branch
behavior documented by the [conditional statements](https://quarkdown.com/wiki/conditional-statements/)
page.

Arkst implements this family as typed evaluator builtins. Numeric ordering
uses the upstream float comparison boundary and accepts the reviewed scalar
numeric text forms; `.equals` preserves typed equality and applies only the
documented plain-text fallback; `.not` accepts boolean values and boolean
literals. Invalid values, duplicate bindings, and unsupported bodies produce a
single source-backed `E3001`; the conditional body is not evaluated and no
partial result is published. The selected family is not a claim that all
DynamicValue conversions or other logical helpers are complete.

### Mathematical and numeric evidence

The v2.5.1 [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
source defines the arithmetic/unary functions, `.logn`, `.pi`, `.sin`, `.cos`,
and `.tan`, plus `.truncate(x, decimals: Int)` and `.round(x)` over `Number`
values. The transcendental boundaries are explicit: `.logn`, `.sin`, `.cos`,
and `.tan` call `x.toFloat()` before Kotlin's Float overload, while `.pi`
passes the binary64 `kotlin.math.PI` constant to `NumberValue`. The public
[`MathFunctionsTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt)
covers `.pi::truncate {2}`, zero trigonometry, `.cos {.pi}`, and
`.pi::multiply {2}::cos` with rendered results `3.14`, `1`, `0`, `0`, `-1`,
and `1`. The decimal slice has
the following observable boundaries:

- `.truncate` rejects negative `decimals` at runtime; fractional `decimals`
  fail the `Int` argument binding rather than being silently truncated.
- `decimals == 0` uses `x.toInt()`. Otherwise, non-integral `x` uses
  `x.toFloat() * 10.0.pow(decimals)` as Double arithmetic, `Double.toInt()`,
  and Float division by `multiplier.toFloat()` in that order.
- `.round` preserves an upstream Int, otherwise applies Kotlin
  `kotlin.math.round(x.toFloat())` (ties-to-even) and then `toInt()`.
- `NumberValue` normalizes integral Float values, including clamped
  non-finite conversions, to Int. This is why the final NaN/Infinity behavior
  must be checked after rounding or conversion, not from `round()` alone.
- The `decimals: Int` binder uses the same invocation-time DynamicValue Number
  conversion as other numeric targets: it parses dynamic text as Int first,
  then Float, and succeeds only when NumberValue normalization produces an
  integral Int. Dynamic `2` and `2.0` therefore succeed, dynamic `1.5` fails,
  and static StringValue `2` remains rejected.

The existing arithmetic path retains the v2.5.1 `toFloat()` boundary;
`.pow` and `.iseven` apply `Number.toInt()`, division-by-zero results clamp to
the upstream Int boundaries when integral, `0/0` and negative square roots
remain `NaN`, and remainder keeps signed floating behavior.

[`NumberValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NumberValue.kt),
[`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt),
and [`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt)
establish the invocation-time numeric and normalization boundaries. Arkst
uses the bounded conversion policy for concrete numeric consumers, including
the same DynamicValue Number conversion for `decimals` before its Int-only
NumberValue normalization check; it does not introduce a general DynamicValue
conversion framework. All numeric functions
use the existing argument binder, preserve `IrValue::Number` or
`IrValue::Boolean`, and reject
unsupported values, unknown/duplicate bindings, arity errors, and block bodies
without publishing partial nested output.

The transcendental implementation uses `libm` `0.2.16` with
`default-features = false`. Its pure-Rust binary64 software functions receive
the already-adapted Float value and are narrowed to Float afterward, matching
the Kotlin/JVM Float overload's `float -> double Math.* -> float` boundary
without depending on Rust `std` math, an OS libc/libm, or target-specific
intrinsics. The pinned version was compared with `0.2.14` and `0.2.15` on the
representative corpus; all three produced the same selected bits, and
`0.2.16` is retained for its current reviewed release and fixes. The helper
tests fix representative `to_bits()` values and separately cover `ln(0)`,
negative-domain NaN, infinities, and signed zero. At the evaluator boundary,
the existing `NumberValue(Float)` normalization remains authoritative: an
integral result becomes an Int, so signed zero becomes `0` and infinities clamp
to the Kotlin Int range; `.pi` does not pass through that Float helper.

The independent unit and integration evidence is:
`arkst-engine/src/builtins.rs::tests::decimal_numeric_surface_matches_upstream_boundaries`,
`arkst-core/src/lib.rs::tests::compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`,
`compile_numeric_decimal_forms_share_one_semantic_path`,
`compile_numeric_decimal_failure_is_atomic_and_source_backed`,
`arkst-test-support/src/lib.rs::tests::quarkdown_conformance_corpus_obeys_declared_levels`,
and `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`.
Transcendental evaluation is covered by
`arkst-engine/src/builtins.rs::tests::transcendental_numeric_surface_matches_upstream_boundaries`,
`deterministic_transcendental_math_has_stable_representative_bits`,
`arkst-core/src/lib.rs::tests::compile_v251_numeric_transcendental_fixture_preserves_typed_value_flow`,
`compile_numeric_transcendental_failure_is_atomic_and_source_backed`,
`arkst-test-support/src/lib.rs::tests::quarkdown_conformance_corpus_obeys_declared_levels`,
and `fixtures/quarkdown-conformance/cases/numeric-transcendental-family/input.qd`.
The arithmetic/unary regression remains covered by the existing numeric tests
and `numeric-arithmetic-family` fixture. `.map` and `.filter` remain Arkst
extensions and are not included in the upstream numeric family.

### String and text evidence

The v2.5.1 public [`Strings.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt)
surface defines scalar `.string`, `.concatenate`, case transforms,
emptiness predicates, and `.startswith`. Arkst implements the bounded
scalar paths through one explicit invocation-boundary adapter for
strings, identifiers, numbers, booleans, typed ranges, and bounded plain-text
content. The results remain typed `IrValue::String` or `IrValue::Boolean`, so
nested calls, chains, variable bindings, and lazy conditionals share the
ordinary evaluator path. This is the bounded `String` conversion surface;
collections, callables, `None`, and rich document values are not stringified.
`.capitalize` and `.startswith(ignorecase:true)` now satisfy their pinned
Temurin 25/Unicode 16 scalar semantics at the audited boundary: titlecase mapping preserves
the remainder of the string, and prefix matching uses Kotlin/JVM-style
character-wise comparison without whole-string case conversion or
normalization. Both rows are `SUPPORTED_SEMANTICS`; the broader string family
remains partial because `.plaintext` and other contracts still need evidence.

Quoted scalar input is classified by the existing Quarkdown grammar, which
removes only its outer quotes and preserves inner whitespace before the typed
IR boundary. `None`, collections, and rich structured content are rejected
instead of being stringified. `.plaintext` is a separate structural projection
of already-parsed inline content; it does not reuse the scalar adapter or the
private `.equals` plain-text fallback.

### Tight-call boundaries

A normal call requires a boundary before and after it: whitespace, a symbol
(including `-`), or the start/end of the line. A call directly adjacent to a
word character — any Unicode letter or digit, plus `_` — is not recognized and
the whole construct stays ordinary text. Examples:

- `.note {x}` is a call; `.note {x}B` and `한.note {x}` are not (both
  Unicode and ASCII letters count as word characters).
- `-.note` and `.note-` are valid calls: `-` is a symbol, not a word
  character.

The brace-wrapped form (`H{.text {2}}O`) lifts the boundary requirement. The
frontend accepts a complete wrapper, consumes the wrapper from output syntax,
and keeps both the wrapper and inner-call provenance. An incomplete wrapper
recovers as ordinary text.

### Existing public-language compatibility debt

Quarkdown has documented features represented in the v2.5.0/v2.5.1 evidence set that
Arkst has not implemented yet. They are listed in the Feature Matrix as
`Planned`, are **not** current compatibility claims, and remain compatibility
debt against the complete target. Standalone lambda values outside the
supported first-class/callback forms, layout semantics, unimplemented data
loading families, and other v2.5.0 built-ins remain additional gaps; generalized or nested
destructuring, arbitrary comparator syntax, descending sorting, and unrelated
collection operations remain deferred within the iteration slice. Right-open and
fully-open Range values are not globally unsupported: their representation is
supported, while standard Iterable consumption rejects them as endless. The
evidenced function row does not promote later consumer-specific Range surfaces.

## Specification Record Format

Each implemented compatibility feature records its public documentation
source, an independently authored input example, and the observed behavior.

```yaml
feature: dot-prefixed-call
specification_source: |
  Quarkdown wiki, "Syntax of a function call":
  https://quarkdown.com/wiki/syntax-of-a-function-call/ (v2.5.0 badge)
independently_authored_input: |
  .heading level:{1}
      Title
  .strong {bold text}
observed_reference_behavior: |
  Dot-prefixed names form function calls; each argument is wrapped in
  curly braces; named arguments use name:{value}; indented lines after
  a block call form its body. The current v2.5.0 documentation describes the
  same basic dot-prefixed, brace-argument model on which Arkst's existing
  parser subset is based.
arkst_behavior: |
  Parses dot calls, positional/named arguments, nested calls, and
  indented bodies into the shared DirectiveCall AST.
compatibility_level: Parsed
known_divergence: null
```

The `independently_authored_input` is written from the public syntax
specification only; it is not copied from Quarkdown sources, examples, or
tests (clean-room policy, see `docs/adr/0007-quarkdown-compatibility-scope-and-clean-room-process.md`).

## Provenance

The call grammar was derived from the public documentation *"Syntax of a
function call"* (wiki, badged `2.5.0`, accessed 2026-08-08). The current
v2.5.0 documentation describes the same basic dot-prefixed, brace-argument
model on which Arkst's currently evidenced parser behavior is based.
Arkst's previous
compatibility baseline was 0.9.x, but no claim is made that the upstream
grammar was verified to be identical across every version in between.
`SPEC_SOURCES.md` documents the source list, per-source version badges, and
accessed dates.

## Known Divergences

- The #157 lexical slice is now aligned with the pinned identifier,
  named-delimiter, numeric-reference, and ASCII/Unicode call-start evidence.
  Numeric identifiers use accepted-prefix parsing (`.1abc` leaves `abc` as
  remainder), and incomplete optional named candidates remain source without a
  fabricated diagnostic. Implicit-reference binding/evaluation remains
  #150-owned, and declaration name validity remains the evaluator-owned
  contract.
- Pinned v2.5.1 permits optional argument separators before the first argument
  and before `::`, and consumes a trailing continuation without an argument;
  current Arkst does not preserve those forms and reports `E2004` in some
  paths; see #164.
- Escaped call/argument delimiters are not fully aligned with pinned
  `GrammarUtils.unescapedMatch()` and balanced-brace behavior; the current
  parser can truncate arguments or report `E2003`, while the escaped call
  introducer remains literal; see #162.
- Arkst's grammar/frontend and IR preserve positional-after-named in the
  source-ordered argument shape without parser `E2001`. The shared engine
  binder rejects that invalid shape with source-backed `E3003`, including the
  offending positional and preceding named spans. #163 owns the representation;
  #165 owns bounded semantic binding. Complete value/conversion compatibility
  remains partial; see #163 and #165.
- Malformed inline recovery currently drops following source text; see #159.
- Supported Markdown inline structure inside static Quarkdown content
  arguments is retained through the Rushdown frontend with original-source
  spans; this is parser/frontend evidence only. Dynamic String/content
  conversion remains a separate #166 boundary, and broader content/output
  support remains under #154.
- Scope note: the matrix is an evidence register, not a permanent language
  boundary. Rows marked **Planned** are *not* implemented and must not be
  claimed; any public Quarkdown behavior absent from the matrix is still a gap
  to investigate against the complete target.
- **Block variable evaluation timing:** Arkst evaluates block variable
  content at declaration time (source order). The cited Quarkdown public
  documentation does not explicitly specify evaluation timing for stored
  block content. This behavior may be refined if upstream semantics are
  clarified. See `docs/SYNTAX.md` for details.

## Upstream Evolution

Arkst tracks two distinct Quarkdown versions:

| Concept | Description | Authority |
|---------|-------------|-----------|
| **Tracked upstream target** | The latest stable Quarkdown release. It automatically becomes the release Arkst must investigate and adapt toward. | Stable-release observer |
| **Verified compatibility baseline** | The release for which permitted evidence, independent fixtures, implementation, regression/conformance tests, and known-divergence records are complete. The existing `supported_baseline` manifest field names this value. | Human-reviewed promotion PR |

The observer (`.github/workflows/upstream-quarkdown.yml`) runs daily, obtains
the latest stable release, and compares it with the verified baseline:

- If they match → no target/baseline lag is detected.
- If they differ → `drift` status and a deduplicated adaptation issue.

A new stable release is never an optional product-adoption question. The issue
asks what changed and what work is required to restore verified compatibility.
The current observer is only the early foundation of the intended pipeline:

```text
release detection
    -> permitted public evidence and release-note delta
    -> structured impact report
    -> independently authored conformance updates
    -> implementation/adaptation PR
    -> conformance and regression verification
    -> review gate
    -> verified baseline promotion
```

The observer does not yet implement this complete pipeline. Future automation
may prepare evidence, fixtures, impact reports, adaptation PRs, validation, and
baseline-promotion changes, but must stop for architecture review when a change
requires new ownership, dependency direction, public abstractions, semantic/IR
redesign, security capabilities, a permanent divergence, weakened invariants,
generic plugins, or backend escape hatches. Human review and merge remain the
authority boundary.

The verified baseline advances only after:

1. permitted public specification/release evidence is reviewed;
2. affected behavior is identified;
3. independent conformance cases exist;
4. required implementation changes are complete;
5. the relevant regression and conformance suites pass; and
6. known divergences are documented and reviewed.

See `docs/adr/0016-full-quarkdown-compatibility-and-upstream-evolution.md` and
`docs/adr/0013-upstream-compatibility-observation-and-baseline-promotion.md`.

## Outside the language-compatibility target

The complete target concerns the public document language and observable
document semantics. The following are implementation/product surfaces rather
than public language claims:

- Quarkdown interactive slide runtime
- Quarkdown internal plugin ABI
- Quarkdown-specific CSS themes
- Quarkdown HTML post-processing
- Quarkdown line click interactivity

These exclusions do not create a general escape hatch for publicly documented
language features. If a public-language behavior is deliberately divergent, it
requires the rationale, compatibility documentation, appropriate diagnostics,
and an ADR when substantial.

The remaining v2.5.1 data-loading families (`.csv`, `.includeall`,
`.listfiles`, `.filename`) and `.llmstxt` are tracked as deferred above; they do
not belong to the non-language exclusions above. As features are implemented,
their matrix status and evidence are promoted; until then they remain explicit
gaps against the complete target.
