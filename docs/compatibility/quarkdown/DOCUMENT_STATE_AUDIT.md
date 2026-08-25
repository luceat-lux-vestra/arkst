# Quarkdown v2.5.1 document metadata and state audit

Status: complete audit artifact for Issue [#152](https://github.com/luceat-lux-vestra/scribium/issues/152). This document is ready for strict review; it is not an implementation or compatibility-baseline promotion.

## 1. Target, base, and evidence policy

| Item | Pinned value |
|---|---|
| Quarkdown target | [`107ec3a9482f10d6f90d7580f8409b46a719d18e`](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e) (v2.5.1) |
| Scribium audit base | [`1bd8cda073be4194ffce8e9e58ef4cfc4d742be1`](https://github.com/luceat-lux-vestra/scribium/tree/1bd8cda073be4194ffce8e9e58ef4cfc4d742be1), the squash merge of audited PR #171 / completed Issue #151 |
| Parent tracker | [#147](https://github.com/luceat-lux-vestra/scribium/issues/147) |
| Audit manifest | [`DOCUMENT_STATE_AUDIT_MANIFEST.tsv`](DOCUMENT_STATE_AUDIT_MANIFEST.tsv) |

The audit follows Scribium's clean-room compatibility policy. Pinned upstream
source, public documentation, independent fixtures, and current Scribium
behavior are evidence; none is silently substituted for another. Upstream
source was inspected at the full target SHA. Upstream tests were used as
behavioral evidence only and were not copied or translated.

The canonical #147 status vocabulary is used exactly:
`SUPPORTED_END_TO_END`, `SUPPORTED_SEMANTICS`, `PARSED_ONLY`, `PARTIAL`,
`UNSUPPORTED`, `DEFERRED`, `BLOCKED`, `NOT_APPLICABLE`, and `UNKNOWN`.

## 2. Inventory methodology

The pinned source sweep started at public exposure mechanisms rather than at
Scribium's existing builtin table:

1. `@QFunction`, `@Name`, `@LikelyNamed`, `@LikelyBody`, `@Body`, and
   `@Injected` declarations in the stdlib;
2. `Stdlib` module registration and localization registration;
3. the complete `Document.kt` module, including adjacent document/layout
   declarations;
4. `DocumentInfo`, `DocumentType`, `DocumentAuthor`, `DocumentTheme`, locale
   and localization core APIs;
5. default context creation, scope/subdocument context sharing, argument
   binding, body fallback, and function-call failure paths;
6. the #151 162-name manifest as a cross-check only; and
7. current Scribium evaluator, IR, serde tests, fixtures, renderer, and
   front-matter conversion.

The resulting manifest contains 43 discovered public names: 10 owned by #152
and 33 retained as explicit handoffs to #153, #154, or #155. A name is not
omitted because Scribium does not implement it. The manifest is an offline
completeness artifact and is checked by
[`document_state_audit.rs`](../../../crates/scribium-core/tests/document_state_audit.rs).

The sweep found no upstream front-matter parser or front-matter metadata API
at the pinned source. The current Scribium front-matter channel is therefore
recorded as a separate boundary, not inferred to be Quarkdown `DocumentInfo`.

## 3. Ownership boundary

### #152: document metadata and evaluator-owned document state

The #152-owned surface is the identity and descriptive state represented by
`DocumentInfo`: `doctype`, `docname`, `docdescription`, `docauthor`,
`docauthors`, `dockeywords`, `doclang`, and `theme`. The related
document-scoped localization API, `localization` and `localize`, is also
owned by this audit because it stores and reads document-scoped language data,
even though it is not currently represented in Scribium's IR.

### #153: layout and document configuration

The sweep saw `numbering`, `nonumbering`, `font`, `paragraphstyle`,
`captionposition`, `texmacro`, `pageformat`, `pagemargin`, `footer`,
`currentpage`, `totalpages`, `formatpagenumber`, `resetpagenumber`,
`lastheading`, `autopagebreak`, `noautopagebreak`, `marker`, `navigation`,
`tableofcontents`, and `slides`. These are document configuration or layout
state, not #152 metadata semantics. `captionposition` has an existing bounded
Scribium slice, but remains cross-owned and is not assigned a #152 status.

### #154: content/media and target-facing output

The sweep saw `htmloptions`, `fragment`, and `speakernote`. HTML title/base URL
options and slide content primitives are output/content concerns. In
particular, `htmloptions.title` is not a second Quarkdown document-name
getter; it is an HTML output option and is handed to #154.

### #155: resources, project, and environment state

The sweep saw `include`, `includeall`, `subdocument`, `read`, `pathtoroot`,
`listfiles`, `filename`, `json`, `csv`, and `env`. These depend on project,
filesystem, subdocument, or environment capabilities and are handed to #155.

The complete ownership and status record is the manifest. Cross-owned rows
are retained there to prove that they were seen, but their canonical semantics
are intentionally not audited by #152.

## 4. Complete #152 inventory

The following signatures are the public shape at the pinned source. Binding
and conversion details are linked to the canonical [#149 value-model audit](VALUE_MODEL_AUDIT.md); they are not redefined here.

| Surface | Pinned signature and identity | Initial state | Mutation/return contract | Scribium boundary | Status |
|---|---|---|---|---|---|
| `.doctype` | `doctype(type: DocumentType? = null)`; no alias | `PLAIN`; getter returns lowercase enum name | Getter returns text. Setter replaces the type and returns `VoidValue`; upstream enum validation occurs before assignment | Typed evaluator state and IR snapshot; body fallback and renderer effects are not equivalent | `PARTIAL` |
| `.docname` | `docname(name: String? = null)`; no alias | Absent name is observable as `""` | Getter returns name or empty text. Setter rejects blank text, replaces the name, and returns `VoidValue` | Current state uses `String` with empty absence; the upstream regular-body fallback is rejected at the current raw-body boundary | `PARTIAL` |
| `.docdescription` | `docdescription(description: String? = null)`; no alias | Absent description is observable as `""` | Getter returns description or empty text. Setter replaces the description and returns `VoidValue` | Same bounded state/IR boundary and body limitation as `.docname` | `PARTIAL` |
| `.docauthor` | `docauthor(author: String? = null)`; no alias | Empty ordered author list; getter is `""` | Getter returns the first author name or empty text. Setter appends one `DocumentAuthor`; duplicates are preserved; setter returns `VoidValue` | Current append path is bounded to scalar conversion and shares state; full upstream body/conversion contract remains open | `PARTIAL` |
| `.docauthors` | `docauthors(authors: Map<String, DictionaryValue<OutputValue<String>>>? = null)`; no alias | Empty ordered author list | Getter returns an ordered dictionary keyed by author name with nested info. Setter maps entries and appends them; dictionary key collisions cannot represent two entries with the same key | Current validation is pre-commit and ordered, but body/value conversion is bounded; mixed singular/plural append behavior is tested | `PARTIAL` |
| `.dockeywords` | `dockeywords(keywords: Iterable<DynamicValue>? = null)`; `@LikelyBody`; no alias | Empty ordered list | Getter returns ordered strings. Setter replaces the complete list, preserving order and duplicate values; no deduplication is evidenced | Current replacement and duplicate behavior are implemented for bounded iterable/scalar inputs; raw-body and generalized conversion remain gaps | `PARTIAL` |
| `.doclang` | `doclang(language: String? = null)`; no alias | Locale absent; getter returns `""` | Getter returns `locale.localizedName` or empty text. Setter resolves case-insensitive English name or language tag, replaces locale, and returns `VoidValue`; invalid identifiers fail before assignment | Current deterministic checked-in locale table is narrower than the upstream JVM locale universe; raw-body fallback is rejected | `PARTIAL` |
| `.theme` | `theme(color: String? = null, layout: String? = null)`; `layout` is `@LikelyNamed`; no alias | No theme (`null`) before the first call | There is no getter. Every successful call replaces the complete `DocumentTheme`; omitted components become null, supplied strings are lowercased, and the setter returns `VoidValue` | Current `Some(empty)` versus `None` distinction and rollback are evidenced; raw-body fallback and theme resolution/rendering are not | `PARTIAL` |
| `.localization` | `localization(name: String, merge: Boolean = false, contents: Map<String, DictionaryValue<OutputValue<String>>>)`; `contents` is `@Body` | No localization tables | Creates a locale-keyed table. Duplicate table names fail unless `merge=true`; merge appends/replaces entries using the upstream table merge behavior; returns `VoidValue` | No evaluator-owned table state or `localize` implementation exists in the current IR/evaluator | `UNSUPPORTED` |
| `.localize` | `localize(key: String, separator: String = ":")`; no alias | No default table or locale; a lookup is therefore an error | Splits the key at the first separator and returns the localized string; missing locale/table/key fails | No current localization table or renderer-facing localization state | `UNSUPPORTED` |

The two localization rows are distinct: `.localization` mutates a table,
while `.localize` reads a table using the current document locale. They are
grouped under one bounded follow-up because neither has a Scribium state or IR
representation yet.

## 5. Family-level semantic analysis

### Identity, type, and description

Pinned `DocumentInfo` starts with `type=PLAIN` and nullable `name` and
`description`; the public getters turn absent name/description into empty text.
`DocumentType` has four values: `PLAIN`, `PAGED`, `SLIDES`, and `DOCS`.
`numberingOrDefault` is a derived layout concern and is not promoted into
#152. `.docname`, `.docdescription`, and `.doctype` setters replace their
respective fields and have no output value. `.docname` additionally rejects
blank text.

Scribium's `DocumentState` has one evaluator-owned state object with `String`
name/description, a typed document type, and a final immutable snapshot into
`IrDocumentState`. The absence distinction for name/description is intentionally
represented by the documented empty getter contract rather than an IR option.
The current frontend rejects raw block bodies for these calls, while pinned
regular argument binding can fall back from a body to the final regular
parameter. That observable mismatch keeps all three rows `PARTIAL`.

“Document title” is not a separate pinned `DocumentInfo` field. The upstream
`.htmloptions(title=...)` value controls an HTML `<title>` and is cross-owned
by #154. Scribium front matter `title` is likewise a separate metadata/output
channel; it must not be used as proof of `.docname` equivalence.

### Author family

The initial upstream author list is empty. `.docauthor` appends one author and
the getter reads the first name. `.docauthors` converts an ordered dictionary
to author records and appends those records. Each author can contain a nested
string map. Repeated singular calls preserve duplicate author records. A
dictionary cannot preserve duplicate keys, and the upstream dictionary builder
replaces a repeated key in its original ordered slot.

Scribium's evaluator has matching append behavior for valid bounded values,
ordered author/info storage, a typed IR snapshot, and mixed singular/plural
state sharing. It validates all candidates before committing and restores the
pre-call state on failure. That is current Scribium behavior and test
evidence, not a claim that upstream has the same transaction boundary for
nested evaluation. The remaining conversion/body gap is tracked under #149
follow-ups #165, #166, and #167.

### Keywords

The initial upstream keyword list is empty. The getter returns the list in
order. The setter converts the supplied iterable to strings and replaces the
whole list; the pinned implementation does not deduplicate. Repeated calls
therefore discard the old list, while duplicate values within the new list
remain observable.

Scribium's bounded implementation preserves these replacement, ordering, and
duplicate semantics in `IrDocumentState.keywords`. It validates the complete
candidate list before one state commit. It does not claim the upstream
general `DynamicValue`/raw-body conversion surface, so the status remains
`PARTIAL`.

### Locale and localization

The initial upstream locale is null. `.doclang` reads the locale's localized
display name, not its tag. `LocaleLoader.SYSTEM.find` tries a case-insensitive
English display name and then a language tag. The upstream JVM loader exposes
the JVM's available locale set, including regional tags; a failed lookup
throws before `DocumentInfo` is assigned.

The `.localization` table builder resolves each table locale, stores nested
string values, and rejects duplicate table names unless merge is requested.
The merge path gives later entries priority while retaining deterministic table
order. `.localize` requires a current locale unless an explicit locale is
passed through the core context API, then fails for missing table or key.

Scribium deliberately uses a deterministic checked-in locale table rather than
an OS/JVM dependency. It stores canonical tag and localized-name data in the
IR and has no localization-table state. Valid upstream identifiers outside the
checked-in table, raw-body fallback, locale-aware rendering, `.localization`,
and `.localize` are therefore not silently treated as supported.

### Theme

The pinned `DocumentTheme` has nullable `color` and `layout`. Despite a KDoc
sentence suggesting omitted values could be retained, the executable
`.theme` implementation constructs a new theme from the two call arguments on
every call. An empty call is consequently an explicit empty theme, not a
getter and not absence. Theme existence is checked later by rendering code in
upstream, outside this state contract.

Scribium records `Option<IrDocumentTheme>` so “never set” and “set to empty”
survive the IR boundary. Supplied strings are lowercased and each successful
call replaces the whole theme. Raw-body fallback and theme registry/output
consumption remain outside the bounded implementation.

## 6. Initial/default state

| State item | Pinned upstream initial value | Scribium `DocumentState` / IR value | Evidence limit |
|---|---|---|---|
| type | `PLAIN` | `IrDocumentType::Plain` | State semantics only; layout/rendering is #153 |
| name | nullable; getter `""` | `String::new()`; getter `""` | Absence is not preserved as an option because the observable getter collapses it |
| description | nullable; getter `""` | `String::new()`; getter `""` | Same collapsed getter contract |
| authors | empty list | empty `Vec` | Ordered records and nested info survive IR |
| keywords | empty list | empty `Vec` | Ordered duplicates are retained |
| locale | null; getter `""` | `None`; getter `""` | Locale universe is bounded in Scribium |
| theme | null; no getter | `None`; first empty setter becomes `Some(empty)` | Option distinction is serialized |
| localization tables | empty map | no corresponding state | Unsupported, follow-up #173 |

## 7. Binding, mutation, return, and failure model

Binding and conversion are the #149 authority. This audit records only state
effects:

| Family | Mutation | Validation point | Partial mutation evidence | Return |
|---|---|---|---|---|
| type/name/description | replace | enum/name conversion; blank name rejected | Pinned setters assign only after their local validation; no generic transaction around a larger document evaluation is evidenced | Getter text or `VoidValue` setter |
| singular authors | append | author string conversion | Current Scribium validates before append; pinned helper directly assigns the copied `DocumentInfo` | First name or `VoidValue` |
| plural authors | append converted map entries | locale-independent nested map/string validation | Current Scribium validates all entries before one append; repeated dictionary keys are already collapsed by dictionary construction | Ordered dictionary or `VoidValue` |
| keywords | replace | iterable element conversion | Current Scribium builds the replacement list before assignment; duplicates/order preserved | Ordered strings or `VoidValue` |
| locale | replace | name/tag lookup | Pinned assignment follows a successful lookup; current Scribium restores old state if a later nested evaluation fails | Localized name or `VoidValue` |
| theme | replace full object | nullable scalar conversion; upstream render-time existence check | Current Scribium commits one whole object and restores on failure | `VoidValue`; no getter |
| localization table | create or merge | table locale/value conversion and duplicate-name rule | Upstream duplicate-table check and table build precede table assignment; no cross-call transaction is evidenced | `VoidValue` |
| localize | none | locale/table/key lookup | No mutation | Localized string |

The pinned `FunctionCall.execute` validates, binds, and invokes without an
observable `DocumentInfo` snapshot/restore wrapper. Consequently, the audit
does not claim upstream atomic rollback when a nested document-state call has
already mutated state and a later expression fails. Current Scribium has
explicit state snapshots and tests for rollback, which is a stronger local
invariant; equivalence of that nested failure boundary remains limited by
upstream evidence and is covered by #167's generic atomicity work.

## 8. Scope, sharing, and shadowing

The generic callable, lazy-body, scope, and lookup rules remain canonical in
[`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md). The
#152-specific result is:

- upstream `ScopeContext` shares the parent `documentInfo`, while a
  `SubdocumentContext` starts from the parent value and keeps assignments local;
  localization tables are shared by the subdocument context;
- current Scribium callables share exactly one evaluator-owned `DocumentState`
  through the documented explicit state exception; nested writes are visible
  after return and to sibling calls, and the final IR snapshot observes the
  post-evaluation state on success;
- current Scribium's document-specific dispatch preserves native-first
  behavior for `.docname`, `.docdescription`, and `.doctype`, while
  source-defined `.docauthor`, `.docauthors`, `.dockeywords`, `.doclang`, and
  `.theme` may shadow the native handler when a source definition exists; and
- the upstream declarations do not add a separate document-state precedence
  rule. Any general source/native lookup conclusion is therefore owned by #150;
  this document records only the current Scribium consequence and does not
  duplicate or broaden #150's model.

Nested argument evaluation is especially important: upstream regular-body
fallback can evaluate or re-lex content according to the target type, while
Scribium currently rejects the unavailable lossless raw-body form for the
affected setters. The current nested rollback witness is therefore evidence
of Scribium's bounded state guarantee, not evidence of upstream equivalence.

## 9. Current Scribium architecture mapping

The implementation retains the accepted architecture:

- `scribium-engine` owns one evaluator `DocumentState` and performs state
  mutation;
- `scribium-ir` owns one backend-neutral immutable `IrDocumentState` inside
  `IrDocument`; and
- `scribium-typst` does not consume these metadata fields as rendered document
  policy. State semantics and renderer support are consequently separate.

No second state subsystem, generic metadata framework, Typst-specific state
type, production conversion change, or broad evaluator refactor is introduced
by this audit.

## 10. IR and serde analysis

`IrDocumentState` persists name, description, document type, ordered authors,
ordered keywords, optional theme, optional locale, and the cross-owned caption
position. The state is immutable after the evaluator snapshot and is
backend-neutral. `IrDocumentState`, `IrMetadata.document_state`, author info,
theme, locale, and the collection fields all use serde defaults sufficient for
historical serialized IR to decode. Existing tests cover deterministic
round-trips, old state with omitted fields, ordered author/info records,
duplicate keywords, `None` locale, and the distinction between absent theme and
explicit empty theme.

Localization tables are not persisted because no Scribium implementation owns
them. The audit does not add a speculative field or generic serialization
framework. Any future localization representation must be decided with #173
and the #156 reconciliation rather than hidden in this document-only change.

## 11. Renderer and front-matter boundary

Current Typst lowering consumes `IrMetadata.title`, `author`, and `date` as
existing metadata comments, but does not consume `IrDocumentState`. This means
the eight state setters can be `PARTIAL` at the evaluator/IR boundary without
being promoted to `SUPPORTED_END_TO_END`. Conversely, renderer output is not
used to excuse incorrect getter, setter, ordering, or rollback semantics.

Scribium front matter is parsed into `IrMetadata` defaults and overrides for
title/author/date/custom fields. It is not merged into evaluator
`DocumentState`, and no pinned upstream front-matter API was found in the
sweep. The audit therefore makes no precedence or merge claim between front
matter and the runtime builtins. A future front-matter interaction is only in
scope when it is evidenced as part of a #152-owned observable contract.

## 12. Canonical status table

| Status | #152-owned rows |
|---|---:|
| `SUPPORTED_END_TO_END` | 0 |
| `SUPPORTED_SEMANTICS` | 0 |
| `PARSED_ONLY` | 0 |
| `PARTIAL` | 8 (`doctype`, `docname`, `docdescription`, `docauthor`, `docauthors`, `dockeywords`, `doclang`, `theme`) |
| `UNSUPPORTED` | 2 (`localization`, `localize`) |
| `DEFERRED` | 0 |
| `BLOCKED` | 0 |
| `NOT_APPLICABLE` | 0 |
| `UNKNOWN` | 0 |
| **Total #152-owned** | **10** |

The 33 cross-owned inventory rows are all `DEFERRED` to their owning
workstream in the manifest: 20 to #153, 3 to #154, and 10 to #155. They are
not included in the #152 status counts.

## 13. Existing evidence and reconciliation

| Existing slice | Issue / PR / commit | Current evidence and reconciliation |
|---|---|---|
| Document-state foundation | [#107](https://github.com/luceat-lux-vestra/scribium/issues/107), [PR #107](https://github.com/luceat-lux-vestra/scribium/pull/107), merge `c77f346` | One evaluator state and serializable IR snapshot are present; this audit confirms the architecture but does not reopen it |
| `.docauthor` | [#135](https://github.com/luceat-lux-vestra/scribium/issues/135), [PR #136](https://github.com/luceat-lux-vestra/scribium/pull/136), merge `7ef8b48` | Revalidated as append/first-getter semantics with bounded conversion; `PARTIAL` because upstream body/conversion/output boundaries remain open |
| `.docauthors` | [#137](https://github.com/luceat-lux-vestra/scribium/issues/137), [PR #138](https://github.com/luceat-lux-vestra/scribium/pull/138), merge `2b685f0` | Revalidated as ordered nested dictionary append with mixed-family state sharing; `PARTIAL` |
| `.dockeywords` | [#139](https://github.com/luceat-lux-vestra/scribium/issues/139), [PR #140](https://github.com/luceat-lux-vestra/scribium/pull/140), merge `8771bd7` | Revalidated as replacement with ordering and duplicate preservation; `PARTIAL` for body/conversion/output boundary |
| `.theme` | [#141](https://github.com/luceat-lux-vestra/scribium/issues/141), [PR #142](https://github.com/luceat-lux-vestra/scribium/pull/142), merge `bf32038` | Pinned implementation resolves the KDoc ambiguity in favor of whole-state replacement; explicit-empty option survives serde; `PARTIAL` |
| `.doclang` | [#143](https://github.com/luceat-lux-vestra/scribium/issues/143), [PR #144](https://github.com/luceat-lux-vestra/scribium/pull/144), merge `c5d596e` | Revalidated against upstream name/tag lookup and localized-name getter; deterministic locale table is narrower than upstream; `PARTIAL` |
| `.captionposition` | [#145](https://github.com/luceat-lux-vestra/scribium/issues/145), [PR #146](https://github.com/luceat-lux-vestra/scribium/pull/146), merge `247d945` | Seen and retained as #153 layout ownership; no #152 canonical status assigned |
| Previous inventory | [PR #171](https://github.com/luceat-lux-vestra/scribium/pull/171), merge base `1bd8cda` | The 162-name manifest is a cross-check seed; this independent source sweep adds the localization/state distinction and does not inherit its semantics blindly |

Relevant current evidence includes
[`evaluator.rs`](../../../crates/scribium-engine/src/evaluator.rs),
[`locale.rs`](../../../crates/scribium-engine/src/locale.rs),
[`scribium-ir`](../../../crates/scribium-ir/src/lib.rs), the core document-state
tests, and the independent author/keyword/locale/theme fixtures. The focused
Issue #152 witnesses are in
[`document_state_audit.rs`](../../../crates/scribium-core/tests/document_state_audit.rs).

## 14. Residual gaps, follow-ups, and completion statistics

Actionable gaps were searched against existing issues before creating a new
one:

- [#165](https://github.com/luceat-lux-vestra/scribium/issues/165) owns generic
  binder validation and remains frozen until #156;
- [#166](https://github.com/luceat-lux-vestra/scribium/issues/166) owns the
  lossless dynamic/raw-body representation prerequisite and remains frozen
  until #156;
- [#167](https://github.com/luceat-lux-vestra/scribium/issues/167) owns generic
  conversion diagnostics and commit atomicity and remains frozen until #156;
- new bounded [#173](https://github.com/luceat-lux-vestra/scribium/issues/173)
  owns deterministic locale closure plus `.localization`/`.localize` state
  and lookup semantics; and
- layout, content/media, and resource/environment rows remain explicitly
  handed to [#153](https://github.com/luceat-lux-vestra/scribium/issues/153),
  [#154](https://github.com/luceat-lux-vestra/scribium/issues/154), and
  [#155](https://github.com/luceat-lux-vestra/scribium/issues/155).

Every implementation follow-up is frozen until the #156 reconciliation is
complete. This audit does not alter #172 or begin any #157+ implementation.

| Measure | Result |
|---|---:|
| Pinned public names discovered in this sweep | 43 |
| #152-owned | 10 |
| Cross-owned | 33 |
| #153 / #154 / #155 handoffs | 20 / 3 / 10 |
| #152 `PARTIAL` | 8 |
| #152 `UNSUPPORTED` | 2 |
| Other #152 statuses | 0 |
| Production semantics changed | No |

The manifest guard detects duplicate names, malformed rows, invalid statuses,
missing ownership/provenance/evidence, stale target/base declarations, and
declared-total mismatches without fetching the network or adding a production
dependency.
