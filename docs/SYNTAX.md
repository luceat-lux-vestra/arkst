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

Planned (M2+):
- Ordered lists, task lists
- Links (`[text](url)`)
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

### Variable Reference (Planned)

```
.variable-name {default}        // planned: evaluates to variable value
```

Not yet implemented. Variable semantics will build on the function-call
parsing above.

### Conditional (Planned)

```
.if {condition}
    true branch
```

Conditional evaluation is planned; parsing of the call syntax above is
implemented, semantic evaluation is not.

### Iteration (Planned)

Planned. Iteration will build on the indented body syntax.

### Variable Binding (Planned)

Planned.

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