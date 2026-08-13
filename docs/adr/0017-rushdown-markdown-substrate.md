# ADR-0017: Rushdown Markdown Substrate

- **Status:** Accepted
- **Date:** 2026-08-13
- **Owners:** Scribium maintainers
- **Related ADRs:** 0014, 0015, 0016
- **Upstream issue:** https://github.com/yuin/rushdown/issues/2

## Decision

Scribium uses Rushdown as its Markdown/CommonMark/GFM parsing substrate. The
Scribium frontend does not develop or maintain a second CommonMark parser.
Rushdown is an implementation dependency, not part of Scribium's public
semantic model. Rushdown types must not escape `scribium-markdown`.

The selected upstream baseline is:

| Field | Value |
|---|---|
| Repository | `https://github.com/yuin/rushdown` |
| Stable release | `0.18.0` |
| Release/tag | `v0.18.0` |
| Tag and default-branch HEAD | `e5eb4e4446541ea0ed53111c1b37e779283ff57c` |
| Latest source commit | `2026-04-30T05:34:45Z` |
| Release date | `2026-04-30T05:37:37Z` |
| License | MIT |
| Archived | No |
| GitHub open issues at adoption | 1, including the safety issue above |
| Cargo features | `std`, `html-entities` |

The dependency is pinned to the exact upstream git revision in the workspace
manifest. A new release is not adopted automatically: it requires the parser,
provenance, WASM, CommonMark/GFM, extension, and safety regression gates plus
maintainer review.

## Ownership and dependency direction

The physical workspace now contains the three frontend boundary crates:

```text
scribium-source
    ↑
scribium-quarkdown ← scribium-markdown → Rushdown
                                  ↓
                         Scribium frontend AST
```

`scribium-source` owns platform-neutral `SourceId`, `ByteSpan`, `SourceSpan`,
and related source primitives. It has no filesystem, process, network, or
backend dependency.

`scribium-quarkdown` owns Quarkdown call-name and argument grammar only. It
depends on `scribium-source`, and never depends on Rushdown, Markdown AST
types, or `scribium-core`.

`scribium-markdown` owns Rushdown construction, `.md`/`.qd` mode selection,
Quarkdown block and inline extension rules, conversion to the Scribium
frontend AST, source provenance checks, and the integration policy. The core
facade invokes this frontend; engine, IR, Typst, and other production crates do
not depend on Rushdown.

## Parser authority and modes

Rushdown is authoritative for CommonMark/GFM block, container, paragraph,
inline, code-shielding, and source-segmentation behavior. Quarkdown extensions
are registered only by `scribium-markdown`:

- `.md` uses the standard Rushdown parser without Quarkdown extensions;
- `.qd` uses the same Rushdown lifecycle with Quarkdown block and inline rules;
- Quarkdown block bodies use Rushdown's nested block lifecycle; and
- Quarkdown inline calls are represented as frontend directive nodes rather
  than flattened ordinary text.

No regular-expression pre-pass or Quarkdown-specific parser is inserted into
the Rushdown fork. A narrow adapter normalization handles Rushdown's ordinary
lazy-paragraph behavior at an indented Quarkdown body boundary without
creating a synthetic source or reparsing transformed Markdown.

Quarkdown content arguments are also processed against the original document
source. Nested Quarkdown calls are scanned with the grammar crate and retain
their original spans. Rushdown 0.18.0 does not expose a public inline-parser
entry point for an arbitrary original-source span, so Markdown inline markers
inside a content argument remain source-backed text and produce explicit
diagnostic `E3010`; the adapter does not create a sentinel prefix, copy a
fragment, or compensate synthetic offsets.

## Safety decision and containment

`KNOWN_UPSTREAM_SOUNDNESS_RISK_ACCEPTED`: Rushdown 0.18.0 has a known public
safe-API soundness defect: `Index` and
`Segment` safe constructors/mutators accept arbitrary byte offsets while their
safe `str()` accessors call unchecked string slicing. The downstream safe-only
reproduction and Miri evidence are recorded in the historical safety report.
This is not hidden or reclassified as a panic.

