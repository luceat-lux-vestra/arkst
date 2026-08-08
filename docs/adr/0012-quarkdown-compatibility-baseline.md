# ADR-0012: Quarkdown Compatibility Baseline

- **Status:** Accepted
- **Date:** 2026-08-08
- **Owners:** Scribium maintainers
- **Related issues:** #25

## Context

The compatibility documentation (see `docs/compatibility/quarkdown/README.md`)
has fixed the Quarkdown compatibility target at **0.9.x**. The current
upstream *stable* release is **Quarkdown v2.5.0** (released 2026-08-04;
`iamgio/quarkdown` tag `v2.5.0`). Since 0.9.x, upstream has added or documented
further syntax and functionality, including:

- line continuation (backslash at end of line continues the argument list)
- tight / brace-wrapped calls (a call wrapped in curly braces can appear
  adjacent to word characters)
- `::` chaining (`.a::b` desugars to `.b {.a}`)
- multi-line arguments
- new built-in functions (e.g. `.json` data loading, `.markdown`, `.llmstxt`,
  `.code` as a primitive)

Even though the basic dot-prefixed call + brace-argument model is unchanged
from 0.9.x through 2.5.0 (verified against the public wiki, accessed
2026-08-08), keeping an ancient version as the reference baseline makes the
reference for future semantic/evaluator work unclear, and misrepresents the
Quarkdown syntax today's users actually write.

This ADR decides the *reference version*. The *approach* to compatibility
(documented subset, clean-room implementation, per-feature compatibility
levels) remains governed by ADR-0007.

## Decision

Adopt the following compatibility policy:

1. The reference baseline for Scribium's Quarkdown compatibility is
   **Quarkdown v2.5.0**.
2. Scribium's compatibility contract is the **documented subset**: only the
   features explicitly listed in the compatibility matrix
   (`docs/compatibility/quarkdown/README.md`) and verified by conformance
   tests are part of the contract.
3. Feature-by-feature support is recorded in the compatibility matrix.
4. Implementing every upstream feature is **not** a goal.
5. A new upstream release does **not** automatically move the baseline.
6. Changing the baseline requires an explicit review and documentation pass:
   1. investigate upstream changes,
   2. update the compatibility matrix,
   3. amend/supersede this ADR or add a new ADR if needed,
   4. update conformance fixtures and provenance records
      (`docs/compatibility/quarkdown/SPEC_SOURCES.md`).
7. The clean-room policy (ADR-0007) is unchanged: no upstream implementation
   code is copied, and no internal implementation is relied upon.

This complements, and does not supersede, ADR-0007: ADR-0007 defines *how*
compatibility is selected and recorded (documented subset, clean-room,
explicit divergence tracking); this ADR defines *which version* is the current
reference for that subset.

## Considered Options

### Option 1: Stay on 0.9.x (rejected)

Keeping the old target makes the semantic/evaluator reference stale: syntax
that current Quarkdown users employ (line continuation, chaining, tight-call
wrapping, newer builtins) would not be visible in the reference at all, and
evaluator work would have to reconstruct behavior from an obsolete snapshot.

### Option 2: Automatically track the latest release (rejected)

Follow-up releases would silently change the compatibility contract. The
documented-subset contract requires a fixed reference so that each feature's
compatibility claim is stable and testable; auto-tracking breaks the
determinism of the matrix and the conformance suite.

### Option 3: Wide version range as a compatibility claim (rejected)

Claiming compatibility with a range such as `>=0.9,<3.0` overstates the
contract: compatibility is decided per feature, not per version axis. A range
provides no testable reference for any single feature and is indistinguishable
from claiming "everything".

## Consequences

### Positive

- Evaluator implementation (M1+; see the follow-up plan) gets a clear
  semantic reference: the v2.5.0 documentation and release.
- The gap between the documented target and what current Quarkdown users see
  is reduced.
- Feature-level compatibility is preserved, while upstream changes are kept
  under review-driven control (no implicit target migration).

### Negative

- Readers may misunderstand the reference as "Scribium supports all of
  Quarkdown v2.5.0". The compatibility document must therefore emphasize the
  documented-subset model and the "not claimed" caveat for any feature not
  listed in the matrix.
- The matrix and provenance records need periodic attention; upstream release
  reviews (decision point 6) become a normal part of ADR/roadmap churn.

### Risks

- User expects "Quarkdown compiler" and finds missing features.
- Mitigation: explicit README status table + matrix with unclaimed rows,
  NEVER claim full compatibility (see also ADR-0007).
- Docs and wiki may lag the v2.5.0 release; only behavior actually verified
  against the release/wiki is recorded in `SPEC_SOURCES.md`; no guessing.

## References

- ADR-0007: Quarkdown compatibility scope and clean-room process
- `docs/compatibility/quarkdown/README.md` — compatibility matrix
- `docs/compatibility/quarkdown/SPEC_SOURCES.md` — provenance records
- Quarkdown v2.5.0 release: https://github.com/iamgio/quarkdown/releases/tag/v2.5.0
- Quarkdown wiki "Syntax of a function call":
  https://quarkdown.com/wiki/syntax-of-a-function-call/ (accessed 2026-08-08)
- Issue #25: Re-evaluate compatibility target: 0.9.x vs current upstream (v2.5.0)