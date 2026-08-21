# Scribium Codebase Health Audit

**Date:** 2026-08-21
**Audited base:** `0d3c4b78e2dddf8b49a9a7d09515c00aa37e15ef`
**Scope:** repository-wide architecture, correctness, testing, documentation,
security, performance, Rust quality, and maintainability review
**Status:** audit only; no production behavior or test behavior was changed

This is an evidence report, not a refactoring plan disguised as an
implementation PR. File and line references below point at the audited base.
The accepted target architecture and the current physical implementation are
reported separately throughout.

## Executive summary

The current main branch is a clean, well-tested partial compatibility baseline,
but it has reached the point where two gates should precede more cross-cutting
semantic growth:

1. the Quarkdown conformance corpus must enforce the semantic claims in its
   metadata rather than only checking parser diagnostics; and
2. native builtin dispatch/signature ownership must be made explicit before
   another sequence of feature-specific handlers grows the evaluator's manual
   dispatch surface.

The accepted ADR-0015 architecture remains coherent. The current physical
workspace has not yet applied most of it: `scribium-core` still owns the
project model, IR, diagnostics, AST-to-IR conversion, compatibility modules,
and evaluator, while `scribium-typst` also contains the native subprocess
adapter. This was explicitly deferred migration state in the ADRs, not an
unreviewed design change. At the current size and change rate, however,
bounded physical migration should begin before the next set of cross-cutting
semantic slices. A single all-at-once crate split is not recommended.

No P0 correctness, security, or data-loss issue was found in the audited
baseline. Nine findings require action, including seven P1 findings and two P2
maintainability/performance findings. The most concrete latent correctness issue
is that accepted 4- and 8-digit hexadecimal colors validate an alpha channel
but always store `alpha: 1.0`. No current production consumer uses the Color
domain, so this is not an observed document-output regression yet; it must be
resolved before color/style semantics are added.

## Baseline and method

The audit started from the requested base after fetching `origin/main`; the
audit branch was created directly from that SHA. The worktree was clean before
and after evidence collection. The review used:

- ADR-0014, ADR-0015, ADR-0016, ADR-0018, ADR-0019, and ADR-0020;
- `docs/ARCHITECTURE.md`, `docs/ENGINEERING.md`, `docs/PRODUCT.md`,
  `docs/ROADMAP.md`, the compatibility specification, and README;
- Cargo manifests, workspace dependency direction, production source, tests,
  fixtures, and GitHub workflows;
- recent feature history, file-size/production-line measurements, and
  targeted searches for hardcoded names, diagnostics, I/O, unsafe operations,
  and transitional variants; and
- the existing repository validation commands listed below.

The large-file measurements are useful context but are not findings by
themselves. On this base:

| File | Total lines | Bytes | Approximate production boundary | Audit conclusion |
|---|---:|---:|---:|---|
| `crates/scribium-core/src/evaluator.rs` | 12,676 | 449,125 | 8,292 | God-module finding: dispatch, scope, evaluation, materialization, and diagnostics change together. |
| `crates/scribium-core/src/lib.rs` | 3,428 | 141,266 | 195 | Mostly test volume; the production facade is small and cohesive. |
| `crates/scribium-core/src/ast_to_ir.rs` | 2,378 | 89,462 | 1,055 | Migration ownership issue; not a size-only split recommendation. |
| `crates/scribium-core/src/builtins.rs` | 2,200 | 76,753 | 56 | Production surface is a small registry/dispatcher; its test volume should not drive extraction. |
| `crates/scribium-core/src/value_conversion.rs` | 2,398 | 72,330 | 1,759 | Central and cohesive conversion boundary, with one correctness finding below. |
| `crates/scribium-markdown/src/parser.rs` | 3,843 | 138,686 | 2,441 | Large but coherent authoritative Rushdown frontend adapter. |
| `crates/scribium-typst/src/lowering.rs` | 3,001 | 113,943 | 1,184 | Cohesive backend lowering plus dense tests; do not split by LOC. |
| `crates/scribium-cli/src/commands.rs` | 2,520 | 93,338 | 873 | Native host boundary; the responsibilities are appropriate for the current CLI. |

The evaluator growth pattern is architectural evidence, not just file size:
the last semantic commits repeatedly changed the same evaluator/builtin paths
for stacked layout, center, align, container, landscape, whitespace, and br.
That makes dispatch and ownership the immediate concern.

## Findings

### F-001 — P1 — Conformance metadata does not control assertion strength

