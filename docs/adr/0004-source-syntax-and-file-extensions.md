# ADR-0004: Source Syntax and File Extensions

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1

## Context

Scribium must define which input formats and file extensions it accepts,
and how it distinguishes Scribium-native source from passthrough Typst.

## Decision Drivers

- Distinguish Scribium source from plain Markdown and Typst
- Support `.qd` as the primary extension for Quarkdown-compatible documents
- Allow `.md` for Markdown-only documents (bypassing Scribium directives)
- Allow `.typ` passthrough (no Scribium processing)

## Considered Options

### Option 1: Single `.scrib` extension

Clean but breaks expectations — users already have `.md` and `.qd` files.

### Option 2: Multi-extension support with auto-detection (chosen)

- `.qd` — Quarkdown-compatible Scribium source (primary)
- `.scrib` — alias for `.qd`
- `.md` — Markdown-only (no Scribium directives; or directives are an opt-in
  front-matter flag)
- `.typ` — passthrough to Typst compiler

### Option 3: Only `.scrib`, convert external formats

Too much friction for adoption.

## Decision

Support three input paths:

| Extension | Processing                                      |
|-----------|--------------------------------------------------|
| `.qd`     | Scribium parser: Markdown + Quarkdown-compatible |
| `.scrib`  | Alias for `.qd`                                  |
| `.md`     | Markdown-only (no `@` directive processing)      |
| `.typ`    | Passthrough to Typst compiler                    |

## Consequences

### Positive

- `.qd` files are unambiguous Scribium source
- Existing Markdown files work without changes
- `.typ` passthrough enables mixed-mode projects

### Negative

- `.md` vs `.qd` distinction may confuse users
- Detection must be explicit (file extension), not heuristic

### Risks

- Low — this is easily changed before v0.1 if feedback suggests a different split

## References

- `docs/SYNTAX.md`
- CLI implementation: `crates/scribium-cli/`