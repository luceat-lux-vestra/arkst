# Issue #187 — In-process Typst backend re-evaluation

Status: completed spike, decision **GO — optional in-process only**.

Date: 2026-08-26
Repository: `luceat-lux-vestra/scribium`
Branch: `research/typst-inprocess-187`
Base: `8eb6260aa811dac0bde24d47c40ac0dd3caebed8`

## Scope and bookkeeping

PR #193 was already squash-merged at the base above. Issue #156 was closed as
completed with a note linking that merge and recording the strict-review pass.
The native GitHub sub-issue hierarchy for #147 was re-read in both directions:
#147 has exactly #148–#156 as children, every child points back to #147, and
all nine children are completed. #147 was then closed as completed. No
compatibility classification was changed to close either tracker.

Issue #187 remains open as the technical-spike tracker. This document records
the implementation and evidence attached to its PR; it does not make the
subprocess backend removable or make in-process compilation the CLI default.

## Current Typst API and dependency findings

The current stable release observed for this spike is Typst **0.15.1**. The
adapter uses the public APIs:

- `typst::World` for the compiler environment;
- `typst::compile::<typst_layout::PagedDocument>(&dyn World)` for compilation;
- `typst_pdf::pdf(&PagedDocument, &PdfOptions)` for PDF export;
- `typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot}` for
  logical source identities and project/package roots;
- `typst::text::{Font, FontBook}` for deterministic embedded/project font
  registration; and
- `typst_assets::fonts()` for the deterministic built-in font set.

