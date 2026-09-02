# ADR-0004: Source Syntax and File Extensions

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Arkst maintainers
- **Related issues:** #1

## Context

Arkst must define which input formats and file extensions it accepts,
and how it distinguishes Arkst-native source from passthrough Typst.

## Decision Drivers

- Distinguish Arkst source from plain Markdown and Typst
- Support `.qd` as the primary extension for Quarkdown-compatible documents
- Allow `.md` for Markdown-only documents (bypassing Arkst directives)
- Allow `.typ` passthrough (no Arkst processing)

## Considered Options

### Option 1: Single `.arkst` extension

Clean but breaks expectations — users already have `.md` and `.qd` files.

> Historical note: before the Arkst rename, the working-name-specific `.scrib`
> alias occupied this role. It was removed before the first public release and
> is not part of the Arkst compatibility contract.

### Option 2: Multi-extension support with auto-detection (chosen)

- `.qd` — Quarkdown-compatible Arkst source (primary)
- `.arkst` — Arkst-native alias for `.qd`
- `.md` — Markdown-only (no Arkst directives; or directives are an opt-in
  front-matter flag)
- `.typ` — host-level passthrough to the selected official Typst compiler

### Option 3: Only `.arkst`, convert external formats

Too much friction for adoption.

## Decision

Support three input paths:

| Extension | Processing                                      |
|-----------|--------------------------------------------------|
| `.qd`     | Arkst parser: Markdown + Quarkdown-compatible |
| `.arkst`  | Alias for `.qd`                                  |
| `.md`     | Markdown-only (no Arkst directive processing)      |
| `.typ`    | Passthrough to Typst compiler                    |

> **Implementation status:** `.typ` passthrough is not implemented yet. The
> CLI currently accepts `.qd`, `.arkst`, and `.md` and rejects `.typ` inputs
> with a clear "unsupported input format" error. The `.typ` row of this ADR
> applies once passthrough lands. This is a host input path, not a raw Typst
> node or generic backend-code escape hatch in Arkst's backend-neutral IR.

## Consequences

### Positive

- `.qd` files are unambiguous Arkst source
- Existing Markdown files work without changes
- `.typ` passthrough enables mixed-mode projects

### Negative

- `.md` vs `.qd` distinction may confuse users
- Detection must be explicit (file extension), not heuristic

### Risks

- Low — this is easily changed before v0.1 if feedback suggests a different split

## References

- `docs/SYNTAX.md`
- CLI implementation: `crates/arkst-cli/`
