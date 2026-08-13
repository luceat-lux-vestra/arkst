# Scribium — Agent Operations Guide

## Project summary

Scribium is an independent Apache-2.0 compiler/toolchain targeting complete
compatibility with the publicly documented Quarkdown document language and
document-observable semantics of the tracked stable upstream release. Current
verified compatibility is partial and evidence-based. Clean-room independence
is an implementation constraint, not a reason to make public language features
permanently out of scope.

- **Current milestone:** M2 Core Language + Markdown MVP
- **Stability:** Pre-alpha, experimental
- **Backend:** official Typst compiler, through accepted lowering and host
  adapter boundaries
- **Non-goals:** custom PDF/HTML renderers, SaaS, package registry, arbitrary
  document shell/process/network access

## Authority and task start

Read in this order before making a change:

1. the latest explicit task constraints and linked issue/PR;
2. relevant accepted and superseding ADRs;
3. `docs/ARCHITECTURE.md`;
4. the relevant product or compatibility specification;
5. `docs/ENGINEERING.md`;
6. `docs/ROADMAP.md` for sequencing/context; and
7. current implementation and tests as evidence of behavior and migration
   state.

The authority order is the same: explicit maintainer/task constraint,
accepted/superseding ADRs, architecture, product/domain specifications,
engineering standard, roadmap, then current code/tests. `AGENTS.md` is an
operational summary; it does not override the documents it references.

There is no `SCRIBIUM_MASTER_EXECUTION_BRIEF.md` authority in this repository.
Do not invent or preserve a phantom reference to it. Current physical code may
be migration state and does not override accepted target architecture.

At task start, preserve unrelated work and verify:

```bash
git status --short --branch
git log -1 --oneline --decorate
```

Confirm the checkout, branch/base, and task attachment before editing. Do not
use sibling or external worktrees without explicit approval.

## Accepted architecture guardrails

ADR-0014, ADR-0015, and `docs/ARCHITECTURE.md` define target ownership even
when crates have not yet been physically extracted. Do not infer target design
from current file placement.

- One authoritative `BlockParser` belongs to the Markdown frontend.
- `scribium-markdown -> scribium-quarkdown`; the Quarkdown crate owns grammar
  only and does not depend on Markdown AST/parser types.
- `scribium-project` owns the in-memory project model; host code owns native
  filesystem/configuration/process composition.
- `scribium-engine` owns semantic analysis/evaluation/normalization.
- `scribium-compat` owns compatibility policy; current core compatibility
  modules are migration state, not a new ownership decision.
- `scribium-ir` owns one backend-neutral `IrDocument`; do not introduce HIR,
  MIR, Typst-oriented IR, or backend-specific raw escape variants.
- `scribium-typst` owns pure IR→Typst lowering and the platform-neutral
  backend contract; concrete execution belongs to the selected host adapter.
- No core→Typst dependency, `RawTypst`, `BackendRaw`, or generic backend-code
  escape hatch.
- `scribium-html` isolates HTML normalization and xberg; Pandoc is an optional
  development/compatibility oracle only.
- Platform-neutral compiler crates remain filesystem-, process-, and
  network-free and WASM-capable. Security capabilities require accepted host
  architecture.
- Rushdown is Scribium's Markdown parsing substrate. Do not implement
  CommonMark/GFM parsing from scratch unless a later accepted architecture
  decision explicitly requires it.
- Do not add Markdown or Quarkdown parser implementations back to
  `scribium-core`.
- Quarkdown extensions belong in `scribium-markdown` and
  `scribium-quarkdown`, never in a Rushdown fork. Rushdown types must not
  escape `scribium-markdown`.
- Any Rushdown version change requires parser, provenance, WASM, CommonMark/
  GFM, and safety validation; exact revisions are reviewed rather than
  automatically upgraded.

Do not start crate extraction, BlockParser refactoring, IR migration, Typst
crate splitting, new Quarkdown semantics, or mature upstream automation in a
documentation/governance task.

## Compatibility operating rules

ADR-0016 is the authority for compatibility policy:

- the **tracked upstream target** is the latest stable Quarkdown release and
  automatically becomes adaptation work;
- the **verified compatibility baseline** is the last release supported by
  reviewed public evidence, independent fixtures, passing tests, and
  documented divergences;
- current matrix claims must be evidence-backed and may be partial;
- missing public language behavior is compatibility debt, not a permanent
  selected-subset product goal; and
- stable upstream changes may use public documentation, release notes,
  reference/API docs, independent fixtures, and permitted black-box evidence,
  never copied/translated upstream source or tests.

