# Markdown Parser Foundation Audit

- **Status:** Accepted design evidence
- **Baseline:** `main` at `06daad7` (2026-08-11 audit)
- **Related work:** closed PR #45; retained branch `feat/m2-blockquotes`
- **Scope:** parser architecture and migration design only

## Executive conclusion

The current parser is a recursive block parser over a source-line slice, not a
container-state parser. That is sufficient for the current main subset, but it
does not give one owner to container continuation, paragraph interruption, or
lazy continuation. PR #45 made the missing ownership visible: the blockquote
collector acquired a second state machine that replays list, quote, fence, and
leaf rules while the real parser still owns those rules elsewhere.

The physical migration should move the Markdown frontend to the target
`scribium-markdown` crate, with block parsing behind one authoritative
`BlockParser` state. Quarkdown-specific grammar belongs in the target
`scribium-quarkdown` crate, which `scribium-markdown` may call. Blockquote
remains out of the foundation implementation; it is a later container
migration using the same state machinery. PR #46 defines this ownership but
does not physically create or move either crate.

## Current pipeline

The current core entry point calls `syntax::markdown::parse_with_diagnostics`,
then `ast_to_ir`, then the evaluator (`crates/scribium-core/src/lib.rs:39-87`).
The Markdown module has one large parser file:

```text
syntax/markdown/parser.rs
  SourceLine + split_lines                 157-269
  parse_blocks                             271-350
  Quarkdown block call/body collection     353-478
  paragraph and block constructors         509-843
  block classifiers                         844-961
  InlineParser + parse_inlines              962-1314
  unit tests                                1315-2822
```

`syntax/quarkdown/parser.rs` owns call grammar, argument scanning, scalar
conversion, and call diagnostics. It explicitly leaves indented block bodies
to the Markdown parser. `ast_to_ir.rs` recursively converts AST children, and
the evaluator recursively evaluates directive bodies and unordered-list item
nodes. Neither downstream layer should participate in block-container
ownership.

An earlier architecture draft described a `Lexer / Tokenizer` between source
abstraction and parser, but no such layer exists in the implementation. The
accepted architecture now uses `SourceLine` and the physical-line
scanner/classifier as the first lexical unit.

## Responsibility audit

| Responsibility | Current owner | Duplicate/reconstructed rule | Foundation disposition |
|---|---|---|---|
| Physical line representation | `SourceLine`, `split_lines` (`parser.rs:157-269`) | `raw`, `text`, `indent`, and absolute offsets are carried into synthetic slices in list/directive parsing | Centralize in `block/line.rs`; preserve byte offsets and LF/CRLF behavior |
| Indentation | `SourceLine::indent`, list `content_col`, directive `MIN_BODY_INDENT` | Unordered and ordered lists recompute marker/content columns separately; quote code replays them again | `LineView` computes one effective indentation and consumed-prefix view |
| Block-start classification | Direct checks in `parse_blocks`; `SourceLine::starts_block` | Paragraph termination and top-level dispatch do not share one result; PR #45 adds `text_starts_block` and quote-local checks | Pure recognizers return candidates; `BlockParser` owns the final classification |
| Block interruption | Implicit in `parse_blocks` order and `starts_block` | There is no explicit paragraph interruption decision; trimmed text can hide indentation context | Central `classify_remaining_line` decision after container reconciliation |
| Paragraph continuation | `parse_paragraph` loop (`parser.rs:509-559`) | Current main uses line shape; PR #45 adds a quote-only logical buffer and continuation state | `OpenLeaf::Paragraph` is the sole owner of lazy/normal continuation |
| List item continuation | `parse_list` / `parse_ordered_list` (`parser.rs:615-825`) | Two list implementations duplicate body collection, blank lookahead, and `strip_indent` reparse | Stack-owned `List` + `ListItem` states; one continuation algorithm |
| Nested blocks | Recursive `parse_blocks` over `item_lines` and directive body slices | Container identity is reconstructed from transformed lines; recursion loses the active parent state | Push/pop containers in one parser; recursion is not used to rediscover block context |
| Fenced block lifecycle | `parse_code_block` (`parser.rs:584-613`) | Fence ownership is implicit in a local loop; PR #45 duplicates open/close fence state | `OpenLeaf::FencedCode` owns opener, closer, and content until closure/EOF |
| Heading/thematic break | Direct branches in `parse_blocks` | Same predicates are reused from paragraph termination without a shared result | Markdown recognizers are pure; interruption policy remains parser-owned |
| Blockquote/container | Not present on main; PR #45 `parse_blockquote` (`feat/m2-blockquotes:1055+`) | `QuoteContinuation` and `ContinuationContainer` replay list/quote/fence/leaf semantics | Reserved `ContainerKind::BlockQuote`; implementation deferred to blockquote-v2 |
| Inline parsing | `InlineParser` in the same 6k-line PR/main parser file | Quote content requires a synthetic joined buffer and span translation | Move behind `inline/`; accept source segments, never publish synthetic offsets |
| Source spans | `ByteSpan` attached in constructors and inline remapping | `strip_indent` and quote buffers require offset translation; each caller knows part of the mapping | Line views and inline source segments own mapping; AST/IR contracts stay unchanged |
| Quarkdown directive recognition | `is_block_directive_line` + `parse_directive_block`; grammar in `quarkdown/parser.rs` | Standalone-vs-inline and indented-body rules are split across two modules without shared block state | Internal `BlockStart::Quarkdown`; grammar parser stays Quarkdown-specific, body lifecycle stays BlockParser-owned |
| Quarkdown body collection | `collect_directive_body` (`parser.rs:400-459`) | Body is sliced and recursively reparsed with a fresh cursor | Use an `ExtensionBody` container in the same parser state |
| Front matter | `parse_front_matter` pre-pass (`parser.rs:39-103`) | Intentionally document-prologue-specific, but it shares `SourceLine` primitives | Keep as document framing before `BlockParser`; use the same line model |
| Diagnostics/recovery | `ParserDiagnostic`, Quarkdown `ParseError`, inline fallback | Recovery is local to each recognizer; duplicate calls can report or suppress errors differently | One diagnostic sink in `BlockParser`; recognizers return recoverable outcomes |

