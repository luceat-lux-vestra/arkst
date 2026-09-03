# Roadmap — Arkst

Status markers: `Not started` | `In progress` | `Completed` | `Deferred`

## M0 — Foundation

**Objective:** Establish legal boundaries, technology choices, and repository structure.

| Item                           | Status       |
|--------------------------------|--------------|
| Repository bootstrap           | Completed    |
| LICENSE/NOTICE                 | Completed    |
| Product documentation          | Completed    |
| Name due diligence             | Completed    |
| Typst integration spike        | Completed    |
| Markdown parser spike          | Completed    |
| ADR 0001–0016                  | Completed    |
| GitHub templates/CI            | Completed    |
| WASM build in CI               | Completed    |
| VirtualProject abstraction     | Completed    |

**Architecture constraint:** `arkst-core` + `arkst-typst` (lowering)
MUST compile for `wasm32-unknown-unknown`. CI verifies this from M0.
Core uses `VirtualProject` for all I/O — no filesystem access.

**Dependencies:** None

## M0.5 — Upstream Compatibility Infrastructure

**Objective:** Detect Quarkdown upstream drift and create actionable
compatibility-adaptation work.

| Item                                | Status       |
|-------------------------------------|--------------|
| Machine-readable upstream baseline  | Completed    |
| Stable release observer             | Completed    |
| Drift issue automation foundation   | Completed    |
| Conformance corpus foundation       | Completed    |

The current observer is the first stage only: latest stable release detection
→ deduplicated drift issue. It does not yet collect public deltas, classify
impact, prepare fixtures, or create adaptation PRs.

**Dependencies:** M0

## M1 — Quarkdown-Compatible Vertical Slice

**Status:** Completed

**Objective:** First end-to-end `.qd → Typst → PDF` pipeline.

Acceptance: dot-prefixed calls, positional/named/body arguments, basic conditional,
front matter, deterministic output.

> **Front Matter scope:** currently a flat line-based `key: value` format only.
> Delimiters and metadata lines must start at column 0; indented keys reject
> the block. Nested objects, arrays, and block strings (full YAML) are deferred
> to a later milestone and tracked separately.

## M2 — Quarkdown Core Language + Markdown MVP

**Status:** Completed (bounded, evidence-backed baseline; broader compatibility remains)

**Objective:** Quarkdown core language + Markdown MVP for v0.1.0, with honest
partial compatibility claims and a growing evidence-backed baseline.

| Item                                | Status       |
|-------------------------------------|--------------|
| Document-scope variable evaluation  | Completed    |
| Generic callables, native iterable operations, and collection transforms | Completed (evidenced slice; `.map`/`.filter` retained as Arkst extensions) |
| Logical/comparison predicates (`.islower`, `.isgreater`, `.equals`, `.not`) | Completed (bounded v2.5.1 slice) |
| Conditional expressions using logical/comparison results | Completed (bounded evidence) |
| Scalar arithmetic beyond `.sum`/`.multiply` | Completed (bounded v2.5.1 arithmetic/unary, transcendental, and decimal post-processing slices; `.range` remains separately evidenced) |
| Scalar string operations (`.string`, `.concatenate`, case/predicate family) and bounded `.plaintext` projection | Completed (bounded v2.5.1 slice; Dynamic String conversion deferred) |
| General typed value conversion | Planned (gap inventory; split by semantic family) |
| Error/absence helpers (`.none`, `.isnone`, `.otherwise`, `.ifpresent`, `.takeif`) | Completed (bounded callback evidence; full error taxonomy remains partial) |
| Callable/function evaluator foundation | Completed (typed callables, lexical capture, caller lookup overlay, parameter precedence, scoped owner writeback, and failure atomicity) |
| Iteration (`.foreach` / `.repeat`) | Completed (evidenced block and native contextual inline callable-body slice; broader consumers and generalized inline bodies deferred) |
| Document-state foundation | Completed (evaluator-owned state with `.docname`, `.docdescription`, `.doctype`, and serializable IR snapshot) |
| Bounded typed layout/component consumers | Completed (Stacked row/column/positive-column grid, bounded Container consumers, and Landscape; complete public surface remains partial) |
| Public-language gap inventory and current slice ordering | Completed (see `docs/compatibility/quarkdown/GAP_INVENTORY.md`) |

Public Quarkdown features not yet implemented are tracked compatibility debt;
they are not permanent product exclusions. The VirtualProject/ResourceProvider
model has `SUPPORTED_SEMANTICS` evidence, while `.read`, `.json`, and `.include`
are only `PARTIAL` source-relative bounded builtin slices owned by #155 and
#188. Remaining data-loading families require separate host/resource evidence;
the complete status and order are in
[`RECONCILIATION.md`](compatibility/quarkdown/RECONCILIATION.md).
The callable/function foundation, iteration slice, document-state foundation,
and bounded typed layout/component consumers above are implemented and
evidenced, while broader function-driven metadata, style/layout families,
generalized conversions, and remaining programmable-document functionality
continue into M3+ and later convergence. See the gap inventory for the complete
evidence-backed classification. v0.1 may be partial.

