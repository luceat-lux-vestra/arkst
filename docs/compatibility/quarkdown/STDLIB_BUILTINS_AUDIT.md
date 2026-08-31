# Quarkdown v2.5.1 Standard-Library and General Builtins Audit

## Audit identity

- Issue: [#151](https://github.com/luceat-lux-vestra/scribium/issues/151)
- Parent: [#147](https://github.com/luceat-lux-vestra/scribium/issues/147)
- Target: Quarkdown v2.5.1
- Pinned upstream commit:
  [107ec3a9482f10d6f90d7580f8409b46a719d18e](https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e)
- Scribium audit base: d049c64934bc42b81bd859d0c70667681718afa2
- Review date: 2026-08-25
- Clean-room policy: public documentation, public API/source declarations,
  release metadata, and independently authored probes/fixtures are allowed.
  Upstream implementation code, tests, and fixtures were not copied,
  translated, or used as Scribium implementation.

This is the canonical #151 inventory. The exact public inventory and source
locations are in
[STDLIB_BUILTINS_AUDIT_MANIFEST.tsv](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv).
Every manifest source URL contains the full pinned SHA; tag-only and main
branch links are not canonical evidence for this audit.

## Scope and ownership

The audit covers the complete pinned public callable surface, then gives #151
canonical treatment to general text, numeric, logical, optionality,
collection, dictionary, inspection, localization, logging, and utility
families. It does not implement or redefine any of them.

| Surface discovered during the sweep | Canonical owner |
|---|---|
| call syntax, argument shape, parser representation, and provenance | #148 |
| value taxonomy, none, binding, target conversion, diagnostics, and commit atomicity | #149; follow-ups #165–#167 |
| lazy bodies, callbacks, callable scope, iteration, let, otherwise, ifpresent, takeif, extend, and super flow | #150; follow-up #169 for extend/super |
| document metadata and document state | #152 |
| layout, configuration, presentation, numbering, and navigation | #153 |
| content, media, Markdown extensions, node, and backend/materialization fidelity | #154 |
| filesystem, project, data, resource, environment, and target-specific loading | #155 |

A callable owned by another audit is still present in the complete manifest.
It is NOT_APPLICABLE to #151's canonical status count rather than silently
omitted or reclassified as a general builtin. none is inventoried here
because it is the value producer paired with isnone, but its value meaning is
canonical in #149. otherwise, ifpresent, and takeif are inventoried here,
while callback and evaluation-flow semantics remain canonical in #150.

The #147 vocabulary used by this document is exactly:
SUPPORTED_END_TO_END, SUPPORTED_SEMANTICS, PARSED_ONLY, PARTIAL,
UNSUPPORTED, DEFERRED, BLOCKED, NOT_APPLICABLE, and UNKNOWN. No additional
compatibility status is introduced.

## How the pinned surface was constructed

The sweep started with the upstream registration mechanism, not with the
existing Scribium gap list:

1. [Stdlib.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Stdlib.kt#L19-L58)
   registers 27 stdlib modules through MultiFunctionLibraryLoader and loads
   lib/localization.qd as localization data after registration.
2. [QFunction.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-native-library-processor/src/main/kotlin/com/quarkdown/processor/annotation/QFunction.kt#L1-L12)
   and [Name.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-native-library-processor/src/main/kotlin/com/quarkdown/processor/annotation/Name.kt#L1-L12)
   define exported callable declarations and public aliases.
3. [MultiFunctionLibraryLoader](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/library/loader/MultiFunctionLibraryLoader.kt#L1-L27)
   is the registration path for module declarations.
4. Every QFunction declaration under quarkdown-stdlib/src/main/kotlin was
   enumerated recursively, aliases were resolved, and exact
   parameter/return declarations were recorded. The result is 162 unique
   public names.
   A separate search of quarkdown-core and the other modules found no second
   public QFunction registration source; core value/conversion declarations
   are type/value dependencies, not additional callable names.
5. [QdLibraryExporter](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/external/QdLibraryExporter.kt#L1-L26)
   was inspected separately. The only stdlib resource loaded during
   registration is the localization table; it does not add an unrecorded
   QFunction declaration.
6. super was checked in
   [Flow.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt#L300-L387)
   and [FunctionExtension.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/FunctionExtension.kt#L20-L70).
   It is an implicit callable installed by extend, not a separate QFunction
   row; #169 remains its implementation follow-up.

The public documentation cross-check includes the
[Iterable wiki](https://quarkdown.com/wiki/iterable/) and
[Typing wiki](https://quarkdown.com/wiki/typing/). The Iterable page confirms
the relevant ordered collection, Pair, Dictionary, and Range categories; the
Typing page documents invocation-time conversion and conversion failure. The
pinned source declarations and manifest remain the authoritative signature
evidence.

### Inventory totals

- Total unique public QFunction names discovered: 162
- #151 general inventory: 60
- Cross-owned/excluded from #151 canonical semantics: 102
- Standalone upstream map/filter declarations: 0; current Scribium map and
  filter are extensions and are not counted in the 162.
- Standalone upstream super declaration: 0; it is the implicit extend
  callable described above.

The canonical ownership reconciliation with #152 retains `localization` and
`localize` in this #151 general stdlib inventory. #152 records both names only
as `NOT_APPLICABLE` ownership handoffs, so the #151 total remains 60 and its
10-name `UNSUPPORTED` count remains unchanged. The standard-library initial
state is also not an empty localization map after registration: `Stdlib` loads
`/lib/localization.qd`, whose `.localization name:{std}` call seeds the standard
translation table before user functions execute.

## Canonical #151 matrix

The manifest gives each row's exact v2.5.1 signature and pinned source
location. The family records below provide the observable contract and the
Scribium comparison fields that are not expressible in a short TSV row:
binding/conversion, evaluation order, laziness/callback behavior, return and
value representation, failure and diagnostics, precedence, implementation
location, architecture owner, and existing evidence.

### Numeric and arithmetic

Upstream public names and signatures are sum(a: Number, b: Number),
subtract(a: Number, b: Number), multiply(a: Number, by: Number),
divide(a: Number, by: Number), rem(a: Number, b: Number),
pow(base: Number, to: Number), abs(x: Number), negate(x: Number),
sqrt(x: Number), logn(x: Number), pi(), sin(x: Number), cos(x: Number),
tan(x: Number), truncate(x: Number, decimals: Int), round(x: Number), and
iseven(x: Number). The exact declaration links are the sum through iseven
rows in the manifest, sourced from
[Math.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt#L20-L208).

- Binding/conversion: upstream Number parameters are invocation-time numeric
  conversions; named parameters are by, to, decimals, and the source names
  shown in the manifest. Scribium routes the regular scalar inventory through
  builtins.rs and shared origin-aware scalar conversion.
- Evaluation: ordinary arguments are evaluated left-to-right before the
  operation; no callback or lazy body is part of this family. Bodies and
  excess/unknown named arguments fail rather than being ignored.
- Return/value representation: upstream numeric wrappers become
  backend-neutral IrValue::Number; iseven returns IrValue::Boolean. No
  Typst-specific concept participates.
- Failure contract: missing/extra/invalid named arguments and invalid numeric
  conversion fail at the call/binding or scalar-conversion layer.
  Operation-domain behavior remains operation-specific; Scribium does not
  fabricate a text fallback.
- Diagnostics/provenance: Scribium emits source-backed E3001/E3003
  diagnostics through the existing evaluator path and preserves the call
  span. Upstream diagnostic wording is not a byte-for-byte compatibility
  requirement.
- Native/source precedence: regular scalar dispatch remains in the engine's
  existing native path, with source-defined lookup/precedence rules owned by
  #150. No new precedence rule is introduced here.
- Scribium evidence/owner/status: crates/scribium-engine/src/builtins.rs
  owns the regular scalar specification and evaluation; existing numeric unit
  cases cover conversion, rounding, domain, and failure behavior. The
  independent #151 manifest guard covers surface identity.
  Status: SUPPORTED_SEMANTICS for 17 names; no claim of
  SUPPORTED_END_TO_END is made.

### Dynamic range

Upstream signature is range(from: Number? = null, to: Number? = null) ->
ObjectValue<Range>, recorded at pinned
[Math.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt#L219-L230).

- Binding/conversion: nullable numeric bounds are evaluated and converted;
  omitted bounds are distinct from an explicitly supplied none at the
  value/conversion boundary owned by #149.
- Evaluation and return: range creates a typed Range value. It does not
  eagerly turn the range into arbitrary text. Consumers materialize it through
  the existing iterable path, with current finite materialization/depth
  limits.
- Failure contract: invalid bounds, unsupported materialization, and limits
  fail with source-backed diagnostics. Empty and open-ended behavior remain
  distinct from a fabricated empty collection.
- Scribium representation/owner: IrValue::Range and
  Evaluator::materialize_range in scribium-engine; no backend involvement.
  Existing range tests cover bounded iteration and limits.
  Status: PARTIAL. The typed bounded slice is present, while the full
  v2.5.1 range/open-bound and downstream-consumer contract is not promoted.

### Text and String

Upstream public names and signatures are string(value: String),
concatenate(a: String, with: String, if: Boolean = true),
uppercase(string: String), lowercase(string: String),
capitalize(string: String), isempty(string: String),
isnotempty(string: String), startswith(string: String, prefix: String,
ignorecase: Boolean = false), and plaintext(content: InlineMarkdownContent)
-> StringValue. Exact rows and pinned locations are in the manifest and
[Strings.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt#L20-L151).
The case strategy is pinned in
[StringCase.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/StringCase.kt#L27-L32).

- Binding/conversion: scalar names use existing invocation-origin-aware
  String/Boolean conversion. concatenate evaluates all regular arguments
  before its Boolean result is applied; it is not a lazy conditional
  callback.
- Return/value representation: scalar results are typed IrValue::String or
  IrValue::Boolean. plaintext is content projection: it consumes already
  parsed inline content and returns plain text; it must not reparse arbitrary
  string text as Markdown.
- Failure contract: wrong value shapes, missing/excess/named arguments,
  bodies, rich content passed to scalar conversion, and invalid Boolean
  conversion fail at the existing binding/evaluator layer. The current
  plaintext path rejects unsupported content materialization instead of
  silently executing nested calls.
- Implementation/precedence: regular scalar names are in
  scribium-engine/src/builtins.rs; frontend content and IrValue::Content
  remain shared engine/IR concerns. Existing source-defined/native precedence
  is not changed.
- Evidence/status: existing scalar and content tests cover whitespace
  preservation, Unicode case behavior, conditional concatenation, prefix case
  handling, conversion errors, and plain-text projection. The eight scalar
  transforms are SUPPORTED_SEMANTICS. `.capitalize` uses the pinned Unicode
  Kotlin `Char.titlecase()` contract: full uppercase mappings use the first
  scalar followed by lowercase of the remaining scalars, while one-scalar
  results use simple titlecase with the JVM uppercase fallback. `.startswith`
  uses direct Unicode 13 simple upper/lower mappings for Kotlin/JVM-compatible
  character-wise comparison without whole-string case conversion or
  normalization. Independent `ǳ`/`ǲ`, `ᾀ`/`Ἀι`, `ŉ`, Greek `ς`/`Σ`, long-s,
  sharp-s, dotted-I, and decomposed-prefix cases exercise those contracts.
  plaintext is PARTIAL because the full upstream
  InlineMarkdownContent/body and output contract is broader than the current
  bounded carrier.
- The engine uses the pinned `unicode-case-mapping = 0.2.0` UCD 13.0 full
  mapping table plus a generated engine-local UCD 13.0 simple mapping table
  from `UnicodeData.txt` fields 12–14. This matches the Unicode data used by
  the pinned Quarkdown JVM 17 runtime and keeps full/simple mappings distinct.
  Rust stdlib has no titlecase/full mapping API and the existing workspace
  dependencies provide no suitable implementation. The narrow dependency and
  generated table are used only by the engine, have no locale or host
  capability, and compile-time guards reject mapping-version drift.

### Boolean, comparison, and optionality

Upstream public names and signatures are islower(a: Number, than: Number,
orequals: Boolean = false), isgreater(a: Number, than: Number,
orequals: Boolean = false), equals(a: DynamicValue, to: DynamicValue),
not(value: Boolean), none(), isnone(value: DynamicValue),
otherwise(value: DynamicValue, fallback: DynamicValue),
ifpresent(value: DynamicValue, mapping: Lambda), and
takeif(value: DynamicValue, condition: Lambda). The pinned declarations are
in the manifest and
[Logical.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt#L18-L80)
and
[Optionality.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt#L18-L123).

- Binding/conversion: numeric comparisons use the existing Number target;
  equals preserves DynamicValue equality at the value boundary; not requires
  Boolean. isnone is a one-argument non-callback predicate and is explicitly
  part of the #151 general inventory. none is the value producer whose
  taxonomy is canonical in #149.
- Evaluation/laziness: islower, isgreater, equals, not, and isnone are
  ordinary scalar calls. otherwise, ifpresent, and takeif have callback/default
  evaluation and state effects owned by #150; this document records their
  signatures and cross-audit relationship only.
- Return/value representation: Boolean predicates return IrValue::Boolean;
  none returns IrValue::None; default/callback operations return the existing
  typed dynamic/IR value without coercing it to text.
- Failure/diagnostics: invalid comparison/value shapes, Boolean conversion,
  callback failures, wrong arity, and named/body misuse fail through existing
  conversion/evaluator diagnostics. Callback rollback and commit behavior are
  not duplicated here.
- Implementation/owner/status: builtins.rs owns the four comparison/logical
  scalar predicates and isnone; evaluator optionality dispatch owns the
  callback family. islower, isgreater, equals, not, and isnone are
  SUPPORTED_SEMANTICS. otherwise, ifpresent, and takeif are PARTIAL pending
  the #150 canonical callback contract. none is NOT_APPLICABLE to #151's
  semantic status because #149 owns its value meaning.

### Collections and iterable access/aggregation

Upstream public names and signatures are getat(from:
Iterable<OutputValue<*>>, index: Int, orelse: DynamicValue =
DynamicValue(NOT_FOUND)), first(from: Iterable<OutputValue<*>>),
second(from: Iterable<OutputValue<*>>), third(from:
Iterable<OutputValue<*>>), last(from: Iterable<OutputValue<*>>),
size(of: Iterable<OutputValue<*>>), sumall(from:
Iterable<OutputValue<*>>), average(from: Iterable<OutputValue<*>>),
distinct(from: Iterable<OutputValue<*>>), reversed(from:
Iterable<OutputValue<*>>), and groupvalues(from:
Iterable<OutputValue<*>>). Exact return declarations and pinned lines are in
the manifest and
[Collection.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt#L25-L212).

- Binding/conversion: collection arguments use the shared iterable target;
  getat uses one-based Quarkdown indexing and orelse fallback. size names its
  operand of; the other functions use from. The current coerce_iterable path
  accepts typed Collection, Pair, Dictionary-entry, and Range values.
- Evaluation/order: input order is preserved for access, reverse, distinct,
  and grouped values as represented by the pinned contract. Aggregates use
  pinned numeric conversion behavior; invalid materialization and limits fail
  before a fabricated result.
- Return/value representation: access returns the existing element or
  IrValue::None/fallback; size, sumall, and average return Number;
  distinct/reversed/groupvalues return typed collections. Group members and
  dictionary entries remain typed rather than debug strings.
- Failure contract: missing/extra/named argument failures, non-iterable
  values, out-of-range index, invalid fallback binding, empty average,
  unsupported nested values, and materialization limits are explicit failure
  or typed None according to the individual contract. Current diagnostics
  preserve the call or value source span.
- Implementation/evidence/status: collection access is evaluator-owned in
  evaluate_collection_access; coerce_iterable, aggregate helpers, and IR
  collection values are the shared bounded path. Existing tests cover
  one-based access, empty/out-of-range fallback, asDouble-compatible
  aggregation, distinct/group order, Pair/Dictionary/Range adaptation, and
  limit failures. These 11 names are SUPPORTED_SEMANTICS.

### Selector-based sorting and higher-order helpers

Upstream public signature is sorted(from: Iterable<OutputValue<*>>,
by: Lambda? = null) -> IterableValue<OutputValue<*>>, pinned at
[Collection.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt#L155-L181).

This is selector/key semantics, not a comparator (left, right) -> Boolean
API:

- without by, each element is converted to its natural comparable value;
- with by, the callback is invoked once per source element, in source order,
  and its result is converted to a comparable key;
- equal keys retain stable ordering;
- keys must be comparable, and current Scribium rejects heterogeneous key
  kinds and None rather than ordering them by debug text;
- invalid selector results and callback failures propagate and restore the
  existing variable snapshot; no partial sorted value is committed.

Scribium implements the bounded path in evaluate_collection_transform:
typed iterable coercion, selector callback invocation, stable key ordering,
typed collection return, source-backed failure, and callback-state rollback.
Callback evaluation/scoping is linked to #150 and conversion ownership to
#149. Existing independent cases cover natural sort, selector sort, one
callback per element, stable ties, heterogeneous keys, None keys, and
callback failure. Status: PARTIAL. The bounded selector path is evidenced,
but the complete upstream DynamicValue/callback/conversion contract is not
promoted. Scribium map/filter use the same evaluator machinery but are
extensions and do not enter the pinned upstream inventory.

### Pair and Dictionary

Upstream public signatures are pair(first: DynamicValue, second: DynamicValue)
-> PairValue<*, *>, dictionary(dictionary: Map<String, OutputValue<*>>) ->
DictionaryValue<*>, and get(dictionary: Map<String, OutputValue<*>>, key:
String, orelse: DynamicValue = DynamicValue(NOT_FOUND)) -> OutputValue<*>;
pinned evidence is in the manifest and
[Dictionary.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Dictionary.kt#L25-L64)
and the Pair declaration in
[Collection.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt#L215-L224).

- Binding/conversion/evaluation: Pair and Dictionary values are recursively
  typed dynamic values; construction validates arity/body/named shape before
  committing the value. Dictionary lookup is a separate key/fallback
  operation and is not implied by construction.
- Return/value representation: IrValue::Pair and IrValue::Dictionary retain
  nested values and source spans; dictionary iteration exposes typed
  key/value Pairs through the shared iterable adapter.
- Failure/diagnostics: wrong arity, unknown names, bodies, non-iterable
  dictionary shape, invalid key conversion, and missing key behavior are
  separate cases. get must not be treated as supported merely because
  dictionary constructs a dictionary.
- Scribium status: pair and dictionary are evaluator-owned typed
  constructors with existing tests and are SUPPORTED_SEMANTICS. get is
  UNSUPPORTED: the pinned declaration exists, but the current evaluator
  dictionary native-owner inventory contains construction and has no get
  dispatch. The bounded implementation owner is [#194](https://github.com/luceat-lux-vestra/scribium/issues/194);
  this audit records the gap and does not implement it.

### Library inspection, localization, and logger utilities

Upstream public signatures are libexists(context: Context, name: String),
functionexists(context: Context, name: String), libraries(context: Context),
libfunctions(context: Context, libraryName: String),
localization(context: MutableContext, name: String, merge: Boolean = false,
contents: Map<String, DictionaryValue<OutputValue<String>>>),
localize(context: Context, key: String, separator: String = ":"),
log(message: String), debug(message: String), and error(message: String).
Exact pinned rows are in the manifest and
[Libraries.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Libraries.kt#L25-L84),
[Localization.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Localization.kt#L95-L152),
and [Logger.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logger.kt#L10-L35).

These are general-language public declarations, not scalar aliases that can
be inferred from existing arithmetic dispatch. Their observable contracts
include context/library lookup, localization table mutation/read, and
host/logging side effects. Scribium has no approved equivalent native owner
for these names in the current engine. No evaluator, host capability,
logging, or localization implementation was added. Status: UNSUPPORTED for
all 9 names. Their bounded ownership is now explicit: [#195](https://github.com/luceat-lux-vestra/scribium/issues/195)
owns library inspection, [#196](https://github.com/luceat-lux-vestra/scribium/issues/196)
owns localization table mutation/lookup, and [#197](https://github.com/luceat-lux-vestra/scribium/issues/197)
owns logger/diagnostic behavior. Host/resource policy is coordinated through
#188/#190; these are real production gaps, not evidence-only omissions.

## Cross-owned public surface

The following 102 public declarations were discovered and recorded in the
manifest but are NOT_APPLICABLE to #151 canonical semantics. The lists are
complete by owner; exact signatures and pinned source lines remain in the
manifest.

- #150: if, ifnot, foreach, repeat, function, extend, var, let (and implicit
  super as the #169 follow-up boundary).
- #152: doctype, docname, docdescription, docauthor, docauthors, dockeywords,
  doclang, theme.
- #153: numbering, nonumbering, font, paragraphstyle, captionposition,
  texmacro, pageformat, pagemargin, footer, currentpage, totalpages,
  formatpagenumber, resetpagenumber, lastheading, autopagebreak,
  noautopagebreak, marker, navigation, tableofcontents, slides, fragment,
  speakernote.
- #154: cite, emoji, allemojis, html, htmloptions, css, cssproperties, icon,
  container, align, center, float, row, column, grid, landscape, fullspan,
  whitespace, clip, box, todo, collapse, textcollapse, numbered, table,
  markdown, mermaid, xychart, subdocumentgraph, node, keybinding, heading,
  paragraph, link, image, pagebreak, code, math, figure, ref, tablesort,
  tablefilter, tablecompute, tablecolumn, tablecolumns, tablebyrows, text,
  br, codespan, match, loremipsum.
- #155: bibliography, read, pathtoroot, listfiles, filename, json, csv,
  include, includeall, subdocument, env, llmstxt, filetree.

llmstxt is an important correction to previous seed material:
[Html.kt](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L188-L210)
contains a public QFunction with content: String and markdownavailable:
Boolean. It is not absent from v2.5.1; its target-specific output and
host/configuration contract remain #155-owned.

## Current Scribium pipeline comparison

The audit traced the current path where evidence was available:

scribium-markdown/scribium-quarkdown frontend
-> existing call/binding representation
-> scribium-engine conversion and evaluator
-> backend-neutral IrValue/IrComponent/content
-> existing scribium-typst lowering where an owning component exists.

- Frontend/parser: call lexing and parsed argument/body representation remain
  #148 evidence. #166 adds the source-backed raw-body boundary beside that
  representation; it does not redefine call lexing or claim malformed-input
  recovery parity.
- Binding/conversion: crates/scribium-engine/src/value_conversion.rs and the
  invocation-origin path provide bounded scalar/enum/iterable conversion.
  Generalized conversion, origin semantics, diagnostics, and atomicity remain
  #149/#165–#167.
- Regular builtin dispatch: crates/scribium-engine/src/builtins.rs contains
  the centralized scalar REGULAR_BUILTINS inventory and typed IrValue
  results. It does not contain cross-owned component/resource functions.
- Evaluator dispatch: crates/scribium-engine/src/evaluator.rs owns
  collection access, selector transforms, Pair/Dictionary construction,
  optionality callbacks, resource boundaries, document state, and source
  precedence. The current Dictionary native-owner list has dictionary but not
  get, which is why get remains UNSUPPORTED and is assigned to #194.
- Return representation: current paths use IrValue Number, String, Boolean,
  None, Range, Collection, Pair, and Dictionary rather than backend-specific
  strings. Content-producing and component-producing functions are not
  pulled into #151.
- Failure/commit: call failures retain source spans and do not fabricate
  values. Existing callback/sort mutation snapshots and evaluator limits are
  evidence; canonical control-flow ownership remains #150.
- Source-defined/native precedence: current lookup and native dispatch
  ordering is preserved. The #151 audit records precedence dependencies; it
  does not introduce name-local exceptions or alter the existing rule.
- Backend: no general builtin requires a Typst workaround in this audit.
  Existing output behavior for cross-owned components is #153/#154 evidence.

## Failure-semantics audit

Success-only inventory was rejected. The following failure classes were
checked against pinned declarations and current pipeline evidence:

| Failure/input class | Pinned contract to preserve | Current Scribium audit result |
|---|---|---|
| missing required, excess, or invalid named argument | binder rejects the call; no silent default unless optional | regular scalar and bounded native paths reject with source-backed diagnostics; generalized binder gap remains #149 |
| invalid scalar/number/Boolean/String conversion | invocation-time conversion fails at conversion layer | shared bounded conversion fails closed; broad target conversion remains #149 |
| wrong value shape or rich content used as scalar | typed target conversion does not fabricate debug text | current scalar helpers reject collections/ranges/components; content projection is explicit |
| empty iterable, out-of-range index, or missing dictionary key | access returns the declared None/fallback shape; other operations retain operation-specific empty behavior | access/fallback and empty aggregate cases are tested; #194 owns the missing `.get` lookup contract |
| incompatible comparison or unsupported natural order | comparison/sort fails rather than inventing an order | typed comparisons and sorted key checks fail with source spans |
| callback or nested callback failure | failure propagates; callback state must not partially commit | existing evaluator callback/sort snapshots cover bounded paths; canonical flow is #150 |
| unknown library/function lookup | lookup failure is observable, not an empty successful result | #195 owns the UNSUPPORTED library-inspection family; no fake result is added |
| arithmetic domain/error condition | numeric operation behavior is retained per operation | existing numeric tests cover bounded f64 behavior; no universal error rewrite is claimed |
| invalid selector result | selector result must be comparable and conversion-valid | sorted rejects None, unsupported, and heterogeneous keys and restores state |

Diagnostics are compared by failure layer, success/failure, provenance,
side-effects, and deterministic category. Exact upstream message bytes are not
copied. Current engine categories include source-backed E3001/E3003 and
materialization E3005 where applicable.

## Independent audit evidence

- [STDLIB_BUILTINS_AUDIT_MANIFEST.tsv](STDLIB_BUILTINS_AUDIT_MANIFEST.tsv) is
  the pinned, complete 162-name fixture with exact public signatures, source
  URLs, owner, and #147 disposition.
- [stdlib_builtin_audit.rs](../../../crates/scribium-core/tests/stdlib_builtin_audit.rs)
  independently asserts 162 rows, unique names, full-SHA evidence, 60 #151
  rows, 102 cross-owned rows, and canonical status counts. It performs no
  network access, and also compiles representative scalar, optionality,
  collection/sort, failure, and Unicode string cases through the public
  Scribium facade, including source provenance for successful and failing
  calls.
- Existing engine/evaluator tests are behavior evidence rather than copied
  upstream fixtures: scalar conversion/math/string cases, typed
  Pair/Dictionary/Range/Collection adaptation, access and aggregation,
  selector sorting and callback failure/atomicity, optionality callback
  behavior, and source-backed diagnostics.
- The guard does not assert support merely because a public name occurs in the
  manifest. get (#194), library inspection (#195), localization (#196),
  logger/diagnostic builtins (#197), plaintext, range, sorted, and callback
  dispositions are separate claim checks.

## Canonical classification summary

Counts below are for the 60-name #151 general inventory only. The 102
cross-owned declarations are NOT_APPLICABLE to this matrix and are counted
separately.

| #147 status | Count |
|---|---:|
| SUPPORTED_END_TO_END | 0 |
| SUPPORTED_SEMANTICS | 43 |
| PARSED_ONLY | 0 |
| PARTIAL | 6 |
| UNSUPPORTED | 10 |
| DEFERRED | 0 |
| BLOCKED | 0 |
| UNKNOWN | 0 |
| NOT_APPLICABLE | 1 |

The six PARTIAL names are sorted, range, plaintext, otherwise, ifpresent, and
takeif. The ten UNSUPPORTED names are get, libexists, functionexists,
libraries, libfunctions, localization, localize, log, debug, and error.
The one NOT_APPLICABLE inventory row is none because its value taxonomy
belongs to #149. The 43 SUPPORTED_SEMANTICS rows are bounded engine semantic
claims; none is promoted to SUPPORTED_END_TO_END.

## Corrections and reconciliation

The previous GAP_INVENTORY.md list was a useful seed but mixed general
builtins with #150 and #152–#155 ownership and used prose classifications
that were not the #147 canonical vocabulary. This audit keeps the complete
name sweep while separating the 60/102 ownership boundary.

Important corrections:

- llmstxt was previously described as a candidate/absent name. The pinned
  Html.kt declaration proves it is public in v2.5.1; it is now explicitly
  recorded under #155 and remains outside #151 implementation.
- get is a pinned public Dictionary callable, but Scribium currently has no
  evaluator native owner for it; it is UNSUPPORTED, not inferred from
  dictionary, and its bounded owner is #194.
- isnone is explicitly recovered into the #151 general inventory. none
  remains a #149 value-model boundary, and callback optionality remains #150.
- sorted is selector/key-based with stable ordering evidence; it is not an
  arbitrary comparator API.
- #172 closes the bounded Unicode string-semantics gap: `.capitalize` now
  reproduces Kotlin `Char.titlecase()` over pinned Unicode 13.0 full/simple
  mappings, and `.startswith(ignorecase:true)` now uses the corresponding
  Kotlin/JVM character-wise simple case comparison. Both rows are promoted to
  SUPPORTED_SEMANTICS; no end-to-end output claim is added.
- map and filter are current Scribium extensions, not pinned v2.5.1 stdlib
  declarations.
- float is a pinned Layout declaration and is #154-owned; it is not a
  general builtin.

Reconciliation links:

- #149: value taxonomy, DynamicValue binding/conversion, diagnostics, and
  atomicity are referenced, not redefined; #165–#167 remain existing
  follow-ups.
- #150: callback, lazy body, scope, iteration, source precedence, and
  extend/super semantics are referenced, not duplicated; #169 is reused.
- #152: all document metadata/state names are retained in the manifest and
  excluded from #151 semantics.
- #153: all layout/configuration names are retained in the manifest and
  excluded from #151 semantics.
- #154: all content/media/Markdown/component names, including node, are
  retained in the manifest and excluded from #151 semantics.
- #155: all resource/data/environment names, including llmstxt, are retained
  in the manifest and excluded from #151 semantics.

## Backlog and #156 handoff

Issue #172 closes the cohesive Unicode string-semantics gap. The four
remaining #151 unsupported families are real pinned gaps with bounded owners:
[#194](https://github.com/luceat-lux-vestra/scribium/issues/194) for dictionary
lookup, [#195](https://github.com/luceat-lux-vestra/scribium/issues/195) for
library inspection, [#196](https://github.com/luceat-lux-vestra/scribium/issues/196)
for localization, and [#197](https://github.com/luceat-lux-vestra/scribium/issues/197)
for logger/diagnostic builtins. Implementation order follows the dependency
bands in #156; no implementation is started here.
Existing issues are reused:

- #149 and #165–#167 for value, binding, conversion, diagnostics, and
  atomicity dependencies;
- #150 and #169 for callback/control-flow and extend/super;
- #152, #153, #154, and #155 for the cross-owned public surface.
- #172 for the completed Unicode titlecase and case-insensitive prefix
  semantics slice.

Remaining implementation questions are the full DynamicValue conversion
matrix, exact diagnostics/atomicity deltas for currently bounded semantics,
and sorted selector/conversion edge cases. Dictionary lookup, library
inspection, localization, and logger ownership are no longer open
reconciliation questions; #194–#197 own those bounded contracts and their
host/resource coordination is explicit. This audit does not select the next
implementation or alter the #157–#169 order.

For #156, the usable reconciliation input is:

- pinned public surface: 162;
- #151-owned inventory: 60;
- cross-owned/excluded: 102;
- #151 status counts: 43 SUPPORTED_SEMANTICS, 6 PARTIAL,
  10 UNSUPPORTED, 1 NOT_APPLICABLE, and zero in the other vocabulary
  categories;
- newly recovered omission: isnone as an explicit general predicate;
- corrected prior omission: llmstxt is public, #155-owned, and excluded from
  #151 implementation;
- corrected prior ownership/status: get is public but currently unsupported;
  float is #154-owned; map/filter are extensions;
- completed implementation: #172, bounded Unicode string case/prefix semantics;
- production behavior: the two audited string semantics are corrected and
  promoted at the bounded semantic boundary; broader output claims remain
  unchanged.