Issue #2 reports the defect upstream with the exact release SHA and separate
`bytes()`/`str()` behavior. The suggested upstream fix is checked slicing or an
API/type invariant that makes invalid states unconstructable. No upstream fix
or permanent fork is assumed by this decision.

Until an upstream fix exists, the local safety delta is limited to the
`scribium-markdown` adapter:

- `DIRECT_AFFECTED_ACCESSORS_AVOIDED_AT_ADAPTER_BOUNDARY`: Rushdown
  `Index`/`Segment` values are converted to `ByteSpan` only after bounds and
  UTF-8-boundary validation, and the adapter does not call the affected
  `str()` accessors;
- `PARSER_PATH_MIRI_AND_PROPERTY_REGRESSION_MONITORED`: parser-produced
  source ranges are checked by deterministic adversarial, `proptest`, and
  Miri-eligible suites; and
- the parser is created per document and a panic is converted to structured
  diagnostic `E9003` with `catch_unwind`. This contains parser robustness
  failures only; it is not a memory-safety proof and does not make the
  upstream unsafe API sound.

These adapter checks reduce direct affected-accessor exposure and validate
frontend provenance. They do not prove that every unsafe path inside Rushdown
is sound, and the accepted upstream issue remains an explicit dependency risk.

The existing safe-only/Miri reproduction remains in
`tools/spikes/rushdown-safety-gate/`. Invalid constructor cases are never run
as an ordinary native executable. The adoption suite is in
`crates/scribium-markdown/tests/range_invariants.rs` and its valid parser paths
are eligible for Miri.

If upstream does not fix the defect, a future patch or fork may contain only
the proven safety correction, selected parser-path safety corrections, urgent
security fixes, or required Rust compatibility changes. It may not contain
Quarkdown syntax, Scribium ASTs, renderer changes, or unrelated refactoring.

## Compatibility and known debt

The migration reuses the previously reviewed Rushdown extension and WASM
mechanics evidence, and adds independent source-range, UTF-8, CRLF, malformed
input, code-shielding, nested-list, blockquote, link/image, GFM, and Quarkdown
fixtures. Existing core tests remain the compatibility regression corpus.

The current frontend AST preserves image, raw HTML, strikethrough, blockquote,
tables, task-list status, and other Rushdown nodes. The existing backend-neutral
IR has narrower legacy variants, so lowering behavior for those nodes remains
explicit compatibility debt until the owning IR/engine decisions are reviewed.
The lowering boundary emits an unsupported diagnostic rather than coercing an
image to a link, a table to a blockquote, raw HTML to text, or rich
strikethrough children to plain text. This ADR does not add a new IR tier or
silently claim complete Quarkdown compatibility.

The Rushdown issue and its known unsafe implementation are part of the
accepted dependency risk. The risk is managed through the exact revision,
upstream issue tracking, source-range validation, Miri/property regression,
dependency/security audits, and human review of every revision change.

## Validation evidence

The exact revision was checked with the following evidence:

- `cargo test --workspace --all-features`, `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`,
  and `cargo doc --workspace --all-features --no-deps` pass.
- `cargo check -p scribium-source -p scribium-quarkdown -p
  scribium-markdown -p scribium-core -p scribium-typst --target
  wasm32-unknown-unknown` passes.
- `MIRIFLAGS=-Zmiri-disable-isolation PROPTEST_CASES=4 cargo +nightly miri
  test -p scribium-markdown` passes. The isolation flag is required by the
  current Miri environment; the initial unadjusted run was blocked by Miri's
  `getcwd` isolation rather than classified as a pass.
- The historical safe-only spike's valid cases pass under Miri. Its invalid
  `Index::str` and `Segment::str` out-of-bounds/reversed cases produce Miri
  precondition violations (`MIRI_UB`) at Rushdown's unchecked access. They are
  not run as native executables.
- The range suite covers valid arbitrary UTF-8 within a bounded property-test
  input, Unicode and Markdown/Quarkdown adversarial fixtures, and recursively
  checks bounds and UTF-8 boundaries for every exposed frontend span.