**Category:** Testing quality, compatibility governance, documentation truthfulness

**Evidence**

- `crates/scribium-test-support/src/lib.rs:42-52` loads a free-form
  `compatibility_level` string into every conformance case.
- `crates/scribium-test-support/src/lib.rs:114-131` implements
  `verify_parses()` by filtering only `E2xxx` parser diagnostics. It does not
  branch on `self.meta.compatibility_level`, and it permits evaluation
  diagnostics even for a case marked above `Parsed`.
- `crates/scribium-test-support/src/lib.rs:135-142` runs every case through that
  parser-only helper and contains the literal
  `TODO: Add more detailed verification based on compatibility_level`.
- The eight currently semantically labelled cases in
  `fixtures/quarkdown-conformance/cases/*/case.toml` use
  `compatibility_level = "Semantically supported"`.
- The semantic tests that do exist at `crates/scribium-test-support/src/lib.rs:207-261`
  assert no diagnostics and a non-empty IR, but do not assert the observable
  semantic result, normalized IR shape, generated Typst, or a backend artifact;
  not every semantic fixture has a corresponding helper test.
- `rg` finds only the definition of `run_all_conformance_cases()`; no test or
  workflow calls it. `.github/workflows/ci.yml:38-73` runs workspace tests and
  example smoke tests, but has no dedicated Quarkdown corpus lane.

**Impact**

The fixture metadata can claim “Semantically supported” while the corpus gate
checks only that parsing did not produce an `E2xxx`. A wrong value, wrong
materialization boundary, or wrong generated output can therefore pass the
conformance corpus. The individual evaluator tests are valuable independent
evidence, but they do not make the corpus metadata truthful or guarantee that
the cases are run as one compatibility suite.

**Recommendation**

Define an explicit assertion policy for `Parsed`, `Semantically supported`,
and any future backend level. Keep parser-only cases parser-only; require
semantic cases to assert independently authored observable expectations, with
source-backed diagnostics and backend checks where the fixture claims them.
Make the all-case runner a real CI test or executable check, and reject unknown
compatibility levels. Keep known divergences explicit rather than weakening the
policy for them.

**Scope:** `scribium-test-support`, conformance fixtures, CI, and compatibility
documentation. No compiler production code is required for the first fix.
**Blocks feature work?:** **Yes.** This should precede #61 and any additional
semantic slice that would otherwise add more claims to a weak corpus gate.

### F-002 — P1 — Native builtin dispatch and signatures are fragmented

**Category:** Evaluator architecture, builtin dispatch, hardcoding,
maintainability

**Evidence**

- `crates/scribium-core/src/evaluator.rs:1331-1770` is a long ordered chain of
  name predicates and bespoke handlers. It includes state/resource builtins,
  variables, user functions, components, ranges, collections, and the scalar
  builtin fallback in one dispatch function.
- The same file has separate predicates at
  `crates/scribium-core/src/evaluator.rs:5633-5659` for `row`/`column`/`grid`,
  `center`, `align`, `container`, `landscape`, `br`, and `whitespace`, with
  additional predicate families later in the file.
- Scalar native names are registered in a second list at
  `crates/scribium-core/src/builtins.rs:16-54` and routed through another
  `match` at `crates/scribium-core/src/builtins.rs:81-117`.
- Component and special builtin binding is separately reimplemented in
  `evaluator.rs`, for example `bind_whitespace_arguments` at
  `5661-5730`, `bind_container_arguments` at `5746-5800`,
  `bind_align_argument` at `5909-5956`, and `bind_stacked_arguments` at
  `5979-6010`.
- Block/inline behavior has a second special-name list at
  `evaluator.rs:1217-1247`. The recent feature commits listed in the audit
  history each add another handler/predicate/binder path.
- One important invariant is already correct: user-defined functions are
  checked before native components at `evaluator.rs:1558-1571`, and tests cover
  shadowing in both contexts. That precedence must remain centralized.

**Impact**

There are multiple places for a new native name to be registered or omitted.
Argument count, duplicate named-argument handling, body policy, lazy
evaluation, inline/block materialization, and source-defined shadowing can
drift between features. A universal binder would also be dangerous: resource,
state, callback, and component semantics intentionally have different lazy and
typed policies.

**Recommendation**

Introduce a narrow, typed inventory for the regular scalar builtin family and
use it to drive one registration/dispatch path and its signature tests. Keep
bespoke evaluator entries for state, resource, callback, collection, and
component builtins. Give each bespoke entry an explicit policy record or test
for body acceptance, inline/block position, evaluation order, and materializer.
Do not create a generic framework for semantics that are not actually regular.
Add a parity test that every native name has exactly one dispatch owner and
that source-defined shadowing remains ahead of native dispatch.

