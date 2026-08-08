# ADR-0010: Versioning and Compatibility Policy

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1

## Context

Scribium must define versioning for the tool itself, its configuration schema,
diagnostic codes, and compatibility profiles.

## Decision Drivers

- SemVer for the Scribium tool
- Configuration schema version independent of tool version
- Diagnostic codes are stable within a major version
- Compatibility profiles have their own versioning

## Considered Options

### Option 1: Single version for everything (rejected)

Bumps to CLI args would force major version even if core semantics unchanged.
Too rigid.

### Option 2: Independent version axes (chosen)

Tool SemVer, config schema semver, diagnostic codes per major, compatibility
profile per target.

## Decision

- **Tool:** SemVer (`v0.1.0`, `v0.2.0`, `v1.0.0`, etc.)
- **Config schema:** Integer version in scribium.toml (`config-version = 1`)
- **Diagnostic codes:** Stable within a major version. Never reassign codes.
  Deprecated codes do not reappear.
- **Compatibility profile:** Named after target version (`quarkdown-v2.5`)
- **Syntax version:** Tied to tool version in pre-1.0; stabilized at 1.0

## Pre-1.0 policy

- Breaking changes increment minor version
- Breaking changes documented in CHANGELOG
- Diagnostic codes may be added but not reassigned
- Config schema version changes only with breaking config changes

## Consequences

### Positive

- Each version axis evolves independently
- Diagnostic codes are reliable for tooling
- Config schema changes are explicit

### Negative

- Multiple version numbers to track
- Compatibility profile maintenance is ongoing

### Risks

- Pre-1.0 minor bumps may be frequent
- Mitigation: batch breaking changes into fewer releases

## References

- `docs/RELEASING.md`