# Syntax — Scribium

> This document is a specification skeleton. Features not yet implemented are
> marked `Planned`. See `docs/compatibility/quarkdown/` for Quarkdown-specific
> syntax notes.

## Lexical Conventions

- Source encoding: UTF-8
- Line endings: LF normalized (CRLF accepted, normalized to LF)
- Indentation: not semantically significant except in fenced code / verbatim
- Whitespace: spaces and tabs; no NBSP or zero-width chars in identifiers
- Comments: `// line comment` (Planned) or within Markdown HTML comment syntax

## Markdown Baseline (Partial)

Scribium targets a CommonMark/GFM-compatible subset. The M1 parser implements
the subset below; the exact baseline will be determined by parser spike
results (ADR 0006).

Implemented (M1):

- ATX headings (`# ` through `###### `), trailing `#` closures stripped
- Paragraphs (contiguous non-blank lines) with soft/hard line breaks
- Emphasis (`*italic*` or `_italic_`)
- Strong (`**bold**` or `__bold__`)
- Unordered lists (`- `, `* `, `+ `) with nested lists and indented items
- Fenced code blocks (triple backtick with optional language)
- Horizontal rules (`---`, `***`, `___`; three or more identical markers)
- Hard line break (trailing two spaces + newline, or backslash + newline)

Known M1 divergences (deterministic, documented in the parser module):

- Delimiter runs of 3+ identical characters (`***x***`) are literal text
- Setext headings are not parsed (`text` + `---` becomes a paragraph
  followed by a horizontal rule)
- Indentation inside code blocks nested in list items is normalized
- Blank lines produce no AST nodes (round-trip support is deferred)

Implemented (M2):

- Ordered lists (`1. `, `2. `, etc.) with nested lists and indented items
  - Starting ordinal preserved when source begins at a value other than 1
  - Only the first item's ordinal sets the start: `3. A` followed by `9. B`
    is one list starting at `3` (later ordinals do not renumber the list)
  - Parentheses marker (`1) `, `2) `) also supported; a list keeps one
    delimiter, so `1. A` followed by `2) B` is two lists
  - Ordered/unordered nesting in either direction
  - Continuation/nested content indentation is derived per item from its own
    marker width (e.g. `9. ` vs `10. `), not a fixed column
  - Markers allow 1 to 9 digits; longer digit runs (`1234567890. `) are
    literal text
- Links (`[text](url)`)
  - The label runs from `[` to the first `]` and keeps full inline markup:
    emphasis, strong text, and Quarkdown inline calls work inside it
    (`[**bold** text](https://example.com)`, `[.strong {hello}](...)`)
  - The destination runs from `(` to the first matching `)`; balanced
    parentheses inside are allowed (`[x](a(b)c)`)
  - Destinations are preserved as-is: `https://` URLs, relative paths,
    and fragments (`#intro`) are passed through without normalization or
    resolution; `\` and `"` are escaped in generated Typst
  - A destination must be non-empty and free of whitespace and control
    characters; an empty or whitespace-only destination (`[]()`,
    `[text]( )`) is not a link
  - Not supported: nested brackets in the label (the label ends at the
    first `]`), link titles (`[text](url "title")`), reference links
    (`[text][id]` / `[id]: url`), autolinks (`<https://...>`), and images
  - Malformed links (`[text](`, `[text]`, `[](url)`, `[text]( )`,
    `[text](url "title")`, ...) recover as literal text

Planned (M2+):

- Task lists
- Images (`![alt](url)`)
- Blockquotes (`> `)
- Code spans (`` `code` ``)
- Tables (GFM pipe tables)
- Footnotes
- Strikethrough (`~~text~~`)
- Math (`$...$` and `$$...$$`)
- HTML passthrough (policy TBD)

## Quarkdown-Compatible Function Calls

Function calls use Quarkdown's dot-prefixed syntax. A call is introduced by
a `.` followed by a function name:

```
.function
.function {arg}
.function {arg1} {arg2}
.function option:{value}
.function {arg} option:{value}
```

### Function-name grammar

```text
normal-call-name:
    [A-Za-z_][A-Za-z0-9_-]*

implicit-positional-reference:
    .[1-9][0-9]*
```

`normal-call-name` may be followed by `{arg}` / `name:{value}` arguments;
`implicit-positional-reference` is a bare token and does not consume
arguments. An implicit reference followed by a word character (any Unicode
letter or digit, or `_`) is not recognized as a reference at all.

Call syntax has the following properties:

- The function name follows the dot directly: alphanumeric, `_` and `-` are
  allowed, the first character must be a letter or `_`.
- **Implicit positional references** (`.1`, `.2`, `.12`, ...) are a separate
  grammar case from normal function names: digits only, no leading `0`, and
  a following word character keeps the whole token ordinary text
  (`.1abc` is not a reference; `.1-1` is a `.1` reference followed by `-1`).
  They are bare reference tokens — unlike normal calls they never take
  arguments (`.1 {item}` does not form a call).
- A call requires a **boundary** on both sides: whitespace, a symbol
  (including `-`), or the start/end of a line. A word character (any Unicode
  letter or digit, or `_`) directly before or after the call makes the whole
  construct ordinary text: `.note {x}suffix` and `word.note {x}` are not
  calls, while `-.note {x}`, `.note {x}-` and `See .note {x} items` are.
