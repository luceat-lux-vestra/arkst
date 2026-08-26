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
| #148 grammar/frontend | [`CALL_GRAMMAR_AUDIT.md`](CALL_GRAMMAR_AUDIT.md); `crates/scribium-markdown/tests/call_grammar_audit.rs` | 15 | 12 `PARTIAL`, 3 `PARSED_ONLY` | Recognition/provenance only; #149 owns binding and #150 owns evaluation. |
| #149 value/binding/conversion | [`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md) | 23 | 10 `SUPPORTED_SEMANTICS`, 12 `PARTIAL`, 1 `NOT_APPLICABLE` | Shared conversion and binding are engine-owned; consumers link back instead of adding local adapters. |
| #150 programmable semantics | [`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md) | 16 | 13 `PARTIAL`, 2 `UNSUPPORTED`, 1 `NOT_APPLICABLE` | Callable, scope, order, failure, and provenance semantics remain separate from syntax and content producers. |
| #151 stdlib/general builtins | [`STDLIB_BUILTINS_AUDIT.md`](STDLIB_BUILTINS_AUDIT.md) and [`STDLIB_BUILTINS_AUDIT_MANIFEST.tsv`](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv) | 162 | 41 `SUPPORTED_SEMANTICS`, 8 `PARTIAL`, 10 `UNSUPPORTED`, 1 `NOT_APPLICABLE` among 60 #151-owned rows; 102 explicit handoffs | The complete pinned declaration sweep is retained; cross-owned names are not silently omitted or reclassified. |
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
| Dot-call grammar, separators, escaped delimiters, tight calls, and malformed recovery | #148; `PARTIAL` or `PARSED_ONLY` per row | #149 binding, #150 evaluation, #154 content | Parser tests retain spans, but pinned lexical/recovery gaps are #157–#164. Parser recognition is never a semantic claim. |
| Value taxonomy, origin-sensitive conversion, binding, and conversion diagnostics | #149; mostly `SUPPORTED_SEMANTICS` or `PARTIAL` per row | #150, #152, #153, #154, #155 | Current typed engine paths are evidenced; shared binder/raw-body/diagnostic/atomicity gaps are #165–#167. |
| Variables, callable scope, lazy evaluation, iteration, optionality, extension, failure, and evaluator provenance | #150; `PARTIAL` except `.node`/`.extend` boundaries | #151 builtin declarations; #154 content results | Current bounded callable paths are tested. `.extend`/`.super` is `UNSUPPORTED` and #169-owned; upstream partial-effects divergence is not fixed here. |
| General stdlib declaration set and bounded scalar/numeric/collection functions | #151; exact per-name status in the 162-row manifest | #149 value boundary; #150 callback flow; #152–#155 consumers | The 162-name pinned sweep is complete. `.capitalize`/`.startswith` remain `PARTIAL` and #172-owned; `.localization`/`.localize` remain #151-owned `UNSUPPORTED`. |
| `.docname`, `.docdescription`, `.doctype`, `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, `.theme` | #152; all eight `PARTIAL` | #149 conversion; #153 layout; #154 output; #173 locale closure | Evaluator/IR state and independent fixtures exist. Rendering, complete locale coverage, raw-body fallback, and broader metadata remain gaps. |
| `.captionposition` | #153; `PARTIAL` | #152 state snapshot; #154 caption-producing content | Typed evaluator/IR merge and serde evidence exists; caption rendering and raw block-body fallback are not claimed. |
| Remaining document-wide layout/configuration (`.numbering`, `.pageformat`, `.font`, page counters, navigation, `.slides`, and related rows) | #153; 19 `PARSED_ONLY` rows | #154 component/content consumers; #175–#178 | Parser/retention evidence does not establish state, IR, or output support. Follow-up ownership remains grouped by contract. |
| Bounded ordinary Markdown block/inline/table/fence/link behavior | #154; `SUPPORTED_END_TO_END` rows | Rushdown frontend and Typst output | Existing parser/IR/Typst/PDF evidence is for the bounded Markdown contract, not Quarkdown callable producers. |
| Quarkdown content producers, captions, references, tables, math, code, slides, media, and raw content | #154; exact manifest status | #149/#150 conversion and callbacks; #153 policy; #155 resources | Unsupported/partial rows retain exact producer issues #180–#185. No backend escape hatch is introduced. |
| `.read` | #155; `PARTIAL` | #188 common logical resolver | Source-relative in-memory text and bounded line selection work. Absolute/global semantics, complete permissions, and full library/project behavior do not. |
| `.json` | #155; `PARTIAL` | #188 resolver; #149 value conversion | Source-relative in-memory JSON object/array/scalar behavior is evidenced. Full recursive/project/permission parity is absent. |
| `.include` | #155; `PARTIAL` | #188 common resolver and nested identity | Nested source identity, cycle detection, and bounded share/scope behavior are evidenced. Absolute/global/library and complete graph parity are absent. |
| VirtualProject / ResourceProvider logical resource model | #155; `SUPPORTED_SEMANTICS` | #187 backend strategy; #188–#191 consumers | In-memory logical paths, source identity, project boundaries, and deterministic provider contracts are evidenced; language-facing breadth remains bounded. |
| Typst entry/source-context contract | #155; `PARTIAL` | #187 backend-strategy spike | Current source context is coupled to subprocess staging/adapter behavior; #187 evaluates in-process integration against the existing VirtualProject. This is not an in-process implementation. |
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
| `PRODUCTION_GAP` | #157–#160, #162–#167, #169, #172–#185, #188, and #189 where the required parser, engine, content, layout, or resource behavior is absent. |
| `EVIDENCE_GAP` | Only where a bounded implementation exists but the correct layer’s independent conformance/output/provenance evidence is still missing; this does not downgrade a real missing semantic implementation to an evidence task. |
| `DOCUMENTATION_GAP` | Stale family-level claims and stale #156 freeze text corrected by this reconciliation; detailed rows remain in the audit artifacts. |
| `BACKEND_GAP` | #187 strategy research and #154 producer/output rows whose semantics cannot be observed through the current backend contract. |
| `PLATFORM_GAP` | #190 explicit host capability/injection boundary and the native/WASM/provider exposure portions of #191. |
| `DEFERRED_PRODUCT_SURFACE` | #191 WASM resource exposure, plus audit rows explicitly deferred in the #151/#154/#155 manifests. |

Every currently open implementation issue discovered during the audit window
is represented below. Issue state was checked on 2026-08-26; the listed issue
numbers are not inferred from a numeric sequence.

| Issues | Origin / canonical owner | Scope and dependency decision | Recommended band |
|---|---|---|---|
| [#157](https://github.com/luceat-lux-vestra/scribium/issues/157), [#159](https://github.com/luceat-lux-vestra/scribium/issues/159), [#162](https://github.com/luceat-lux-vestra/scribium/issues/162), [#164](https://github.com/luceat-lux-vestra/scribium/issues/164) | #148 / Markdown and Quarkdown frontend | Lexical boundaries, malformed recovery, escaped delimiters, and separator placement. These are production grammar/provenance gaps; they must not absorb binder/evaluator behavior. | Frontend band; parallel after #187. |
| [#158](https://github.com/luceat-lux-vestra/scribium/issues/158), [#160](https://github.com/luceat-lux-vestra/scribium/issues/160) | #148 → #154 / Markdown content conversion | Nested tight-call structure and inline Markdown content retention. #160 consumes the frontend/content boundary; #158 is its structural prerequisite where nested wrappers are involved. | Frontend/content band; #160 follows #158 where the shared representation is required. |
| [#163](https://github.com/luceat-lux-vestra/scribium/issues/163) | #148 → #149 / grammar representation with engine handoff | Preserve positional-after-named shape in frontend data; semantic rejection remains #149-owned. Depends on #157’s name/shape contract. | After #157, before #165. |
| [#165](https://github.com/luceat-lux-vestra/scribium/issues/165), [#166](https://github.com/luceat-lux-vestra/scribium/issues/166), [#167](https://github.com/luceat-lux-vestra/scribium/issues/167) | #149 / engine binder, conversion, raw body, diagnostics, atomicity | One shared engine contract; no per-builtin binder or conversion duplicates. #165 depends on #163; #166 consumes structured content/call shapes; #167 hardens the shared commit/diagnostic boundary used by state and content consumers. | Engine prerequisite band. |
| [#169](https://github.com/luceat-lux-vestra/scribium/issues/169) | #150 / engine evaluator | `.extend`/`.super`, condition, chaining, scope, and failure semantics. Depends on the grammar and shared binder/evaluator contracts (#157–#167). | After engine prerequisite band. |
| [#172](https://github.com/luceat-lux-vestra/scribium/issues/172), [#173](https://github.com/luceat-lux-vestra/scribium/issues/173) | #151 / #152 | #172 owns Unicode titlecase and case-insensitive prefix semantics; #173 owns bounded `.doclang` locale closure. They consume shared conversion/state contracts and do not duplicate localization ownership. | Parallel after shared engine prerequisites. |
| [#175](https://github.com/luceat-lux-vestra/scribium/issues/175), [#176](https://github.com/luceat-lux-vestra/scribium/issues/176), [#177](https://github.com/luceat-lux-vestra/scribium/issues/177), [#178](https://github.com/luceat-lux-vestra/scribium/issues/178) | #153 / document-wide configuration and layout policy | #175 owns document-wide state; #176 owns page margins/counters/heading/page policy; #177 owns navigation/markers/TOC; #178 owns slide configuration. Component-local content remains #154-owned. They consume #165–#167 and the backend strategy decision where output is involved. | Layout/configuration band after prerequisites. |
| [#180](https://github.com/luceat-lux-vestra/scribium/issues/180) | #153 → #154 / `.texmacro` state and raw body | Distinct macro-map/raw-body/math-renderer contract. Depends on #166–#167 and is consumed by #185; it is not a generic backend escape hatch. | After raw-body/atomicity prerequisites. |
| [#181](https://github.com/luceat-lux-vestra/scribium/issues/181) | #154 / structural content and shared identifiers/references/index | Shared caption, identifier, reference, and index infrastructure. It is consumed by #177, #183, and #185; it does not own their producer semantics. | Shared content infrastructure band. |
| [#182](https://github.com/luceat-lux-vestra/scribium/issues/182) | #154 / media and image producers | Media sizing, icons, diagrams, and output contract. Hard dependency on #188 for project resources; consumes #160/#166–#167 and backend strategy. | After #188 and content prerequisites. |
| [#183](https://github.com/luceat-lux-vestra/scribium/issues/183) | #154 / table producers | Table generation/computation and output. Depends on #181 shared identifiers/captions, #165–#167 conversion, and #189 for CSV/data-file input where applicable. | After shared content and data-file prerequisites. |
| [#184](https://github.com/luceat-lux-vestra/scribium/issues/184) | #154 / component and slide content | Component-local content, containers, and slide content; no generalized style framework. Depends on #178 for slide configuration and #175 only for document-wide policy. | After layout/content prerequisites. |
| [#185](https://github.com/luceat-lux-vestra/scribium/issues/185) | #154 / math, code, and explicit breaks | Math/code/break producers and output. Depends on #180 and #181 plus shared conversion/raw-body contracts. | After macro/shared-content prerequisites. |
| [#187](https://github.com/luceat-lux-vestra/scribium/issues/187) | #155 → backend strategy | Re-evaluate in-process Typst against the existing VirtualProject architecture. It is a technical spike, not builtin implementation; preferred native in-process backend requires parity evidence, while subprocess remains fallback/transition until a separate migration decision. | **Immediate next technical work.** |
| [#188](https://github.com/luceat-lux-vestra/scribium/issues/188) | #155 / logical project resource resolution | Common resolver, nested loading, `.read`, `.json`, `.include`, `.includeall`, `.pathtoroot`, and subdocument resource identity. Hard prerequisite for #189 and resource portions of #182/#183; not a prerequisite to start #187. | After #187; first resource implementation band. |
| [#189](https://github.com/luceat-lux-vestra/scribium/issues/189) | #155 / project data and file identity | `.listfiles`, `.filename`, `.csv`, `.bibliography`; depends on #188 and coordinates with #181/#183. | After #188; data consumer band. |
| [#190](https://github.com/luceat-lux-vestra/scribium/issues/190) | #155 / host capability boundary | Deterministic `.env` injection/denial and native/WASM contract; no `std::env`. It can be designed in parallel with #188/#189 after #187, but must precede any environment-dependent exposure. | Parallel platform-contract band after #187. |
| [#191](https://github.com/luceat-lux-vestra/scribium/issues/191) | #155 / M6 WASM/embedder | WASM VirtualProject/resource boundary, provider exposure, diagnostics, and parity. It remains deferred to the WASM milestone; it is not immediately eligible because #156 completed. | Deferred. |

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
                         │ #187 backend  │  immediate next
                         │ strategy      │
                         └───────┬───────┘
                                 │ sequencing preference; no hard #188 edge
        ┌────────────────────────┼────────────────────────┐
        ▼                        ▼                        ▼
  #157/#159/#162/#164       #188 resolver              #190 capability
        │                        │                        │
        ▼                        ▼                        │
      #163                    #189 data                 │
        │                        │                        │
        ▼                        └──────────┬─────────────┘
  #165/#166/#167                           │
        │                                  ▼
        ├───────────────┬────────────── #182 media
        ▼               ▼
      #169          #172/#173
        │
        ▼
  #175/#176/#177/#178 ──► #181 shared content ──► #183/#184/#185
             │                  ▲       ▲
             └──────────────────┘       │
                          #180 ─────────┘

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

### Immediate next: #187

**BLOCKER:** none found during reconciliation.

**WHY IT BLOCKS #187:** not applicable; no blocker is present.

**OWNER ISSUE:** #187 owns the backend-strategy question.

**WHY THAT ISSUE MUST PRECEDE #187:** not applicable; #187 is the next work.

**EVIDENCE:** #155 records the current VirtualProject/ResourceProvider logical
model as `SUPPORTED_SEMANTICS` and the Typst source-context contract as
`PARTIAL`; #187’s issue body explicitly evaluates in-process Typst against
that current architecture. The richer logical resolver in #188 is not needed
to start this spike: #187 can evaluate the existing provider contract. If the
spike later proves that a richer World/resource mapping is structurally
required, that is a new evidence-backed dependency decision, not an assumption
made by #156.

The intended backend direction is:

```text
InProcessBackend  -> preferred default native backend if parity evidence passes
SubprocessBackend -> fallback / transition path initially
```

Removing subprocess execution is not decided by #156 or automatically by
#187; it requires separate parity and migration evidence.

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

- #187 is **now eligible as the immediate next technical spike**;
- #157–#185 are **sequenced after the relevant shared engine/backend/content
  prerequisites**, with parallel work only where the graph permits it;
- #188 is **after #187**, #189 is **after #188**, and #190 is a parallel
  capability-contract band after #187;
- #191 is **deferred/milestone-blocked** to M6/WASM; and
- no issue is authorized to bypass the architecture, evidence, or host
  boundaries recorded here.

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
  points to #157–#191, an explicit defer, or an explicit blocker;
- historical trackers are retained without being mistaken for current
  compatibility proof;
- the conformance corpus, docs, manifests, and backlog share the same target
  revision and layer-specific claims; and
- residual gaps and the dependency-aware order are explicit.

This is **closure-ready evidence for #147 after this branch/PR is merged and
its checks pass**. #147 remains open and is not automatically closed by this
change.
