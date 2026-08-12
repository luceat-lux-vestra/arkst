# Markdown parser substrate spike

> **Preliminary research record.** The markdown-rs/thin-fork recommendation
> below was subsequently tested and superseded by the
> [final feasibility gate](markdown-substrate-final-feasibility.md). Do not
> treat this document's recommendation as the current architecture
> recommendation.

Status: research evidence and recommendation only. This document does not
accept a new architecture, supersede ADR-0014, or migrate production code.

Verified: 2026-08-13 (Asia/Seoul)

## Scope and method

This spike started from the actual `main` after PR #48:

`75195ff1ed425f7ae4848fceafddcfc3429c58b0` (`docs(architecture): establish
full compatibility and engineering policy (#48)`).

The candidate repositories, tags, source, tests, dependency manifests, and
public APIs were inspected directly. The disposable PoC is in
[`tools/spikes/markdown-substrate`](../../tools/spikes/markdown-substrate/).
No production crate was changed and ADR-0014 was not modified.

The base was revalidated on 2026-08-13 without relying on the local tracking
ref: `git ls-remote` reported the same `main` SHA, and the GitHub PR API
reported PR #48 merged into `main` at 2026-08-12 11:39:15Z with that merge
commit. The checkout could not write `.git/FETCH_HEAD` in the restricted
environment, so this direct remote verification replaced a local-ref update;
the research branch is based directly on the verified commit.

The Quarkdown examples used here are clean-room fixtures derived from the
public syntax evidence recorded in
[`SPEC_SOURCES.md`](../compatibility/quarkdown/SPEC_SOURCES.md), including
function-call forms such as `.align {center}`, indented bodies, and nested
calls. The fixture also intentionally exercises Markdown constructs that must
remain opaque or retain their own precedence.

## Current upstream snapshot

All dates below are upstream dates observed during this evaluation. “Latest
release” means the latest stable crate tag/release visible in the official
repository at verification time. For `markdown-it-rust`, the repository has a
crate tag but no GitHub Release object.

