# ADR-0014: Markdown Block Parser Foundation

- **Status:** Accepted
- **Date:** 2026-08-11
- **Owners:** Scribium maintainers
- **Related ADRs:** 0002, 0006, 0007, 0012, 0015
- **Related work:** closed PR #45; `refactor/markdown-parser-foundation`

ADR-0015 is the authority for the final crate ownership surrounding this
parser design.

## Context

Scribium's Markdown implementation is a custom, span-preserving subset parser,
currently housed in `scribium-core`. It represents physical lines and
recursively reparses transformed line slices for list items and Quarkdown
bodies. Block-start checks are repeated between the main dispatcher and
paragraph termination.

PR #45 attempted blockquotes and exposed the consequence. Lazy paragraph
continuation, nested quote/list containers, list indentation, fence lifecycle,
and block interruption required a second `QuoteContinuation` state machine
that mirrored the real parser. The PR is closed without merge; its tests and
failure cases are retained as future regression fixtures. This ADR defines the
foundation before blockquote is implemented again.

## Decision 1: dedicated frontend crate ownership

The long-term parser implementation is not owned by `scribium-core`. After
this architecture is accepted, frontend extraction targets two first-party
crates. The current implementation may remain physically under `scribium-core`
during that migration; that transitional location does not change the target
ownership.

### `scribium-markdown`

`scribium-markdown` will own the Scribium Markdown frontend:

- physical-line scanning and classification;
- `LineView`;
- `BlockParser`;
- block and container lifecycle;
- Markdown block recognizers;
- the Markdown inline parser;
- front-matter framing;
- parser recovery at the Markdown/block layer; and
- the frontend AST produced by this parser.

The frontend AST may contain Scribium or Quarkdown extension nodes, including
directive and function-call nodes. `scribium-markdown` does not implement
Quarkdown argument grammar itself.

### `scribium-quarkdown`

`scribium-quarkdown` will own only Quarkdown-specific grammar:

- dot-prefixed function/call name grammar;
- call-boundary recognition;
- positional argument parsing;
- named argument parsing;
- scalar/content argument classification;
- nested Quarkdown argument grammar;
- Quarkdown grammar parse errors; and
- grammar-level intermediate call and argument types.

It must not own physical-line iteration, Markdown paragraph state,
list/container lifecycle, block interruption, indented-body collection,
Markdown AST construction, or `BlockParser` state.

### Dependency direction

The dependency direction is architectural and must not be reversed:

```text
scribium-markdown
        |
        v
scribium-quarkdown
```

`scribium-markdown` depends on `scribium-quarkdown`. `scribium-quarkdown` must
never depend on `scribium-markdown`.

`scribium-markdown` may invoke the Quarkdown grammar when recognizing block or
inline calls and then normalize the result into the frontend AST.
`scribium-quarkdown` must not depend on Markdown parser or Markdown AST types.

If the frontend split requires shared source/span types, `scribium-markdown`
depends on `scribium-source`; `scribium-quarkdown` may do so when its grammar
result requires those low-level types. `scribium-source` is the lower-level
owner of source identity and source-location primitives. ADR-0015 records the
broader source, project, and core crate ownership details; it does not design
the future segment-aware inline-input API.

## Decision 2: physical-line scanning is the lexer layer

Choose Option B from the design review: the physical-line scanner and
classifier are `scribium-markdown`'s lexical layer. `SourceLine` already
provides the needed raw slice, content slice, indentation, line terminator,
and absolute byte offsets. A generic token stream would add no value for the
current block grammar and would not solve container ownership.

`docs/ARCHITECTURE.md` must therefore describe:

```text
Source abstraction
  → scribium-markdown physical-line scanner/classifier
  → scribium-markdown BlockParser (Markdown baseline + Quarkdown extension dispatch)
  → frontend AST
```

- architecture documentation must clearly distinguish current implementation from the accepted target architecture;
- accepted target layers may be documented before physical extraction when they are explicitly identified as design targets;
- documentation must never claim that a target crate or layer already exists physically when it does not.

