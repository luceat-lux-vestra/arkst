# Markdown substrate final feasibility gate

Status: research evidence and recommendation only. This document does not
accept an architecture, supersede ADR-0014, or migrate production code.

Verified: 2026-08-13 (Asia/Seoul)

## A. Scribium live repository state

The repository was refreshed with `git fetch --all --tags --prune` before this
gate. The live state was:

| Item | Observed state |
| --- | --- |
| `origin/main` | `75195ff1ed425f7ae4848fceafddcfc3429c58b0` |
| PR #48 | `MERGED` at 2026-08-12T11:39:15Z; all reported checks successful |
| PR #48 merge commit | `75195ff1ed425f7ae4848fceafddcfc3429c58b0` |
| Prior spike commit | `8f17133de13572616ceda5780aa7579a8441c89c` |
| Prior spike containment | present only on `origin/research/markdown-parser-substrate-spike`; not an ancestor of `origin/main`; GitHub returned no associated PR |
| Task branch | `research/markdown-substrate-final-feasibility` |
| Task base | merge-base `75195ff1ed425f7ae4848fceafddcfc3429c58b0`; branch started at prior spike commit `8f17133...` |

The accepted ADRs, `docs/ARCHITECTURE.md`, public Quarkdown evidence in
`docs/compatibility/quarkdown/SPEC_SOURCES.md`, and engineering constraints
were inspected before the PoC. The representative public syntax is
`.align {center}` as a block call with an indented body and `.text {red}` as an
inline call. The spike does not invent production grammar.

## B. Current upstream state

### markdown-rs

