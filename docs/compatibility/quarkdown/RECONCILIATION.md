# Quarkdown v2.5.1 compatibility reconciliation

## Decision record

- **Issue:** [#156](https://github.com/luceat-lux-vestra/scribium/issues/156)
- **Parent:** [#147](https://github.com/luceat-lux-vestra/scribium/issues/147)
- **Tracked target:** Quarkdown v2.5.1
- **Pinned upstream revision:** [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e)
- **Requested main baseline:** `4875fb1210f0f9f3fdadc47bf48197b2bdaa17ec`
- **Reconciliation checkout:** `4875fb1210f0f9f3fdadc47bf48197b2bdaa17ec`
- **Observation date:** 2026-08-26

The requested baseline and the current `origin/main` are the same commit. The
audit bases recorded in the individual records remain historical provenance;
they are not silently rewritten. Issues [#148](https://github.com/luceat-lux-vestra/scribium/issues/148)
through [#155](https://github.com/luceat-lux-vestra/scribium/issues/155) are
closed, are all children of #147, and have their canonical audit artifacts
listed below. #156 does not implement a compatibility feature and does not
close #147.

## Canonical status rule

Each public surface has exactly one primary status in the canonical owner’s
inventory. A surface may occur in another audit as a handoff, but that row is
`NOT_APPLICABLE` to the consumer audit and must point to the canonical owner.
The allowed primary vocabulary is:

`SUPPORTED_END_TO_END`, `SUPPORTED_SEMANTICS`, `PARSED_ONLY`, `PARTIAL`,
`UNSUPPORTED`, `DEFERRED`, `BLOCKED`, `NOT_APPLICABLE`, and `UNKNOWN`.

The status is deliberately layer-specific:

- `SUPPORTED_END_TO_END` requires frontend, binding/conversion, evaluator,
  required backend-neutral IR, observable backend/output evidence, and a
  relevant independent conformance case.
- `SUPPORTED_SEMANTICS` requires deterministic evaluator behavior, diagnostics,
  and the required backend-neutral representation. It makes no output claim.
- `PARSED_ONLY` requires recognition plus source identity/span evidence only.
  It is not evaluator or renderer support.
- `PARTIAL` identifies a bounded supported slice and an exact remaining gap.
- `UNSUPPORTED`, `DEFERRED`, `BLOCKED`, and actionable `UNKNOWN` each require a
  bounded issue, an explicit defer, or an explicit blocker. No generic “future
  work” disposition is canonical.

Counts in the following table are per audit inventory, not a global support
total. Cross-audit rows overlap by design and must not be summed as unique
surfaces.

| Audit / canonical owner | Canonical artifact and executable guard | Enumerated rows | Status result | Cross-audit rule |
|---|---|---:|---|---|
| #148 grammar/frontend | [`CALL_GRAMMAR_AUDIT.md`](CALL_GRAMMAR_AUDIT.md); `crates/scribium-markdown/tests/call_grammar_audit.rs` | 15 | 8 `PARTIAL`, 7 `PARSED_ONLY` | Recognition/provenance only; #165 owns shared structural binding, #149 owns target-driven conversion, and #150 owns evaluation. #157, #158, and #163's bounded grammar/frontend slices are implemented; remaining grammar gaps retain their row statuses. |
| #149 value/binding/conversion | [`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md) | 23 | 10 `SUPPORTED_SEMANTICS`, 12 `PARTIAL`, 1 `NOT_APPLICABLE` | Shared conversion and binding are engine-owned; consumers link back instead of adding local adapters. |
| #150 programmable semantics | [`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md) | 16 | 14 `PARTIAL`, 1 `UNSUPPORTED`, 1 `NOT_APPLICABLE` | Callable, scope, order, failure, extension, and provenance semantics remain separate from syntax and content producers. |
| #151 stdlib/general builtins | [`STDLIB_BUILTINS_AUDIT.md`](STDLIB_BUILTINS_AUDIT.md) and [`STDLIB_BUILTINS_AUDIT_MANIFEST.tsv`](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv) | 162 | 43 `SUPPORTED_SEMANTICS`, 6 `PARTIAL`, 10 `UNSUPPORTED`, 1 `NOT_APPLICABLE` among 60 #151-owned rows; 102 explicit handoffs | The complete pinned declaration sweep is retained; cross-owned names are not silently omitted or reclassified. |
| #152 document metadata/state | [`DOCUMENT_STATE_AUDIT.md`](DOCUMENT_STATE_AUDIT.md) and [`DOCUMENT_STATE_AUDIT_MANIFEST.tsv`](DOCUMENT_STATE_AUDIT_MANIFEST.tsv) | 43 | 8 #152-owned `PARTIAL`; 35 `NOT_APPLICABLE` handoffs | #152 owns document metadata/state semantics; layout, content, resource, and localization rows retain their owners. |
| #153 layout/configuration | [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md) and [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv) | 47 | 19 #153-owned `PARSED_ONLY`, 1 #153-owned `PARTIAL`; 27 handoffs | Document-wide configuration is not promoted from component-local content or parser retention. |
| #154 content/media/Markdown extensions | [`CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md`](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md) and [`CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv`](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv) | 83 | 13 `SUPPORTED_END_TO_END`, 3 `SUPPORTED_SEMANTICS`, 1 `PARSED_ONLY`, 13 `PARTIAL`, 37 `UNSUPPORTED`, 2 `DEFERRED`, 1 `BLOCKED`, 1 `UNKNOWN` among 71 #154-owned rows; 12 handoffs | Ordinary Markdown, Quarkdown content producers, resources, and global configuration remain distinct layers. |
| #155 filesystem/project/data/resources | [`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md) and [`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv) | 29 | 4 `SUPPORTED_SEMANTICS`, 8 `PARTIAL`, 8 `UNSUPPORTED`, 1 `DEFERRED` among 21 #155-owned rows; 8 handoffs | Logical project/resource semantics are separate from host access, backend staging, and WASM exposure. |

The eight inventories above are the complete row sources. This document is
the cross-audit view: the individual manifest or matrix remains the detailed
row authority, while the owner and handoff rules below prevent a row from
acquiring a second status when it is consumed by another audit.

## Reconciled artifact register

The following register is the complete v2.5.1 audit/evidence surface in the
checkout. It distinguishes row authorities from supporting records and
executable evidence so that a missing fixture cannot be mistaken for a missing
audit row, or vice versa.

| Evidence role | Artifacts |
|---|---|
| Target and source provenance | [`SPEC_SOURCES.md`](SPEC_SOURCES.md), [`upstream.toml`](upstream.toml), [`V2_5_1_IMPACT.md`](V2_5_1_IMPACT.md) |
| Audit records | [`CALL_GRAMMAR_AUDIT.md`](CALL_GRAMMAR_AUDIT.md), [`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md), [`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md), [`PROGRAMMABLE_DOCUMENT_SEMANTICS.md`](PROGRAMMABLE_DOCUMENT_SEMANTICS.md), [`STDLIB_BUILTINS_AUDIT.md`](STDLIB_BUILTINS_AUDIT.md), [`DOCUMENT_STATE_AUDIT.md`](DOCUMENT_STATE_AUDIT.md), [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md), [`CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md`](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md), [`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md) |
| Row manifests | [`STDLIB_BUILTINS_AUDIT_MANIFEST.tsv`](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv), [`DOCUMENT_STATE_AUDIT_MANIFEST.tsv`](DOCUMENT_STATE_AUDIT_MANIFEST.tsv), [`LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv`](LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv), [`CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv`](CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv), [`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv) |
| Compatibility views | [`README.md`](README.md), [`GAP_INVENTORY.md`](GAP_INVENTORY.md), this reconciliation, [`docs/ROADMAP.md`](../../ROADMAP.md), and [`docs/SYNTAX.md`](../../SYNTAX.md) |
| Audit guards and layer evidence | `crates/scribium-markdown/tests/call_grammar_audit.rs`; `crates/scribium-core/tests/quarkdown_value_model_audit.rs`, `stdlib_builtin_audit.rs`, `document_state_audit.rs`, `layout_document_configuration_audit.rs`, `content_media_markdown_extensions_audit.rs`, `filesystem_project_data_resources_audit.rs`, and `quarkdown_v251_reconciliation.rs` |
| Parser, evaluator/IR, backend, and resource evidence | `crates/scribium-markdown/tests/quarkdown_v2_5_1.rs`; `crates/scribium-core/tests/quarkdown_br.rs`, `quarkdown_html_contract.rs`, `quarkdown_resource_builtins.rs`, `quarkdown_stacked_layout.rs`, `quarkdown_center.rs`, `quarkdown_align.rs`, `quarkdown_container.rs`, `quarkdown_landscape.rs`, and `quarkdown_whitespace.rs`; `crates/scribium-typst-subprocess/tests/backend_integration.rs`; the corresponding `scribium-core`, `scribium-ir`, and `scribium-typst` unit tests referenced by the audit records |
| Serde and WASM boundary evidence | Document-state, value, collection, and component serde tests referenced by the audit records; [`FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md`](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md) records that no WASM resource binding or native/WASM resource fixture exists, so the boundary remains `DEFERRED` under #191 |
| Independent conformance corpus | [`fixtures/quarkdown-conformance/README.md`](../../../fixtures/quarkdown-conformance/README.md) and exactly these 17 case directories: `br-line-break-family`, `call-dot-prefixed-basic`, `call-indented-body-basic`, `call-positional-basic`, `captionposition-document-state`, `docauthor-family`, `docauthors-family`, `dockeywords-family`, `doclang-family`, `dynamic-value-scalar-family`, `numeric-arithmetic-family`, `numeric-decimal-family`, `numeric-transcendental-family`, `optionality-callback-family`, `plaintext-family`, `string-scalar-family`, and `theme-document-state` |

The compatibility README and gap inventory are views over the audit records,
not additional row owners. The referenced unit and integration tests are
layer evidence; their existence does not promote a row beyond the canonical
status recorded by its owner.

## Cross-audit canonical ownership

| Surface or boundary | Primary owner and status | Consumer/handoff | Evidence and residual gap |
|---|---|---|---|
| Dot-call grammar, separators, escaped delimiters, tight calls, and malformed recovery | #148; `PARTIAL` or `PARSED_ONLY` per row | #165 binding, #149 conversion, #150 evaluation, #154 content | Parser tests retain spans; the remaining pinned grammar/provenance gaps are #159, #162, and #164. #158's nested tight-call and #160's Markdown-content preservation, plus #163's ordered argument handoff, are implemented bounded slices. Parser recognition is never a semantic claim. |
| Value taxonomy, origin-sensitive conversion, binding, and conversion diagnostics | #149 plus #165 for structural binding and #166 for the bounded target/raw-body slice; mostly `SUPPORTED_SEMANTICS` or `PARTIAL` per row | #150, #152, #153, #154, #155 | Current typed engine paths and bounded source-backed conversion consumers are evidenced; broader target coverage, diagnostics, and atomicity gaps remain. |
| Variables, callable scope, lazy evaluation, iteration, optionality, extension, failure, and evaluator provenance | #150; `PARTIAL` except `.node` | #151 builtin declarations; #154 content results | Current bounded callable and `.extend`/`.super` paths are tested. #169 covers source-defined and regular scalar-native targets through the shared evaluator and canonical binder/conversion path, including stable scope-local extension-link identity, replacement retirement, and lifetime; specialized native owners, renderer output, and upstream partial-effects parity remain gaps. |
| General stdlib declaration set and bounded scalar/numeric/collection functions | #151; exact per-name status in the 162-row manifest | #149 value boundary; #150 callback flow; #152–#155 consumers | The 162-name pinned sweep is complete. `.capitalize`/`.startswith` are now `SUPPORTED_SEMANTICS` at the bounded scalar boundary after #172; `.get` is #194-owned, library inspection is #195-owned, localization is #196-owned, and `.log`/`.debug`/`.error` are #197-owned `UNSUPPORTED` contracts. |
| `.docname`, `.docdescription`, `.doctype`, `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, `.theme` | #152; all eight `PARTIAL` | #149 conversion; #153 layout; #154 output; #173 locale closure | Evaluator/IR state and independent fixtures exist. #166 covers bounded source-backed body conversion without parsed-body evaluation; rendering, complete locale coverage, and broader metadata remain gaps. |
| `.captionposition` | #153; `PARTIAL` | #152 state snapshot; #154 caption-producing content | Typed evaluator/IR merge, serde, and bounded raw block-body conversion evidence exists; caption rendering and broader target coverage remain open. |
| Remaining document-wide layout/configuration (`.numbering`, `.pageformat`, `.font`, page counters, navigation, `.slides`, and related rows) | #153; 19 `PARSED_ONLY` rows | #154 component/content consumers; #175–#178 | Parser/retention evidence does not establish state, IR, or output support. Follow-up ownership remains grouped by contract. |
| Bounded ordinary Markdown block/inline/table/fence/link behavior | #154; `SUPPORTED_END_TO_END` rows | Rushdown frontend and Typst output | Existing parser/IR/Typst/PDF evidence is for the bounded Markdown contract, not Quarkdown callable producers. |
| Quarkdown content producers, captions, references, tables, math, code, slides, media, and raw content | #154; exact manifest status | #149/#150 conversion and callbacks; #153 policy; #155 resources | Unsupported/partial rows retain exact producer issues #180–#185, with `.match` in #198, `.subdocumentgraph` in #199 after #188, and `.keybinding`/`.loremipsum` explicitly in #184. CSS rows remain an explicit HTML-backend/product defer. No backend escape hatch is introduced. |
| `.read` | #155; `PARTIAL` | #188 common logical resolver | Source-relative in-memory text and bounded line selection work. Absolute/global semantics, complete permissions, and full library/project behavior do not. |
| `.json` | #155; `PARTIAL` | #188 resolver; #149 value conversion | Source-relative in-memory JSON object/array/scalar behavior is evidenced. Full recursive/project/permission parity is absent. |
| `.include` | #155; `PARTIAL` | #188 common resolver and nested identity | Nested source identity, cycle detection, and bounded share/scope behavior are evidenced. Absolute/global/library and complete graph parity are absent. |
| VirtualProject / ResourceProvider logical resource model | #155; `SUPPORTED_SEMANTICS` | #187 backend strategy; #188–#191 consumers | In-memory logical paths, source identity, project boundaries, and deterministic provider contracts are evidenced; language-facing breadth remains bounded. |
| Typst entry/source-context contract | #155; `PARTIAL` | #187 strategy; #200 explicit selection; #201 parity | The subprocess path remains the default and uses its explicit source context. The optional native in-process adapter maps the same `VirtualProject` boundary; broader cross-platform parity remains in #201. |
| WASM resource boundary | #155; `DEFERRED` | #191 M6/embedder boundary | Core/provider ideas are portable, but no public WASM resource API or native/WASM end-to-end equivalence exists. |

Thus, for example, evaluator support for `.captionposition` does not make
caption output supported, and a parsed `.pageformat` call does not make page
geometry supported. The canonical status is the status of the whole owned
surface at the boundary stated by its row.

## Evidence and conformance reconciliation

All upstream evidence used for a current claim is pinned to
`107ec3a9482f10d6f90d7580f8409b46a719d18e`; [`SPEC_SOURCES.md`](SPEC_SOURCES.md)
records the permitted public documentation, declarations, and black-box
evidence. No Quarkdown source, test, or fixture was copied or translated.

The independent corpus currently contains 17 cases:

- parser/provenance: `call-dot-prefixed-basic`, `call-positional-basic`, and
  `call-indented-body-basic`;
- document state: `docauthor-family`, `docauthors-family`,
  `dockeywords-family`, `doclang-family`, `theme-document-state`, and
  `captionposition-document-state`;
- evaluator/value semantics: `dynamic-value-scalar-family`,
  `numeric-arithmetic-family`, `numeric-decimal-family`,
  `numeric-transcendental-family`, `optionality-callback-family`,
  `plaintext-family`, `string-scalar-family`, and `br-line-break-family`.

The corpus is intentionally bounded and is not a claim that every manifest
row has an executable fixture. `Parsed` cases establish parser-level evidence;
`Semantically supported` cases establish exact backend-neutral IR behavior;
there are currently no corpus cases that promote the broad audit inventories
to output-equivalent Quarkdown support. Unsupported, deferred, blocked, and
unknown rows remain supported by their pinned source record, current tests,
explicit issue/defer/blocker, and manifest guard rather than by a fabricated
positive fixture. The corpus rules are defined in
[`fixtures/quarkdown-conformance/README.md`](../../../fixtures/quarkdown-conformance/README.md).

Evidence layers are kept separate:

`frontend grammar -> binding/conversion -> evaluator/state -> backend-neutral
IR -> content/resource consumers -> pure Typst lowering -> native adapter or
WASM/embedder boundary`.

AST retention, an IR node, or a successful semantic test cannot promote a row
to `SUPPORTED_END_TO_END` without the missing downstream evidence.

## Gap class and backlog reconciliation

The residual work is classified as follows. These are gap classes, not new
implementation requests created by #156.

| Gap class | Current ownership |
|---|---|
| `PRODUCTION_GAP` | #159, #162–#167, #169, #173–#185, #188, #189, and #194–#199 where the required parser, engine, content, layout, or resource behavior is absent or only bounded. |
| `EVIDENCE_GAP` | Only where a bounded implementation exists but the correct layer’s independent conformance/output/provenance evidence is still missing; this does not downgrade a real missing semantic implementation to an evidence task. |
| `DOCUMENTATION_GAP` | Stale family-level claims and stale #156 freeze text corrected by this reconciliation; detailed rows remain in the audit artifacts. |
| `BACKEND_GAP` | #201 parity evidence and #154 producer/output rows whose semantics cannot be observed through the current backend contract. |
| `PLATFORM_GAP` | #190 explicit host capability/injection boundary and the native/WASM/provider exposure portions of #191. |
| `DEFERRED_PRODUCT_SURFACE` | #191 WASM resource exposure, `.css`, and `.cssproperties`, plus audit rows explicitly deferred in the #151/#154/#155 manifests. |

Every currently open implementation issue discovered during the audit window
is represented below. Issue state was checked on 2026-08-26; the listed issue
numbers are not inferred from a numeric sequence.

| Issues | Origin / canonical owner | Scope and dependency decision | Recommended band |
|---|---|---|---|
| [#159](https://github.com/luceat-lux-vestra/scribium/issues/159), [#162](https://github.com/luceat-lux-vestra/scribium/issues/162), [#164](https://github.com/luceat-lux-vestra/scribium/issues/164) | #148 / Markdown and Quarkdown frontend | Malformed recovery, escaped delimiters, and separator placement. These are production grammar/provenance gaps; they must not absorb binder/evaluator behavior. | Frontend band; parallel after #187. |
| [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) | #148 → #154 / Markdown content conversion | Implemented bounded frontend slice: supported Markdown inline nodes in static Quarkdown content arguments retain Rushdown structure and original-source spans, including the #158 nested tight-call shape. This establishes no evaluator, IR, or output compatibility; dynamic/content conversion is separately owned by #166. | Completed parser/frontend slice; broader content and output contracts remain with #154. |
| [#163](https://github.com/luceat-lux-vestra/scribium/issues/163) | #148 → #165 / grammar representation for engine binding | Implemented in the grammar/frontend and IR: head and chain segments preserve one source-ordered argument sequence with provenance while retaining legacy projections for adapters. #165 consumes this representation for semantic binding. | Completed bounded representation prerequisite; grammar recognition remains separate from semantic compatibility. |
| [#165](https://github.com/luceat-lux-vestra/scribium/issues/165) | #149 / shared engine binder | Implemented bounded engine contract: one binder validates ordered mixed arguments, exact names/aliases, named eligibility, duplicate/collision/excess rules, required/optional/default slots, and target-owned body policy for native, source-defined, and callback paths. Broader commit/diagnostic guarantees remain separate. | Completed bounded engine binding slice; #166 and #167 remain. |
| [#166](https://github.com/luceat-lux-vestra/scribium/issues/166) | #149 / conversion, raw body | Implemented bounded engine-owned target conversion: source-backed raw bodies are retained beside structured calls and dynamic/static/content distinctions remain explicit for the demonstrated consumers; no evaluated nested call is stringified and reparsed. | Completed bounded conversion/raw-body slice; broader targets remain partial. |
| [#167](https://github.com/luceat-lux-vestra/scribium/issues/167) | #149 / diagnostics, atomicity | #167 hardens conversion diagnostics and the shared commit/rollback boundary used by state and content consumers. | Engine follow-up band. |
| [#169](https://github.com/luceat-lux-vestra/scribium/issues/169) | #150 / engine evaluator | Implemented bounded `.extend`/`.super` evaluator semantics for visible source-defined callables and regular scalar builtins: shared target binding and canonical builtin conversion, condition/delegation, ordered chains, scope layering, source-backed failures, and #167 savepoint rollback. Scope-local extension links do not leak after ephemeral owners end; specialized native owners, complete upstream failure-effect parity, and renderer/output evidence remain deferred. | Bounded engine slice complete; broader target/output convergence remains after the engine prerequisite band. |
| [#172](https://github.com/luceat-lux-vestra/scribium/issues/172) | #151 / engine string semantics | Implemented bounded Unicode string slice: `.capitalize` reproduces Kotlin `Char.titlecase()` over JDK 17-compatible Unicode 13.0 full/simple mapping data while preserving the remainder, and `.startswith(ignorecase:true)` uses the corresponding Kotlin/JVM character-wise simple upper/lower comparison without normalization or global state. Compile-time guards cover both mapping tables, and post-Unicode-13 plus full/simple-divergence regressions prevent newer data or inferred mappings from widening the contract. The two #151 rows are promoted to `SUPPORTED_SEMANTICS`; no end-to-end claim is made. | Completed bounded engine slice; broader string-family and output contracts remain with their owners. |
| [#173](https://github.com/luceat-lux-vestra/scribium/issues/173) | #152 / document locale state | #173 owns bounded `.doclang` locale closure. It consumes shared conversion/state contracts and does not duplicate localization ownership. | Parallel after shared engine prerequisites. |
| [#175](https://github.com/luceat-lux-vestra/scribium/issues/175), [#176](https://github.com/luceat-lux-vestra/scribium/issues/176), [#177](https://github.com/luceat-lux-vestra/scribium/issues/177), [#178](https://github.com/luceat-lux-vestra/scribium/issues/178) | #153 / document-wide configuration and layout policy | #175 owns document-wide state; #176 owns page margins/counters/heading/page policy; #177 owns navigation/markers/TOC; #178 owns slide configuration. Component-local content remains #154-owned. They consume #165–#167 and the backend strategy decision where output is involved. | Layout/configuration band after prerequisites. |
| [#180](https://github.com/luceat-lux-vestra/scribium/issues/180) | #153 → #154 / `.texmacro` state and raw body | Distinct macro-map/raw-body/math-renderer contract. Depends on #166–#167 and is consumed by #185; it is not a generic backend escape hatch. | After raw-body/atomicity prerequisites. |
| [#181](https://github.com/luceat-lux-vestra/scribium/issues/181) | #154 / structural content and shared identifiers/references/index | Shared caption, identifier, reference, and index infrastructure. It is consumed by #177, #183, and #185; it does not own their producer semantics. | Shared content infrastructure band. |
| [#182](https://github.com/luceat-lux-vestra/scribium/issues/182) | #154 / media and image producers | Media sizing, icons, diagrams, and output contract. Hard dependency on #188 for project resources; consumes #160/#166–#167 and backend strategy. | After #188 and content prerequisites. |
| [#183](https://github.com/luceat-lux-vestra/scribium/issues/183) | #154 / table producers | Table generation/computation and output. Depends on #181 shared identifiers/captions, #165–#167 conversion, and #189 for CSV/data-file input where applicable. | After shared content and data-file prerequisites. |
| [#184](https://github.com/luceat-lux-vestra/scribium/issues/184) | #154 / component and slide content | Component-local content, containers, slide content, plus `.keybinding` and `.loremipsum` content-producer review; no generalized style framework. Depends on #178 for slide configuration and #175 only for document-wide policy. | After layout/content prerequisites. |
| [#185](https://github.com/luceat-lux-vestra/scribium/issues/185) | #154 / math, code, and explicit breaks | Math/code/break producers and output. Depends on #180 and #181 plus shared conversion/raw-body contracts. | After macro/shared-content prerequisites. |
| [#194](https://github.com/luceat-lux-vestra/scribium/issues/194) | #151 / dictionary lookup | `.get` lookup, key conversion, missing-key/`orelse` behavior, typed nested values, diagnostics, and atomicity. Depends on #165–#167; it does not duplicate dictionary construction. | After #187; parallel stdlib family band after shared engine prerequisites. |
| [#195](https://github.com/luceat-lux-vestra/scribium/issues/195) | #151 / library/runtime inspection | `.libexists`, `.functionexists`, `.libraries`, and `.libfunctions` under one deterministic registry view. Depends on #165–#167 and coordinates capability/resource policy with #188/#190; no plugin registry. | After shared engine and #187 strategy; capability/resource coordination band. |
| [#196](https://github.com/luceat-lux-vestra/scribium/issues/196) | #151 / localization table and lookup | `.localization` mutation and `.localize` lookup, including seeded `std`, merge/replace, separators, typed values, diagnostics, and atomicity. Depends on #165–#167 and coordinates with #173 without moving `.doclang`. | After #187; parallel stdlib/state band after shared engine prerequisites. |
| [#197](https://github.com/luceat-lux-vestra/scribium/issues/197) | #151 / logger and diagnostic builtins | `.log`, `.debug`, and `.error` severity/return behavior through an explicit sink/capability or deterministic rejection. Depends on #165–#167 and #190; no implicit process streams. | After shared engine and #187/#190 host-boundary decisions; semantic work can be parallel. |
| [#198](https://github.com/luceat-lux-vestra/scribium/issues/198) | #154 → #150 / `.match` content transformation | Pattern/callback traversal and inline-content replacement; #181 remains shared infrastructure only. Depends on #165–#167 and coordinates output strategy with #187. | After #187 and shared binding/atomicity prerequisites; content/evaluator band. |
| [#199](https://github.com/luceat-lux-vestra/scribium/issues/199) | #154 / subdocument graph producer/output | `.subdocumentgraph` graph/content semantics and output. Hard dependency on #188 logical resolution; coordinates shared identifiers/indexing with #181 and backend strategy with #187. | After #188 and backend/content prerequisites. |
| [#187](https://github.com/luceat-lux-vestra/scribium/issues/187) | #155 → backend strategy | Completed re-evaluation: native in-process Typst is accepted as an optional adapter over `VirtualProject`; subprocess remains the default. Issue #200 adds explicit native selection and #201 owns broader parity evidence. | #200 implemented; #201 remains the companion parity follow-up. |
| [#188](https://github.com/luceat-lux-vestra/scribium/issues/188) | #155 / logical project resource resolution | Common resolver, nested loading, `.read`, `.json`, `.include`, `.includeall`, `.pathtoroot`, and subdocument resource identity. Hard prerequisite for #189, #199, and resource portions of #182/#183; not a prerequisite to start #187. | After #187; first resource implementation band. |
| [#189](https://github.com/luceat-lux-vestra/scribium/issues/189) | #155 / project data and file identity | `.listfiles`, `.filename`, `.csv`, `.bibliography`; depends on #188 and coordinates with #181/#183. | After #188; data consumer band. |
| [#190](https://github.com/luceat-lux-vestra/scribium/issues/190) | #155 / host capability boundary | Deterministic `.env` injection/denial and native/WASM contract; no `std::env`. It can be designed in parallel with #188/#189 after #187, but must precede any environment-dependent exposure. | Parallel platform-contract band after #187. |
| [#191](https://github.com/luceat-lux-vestra/scribium/issues/191) | #155 / M6 WASM/embedder | WASM VirtualProject/resource boundary, provider exposure, diagnostics, and parity. It remains deferred to the WASM milestone; it is not immediately eligible because #156 completed. | Deferred. |

The remaining #154 rows that are not assigned an implementation issue have an
explicit product/backend disposition: `.css` and `.cssproperties` remain
`UNSUPPORTED` and are deferred until a target-specific HTML backend/product
contract is accepted. Closed [#58](https://github.com/luceat-lux-vestra/scribium/issues/58)
is retained as the historical raw-HTML-policy tracker only; it is not their
current owner. The `subdocumentgraph` blocker is no longer #155: #188 is the
logical-resource prerequisite and #199 owns the graph/content/output contract.

No duplicate issue was created by #156. Historical trackers are retained with
their historical meaning: [#24](https://github.com/luceat-lux-vestra/scribium/issues/24)
is the closed Typst source-context tracker now split between #155 evidence and
#187 strategy; [#56](https://github.com/luceat-lux-vestra/scribium/issues/56)
is the closed M2 aggregate; [#60](https://github.com/luceat-lux-vestra/scribium/issues/60)
is the closed syntax tracker; [#61](https://github.com/luceat-lux-vestra/scribium/issues/61)
is the closed programmable foundation; [#62](https://github.com/luceat-lux-vestra/scribium/issues/62)
is the closed resource implementation tracker whose current bounded ownership
is #188–#191; and [#63](https://github.com/luceat-lux-vestra/scribium/issues/63)
is the closed conformance-corpus foundation. None is reopened, silently
closed, or treated as proof of complete v2.5.1 compatibility.

## Dependency graph and implementation order

```text
                         ┌───────────────┐
                         │ #187 backend  │  completed strategy
                         │ → #200/#201  │  selection and parity
                         └───────┬───────┘
                                 │ sequencing preference; no hard #188 edge
        ┌────────────────────────┼────────────────────────┐
        ▼                        ▼                        ▼
  #159/#162/#164            #188 resolver              #190 capability
        │                        │                        │
        ▼                        ▼                        │
      #163                    #189 data                 │
        │                        │                        │
        ▼                        └──────────┬─────────────┘
  #165/#166/#167                           │
        │                                  ▼
        ├───────────────┬────────────── #182 media
        ▼               ▼
      #169          #172 (done) / #173
        │
        ▼
  #175/#176/#177/#178 ──► #181 shared content ──► #183/#184/#185
             │                  ▲       ▲
             └──────────────────┘       │
                          #180 ─────────┘

  #165/#166/#167 ──► #194 dictionary, #196 localization, #197 logger
       #190 capability ──► #195 library inspection
       #188 resolver ──► #199 subdocumentgraph
       #184 also owns the #154 keybinding/loremipsum producer review

  #191: deferred WASM/embedder milestone after logical provider/capability
        contracts and relevant parity evidence.
```

The graph distinguishes three relationships:

- **Hard dependency:** the consumer cannot implement its stated contract
  without the producer/prerequisite (for example #189 after #188, #183’s
  resource-backed CSV path after #189, and #182’s project media path after
  #188).
- **Sequencing preference:** shared engine, content, or backend decisions
  should stabilize first, but the issue is not structurally impossible to
  begin (for example #187 before most output work).
- **Parallelizable/deferred:** independent contracts can proceed in the same
  band; #190 can proceed alongside the resolver after #187, while #191 is
  deferred by the M6 platform milestone.

### Backend strategy closure: #187 → #200

**STATUS:** #187's strategy decision is complete; #200 is the production
selection follow-up and #201 owns broader parity evidence.

**OWNER ISSUE:** #200 owns explicit backend selection and adapter promotion;
#201 owns cross-platform parity.

**WHY THAT ISSUE MUST PRECEDE #187:** not applicable; the strategy decision is
complete and #200 is the explicit-selection follow-up.

**EVIDENCE:** #155 records the current VirtualProject/ResourceProvider logical
model as `SUPPORTED_SEMANTICS` and the Typst source-context contract as
`PARTIAL`; #187 evaluated in-process Typst against that current architecture,
and #200 exposes the resulting adapter only through explicit native selection.
The richer logical resolver in #188 is not needed for this bounded adapter. If
a richer World/resource mapping is later required, that is a new
evidence-backed dependency decision, not an assumption made by #156.

The resulting backend direction is:

```text
SubprocessBackend -> default and supported baseline
InProcessBackend  -> explicit native-only opt-in
```

Removing subprocess execution or changing the default is not decided by #156,
#187, or #200; it requires separate parity and migration evidence.

### Resource ordering: #188–#191

- **#188** is the common logical resolver and nested-loading contract. It is
  first among the resource follow-ups and is a prerequisite for #189 and for
  resource-consuming portions of #182 and #183. It is not a hard prerequisite
  for #187.
- **#189** adds data-file and logical file-identity consumers after #188. CSV
  output is coordinated with #183; bibliography/reference output is
  coordinated with #181.
- **#190** defines an explicit, deterministic host capability. It has no
  permission to add `std::env` or implicit process state. Its contract can be
  developed in parallel with #188/#189 after #187, and it must precede any
  native/WASM environment exposure.
- **#191** remains `DEFERRED` to the M6/WASM/embedder milestone. It must reuse
  the logical provider/capability contracts and prove native/WASM parity; it is
  not a reason to add a temporary filesystem API now.

## Freeze reconciliation

The pre-reconciliation freeze is replaced by the order above. The earlier
freeze wording is no longer a complete status. After
#156 is merged, issues are eligible only in their dependency band:

- #187 is **completed as the backend-strategy re-evaluation** and #200 is the
  explicit-selection follow-up;
- #159–#185 are **sequenced after the relevant shared engine/backend/content
  prerequisites**, with parallel work only where the graph permits it;
- #188 is **after #187**, #189 is **after #188**, and #190 is a parallel
  capability-contract band after #187;
- #191 is **deferred/milestone-blocked** to M6/WASM; and
- no issue is authorized to bypass the architecture, evidence, or host
  boundaries recorded here.

## Issue #166 implementation reconciliation

The bounded #166 engine slice closes the raw-body prerequisite for the
reviewed conversion consumers. `scribium-markdown` retains one shared,
immutable source buffer and the exact body-token span beside the structured
body; `scribium-ir` carries that durable source data as `IrRawBody`, with a
source-local `ByteSpan` and caller provenance on the containing node, never as
`IrValue` state or `ValueOrigin`. The engine-owned `value_conversion` target
boundary preserves
typed scalar/content/node/iterable/dictionary/callable distinctions, returns an
explicit contextual Markdown request for dynamic text, and materializes only
the targets consumed by this slice.

The demonstrated consumers are dynamic String to inline Markdown through
`.plaintext`, typed and Markdown-list iterable conversion, and regular
document-state body fallback for `.theme`, `.doclang`, and `.captionposition`.
The retained body source range is the complete upstream body token, including
leading/trailing blank lines; each non-blank continuation line independently
requires the pinned two-space-or-tab prefix, and its target value applies
`trimIndent().trimEnd()`. JSON serialization of an `IrDocument` stores each
distinct source buffer once in a document-scoped source table while retaining
raw-body spans, so inspect output does not repeat the full document for every
unresolved call. The source-table `source_ref` is private to the document wire
model; standalone `IrRawBody` and `IrNode` values reject unresolved references.
Source-backed body fallback does not evaluate or stringify the parsed nested
body. Dynamic iterable conversion evaluates one raw expression in context and
uses typed Iterable/Dictionary results before its Markdown-list fallback.
Static String content remains literal text, raw HTML keeps its existing
source-preserving path, and #158/#160 structure remains unchanged. The
document-level source-table reference is a wire-only encoding: standalone
`IrRawBody` values reject `source_ref`, missing/out-of-range table references
are rejected, and successful deserialization restores the source-local span
invariant. Callable,
dictionary, remaining block-content consumers, complete upstream target
coverage, and diagnostic/commit parity remain partial or deferred to their
owners; #167 is not absorbed here.

Individual issue bodies are cross-linked to this decision and retain their
original scope/non-goals. No production implementation was started by #156.

## #147 completion readiness

The reconciliation supplies evidence for each parent completion criterion:

- all eight audit inventories and their row totals are enumerated;
- each audit’s detailed row authority and exact canonical vocabulary are
  identified;
- cross-audit duplicates have one owner and explicit handoffs;
- supported claims are tied to the appropriate frontend, evaluator, IR,
  backend/output, fixture, or platform evidence layer;
- every actionable partial/unsupported/unknown resource and implementation gap
  points to the listed bounded follow-ups (including #194–#199), an explicit
  product/backend defer, or an explicit blocker;
- historical trackers are retained without being mistaken for current
  compatibility proof;
- the conformance corpus, docs, manifests, and backlog share the same target
  revision and layer-specific claims; and
- residual gaps and the dependency-aware order are explicit.

This is **closure-ready evidence for #147 after this branch/PR is merged and
its checks pass**. #147 remains open and is not automatically closed by this
change.
