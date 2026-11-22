# #719 G1 — residual runtime / IR bridge inventory

- **Status:** refreshed after PR #819 exact-head compiler evidence; **G1 exit proof remains pending**
- **Protected baseline:** `main@76a475d9676bf7133a003be2283e33a20adb48c0`
- **Authority:** [issue #719](https://github.com/luceat-lux-vestra/arkst/issues/719), [ADR-0022](../adr/0022-runtime-final-document-ir.md), [G0 surface ledger](719-g0-audit.md)
- **Purpose:** distinguish evaluator-owned runtime inspection from explicit legacy/final IR emission boundaries. Absence of search results is not accepted as proof for large source files.

## Evidence correction

The earlier G1-exit audit incorrectly treated `evaluator.rs` as a zero-projection production baseline. The connector's ordinary file/search path returned incomplete data for the large evaluator source, and that absence was mistakenly interpreted as evidence.

PR #819 CI #2416 / run #44931779968 invalidated that conclusion with compiler diagnostics after the generic projection helpers were test-confined. The exact evaluator blob was then read through the Git blob API and showed 61 production references to `semantic_value`, `to_ir_value`, or `into_ir_value`.

That historical audit result is superseded. CI #2416 is retained as immutable evidence of the inventory defect.

## Current G1 ownership model

### A1 — runtime inspection: must remain RuntimeValue-native

Runtime semantics must not project through `IrValue` merely to inspect a value. PR #819 now migrates these evaluator consumers to typed RuntimeValue operations:

- sort-key Number/String/Boolean classification;
- explicit-error rollback/owner detection;
- runtime source-span recovery;
- dynamic table sort-order classification;
- table Content inspection;
- code callout/text evaluation;
- document-theme scalar validation;
- `.includeall` String adaptation;
- collection index and repeat-count Number inspection;
- table-compute deterministic text projection;
- iterable child-span evidence;
- `.extend` target-name classification;
- page-format multi-size text inspection;
- collection `asDouble` and collection equality semantics;
- scalar Boolean inspection;
- evaluated Markdown-list detection for variable assignment.

RuntimeValue now exposes only the typed access required by those consumers, including Number/Boolean/Component/Collection access, text identity, source-span recovery, and collection-specific equality semantics.

**Candidate source invariant:** production `evaluator.rs` contains zero calls to `.semantic_value()`, `.to_ir_value()`, `.into_ir_value()`, or `RuntimeValue::into_ir_value`.

### A2/C — explicit transitional IR emission boundaries

Some paths still have an `IrValue` contract because later mandatory gates have not yet replaced the mixed/executable IR model. These are not runtime-inspection consumers.

They use the deliberately explicit names `to_legacy_ir_value` / `into_legacy_ir_value` and are grouped as follows:

| Boundary family | Why IR still exists | Required owner |
| --- | --- | --- |
| legacy scope/outcome adapters | compatibility evaluator surfaces still return `IrValue` | G3/G4/G7 |
| block/inline/content materialization fallback | final document materializers still consume mixed `IrValue` | G4/G6 |
| table-cell / flow-join materialization | current document-node materializers accept legacy semantic values | G4/G6 |
| numbering/localization/authors/keywords state | DocumentState remains IR-shaped | G5 |
| target resolution / chain native argument array | legacy executable invocation pipeline remains IR-shaped | G3/G4 |
| evaluate/preserve positional and named values | unresolved/executable preservation still serializes mixed IR | G3/G4/G7 |
| owned collection projection helper | legacy collection-output callers still require `Vec<IrValue>` | G3/G4/G6 |

At the current PR #819 candidate there are 26 explicit legacy projection call sites in production evaluator code: 2 borrowed projections, 22 consuming projections, and 2 consuming function references.

These boundaries must not be reused for ordinary runtime inspection. Their retirement belongs to the later gate listed above.

## B — allowed semantic leaf sharing at G1 exit

The crate that defines a data type does not alone determine evaluator ownership. Immutable backend-neutral leaf payloads may remain shared where they carry no execution state:

- `IrRange`
- `IrSize`
- `IrColor`
- `IrEnumValue`
- `IrComponent`
- already-evaluated semantic `IrNode` content carried by RuntimeValue during this migration stage

G1 does not introduce duplicate RuntimeRange/RuntimeSize/etc. wrappers merely to remove an `arkst-ir` type name.

## C — later-gate migration debt

The following remain intentionally outside G1 completion:

| Boundary | Current purpose | Required owner |
| --- | --- | --- |
| `RuntimeValue::new(IrValue, origin)` / `static_value(IrValue)` | one-way ingress from legacy IR execution/facade paths | G3/G4/G7 |
| callable capture reconstruction from `IrCallableCapture` | one-way legacy callable ingress | G4 |
| `RuntimeCallable.body: Vec<IrNode>` | callable bodies still execute through mixed IR | G4 |
| `RuntimeInlineBody::{content, body}: Vec<IrNode>` | contextual body carrier before direct nested AST execution | G4 |
| `numbering::format_from_value(&IrValue)` | document metadata still uses legacy IR-shaped state | G5/G6 |
| `ast_to_ir` and the core compatibility facade | pre-evaluation/executable IR route | G3/G4/G7 |
| executable `IrNode::{FunctionCall, ChainedFunctionCall, FunctionDeclaration}` | transitional execution model | G3/G4/G6 |
| `IrInline::{DirectiveCall, ChainedDirectiveCall}` | transitional nested executable model | G4/G6 |
| durable `IrValue::{Callable, InlineBody}` and executable wire variants | mixed final/runtime schema | G6/G7 |
| Typst handling of executable/mixed IR | negative baseline until final-only IR | G6 |

## PR #819 candidate acceptance

Before the PR can be qualified, authoritative read-back and CI must establish all of the following:

- no production evaluator use of the old generic projection names;
- A1 runtime inspection is implemented through RuntimeValue-native accessors/helpers;
- every remaining production RuntimeValue→IrValue projection is explicitly named as a legacy boundary and belongs to an A2/C family above;
- production builtins still have no generated `IrValue -> RuntimeValue` immediate wrapper;
- conversion-generated scalar/domain/content/callable values remain runtime-native;
- InlineBody→Callable target conversion does not round-trip through `IrCallable`/`IrValue`;
- origin/provenance behavior remains unchanged at fresh materialization boundaries;
- formatting, stable/MSRV build, clippy, platform tests, WASM, compatibility/oracle checks and required Merge Gate pass on one exact candidate HEAD.

Historical failures remain evidence:

- `f8ed58bb40db6c52c869e85e2f4b5a5c1eb63503` / CI #2415 / run #44929119886 — malformed `LoremIpsum` delimiter;
- `de521ce90d3c02c6f0f824786721fb6ce2c87411` / CI #2416 / run #44931779968 — evaluator projection inventory defect exposed by compilation.

## Exit sequencing

1. authoritative candidate read-back;
2. exact-head CI qualification once;
3. deterministic remediation only if that exact run fails;
4. guarded squash merge only after qualification;
5. post-merge reconciliation;
6. fresh G1 exit proof against protected main.

Only the final proof may mark G1 complete. Later-gate C debt remains open by design.
