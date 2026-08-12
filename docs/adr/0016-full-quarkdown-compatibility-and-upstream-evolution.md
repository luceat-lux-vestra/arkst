# ADR-0016: Full Quarkdown Compatibility and Continuous Upstream Evolution

- **Status:** Accepted
- **Date:** 2026-08-12
- **Owners:** Scribium maintainers
- **Related ADRs:** 0007, 0012, 0013, 0014, 0015
- **Supersedes:** The compatibility-target and upstream-adoption portions of ADR-0007, ADR-0012, and ADR-0013

## Context

Scribium is an independent Quarkdown-compatible compiler and toolchain. Earlier
ADRs selected a documented feature subset and a fixed upstream baseline as the
product contract. That policy is no longer the intended product goal. It made
unimplemented public language behavior look permanently out of scope and made
upstream adoption an optional product decision.

The accepted crate topology and migration boundaries are defined by ADR-0014,
ADR-0015, and `docs/ARCHITECTURE.md`. This ADR changes the product and
compatibility policy only. It does not extract crates, refactor `BlockParser`,
migrate the IR, split Typst crates, or otherwise change physical architecture.

## Decision drivers

- complete public-language compatibility is the long-term product objective;
- current claims must remain evidence-based and honest;
- stable upstream evolution must become actionable adaptation work;
- clean-room independence must remain an implementation constraint;
- Typst must be tracked as a backend contract, not reimplemented as a language;
- automation must stop at architecture and security review boundaries; and
- human review and merge must remain the authority boundary.

## Decision 1: complete documented Quarkdown compatibility is the target

Scribium aims for complete compatibility with the publicly documented
Quarkdown document language and document-observable semantics of the tracked
stable upstream release. This target includes publicly specified:

- grammar and syntax, calls, and argument forms;
- variables and scopes;
- functions, lambdas, and components;
- conditionals and iteration;
- built-ins and standard-library document behavior;
- include, read, and data operations subject to Scribium's accepted
  security/project model;
- Markdown extensions introduced by Quarkdown; and
- other publicly documented language constructs and observable evaluation
  behavior.

There is no permanent selected-subset product goal. A documented public
language feature that is not implemented is compatibility debt and planned
work. It is not automatically outside the contract forever.

This target does not require implementation identity. Scribium need not
reproduce Quarkdown's internal implementation, private APIs, undocumented
bugs, internal data structures, private plugin ABI, or compiler architecture.
The target is the public document language and observable document semantics.

For eventual 1.0 readiness, the selected verified Quarkdown baseline is
expected to have no known unaccounted public-language compatibility gaps,
except for explicit documented divergences approved through architecture or
compatibility review.

## Decision 2: distinguish target from verified compatibility

The following concepts are authoritative:

| Concept | Meaning |
|---------|---------|
| **Tracked upstream target** | The latest stable Quarkdown release. It automatically becomes the release Scribium must investigate and adapt toward. |
| **Verified compatibility baseline** | The release for which permitted evidence was reviewed, independent conformance fixtures exist, required implementation work is complete, tests pass, and known divergences are documented. |
| **Current compatibility claim** | Only behavior supported by conformance evidence at a stated compatibility level in the matrix. |

The tracked target is not a product opt-in question. When it advances, the
question is what changed, what it affects, and what work restores full verified
compatibility. A target/baseline lag is allowed, but it is visible and
actionable compatibility debt.

The existing serialized `supported_baseline` field in
`docs/compatibility/quarkdown/upstream.toml` is retained for schema stability
in this incremental change and means the **verified compatibility baseline**.
The observer's `observed_tag`, obtained from the latest stable release, is the
current computed tracked target. A future metadata schema may give those
concepts more explicit field names.

The baseline advances only after:

1. permitted upstream public evidence has been reviewed;
2. affected behavior has been identified;
3. independent conformance fixtures exist;
4. required implementation changes are complete;
5. regression and conformance tests pass; and
6. known divergences are documented and reviewed.

## Decision 3: preserve clean-room independence

Complete compatibility is a product target; clean-room independence is an
implementation constraint. These are compatible goals.

Compatibility work may rely on public documentation, public release notes,
public API/reference documentation where applicable, independently authored
fixtures, and permitted black-box behavior observation. It must not make
Quarkdown implementation source code a dependency of the process. Scribium
contributors and automation must not copy or translate upstream implementation
code, upstream tests, or upstream fixtures.

Any deliberate permanent divergence from publicly documented language behavior
requires an explicit rationale, compatibility documentation, diagnostics where
appropriate, and an architecture/compatibility decision when substantial. The
implementation-identity distinction must not be used to silently exclude a
public language feature.

## Decision 4: Quarkdown upstream evolution model

Stable releases are the primary adoption boundary. Arbitrary development
branch commits are not compatibility requirements unless separately approved.
The intended mature pipeline is:

```text
new stable Quarkdown release
    -> release detection
    -> permitted public documentation/release-note delta collection
    -> structured impact report
    -> affected grammar/semantics/stdlib/compatibility areas
    -> independently authored conformance updates
    -> implementation/adaptation PR
    -> conformance and regression verification
    -> review gate
    -> verified baseline promotion
```