The existing Quarkdown release observer is an early foundation: release
detection → drift issue. Its target evolution is evidence/delta → impact
analysis → independent conformance → adaptation PR → verification → baseline
promotion. Agents must stop for architecture review when adaptation requires
new ownership, dependency direction, abstractions, semantics/IR, security
capabilities, intentional divergences, weakened invariants, plugins, or
backend escape hatches. Human review/merge remains authoritative.

Typst is tracked separately as a generated-source/backend contract. Do not
reimplement Typst grammar. See `docs/compatibility/typst/README.md`.

## Engineering rules

`docs/ENGINEERING.md` is the detailed project quality contract. The
non-negotiable summary is:

- correctness before velocity; fix the owning invariant, not the visible test;
- handle malformed/adversarial input, deterministic output, limits, and errors;
- preserve source identity and reliable spans; never invent provenance;
- keep public APIs minimal, documented, and inside accepted ownership;
- use typed Rust errors and structured E1xxx–E9xxx diagnostics; no panics or
  silent fallbacks for user errors;
- avoid speculative abstractions and patch-shaped special cases;
- review dependencies for ownership, license, security, WASM, and transitive
  cost;
- maintain host/security boundaries and resource limits; and
- test behavior/invariants, not implementation structure.

## Rust and library implementation discipline

See `docs/ENGINEERING.md` for the full standard. Forward-looking Rust work
must also preserve these non-negotiables:

- `unsafe` is prohibited by default; any exception requires an accepted
  architecture/security decision or maintainer approval, a narrow boundary,
  documented safety invariants, justification, tests, and review.
- Do not use production/library `unwrap` or `expect` for user input, malformed
  documents, recoverable failures, or normal compiler/backend failure paths.
  Tests, bootstrap infrastructure, and trivially guaranteed invariants are
  the limited exceptions.
- Do not introduce hidden global mutable state. Pass compiler state through
  explicit ownership/context and preserve deterministic behavior.
- Keep platform-neutral compiler/project abstractions free of native
  `PathBuf`/filesystem assumptions; native paths belong in CLI/host/adapters.
- Libraries do not call `process::exit`; use structured errors, diagnostics,
  no silent fallback, and no user-facing panic.

## ADR history discipline

Do not rewrite historical ADR decisions, considered options, consequences, or
rationale to make them agree with newer architecture. Use explicit
supersession/addendum relationships and preserve the original record.

## Required checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p scribium-core
cargo run -p scribium-cli -- build examples/hello/main.qd
cargo run -p scribium-cli -- build examples/hello/main.qd --format pdf
cargo run -p scribium-cli -- inspect examples/hello/main.qd --emit typst
```

Run relevant checks proportional to risk and report checks that could not be
run. Do not disable CI or weaken a test to make a change pass.

| Change | Minimum evidence |
|--------|------------------|
| Parser/frontend | unit, snapshot/golden as applicable, malformed and boundary input, provenance/span assertions |
| Semantic/engine | unit, golden, error case, evaluation-order/scope/determinism invariants |
| Typst lowering | deterministic golden output, source-map behavior, real compiler validation where appropriate |
| Diagnostics | source-span and structured diagnostic snapshots |
| CLI/config | integration/help snapshots and invalid/default/migration cases |
| Compatibility | independent fixture, provenance record, compatibility level, target/baseline metadata |
| Architecture migration | observable before/after regression evidence; no behavior change unless explicitly requested |

## Agent authority boundary

Agents may choose local details, refactor inside task scope when necessary,
add tests, and improve internal naming. They must not independently decide new
crate ownership/dependency direction, semantic layers or IR tiers,
compatibility exceptions/permanent divergences, security-model changes,
generic plugin systems/backend escape hatches, breaking public API strategy, or
architecture-wide migration sequencing.

When such a decision is necessary:

```text
stop architectural invention
    -> record the blocking question and evidence
    -> request architecture review
```

## End-of-task checklist

1. Format, lint, and run relevant/full tests where feasible.
2. Update affected product, compatibility, architecture, ADR, and changelog
   documentation; do not duplicate the full engineering standard in PR text.
3. Search the repository for stale policy/ownership statements.
4. Review `git diff --check`, the complete diff, and unrelated changes.
5. Keep commits focused; substantial agent-authored commits include:
   `Co-Authored-By: OpenAI Codex <codex@openai.com>`.
6. Report implementation, tests/CI, merge state, and working-tree state
   separately. Do not imply a physical/device check from logs or unit tests.

## Forbidden actions

- Copying Quarkdown source, upstream tests, or prior port code.
- Deleting/weakening failing tests or batch-updating snapshots without review.
- Disabling CI, hiding fixture provenance, or committing secrets.
- Arbitrary shell/network/process execution in compiler documents or core.
- Large-scale refactors, opportunistic API redesign, or architecture invention
  outside the assigned scope.
- Overwriting unrelated user-authored files.
