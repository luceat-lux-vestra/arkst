# Quarkdown Compatibility Specification

## Status

- **Specification version:** 0.1 (initial draft)
- **Target Quarkdown version:** 0.9.x
- **Compatibility level:** In progress — individual features are listed below

## Scope

This document defines Scribium's Quarkdown-compatible syntax and semantics.
Each feature records its specification source, compatibility level, and known
divergences.

## Feature Matrix

| Feature               | Compatibility | Status      | Spec Source                      |
|-----------------------|---------------|-------------|----------------------------------|
| Dot-prefixed call     | Parsed        | Planned     | Public syntax documentation      |
| Positional arguments  | Parsed        | Planned     | Public syntax documentation      |
| Named arguments       | Parsed        | Planned     | Public syntax documentation      |
| Body/block argument   | Parsed        | Planned     | Public syntax documentation      |
| Nested calls          | Parsed        | Planned     | Public syntax documentation      |
| Variables             | Parsed        | Planned     | Public syntax documentation      |
| Conditionals          | Parsed        | Planned     | Public syntax documentation      |
| Iteration             | Parsed        | Planned     | Public syntax documentation      |
| Functions/components  | Parsed        | Planned     | Public syntax documentation      |
| Include/read          | Parsed        | Planned     | Public syntax documentation      |
| Metadata              | Parsed        | Planned     | Public syntax documentation      |
| Row/column/grid       | Parsed        | Planned     | Public syntax documentation      |

## Compatibility Levels

- **Unsupported:** Produces explicit `E8xxx` diagnostic
- **Parsed:** Accepted syntactically; behavior may be undefined or rejected
- **Semantically supported:** Scribium semantics match documented behavior
- **Output-equivalent:** Typst output matches reference for tested inputs
- **Known divergence:** Deliberate behavioral difference with documented rationale

## Specification Record Format

Each compatibility feature must record:

```yaml
feature: dot-prefixed-call
specification_source: |
  Public Quarkdown syntax documentation: https://quarkdown.org/docs/syntax
independently_authored_input: |
  @heading(level: 1)[Title]
  @strong[bold text]
observed_reference_behavior: |
  Parses @heading and @strong as function calls
scribium_behavior: |
  Parses @-prefixed identifiers as function calls
compatibility_level: parsed
known_divergence: null
```

## Known Divergences (Initial)

- (None yet — implementation not started)

## Unsupported Features

Features intentionally not supported (produce `E8xxx` diagnostic):

- Quarkdown interactive slide runtime
- Quarkdown internal plugin ABI
- Quarkdown-specific CSS themes
- Quarkdown HTML post-processing