**Scope:** `evaluator.rs`, `builtins.rs`, focused evaluator tests. This can be
implemented without moving crates.
**Blocks feature work?:** **Yes.** Resolve the registry/signature direction
before another series of native component or builtin additions, including #61.

### F-003 — P1 — Physical crate boundaries and Typst execution ownership have reached migration threshold

**Category:** Architecture, dependency direction, host/security boundary

**Evidence**

- The workspace in `Cargo.toml:1-18` contains `scribium-core`, source,
  Quarkdown, Markdown, Typst, CLI, and test-support, but no physical
  `scribium-project`, `scribium-engine`, `scribium-ir`,
  `scribium-diagnostics`, `scribium-compat`, `scribium-html`, or
  `scribium-typst-subprocess` crates.
- `crates/scribium-core/Cargo.toml:9-19` describes and depends on the core as
  composition, semantics, evaluation, IR, source maps, Markdown, Quarkdown,
  and source. The physical modules include `virtual_project`, `ir`,
  `diagnostics`, `compatibility`, `ast_to_ir`, `value_conversion`, and the
  evaluator.
- `crates/scribium-typst/Cargo.toml:9-17` depends directly on
  `scribium-core`. Its `lib.rs:3-8` says the crate owns lowering, the backend
  trait, subprocess adapter, diagnostics, and source maps.
- `crates/scribium-typst/src/backend.rs:1-7` imports `std::fs`, native paths,
  and `std::process::Command`; `SubprocessBackend` at `87-125` creates a
  temporary directory and begins mirroring the project before invoking Typst.
- The target architecture explicitly assigns project/engine/IR/diagnostics
  ownership in `docs/ARCHITECTURE.md:222-243`, forbids native I/O in
  platform-independent crates at `245-265`, and separates pure lowering from
  subprocess execution at `744-875`. ADR-0015 gives the dependency direction
  at `412-451`, forbids `scribium-typst -> scribium-core` at `480-497`, and
  records bounded migration as the accepted next step at `813-829`.

**Impact**

The current implementation is still behaviorally bounded and passes the WASM
check, but physical ownership and dependency direction no longer make the
target architecture enforceable by Cargo. Continued semantic additions make a
later split more invasive: evaluator code can acquire more accidental
dependencies, and native backend concerns remain adjacent to pure lowering.

This is not a claim that the ADR was violated when the migration was deferred;
the ADR explicitly called it migration state. The audit conclusion is that the
repository has now crossed the practical threshold for beginning the accepted
bounded migration.

**Recommendation**

Use small behavior-preserving migration PRs: first extract source/project,
diagnostics, compatibility policy, and the existing single IR behind stable
facade re-exports; then extract engine/evaluator and AST-to-IR ownership; then
split pure Typst lowering from the native subprocess adapter. Preserve one IR,
public facade compatibility, source spans, and the existing test baseline at
each step. Do not combine this with new semantic behavior.

**Scope:** workspace membership, Cargo dependencies, module ownership, facade
re-exports, and Typst adapter placement.
**Blocks feature work?:** **Yes for new cross-cutting semantic slices.** Small
bug fixes and the migration itself can be reviewed independently, but the
current evaluator should not continue growing without this checkpoint.

### F-004 — P1 — `RawTypst` remains a backend escape hatch in public IR

**Category:** IR neutrality, backend boundary, security/maintainability

**Evidence**

- `crates/scribium-core/src/ir.rs:269-270` exposes
  `IrNode::RawTypst { source: String, span: SourceSpan }` with the comment
  “inserted verbatim into the output”.
- `crates/scribium-typst/src/lowering.rs:213-224` emits that string directly,
  temporarily enabling verbatim output and recording a source map.
- The target contract forbids this path in
  `docs/ARCHITECTURE.md:617-625` and `857-870`, and ADR-0016 explicitly
  preserves the no-generic-backend-raw rule at
  `docs/adr/0016-full-quarkdown-compatibility-and-upstream-evolution.md:234-239`.

**Impact**

Even if the current frontend does not construct this node for ordinary user
input, a public/serde-visible IR variant permits manually constructed IR or a
future shortcut to bypass backend-neutral semantics and escaping. It keeps a
forbidden backend-specific representation alive in the contract and makes a
second backend or an IR extraction less fail-closed.

