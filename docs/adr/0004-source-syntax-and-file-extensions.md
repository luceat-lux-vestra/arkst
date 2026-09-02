# ADR-0004: Source Syntax and File Extensions

- **Status:** Accepted, amended 2026-09-02
- **Date:** 2026-08-02
- **Owners:** Arkst maintainers
- **Related issues:** #1, #253

## Context

Arkst must define which input formats and file extensions it accepts,
and how it distinguishes Arkst-native source from passthrough Typst.

## Decision Drivers

- Distinguish Arkst source from plain Markdown and Typst
- Support `.qd` as the primary extension for Quarkdown-compatible documents
- Allow `.md` for Markdown-only documents (bypassing Arkst directives)
- Allow `.typ` passthrough (no Arkst processing)

## Considered Options at Initial Acceptance

The original 2026-08-02 decision was made while the project still used the
working name Scribium. The option names below are preserved as historical
record rather than rewritten to the later Arkst name.

### Option 1: Single `.scrib` extension

Clean but breaks expectations — users already have `.md` and `.qd` files.

### Option 2: Multi-extension support with auto-detection (chosen)

- `.qd` — Quarkdown-compatible source (primary)
- `.scrib` — working-name-specific alias for `.qd`
- `.md` — Markdown-only (no project directives; or directives are an opt-in
  front-matter flag)
- `.typ` — host-level passthrough to the selected official Typst compiler

### Option 3: Only `.scrib`, convert external formats

Too much friction for adoption.

## 2026-09-02 Amendment: Arkst Rename

Before the first public release, the project was renamed from Scribium to
Arkst. Because `.scrib` encoded the retired working name and no public release
had established a compatibility obligation for it, the live alias was replaced
rather than carried forward as legacy syntax.

The current extension contract is therefore:

- `.qd` remains the primary Quarkdown-compatible extension;
- `.arkst` is the Arkst-native alias for `.qd`;
- `.md` remains the Markdown input extension;
- `.scrib` is not a supported CLI input extension; and
- `.typ` passthrough remains planned but is not implemented yet.

## Decision

Support three currently implemented input paths plus the planned Typst
passthrough path:

| Extension | Processing |
|---|---|
| `.qd` | Arkst parser: Markdown + Quarkdown-compatible |
| `.arkst` | Arkst-native alias for `.qd` |
| `.md` | Markdown-only (no Arkst directive processing) |
| `.typ` | Planned passthrough to the Typst compiler |

> **Implementation status:** `.typ` passthrough is not implemented yet. The
> CLI currently accepts `.qd`, `.arkst`, and `.md`, rejects `.scrib`, and
> rejects `.typ` inputs with a clear unsupported-input error. The `.typ` row
> applies once passthrough lands. This is a host input path, not a raw Typst
> node or generic backend-code escape hatch in Arkst's backend-neutral IR.

## Consequences

### Positive

- `.qd` files are unambiguous Quarkdown-compatible Arkst source
- `.arkst` provides a project-native extension without carrying the retired
  working name
- Existing Markdown files work without changes
- Planned `.typ` passthrough enables mixed-mode projects

### Negative

- `.md` vs `.qd` distinction may confuse users
- Pre-v0.1 `.scrib` files must be renamed to `.arkst` or `.qd`
- Detection must be explicit at the user-facing host boundary, not heuristic

### Risks

- Low — the `.scrib` replacement occurs before the first public release, so no
  released compatibility contract is being broken.

## References

- `docs/SYNTAX.md`
- CLI implementation: `crates/arkst-cli/`
