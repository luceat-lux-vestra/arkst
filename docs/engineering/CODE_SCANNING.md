# Code scanning — weekly/manual exact-SHA authority

Arkst retains a single checked-in CodeQL advanced-setup authority: `.github/workflows/codeql.yml`. Normal PR/push events **do not invoke CodeQL**. The existing required `Merge Gate` stays independent; `fmt`, `clippy`, tests, dependency review and cargo-deny continue to gate PR merges. Vulnerabilities introduced between merges may remain undetected until the Thursday 03:23 UTC scan or a deliberate manual prepublication scan; this is an explicit trade-off, not equivalent PR-time detection.

## Scan and evidence contract

- Scheduled (Thursday 03:23 UTC) and manual `workflow_dispatch(candidate_sha)` run both **Actions and Rust** with the previously validated `build-mode: none`; no PR change-impact skip.
- Manual `candidate_sha` must be an exact 40-lowercase-hex commit SHA equal to the workflow event SHA and *current* protected `main` SHA; scans from other refs and stale candidates fail.
- Checkout pins that exact SHA and verifies `git rev-parse HEAD`. Recheck `main` after analysis. This bounds the trusted source; it does not bind any future tag automatically.
- Each language retains the original CodeQL SARIF artifact for 90 days (not just a rendered alert count). The verifier checks SARIF v2.1.0, CodeQL invocation success, matching analysis category, language query pack/rules, result severity, and rejects High/Critical (security severity >= 7.0). It logs the SHA-256 of the raw SARIF bytes and zero-High/Critical verdict, with synthetic negative controls exercised in PR CI.
- Both per-language jobs must succeed and their artifacts and logs must be independently read back for release preflight. Workflow success alone is insufficient. Artifact expiration, missing language, run-attempt uncertainty or altered evidence fails closed.
- Check in GitHub Settings that **Default Setup is disabled** to avoid a second scanning authority; administration settings remain a manual control.

## Release boundary / explicit publication hold

Arkst has **no GitHub Release publisher yet**. Until #249 implements a trusted prepublication guard, **do not publish any GitHub Release via Actions, the UI or API** without operator verification on the exact protected candidate and the selected tag/commit/tree. Before publication: validate the immutable release tag, full Actions/Rust CodeQL run success at that exact SHA, original nonexpired SARIF plus hashes, successful query invocation and category/packs, and zero High/Critical, then document approval. A successful scheduled scan of another SHA does not qualify. Owner/admin UI/API uploads bypass repository workflows; this is an operator hold, **not** cryptographic/non-bypassable enforcement.

The future #249 publishing workflow must block *before* GitHub Release creation or artifact upload, fail closed on SHA/tree/tag mismatch and absent/stale/tampered analysis, and treat reruns/attempts explicitly. Never mistake a `release: released` post-publication scan for a release gate.

## Trust boundaries and controls

Actions remain immutable-SHA pinned, checkout does not persist credentials, CodeQL has narrowly scoped `security-events: write`, and the merge-gate's trusted-base docs-only and compatibility routing retain fail-closed semantics. Separate `.github/workflows/security.yml` cargo-deny auditing remains in place; it is not a CodeQL replacement. This migration removes the obsolete Rust PR CodeQL scope classifier; it does not reopen performance investigation #585.

## Historical Rust CodeQL performance research (#585)

Stage C profiling ended **NO-GO**: no security-equivalent, causally proven
performance intervention was found. Actual additional Rust-source-changing
PR all-validation wall-time improvement demonstrated by this research: **0s**.
The offline log-timing parser and its dedicated synthetic tests were retired
from the repository after their CI invocation was removed in PR #607.
They were diagnostic research artifacts, not CodeQL scanning authorities.

Observed first-attempt Rust-positive CodeQL jobs (not matched experiments):

| PR | Extraction | Finalize | Queries to shutdown | Largest completion-record gap |
| --- | ---: | ---: | ---: | ---: |
| #581 | 180.510s | 29.629s | 229.281s | 118.271s after 13/37 |
| #584 | 188.305s | 30.616s | 237.603s | 123.115s after 13/37 |

The completion gap cannot establish individual query CPU cost or causation.
Do not remove or suppress security queries based on this observation. The
original experiment, measured cohorts, abandoned profiler attempts, privacy
constraints, and reopening criteria remain recorded in
[Arkst #585](https://github.com/luceat-lux-vestra/arkst/issues/585) and
[oxide-batch #433](https://github.com/luceat-lux-vestra/oxide-batch/issues/433).
That completed profiler study is historical; current scan cadence and PR scope classifier
were separately updated by the R2A #105 migration.
