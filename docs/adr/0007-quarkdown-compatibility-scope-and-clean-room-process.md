# ADR-0007: Quarkdown Compatibility Scope and Clean-Room Process

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1, #4
- **Superseded by:** ADR-0016 for the compatibility-target scope only

## Supersession note

ADR-0016 supersedes this ADR's documented-subset product-target decision. The
clean-room process, provenance requirements, and explicit compatibility
evidence requirements remain accepted.

## Context

Scribium implements Quarkdown-compatible syntax without using Quarkdown source
code. The scope and process for this clean-room implementation must be defined.

## Decision Drivers

- Legal: no risk of copyright infringement
- Practical: users should be able to migrate existing Quarkdown documents
- Transparent: compatibility level must be documented per feature

## Considered Options

### Option 1: Full compatibility at all costs (rejected)

Would require reversing non-public behavior. Impossible without source access
and creates legal risk.

### Option 2: Documented subset with clean-room process (chosen)

Implement features based on public documentation only. Each feature records
its specification source and compatibility level. Unsupported features produce
explicit diagnostics.

### Option 3: No Quarkdown compatibility (rejected by product definition)

Scribium identity IS Quarkdown compatibility. Not optional.

## Decision

Adopt document subset approach. Core features (dot-calls, arguments, conditionals,
iteration, variables, components, include) are M1-M2 targets. Each feature
tracks provenance. Unsupported Quarkdown features produce `E8xxx` diagnostics.

## Consequences

### Positive

- Legally clean — no source code review dependency
- Transparent — users see exactly what is and isn't compatible
- Iterative — compatibility improves over time

### Negative

- Some edge cases will diverge from reference implementation
- Compatibility matrix maintenance is ongoing work

### Risks

- User expects "Quarkdown compiler" and finds missing features
- Mitigation: explicit README status table, clear feature tracking

## References

- `docs/legal/CLEAN_ROOM_POLICY.md`
- `docs/compatibility/quarkdown/`
