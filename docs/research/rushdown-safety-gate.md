# Rushdown safety gate

Status: focused safety evidence and recommendation only. This document does
not select a Markdown substrate, change an ADR, add a production dependency,
or authorize a parser migration.

Verified: 2026-08-13 (Asia/Seoul)

Baseline: [`markdown-maintained-substrate-check.md`](markdown-maintained-substrate-check.md).
The earlier Rushdown safety wording is retained there as historical research;
this document records the narrower safe-API/Miri follow-up.

## A. Live upstream baseline

The official upstream and crates.io were refreshed from `origin/main` at
`eeaa6301fb582d034cb5903ab43d9159340cbbae`. The current stable release is still
Rushdown `0.18.0`:

| Field | Observed value |
| --- | --- |
| Crate | [`rushdown 0.18.0`](https://crates.io/crates/rushdown/0.18.0) |
| Stable tag / tag SHA | `v0.18.0` / `e5eb4e4446541ea0ed53111c1b37e779283ff57c` |
| Default branch HEAD | `main` at `e5eb4e4446541ea0ed53111c1b37e779283ff57c` |
| Release date | 2026-04-30T05:37:37Z |
| Latest meaningful source commit | `e5eb4e4446541ea0ed53111c1b37e779283ff57c`, `refactor!: Emphasis -> {Emphasis, Strong}`, 2026-04-30T05:34:45Z |
| Repository | [`yuin/rushdown`](https://github.com/yuin/rushdown), MIT, not archived |
| Repository dates | created 2026-03-04T01:39:12Z; latest source push 2026-04-30T05:37:36Z |
| Current relevant issues | 0 open issues at verification; this is not proof that no defects exist |
| MSRV | Rust 1.87, as reported by `cargo info rushdown` |

The recent history contains meaningful parser/API changes from March through
April 2026, but the project is still short-lived and the observed source
history is concentrated in one maintainer. No newer stable release or fix to
the examined text API was found.

## B. Public safe API invariant

At the exact release SHA, [`Index`](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/text.rs#L137-L195)
and [`Segment`](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/text.rs#L565-L752)
have safe/public constructors and mutators:

```rust
Index::new(start, stop)
Index::with_start(...)
Index::with_stop(...)
Index::bytes(source)
Index::str(source)

Segment::new(start, stop)
Segment::with_start(...)
Segment::with_stop(...)
Segment::bytes(source)
Segment::str(source)
```

`new` and `with_*` store arbitrary `usize` positions without checking ordering,
source bounds, or UTF-8 boundaries. `bytes` uses ordinary slice indexing and
therefore panics for an invalid range. `str` uses
`source.get_unchecked(start..stop)` and its documentation explicitly says that
UTF-8 boundaries are not checked. The required preconditions are therefore not
owned by the public type or enforced by the safe constructor.

The safety question is not the number of `unsafe` sites. It is whether a safe
caller can manufacture a value that a safe accessor treats as if those
preconditions had been established. The following safe-only fixture answers
that question without using `unsafe` in downstream code.

## C. Safe-only reproduction

The disposable harness is
[`tools/spikes/rushdown-safety-gate`](../../tools/spikes/rushdown-safety-gate/).
Its runtime dependency is pinned to `rushdown = "=0.18.0"` with only
`std` and `html-entities`; `proptest = "=1.11.0"` is dev-only. The invalid
cases are behind the `invalid-cases` fixture feature so the ordinary harness
test suite remains green.

The valid cases cover ASCII, Korean/CJK, emoji, combining marks, variation
selectors, ZWJ sequences, empty ranges, and exact full-source ranges. The
invalid cases construct safe `Index` and `Segment` values for an interior
UTF-8 byte, out-of-bounds start/stop, and reversed ranges, then call only the
public safe `str` accessor. The boundary cases additionally verify that the
returned bytes are not valid UTF-8; Miri does not diagnose that representation
violation itself.

## D. Miri results

Miri setup and the valid-path commands were:

```text
rustup component add --toolchain nightly-aarch64-apple-darwin miri
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri setup
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test --manifest-path Cargo.toml tests::valid_index_cases -- --exact
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test --manifest-path Cargo.toml tests::valid_segment_cases -- --exact
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test --manifest-path Cargo.toml tests::adversarial_corpus_has_valid_parser_ranges -- --exact
PROPTEST_CASES=4 PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test --manifest-path Cargo.toml tests::arbitrary_valid_utf8_has_valid_parser_ranges -- --exact
```

The initial full isolated Miri run was `MIRI_BLOCKED`: proptest's failure
persistence attempted `getcwd`, which is unavailable under Miri isolation. The
rerun disables only Miri process isolation and proptest persistence for this
disposable harness; the library and downstream code remain unchanged.

Each invalid case was run independently with:

```text
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test \
  --manifest-path Cargo.toml --features invalid-cases \
  tests::<case_name> -- --exact
```

| Case | `Index::str` | `Segment::str` | Observation |
| --- | --- | --- | --- |
| valid UTF-8 boundary | `PASS` | `PASS` | valid `&str` and bytes |
| invalid start boundary | `PASS` | `PASS` | returned `&str` bytes fail `from_utf8`; Miri does not report the boundary violation |
| invalid stop boundary | `PASS` | `PASS` | returned `&str` bytes fail `from_utf8`; Miri does not report the boundary violation |
| `stop > source.len()` | `MIRI_UB` | `MIRI_UB` | Miri reports `str::get_unchecked` range precondition violation |
| `start > source.len()` | `MIRI_UB` | `MIRI_UB` | Miri reports `str::get_unchecked` range precondition violation |
| `start > stop` | `MIRI_UB` | `MIRI_UB` | Miri reports `str::get_unchecked` range precondition violation |

The Miri output says “unsafe precondition(s) violated” and aborts the test;
that is recorded as `MIRI_UB`, not as an ordinary recoverable panic. The
boundary cases are still a factual safe-API failure because a safe call
produces a value whose bytes are not UTF-8, even though this Miri version does
not dynamically flag the character-boundary invariant.

## E. Parser-produced range validation

The harness parses every input in its adversarial corpus, walks every AST node,
and checks every source `Segment` exposed by block `TypeData`:

```text
start <= stop
stop <= source.len()
source.is_char_boundary(start)
source.is_char_boundary(stop)
```

It also calls the public source-backed accessors for `Text`, `CodeSpan`, links,
images, reference definitions, raw HTML, code blocks, and HTML blocks, then
invokes each node's public pretty-printer. The ordinary run validated 17
parser-produced ranges across 11 corpus inputs. The Miri adversarial-corpus
run passed the same full traversal.

No parser-produced range in this corpus violated the checked bounds or UTF-8
boundary invariants. This is evidence about ranges generated by Rushdown's
parser; it does not repair the independently reachable public constructors.

## F. Property and adversarial validation

The corpus includes:

- Korean, CJK, emoji, combining marks, variation selectors, ZWJ sequences, and
  Unicode adjacent to delimiters;
- emphasis, strong, links, images, autolinks, inline code, fenced code,
  indented code, blockquotes, nested lists, tables, task lists, and raw HTML;
- LF, CRLF, no final newline, empty input, one-character input, and multibyte
  characters at the beginning and end; and
- Quarkdown-looking strings `.align {center}` and `.text {빨강}` as ordinary
  Markdown input. No Quarkdown grammar was implemented.

The default proptest run passed arbitrary valid UTF-8 sources through parsing,
full AST traversal, source-backed accessor calls, and the same range
assertions. A reduced four-case proptest run also passed under Miri after
disabling Miri isolation and failure persistence.

## G. Selected unsafe reachability

This is a selected parser-profile review, not a total unsafe-site count. The
normal feature graph is `std + html-entities`; `linkify`, `profile`, renderer,
and development-only upstream features were not enabled.

| Area | Reachable? | Unsafe invariant | Established by | Safe caller can violate? |
| --- | --- | --- | --- | --- |
| text/index | yes; direct public API | `Index`/`Segment` ranges must be ordered, in bounds, and on UTF-8 boundaries before `str` | no constructor/type-level validation; parser normally emits valid segments | **yes**, directly reproduced; Miri UB for OOB/reversed and invalid UTF-8 bytes for interior boundaries |
| parser | yes in selected profile | entity/link/auto-link conversions must only convert valid UTF-8 byte slices | parser scanner and preceding byte/UTF-8 checks | no direct safe-caller violation reproduced in this gate; not exhaustively audited |
| scanner | yes; generated scanner is on the parse path | cursor/index positions must remain inside the input and `BasicReader::new_unchecked` must receive valid UTF-8 | scanner state machine and parser-owned source | no direct public safe route identified in this gate; not exhaustively audited |
| util | yes through entity/case-folding paths | unchecked UTF-8 conversion and character decoding require validated byte sequences | parser byte classification and length checks | no direct public safe route identified in this gate; not exhaustively audited |

The selected graph had 9 normal/build external crates (including Rushdown)
and 32 crates in the generated lockfile after adding the dev-only proptest
graph. This table does not claim that every upstream unsafe site is sound.

## H. Dependency graph

Exact commands:

```text
cargo tree --manifest-path tools/spikes/rushdown-safety-gate/Cargo.toml
cargo tree --manifest-path tools/spikes/rushdown-safety-gate/Cargo.toml -e features
cargo audit --file tools/spikes/rushdown-safety-gate/Cargo.lock
```

The selected runtime/build graph contains Rushdown, `bitflags`, `memchr`,
`phf`, `phf_codegen`, `phf_generator`, `phf_shared`, `fastrand`, and
`siphasher`. The dev-only property graph adds `proptest`, `rand`, `getrandom`,
`regex-syntax`, and their test dependencies. `cargo audit` reported no
advisories for the 32-crate spike lockfile on the current local RustSec
database. No advisory allow-list or policy exception was added.

## I. Remaining risks

- The public safe API can manufacture invalid source ranges and the `str`
  accessors do not enforce their documented preconditions.
- Miri reports OOB/reversed `get_unchecked` precondition violations; its lack
  of a boundary diagnostic for interior UTF-8 offsets is not evidence of safety.
- Parser-generated ranges passed the selected corpus and property checks, but
  those checks do not establish a public API invariant for downstream callers.
- The selected graph was audit-clean, while Rushdown remains a very young,
  single-maintainer project with substantial internal unsafe code.
- No full upstream unsafe audit, fuzz campaign, or production integration was
  attempted.

## J. Factual classification and recommendation

```text
PUBLIC_SAFE_API_UNSOUNDNESS_CONFIRMED
```

The classification follows from safe-only construction of invalid `Index` and
`Segment` ranges, Miri-reported `str::get_unchecked` precondition violations
for out-of-bounds and reversed ranges, and the boundary cases returning bytes
that fail UTF-8 validation. This is not a claim that every parser-generated
range is invalid, nor a substitute for a complete unsafe audit.

```text
RUSHDOWN_REJECTION_RECOMMENDED
```

This recommendation is limited to the current Rushdown safety gate. It does
not select markdown-it-rust or make a final parser-substrate decision. No
production dependency, fork, facade, ADR change, upstream issue, or parser
migration was made.
