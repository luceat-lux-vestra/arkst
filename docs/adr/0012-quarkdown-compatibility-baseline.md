# ADR-0012: Quarkdown Compatibility Baseline

- **Status:** Superseded
- **Date:** 2026-08-08
- **Owners:** Scribium maintainers
- **Related issues:** #25
- **Superseded by:** ADR-0016

## Context

The compatibility documentation (see `docs/compatibility/quarkdown/README.md`)
had fixed the Quarkdown compatibility target at **0.9.x**. The upstream stable
release selected for the then-current evidence baseline was **Quarkdown v2.5.0** (released 2026-08-04;
`iamgio/quarkdown` tag `v2.5.0`). Since 0.9.x, upstream has added or documented
further syntax and functionality, including:

- line continuation (backslash at end of line continues the argument list)
- tight / brace-wrapped calls (a call wrapped in curly braces can appear
  adjacent to word characters)
- `::` chaining (`.a::b` desugars to `.b {.a}`)
- multi-line arguments
- new built-in functions (e.g. `.json` data loading, `.markdown`, `.llmstxt`,
  `.code` as a primitive)

The current v2.5.0 documentation (public wiki, accessed 2026-08-08)
describes the same basic dot-prefixed call + brace-argument model on which
Scribium's existing parser subset is based. Scribium's earlier compatibility
baseline was 0.9.x; no claim is made that the upstream grammar was verified
to be identical in every release in between. Keeping an ancient version as
the reference baseline nonetheless makes the reference for future
semantic/evaluator work unclear, and misrepresents the Quarkdown syntax
today's users actually write.

This ADR recorded a reference version for the earlier compatibility policy.
ADR-0016 now defines the product target, automatic stable-release adaptation
target, and verified-baseline promotion policy. The v2.5.0 selection and its
evidence discipline remain useful as historical/current baseline data.

## Historical decision and retained evidence

The following decisions were made under the superseded fixed-baseline policy:

1. The reference baseline for the then-current Scribium evidence was
   **Quarkdown v2.5.0**.
2. Feature-by-feature support was recorded in the compatibility matrix.
3. Claims required independent evidence rather than documentation alone.
4. Moving the reference baseline required an explicit review and documentation
   pass:
   1. investigate upstream changes,
   2. update the compatibility matrix,
   3. amend/supersede this ADR or add a new ADR if needed,
   4. update conformance fixtures and provenance records
      (`docs/compatibility/quarkdown/SPEC_SOURCES.md`).
5. The clean-room policy (ADR-0007) remained unchanged: no upstream implementation
   code is copied, and no internal implementation is relied upon.

ADR-0016 supersedes the fixed selected-subset contract and the idea that a new
stable release is an optional adoption decision. It retains the useful rule
that a verified baseline promotion requires reviewed public evidence,
independent fixtures, passing tests, and documented divergences.

## Considered Options

### Option 1: Stay on 0.9.x (rejected)

Keeping the old target makes the semantic/evaluator reference stale: syntax
that current Quarkdown users employ (line continuation, chaining, tight-call
wrapping, newer builtins) would not be visible in the reference at all, and
evaluator work would have to reconstruct behavior from an obsolete snapshot.

### Option 2: Automatically promote a baseline without evidence (rejected)

An unreviewed baseline promotion would silently turn a release observation into
a compatibility claim. ADR-0016 instead makes the latest stable release the
automatic adaptation target while keeping verified-baseline promotion
evidence-gated.

### Option 3: Wide version range as a compatibility claim (rejected)

Claiming compatibility with a range such as `>=0.9,<3.0` overstates the
contract: compatibility is decided per feature, not per version axis. A range
provides no testable reference for any single feature and is indistinguishable
from claiming "everything".

## Consequences

### Positive

- Evaluator implementation received a clear historical semantic reference: the
  v2.5.0 documentation and release.
- The gap between the documented target and what current Quarkdown users see
  is reduced.
- Feature-level evidence and review discipline are retained under ADR-0016.

### Negative

- Readers may misunderstand the historical reference as a full current claim.
  The compatibility document must distinguish the complete target, verified
  baseline, and current feature evidence.
- The matrix and provenance records need periodic attention; upstream release
  reviews (decision point 6) become a normal part of ADR/roadmap churn.

### Risks

- User expects current full compatibility before evidence exists.
- Mitigation: explicit README status, verified-baseline metadata, matrix
  evidence, and compatibility-debt tracking.
- Docs and wiki may lag the v2.5.0 release; only behavior actually verified
  against the release/wiki is recorded in `SPEC_SOURCES.md`; no guessing.

## References

- ADR-0007: Quarkdown compatibility scope and clean-room process
- `docs/compatibility/quarkdown/README.md` — compatibility matrix and evidence
- `docs/compatibility/quarkdown/SPEC_SOURCES.md` — provenance records
- Quarkdown v2.5.0 release: https://github.com/iamgio/quarkdown/releases/tag/v2.5.0
- Quarkdown wiki "Syntax of a function call":
  https://quarkdown.com/wiki/syntax-of-a-function-call/ (accessed 2026-08-08)
- Issue #25: Re-evaluate compatibility target: 0.9.x vs current upstream (v2.5.0)
- ADR-0016: Full Quarkdown Compatibility and Continuous Upstream Evolution
