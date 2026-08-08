# Quarkdown Compatibility Specification

## Status

- **Specification version:** 0.3 (reference baseline v2.5.0)
- **Reference upstream:** Quarkdown v2.5.0
- **Compatibility model:** documented feature subset
- **Full Quarkdown compatibility:** not claimed

## Scope

This document defines Scribium's Quarkdown-compatible syntax and semantics.
Each feature records its specification source, compatibility level, and known
divergences.

Scribium does **not** claim full Quarkdown v2.5.0 compatibility. The
compatibility contract is the **documented subset** defined by this document:
only the **Implemented** rows in the Feature Matrix below — at the stated
compatibility level and backed by conformance evidence recorded in
`SPEC_SOURCES.md` — are part of the contract. The Feature Matrix also lists
`Planned` and `Not implemented` rows; those are tracking entries and
constitute **no** compatibility claim. Features that Quarkdown v2.5.0
documents but Scribium does not implement are not bugs; they are outside the
contract until implemented and recorded in this matrix (see ADR 0012).

The Quarkdown function-call grammar is implemented clean-room from the public
documentation, notably *"Syntax of a function call"* on the Quarkdown wiki.
No Quarkdown source code is copied or translated. See `SPEC_SOURCES.md` for
provenance records.

## Feature Matrix