## Decision 3: one authoritative `BlockParser` state

All physical lines are processed by one state owner:
`scribium-markdown::BlockParser`. Feature recognizers are pure candidate
classifiers; they do not own a cursor, container stack,
paragraph state, body collector, or diagnostic sink.

The target frontend model is conceptually:

```text
BlockParser
├── source lines and current SourcePosition
├── open_containers: Vec<OpenContainer>
│   ├── List(ListState)
│   ├── ListItem(ListItemState)
│   ├── ExtensionBody(ExtensionBodyState)
│   └── BlockQuote(BlockQuoteState)       # activated by a later PR
├── open_leaf: Option<OpenLeaf>
│   ├── Paragraph(ParagraphState)
│   ├── FencedCode(FenceState)
│   ├── Heading(HeadingState)
│   └── other finalized leaf data
├── emitted blocks / AST builder
└── diagnostics and source mapping
```

`BlockQuoteState` is part of the target model but is not enabled by the
foundation implementation. The foundation must not add a blockquote AST/IR
node, Typst lowering, or blockquote behavior.

The state invariant is:

> For each physical line, one `BlockParser::process_line` path decides
> container continuation, block interruption, paragraph continuation, leaf
> transition, and source/diagnostic updates.

There must be no `QuoteContinuation`, `ListContinuation`, or
`DirectiveContinuation` feature-local machine that reimplements this sequence.
Container-specific data is allowed inside `OpenContainer`; ownership of the
decision remains with `BlockParser`.

## Line processing contract

`BlockParser::process_line` follows this order for every physical line:

1. Build a `LineView` from the physical source line. It contains the raw
   source range, effective indentation, content range, terminator, and the
   currently consumed container prefix. The existing indentation/CRLF
   behavior is frozen during the foundation refactor.
2. Reconcile the existing `open_containers` against the line. Close the
   longest non-continuing suffix and finalize any affected leaf. Compute the
   remaining line view once. Do not recreate a synthetic line and call the
   whole parser again.
3. Ask pure recognizers for a `BlockStart` candidate at the remaining view.
   The candidate includes the construct and source spans, but not parser state
   mutation.
4. Apply the centralized interruption policy. A block candidate can interrupt
   a paragraph only when the current leaf and container context allow it.
5. If no interrupting block starts and the current leaf is a paragraph, apply
   the generic lazy/ordinary paragraph continuation rule. This rule is not
   quote-specific and is the only authority for paragraph continuation.
6. Transition the leaf/container state, consuming only the current line. A
   list item, extension body, or future quote pushes a container; a fence
   owns subsequent lines as a leaf until its closer or EOF.
7. Record source spans and diagnostics through the parser-owned sinks, then
   advance the single source position.

EOF finalization uses the same close/finalize path as a terminating line. Blank
lines are classified once and their effect on an open paragraph/container is
owned by the same state machine.

## Block recognizer contract

The block layer may use internal enum dispatch rather than a trait:

```text
BlockStart
├── Heading
├── ThematicBreak
├── ListItem
├── Fence
├── QuarkdownCall
└── (future) BlockQuote
```

Recognizers are allowed to inspect only `LineView` and immutable grammar
helpers. They return `None`, a candidate, or a recoverable diagnostic result.
They may not decide whether a candidate interrupts a paragraph, push a
container, collect following lines, or invoke `BlockParser` recursively.

## Markdown and Quarkdown boundary

`scribium-quarkdown` recognizes and parses Quarkdown grammar. `scribium-markdown`
decides how that grammar participates in a Markdown document.

For example, Quarkdown grammar can recognize `.foo {bar}` and return parsed
call data. Markdown decides whether a recognized call is block or inline in
the current document context, owns an indented body/container following a
block call, and converts the Quarkdown grammar result into the frontend AST.