**Recommendation**

Treat this as transitional debt in the IR migration: inventory any serialized
or external consumers, add an explicit compatibility decision if old data
exists, and remove the variant and lowering branch. Do not replace it with a
more generic raw backend variant. Any intentional native escape must remain a
host capability outside backend-neutral IR.

**Scope:** `ir.rs`, Typst lowering, serde compatibility policy.
**Blocks feature work?:** **Conditionally.** It blocks new backend escape paths
and the IR/backend migration; it does not invalidate the current scalar #61
slice if that slice remains backend-neutral.

### F-005 — P1 — Closed range materialization has no semantic resource budget

**Category:** Correctness, denial of service, performance/resource limits

**Evidence**

- `crates/scribium-core/src/evaluator.rs:4484-4544` converts every closed
  range to a `Vec<IrValue>`. It checks integer arithmetic, `usize` conversion,
  and allocation failure, but it has no configured maximum element count or
  evaluation budget before reserving.
- For a valid signed range with a very large cardinality, `try_reserve_exact`
  can request a very large allocation; if it succeeds, the loop then creates
  one IR value per element.
- `docs/ARCHITECTURE.md:1121-1133` makes semantic/evaluation resource limits
  a compiler-crate requirement, while `docs/ROADMAP.md:86-89` lists resource
  limits as M3 work.

**Impact**

An in-memory document can force large memory and CPU consumption without
crossing a parser or host filesystem boundary. Allocation failure is not a
sufficient semantic limit because the process may incur substantial pressure
before failure, and normal-sized behavior has no explicit policy to document or
test.

**Recommendation**

Add an evaluator-owned resource budget covering materialized elements,
recursive evaluation/call depth, and aggregate output where appropriate. Make
the failure source-backed and atomic, preserve normal range semantics, and add
boundary tests before expanding iterable or collection features. A budget
should be an explicit option/context, not a hidden global.

**Scope:** evaluator context/options and iteration tests; later compatible with
the project/engine extraction.
**Blocks feature work?:** **Yes for further iterable/materialization growth**;
conditionally blocks #61 if that slice exercises unbounded iteration.

### F-006 — P1 — `CompileOptions.compatibility_profile` is accepted but ignored

**Category:** Configuration correctness, compatibility policy, stringly-typed API

**Evidence**

- `crates/scribium-core/src/lib.rs:113-121` passes `CompileOptions` into
  `compile_with_capabilities` but names the parameter `_options` and never
  reads it.
- `crates/scribium-core/src/lib.rs:183-187` exposes
  `pub compatibility_profile: Option<String>` as part of the public compile
  options.
- ADR-0015 assigns compatibility-policy selection to `CompileOptions` at
  `docs/adr/0015-compiler-crate-boundaries.md:122-132` and requires core to
  pass the selected profile at `408-410`.
- Existing tests at `lib.rs:220-223` only verify that the field can hold
  `None`; they do not demonstrate that a non-default profile changes policy or
  is rejected.

**Impact**

Callers can supply a profile-looking string with no observable effect. This is
silent configuration loss rather than a type error, so future compatibility
claims or profile-specific behavior can be reported as selected when the
compiler actually used the default path.

**Recommendation**

Before adding profile-dependent compatibility behavior, either route a closed
typed profile through the frontend/engine policy or remove/explicitly defer the
field from the public API. If a string compatibility surface must remain for
source compatibility, validate it and produce a structured diagnostic rather
than silently ignoring it. Add a behavior test for default, selected, and
unknown profiles.

**Scope:** `CompileOptions`, compatibility policy, facade tests and docs.
**Blocks feature work?:** **Yes before profile-dependent compatibility claims**;
not a blocker for default-only behavior once the limitation is documented.

### F-007 — P1 — Hexadecimal color alpha is validated and then discarded

**Category:** Value conversion correctness, typed IR

**Evidence**

- `crates/scribium-core/src/ir.rs:89-96` defines `IrColor::alpha` as the
  backend-neutral upstream `0.0..=1.0` fraction.
- `crates/scribium-core/src/value_conversion.rs:607-617` validates the fourth
  digit of a 4-digit hex color but returns `alpha: 1.0`.
- `value_conversion.rs:625-635` validates the final byte of an 8-digit hex
  color but also returns `alpha: 1.0`.
- The test table at `value_conversion.rs:2077-2128` currently expects
  `alpha: 1.0` for `#369f`, `#33669980`, `#aBcD`, `#aAbBcCdD`,
  `#33669900`, and `#3690`. Those expectations ratify the loss rather than
  detecting it.