- `cargo audit --json` reports zero vulnerabilities in the selected workspace
  graph. `cargo deny check` passes; its existing duplicate-license/dependency
  configuration warnings are non-failing. The selected Rushdown runtime graph
  is exact git Rushdown plus `bitflags`, `memchr`, `phf`, `phf_shared`, and
  `siphasher`; `proptest` is test-only.

The selected feature graph reaches Rushdown scanner/parser text and link paths,
including `text.rs`, `scanner`, parser core, link/auto-link, and linkify when
GFM is enabled. Renderer, benchmark, binary, and test-only unsafe sites are
outside the selected frontend runtime graph. Rushdown does not currently
forbid unsafe code at crate level. Scribium adds no unsafe code and avoids the
affected raw string accessors at its adapter boundary; issue #2 remains the
upstream safety debt.

## Acceptance matrix

| Criterion | Result |
|---|---|
| block extension | PASS |
| inline extension | PASS |
| precedence | PASS |
| container integration | PASS |
| exact provenance | PASS |
| `.md`/`.qd` isolation | PASS |
| CommonMark | PASS; the official 652-fixture baseline is reused from PR #49 because the version is unchanged |
| required GFM profile | See the stage matrix below; parser and frontend preservation pass for the covered table/task/strikethrough/link features, while unsupported lowering is explicit |
| known panic exposure | Parser panic is converted to explicit `E9003` failure at the document boundary; `catch_unwind` is robustness containment only |
| silent corruption exposure | Not claimed as excluded; original-source spans are checked at the adapter boundary while the upstream soundness risk remains accepted |
| selected dependency graph | CLEAN |
| WASM | PASS |
| unsafe policy | FAIL upstream policy (`unsafe` is present and not forbidden); local Scribium delta is zero and the known defect is tracked/contained |
| maintenance | HIGH_RISK; exact pin, audit, regression, and human review are required |
| permanent fork required | NO |

The stage-specific support status is:

| Feature | Parser | Frontend | IR | Typst |
|---|---|---|---|---|
| blockquote | PASS | PASS | DEFERRED; `E8001` | DEFERRED |
| image | PASS | PASS | DEFERRED; `E8001` | DEFERRED |
| table | PASS | PASS | DEFERRED; `E8001` | DEFERRED |
| task list | PASS | PASS; status preserved | DEFERRED; `E8001` | DEFERRED |
| strikethrough | PASS | PASS; children preserved | DEFERRED; `E8001` | DEFERRED |
| raw HTML | PASS | PASS | DEFERRED; `E8001` | DEFERRED |
| autolink/linkify | PASS | PASS when enabled | existing Link path where applicable | existing Link lowering where applicable |
| Quarkdown block/inline | PASS | PASS | existing directive path where applicable | existing directive lowering where applicable |

`PASS` at the parser or frontend stage does not claim end-to-end rendering.
For a Quarkdown content argument containing Markdown inline markers, the
current stage is parser/original-span preservation with `E3010`, not Strong or
Emphasis lowering; arbitrary-span Rushdown inline parsing is not public in
0.18.0.

The acceptance decision is `RUSHDOWN_SELECTED` under this ADR. This is a
substrate adoption decision only; it does not select a future architecture
layer, authorize a permanent fork, or waive the upstream safety issue.

## Consequences

- Scribium gains one Markdown lifecycle for CommonMark/GFM and Quarkdown
  integration instead of growing a parallel parser.
- Rushdown changes are isolated behind `scribium-markdown`.
- The old parser modules remain as migration-era compatibility/test material;
  new Markdown behavior belongs in the Rushdown frontend.
- Production adoption preserves WASM and filesystem-free lower-level crates.
- Issue #24 and resource-resolution work remain deferred; this ADR does not
  move `VirtualProject` or implement assets/imports.

## Implementation note / follow-up

The legacy first-party Markdown and Quarkdown parser modules were removed in a
follow-up cleanup after the Rushdown frontend migration completed. The
decision and consequence recorded above are preserved as historical context.
