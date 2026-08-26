# ADR-0021: Optional in-process Typst backend after #187

- **Status:** Accepted
- **Date:** 2026-08-26
- **Owners:** Scribium maintainers
- **Related issues:** #187, #200, #201, #12, #188, #190, #191
- **Supersedes:** the in-process decision and re-evaluation timing in
  ADR-0011; refines the future-in-process portion of ADR-0005

## Context

ADR-0005 selected the subprocess adapter for the M1 vertical slice and left
in-process embedding for a feasibility investigation. ADR-0011 recorded the
historical #12 investigation and deferred adoption because a clean
`World`/resource integration had not been demonstrated and the cost/risk was
unknown. That historical decision remains correct for its evidence and is not
rewritten here.

Issue #187 re-evaluated the question against the current `VirtualProject`,
`SourceStore`, `AssetStore`, source-identity, and native adapter boundaries.
The complete evidence is in
[`docs/research/typst-inprocess-187.md`](../research/typst-inprocess-187.md).

## Decision

Scribium accepts a **native, optional in-process Typst adapter** in a
dedicated `scribium-typst-inprocess` crate. The adapter may be used as an
explicitly selected backend after the production follow-up is reviewed, but
the subprocess backend remains the default and supported baseline.

The adapter must:

- consume the existing `TypstBackend` contract after `scribium-typst` has
  generated Typst source;
- map Typst's public `World` interface from an existing `VirtualProject`;
- keep source, asset, font, path, and project-boundary policy owned by
  Scribium's virtual project model;
- fail closed for unavailable package/network capabilities;
- keep Typst compiler types inside the native adapter crate;
- remain separate from the platform-neutral `scribium-typst` lowering and its
  `wasm32-unknown-unknown` boundary; and
- retain the subprocess backend as the default/fallback path.

This is **GO — optional in-process only**, not a default-backend migration and
not approval for browser/WASM rendering.

## Evidence and changed assumptions since #12

The current public Typst 0.15.1 compile and PDF APIs were sufficient to build a
`ProjectWorld` over `VirtualProject`. Real generated Scribium source compiled
to valid PDFs. Images, project fonts, missing resources, traversal, package
denial, fail-closed date behavior, repeated reads, and structured failure
diagnostics were exercised. A subprocess parity test covered generated
multi-page success and invalid-source failure classification.

The workspace temporarily pins citationberg's upstream PR #44 merge revision
`06a591e2f237d25e1dfdedac3f3d1494c496c52d` because the crates.io `0.7.0`
release still selects vulnerable quick-xml 0.38.x. This exact immutable patch
resolves quick-xml 0.41.0 without suppressing the advisory or changing the
Typst release line. It is to be removed when a fixed citationberg release is
available.

The assumptions that changed since #12 are:

1. `VirtualProject` now provides an established immutable source/asset model
   and normalized project-relative paths, so the adapter does not need a second
   filesystem abstraction.
2. The platform-neutral lowering/native execution split is now an accepted
   physical boundary, so Typst types can be contained without entering IR or
   frontend crates.
3. The current evidence also makes the cost concrete: the in-process graph had
   373 unique normal dependency-tree lines versus 29 for the subprocess
   adapter, a fresh release adapter-example build took 154.95 seconds versus
   12.58 seconds, and the release example was about 45.6 MB versus 2.0 MB on
   the measured macOS arm64 host.

These results establish viability, not default suitability. The adapter's
source-map handoff, package/date policy, broader corpus parity, and production
opt-in review remain production gates. The native CI matrix has passed on
Ubuntu, macOS, and Windows for the spike; the local performance measurements
remain macOS arm64 measurements.

## Consequences

### Positive

- Native embedders can avoid a process boundary after an explicit opt-in.
- In-process failures can become structured Scribium diagnostics without
  exposing Typst types across semantic boundaries.
- `VirtualProject` remains the resource/security authority.
- The existing subprocess path remains available as the default and fallback.

### Negative

- The optional adapter adds a large Typst compiler/PDF dependency graph and
  materially increases clean-build and binary costs.
- The cargo-deny advisory policy keeps vulnerability and unsoundness advisories
  as hard failures and fails unmaintained advisories for direct workspace
  dependencies. Transitive unmaintained Typst paths remain tracked upstream
  dependency-hygiene debt rather than blocking this spike.
- Typst 0.x public APIs remain version-coupled and require re-validation on
  stable-release changes.
- Generated-source source-map handoff is not yet complete.
- Native Linux/macOS/Windows CI evidence has passed for the spike; broader
  corpus parity and production opt-in review are still required.

### Explicitly not decided

- This ADR does not implement a CLI default switch or remove subprocess.
- It does not add package/network capability, document environment state, or
  browser/WASM rendering.
- It does not authorize direct Scribium IR lowering into Typst `Content`,
  frames, layouts, or renderer internals.

## Follow-up and migration policy

Production promotion requires a reviewed follow-up that adds explicit backend
selection, complete parity/diagnostic/source-map coverage, and native CI
evidence. The subprocess backend remains the rollback path. A default
migration requires a separate maintainer decision after the re-evaluation
trigger in the issue #187 evidence document is satisfied; this ADR does not
authorize automatic migration or auto-removal.

## Implementation status addendum (2026-08-26, issue #200)

The production follow-up keeps the decision above unchanged. The trusted
native CLI now accepts `--backend subprocess|in-process`, with subprocess as
the default. Explicit in-process selection invokes `InProcessBackend` directly
and returns its typed adapter error without fallback. The selection enum lives
at the native host boundary; neither `scribium-core` nor the platform-neutral
`scribium-typst` lowering crate selects a native backend.

The in-process adapter now accepts the lowering-owned source map through its
adapter API. It maps a complete generated Typst diagnostic range to a unique,
validated original `SourceSpan` when possible and omits the span otherwise.
Diagnostic display paths are logical project paths; temporary/native paths and
Typst compiler types do not cross the adapter boundary. The adapter remains
`wasm32`-unavailable, uses only `VirtualProject` resources, and keeps package,
network, and date/environment capabilities fail-closed.

Issue #201 remains the companion work for cross-platform parity corpus and
fixture-level semantic-oracle expansion. Issue #203's citationberg pin and
the related `deny.toml` policy were not changed by this implementation.

## References

- [ADR-0005: Typst backend strategy](0005-typst-backend-strategy.md)
- [ADR-0011: In-process Typst backend feasibility investigation](0011-in-process-typst-backend-feasibility.md)
- [ADR-0015: Compiler crate boundaries](0015-compiler-crate-boundaries.md)
- [ADR-0019: Typst source and resource context](0019-typst-source-and-resource-context.md)
- [Issue #187 spike evidence](../research/typst-inprocess-187.md)
