# Actively maintained extensible Markdown substrate check

Status: follow-up research evidence and recommendation only. This document
does not accept a parser substrate, change ADR-0014, or add a production
dependency.

Verified: 2026-08-13 (Asia/Seoul)

Baseline: [`markdown-substrate-final-feasibility.md`](markdown-substrate-final-feasibility.md).
That report's results for markdown-rs, its lower-bound fork, comrak,
pulldown-cmark, and markdown-it-rust were reused without rerunning their PoCs.

Rushdown's follow-up safety evidence is recorded in
[`rushdown-safety-gate.md`](rushdown-safety-gate.md). This link does not rewrite
the earlier substrate-screening conclusion.

## A. Question and screening method

This gate asked whether a current Rust Markdown library has better upstream
viability than markdown-it-rust while exposing all of the parser lifecycle
needed for Quarkdown: downstream block and inline recognition, controllable
precedence, container-aware nested-body parsing, and exact physical byte
provenance without a parser fork.

Discovery was deliberately narrow. GitHub repository search, crates.io search,
and the official source/API of plausible results were inspected. A result was
discarded before PoC if any mandatory property failed: permissive license,
non-archived and meaningfully active upstream, credible CommonMark baseline,
public downstream block and inline parser rules, ordering control, exact source
bytes, no parser fork, and a platform-neutral/WASM path. Renderer callbacks,
fixed built-in feature flags, completed-AST rewrites, and syntax that requires
editing or regenerating an upstream grammar did not count as parser extension.

The searched universe was:

- the baseline libraries markdown-rs, comrak, pulldown-cmark, and
  markdown-it-rust;