The current release observer implements only the early foundation:

```text
latest stable release detection -> deduplicated drift issue
```

It remains useful and must not be removed. Its issue is an adaptation-work
entry point, not the final evolution strategy and not a request to decide
whether the project wants to support the release.

Over time, automation or agents may detect drift, collect permitted public
evidence, classify likely affected features, prepare fixtures, update
implementation where the accepted architecture permits it, create adaptation
PRs, run validation, and prepare baseline-promotion changes.

Automation must stop at an explicit architecture-review-required boundary if
the change would require:

- new crate ownership or dependency direction;
- a new public abstraction or semantic model redesign;
- a new security capability;
- an intentional compatibility divergence;
- weakening an existing invariant;
- a generic extension/plugin architecture; or
- a backend-specific escape hatch.

Automation may not invent architecture. Human review and merge remain the
authority boundary.

## Decision 5: Typst is a backend compatibility target, not a language to reimplement

Scribium does not reimplement Typst grammar. Under ADR-0015, the target flow is:

```text
normalized backend-neutral IrDocument
    -> scribium-typst
    -> generated Typst source
    -> concrete Typst compiler adapter
    -> official Typst compiler
```

The latest stable Typst release automatically becomes the backend adaptation
target. The last fully validated release remains distinguishable as the
verified backend baseline. Tracking protects the generated-source and adapter
contract, including:

- emitted Typst syntax and changed/deprecated constructs;
- Typst CLI behavior used by the subprocess adapter;
- output capabilities relevant to Scribium;
- future in-process compiler/backend API changes; and
- diagnostic and source-map implications.

The intended future pipeline is:

```text
new stable Typst release
    -> release/change detection
    -> official release notes/documented compatibility changes
    -> generated-Typst corpus validation
    -> compilation with the new stable compiler
        -> success: compatibility evidence
        -> failure: lowering/adapter impact classification -> adaptation PR
```

This policy does not create a Scribium Typst parser. If native `.typ`
passthrough is implemented in the future, the selected official Typst compiler
normally owns syntax evolution; Scribium does not add a generic raw-backend
escape hatch to backend-neutral IR. The future watcher and machine-readable
baseline are separate implementation work documented in
`docs/compatibility/typst/README.md`.

## Decision 6: Markdown, HTML, and Pandoc have different tracking roles

Markdown/CommonMark remains a specification-driven frontend. It does not need
Quarkdown-style release adaptation machinery unless its selected specification
or baseline changes. This ADR does not add a Markdown watcher.

HTML interoperability remains isolated behind `scribium-html`; the
`html-to-markdown-rs`/xberg dependency is hidden behind that accepted boundary
and is maintained as a normal dependency. HTML dependency maintenance is not
Quarkdown-language compatibility work.

Pandoc remains an optional development/compatibility oracle only. It is not a
Scribium runtime dependency, language authority, or production subprocess.

## Decision 7: authority and migration boundary

The authority order for this policy is:

```text
latest explicit maintainer/task constraint
    > accepted/superseding ADRs
    > docs/ARCHITECTURE.md
    > product and compatibility specifications
    > docs/ENGINEERING.md
    > ROADMAP for sequencing
    > current physical code/tests as implementation state
```

AGENTS.md is the operational entry point and summarizes these rules; it does
not override the documents it points to. Current code describes implementation
and migration state and does not override accepted target architecture.

This ADR does not reopen or redesign ADR-0014/0015 decisions: the authoritative
Markdown `BlockParser`, frontend dependency direction, Quarkdown grammar-only
responsibility, source/project/engine/diagnostics/compatibility/IR ownership,
one backend-neutral IR, Typst lowering/execution separation, HTML/xberg
isolation, Pandoc oracle role, no core → Typst dependency, no `RawTypst` or
generic backend-raw escape hatch, and host security boundaries remain intact.

## Consequences

### Positive

- public Quarkdown language compatibility is an explicit long-term objective;
- incomplete implementation is visible as evidence-backed compatibility debt;
- every stable release has an automatic adaptation target;
- the existing observer has a clear incremental role;
- Typst tracking protects the actual generated-source/backend contract; and
- future automation has a clear human and architecture review boundary.

### Negative

- the compatibility matrix and conformance corpus require ongoing work;
- the target may temporarily lead the verified baseline; and
- future automation needs impact analysis and independent fixture generation,
  not only release detection.

### Intentionally deferred

This PR does not implement public Quarkdown features, the mature Quarkdown
adaptation pipeline, a Typst watcher, crate extraction, BlockParser refactoring,
IR migration, Typst crate splitting, or any other physical compiler migration.
Those are later, independently reviewable implementation units.

## References

- ADR-0007 — clean-room process, partially superseded only for target scope
- ADR-0012 — historical fixed-baseline policy, superseded for compatibility policy
- ADR-0013 — current observer foundation, superseded for the final evolution model
- ADR-0014 — Markdown BlockParser foundation
- ADR-0015 — compiler crate boundaries
- `docs/ARCHITECTURE.md`
- `docs/ENGINEERING.md`
- `docs/compatibility/quarkdown/README.md`
- `docs/compatibility/typst/README.md`