The API references used for this review are the [Typst 0.15.1 API
documentation](https://docs.rs/typst/0.15.1), the [`typst-pdf` 0.15.1
exporter](https://docs.rs/typst-pdf/0.15.1), and the [Typst release
history](https://github.com/typst/typst/releases/).

The smallest maintainable dependency set found for the adapter is:

```text
typst, typst-layout, typst-pdf, typst-assets (fonts feature)
```

`typst-kit` was intentionally not selected. Its convenience environment is
designed to discover host fonts and package/filesystem capabilities that are
not part of Scribium's `VirtualProject` policy. Package/network access is
therefore denied in this spike, rather than silently widening the host
capability model.

The adapter does not depend on Typst compiler internals, renderer frames,
`Content`, or layout objects. Typst types are confined to the native
`scribium-typst-inprocess` crate. The platform-neutral `scribium-typst` crate
continues to lower `IrDocument` to generated Typst source only.

## Ownership mapping

| Typst `World` capability | Scribium authority/policy | Spike result |
|---|---|---|
| `main` and entry source | `VirtualProject.entry()` plus `TypstInput.source` | Generated logical `.typ` entry; entry mismatch rejected |
| Additional `source`/`file` | `SourceStore` and `AssetStore` | Deterministic immutable maps; project-relative lookup |
| Source identity | `SourceId` and `VirtualPathBuf` | Existing project `.typ` sources map to `SourceSpan`; generated source has no fabricated source span |
| Relative paths | `VirtualPathBuf` normalization and Typst `VirtualRoot::Project` | Traversal and project escape reject closed |
| Images/resources | `AssetStore` bytes | Loaded without filesystem access; missing bytes fail with virtual paths |
| Missing/inaccessible files | `FileError` using synthetic logical paths | No host/native temporary path is exposed |
| Fonts | Embedded `typst-assets` fonts plus project font assets (`otf`, `ttf`, `otc`, `ttc`) | Deterministic `FontBook`; no system-font discovery |
| Packages | `VirtualRoot::Package` policy | Explicit `PackageError::NotFound`; no network or package cache |
| Date/environment | Explicit fixed date `1970-01-01` | No process-clock nondeterminism; future host capability is deferred |
| Repeated loads/caching | Per-compile immutable `BTreeMap`/font vectors | Stable repeated reads without global mutable cache |

This preserves `VirtualProject` as the semantic resource source of truth. The
adapter does not create a second project model, infer a host root from the
current directory, or add unrestricted filesystem/network access.

## Implementation and focused evidence

The spike adds `crates/scribium-typst-inprocess`. It implements the existing
platform-neutral `TypstBackend` contract and performs:

```text
VirtualProject + generated Typst source
    -> ProjectWorld
    -> typst::compile::<PagedDocument>
    -> typst_pdf::pdf
    -> TypstOutput PDF
```

Focused tests cover:

- actual Scribium compile/lowering into a valid PDF;
- `AssetStore` SVG loading with no host filesystem;
- project font bytes and case-insensitive font extensions;
- missing resources and project-boundary traversal;
- package denial without network access;
- generated-source diagnostics with `/docs/main.typ` and no fabricated span;
- generated-entry collision with an existing project `.typ` source; and
- subprocess parity for the existing `examples/hello/main.qd` Quarkdown
  fixture, a real generated multi-page workload, and an invalid generated
  Typst failure.

The focused command passed:

```text
cargo test --jobs 2 -p scribium-typst-inprocess --all-features -- --nocapture
11 integration tests and 2 unit tests passed; 0 failed
```

## Diagnostic comparison

For `#unknown-function()` both adapters classify the document as a compile
failure. The in-process adapter returns a Scribium `E5001` diagnostic with a
logical location such as `/docs/main.typ` and structured severity. Because this
spike does not yet receive the lowerer's source map, the generated main source
has no fabricated `SourceSpan`. Existing project `.typ` sources can retain a
real project source span.

The subprocess adapter continues to return its existing sanitized stderr error
(`Typst compilation failed`) with temporary-root components rewritten to
`<typst-build>`. Thus the spike improves structure and stable classification
for in-process failures without changing the global diagnostic system or
claiming complete source-map equivalence.

Resource and package errors remain fail-closed in both paths. The in-process
path reports logical paths and package errors; the subprocess path preserves
its existing staging and sanitized-stderr behavior. No temporary directory or
native project path appeared in the focused in-process diagnostic assertions.

## Parity oracle and results

The parity oracle is deliberately document-observable rather than PDF byte
identity:

1. both adapters must agree on success/failure classification;
2. successful output must begin with `%PDF-`;
3. the generated 100-paragraph workload must produce at least two pages in
   both backends, with equal PDF page-object count; and
4. an invalid generated document must produce structured in-process diagnostics
   and the existing subprocess compilation-failure classification.

The parity tests passed on macOS arm64 with local Typst 0.15.1. Byte-identical
PDFs are not used as an oracle: exporter metadata and font/resource ordering
are not a stable contract. Resource ownership is additionally tested directly
against `VirtualProject` in the in-process suite; the subprocess adapter's
existing context-backed resource tests remain separate because its API accepts
an explicit native source context rather than a `VirtualProject`.

## Measurements

Machine: macOS arm64, 2026-08-26. Profile: Cargo `--release` (`opt-level = 2`,
workspace profile reported as optimized + debuginfo). The workload was the
same generated Scribium Typst source containing 100 paragraphs. Eight runs
were performed in one process per adapter; the first run is cold within that
process and the remaining runs are repeated/warm. `/usr/bin/time -l` measured
the adapter-specific benchmark process. These are current spike measurements,
not the historical #12 numbers or acceptance thresholds.

| Measurement | In-process | Subprocess |
|---|---:|---:|
| First compile in process | 19 ms | 1,121 ms |
| Repeated compile range (runs 2–8) | 3–4 ms | 1,072–1,096 ms |
| Benchmark-process peak RSS | 20,299,776 bytes | 56,328,192 bytes |
| Clean release build of adapter example | 154.95 s | 12.58 s |
| Release example binary | 45,630,304 bytes | 2,010,704 bytes |
| `strip -S` output | 45,630,336 bytes | 2,010,752 bytes |

The clean builds used separate fresh target directories:

```text
CARGO_TARGET_DIR=/tmp/scribium-187-clean-inprocess \
  cargo build -q --release -p scribium-typst-inprocess --example measure_inprocess
CARGO_TARGET_DIR=/tmp/scribium-187-clean-subprocess \
  cargo build -q --release -p scribium-typst-subprocess --example measure_subprocess
```

The in-process graph contains **373** unique normal dependency-tree lines in
`cargo tree`; the subprocess graph contains **29**. The in-process dependency
set brings in the compiler, layout, font, image, and PDF-export graph. The
runtime speedup is real for this workload, but the clean-build and binary-size
costs are material and are the main reason this spike does not recommend a
default migration.

The dependency/security gate was also run after adding the Typst graph:

```text
cargo deny check
```

Licenses, bans, and sources passed, but the command failed the advisory gate on
transitive Typst dependencies: `quick-xml 0.38.4` has the current
RUSTSEC-2026-0194 and RUSTSEC-2026-0195 advisories, and the graph also reports
unmaintained `paste`, `rustybuzz`, `ttf-parser`, and `yaml-rust` paths. The
current Typst 0.15.1 dependency constraints do not provide a safe in-spike
upgrade for those paths. No advisory was suppressed and no unrelated
dependency migration was absorbed into #187; this is an explicit production
promotion risk for #200.

## Platform and WASM findings

The local native evidence is macOS arm64 only. Linux and Windows were not
executed locally; the repository's Linux, macOS, and Windows native CI matrix
must provide the supported-platform evidence for this optional adapter before
production use. The adapter is intentionally native-only.

The platform-neutral boundary remains separate and was checked with:

```text
cargo check -p scribium-core -p scribium-typst \
  --target wasm32-unknown-unknown --all-features
```

The in-process Typst compiler dependencies are not pulled into
`scribium-core` or `scribium-typst`, and the WASM check remains scoped to those
lowering/compiler crates. This spike is not evidence for the future browser
WASM backend (#191).

## Decision

**GO — optional in-process only.**

The adapter is viable inside the current architecture: `VirtualProject` can
provide a bounded public `World`, resource ownership remains correct, real
Scribium-generated source compiles, parity is acceptable under the stated
oracle, and diagnostics are at least as structured for the exercised failures.

The subprocess backend remains the default because the current adapter has a
large dependency/build/binary footprint, package/date/font capability policy
is intentionally incomplete, generated-source source-map handoff is not yet
implemented, and only macOS native execution was available locally. The
optional path must not be exposed as a production default until Linux/Windows
CI, broader corpus parity, source-map handoff, and an explicit opt-in UX are
reviewed.

Re-evaluation trigger: revisit the default only after the bounded follow-ups
below have green cross-platform CI and corpus parity, and a maintainer accepts
the measured footprint and explicit date/package/font policy. If those checks
fail, keep the subprocess backend as the supported strategy and do not broaden
the resource model to accommodate Typst.

## Deferred risks and out-of-scope items

- No package/network capability was added. #188 is not a prerequisite for this
  spike; package support needs a separate accepted policy.
- No global document environment/date capability was implemented (#190).
- No browser/WASM rendering backend was implemented (#191).
- The subprocess backend was not deleted or changed into a fallback inside this
  issue.
- No direct Scribium IR to Typst internal layout lowering was introduced.
- No global diagnostic-system redesign was attempted.

## Bounded follow-ups

- [#200: explicit native in-process backend selection](https://github.com/luceat-lux-vestra/scribium/issues/200)
  — production-quality opt-in selection, source-map handoff, and policy
  review while retaining subprocess as default/fallback.
- [#201: cross-platform in-process parity](https://github.com/luceat-lux-vestra/scribium/issues/201)
  — Linux/macOS/Windows corpus, resource, diagnostic, and WASM-boundary
  evidence under pinned Typst 0.15.1.

See ADR-0021 for the preserved historical decision and the optional strategy
addendum.