| Feature                        | Syntax                           | Compatibility            | Status           |
|--------------------------------|----------------------------------|--------------------------|------------------|
| Dot-prefixed call              | `.note`                          | Parsed                   | Implemented      |
| Implicit positional refs       | `.1`, `.2`, ...                  | Parsed                   | Implemented      |
| Positional arguments           | `.range {1} {10}`                | Parsed                   | Implemented      |
| Named arguments                | `.panel width:{320}`             | Parsed                   | Implemented      |
| Mixed positional/named         | `.panel {Intro} width:{320}`     | Parsed                   | Implemented      |
| Indented body argument         | `.panel {x}` + indent            | Parsed                   | Implemented      |
| Nested calls                   | `.outer {.inner {x}}`            | Parsed                   | Implemented      |
| Inline (mid-paragraph) call    | `see .note {x}`                  | Parsed                   | Implemented      |
| Tight-call boundaries          | word adjacency rejected          | Parsed                   | Implemented      |
| Malformed-call diagnostics     | `E2001`, `E2002`, `E2003`        | Error                    | Implemented      |
| Variables                      | —                                | —                        | Planned          |
| Conditionals                   | —                                | —                        | Planned          |
| Iteration                      | —                                | —                        | Planned          |
| Functions/components            | —                                | —                        | Planned          |
| Include/read                   | —                                | —                        | Planned          |
| Metadata                       | —                                | —                        | Planned          |
| Row/column/grid                | —                                | —                        | Planned          |
| Semantic evaluation            | —                                | —                        | Planned          |
| Call chaining (`::`)           | `.a {x}::b {y}`                  | Not implemented          | Planned          |
| Line continuation (`\`)        | `\` at end of line               | Not implemented          | Planned          |
| Tight / brace-wrapped calls    | `.x` wrapped in braces at adjacency | Not implemented       | Planned          |
| Multi-line arguments           | `{.…}` parsing spans lines        | Not implemented (E2xxx today) | Planned          |
| `.json` data loading           | `.json {path}` (new in v2.5.0)   | Not implemented          | Planned          |
| `.markdown` / `.llmstxt`       | (new in v2.5.0)                  | Not implemented          | Planned          |

`Implemented` rows are covered by unit and golden tests. `Planned` means the
syntax is not implemented yet, in whole or in part: it has no documented
`Unsupported` diagnostic and must not be assumed to work.

## Compatibility Levels

- **Unsupported:** Produces explicit `E8xxx` diagnostic (used only by the
  compatibility-profile diagnostics; see `compatibility/diagnostics.rs`)
- **Error:** Produces an explicit parse diagnostic (`E2xxx`) at the call site
- **Parsed:** Accepted syntactically; behavior may be undefined or rejected
- **Semantically supported:** Scribium semantics match documented behavior
- **Output-equivalent:** Typst output matches reference for tested inputs
- **Known divergence:** Deliberate behavioral difference with documented
  rationale

Function calls are currently **Parsed**: `.name`, positional arguments
`{arg}`, named arguments `name:{arg}`, nested calls, and indented block
bodies are parsed into the Scribium AST/IR. Semantic evaluation is the next
milestone (see `docs/SYNTAX.md` and `docs/ROADMAP.md`). Note that a feature
which currently fails to parse (e.g. `E2xxx` syntax errors on some input
forms) is still labeled by its documented support level in the matrix — an
input-level parse error is not an `Unsupported` marker.

### Tight-call boundaries

A call requires a boundary before and after it: whitespace, a symbol
(including `-`), or the start/end of the line. A call directly adjacent to a
word character — any Unicode letter or digit, plus `_` — is not recognized and
the whole construct stays ordinary text. Examples:

- `.note {x}` is a call; `.note {x}B` and `한.note {x}` are not (both
  Unicode and ASCII letters count as word characters).
- `-.note` and `.note-` are valid calls: `-` is a symbol, not a word
  character.

The new-in-Quarkdown brace-wrapped form (`H{.text {2}}O`), which lifts the
boundary requirement, is a documented v2.5.0 behavior but is **not
implemented** here; the inner call parses, but the wrapping braces are kept
as literal text.

### v2.5.0 additions outside the contract

Quarkdown has documented features that are part of the v2.5.0 baseline but are
outside Scribium's current contract. These are listed in the Feature Matrix as
`Planned` and are **not** claimed as compatible. They do not produce `E8xxx`
diagnostics today and their current behavior is undefined for the purposes of
this contract; examples: line continuation (`\` at EOL), `::` chaining, tight
brace-wrapped calls, multi-line arguments spanning raw lines, and the new
v2.5.0 builtins (data loading, `.markdown`).

## Specification Record Format

Each implemented compatibility feature records its public documentation
source, an independently authored input example, and the observed behavior.

```yaml
feature: dot-prefixed-call
specification_source: |
  Quarkdown wiki, "Syntax of a function call":
  https://quarkdown.com/wiki/syntax-of-a-function-call/ (v2.5.0 badge)
independently_authored_input: |
  .heading level:{1}
      Title
  .strong {bold text}
observed_reference_behavior: |
  Dot-prefixed names form function calls; each argument is wrapped in
  curly braces; named arguments use name:{value}; indented lines after
  a block call form its body. The current v2.5.0 documentation describes the
  same basic dot-prefixed, brace-argument model on which Scribium's existing
  parser subset is based.
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
function call"* (wiki, badged `2.5.0`, accessed 2026-08-08). The current
v2.5.0 documentation describes the same basic dot-prefixed, brace-argument
model on which Scribium's parser subset is based. Scribium's previous
compatibility baseline was 0.9.x, but no claim is made that the upstream
grammar was verified to be identical across every version in between.
`SPEC_SOURCES.md` documents the source list, per-source version badges, and
accessed dates.

## Known Divergences

- (None yet for the implemented call-syntax subset)
- Scope note: only the features listed in the matrix above are implemented
  and claimed. Rows marked **Planned** are *not* implemented; anything
  documented in Quarkdown but absent from this matrix must not be assumed to
  work in Scribium.

## Features Outside the Contract

The following are not part of the documented subset and are not claimed:

- Quarkdown interactive slide runtime
- Quarkdown internal plugin ABI
- Quarkdown-specific CSS themes
- Quarkdown HTML post-processing
- Quarkdown line click interactivity

New v2.5.0 builtins (data loading via `.json`, `.markdown`, `.llmstxt`,
stdlib `foreach`/iterables) are tracked as `Planned` above; they do not belong
here because none produces an `E8xxx` diagnostic today. As features are
implemented, they move from this section into the Feature Matrix.