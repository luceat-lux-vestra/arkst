# Roadmap — Scribium

Status markers: `Not started` | `In progress` | `Completed` | `Deferred`

## M0 — Foundation

**Objective:** Establish legal boundaries, technology choices, and repository structure.

| Item                    | Status       |
|-------------------------|--------------|
| Repository bootstrap    | In progress  |
| LICENSE/NOTICE          | In progress  |
| Product documentation   | In progress  |
| Name due diligence      | Not started  |
| Typst integration spike | Not started  |
| Markdown parser spike   | Not started  |
| ADR 0001–0010           | Not started  |
| GitHub templates/CI     | Not started  |

**Dependencies:** None

## M1 — Quarkdown-Compatible Vertical Slice

**Objective:** First end-to-end `.qd → Typst → PDF` pipeline.

Acceptance: dot-prefixed calls, positional/named/body arguments, basic conditional,
front matter, deterministic output.

## M2 — Quarkdown Core Language + Markdown MVP

**Objective:** Production-ready Quarkdown core subset and Markdown baseline.
v0.1.0 release.

## M3 — Programmable Documents

**Objective:** Components, data loading, iteration, resource limits.

## M4 — Developer Experience

**Objective:** Watch mode, inspect commands, source maps, structured diagnostics.

## M5 — Quarkdown Compatibility Expansion

**Objective:** Expanded compatibility subset, matrix, conformance suite.

## M6 — Library API, LSP, WASM

**Objective:** Embedding, editor integration, WASM feasibility.

## M7 — Hardening

**Objective:** Fuzzing, benchmarks, security audit, 1.0 release.

---

## Explicitly Deferred Work

- WASM support (deferred to M6, feasibility decision first)
- LSP server (deferred to M6, core API must stabilize first)
- Package registry (not planned)
- Web editor / SaaS (not planned)
- JavaScript plugin runtime (not planned)
- Full Quarkdown 100% compatibility (not a goal)