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

## Quarkdown-Compatible Directives

Directives use the `@` prefix to distinguish Scribium constructs from
plain Markdown. This is the primary mechanism for programmable documents.

### Function Call (Planned)

```
@function-name
@function-name[pos1, pos2, named: value]
@function-name(pos1)[body content]
@function-name(named: value)[
  block body
  with multiple lines
]
```

- `@` prefix introduces the call
- Arguments can be positional (`pos1, pos2`) or named (`name: value`)
- Body argument in `[...]` as the last (or only) argument
- Empty parentheses `()` or empty brackets `[]` are valid
- No space between `@name` and `(` or `[`

### Variable Reference (Planned)

```
@variable-name        // evaluates to variable value
@(expression)         // parenthesized expression
```

### Conditional (Planned)

```
@if(condition)[true branch]
@if(condition)[true branch][false branch]
```

### Iteration (Planned)

```
@for(item in list)[
  @item
]
```

### Variable Binding (Planned)

```
@let name = value
@let name = expression
```

### Include / Read (Planned)

```
@include("path/to/file.qd")
@read("path/to/data.yaml")
```

### Typst Escape (Planned)

````markdown
```typst
#figure(table(...))
```
````

### Data Loading (Planned)

```
@let data = read("data.yaml")
@for(item in data.items)[...]
```

## Reserved Syntax

The following prefixes are reserved for future Scribium syntax:

- `@` — directives and function calls
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