- Positional arguments are wrapped in curly braces: `{...}`.
- Named arguments are `name:{...}`.
- Positional and named arguments may be mixed, but every argument after a
  named argument must also be named.
- An argument may contain a plain value (`{320}`, `{center}`, `{"text"}`) or
  arbitrary content, including **nested calls**: `.outer {.inner {value}}`.
- Inline calls appear inside a paragraph: `.strong {bold}` in surrounding
  text. A call that has trailing text after it on the same line is treated
  as an inline call, not a block-level call.

### Block-level calls with indented body (Implemented)

A call that stands alone on its line (with only whitespace after it) is a
block-level call. Its body is the indented content that follows:

```
.panel {Introduction}
    This is the panel body.
    It may contain **Markdown** content.
```

- The body starts at the next non-blank line indented by at least 2 spaces
  or one tab.
- All body lines share the same indentation; deeper indentation is allowed
  inside for nested calls.
- The body ends at the first line with less indentation.
- Markdown parsing continues inside the body, including nested block calls:

```
.panel {Outer}
    Hello

    .note {Nested}
        Nested body
```

### Variable Reference (Implemented)

Variable references use the same parameterless call syntax as function calls.
A variable must be declared with `.var` before it can be referenced.

```
.var {name} {value}         // declaration (no output)
.name                       // reference (evaluates to variable value)
.name {new-value}           // reassignment (only if `name` is a variable)
```

- Variable names follow `normal-call-name` grammar: `[A-Za-z_][A-Za-z0-9_-]*`
- Declarations accept scalar values, boolean identifiers, rich/content values (e.g., `**bold**`), or indented block content
- References in conditionals (`.if {.name}`) resolve to the variable's boolean value
- Unknown parameterless calls are preserved as function calls, not variable errors

### Variable Binding (Implemented)

Variables are document-scoped and evaluated in source order.

```
.var {language} {Rust}
Language: .language
```

Output:
```
Language: Rust
```

Reassignment:
```
.var {name} {A}
.name {B}
.name
```

Output:
```
B
```

Block variables:
```
.var {section}
    # Title
    body
.section
```

Conditional integration:
```
.var {enabled} {yes}
.if {.enabled}
    visible
```

Boolean identifiers: `true` / `false` / `yes` / `no` (case-insensitive).

Malformed `.var` declarations (missing name or value) produce `E3002`.
Invalid variable names (not matching `normal-call-name` grammar) produce `E3002`.

> **Note on block variable evaluation timing:** Scribium currently evaluates block variable content at declaration time (source order). The cited Quarkdown public documentation does not explicitly specify evaluation timing for stored block content. This behavior may be refined if upstream semantics are clarified.

### Conditional (Implemented)

```
.if {condition}
    true branch

.ifnot {condition}
    false branch

.if condition:{condition}
    true branch

.ifnot condition:{condition}
    false branch

.if {condition} body:{content}

.if condition:{condition} body:{content}
```

Conditionals evaluate the `condition` argument as a boolean condition. The
argument can be provided as the first positional argument or as a named
argument `condition`:

- Boolean literals: `true` / `false`
- Boolean identifiers (case-insensitive): `yes` / `no`
- Missing or unresolvable conditions are reported as `E3001` (evaluation
  error) and the conditional is treated as `false` for deterministic
  output.

The content is, in order of priority: the indented block body, the named
`body` argument, the second positional argument (a content value or bare
scalar), or nothing.

`.ifnot` inverts the condition: its content is rendered when the
condition is false.

Nested conditionals are supported. Variable references (`.name`) in conditions resolve to the variable's boolean value. Function-call conditions (e.g., `.if {.func {x}}`) are not supported and produce an `E3001` diagnostic.

### Iteration (Planned)

Planned. Iteration will build on the indented body syntax.

### Include / Read (Planned)

Planned. `include` and `read` are function calls like any other and will be
evaluated by the builtin layer.

### Typst Escape (Planned)

Planned.

### Data Loading (Planned)

Planned.

## Reserved Syntax

The following prefixes are reserved for future Scribium syntax:

- `.` — Quarkdown-compatible function calls (dot-prefixed)
- `$` — math (delegated to Typst or pass-through)
- `#` — raw Typst (pass-through in Typst escape blocks)
- Front matter delimiter `---`

## Front Matter (Implemented)

A `---`-delimited block at the start of a document carries metadata
(`title`, `author`, `date`, and custom keys). It is a flat, line-based
`key: value` format — **not full YAML**:

- Keys and values are split on the first colon (`key: rest of line`).
- Nested objects, arrays, and block strings are not supported.
- The opening delimiter must be `---` at column 0; every non-empty metadata
  line must also start at column 0. Indented metadata lines (nested structure)
  reject the whole block, which is preserved intact as regular Markdown.
- A line without a colon, an empty key, or an indented `---` delimiter
  rejects the whole block (it is treated as regular Markdown).
- Duplicate keys use last-wins semantics.
- Custom metadata is stored in the IR in deterministic (lexicographic
  key) order.

Example:

```markdown
---
title: My Document
author: Alice
date: 2026-08-06
custom: value
---

# Heading
```

Full YAML support is a separate, future milestone.

## Versioning

- Syntax version: tied to Scribium release version
- Breaking syntax changes require a major version bump
- Old syntax may be supported via compatibility profile