### M2 closure evidence

The M2 completion audit is independently reviewable from the repository:

- [current product status and Markdown evidence](../README.md)
- [Markdown capability matrix and differential-corpus workflow](compatibility/markdown/README.md)
- [Quarkdown capability matrix and v2.5.1 gap inventory](compatibility/quarkdown/README.md)
- [executable independently authored Quarkdown conformance corpus](../fixtures/quarkdown-conformance/README.md)
- [raw HTML policy](compatibility/RAW_HTML_POLICY.md) and [resource-context contract](adr/0019-typst-source-and-resource-context.md)
- [real Typst/PDF backend integration coverage](../crates/arkst-typst-subprocess/tests/backend_integration.rs)

This closure means the bounded M2 baseline is implemented and evidenced. It
does not claim complete v2.5.1 compatibility; deferred and partial families
remain compatibility debt for M3+ and later convergence.

## M3 — Programmable Documents

**Status:** In progress (bounded document-state and callable slices)

**Objective:** Broader components/style/layout families, remaining data loading,
and later programmable-document convergence beyond the evidenced M2 slices.

| Item | Status |
|------|--------|
| Bounded `.docauthor` document-state semantics | Completed (v2.5.1 evidenced slice) |
| Bounded `.docauthors` document-state semantics | Completed (v2.5.1 evidenced slice) |
| Bounded `.dockeywords` document-state semantics | Completed (v2.5.1 evidenced slice) |
| Bounded `.doclang` document-state semantics | Completed (deterministic Temurin 25.0.4.1+1 CLDR locale closure at the evaluator/IR boundary; localization and rendering deferred) |
| Bounded `.theme` document-state semantics | Completed (bounded evaluator/IR boundary; upstream block-body fallback deferred) |
| Bounded `.captionposition` document-state semantics | Completed (bounded evaluator/IR snapshot; caption rendering deferred) |
| Bounded Quarkdown v2.5.1 `.extend` / `.super` evaluator semantics (#169) | Completed (source-defined and regular scalar-native targets; specialized native owners and renderer/output compatibility deferred) |
| Quarkdown v2.5.1 layout/document-configuration audit (#153) | Completed (20-row canonical inventory; #175–#178 follow the dependency bands in [#156 reconciliation](compatibility/quarkdown/RECONCILIATION.md)) |
| Quarkdown v2.5.1 content/media/Markdown-extension audit (#154) | Completed (83-row canonical inventory; #181–#185 and the bounded #198–#199 follow the shared engine/backend/content prerequisites in [#156 reconciliation](compatibility/quarkdown/RECONCILIATION.md)) |
| Broader document metadata and programmable-document convergence | #152 audited the v2.5.1 document-state surface; implementation ordering, including #187 as the next technical work, is defined by [#156 reconciliation](compatibility/quarkdown/RECONCILIATION.md) |

## M4 — Developer Experience

**Status:** Not started

**Objective:** Watch mode, inspect commands, source maps, structured diagnostics.

## M5 — Quarkdown Compatibility Convergence

**Status:** Not started

**Objective:** Converge toward complete verified compatibility with the
publicly documented Quarkdown language as the stable upstream target advances.

| Item | Status |
|------|--------|
| Public-language gap inventory and impact reports | Not started |
| Independently authored conformance expansion | Not started |
| Human-reviewed verified-baseline promotion procedure | Completed |
| Adaptation PR preparation and verification automation | Not started |
| Typst generated-source compatibility corpus | Not started |

Full verified compatibility with the selected baseline is a major pre-1.0
quality objective. It is not complete merely because a release is detected or
because a feature is documented upstream.

## M6 — Library API, LSP, WASM Bindings

**Status:** Not started

**Objective:** Embedding, editor integration, `arkst-wasm` bindings crate.

WASM compilation is an M0 architecture constraint (core + lowering).
M6 delivers the `arkst-wasm` bindings crate and WASM CI coverage.

## M7 — Hardening

**Status:** Not started

**Objective:** Fuzzing, benchmarks, security audit, 1.0 release.

---

## Explicitly Deferred Work

- Browser-side Typst compilation (arkst-typst-web, gate behind feasibility)
- LSP server (deferred to M6, core API must stabilize first)
- Package registry (not planned)
- Web editor / SaaS (not planned)
- JavaScript plugin runtime (not planned)
- Mature automated Quarkdown adaptation pipeline (future M5 work)
- Automated Typst compatibility watcher and adaptation PRs (future work)
