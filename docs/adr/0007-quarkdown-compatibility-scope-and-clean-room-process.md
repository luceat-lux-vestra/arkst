# ADR-0007: Quarkdown Compatibility Scope and Clean-Room Process

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1, #4
- **Superseded by:** ADR-0016 for the compatibility-target scope only

## Context

Scribium implements Quarkdown-compatible syntax without using Quarkdown source
code. The scope and process for this clean-room implementation must be defined.

## Decision Drivers

- Legal: no risk of copyright infringement
- Practical: users should be able to migrate existing Quarkdown documents
- Transparent: compatibility level must be documented per feature

## Considered Options

### Option 1: Implementation identity at all costs (rejected)

Would require reproducing private implementation details and undocumented
behavior. That is neither required for public-language compatibility nor
permitted by the clean-room constraint.

### Option 2: Public-specification clean-room process (chosen)

Implement public language behavior from permitted evidence only. Each feature
records its specification source, independently authored fixture, evidence,
compatibility level, and known divergence. Unsupported behavior produces an
explicit diagnostic where the owning contract requires one.

### Option 3: No Quarkdown compatibility (rejected by product definition)

Scribium identity IS Quarkdown compatibility. Not optional.

## Decision

Adopt a clean-room process for the complete public-language compatibility target
now defined by ADR-0016. Public documentation, release notes, public
reference/API documentation, independently authored fixtures, and permitted
black-box observations are allowed evidence. Quarkdown implementation source,
tests, and fixtures are not inputs to the process. Each compatibility claim
tracks provenance and evidence; unsupported behavior uses `E8xxx` diagnostics
when appropriate.

The earlier documented-subset wording described the initial evidence boundary,
not a permanent product limitation. ADR-0016 supersedes that target-scope
decision while leaving this clean-room process accepted.

## Consequences

### Positive

- Legally clean — no source code review dependency
- Transparent — users see exactly what is and isn't compatible
- Iterative — compatibility improves over time

### Negative

- Some edge cases may remain documented divergences from public behavior
- Compatibility matrix maintenance is ongoing work

### Risks

- User expects current full compatibility before the evidence exists
- Mitigation: explicit target/baseline distinction, matrix evidence, and
  compatibility-debt tracking

## References

- `docs/legal/CLEAN_ROOM_POLICY.md`
- `docs/adr/0016-full-quarkdown-compatibility-and-upstream-evolution.md`
- `docs/compatibility/quarkdown/`