- The Color domain is currently not consumed by a production layout/style
  builtin, so this has not yet been observed as rendered document output.

**Impact**

Accepted alpha syntax does not preserve typed semantic identity. Transparent
and partially transparent colors become opaque before a future consumer can
use them. This is a latent correctness defect in a domain adapter, not a reason
to broaden color support in this audit.

**Recommendation**

Confirm the nibble/byte expansion and alpha normalization against the permitted
public v2.5.1 evidence, then update the decoder and tests together before
adding a color/style consumer. Add non-1.0 and zero-alpha assertions and keep
the Dynamic/Static conversion gate unchanged.

**Scope:** `value_conversion.rs` and focused tests; no current backend behavior
needs to change in this audit.
**Blocks feature work?:** **Yes for color/style consumers.** It does not block
unrelated #61 scalar semantics.

### F-008 — P2 — Scope/callable snapshots create a measurable copy-growth risk

**Category:** Evaluator performance, state/scoping maintainability

**Evidence**

- `EvaluationContext::child()` at
  `crates/scribium-core/src/evaluator.rs:585-622` clones the entire context
  into a boxed parent, clones the `VirtualProject`, and clones active-source
  state while sharing only document state through `Rc<RefCell<_>>`.
- `capture_snapshot()` and `collect_bindings()` at
  `evaluator.rs:659-692` recursively copy visible variables and clone function
  bindings. `with_caller_overlay()` at `713-737` copies caller-visible maps
  into a new lookup layer.
- `make_callable()` at `evaluator.rs:4108-4120` copies the callable body and
  captures the current bindings. `invoke_bound_callable()` at `4146-4165`
  reconstructs the definition context, caller overlay, and child context per
  invocation.
- Source contents and asset bytes use `Arc` in
  `crates/scribium-core/src/source/source_store.rs:22-46` and
  `source/asset_store.rs:19-25`, which limits some byte duplication, but the
  maps, values, function bindings, and IR bodies are still copied.

**Impact**

Nested calls, transforms, or iteration over a large captured environment can
produce repeated map/body copies. This is a static complexity risk; no
benchmark was introduced by this audit, so it is not asserted as a measured
regression. The current snapshot semantics are deliberately explicit and are
covered by scope/shadowing tests.

**Recommendation**

Add structural benchmarks or instrumentation before M3-style callable and
collection expansion. If evidence warrants it, use immutable shared
environments or a deliberately owned state handle while retaining definition
environment, caller overlay, document-state sharing, and mutation atomicity
semantics. Do not replace the current model based on allocation intuition alone.

**Scope:** evaluator context/callable implementation and performance tests.
**Blocks feature work?:** **No immediate block.** Track it before large
collection/callable expansion and alongside resource-budget work.

### F-009 — P2 — Diagnostic code construction is repeated across ownership seams

**Category:** Diagnostics, provenance, maintainability

**Evidence**

- `crates/scribium-core/src/diagnostics.rs:5-14` represents `Diagnostic.code`
  as an unconstrained `String`.
- `evaluator.rs:7673-7839` contains repeated builders for iteration,
  component, center, align, landscape, br, and whitespace diagnostics. They
  repeat literal `E3001`/`E3003` construction and generally initialize
  `secondary: Vec::new()`.
- `crates/scribium-core/src/ast_to_ir.rs:960-1014` independently constructs
  `E8001` and `E3003`; parser codes are separately declared as string literals
  in `crates/scribium-markdown/src/parser.rs:405-417` and related paths.
- ADR-0020's conversion contract requires argument-span diagnostics with a
  parameter-name secondary span, but the repeated feature builders provide no
  shared catalog/builder that enforces this shape. Existing tests do verify many
  primary spans and diagnostic de-duplication paths.

**Impact**

Adding a new feature can accidentally introduce a typo, inconsistent hint, or
missing secondary span without a compile-time signal. The current coarse code
taxonomy may be intentional, so changing codes or messages globally would be
an unsafe audit-time refactor.

**Recommendation**

After diagnostics and engine ownership are physically separated, define a
small stable code catalog plus builders for the recurring argument, conversion,
resource, and unsupported-boundary shapes. Add matrix tests for code, primary
span, secondary parameter span, and nested failure de-duplication. Preserve
existing public codes unless a reviewed compatibility decision changes them.

