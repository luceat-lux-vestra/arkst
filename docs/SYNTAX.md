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

Remaining M2+ / deferred:

- Footnotes
- Math (`$...$` and `$$...$$`)
- General raw HTML semantics (the bounded policy is documented in
  `docs/compatibility/RAW_HTML_POLICY.md`)

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
call-identifier:
    [A-Za-z][A-Za-z0-9]* | [0-9]+

implicit-positional-reference:
    .[0-9]+
```

The call lexer uses the pinned Quarkdown v2.5.1 identifier alternatives. An
alphabetic identifier consumes only ASCII letters and digits; a numeric
identifier consumes ASCII digits, including `0` and leading zeros. The scanner
returns the prefix accepted by that grammar, so `.1abc` is the numeric call
`.1` followed by the untouched remainder `abc`. Numeric call identifiers use
the same argument grammar as other call identifiers; interpreting `.1`, `.01`,
and related tokens as implicit references remains an evaluator concern owned by
#150.

Call syntax has the following properties:

- The same `call-identifier` scanner is used for function-call names and named
  argument candidates. `_`, `-`, and hyphenated forms are not part of a call or
  named-argument identifier; nonmatching suffixes remain source text rather
  than being folded into the identifier.
- **Implicit positional references** (`.1`, `.2`, `.12`, `.0`, `.01`, ...) are
  numeric call identifiers at the grammar boundary. `.1abc` is the `.1` token
  followed by ordinary source, just as `.1-1` is. Binding and evaluation of
  these references remain separate from lexical recognition.
- A call introducer may begin at source start or after a byte other than ASCII
  alphanumeric, `.`, or `\\`, matching the pinned call-pattern evidence. This
  permits UTF-8 surroundings and symbol/underscore surroundings while keeping
  `word.foo` and `..foo` outside the call. Source following a parsed call, such
  as `.foo {x}한글`, remains available to the frontend placement layer.
- Positional arguments are wrapped in curly braces: `{...}`.
- Named arguments are `name:{...}`. The identifier, `:`, and `{` must be
  adjacent; `name :{...}`, `name: {...}`, and `name : {...}` do not create a
  named argument. When that optional argument does not match, the parser stops
  at the call prefix and leaves the candidate as source remainder; malformed
  braced values that do match the `name:{` boundary retain their structured
  diagnostic path.
- Positional and named arguments may be mixed. Scribium's grammar/frontend
  preserves the complete source-ordered argument sequence, including an
  unnamed argument after a named argument, and does not report parser `E2001`
  for that shape. The engine retains that sequence alongside the legacy
  positional/named projections, and its shared invocation binder rejects
  positional-after-named with source-backed `E3003`, using the original
  argument spans. In pinned Quarkdown v2.5.1, `FunctionCallGrammar` preserves
  the sequence and `RegularArgumentsBinder` owns validity checking. The
  frontend/IR representation is the bounded #163 prerequisite; shared
  semantic binding is the #165 engine contract.
- An escaped call introducer is literal, and escaped `{`/`}` delimiters do not
  change call-argument depth in pinned v2.5.1. Scribium currently records the
  introducer boundary but counts escaped argument braces while scanning; the
  resulting UTF-8/CRLF truncation and malformed behavior are tracked by #162.
- An argument may contain a plain value (`{320}`, `{center}`, `{"text"}`) or
  arbitrary content, including **nested calls**: `.outer {.inner {value}}`.
  Supported Markdown inline structure in static content arguments is retained
  as the existing source-backed `Inline` nodes, including nested calls. This is
  parser/frontend evidence only; dynamic String/content conversion remains a
  separate boundary.
- Braced arguments may span physical lines, including nested braces. Their
  indentation is preserved as source content and is not a fixed-width syntax
  rule.
- Current Scribium consumes a backslash continuation only after an argument
  has already been parsed; `.foo \\` followed by a first argument stops at
  `.foo`, and a trailing continuation reports `E2004`. Pinned v2.5.1 places an
  optional separator before every argument and separately consumes a trailing
  continuation. The pinned token directly checks backslash + LF, so local
  CRLF acceptance is recorded separately rather than treated as raw-CRLF
  upstream conformance. Separator placement is tracked by #164.
- `::` parses and structurally preserves a direct call chain (`.a {x}::b {y}`),
  but current Scribium requires `::` immediately at the current call end;
  whitespace or a line continuation before `::` is the #164 separator gap.
  The direct form includes each segment and argument source span. The
  evaluator executes
  supported chain segments directly in strict left-to-right order: the prior
  semantic value is injected as the next segment's first positional argument,
  while explicit positional and named arguments retain their order and names.
  For the evidenced surface, `.a::b` and its documented nested equivalent
  `.b {.a}` use the same value-context invocation path and therefore produce
  equivalent semantic values and observable output. The current semantic
  evidence set is `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`,
  `.abs`, `.negate`, `.sqrt`, `.truncate`, `.round`, `.iseven`, `.string`, `.concatenate`, `.uppercase`,
  `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, and `.startswith`; an
  unknown or otherwise unexecutable chain segment reports a source-backed
  `E3001` evaluation diagnostic and does not fabricate a value.
  The parser's structural representation is consumed directly; no synthetic
  source or Markdown/Typst round trip is used.
- A complete call may be wrapped in braces to lift word-adjacency boundaries,
  for example `H{.text {2}}O`. The wrapper is consumed by the Quarkdown
  frontend and its source span remains available. A tight call nested inside
  a braced content argument is also preserved as one source-backed nested call;
  its wrapper braces are not emitted as ordinary content text.
- Inline calls appear inside a paragraph: `.strong {bold}` in surrounding
  text. A call that has trailing text after it on the same line is treated
  as an inline call, not a block-level call.
- Malformed inline calls retain their diagnostic span, but recovery must also
  retain following source text; the current suffix-loss gap is tracked by
  #159.

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

### `.docauthor` document state (M3 bounded slice)

`.docauthor` uses the existing document-state read/write convention:

```quarkdown
.docauthor
.docauthor {Alice}
.docauthor author:{Bob}
The first author remains .docauthor.
```

An argumentless call returns an empty string when no author has been added;
otherwise it returns the first author name. A successful positional or named
setter appends one author to the evaluator-owned document state and emits no
document content. Insertion order is preserved in the immutable IR snapshot.
Invalid arity or named arguments fail with a source-backed diagnostic before
the new author is committed. `.docauthors`, front-matter `author`, and other
document metadata are separate or deferred contracts.

### Variable Reference (Implemented)

Variable references use the same parameterless call syntax as function calls.
A variable must be declared with `.var` before it can be referenced.

```
.var {name} {value}         // declaration (no output)
.name                       // reference (evaluates to variable value)
.name {new-value}           // reassignment (only if `name` is a variable)
```

- Variable declaration names follow the evaluator-owned declaration grammar:
  `[A-Za-z_][A-Za-z0-9_-]*`; this is distinct from call-token lexing.
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
Invalid variable names (not matching the declaration-name grammar) produce
`E3002`.

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

Generic standalone lambdas outside the supported first-class forms and
components remain outside this slice. A rich block result that cannot be
represented in an inline context is rejected with a source-backed diagnostic
rather than flattened or dropped.

### First-class callable values and collection transforms

The evaluator supports a typed first-class callable value. The explicit
source-backed form is `@lambda`, while transform callbacks also accept the
contextual unmarked form in a `by` argument:

```text
.var {identity} {@lambda .1}
.map {1..3} by:{.identity}
.map {1..3} by:{value: .value}
.filter {1..3} by:{@lambda .1::isnone}
.sorted {.map {1..3} by:{@lambda .1}}
.sorted {3..1} by:{@lambda .1}
```

Explicit parameters bind in a fresh child scope; headerless lambdas bind
`.1`, `.2`, and later arguments in the nearest invocation scope. Captured
values are immutable snapshots of the definition context. `.map`, `.filter`,
and `.sorted` all consume the shared typed iterable path and return recursive
typed `Collection` values. `.filter` requires a Boolean predicate. `.sorted`
is stable ascending natural/selector ordering for homogeneous Number, String,
or Boolean keys; unsupported, heterogeneous, `None`, and invalid key values
produce diagnostics. Descending options and arbitrary comparator syntax are
not part of this slice. `.foreach` and `.sorted` are the Quarkdown v2.5.1
evidenced operations. The retained `.map` and `.filter` calls are Scribium
extensions, not upstream v2.5.1 functions, and are excluded from compatibility
coverage counts.

### Collection operations (Quarkdown v2.5.1 evidenced slice)

All operations consume the same typed iterable sequence as `.foreach`, so
`Collection`, `Pair`, ordered `Dictionary` entries, finite `Range`, and a
supported Markdown list have identical element semantics:

```text
.values::second
.values::third
.values::distinct
.values::reversed
.values::sumall
.values::average
.values::groupvalues
```

`.second` and `.third` are one-based accessors and return `None` when the
sequence is too short, matching `.getat {2}` and `.getat {3}` without a
fallback. `.distinct` keeps the first occurrence. `.reversed` returns a new
typed Collection. `.groupvalues` returns a Collection of Collections in
first-seen group order, preserving order inside each group. `.sumall` applies
the upstream `asDouble()` conversion to every element; invalid conversions
contribute zero, and `.average` divides by the full input count (empty input
therefore produces `NaN`). These results remain typed until an output boundary.

### Scoped `.let` (Implemented slice)

Block-form `.let` invokes a one-parameter lambda in a child scope. The value
argument is evaluated once in the caller scope, then binds either the explicit
header parameter or the headerless implicit `.1` parameter. Parent variables
and functions remain visible, while local declarations and shadowing stay
inside the invocation. The callable body uses the same semantic result
accumulator as `.function`, preserving a single scalar or structured content
result and composing multiple outputs in source order.

```text
.let {Quarkdown}
    name:
    .uppercase {.name}

.let {Quarkdown}
    .uppercase {.1}
```

Only block-form `.let` is implemented in this slice; first-class callable
values are available to the collection-transform callback path described above.

### Evaluation scope (Implemented)

The evaluator now has explicit parent/child scope APIs with deterministic
lookup, local variable bindings, and source-backed local function bindings.
Child scopes inherit visible parent bindings and local writes do not leak back
to the parent. Existing `.var` declarations continue to use the document-level
scope and are evaluated in source order. The evaluator represents callable
parameters as either explicit source-backed bindings or an implicit positional
binding mode. Each invocation installs its own lambda-local argument scope;
nested invocations therefore shadow only while active and restore the outer
implicit arguments afterward. Standalone lambda syntax outside the supported
`@lambda`/transform forms remains deferred; the iteration forms below reuse
this same invocation machinery.

Function arguments and chain intermediates are evaluated in value context,
which preserves scalar values and evaluated content until a final document
output context materializes them as nodes or inline text. Conditional bodies
remain lazy until the callee selects a branch. The current string-family and
numeric builtins use deliberately small invocation-boundary adaptation
contracts for strings, identifiers, booleans, numbers, and plain text content;
this is not a claim of complete Quarkdown `DynamicValue` or standard-library
compatibility. Numeric functions accept only the scalar number forms evidenced
by the v2.5.1 `ValueFactory` boundary; structured values are not coerced
through text or a backend.

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

Nested conditionals are supported. Variable references (`.name`) in conditions
resolve to the variable's boolean value. The bounded logical/comparison family
also produces typed condition values:

```text
.if {.islower {2} than:{3}}
    lower
.ifnot {.isgreater {2} than:{3}}
    not-greater
.if {.equals {2} to:{"2"}}
    equal
.if {.not {.equals {2} to:{3}}}
    different
```

`.islower` and `.isgreater` accept numeric `a`/`than` values and optional
`orequals:{true|false}`; `.equals` accepts `a` and `to`; `.not` accepts one
boolean. Numeric ordering follows the upstream float comparison boundary, and
equality applies the documented plain-text fallback only for comparable
strings, numbers, and Markdown content. Invalid values produce one
source-backed `E3001`, and a failing condition does not evaluate or publish its
body. Other function-call conditions remain outside this bounded slice until
their owning semantic family is implemented.

### Mathematical and numeric operations (Implemented bounded slice)

The v2.5.1 arithmetic/unary slice uses the existing typed evaluator boundary:

```text
.sum {1} {2}
.subtract {10} {3}
.multiply {4} by:{2}
.divide {7} by:{2}
.rem {-5} {2}
.pow {-2} to:{0.5}
.abs {-3.5}
.negate {3}
.sqrt {9}
.truncate {201.06194} decimals:{2}
.round {2.5}
.iseven {4}
```

The implemented functions are `.sum`, `.subtract`, `.multiply`, `.divide`,
`.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`,
`.tan`, `.truncate`, `.round`, and `.iseven`, integrated with the existing
numeric builtins. `.range` remains a separate typed constructor. Binary and
unary calls use the shared
positional/named/mixed argument binder; `x` uses the existing narrow numeric
adaptation, while `.truncate`'s `decimals` uses a strict integral-compatible
adapter. Results remain `IrValue::Number`, except `.iseven`, which returns
`IrValue::Boolean` and can feed `.if`/`.ifnot` without text materialization.

Arithmetic follows the v2.5.1 `Math.kt` floating boundary and its
`NumberValue` normalization. `.truncate` rejects negative `decimals`, rejects
fractional or quoted-text decimal arguments, uses `x.toInt()` for zero
decimals, and otherwise preserves the upstream Float/Double/toInt/Float
operation order. Negative values truncate toward zero. `.round` uses explicit
Kotlin ties-to-even behavior followed by `toInt()`; `2.5`, `3.5`, `-2.5`, and
`-3.5` therefore produce `2`, `4`, `-2`, and `-4`. Division-by-zero results
clamp to the upstream Int boundaries when integral, `0/0` remains `NaN`, and
remainder keeps signed floating behavior. `.pow` truncates its exponent
through the upstream `Number.toInt()` boundary, `.iseven` checks that same
truncated integer, and a negative `.sqrt` produces `NaN`. Invalid values,
unsupported structured conversions, arity errors, unknown/duplicate names, and
block bodies fail closed with the existing source-backed evaluator diagnostic.
Nested failure does not publish a partial value or enclosing document output.

The transcendental functions reuse the shared `x` numeric boundary. They first
adapt to Float, then use the pinned pure-Rust `libm` binary64 software
operation and narrow to Float, matching the Kotlin/JVM Float overload without
Rust `std` transcendental calls or OS math FFI. `.pi` preserves the upstream
binary64 `PI` constant and bypasses Float result normalization; the existing
`NumberValue`-compatible evaluator normalization remains authoritative for
the four Float results, including NaN, infinity, and signed-zero cases.

### Scalar string operations (Implemented bounded slice)

The v2.5.1 scalar string family uses the existing typed evaluator invocation
boundary:

```text
.string {value}
.concatenate {abc} with:{def} if:{yes}
.uppercase {hello}
.lowercase {HELLO}
.capitalize {hello, world!}
.isempty {""}
.isnotempty {" "}
.startswith {Hello} {he} ignorecase:{yes}
```

`.string` accepts the bounded scalar forms already represented by the frontend
and returns `IrValue::String`. A quote-delimited scalar such as
`.string {"  Hello  "}` is classified by the Quarkdown grammar, with only the
outer quotes removed and the inner whitespace preserved. `.concatenate` uses
`a`, `with`, and optional `if` (default `true`); `.startswith` uses
`string`, `prefix`, and optional `ignorecase` (default `false`). The case
transforms and predicates use the same named/positional binding and scalar
adaptation contract. The predicates return typed `IrValue::Boolean` values and
can be used directly in lazy `.if`/`.ifnot` conditions.

Strings, identifiers, numbers, booleans, and bounded plain-text content are
adapted at this function boundary. `None`, collections, ranges, pairs,
dictionaries, callables, and rich Markdown content are not implicitly
stringified. `.plaintext` is a separate projection of already-parsed
`IrValue::Content`: formatting delimiters are omitted, code and link labels
recurse, soft breaks emit a newline, and hard breaks/images emit nothing.
Markdown-bearing `String` values are reparsed only by the explicit
`.plaintext` Dynamic String → InlineMarkdownContent target; generic
String-to-Markdown conversion remains a documented compatibility gap.

### Iteration (Implemented first slice)

`Range` is a typed value, not text that the evaluator reparses. Its literal
syntax accepts non-negative integer endpoints and preserves open endpoints:

```text
2..4
2..
..4
..
```

Closed ranges are inclusive. `Collection` is an ordered, recursive typed
iterable value whose elements retain their semantic kinds (`Number`, `String`,
`Boolean`, `Content`, `Range`, or another `Collection`).

`.foreach` maps one iterable through a block lambda and returns a
`Collection` containing one typed result per iteration:

```text
.foreach {2..4}
    number:
    Number .number

.foreach {2..4}
    .1
```

The iterable expression is evaluated once. Each element receives a fresh
child scope with parent visibility; explicit `number:` binds a typed value,
while a headerless body uses the nearest invocation-local `.1`. A body result
of `NoValue` is an error, not fabricated `None` or an empty string, and a
failed iteration stops the mapping without duplicating its diagnostic.

`.repeat {n}` is the shared iteration engine with one-based values `1..n`:

```text
.repeat {3}
    n:
    .n
```

`repeat {0}` returns an empty collection. Fractional, non-finite, negative,
and unrepresentable counts are rejected. Descending closed ranges follow the
verified upstream v2.5.1 behavior of an empty iteration. Left-open ranges use
`1` as their default start when consumed as an Iterable; right-open and
fully-open ranges remain valid typed values but are rejected by the shared
finite Iterable path as endless.

An exactly-one Markdown ordered or unordered list is adapted to a Collection
only when a value is required by `.foreach`; ordinary document lists remain
`UnorderedList` or `OrderedList` IR nodes. Nested list-only items adapt
recursively to nested Collections, while rich list-item content remains typed
content. Pair, Dictionary, and generalized destructuring remain bounded to the
evidenced forms. Native `.foreach`, `.sorted`, and the Collection operations
listed above use the shared typed iterable path. `.map` and `.filter` use that
same path as Scribium extensions; they are not asserted as Quarkdown v2.5.1
functions. Dynamic
`.range` is a typed constructor with optional `from`/`to` bounds; its numeric
bounds are evaluated normally and truncated to signed integer endpoints using
the verified upstream Number-to-Int behavior.

### Include / Read (Partial; bounded VirtualProject subset)

`.include`, `.read`, and `.json` are evaluated by `scribium-engine` through its
engine-neutral `ResourceProvider` interface. `scribium-core` owns the adapter
that backs that interface with `VirtualProject`; the engine does not own or
depend directly on the project model. Paths are logical and source-relative;
normalization rejects traversal outside the project boundary, and nested
includes retain source identity for subsequent relative resources. The
compiler does not access the host filesystem or network from this boundary.

The three builtin rows are canonical `PARTIAL` resource support, not complete
Quarkdown compatibility. The common logical resolver is #188; the
VirtualProject/ResourceProvider contract is a separate `SUPPORTED_SEMANTICS`
row, and Typst source context is `PARTIAL` pending #187. See the cross-audit
decision in [`RECONCILIATION.md`](compatibility/quarkdown/RECONCILIATION.md).

See [`docs/compatibility/quarkdown/README.md`](compatibility/quarkdown/README.md),
[`docs/compatibility/quarkdown/GAP_INVENTORY.md`](compatibility/quarkdown/GAP_INVENTORY.md),
and [`docs/adr/0019-typst-source-and-resource-context.md`](adr/0019-typst-source-and-resource-context.md).

### Native Typst passthrough

Native `.typ` passthrough, if implemented, is a host-level input capability
that sends a `.typ` document to the selected official Typst compiler. Scribium
does not embed raw Typst source in backend-neutral IR and does not define a
generic backend escape block. The current CLI rejects `.typ` input until the
separate passthrough capability is implemented.

### Data Loading (Partial; bounded `.read` / `.json` / `.include`)

The implemented resource model covers project-backed text, JSON, and nested
document inclusion. Other data-loading families, including directory
enumeration, CSV/list loading, and remote/package resources, remain deferred
until their separate compatibility and host-boundary contracts are reviewed.

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