- [`rushdown`](https://github.com/yuin/rushdown), `sparkdown`, `ferromark`,
  `oak-markdown`, `mdx-gen`, `satteri`, `ndg-commonmark`, and
  `markdown-that`; and
- [`tree-sitter-markdown`](https://github.com/tree-sitter-grammars/tree-sitter-markdown)
  as the plausible grammar-based alternative.

This was a targeted substrate search, not a claim that every crate containing
the word “markdown” was enumerated.

## B. Screening results

| Candidate | Current evidence | Result |
| --- | --- | --- |
| `rushdown` | MIT; non-archived; released in 2026; public `BlockParser`, `InlineParser`, registration priorities, nested block lifecycle, and byte `Segment`s | advanced to PoC |
| `sparkdown` | MIT and recent, with claimed CommonMark/GFM coverage and source positions | rejected: public API exposes render/AST functions and fixed option flags; block, inline, and directive parsers remain private, so downstream syntax injection and precedence are unavailable |
| `ferromark` | MIT, recent activity, CommonMark/GFM-focused | rejected: upstream explicitly provides no AST and no source maps; its custom fenced-code callback is rendering, not block/inline grammar injection |
| `oak-markdown` | parser example within `ygg-lang/oaks` | rejected: MPL-2.0 rather than a permissive license, old parser HEAD, incomplete/toy CommonMark surface, and no downstream parser-rule registration |
| `mdx-gen` | Apache-2.0 and active | rejected: parses with comrak first, then rewrites completed `HtmlBlock` nodes; no new block/inline parser lifecycle is exposed |
| `satteri` | MIT and active | rejected: its parser is explicitly `satteri-pulldown-cmark`, a fork, while plugins visit AST/HAST after parsing |
| `ndg-commonmark` | active Nix documentation implementation | rejected: MPL-2.0 and a fixed comrak-based flavor, without generic downstream syntax rules |
| `markdown-that` | `markdown-that 0.7.1`, MIT workspace license, and an explicit fork of markdown-it-rust with its public rule/ruler model retained | `REJECTED_BEFORE_POC`: technically relevant markdown-it-rust fork, but the current source history has no meaningful source activity after 2025-05-18 |
| `tree-sitter-markdown` | MIT and active grammar repository | rejected: upstream cautions against correctness-critical use, custom syntax requires grammar modification/regeneration, and WASM does not work out of the box |

The four baseline libraries were not re-benchmarked. No new material evidence
changed PR #49's conclusions: vanilla markdown-rs is not extensible at the
required lifecycle, its complete fork would own too much parser internals,
comrak lacks the required public parsing surface, pulldown-cmark lacks the
custom block lifecycle, and markdown-it-rust remains technically feasible but
weak in upstream viability.

## C. Rushdown exact upstream state

The only candidate reaching PoC was the official
[`yuin/rushdown`](https://github.com/yuin/rushdown) repository.

| Field | Observed value |
| --- | --- |
| Crate / release | `rushdown 0.18.0`, published 2026-04-30T05:37:37Z |
| Tag and current HEAD | `v0.18.0`, `e5eb4e4446541ea0ed53111c1b37e779283ff57c` |
| Crate checksum | `57358ca9b61ea373ec6bf1c9e916f521f19a919587d9179624d47c40da2ffc5d` |
| License | MIT |
| MSRV | Rust 1.87 |
| Repository | not archived; created 2026-03-04; last core commit 2026-04-30 |
| Maintainers | one observed contributor (`yuin`, 47 commits) |
| Tracker | zero open issues at verification; this is not proof that no defects exist |
| Platform | `std` and `no_std` feature paths; tested `wasm32-unknown-unknown` with the PoC feature set |

The release history is recent and meaningful: 24 tags from `v0.9.0` on
2026-03-04 through `v0.18.0` on 2026-04-30, including parser and extension API
changes. That is materially more recent than markdown-it-rust. It is also only
about five months of history, with one observed maintainer and no core commit
after April, so maintenance durability is not established.

### Parsing and conformance surface

At the exact SHA, rushdown publicly exposes:

- [`BlockParser`](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/parser/mod.rs#L1707-L1769)
  with `open`, `cont`, `close`, paragraph interruption, and indented-line policy;
- [`InlineParser`](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/parser/mod.rs#L1902-L1937);
- public block/inline registration with numeric priorities
  ([source](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/parser/mod.rs#L960-L991)); and
- exact start/stop byte `Segment`s over the original source
  ([source](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/text.rs#L565-L664)).

The exact upstream checkout passed `cargo test --all-features`: 26 unit tests,
2 AST tests, the extension test, 6 GFM test groups, other integration tests,
and 21 doctests. Its CommonMark harness executed the bundled 652-example
CommonMark 0.31.2 corpus. GFM tests cover tables, task lists, strikethrough,
and linkification; the upstream README notes a raw-HTML policy difference from
GFM. This is credible current baseline evidence, not independent Scribium
conformance certification.

### Safety and dependency quality

The exact PoC feature graph is small: rushdown, bitflags, memchr, phf,
phf_codegen, phf_generator, phf_shared, fastrand, and siphasher. All report
MIT/Apache-2.0/Unlicense-compatible terms. `cargo audit` found no advisory in
that 10-package lock resolution on the current local RustSec database.

The upstream all-features/development lock is not equally clean. Its optional
`profile` and development graph produced seven current advisory findings,
including `bytes`, `crossbeam-epoch`, `quick-xml`, and `time`; these packages
are absent from the exact PoC/normal parser graph. They do not prove a runtime
production vulnerability, but they show that upstream's own broad validation
graph needs dependency maintenance.

More importantly, the library does not meet Scribium's default safety
discipline. A source scan found 341 `unsafe { ... }` blocks across 12 `src`
files, heavily concentrated in the generated scanner but also present in text,
parser, utility, and renderer code. Public `Segment::new` accepts arbitrary
byte indexes while the safe `Segment::str` method uses unchecked string slicing
and documents that it does not check UTF-8 boundaries. The same relationship
exists for the public safe `Index::new(start, stop)` and `Index::str(source)`
methods: the constructor represents arbitrary byte positions, while `str`
uses unchecked slicing and explicitly documents that UTF-8 boundaries are not
checked. The concern is therefore not simply that the implementation contains
`unsafe`; a source-slice safety invariant appears to cross a public safe API
boundary. This report does not classify rushdown as unsound, rejected, or
accepted, and no broader unsafe audit is part of this gate. No production
adoption can proceed without the architecture/security exception and safety
proof required by `docs/ENGINEERING.md`.

## D. Disposable PoC

[`tools/spikes/markdown-maintained-substrate-check`](../../tools/spikes/markdown-maintained-substrate-check/)
pins `rushdown = "=0.18.0"`, disables default features, and enables only `std`
and `html-entities`. Its lockfile is ignored because the fixture is research
evidence rather than a production dependency addition. The exact crate
checksum and upstream tag/SHA above preserve the verified input.

The PoC installs ordinary downstream block and inline parsers. The block rule
owns a representative indented body and lets the same Markdown parser parse
that body. The inline rule emits a custom node with the call's exact original
byte segment. Registration priority places the custom rules explicitly among
the built-in rules. Omitting the extension registration gives ordinary
Markdown-only behavior, establishing `.md`/`.qd` isolation without a global
mode or parser fork.

### Fixture matrix

| Requirement | Result |
| --- | --- |
| root block | pass |
| blockquote | pass |
| list | pass |
| nested list | pass; custom block retained two list ancestors |
| list to blockquote | pass |
| blockquote to list | pass |
| paragraph interruption | pass |
| lazy continuation boundary | pass; following custom block stayed outside the blockquote |
| indented body | pass; nested `**Markdown**` became a standard strong node |
| inline emphasis adjacency | pass |
| link context | pass; custom inline retained inside link label |
| image context | pass; custom inline retained inside image label |
| inline code shielding | pass |
| fenced code shielding | pass |
| indented code shielding | pass |
| UTF-8 exact bytes | pass; `.text {빨강}` starts at byte `"한글 ".len()` |
| CRLF exact bytes | pass; identical physical call range without normalization |
| extension disabled | pass; no custom nodes |
| WASM | pass for `wasm32-unknown-unknown` |

Commands and results:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | pass |
| `cargo clippy --all-targets --all-features -- -D warnings` | pass |
| `cargo run` | pass; `RUSHDOWN_EXTENSION_MATRIX_PASS` |
| `cargo check --target wasm32-unknown-unknown --no-default-features` | pass |
| `cargo audit` on the exact PoC resolution | pass; no vulnerability found |

This demonstrates a lower-bound extension lifecycle. It does not constitute a
production adapter, an exhaustive Quarkdown grammar, or a safety acceptance.

## E. Markdown-that screening record

[`markdown-that`](https://github.com/z0ne-dev/markdown-that) was included in the
candidate universe because it is a technically relevant fork of markdown-it-rust
rather than a renderer-only wrapper. The official repository identifies itself
as forked from `markdown-it-rust/markdown-it`; its README retains the public
custom syntax claim, and the current source retains public
[`BlockRule`](https://github.com/z0ne-dev/markdown-that/blob/2f410e3a629219d9f2688b5a67580de725a4e861/crates/markdown-that/src/parser/block/rule.rs),
[`InlineRule`](https://github.com/z0ne-dev/markdown-that/blob/2f410e3a629219d9f2688b5a67580de725a4e861/crates/markdown-that/src/parser/inline/rule.rs),
and ordered
[`Ruler`](https://github.com/z0ne-dev/markdown-that/blob/2f410e3a629219d9f2688b5a67580de725a4e861/crates/markdown-that/src/common/ruler.rs)
APIs. Those parser mechanics are already covered by the markdown-it-rust
evidence in the baseline report, so this candidate did not receive a second
PoC.

The live primary-source snapshot was:

| Field | Observed value |
| --- | --- |
| Crate | [`markdown-that 0.7.1`](https://crates.io/crates/markdown-that/0.7.1), published 2025-05-18 |
| Current HEAD | `2f410e3a629219d9f2688b5a67580de725a4e861` on `master` |
| Repository created | 2025-05-18T09:27:54Z |
| Latest source commit | `2f410e3a629219d9f2688b5a67580de725a4e861`, 2025-05-18T15:02:10Z; pushed 2025-05-18T15:04:46Z |
| License | MIT in the workspace/package metadata |
| Upstream relationship | fork of markdown-it-rust; 13 commits ahead of the `0.6.1` baseline at this snapshot |
| Maintenance evidence | no source commit after 2025-05-18; the later repository metadata update was not source activity |
| WASM/platform | README retains the original browser/WASM demo claim, but current CI only runs stable `cargo test`; no current WASM target check was found. `stacker` is a normal dependency and default features add optional `syntect`, so platform-neutral/WASM suitability is not independently demonstrated |

The README's “updated and maintained crates” wording does not override the
source history. `markdown-that` therefore fails the mandatory meaningful
maintenance gate before PoC:

```text
REJECTED_BEFORE_POC
```

It is a technically relevant markdown-it-rust fork, but it does not satisfy
the current meaningful-maintenance gate. It does not change the rushdown versus
markdown-it-rust comparison or the final recommendation.

## F. Direct comparison

| Criterion                  | rushdown | markdown-it-rust                   |
| -------------------------- | -------- | ---------------------------------- |
| public block extension     | pass | pass                               |
| public inline extension    | pass | pass                               |
| precedence                 | pass | pass                               |
| nested body                | pass | pass                               |
| exact provenance           | pass | pass                               |
| `.md/.qd` isolation        | pass | pass                               |
| CommonMark                 | strong; bundled 652-case suite passed | strong                             |
| GFM                        | strong tested bundle; raw-HTML policy differs | incomplete relative to markdown-rs |
| maintenance                | recent but very new, single-maintainer history | weak                               |
| known correctness issues   | no open issue; public unchecked-slice safety invariant is unresolved | open risk                          |
| security/dependency health | selected graph audit-clean; extensive `unsafe`; broad dev/profile graph has advisories | risk                               |
| WASM                       | tested | tested                             |
| fork required              | no | no                                 |

No score total was used. Rushdown is better on recency and built-in GFM
coverage. Markdown-it-rust has a much longer field history and forbids unsafe
code, but is stale and retains its open panic and transitive advisory risks.
The trade-off is not clearly dominated in either direction.

## G. Final recommendation

```text
ARCHITECTURE_UNRESOLVED
```

An actively developed extensible candidate was found, so
`NO_MAINTAINED_EXTENSIBLE_ALTERNATIVE_FOUND` and an automatic confirmation of
markdown-it-rust are not supported. Rushdown's public parser surface and PoC
results are technically credible, but its short maintenance history,
single-maintainer concentration, broad use of unsafe code, and unresolved safe
source-slice invariant prevent `MAINTAINED_EXTENSIBLE_SUBSTRATE_FOUND` in the
sense of a clearly superior production choice.

The unresolved risks requiring architecture/security review are:

1. whether rushdown's public and internal unsafe invariants can be proved and
   accepted under Scribium's default prohibition;
2. whether its maintenance record and API stability become durable enough for
   a compiler substrate;
3. whether independent CommonMark/GFM and adversarial-input validation confirms
   the upstream claims; and
4. whether markdown-it-rust's stale-upstream/advisory risk is preferable to
   rushdown's safety and maturity risk.

No production dependency, parser migration, abstraction, ADR change, or
architecture decision was made.
