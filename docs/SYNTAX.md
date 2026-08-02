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

## Markdown Baseline (Planned)

Scribium targets a CommonMark/GFM-compatible subset. The exact baseline will
be determined by parser spike results (ADR 0006).

Supported (M1+):
- ATX headings (`# ` through `###### `)
- Paragraphs (contiguous non-blank lines)
- Emphasis (`*italic*` or `_italic_`)
- Strong (`**bold**` or `__bold__`)
- Unordered lists (`- `, `* `, `+ `)
- Fenced code blocks (triple backtick with optional language)
- Horizontal rules (`---`, `***`, `___`)
- Hard line break (trailing two spaces + newline)

Planned (M2+):
- Ordered lists, nested lists, task lists
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

## Versioning

- Syntax version: tied to Scribium release version
- Breaking syntax changes require a major version bump
- Old syntax may be supported via compatibility profile