**Scope:** diagnostics ownership and feature diagnostic helpers.
**Blocks feature work?:** **No immediate block.** It should accompany the
physical diagnostics/engine migration and any new diagnostic-heavy slice.

## Cross-cutting audit results

### Architecture and ownership

The target architecture is internally consistent and explicit about migration
state. The current physical mismatch is captured in F-003 rather than treated
as a reason to invent new crates. The following ownership conclusions are
supported by the code and accepted documents:

- `scribium-markdown` owns the single authoritative Rushdown-backed block
  parser. Its public output is Scribium-owned `Document`/AST data; Rushdown
  types are private imports at `parser.rs:4-20`. No Rushdown type leaks through
  `ParseOutput` at `parser.rs:35-46`.
- Quarkdown grammar remains in `scribium-quarkdown`; the Markdown parser calls
  its narrow grammar helpers rather than embedding a second full grammar.
- `ast_to_ir.rs` currently combines AST conversion, metadata normalization, and
  narrow raw-HTML pairing/compatibility classification. That is a physical
  migration concern for engine/HTML ownership, not evidence that a second
  parser should be built.
- `scribium-core/src/lib.rs:108-180` is a small orchestration facade in
  production code, even though its test module makes the file large. It should
  not be split solely because the file is 141 KB.
- `scribium-typst/src/lowering.rs` has a cohesive IR-to-Typst responsibility;
  its backend-specific syntax fragments and escaping belong there. The native
  filesystem/process portion belongs in the physical adapter identified by
  F-003.
- `scribium-cli/src/commands.rs:1-18` and `85-172` correctly own native path,
  filesystem, project loading, and output composition. The current explicit
  logical-root, canonicalization, symlink, and atomic-output tests are the
  right host boundary.

### Hardcoding and stringly-typed design

The audit does not recommend removing all literals. They fall into three
classes:

| Literal family | Classification | Decision |
|---|---|---|
| Quarkdown builtin names and closed argument names | Language-specified constants, but currently duplicated across dispatch and binders | F-002: centralize regular signatures/ownership; retain bespoke policies. |
| Diagnostic codes | Stable external taxonomy, currently represented as strings | F-009: catalog/builders later; do not rename codes in this audit. |
| Quarkdown v2.5.1 profile/version and fixture levels | Compatibility metadata | Keep as evidence-backed metadata; make level drive assertions in F-001 and profile drive policy in F-006. |
| Typst syntax fragments and escaping | Legitimate backend implementation detail | Keep in `scribium-typst`; do not move into core IR. |
| `.md`, `.qd`, `.scrib` extension handling and case-insensitive final extension | Explicit input contract | Keep at the frontend/CLI boundary; do not make arbitrary extensions implicit. |
| Closed enum conversion tables | Appropriate typed registry | Keep `ClosedEnumSpec`/typed enum values; do not replace with generalized string coercion. |
| Zero-size/default layout values | Local semantic defaults where documented by the bounded feature | Review only when upstream evidence changes; no blanket constant extraction. |

### Builtin binding, state, and materialization

The evaluator already preserves several important semantics and these should
not be weakened while addressing F-002 or F-008:

- user-defined functions shadow native builtins in the central value dispatch;
- definition environment, caller overlay, invocation parameters, and document
  state have distinct ownership paths;
- document state is explicitly shared through the evaluator-owned handle while
  lexical maps are isolated;
- body validation occurs before body evaluation for the bounded components;
- `IrValue::Component` remains typed until block materialization;
- `IrValue::Content` remains structured and is not stringified/reparsed;
- `materialize_block_value()` at `evaluator.rs:5219-5265` and
  `materialize_inline_value()` at `5268-5310` are the explicit output
  boundaries; component, range, collection, pair, dictionary, scalar, and
  rich-content behavior should remain fail-closed and atomic; and
- `InvocationValue`/`ValueOrigin` at `value_conversion.rs:17-73` and the
  domain gate at `379-505` preserve typed identity and Dynamic-vs-Static
  conversion boundaries. The separate `adapt_string_argument()` path is a
  documented structural resource/native-content adapter, not permission to
  add generic stringification.

The current IR intentionally has one semantic model rather than a new HIR/MIR
stack. `IrValue::Component`, `IrValue::Content`, unresolved calls, and block vs
inline materializers are complementary positions in that model, not duplicate
backend representations. The exception is `RawTypst`, covered by F-004.

### Frontend and HTML boundaries