Standalone-call recognition is a `BlockStart::QuarkdownCall` candidate. The
`BlockParser` owns whether the call is a block or inline and owns the indented
body as an `ExtensionBody` container. Inline parsing may call the Quarkdown
grammar parser for inline calls, but Quarkdown grammar does not own block
state. This remains a first-party Scribium frontend integration, not a plugin
API or generic extension framework.

Front matter remains a document-start framing operation. It uses the same
physical-line primitives and consumes its prologue before `BlockParser` starts;
it is not a second block parser.

## Source spans and non-contiguous content

Container markers and list indentation are syntax, not inline content. A
`LineView` therefore retains both the original source range and the remaining
content range. AST and diagnostic spans always point into the original source.

When inline content is logically adjacent but physically separated by removed
container prefixes, the inline boundary receives a segment-aware input (source
ranges plus logical line-break metadata). Any normalization buffer is private
to the inline parser and carries a complete segment map; synthetic offsets must
never escape into AST, IR, or diagnostics. This replaces quote-specific span
translation with one reusable source-mapping boundary.

## Module boundary

The target `scribium-markdown` frontend layout is:

```text
crates/scribium-markdown/src/
├── lib.rs
├── ast.rs
├── block/
│   ├── mod.rs
│   ├── parser.rs
│   ├── state.rs
│   ├── line.rs
│   ├── classify.rs
│   ├── list.rs
│   ├── fence.rs
│   └── heading.rs
└── inline/
    ├── mod.rs
    ├── parser.rs
    ├── delimiter.rs
    ├── code.rs
    └── link.rs
```

This target layout is design-only in PR #46; no crate is created or moved here.
The layout is introduced only where it expresses a real dependency boundary;
it is not a requirement to split every function immediately. Dependencies
flow from source/span primitives to line views, from line views to block state,
from state to pure recognizers, and from parser output to AST. Inline parsing
depends on source/span primitives and Quarkdown grammar helpers, not on
`BlockParser`'s semantic state.

## Compatibility and non-goals

The foundation PR is behavior-preserving for `main`. It does not:

- add blockquote AST, IR, evaluator, lowering, or CLI behavior;
- cherry-pick or structurally reuse PR #45's implementation;
- expand Quarkdown compatibility;
- redesign AST/IR/evaluator/Typst;
- physically create or move crates;
- add a public parser trait;
- decide the remaining compiler boundaries owned by ADR-0015, including diagnostics, IR, evaluation, compatibility, and backend/lowering responsibilities;
- treat crate extraction as complete before the architecture is accepted and
  the migration is performed; or
- special-case CommonMark examples outside the centralized state rules.

PR #46 defines the target frontend ownership. Crate extraction is a
prerequisite migration after PR #46 is merged and before the new `BlockParser`
implementation is treated as complete.

The foundation must freeze and preserve paragraphs, soft/hard breaks, headings,
thematic breaks, unordered/ordered/nested lists, list continuation, fenced
code, emphasis/strong, code spans, links, directives and bodies, front matter,
malformed diagnostics, LF/CRLF, UTF-8/CJK, and exact source spans.

## Migration and review gates

The implementation is split into buildable, behavior-preserving commits:

1. Freeze the main behavior in parser, snapshot, fixture, and cross-layer tests.
2. Extract physical-line primitives and one block candidate classification.
3. Introduce `BlockParser` state and centralized line processing without quote
   support.
4. Migrate paragraph continuation/interruption, then unordered/ordered lists
   and nested item containers.
5. Migrate fences, headings, thematic breaks, front matter framing, and
   Quarkdown indented bodies.
6. Isolate the inline parser behind source-segment input.
7. Update architecture documentation and run the complete foundation gate.

After each step, existing tests must pass. The PR #45 corpus is imported as
disabled/design fixtures during the foundation review and is activated only by
the later `feat/m2-blockquotes-v2` PR.

The foundation gate is:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo check -p scribium-core -p scribium-typst --target wasm32-unknown-unknown
cargo deny check
```

This ADR is Accepted after review of the state model, line-processing order,
Quarkdown boundary, and Option B terminology.
