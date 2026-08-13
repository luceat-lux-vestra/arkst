# Rushdown Safety Gate

Status: focused safety evidence and recommendation only. This document does
not select a Markdown substrate, change an ADR, add a production dependency,
or authorize a parser migration.

Verified: 2026-08-13 (Asia/Seoul)

Baseline: [`markdown-maintained-substrate-check.md`](markdown-maintained-substrate-check.md).
The earlier Rushdown screening findings remain historical evidence; this gate
records the narrower public-safe-API investigation.

## A. Live upstream baseline

The official upstream repository and crates.io were queried again. The latest
stable release is still Rushdown `0.18.0`, so the exact stable release—not an
unreleased development snapshot—is the tested candidate.

| Field | Observed value |
| --- | --- |
| Crate | [`rushdown 0.18.0`](https://crates.io/crates/rushdown/0.18.0) |
| crates.io publish time | `2026-04-30T05:38:05.505064Z` |
| Latest stable release | `v0.18.0`, GitHub published `2026-04-30T05:37:37Z` |
| Latest tag | `v0.18.0` |
| Exact tag SHA | `e5eb4e4446541ea0ed53111c1b37e779283ff57c` |
| Default branch HEAD | `main` at `e5eb4e4446541ea0ed53111c1b37e779283ff57c` |
| Latest meaningful source commit | `e5eb4e4446541ea0ed53111c1b37e779283ff57c`, `refactor!: Emphasis -> {Emphasis, Strong}`, `2026-04-30T05:34:45Z` |
| Repository | [`yuin/rushdown`](https://github.com/yuin/rushdown), MIT, not archived |
| Current relevant open issues | `0` at verification time |
| MSRV | Rust `1.87` |
| Crate checksum | `57358ca9b61ea373ec6bf1c9e916f521f19a919587d9179624d47c40da2ffc5d` |

The release and default branch point to the same commit. No newer stable
release or safety fix to the examined text API was found. The repository is
young and its observed source history is concentrated in one maintainer; that
is a remaining maturity risk, not the factual safety classification below.

Commands used for the live refresh included:

```text
git ls-remote https://github.com/yuin/rushdown.git refs/heads/main refs/tags/v0.18.0 'refs/tags/v0.18.0^{}'
cargo info rushdown@0.18.0
gh release list --repo yuin/rushdown --limit 20 --json tagName,name,isDraft,isPrerelease,isLatest,publishedAt
gh api repos/yuin/rushdown
gh api repos/yuin/rushdown/commits?per_page=10
gh issue list --repo yuin/rushdown --state open --limit 100
```

## B. Public safe API invariant

The exact release source is [`src/text.rs` at `e5eb4e4`](https://github.com/yuin/rushdown/blob/e5eb4e4446541ea0ed53111c1b37e779283ff57c/src/text.rs).

### `Index`

The relevant public signatures are:

```rust
pub fn new(start: usize, stop: usize) -> Self
pub fn with_start(&self, v: usize) -> Index
pub fn with_stop(&self, v: usize) -> Index
pub fn bytes<'a>(&self, source: &'a str) -> &'a [u8]
pub fn str<'a>(&self, source: &'a str) -> &'a str
```

Source trace:

```text
Index::new(start, stop)
  -> stores arbitrary usize start/stop without validation
  -> Index::with_start/with_stop call Index::new with the replacement value
  -> Index::bytes(source)
  -> source.as_bytes()[start..stop]
  -> ordinary slice bounds/order checks; no UTF-8 boundary requirement
  -> invalid bounds/order panic, interior UTF-8 offsets return raw bytes

Index::new(start, stop)
  -> stores arbitrary usize start/stop without validation
  -> Index::str(source)
  -> safe method reaches source.get_unchecked(start..stop)
  -> requires an in-bounds ordered range whose endpoints are UTF-8 boundaries
  -> no constructor, mutator, or type-level invariant enforces that precondition
```

### `Segment`

The relevant public signatures are:

```rust
pub fn new(start: usize, stop: usize) -> Self
pub fn with_start(&self, v: usize) -> Segment
pub fn with_stop(&self, v: usize) -> Segment
pub fn bytes<'a>(&self, source: &'a str) -> Cow<'a, [u8]>
pub fn str<'a>(&self, source: &'a str) -> Cow<'a, str>
```

Source trace:

```text
Segment::new(start, stop)
  -> stores arbitrary usize start/stop with padding=0 and force_newline=false
  -> Segment::with_start/with_stop preserve padding but accept arbitrary values
  -> Segment::bytes(source)
  -> source.as_bytes()[start..stop] in the borrowed/default path
  -> ordinary slice bounds/order checks; no UTF-8 boundary requirement
  -> invalid bounds/order panic, interior UTF-8 offsets return raw bytes

Segment::new(start, stop)
  -> stores arbitrary usize start/stop
  -> Segment::str(source)
  -> safe method reaches source.get_unchecked(start..stop), either as a borrowed
     Cow or while constructing the padded owned result
  -> requires an in-bounds ordered range whose endpoints are UTF-8 boundaries
  -> no constructor, mutator, or type-level invariant enforces that precondition
```

`Segment::bytes` and `Segment::str` have `Cow` return types because padding and
forced-newline handling can produce an owned value. `Segment::new` uses the
borrowed source-slice path, which is the path exercised by the invalid-range
fixture. The safety concern is the same in the owned branch because its source
slice is also obtained through `get_unchecked`.

Therefore a safe caller can create invalid `Index` and `Segment` states and
pass them to a safe `str` accessor. `bytes()` and `str()` must not be conflated:
the former has ordinary slice panic behavior and does not create a `str`; the
latter is a safe-signature wrapper around an unchecked operation.

## C. Safe-only reproduction

The disposable harness is
[`tools/spikes/rushdown-safety-gate`](../../tools/spikes/rushdown-safety-gate/).
It is not a workspace member or production dependency. Its only runtime
dependency is pinned as follows:

```toml
rushdown = { version = "=0.18.0", default-features = false, features = ["std", "html-entities"] }
```

The downstream fixture contains no `unsafe` token or operation. Invalid cases
are behind the `invalid-cases` feature and were invoked only through Miri, not
through a general native executable. Valid tests cover ASCII, full-source
Unicode ranges, Korean, CJK, emoji, combining marks, and empty ranges.

The invalid source is `"한글"`. For each type, the fixture separately tests:

- an interior start boundary (`1`);
- an interior stop boundary (`1`);
- `stop > source.len()`;
- `start > source.len()`; and
- `start > stop`.

The `bytes()` cases verify raw-byte behavior or are expected to panic. The
`str()` cases call the public safe method. Interior-boundary cases then pass
the returned bytes to safe `std::str::from_utf8`; this observes bytes that are
not valid UTF-8 even though the current Miri build does not dynamically flag
the invalid `str` representation.

## D. Miri results

Setup and valid-only command:

```text
cargo +nightly miri setup
cargo +nightly miri test --manifest-path tools/spikes/rushdown-safety-gate/Cargo.toml
```

Result: `PASS`; both valid `Index` and valid `Segment` tests passed. The
invalid cases were run independently with this exact command shape:

```text
cargo +nightly miri test \
  --manifest-path tools/spikes/rushdown-safety-gate/Cargo.toml \
  --features invalid-cases \
  "tests::<case_name>" -- --exact
```

The command used the installed nightly Miri toolchain and returned the
following taxonomy:

| Type | `bytes()` interior boundary | `bytes()` OOB/reversed | `str()` interior boundary | `str()` OOB/reversed |
| --- | --- | --- | --- | --- |
| `Index` | `PASS`: raw bytes, `from_utf8` rejects them | `PANIC`: ordinary slice bounds check | `PASS` from Miri: returned bytes are not UTF-8; Miri did not diagnose the invalid representation | `MIRI_UB`: all three cases reported `str::get_unchecked` precondition violation |
| `Segment` | `PASS`: raw bytes, `from_utf8` rejects them | `PANIC`: ordinary slice bounds check | `PASS` from Miri: returned bytes are not UTF-8; Miri did not diagnose the invalid representation | `MIRI_UB`: all three cases reported `str::get_unchecked` precondition violation |

The out-of-bounds and reversed `str()` output was:

```text
unsafe precondition(s) violated: str::get_unchecked requires that the range is within the string slice
This indicates a bug in the program. This Undefined Behavior check is optional, and cannot be relied on for safety.
```

The interior-boundary `PASS` results are not safety evidence: the upstream
source explicitly omits UTF-8-boundary checking, and the returned value's
bytes fail UTF-8 validation. The dynamic check limitation is recorded rather
than relabeled as `MIRI_UB`.

## E. Parser-produced range validation

```text
NOT RUN — Phase A stop condition.
```

The public safe API unsoundness condition was confirmed before Phase B. The
parser-produced-range question is therefore deliberately not used to offset
or dilute the independent public-constructor result. No claim is made here
that all parser-produced ranges are invalid or valid.

## F. Property/adversarial validation

```text
NOT RUN — Phase A stop condition.
```

The broader parser corpus and arbitrary-valid-UTF-8 property path were not
entered after the current stable release reproduced the public safe-API
hazard. The safe-only Phase A fixture still covers representative UTF-8
boundaries and valid ranges without claiming parser-wide coverage. No fuzz
harness or long-running fuzz infrastructure was added.

## G. Selected-feature unsafe reachability

The production-like feature profile was pinned to:

```toml
rushdown = {
  version = "=0.18.0",
  default-features = false,
  features = ["std", "html-entities"],
}
```

This is a reachability summary, not a total unsafe-site count or a complete
upstream audit:

| Area | Selected parser graph | Invariant/precondition | Evidence and ownership result |
| --- | --- | --- | --- |
| `text` / `Index` | Reachable directly through public API | ordered, in-bounds, UTF-8-boundary range before `str` | `new`/`with_*` do not establish it; safe caller violation reproduced; Miri reports OOB/reversed `get_unchecked` violations |
| `text` / `Segment` | Reachable directly through public API | same range invariant, plus padding/newline branch assumptions | `new`/`with_*` do not establish it; safe caller violation reproduced independently; Miri reports OOB/reversed `get_unchecked` violations |
| parser | Not exhaustively audited after Phase A stop | parser-produced source ranges must satisfy the same invariant | no claim beyond the public API result |
| scanner | Not exhaustively audited after Phase A stop | cursor remains in bounds before unchecked byte reads; unchecked reader input is valid UTF-8 | no direct public safe route was needed to establish the gate failure |
| util | Not exhaustively audited after Phase A stop | unchecked UTF-8 conversion and character decoding require validated byte sequences | no claim of complete proof |
| renderer / optional profile paths | Outside the selected parser profile | not relevant to the direct parser safe-API reproduction | not used to classify the selected parser graph |

The selected graph reaches the text API without requiring a downstream unsafe
operation. The failed ownership boundary is therefore sufficient for this
gate; a larger audit of internal parser/scanner/util sites was intentionally
stopped.

## H. Dependency graph

Exact commands:

```text
cargo tree --manifest-path tools/spikes/rushdown-safety-gate/Cargo.toml
cargo tree -e features --manifest-path tools/spikes/rushdown-safety-gate/Cargo.toml
cargo audit --file tools/spikes/rushdown-safety-gate/Cargo.lock
```

The selected runtime/build graph contains 10 lockfile packages including the
spike package: `rushdown`, `bitflags`, `memchr`, `phf`, `phf_shared`,
`siphasher`, `phf_codegen`, `phf_generator`, and `fastrand`. The feature tree
enabled only `std` and `html-entities` for Rushdown; the spike has no dev
dependency and no additional production dependency.

`cargo audit-audit 0.22.2` scanned the exact spike lockfile against the local
RustSec database and reported no advisories. No advisory allow-list or
security exception was added. This is a selected-graph result; it does not
promote Rushdown's optional renderer/profile or upstream development graph to
runtime evidence.

## I. Remaining risks

- `Index` and `Segment` safe constructors/mutators can create states whose
  safe `str()` accessors violate the `get_unchecked` range and UTF-8
  preconditions.
- `bytes()` panics on bounds/order errors rather than returning an error; that
  is distinct from the `str()` undefined-behavior risk.
- Miri dynamically reports OOB/reversed `str()` precondition violations but
  does not diagnose the interior-boundary invalid-`str` representation in this
  toolchain. The source contract and raw-byte observation still establish the
  violated UTF-8 invariant.
- Parser-produced ranges were not promoted to a compatibility or safety claim;
  Phase B stopped after Phase A confirmation.
- The project is very young, with one observed maintainer and no open issues at
  this snapshot. Zero open issues is not evidence that no defects exist.
- No complete unsafe audit, production integration, or fuzz campaign was
  attempted. Those are unnecessary for this gate's rejection recommendation.

## J. Factual safety classification

```text
PUBLIC_SAFE_API_UNSOUNDNESS_CONFIRMED
```

This factual classification is supported separately for both `Index` and
`Segment`:

1. downstream code uses no unsafe operation;
2. public safe constructors and mutators accept arbitrary `usize` positions;
3. public safe `bytes()` behavior demonstrates the distinction between raw
   byte slicing and UTF-8 validation;
4. public safe `str()` reaches `source.get_unchecked(start..stop)`;
5. Miri reports the violated unchecked-range precondition for out-of-bounds
   and reversed ranges; and
6. interior-boundary `str()` calls return bytes rejected by safe UTF-8
   validation, while the upstream documentation explicitly says boundaries
   are not checked.

Simple panic cases are not the basis for this classification. The decisive
dynamic evidence is the Miri-reported `get_unchecked` precondition violation;
the interior-boundary result is recorded as a separate source/API validity
violation rather than falsely calling its Miri result `MIRI_UB`.

## K. Architecture recommendation

```text
RUSHDOWN_REJECTION_RECOMMENDED
```

The same public safe-API hazard reproduces in the latest stable release and
the latest default-branch commit. Under the Phase A stop condition, this is a
sufficient blocker for treating Rushdown as a production parser substrate
candidate pending maintainer/security review. This recommendation does not
select `markdown-it-rust`, does not declare `RUSHDOWN_SELECTED`, and does not
change any architecture decision.

No Rushdown fork, vendoring, local patch, wrapper workaround, production
dependency, parser migration, ADR change, upstream issue, or upstream PR was
created.

## Adoption addendum (2026-08-13)

The historical safety-gate recommendation above records the evidence and
recommendation made before a maintainer architecture decision. The maintainer
has since accepted ADR-0017, which adopts the exact `0.18.0` revision behind a
narrow `scribium-markdown` adapter while the defect is disclosed upstream:

- upstream issue: https://github.com/yuin/rushdown/issues/2;
- selected revision: `e5eb4e4446541ea0ed53111c1b37e779283ff57c`;
- adapter policy: checked bounds/UTF-8 validation and no direct unchecked
  `Index::str`/`Segment::str` call;
- parser-produced range regression: `crates/scribium-markdown/tests/range_invariants.rs`;
- exact dependency and feature policy: `docs/adr/0017-rushdown-markdown-substrate.md`.

This addendum does not delete or rewrite the original factual classification.
The known upstream defect remains an accepted, explicitly tracked dependency
risk and is not represented as fixed.
