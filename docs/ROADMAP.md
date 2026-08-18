# Roadmap — Scribium

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

**Architecture constraint:** `scribium-core` + `scribium-typst` (lowering)
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

**Status:** In progress

**Objective:** Quarkdown core language + Markdown MVP for v0.1.0, with honest
partial compatibility claims and a growing evidence-backed baseline.

| Item                                | Status       |
|-------------------------------------|--------------|
| Document-scope variable evaluation  | Completed    |
| Generic callables, native iterable operations, and collection transforms | Completed (evidenced slice; `.map`/`.filter` retained as Scribium extensions) |
| Logical/comparison predicates (`.islower`, `.isgreater`, `.equals`, `.not`) | Completed (bounded v2.5.1 slice) |
| Conditional expressions using logical/comparison results | Completed (bounded evidence) |
| Scalar arithmetic beyond `.sum`/`.multiply` | Completed (bounded v2.5.1 arithmetic/unary slice; `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, and `.round` deferred) |
| Scalar string operations (`.string`, `.concatenate`, case/predicate family) | Completed (bounded v2.5.1 slice; `.plaintext` and general conversion deferred) |
| General typed value conversion | Planned (gap inventory; split by semantic family) |
| Error/absence helpers beyond `.isnone`/`.otherwise` | In progress (bounded evidence) |
| Public-language gap inventory and current slice ordering | Completed (see `docs/compatibility/quarkdown/GAP_INVENTORY.md`) |

Public Quarkdown features not yet implemented are tracked compatibility debt;
they are not permanent product exclusions. Include/read/data loading is
deferred to M3 because it crosses the host filesystem boundary; function-driven
metadata and layout/components are deferred to M3+; see the gap inventory for
the complete evidence-backed classification. v0.1 may be partial.

## M3 — Programmable Documents

**Objective:** Components, data loading, iteration, resource limits.

## M4 — Developer Experience

**Objective:** Watch mode, inspect commands, source maps, structured diagnostics.

## M5 — Quarkdown Compatibility Convergence

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

**Objective:** Embedding, editor integration, `scribium-wasm` bindings crate.

WASM compilation is an M0 architecture constraint (core + lowering).
M6 delivers the `scribium-wasm` bindings crate and WASM CI coverage.

## M7 — Hardening

**Objective:** Fuzzing, benchmarks, security audit, 1.0 release.

---

## Explicitly Deferred Work

- Browser-side Typst compilation (scribium-typst-web, gate behind feasibility)
- LSP server (deferred to M6, core API must stabilize first)
- Package registry (not planned)
- Web editor / SaaS (not planned)
- JavaScript plugin runtime (not planned)
- Mature automated Quarkdown adaptation pipeline (future M5 work)
- Automated Typst compatibility watcher and adaptation PRs (future work)