No frontend architecture finding was opened beyond the migration evidence in
F-003. The parser's mode selection, contextual lambda handling, Rushdown
extension adapter, and source-span reconstruction are concentrated in the
Markdown frontend. AST-to-IR performs a narrow whitelist for already parser-
owned raw HTML and preserves unsupported/mixed forms as source-backed
`E8001`; it does not constitute a new Markdown parser. The closed
target-specific HTML/NativeContent boundary remains distinct from ordinary raw
HTML and should stay that way.

### Resource, I/O, and security boundaries

The audit found no P0 host escape, arbitrary shell execution, or core filesystem
access. The current tests cover logical project paths, source-relative resource
lookup, symlink escape rejection, atomic output, and direct Typst invocation.
The native `std::fs`/`Command` use found in CLI and the current Typst backend is
consistent with current physical host behavior but inconsistent with the target
crate split, which is why it is F-003 rather than a claim of an exposed source
command injection.

The remaining security-relevant gap is resource exhaustion from unbounded
semantic materialization (F-005). Resource budgets belong in compiler/evaluator
state, not in a backend or host-only workaround.

### Rust quality and transitional code

The audited production workspace contains no `unsafe` implementation path in
the reviewed compiler/backend/CLI sources, and strict clippy passed. Test and
fixture helpers use deliberate `expect`/panic paths for malformed test setup;
those are not evidence of production user-input handling. The Markdown parser
also catches a Rushdown panic at its boundary and returns `E9003` rather than
letting a parser panic escape.

The confirmed transitional code is the physical architecture in F-003 and the
`RawTypst` variant in F-004. No deletion of helpers, fixtures, ADR history, or
compatibility shims is recommended without separate usage/ownership evidence.

## Baseline validation

All checks below were run on the audited base without changing production code
or weakening tests:

| Check | Result |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace --all-features` | PASS, including core, Markdown, Quarkdown, test-support, Typst backend/integration, Markdown compatibility, and upstream-watch tests |
| `cargo test -p scribium-core` | PASS |
| `cargo run -p scribium-cli -- build examples/hello/main.qd` | PASS; generated Typst |
| `cargo run -p scribium-cli -- build examples/hello/main.qd --format pdf` | PASS; generated PDF with installed Typst |
| `cargo run -p scribium-cli -- inspect examples/hello/main.qd --emit typst` | PASS; emitted Typst |
| Existing Typst-required backend integration | PASS as part of workspace tests; 38 integration tests and subprocess checks passed |
| Existing compatibility/conformance tests | PASS as part of workspace tests; Markdown compatibility and test-support tests pass, with F-001 documenting their semantic-strength limitation |
| `cargo check -p scribium-core -p scribium-typst --target wasm32-unknown-unknown --all-features` | PASS |
| `cargo doc --workspace --all-features --no-deps` | PASS |

The generated example outputs are ignored by the repository and were not added
to this audit change. No baseline failure was observed to hide or reclassify.

## Documentation truthfulness review

The README and compatibility specification generally use the required
distinctions correctly: compatibility is partial/evidence-based, bounded
semantic slices are called out, backend support is separate, and planned or
deferred rows are not presented as implemented. `docs/ROADMAP.md` also labels
the conformance expansion and resource-limit work as unfinished.

The qualification that needs correction is evidentiary rather than a broad
README rewrite: the compatibility specification says implemented rows are
claims only with their listed conformance evidence, while the current
conformance helper does not enforce the `Semantically supported` level. F-001
is the smallest truthful fix: strengthen the evidence gate and then update any
matrix wording that does not match the resulting policy.

## Recommended remediation sequence

These are independent, reviewable work units. None is part of this audit PR.

| ID | Goal | Dependency | Risk | Expected change area | Behavior-preserving? | Blocks #61? |
|---|---|---|---|---|---|---|
| R1 | Make conformance levels executable assertions and run the corpus in CI | None | Medium: test policy may expose existing fixture gaps | `scribium-test-support`, fixture metadata/expected outputs, CI, compatibility docs | Yes for compiler behavior; strengthens verification | **Yes; first** |
| R2 | Establish one native builtin inventory and regular-signature path while retaining bespoke handlers | R1 recommended for evidence | Medium: dispatch ordering and shadowing must be regression-tested | `evaluator.rs`, `builtins.rs`, native builtin tests | Yes | **Yes; before more native semantic additions** |
| R3 | Resolve `CompileOptions.compatibility_profile` as a real typed policy or explicitly remove/defer it | R1 not technically required | Medium: public API/configuration decision | `CompileOptions`, compatibility policy, facade/CLI/docs/tests | Yes if default behavior remains unchanged | **Yes before profile-dependent claims** |
| R4 | Add evaluator resource budgets for range/materialization/call depth | R1 and the existing evaluator context; independent of crate moves | Medium: explicit limits can expose large-input behavior | evaluator context/options, iteration/call tests, diagnostics | Yes for inputs below documented limits | **Yes for iterable/materialization slices** |
| R5 | Extract source/project/diagnostics/compatibility and the existing single IR behind the core facade | R1-R4 should define contracts; no semantic redesign | High: Cargo dependency and public re-export migration | new target crates, core facade, module moves, serde/source-span tests | Yes | **Recommended gate before continued cross-cutting growth** |
| R6 | Extract engine/evaluator and AST-to-IR ownership; preserve one IR and typed value flow | R5 | High: scope, diagnostics, and frontend ownership | `scribium-engine`, core orchestration, AST-to-IR, evaluator tests | Yes | **Yes for sustained #61 expansion** |
| R7 | Split pure Typst lowering from native subprocess execution and remove `RawTypst` | R5; coordinate with R6 for IR ownership | High: adapter/API and serde compatibility | `scribium-typst`, `scribium-typst-subprocess`, CLI, IR/lowering tests | Yes after reviewed legacy-IR decision | No for scalar-only work; yes for backend escape work |
| R8 | Correct hex alpha conversion and add consumer-level color evidence | Before any color/style consumer | Low-to-medium: verify public upstream representation | `value_conversion.rs`, tests, later style consumer | Yes for unrelated behavior | **Yes for color/style work only** |
| R9 | Add diagnostic catalog/builders and secondary-span/de-duplication matrix tests | R5/R6 diagnostics ownership | Medium: message/code compatibility risk | diagnostics crate, evaluator/AST-to-IR/parser adapters | Yes if codes/messages are retained | No immediate block |
| R10 | Measure callable/context copy costs and optimize only if evidence warrants | R4 resource instrumentation; after semantic contract stabilization | Medium: scope semantics can regress | evaluator benchmarks/instrumentation and context internals | Must be behavior-preserving | No immediate block; before large M3 collection growth |

The recommended ordering is therefore:

```text
R1 → R2 → R3/R4 → R5 → R6 → R7
                 ↘ R8 (before color consumers)
                         ↘ R9/R10 (as evidence and ownership permit)
