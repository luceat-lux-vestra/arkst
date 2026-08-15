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
- Code spans (`` `code` ``)
  - A code span opens with a run of one or more backticks and closes with a
    backtick run of exactly the same length (``foo` bar`` stays inside a
    double-backtick span); runs of other lengths do not close the span
  - Contents are opaque: no Markdown or Quarkdown syntax inside a code span
    is parsed (`**bold**`, `[link](url)`, and `.strong {x}` stay literal)
  - Line endings inside a code span become ordinary spaces
  - Per CommonMark: if the content starts and ends with an ASCII space and
    is not composed entirely of spaces, exactly one leading and one trailing
    space is removed (`  code  ` keeps one space each side)
  - An opener with no matching closer recovers deterministically as literal
    text with no character loss and no diagnostic

- End-to-end Markdown structures (M2, tested subset)
  - Blockquotes (`> `) preserve recursively structured paragraphs, lists, and
    inline markup through IR and Typst lowering
  - Strikethrough (`~text~` and `~~text~~`, as accepted by the pinned
    Rushdown substrate) preserves nested inline markup and lowers as a Typst
    strike element
  - GFM task lists preserve unchecked/checked state as semantic IR state and
    lower to deterministic unchecked/checked markers
  - GFM tables preserve header/body rows, cell order, inline markup, and
    left/center/right/default alignment through IR and Typst lowering
  - Source spans remain byte-based and source-backed for UTF-8 and CRLF inputs
  - Evidence covers `.md`, `.qd`, and Markdown in an indented Quarkdown body
  - This is a tested M2 slice, not a claim of complete CommonMark/GFM support

Planned (M2+):

- Images (`![alt](url)`)
- Footnotes
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
- Braced arguments may span physical lines, including nested braces. Their
  indentation is preserved as source content and is not a fixed-width syntax
  rule.
- A trailing backslash immediately before a line ending continues the
  argument list. The continuation marker and line ending are syntax, not
  argument content; leading spaces or tabs on the continuation line are
  ignored for argument recognition.
- `::` parses and structurally preserves a call chain (`.a {x}::b {y}`),
  including each segment and argument source span. The evaluator executes
  supported chain segments directly in strict left-to-right order: the prior
  semantic value is injected as the next segment's first positional argument,
  while explicit positional and named arguments retain their order and names.
  For the evidenced surface, `.a::b` and its documented nested equivalent
  `.b {.a}` use the same value-context invocation path and therefore produce
  equivalent semantic values and observable output. The current semantic
  evidence set is `.sum`, `.multiply`, `.uppercase`, and `.lowercase`; an
  unknown or otherwise unexecutable chain segment reports a source-backed
  `E3001` evaluation diagnostic and does not fabricate a value.
  The parser's structural representation is consumed directly; no synthetic
  source or Markdown/Typst round trip is used.
- A complete call may be wrapped in braces to lift word-adjacency boundaries,
  for example `H{.text {2}}O`. The wrapper is consumed by the Quarkdown
  frontend and its source span remains available.
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
- A multiline braced argument or a continued argument list is completed
  before body parsing begins. For example, the lines inside
  `.call { ... }` are argument content, while `.call` followed by an
  indented line is a body argument.
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

## User-defined functions and lambda parameters (Implemented slice)

Scribium evaluates the documented `.function` declaration form for
headerless implicit-parameter and explicit-parameter functions. A declaration
is source-order state and produces no document output:

```
.function {hello}
    Hello, world!

.function {greet}
    to from?:
    Hello, .to from .from::otherwise {unnamed}!

.hello
.greet {world}
.greet {world} from:{John}
```

The first body line of `.function` is contextually parsed by the Quarkdown
grammar as a structured lambda header only when it ends in `:`. Ordinary call
bodies keep their normal Markdown interpretation. Parameter names and the
optional marker retain original source spans through the frontend and IR. A
headerless callable uses implicit positional parameters; the parser preserves
`.1`, `.2`, and later references as call nodes so the evaluator can resolve
them without source rewriting.

Supported invocation semantics are positional and named binding, a block body
bound to the final parameter, parent-visible/child-local scope, source-order
redeclaration, and user-defined bindings taking precedence over an evidenced
builtin after declaration. Outputless body statements update the child scope;
one substantive semantic value remains typed across the function boundary,
while multiple rich or Markdown outputs become structured content only when
composition requires it. Nested and chained calls use the same evaluator value
path. An omitted `parameter?` binds the semantic value `None`; it is not an
outputless evaluator result. At an output boundary it materializes as the text
`None`.

Optional values can use the evidenced builtins below:

```
.from::otherwise {unnamed}
.value::isnone
```

`.otherwise` returns its original value when it is not `None`, otherwise it
returns its fallback value. Both branches retain their semantic type until
the surrounding output context materializes them. `.isnone` returns a semantic
boolean. A `None` value is distinct from an outputless `NoValue` result: the
latter remains an evaluator control outcome and is still an error when a
nested value-required context needs a value.

Implicit lambda parameters are 1-based and invocation-local. `.1` is the
first positional argument, `.2` the second, and so on; `.0`, leading-zero
spellings, and word-adjacent forms are not implicit references. An explicit
header is an explicit binding mode and does not synthesize `.1` aliases. A
missing implicit argument produces a deterministic source-backed `E3003`
diagnostic rather than `None`, `NoValue`, or a panic. The callable body keeps
the same semantic accumulator as explicit functions, so numbers, booleans,
strings, `None`, and structured content remain typed until an output boundary.

Generic standalone lambdas, iteration, and components remain outside this
slice. A
rich block result that cannot be represented in an inline context is rejected
with a source-backed diagnostic rather than flattened or dropped.

### Evaluation scope (Implemented)

The evaluator now has explicit parent/child scope APIs with deterministic
lookup, local variable bindings, and source-backed local function bindings.
Child scopes inherit visible parent bindings and local writes do not leak back
to the parent. Existing `.var` declarations continue to use the document-level
scope and are evaluated in source order. The evaluator represents callable
parameters as either explicit source-backed bindings or an implicit positional
binding mode. Each invocation installs its own lambda-local argument scope;
nested invocations therefore shadow only while active and restore the outer
implicit arguments afterward. Standalone lambda syntax, `.let`, `.foreach`,
and `.repeat` remain subsequent semantic slices; this slice does not claim
those user-facing features are implemented.

Function arguments and chain intermediates are evaluated in value context,
which preserves scalar values and evaluated content until a final document
output context materializes them as nodes or inline text. Conditional bodies
remain lazy until the callee selects a branch. The current case-transform
builtins use a deliberately small invocation-boundary adaptation contract for
strings, identifiers, booleans, numbers, and plain text content; this is not a
claim of complete Quarkdown `DynamicValue` or standard-library compatibility.

For a user-defined call, positional and named arguments are evaluated in
source order before the callee body can run. A successful argument set creates
a child scope, binds parameters, and then evaluates the body. Any argument
failure prevents body execution.

Evaluator outcomes distinguish a successful value, a successful outputless
side effect, a failed evaluation, and an unresolved call. A terminal
outputless call such as variable declaration or reassignment is legal and
produces no document nodes. The same outputless result is an `E3001` when a
nested argument or non-final chain segment requires a value; failures
propagate their original diagnostic without an additional generic no-value
error. Unresolved ordinary calls remain preservable, while unresolved chain
segments report source-backed `E3001` because a chain cannot fabricate an
intermediate value.

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

### Native Typst passthrough

Native `.typ` passthrough, if implemented, is a host-level input capability
that sends a `.typ` document to the selected official Typst compiler. Scribium
does not embed raw Typst source in backend-neutral IR and does not define a
generic backend escape block. The current CLI rejects `.typ` input until the
separate passthrough capability is implemented.

### Data Loading (Planned)

Planned.

## Reserved Syntax

The following prefixes are reserved for future Scribium syntax:

- `.` — Quarkdown-compatible function calls (dot-prefixed)
- `$` — math (delegated to Typst or pass-through)
- `#` — Typst syntax is generated by the Typst lowering boundary; it is not a
  Scribium raw-backend escape syntax
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
