# Quarkdown Compatibility Specification

## Status

- **Specification version:** 0.2 (function-call syntax)
- **Target Quarkdown version:** 0.9.x
- **Compatibility level:** In progress — implemented features are listed below

## Scope

This document defines Scribium's Quarkdown-compatible syntax and semantics.
Each feature records its specification source, compatibility level, and known
divergences.

The Quarkdown function-call grammar is implemented clean-room from the public
documentation, notably *"Syntax of a function call"* on the Quarkdown wiki.
No Quarkdown source code is copied or translated. See `SPEC_SOURCES.md` for
provenance records.

## Feature Matrix

| Feature                     | Syntax                 | Compatibility | Status      |
|-----------------------------|------------------------|---------------|-------------|
| Dot-prefixed call           | `.note`                | Parsed        | Implemented |
| Implicit positional refs   | `.1`, `.2`, ...        | Parsed        | Implemented |
| Positional arguments        | `.range {1} {10}`      | Parsed        | Implemented |
| Named arguments             | `.panel width:{320}`   | Parsed        | Implemented |
| Mixed positional/named      | `.panel {Intro} width:{320}` | Parsed | Implemented |
| Indented body argument      | `.panel {x}` + indent  | Parsed        | Implemented |
| Nested calls                | `.outer {.inner {x}}`  | Parsed        | Implemented |
| Inline (mid-paragraph) call | `see .note {x}`        | Parsed        | Implemented |
| Tight-call boundaries       | word adjacency rejected | Parsed       | Implemented |
| Variables                   | —                      | Parsed        | Planned      |
| Conditionals                | —                      | Parsed        | Planned      |
| Iteration                   | —                      | Parsed        | Planned      |
| Functions/components        | —                      | Parsed        | Planned      |
| Include/read                | —                      | Parsed        | Planned      |
| Metadata                    | —                      | Parsed        | Planned      |
| Row/column/grid             | —                      | Parsed        | Planned      |
| Call chaining (`::`)        | —                      | Unsupported   | Planned      |
| Line continuation (`\`)     | —                      | Unsupported   | Planned      |
| Multi-line arguments        | —                      | Unsupported   | Planned      |
| Semantic evaluation         | —                      | Unsupported   | Planned      |

Planned status means the syntax is not implemented yet, in whole or in part.
Implemented rows are covered by tests and goldens.

## Compatibility Levels

- **Unsupported:** Produces explicit `E8xxx` diagnostic
- **Parsed:** Accepted syntactically; behavior may be undefined or rejected
- **Semantically supported:** Scribium semantics match documented behavior
- **Output-equivalent:** Typst output matches reference for tested inputs
- **Known divergence:** Deliberate behavioral difference with documented rationale

Function calls are currently **Parsed**: `.name`, positional arguments
`{arg}`, named arguments `name:{arg}`, nested calls, and indented block
bodies are parsed into the Scribium AST/IR. Semantic evaluation is the next
milestone (see `docs/SYNTAX.md` and `docs/ROADMAP.md`).

### Tight-call boundaries

A call requires a boundary before and after it: whitespace, a symbol
(including `-`), or the start/end of the line. A call directly adjacent to a
word character — any Unicode letter or digit, plus `_` — is not recognized and
the whole construct stays ordinary text. Examples:

- `.note {x}` is a call; `.note {x}B` and `한.note {x}` are not (both
  Unicode and ASCII letters count as word characters).
- `-.note` and `.note-` are valid calls: `-` is a symbol, not a word
  character.

## Specification Record Format

Each implemented compatibility feature records its public documentation
source, an independently authored input example, and the observed behavior.

```yaml
feature: dot-prefixed-call
specification_source: |
  Quarkdown wiki, "Syntax of a function call":
  https://quarkdown.com/wiki/syntax-of-a-function-call/
independently_authored_input: |
  .heading level:{1}
      Title
  .strong {bold text}
observed_reference_behavior: |
  Dot-prefixed names form function calls; each argument is wrapped in
  curly braces; named arguments use name:{value}; indented lines after
  a block call form its body.
scribium_behavior: |
  Parses dot calls, positional/named arguments, nested calls, and
  indented bodies into the shared DirectiveCall AST.
compatibility_level: Parsed
known_divergence: null
```

The `independently_authored_input` is written from the public syntax
specification only; it is not copied from Quarkdown sources, examples, or
tests (clean-room policy, see `docs/adr/0007-quarkdown-compatibility-scope-and-clean-room-process.md`).

## Provenance

The call grammar was derived from the public documentation *"Syntax of a
function call"* (quarkdown.com wiki; accessed 2026-08-08). The basic
dot-and-braces grammar is documented across the release history; it is
valid for the 0.9.x target and has not changed as of the 2.5.x series.
Line continuation and `::`-chaining are newer additions and stay outside
the current scope. `SPEC_SOURCES.md` documents the source list.

## Known Divergences

- (None yet for the implemented call-syntax subset)
- Scope note: only the features listed in the matrix above are implemented.
  Rows marked **Planned** (Variables, Conditionals, Iteration, ... and the
  **Unsupported** rows below) are *not* implemented; public-syntax elements
  outside this matrix must not be assumed to work in Scribium.

## Unsupported Features

Features intentionally not supported for this target (produce an `E8xxx`
diagnostic where applicable):

- Quarkdown interactive slide runtime
- Quarkdown internal plugin ABI
- Quarkdown-specific CSS themes
- Quarkdown HTML post-processing
- Call chaining (`::`) and line continuation (`\`) — planned, not yet
  implemented