## Architectural findings

1. `parse_blocks` is the de facto dispatcher, but it has no open-container
   stack. Its `depth` parameter only bounds recursion; it is not semantic
   parser state.
2. `SourceLine::starts_block` and the `parse_blocks` branch sequence are two
   classifications of the same physical line. The paragraph loop consults one
   while the dispatcher executes the other.
3. Lists and directive bodies create new `SourceLine` views and invoke
   `parse_blocks` again. This preserves absolute offsets in many cases, but it
   reconstructs container semantics from source shape instead of carrying the
   active container chain.
4. PR #45 adds the clearest duplicate: `QuoteContinuation::feed` has its own
   container stack, leaf enum, fence ownership, indentation arithmetic, and
   block-start checks. Its last commit grew to 557 insertions and 311
   deletions in `parser.rs`; the full PR diff adds 4,632 lines to that file and
   changes AST/IR/evaluator/lowering and E2E tests as well.
5. The quote implementation therefore exposed a design limitation rather than
   an isolated quote bug. The regression cases are valuable evidence, but the
   implementation must not be promoted or cherry-picked into the foundation.
6. The inline parser is a legitimate recursive parser for inline delimiters and
   nested argument content. That recursion is different from block-container
   reparsing and should be isolated rather than removed indiscriminately.

## Accepted design decisions

### Frontend crate ownership

The target frontend split is `scribium-markdown` for the Markdown frontend and
`scribium-quarkdown` for Quarkdown-specific grammar. The Markdown crate owns
line scanning, `LineView`, `BlockParser`, container lifecycle, Markdown
recognizers, inline parsing, front-matter framing, block-layer recovery, and
the frontend AST. The Quarkdown crate owns call and argument grammar only; it
does not own Markdown parser state or AST types. Extraction remains deferred to
the post-merge physical migration.

### Lexer/tokenizer terminology

Choose Option B: the physical-line scanner and classifier are Scribium's
Markdown frontend lexical layer. Do not add a token stream merely to satisfy
the current architecture diagram. The accepted architecture document replaces
the nonexistent generic lexer box with `physical-line scanner/classifier`,
then `BlockParser`.

### Markdown/Quarkdown boundary

`scribium-markdown` owns line consumption, containers, leaves, interruption,
continuation, spans, and recovery. `scribium-quarkdown` owns only dot-call
names, arguments, scalar/content classification, and grammar errors. The
first-party frontend integration uses enum/function dispatch
(`BlockStart`), not a public plugin trait.

## Target dependency direction

```text
scribium-markdown
        |
        v
scribium-quarkdown
```

`scribium-markdown` depends on `scribium-quarkdown`, may invoke it for block or
inline calls, and normalizes the result into its frontend AST.
`scribium-quarkdown` must never depend on Markdown parser or AST types. Any
shared source/span dependency is a lower-level owner whose final crate
boundary is resolved separately.

Recognizers may classify a candidate but may not advance the cursor, mutate the
container stack, collect a body, or start a lazy continuation. Those operations
belong only to `BlockParser`.

## Target module structure

```text
crates/scribium-markdown/src/
├── lib.rs
├── ast.rs
├── block/
│   ├── mod.rs
│   ├── parser.rs       # process_line, reconcile, finalize, emit
│   ├── state.rs        # BlockParser, OpenContainer, OpenLeaf, diagnostics
│   ├── line.rs         # SourceLine, LineView, source segments, indentation
│   ├── classify.rs     # pure Markdown/Quarkdown block candidates
│   ├── list.rs         # list candidate data; no independent parser loop
│   ├── fence.rs        # fence candidate/leaf data
│   └── heading.rs      # heading candidate/content boundaries
└── inline/
    ├── mod.rs
    ├── parser.rs
    ├── delimiter.rs
    ├── code.rs
    └── link.rs
```

This is a responsibility boundary, not a requirement to create every file in
one commit. The first refactor may keep small recognizers together until a
dependency direction is proven. No public API changes are planned.

## Main baseline evidence

An extracted `main` snapshot passed `cargo fmt --all --check` and
`cargo test --workspace --all-features`: 58 CLI, 292 core, 8 test-support, 21
Typst unit, 5 backend integration, and 7 upstream-watch tests passed. This is
the behavior-freeze starting point for the foundation migration.
