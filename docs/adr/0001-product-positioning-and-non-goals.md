# ADR-0001: Product Positioning and Non-Goals

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1

## Context

Scribium needs a clear product identity that distinguishes it from Typst,
Quarkdown, and Markdown-to-anything converters. Without explicit boundaries,
scope creep threatens the initial milestones.

## Decision Drivers

- Must be independently implementable by a small team
- Must provide real value at v0.1 (not just a Markdown converter)
- Must not duplicate Typst's existing functionality
- Must respect clean-room constraints

## Considered Options

### Option 1: Markdown-to-Typst converter

Narrow, safe, but delivers little value over `pandoc`. No differentiation.

### Option 2: Quarkdown-compatible compiler (chosen)

Independent reimplementation of Quarkdown syntax and semantics + Markdown
baseline + Typst backend. Provides clear value, distinct identity.

### Option 3: Full-featured document system

Includes web editor, live preview, package registry. Too broad for initial team.

## Decision

Scribium is a Quarkdown-compatible compiler and toolchain that independently
reimplements Quarkdown syntax and core execution semantics, connecting them
to the official Typst compiler.

## Consequences

### Positive

- Clear identity: not a Markdown converter, not a Typst clone
- Existing Quarkdown users have a migration path
- Permissive license differentiates from original

### Negative

- Clean-room implementation requires careful provenance tracking
- Compatibility expectations must be managed explicitly

### Risks

- Quarkdown syntax may evolve in incompatible ways
- Name confusion with original project

## Validation Plan

- M1 vertical slice produces `.qd → PDF`
- README positions Scribium accurately
- Compliance with clean-room policy documented

## References

- SCRIBIUM_MASTER_EXECUTION_BRIEF.md