The live official repository is
[`wooorm/markdown-rs`](https://github.com/wooorm/markdown-rs). Direct Git refs,
the GitHub release API, the checked-out source, and `cargo info` agreed on this
snapshot:

| Field | Observed value |
| --- | --- |
| Published crate | `markdown` |
| Stable release/tag | `1.0.0` |
| Stable SHA | `1506572f9b406431402928f3a8b3df0b4ae2d8f5` |
| Release published | 2025-04-23T15:51:29Z |
| Tag commit date | 2025-04-23T15:49:58Z |
| Default branch HEAD | `1506572f9b406431402928f3a8b3df0b4ae2d8f5` |
| Last source commit | 2025-04-23, release `1.0.0` |
| Repository | not archived; 36 open issues at verification |
| License | MIT |
| Rust version | 1.56 |
| Platform | `#![no_std]` plus `alloc`; no parser filesystem, network, or process dependency |

The project claims 100% CommonMark and GFM support and records the basis as
the generated CommonMark corpus, GFM-specific suites, 2300+ tests, coverage,
and fuzzing in its
[`README`](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/readme.md#L14-L25).
The repository includes a generated CommonMark suite and separate suites for
GFM tables, strikethrough, task lists, autolinks, tag filtering, and footnotes.
`ParseOptions::gfm()` enables the fixed GFM construct set. Tables,
strikethrough, task lists, autolinks, fenced code, nested lists, blockquotes,
inline code, and raw HTML nodes are present in the current implementation.

The public representation is a fixed MDAST enum
([source](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/src/mdast.rs#L161-L256)).
Public `unist::Position` values carry one-based line/column and zero-based,
half-open byte offsets
([source](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/src/unist.rs#L7-L61)).
The parser, tokenizer, event, resolver, state, subtokenizer, and construct
modules are private
([source](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/src/lib.rs#L25-L51)).

Current activity is quiet rather than formally abandoned: tag and HEAD are
identical and there have been no source commits since the 1.0.0 release. This
means the current fork sync rehearsal has no post-release drift to test.

### markdown-it-rust comparison baseline

Phase C used the live official
[`markdown-it-rust/markdown-it`](https://github.com/markdown-it-rust/markdown-it)
repository:

| Field | Observed value |
| --- | --- |
| Published crate | `markdown-it` |
| Stable tag | `0.6.1` (no GitHub Release object) |
| Tag and `master` SHA | `2d7c085046a144d221490331b25ca565ecddbb1b` |
| Tag commit date | 2024-07-07T17:51:27Z |
| License | MIT (`LICENSE`; GitHub API itself reports `NOASSERTION`) |
| MSRV | not documented |
| Repository | not archived; last source commit 2024-07-07 |
| Safety | library has `#![forbid(unsafe_code)]` |

The maintenance risk is material. The official tracker has an unanswered
[maintenance-status issue](https://github.com/markdown-it-rust/markdown-it/issues/50),
an open [parser panic](https://github.com/markdown-it-rust/markdown-it/issues/48),
and an open [transitive `idna` advisory issue](https://github.com/markdown-it-rust/markdown-it/issues/47).
A new parsing issue was filed in July 2026, but no source update followed. The
crate's default-feature official suite did pass in this gate: 48 unit tests,
652 CommonMark fixtures, and all observed extras, source-map, renderer, and
pathological groups passed. It offers CommonMark plus selected extras such as
tables and strikethrough, but does not provide markdown-rs's complete built-in
GFM bundle (notably no built-in task-list/footnote equivalent was found).

## C. Phase A — vanilla markdown-rs feasibility

The disposable PoC in
[`tools/spikes/markdown-final-feasibility`](../../tools/spikes/markdown-final-feasibility/)
uses exactly `markdown = "=1.0.0"` without patching upstream. It parses the
actual representative calls at root, in nested containers, around links and
images, in code, under LF/CRLF, and after multibyte UTF-8.

### Public API analysis

`ParseOptions` exposes only the fixed `Constructs` switches, two scalar parsing
options, and MDX payload parsers
([source](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/src/configuration.rs#L1044-L1263)).
The MDX callbacks parse payloads after hard-coded MDX syntax has already been
recognized; they cannot register dot-call syntax. Flow and text precedence are
hard-coded in private dispatch tables
([flow](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/src/construct/flow.rs#L38-L87),
[text](https://github.com/wooorm/markdown-rs/blob/1506572f9b406431402928f3a8b3df0b4ae2d8f5/src/construct/text.rs#L34-L123)).

There is no public API for:

- registering a block or inline construct;
- ordering one against built-in block, code, link, or delimiter rules;
- receiving container, interruption, or lazy-continuation state;
- emitting a custom event or MDAST node; or
- intercepting a recognized construct before subtokenization and fixed MDAST
  compilation.

Vanilla standard nodes preserve original source byte offsets, including the
root's physical CRLF length. That precision does not create a span for an
unrecognized embedded call: `한글 .text {빨강} 끝` is one ordinary text region,
so no exact Quarkdown-node range exists to adapt.

### Phase A blockers

| Blocker | Requirement and lifecycle point | Available public API | Why insufficient | Smallest hypothetical extension |
| --- | --- | --- | --- | --- |
| BLOCKER-1 | recognize a block before paragraph/container commitment | fixed `Constructs` booleans | no custom flow tokenizer or registration | flow recognizer callback plus ordered dispatch slot |
| BLOCKER-2 | recognize inline syntax before text/emphasis/link resolution | fixed built-in text markers | no custom marker, tokenizer, or precedence API | inline recognizer callback plus ordered dispatch slot |
| BLOCKER-3 | use blockquote/list/indent/lazy/interruption state | none; tokenizer/state modules are private | downstream code cannot make the same lifecycle decision as Markdown | narrowly exposed immutable context, with safe nested-body entry |
| BLOCKER-4 | emit exact source-positioned custom nodes | fixed public `mdast::Node`; private events/compiler | a text search cannot add a native parser event/node or recover body ownership | extension event/node representation or a public event-to-adapter boundary |
| BLOCKER-5 | preserve link/image/code context | completed fixed AST only | code is safely opaque, but links/images have already decided and image labels are flattened in MDAST | inline parent/context policy before final AST compilation |

### Phase A verdict

`VANILLA_FEASIBLE` is false. `.md`/`.qd` isolation is trivially true only
because neither mode can install Quarkdown syntax. The required Quarkdown mode
cannot be implemented in the parser lifecycle, so Phase B was required.

## D. Phase B — minimal thin-fork feasibility

A temporary branch based exactly on `1.0.0` added the smallest useful generic
lower-bound hook: two function-pointer recognizers, one flow dispatch, one text
dispatch, and four internal states. Quarkdown recognition remained in the
external harness. To keep the lower bound small, the patch reused MDX nodes;
that is not an acceptable production AST design and deliberately understates
the complete patch.

### Measured patch surface

Baseline: `1506572f9b406431402928f3a8b3df0b4ae2d8f5`

Temporary patch commit: `a2af95910bc8bdf77d2edd4ab5fb8f5fcf4e9946`

| Metric | Result |
| --- | --- |
| Files modified | 6 |
| Files added | 2 (one construct, one test) |
| Insertions/deletions | 286 / 2 |
| Existing parser-core files modified | `src/construct/flow.rs`, `src/construct/text.rs`, `src/construct/mod.rs`, `src/state.rs` |
| Tokenizer files modified | 0, but the new construct directly depends on private `Tokenizer` fields and scratch state |
| Resolver/subtokenizer files modified | 0 in the incomplete lower bound |
| AST/compiler files modified | 0 only because MDX nodes were reused; a real custom node would require them |
| Dispatch sites modified | flow dispatch, text marker/dispatch, central state dispatch |
| Public API additions | `ExtensionInput`, `ExtensionParse`, and two `ParseOptions` fields |

Exact numstat:

```text
38   0  src/configuration.rs
96   0  src/construct/extension.rs
7    0  src/construct/flow.rs
1    0  src/construct/mod.rs
9    1  src/construct/text.rs
3    1  src/lib.rs
10   0  src/state.rs
122  0  tests/final_gate.rs
```

### Ownership surface

| Class | Files | Assessment |
| --- | --- | --- |
| A. Stable extension surface | `configuration.rs`, `lib.rs` | small in concept, but callback contract would become long-lived public API |
| B. Frequently changing parser internals | `flow.rs`, `text.rs`, `state.rs`, new construct using `Tokenizer` internals | unavoidable even for the lower bound; precedence and state-machine coupling are semantic, not mechanical |
| C. Scribium-specific logic | external harness/test | grammar remained outside parser core as intended |

The lower bound recognized calls at root and inside lists, nested lists,
blockquotes, list-to-blockquote, and blockquote-to-list. Code spans, indented
code, and fenced code remained opaque. Recognized flat tokens had exact UTF-8
and CRLF byte spans, and disabling callbacks left Markdown behavior unchanged.

It did not satisfy the gate:

- an indented Quarkdown body remained a separate Markdown code node rather
  than a child of the custom block;
- image label compilation flattened the custom inline node;
- a real custom AST node would additionally touch `event.rs`, `mdast.rs`, and
  `to_mdast.rs`; and
- attaching and recursively parsing a body with exact physical mappings would
  require document/container or subtokenization changes.

Those are the requirements for which the fork was proposed, not optional
polish. Extending the patch until they pass would move ownership into the
container lifecycle, event model, AST compiler, and subtokenizer. Therefore
the measured patch is a lower bound that fails, while the expected complete
patch is no longer a minimal extension surface.

### Upstream regression and sync rehearsal

The lower-bound library passed `cargo check --all-features --workspace`,
`cargo fmt --all --check`, a downstream harness run, and
`cargo check --target wasm32-unknown-unknown -p markdown --lib`.

The current upstream full test command cannot reach parser tests with today's
resolved dev dependencies: `swc_common 8.1.1` imports the removed
`serde::__private` module from `serde 1.0.229`. Upstream clippy also fails on
new Rust 1.97 lints in unchanged source. These same environmental failures are
not attributed to the fork, but they prevent a clean full-conformance
regression result. The downstream no-dev-dependency harness passed.

Stable tag and current upstream HEAD are the same SHA. The temporary patch was
fetched into a fresh HEAD worktree and cherry-picked with zero conflicts
(`631eb445901cc951807f574f3896e701412f7b77`). This is a valid zero-conflict
rehearsal but weak evidence for future upgrades because no later upstream
commit exists. Since 2024, touched-file commit counts were: configuration 7,
flow 1, text 1, state 5, and lib 6. The likely future conflicts in state,
dispatch, AST, and subtokenization would be semantic.

### Phase B verdict

`THIN_FORK_NOT_RECOMMENDED`. The small patch does not meet correctness; the
patch that likely would meet correctness crosses too many private parser
layers to qualify as a low-ownership thin fork.

## E. Phase C — alternative substrate validation

Because both previous routes failed, the same disposable PoC registered
ordinary downstream `BlockRule` and `InlineRule` implementations against
unmodified `markdown-it 0.6.1`.

This API is genuinely public, not a renderer plugin:

- public block and inline rule traits return custom nodes
  ([block](https://github.com/markdown-it-rust/markdown-it/blob/2d7c085046a144d221490331b25ca565ecddbb1b/src/parser/block/rule.rs#L4-L13),
  [inline](https://github.com/markdown-it-rust/markdown-it/blob/2d7c085046a144d221490331b25ca565ecddbb1b/src/parser/inline/rule.rs#L4-L15));
- rule registration and order are public;
- `BlockState` exposes the current adjusted line offsets, `blk_indent`, list
  state, source, and current node
  ([source](https://github.com/markdown-it-rust/markdown-it/blob/2d7c085046a144d221490331b25ca565ecddbb1b/src/parser/block/state.rs#L8-L45));
- nested body parsing can reuse the same block tokenizer, which assigns source
  maps against the original state
  ([source](https://github.com/markdown-it-rust/markdown-it/blob/2d7c085046a144d221490331b25ca565ecddbb1b/src/parser/block/mod.rs#L40-L117)); and
- `InlineState` exposes absolute source mapping and link nesting state
  ([source](https://github.com/markdown-it-rust/markdown-it/blob/2d7c085046a144d221490331b25ca565ecddbb1b/src/parser/inline/state.rs#L25-L58)).

The PoC passed the representative matrix without patching the crate. Eight
standalone calls became custom blocks (six container/body cases plus two
paragraph-interruption cases), six indented bodies were parsed by the same
Markdown block parser, and three inline calls became custom nodes in normal,
link, and image-label contexts. Strong Markdown inside a Quarkdown body was
retained. Code span/fence/indented-code shielding passed. A markerless lazy
blockquote continuation stayed in the quote and the following custom block
correctly terminated it.

The custom node stores the call's original byte range while its `srcmap`
covers the complete call/body. For both LF and CRLF, slicing the original
source by the inline node span produced `.text {빨강}`, beginning after the
multibyte prefix at byte offset `"한글 ".len()`. No normalization or generated
Markdown was reparsed. Registering no rules produced Markdown-only behavior,
proving explicit `.md`/`.qd` isolation.

The disposable crate passed
`cargo check --target wasm32-unknown-unknown` with both parsers and
`markdown-it` default features disabled. The library uses `std` but the tested
WASM target compiled; the parser library has no required filesystem, process,
or network operation. Its optional CLI has filesystem calls and is not part of
the compiler dependency.

Technical extensibility is therefore demonstrated. Upstream viability is not:
the stale release, unanswered maintenance question, open panic, open advisory,
and incomplete GFM bundle prevent treating this as an unqualified mature
replacement.

## F. Required fixture matrix

`pass` means the route both recognized the custom construct and retained the
expected Markdown lifecycle. `standard only` means Markdown itself parsed
correctly but no Quarkdown integration was possible.

| Context | Vanilla markdown-rs | Lower-bound fork | Vanilla markdown-it |
| --- | --- | --- | --- |
| root block | standard only | pass, flat token | pass |
| blockquote | standard only | pass, flat token | pass |
| list item | standard only | pass, flat token | pass |
| nested list | standard only | pass, flat token | pass |
| list to blockquote | standard only | pass, flat token | pass |
| blockquote to list | standard only | pass, flat token | pass |
| lazy continuation boundary | no extension lifecycle | partial | pass |
| paragraph interruption | no extension lifecycle | flat interruption | pass |
| fenced code shielding | standard code | pass | pass |
| indented code shielding | standard code | pass | pass |
| inline code shielding | standard code | pass | pass |
| emphasis adjacency | text only | flat token | pass |
| link interaction | text only | custom token retained | pass |
| image interaction | flattened/no custom node | **fail** | pass |
| indented Quarkdown body | Markdown code/paragraph | **fail: detached** | pass with nested standard parser |
| UTF-8 exact custom span | no custom span | pass for flat token | pass |
| CRLF exact custom span | no custom span | pass for flat token | pass |
| extension disabled | identical Markdown | pass | pass |
| WASM target | pass | pass | pass with default features disabled |

## G. Cost model

| Criterion | vanilla markdown-rs | thin-fork markdown-rs | alternative: markdown-it-rust |
| --- | --- | --- | --- |
| CommonMark/GFM quality | excellent, complete documented suites | inherits upstream only while patch stays clean | strong CommonMark; partial GFM bundle |
| Quarkdown block integration | unavailable | incomplete unless container/subtokenizer scope expands | public downstream rule; PoC pass |
| Quarkdown inline integration | unavailable | incomplete around final AST/image compilation | public downstream rule; PoC pass |
| precedence control | unavailable | hard-coded dispatch must be patched | ordered public ruler |
| nested container correctness | Markdown only | flat recognition works; body ownership fails | PoC pass using adjusted `BlockState` |
| exact provenance | excellent for standard nodes; no custom span | flat tokens pass; full-body design unproved | PoC pass for call and body |
| `.md` isolation | yes, but no `.qd` feature | callback opt-in pass | rule-registration opt-in pass |
| upstream conformance inheritance | direct | conditional on every rebase | direct for unmodified crate |
| upstream updates | currently quiet | rebase parser-core patch | no updates since 2024 |
| fork ownership | none | medium/high for a complete solution | none for parser; possible dependency-risk mitigation would reintroduce ownership |
| maintenance burden | low but requirement fails | high relative to the intended thin hook | low integration burden, high upstream viability risk |
| WASM suitability | excellent, `no_std` | unchanged in lower bound | tested pass; `std`, default features disabled |
| architecture risk | correctness blocker | parser-internal ownership expansion | maintenance/security/GFM risk |

The priority ordering was applied as a constraint, not a score. Vanilla
markdown-rs is eliminated because it cannot meet correctness. The measured
thin fork is eliminated because the small version fails correctness and the
complete version would own broad private internals. Vanilla markdown-it-rust
meets the integration mechanics, but its current upstream state does not meet
the intended “mature, maintained substrate” premise without an explicit risk
decision.

## H. Validation

Repository checks:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo test --workspace --all-features` | pass |
| `cargo test -p scribium-core` | pass, 335 tests |
| `git diff --check` | pass |
| `cargo run -p scribium-cli -- build examples/hello/main.qd` | existing `E3001`: example condition at bytes 247..356 is not boolean-compatible |
| `cargo run -p scribium-cli -- build examples/hello/main.qd --format pdf` | same existing `E3001`; PDF compiler not reached |
| `cargo run -p scribium-cli -- inspect examples/hello/main.qd --emit typst` | exits with the same one-error compilation result |

The spike changes no production crate or example; those paths are byte-for-byte
identical to `origin/main`, so the example validation failure is not introduced
by this research branch.

Disposable PoC checks, run from
`tools/spikes/markdown-final-feasibility`:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo run --quiet` | pass; both Phase A blocker and Phase C success assertions executed |
| `cargo check --target wasm32-unknown-unknown` | pass |

Official `markdown-it-rust` `cargo test --tests` passed at exact tag `0.6.1`,
including 652 CommonMark fixtures. The exact markdown-rs upstream and fork
commands and their current dependency/toolchain blockers are recorded in
Phase D rather than reported as passes.

## I. Final recommendation

```text
ARCHITECTURE_UNRESOLVED
```

The gate disproves vanilla markdown-rs and does not support recommending a
markdown-rs thin fork. It proves that unmodified markdown-it-rust has the right
extension and provenance model, making it the technical front-runner, but its
maintenance, open panic/advisory, and GFM-completeness risks are too material
to recommend it as the production substrate without architecture review.

The remaining architecture questions are:

1. Is Scribium willing to accept or actively sponsor the upstream viability
   risk of markdown-it-rust, including a plan for the open panic and advisory?
2. Would markdown-rs upstream accept a public generic extension surface broad
   enough for body/container and inline context, avoiding a Scribium fork?
3. If neither is acceptable, should a narrowly scoped follow-up inspect one
   additional actively maintained extensible candidate rather than adopt a
   heavy fork or continue the custom CommonMark parser?

No architecture decision was made in this spike.