```

## Do not refactor / keep as-is

The following current implementation choices are supported by accepted
architecture or semantic tests and should not be “cleaned up” during the
remediation work without new evidence:

- Do not split `lib.rs` merely because its test module makes it large; its
  production facade is cohesive.
- Do not replace the single backend-neutral IR with HIR/MIR or stringify
  `IrValue::Component`, `IrValue::Content`, pairs, dictionaries, ranges, or
  callables for convenience.
- Do not flatten block content into inline text, serialize values to strings,
  or reparse synthesized Markdown. Keep conversion at the explicit block/
  inline materialization boundary.
- Do not turn `ValueOrigin`/`InvocationValue` into generalized coercion. The
  Dynamic-vs-Static gate and typed domain identity are intentional.
- Do not make a universal builtin binder that evaluates lazy bodies, callbacks,
  resource calls, and components identically. F-002 asks for a narrow registry
  for regular signatures plus explicit bespoke policies.
- Do not move Quarkdown grammar or Markdown parsing into core, fork Rushdown,
  or make the parser own semantic evaluation policy. The current parser adapter
  and source-provenance tests are the correct ownership boundary.
- Do not move Typst syntax fragments into core IR or use a generic raw backend
  escape hatch. The required change is removal of `RawTypst`, not a replacement
  with a broader escape mechanism.
- Do not move filesystem, process, canonicalization, or symlink policy into
  compiler semantics as a workaround for the physical Typst split. Keep those
  responsibilities at the native host/adapter boundary.
- Do not rewrite accepted ADR history or delete transitional code without
  confirming production, test, documentation, and serialized-IR references.

## Audit conclusion

The baseline is suitable for a checkpoint PR: formatting, lint, tests, Typst
integration, WASM, and docs all pass, and the existing evaluator semantics have
substantial scope, provenance, atomicity, and typed-value coverage. It is not
yet safe to treat the conformance corpus as a semantic gate, nor to continue
adding native semantic handlers indefinitely to the current manual dispatch
surface. The next work should be R1 plus an independently reviewed R2/R3/R4
decision, followed by bounded architecture migration before the evaluator and
backend boundaries become harder to move.