| Candidate | Official repository and crate | Exact version/tag and commit | Release/tag date | Last meaningful activity | License | MSRV / platform notes |
| --- | --- | --- | --- | --- | --- | --- |
| markdown-rs | [`wooorm/markdown-rs`](https://github.com/wooorm/markdown-rs), crate [`markdown`](https://crates.io/crates/markdown) | `1.0.0`, `1506572f9b406431402928f3a8b3df0b4ae2d8f5` | 2025-04-23 | 2025-04-23 release commit; repository not archived, 36 open issues | MIT | `rust-version = 1.56`; `#![no_std]` with `alloc`; no production unsafe found in the parser crate |
| comrak | [`kivikakk/comrak`](https://github.com/kivikakk/comrak), crate [`comrak`](https://crates.io/crates/comrak) | `v0.54.0`, `172c2ee7d2c5c262a28be3e407aadf705daea2b7` | 2026-07-12 | 2026-08-10 merge updating the `emojis` dependency; repository not archived, 12 open issues | BSD-2-Clause (`Cargo.toml`, `COPYING`) | `rust-version = 1.85`, edition 2024; library build is WASM-checkable with default features disabled; generated scanners contain unsafe |
| pulldown-cmark | [`pulldown-cmark/pulldown-cmark`](https://github.com/pulldown-cmark/pulldown-cmark), crate [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark) | `v0.13.4`, `38e4d08f14ec4bd9783270e9623db7681ebed968` | 2026-05-20 | 2026-08-07 merge removing a clone; repository not archived | MIT | `rust-version = 1.71.1`; default non-SIMD build forbids unsafe; SIMD is an opt-in unsafe path |
| markdown-it-rust | [`markdown-it-rust/markdown-it`](https://github.com/markdown-it-rust/markdown-it), crate [`markdown-it`](https://crates.io/crates/markdown-it) | `0.6.1`, `2d7c085046a144d221490331b25ca565ecddbb1b` | 2024-07-07 tag; no GitHub Release object | 2024-07-07 tag and issue-40 panic fix; repository not archived, 25 open issues | MIT (`Cargo.toml`, `LICENSE`) | No documented MSRV; edition 2021; `#![forbid(unsafe_code)]`; uses immutable `once_cell`/`OnceCell` caches |

The repository named `rlidwka/markdown-it` is the original JavaScript
markdown-it project, not the Rust candidate. The Rust candidate evaluated here
is the separate `markdown-it-rust/markdown-it` repository.

### Upstream test and dependency observations

The current official source was used rather than crates.io download counts or
older comparison articles.

* `comrak` official tests passed with `cargo test --no-default-features`:
  488 passed, 1 ignored, 0 failed.
* `pulldown-cmark` official tests passed with
  `cargo test -p pulldown-cmark --no-default-features`: the observed unit,
  error, and documentation groups were 56, 23, and 2 tests respectively, all
  passing.
* `markdown-it-rust` official default-feature tests passed with
  `cargo test --tests`. The observed groups included 652 CommonMark tests,
  48 library tests, and the extras, source-map, pathological, and renderer
  groups, with no failures. Its `--no-default-features` test suite currently
  has test files that reference feature-gated `extra::linkify` APIs and does
  not compile; that is a test feature-gating issue, not evidence of a parser
  conformance failure.
* `markdown-rs` contains the official CommonMark/GFM test corpus and documents
  100% CommonMark and GFM claims, but its current workspace test command is
  blocked before parser tests by the upstream dev-dependency combination:
  `swc_common 8.1.1` imports the removed `serde::__private` module with the
  currently resolved `serde 1.0.229`. This was recorded as an upstream test
  environment failure, not counted as a parser failure.

## Standards support

The following summarizes current source, documented options, and the PoC. A
feature being supported by a parser does not mean that it is exposed through a
Quarkdown extension hook.

| Feature | markdown-rs | comrak | pulldown-cmark | markdown-it-rust |
| --- | --- | --- | --- | --- |
| CommonMark | Strongest documented conformance claim; official CommonMark tests in the repository | Official compatibility is tested against CommonMark 0.31.2; 652/652 test claim and current tests | CommonMark parser with official conformance tests; current goal is complete CommonMark | 652 CommonMark tests pass in the current default-feature suite |
| GFM baseline | `Options::gfm()` enables the current GFM constructs; official GFM tests are present | GFM extensions are first-class options and official GFM fixture coverage is present | Tables, strikethrough, task lists, footnotes, and other options are explicit; `ENABLE_GFM` is not a general extension API | Extra plugin covers tables, strikethrough, linkify, typographer, and related features; no equivalent current built-in task-list/footnote set was found |
| Tables | Yes through GFM constructs | Yes | Yes through `ENABLE_TABLES`/GFM options | Yes through `plugins::extra` |
| Strikethrough | Yes | Yes | Yes | Yes through `plugins::extra` |
| Task lists | Yes through GFM constructs | Yes | Yes through task-list options | Not established as a current built-in plugin |
| Autolinks | Yes | Yes | Yes | CommonMark autolinks; linkify is an optional extra |
| Fenced code | Yes | Yes | Yes | Yes |
| Nested lists | Yes | Yes | Yes | Yes |
| Blockquotes | Yes | Yes | Yes | Yes |
| Inline code | Yes | Yes | Yes | Yes |
| Raw HTML | Exposed as MDAST HTML nodes | Exposed as `HtmlBlock`/`HtmlInline`; rendering policy is separate | Exposed as HTML/InlineHtml events | HTML is supplied by the separate HTML plugin and can be omitted |

The “100%” claims are not treated as a substitute for tests. `comrak`,
`pulldown-cmark`, and `markdown-it-rust` were exercised with their current
official suites as described above. `markdown-rs`’s official corpus and
documentation were inspected, but the current upstream workspace dependency
failure prevented a clean test run.

## Representation and public extension surface

| Candidate | Representation | Positions | Custom parser nodes/rules |
| --- | --- | --- | --- |
| markdown-rs | Public fixed MDAST `Node` enum; parser internals/events are private | Public `unist::Position`/`Point`; offsets are UTF-8 byte offsets, line/column are 1-based | No arbitrary public block or inline rule registration. `Constructs` only enables/disables built-in constructs. MDAST node variants are fixed. |
| comrak | Arena-owned tree with `Ast` and fixed `NodeValue`; block and inline nodes are represented in one tree | Public line/column `Sourcepos`, 1-based inclusive; default columns count UTF-8 bytes; no public absolute byte offset | No public parser rule registry. Renderer plugins, syntax highlighting, and programmatic tree mutation are not parser extension hooks. |
| pulldown-cmark | Public event stream with `Start`/`End`, text, code, HTML, breaks, and metadata events; no public AST | `into_offset_iter()` returns source byte `Range<usize>` for each event | No public custom syntax injection or custom event/node type. Events can be filtered or adapted after parsing. |
| markdown-it-rust | Public tree of `Node`s with user-defined `NodeValue` implementations | Block `srcmap` and inline `srcmap` are absolute byte offsets | Real public `BlockRule` and `InlineRule` traits; custom `NodeValue`; `Ruler::before`, `after`, `before_all`, `after_all`, and `require` precedence controls |

The markdown-it API was verified from the current Rust source and by compiling
and running downstream rules, not inferred from the README. A custom block rule
and a custom inline rule were registered as ordinary PoC code and produced
custom nodes with exact source maps.

The markdown-rs internal construct machinery is not public. Its `construct`
and parser modules are private in `src/lib.rs`; the public `Constructs` value is
a fixed set of switches. This is an important distinction from markdown-it’s
public rule registry.

## Source provenance

This is the main acceptance dimension. For the UTF-8 input:

```text
한글 **bold** text
```

the byte ranges are:

* `Strong`: byte range `7..15`, covering `**bold**`;
* strong content: byte range `9..13`, covering `bold`.

The PoC observed these exact values in:

* markdown-rs: `Position` offsets `(7-15)` and `(9-13)`;
* pulldown-cmark: `Start(Strong)` range `7..15` and text range `9..13`;
* markdown-it-rust: node `srcmap` `(7, 15)` and `(9, 13)`;
* comrak: `1:8-1:15` for the strong node and `1:10-1:13` for its content.

Comrak’s columns are UTF-8 byte-oriented in the default source-position mode,
but its public representation does not expose the equivalent absolute byte
offsets. The inclusive line/column convention also differs from the half-open
byte ranges of the other three candidates.

The same PoC used CRLF input and printed source positions/events for headings,
links, lists, blockquotes, inline code, and fenced code. The parsers retained
source positions against the physical CRLF input. Payloads are not always raw
source slices: code content can be normalized while its event/node span still
points into the original input. An adapter must therefore carry the original
source identity and use spans to slice raw source when exact spelling is
required; it must not assume that a node’s string payload is the source.

Nested container source ranges were observable in all four representations,
but their extension APIs differ. The candidates do not all expose both raw
byte spans and a first-class container lifecycle to downstream custom rules.

## Quarkdown integration strategies

### A — Post-process a completed Markdown AST

This is not safe as the primary integration strategy and was actively tested
against the fixtures.

The standard parsers correctly interpret `.foo {bar}` as ordinary paragraph
text when no extension exists. By the time a completed AST is available:

* a block call and its indented body may already be one paragraph or a Markdown
  paragraph inside a list item;
* a blockquote/list boundary and lazy continuation have already been consumed;
* emphasis, links, HTML, and code spans have already divided or shielded the
  text;
* fixed AST/event models in markdown-rs, comrak, and pulldown-cmark cannot
  represent a new Quarkdown node without a lossy text rewrite;
* transforming text nodes cannot reconstruct the parser’s original block/body
  lifecycle or prove that a call was not inside a link, code span, or fence.

Limited post-processing may be useful for a deliberately text-only extension,
but it fails the required Quarkdown block and inline precedence contract.

### B — Preprocessing and placeholders

This is rejected for exact provenance. Replacing constructs before parsing can
change byte offsets and Markdown delimiter behavior, hide nested body content
from the Markdown parser, and make diagnostics refer to synthetic text. A
placeholder map could be engineered for some flat cases, but it would be a
second source-mapping and container parser, and would violate the requirement
that synthetic reconstructed spans are not accepted as original provenance.

### C — Public extension hooks

Only markdown-it-rust provides the required public shape today:

* downstream `BlockRule` and `InlineRule` implementations;
* custom `NodeValue` types;
* rule ordering through `Ruler` methods;
* block source access through `BlockState`, including line offsets and map;
* inline source position and link nesting state through `InlineState`;
* construct shielding inherited from the built-in code-span/fence rules.

The PoC registered a block rule for `.foo {bar}` and an inline rule for the
same syntax. Both produced custom nodes. The block rule ran at the document
root and from the nested parser state established for a list item and a
blockquote; all three nodes retained exact byte ranges for `.foo {bar}`. This
confirms that downstream rules participate in the parser's adjusted container
state instead of requiring a parallel list/quote scanner.

The inline rule was placed before the built-in rules. Code spans and fences,
escaped `\.foo`, and entity spelling such as `&period;foo` remained opaque and
did not produce a custom node. Ordinary text, link text, image alt text, and
text between raw HTML tags did invoke the deliberately naive rule. The rule
could read `InlineState::link_level`, which was nonzero in both link and image
text. This proves that the public API exposes the information needed to narrow
those contexts, but the downstream rule must actually enforce Scribium's
policy. It does not prove that markdown-it-rust has all Quarkdown container or
body semantics already implemented. A custom block rule must consume and
model its body using the exposed block state.

### D — Thin fork

For markdown-rs, a thin fork appears to be the practical route if its standard
conformance and `no_std` properties remain the first-choice basis. The likely
change would add Quarkdown constructs at the existing block/inline construct
and parser state boundaries, add custom internal events or MDAST variants, and
preserve the existing source-position accounting. A preliminary estimate is
roughly 6–10 upstream parser files and 500–1500 lines, depending on whether
Quarkdown body nodes can reuse existing container events. This is an estimate,
not an implementation measurement.

The fork would be coupled to construct dispatch, parser state, event handling,
MDAST conversion, and position accounting. The merge-conflict surface is
medium to high because these are parser-core files, although the patch can be
kept semantically narrow. A comrak fork would similarly touch scanner/parser,
node, and option internals and would have to reconcile the arena tree. A
pulldown-cmark fork would need changes across first-pass parsing/tree building
and event generation, making it less attractive for custom block lifecycle.

## Container semantics and precedence

All candidates correctly own standard Markdown container lifecycle for their
supported grammar. None except markdown-it-rust exposes a public custom parser
hook, and markdown-it-rust exposes state rather than a Quarkdown-specific
container lifecycle.

| Required interaction | Observation |
| --- | --- |
| Code span/fence before Quarkdown | Standard parsers shield code spans and fences. The markdown-it PoC confirms the custom inline rule does not run inside a code span or fenced block; escaped and entity spellings are also consumed before the literal-dot rule. |
| Quarkdown vs emphasis/link | markdown-it can order a custom rule and exposes `InlineState::link_level`. The naive PoC rule ran in ordinary, link, image-alt, and HTML-surrounded text, demonstrating both configurability and the need for an explicit Scribium policy. The other candidates have no public pre-interpretation hook. |
| List/blockquote nesting | Standard container parsing is internal and correct for Markdown. A downstream markdown-it block rule ran from root, list-item, and blockquote states and received exact source maps. `BlockState` also exposes line, indentation, level, and list state, but no dedicated Quarkdown body callback. |
| Lazy continuation and indented body | The substrate can parse the Markdown context, but a Quarkdown body recognizer must participate before the ordinary paragraph/lazy-continuation decision or a fork must add that decision point. |
| Physical source span | markdown-rs, pulldown-cmark, and markdown-it-rust expose byte offsets; comrak exposes byte-oriented line/columns only. Original source must remain available outside normalized node payloads. |

The core architectural risk is not recognizing `.foo`; it is deciding whether a
Quarkdown construct is a block or inline construct at the same point where
Markdown decides paragraph interruption, list continuation, blockquote
continuation, and code shielding. A post-parser text transform cannot recover
that decision reliably.

## Raw HTML and front matter

Raw HTML is exposed by all candidates in their native representation:

* markdown-rs exposes HTML MDAST nodes;
* comrak exposes `HtmlBlock` and `HtmlInline`;
* pulldown-cmark exposes `Html` and `InlineHtml` events;
* markdown-it-rust supplies HTML through its separate HTML plugin.

The PoC observed raw HTML in all four where the corresponding feature/plugin
was enabled. Comrak’s `render.unsafe` is a rendering policy, not a parser
extension mechanism. Scribium’s existing HTML boundary can therefore make the
policy decision without routing HTML through Markdown strings.

Front matter is configurable in markdown-rs, comrak, and pulldown-cmark. The
PoC observed YAML/front-matter nodes or metadata events in those candidates.
The default markdown-it-rust CommonMark setup does not produce a front-matter
node; its parser can be framed by Scribium before parsing. Continuing to own
front-matter framing is compatible with all candidates and avoids making it a
Quarkdown extension concern.

## WASM and platform constraints

The isolated PoC was checked with:

```text
cargo check --manifest-path tools/spikes/markdown-substrate/Cargo.toml \
  --target wasm32-unknown-unknown
```

This passed for all four candidates with default features disabled.

* markdown-rs has the clearest platform story: `no_std` plus `alloc`, no
  filesystem/process API in the parser crate, and no production unsafe found.
* comrak’s parser library can be used in a WASM-compatible no-default-feature
  configuration. Its full default feature set includes CLI/rendering and other
  optional functionality that needs separate audit; the parser layer must not
  inherit those host features accidentally.
* pulldown-cmark’s default non-SIMD path is safe and platform-neutral. SIMD is
  an explicit opt-in and is not appropriate for a generic WASM parser profile.
* markdown-it-rust uses `std` and immutable lazy global caches, but the parser
  itself has no filesystem/process dependency and passed the target check with
  default features disabled. Its optional renderer/highlighting features need
  to remain outside the platform-neutral compiler crate.

No candidate was found to require filesystem, network, or process access for
the core parser API. Feature selection remains part of the eventual dependency
review.

## Safety and error behavior

The candidates differ in implementation policy:

* markdown-rs is safe in the inspected parser sources and is designed as a
  byte-accounting state machine. Its public parse APIs return `Result` for
  MDAST parsing.
* comrak contains unsafe code in generated scanner code. This is dependency
  implementation detail, not Scribium-owned unsafe, but it expands the audit
  surface. Parsing is exposed as a direct tree API rather than a structured
  parse-error result for ordinary syntax.
* pulldown-cmark forbids unsafe in the normal non-SIMD build; its opt-in SIMD
  path is the exception. It is an iterator API and does not generally report
  Markdown syntax errors because Markdown recovery is part of its model.
* markdown-it-rust forbids unsafe. Its immutable `once_cell`/`OnceCell` global
  caches are not mutable compiler state, but they are still a global
  implementation dependency to record. The API is generally recovery-oriented
  and tree-producing rather than a rich diagnostic result.

None of these observations alone rejects a candidate. The selected adapter
must still convert parser failures/recovery and source positions into
Scribium’s typed diagnostics without panics, hidden mutable state, or invented
spans.

## Maintenance, ownership, and performance observations

`comrak` and `pulldown-cmark` show recent 2026 maintenance activity and have
current release tags. `markdown-rs` has a mature 1.0.0 release and extensive
tests, but its latest meaningful upstream commit is the 2025 release. It is
not archived, yet a fork would need to account for a slower observed release
cadence. `markdown-it-rust` has the most attractive extension API but its last
tag and meaningful activity are from 2024; the maintenance risk is material.

The obvious ownership models are:

* markdown-rs: owned MDAST tree, allocations through `alloc`, position-rich,
  no-std-friendly; no public parser plugin surface;
* comrak: arena allocation and borrowed node lifetime, useful for a tree
  adapter but awkward if Scribium requires owned, independently-lived frontend
  nodes;
* pulldown-cmark: streaming/event-oriented output with low structural
  ownership, useful for a standards oracle or direct event adapter but limiting
  for custom Quarkdown block bodies;
* markdown-it-rust: owned parser tree with extensible node values and rule
  ordering, but `std`, older maintenance, and incomplete current GFM coverage
  make the API advantage a tradeoff.

No elaborate benchmark was run. The PoC showed no pathological behavior on the
fixtures. Performance cannot compensate for a substrate that loses precedence
or source identity, so it was not used as a selection criterion.

## Scorecard

Ratings are qualitative and reflect the Quarkdown integration question, not
just Markdown rendering.

| Criterion | markdown-rs | comrak | pulldown-cmark | markdown-it-rust |
| --- | --- | --- | --- | --- |
| CommonMark | Strong | Strong | Strong | Strong |
| GFM | Strong, current constructs | Strong | Good, option-based | Partial/current plugin gap |
| Source provenance | Strong byte offsets | Good line/byte columns, no absolute offsets | Strong byte ranges | Strong byte ranges |
| Block extension | None public; fork likely | None public; fork likely | None public | Strong public rule hook |
| Inline extension | None public; fork likely | None public; fork likely | None public | Strong public rule hook |
| Precedence control | Internal only | Internal only | Internal only | Public rule ordering |
| Container integration | Internal standard context; fork needed for Quarkdown lifecycle | Internal arena parser; fork needed | Internal parser; events arrive after decisions | State is visible, but lifecycle/body policy remains adapter code |
| WASM | Strongest; no_std | Good with feature discipline | Good in default safe profile | Good with feature discipline; std |
| Safety | Safe parser source observed | Generated unsafe scanner code | Safe unless SIMD opt-in | `forbid(unsafe_code)` |
| License | MIT | BSD-2-Clause | MIT | MIT |
| Maintenance | Mature but last release 2025 | Active 2026 | Active 2026 | Risky; last tag 2024 |
| Fork requirement | Likely for full Quarkdown integration | Likely | Likely and structurally invasive | Not required for basic hooks |
| Upstream sync burden | Medium-high if forked | Medium-high if forked | High if forked | Low fork burden, high maintenance risk |

## Recommendation

No candidate currently satisfies all acceptance dimensions unmodified.

### First choice: markdown-rs, conditional on architecture review

Use `markdown-rs` 1.0.0 as the first substrate candidate for a production
design because it combines the strongest documented CommonMark/GFM surface,
explicit byte-accurate positions, no-std/alloc support, and no observed
parser-level unsafe. It is the best fit for a platform-neutral compiler layer
that must preserve original source identity.

A thin fork appears necessary for complete Quarkdown block/inline integration.
The fork should add only the Quarkdown construct dispatch, custom node/event
representation, and the minimum parser-state integration needed to preserve
the substrate’s existing container and position logic. The fork must not copy
or reimplement CommonMark behavior outside those seams.

This is a recommendation for maintainer review, not an accepted architecture
decision. The first implementation gate should be a small fork feasibility
experiment that proves one block construct, one inline construct, code/fence
shielding, nested containers, and exact UTF-8/CRLF spans before any production
migration.

### Second choice: markdown-it-rust for extension feasibility

`markdown-it-rust` 0.6.1 is the fallback when avoiding a fork is more important
than current maintenance and GFM confidence. Its public extension model is
the only one that directly proves downstream custom block/inline rules,
custom nodes, and precedence ordering. It is an excellent extension-prototype
substrate and could be reconsidered if upstream maintenance resumes or if the
maintainer explicitly accepts the current maintenance/GFM risk.

`pulldown-cmark` is a strong standards oracle and possible `.md` parser when a
streaming event adapter is sufficient, but it is not the primary choice for
Quarkdown’s custom block lifecycle. `comrak` is a strong complete Markdown
tree parser, but its arena/source-position model and lack of parser extension
hooks make it a less suitable substrate for this integration.

### Fork synchronization model

If a markdown-rs fork is approved, the intended maintenance flow is:

```text
upstream parser release/tag
        |
        v
automated version and change detection
        |
        v
fetch/rebase candidate fork
        |
        v
Markdown conformance suite + Quarkdown integration corpus
        |
        +--> clean: adaptation PR for review
        |
        `--> conflict or regression: explicit maintainer review
```

The automation is not implemented in this spike. It is viable because the
proposed delta is localized, but the conformance and Quarkdown corpus must be
mandatory gates rather than relying on a clean textual rebase.

## ADR-0014 impact

ADR-0014 remains unchanged. The following classification is the impact of this
evidence if a substrate-based architecture is later accepted.

### Keep

* `scribium-markdown` remains the Markdown frontend/document-context owner and
  owns adaptation into Scribium’s frontend AST/IR boundary.
* `scribium-quarkdown` remains grammar-focused and must not depend on the
  external Markdown AST/parser implementation.
* Dependency direction remains `scribium-markdown -> scribium-quarkdown`.
* Original source identity, exact source spans, and source segments remain
  required; normalized parser payloads cannot replace raw source provenance.
* No synthetic recursive Markdown reparsing or transformed-Markdown workaround.
* Markdown/container context has one effective owner; Quarkdown must not grow a
  duplicate container state machine.
* Behavior-preserving migration and existing engine/IR/backend ownership
  constraints remain in force.

### Supersede if the substrate succeeds

The following ADR-0014 implementation decisions would be candidates for
explicit supersession or narrowing, not silent reinterpretation:

* Scribium-owned physical-line scanner;
* Scribium-owned `LineView`;
* Scribium-owned authoritative Markdown `BlockParser`;
* Scribium-owned standard Markdown block recognizers;
* Scribium-owned standard Markdown inline parser;
* custom continuation, interruption, fence, and standard container machinery
  that the selected substrate already owns correctly.

The external parser would replace standard Markdown machinery only. It would
not automatically take ownership of Quarkdown grammar, semantic evaluation,
compatibility policy, source identity, or backend/IR layers.

### Unresolved and requiring new wording

* Exact production crate boundary and whether the adapter is inside
  `scribium-markdown` or split into a separate platform-neutral layer.
* The frontend AST/source-segment mapping from the selected parser’s node or
  event model.
* Quarkdown block-body ownership, especially lazy continuation and nested
  list/blockquote cases.
* Whether front matter remains framed by Scribium or is delegated for each
  selected substrate.
* Raw HTML policy at the existing HTML interoperability boundary.
* The fork-versus-upstream-extension decision and its maintenance authority.
* Diagnostic and recovery mapping for parser errors and normalized payloads.

These items require maintainer architecture review. No superseding ADR was
created by this spike.

## High-level migration sketch for later review

This is sequencing guidance only, not work performed here:

1. Select a substrate and approve the extension/fork boundary.
2. Prove exact source mapping and Quarkdown precedence in an isolated adapter
   against the full fixture corpus.
3. Build a Markdown frontend adapter that emits the existing backend-neutral
   frontend/IR contracts without changing semantic ownership.
4. Run behavior-preserving differential and compatibility tests against the
   current implementation.
5. Migrate `.md` first, then reuse the same substrate for `.qd` with only the
   approved Quarkdown delta.
6. Remove superseded Markdown machinery only after review and regression
   evidence; `.typ` passthrough remains unchanged.

## PoC files and exact commands

The PoC is intentionally outside the root Cargo workspace. It has exact
dependency versions in its own `Cargo.toml` and generated `Cargo.lock`.

```text
cargo fmt --manifest-path tools/spikes/markdown-substrate/Cargo.toml
cargo fmt --manifest-path tools/spikes/markdown-substrate/Cargo.toml -- --check
cargo clippy --manifest-path tools/spikes/markdown-substrate/Cargo.toml -- \
  -D warnings
cargo run --manifest-path tools/spikes/markdown-substrate/Cargo.toml
cargo check --manifest-path tools/spikes/markdown-substrate/Cargo.toml \
  --target wasm32-unknown-unknown
```

All five commands completed successfully. The PoC covered:

* UTF-8 byte spans for strong text;
* CRLF headings, paragraphs, links, lists, blockquotes, inline code, and
  fenced code;
* nested lists and blockquotes, list/blockquote crossing, lazy continuation,
  raw HTML, front matter, emphasis adjacent to calls, link text containing
  call-looking text, and nested call-looking bodies;
* custom markdown-it-rust block and inline rules, including custom block nodes
  at root, list-item, and blockquote levels;
* exact custom-node byte spans and access to link/image nesting state;
* inline policy interactions with emphasis, strong, links, images, escapes,
  entities, raw HTML, inline code, and fenced code;
* code/fence, escape, and entity shielding from the literal-dot custom rule;
* custom-node/source-map output and standard-node output for all candidates.

The PoC is evidence for API shape and provenance behavior, not a complete
Quarkdown implementation or a production conformance suite.

## Stop boundary

This spike stops at research, disposable PoC, report, recommendation, and
validation. It does not replace the production Markdown parser, delete
`BlockParser`, migrate crates, add Quarkdown features, promote compatibility
baselines, modify Typst passthrough, or implement upstream synchronization.
