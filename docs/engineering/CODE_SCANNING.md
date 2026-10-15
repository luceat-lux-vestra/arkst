# Code scanning — weekly/manual exact-SHA authority

Arkst retains a single checked-in CodeQL advanced-setup authority: `.github/workflows/codeql.yml`. Normal PR/push events **do not invoke CodeQL**. The existing required `Merge Gate` stays independent; `fmt`, `clippy`, tests, dependency review and cargo-deny continue to gate PR merges. Vulnerabilities introduced between merges may remain undetected until the Thursday 03:23 UTC scan or a deliberate candidate scan; this is an explicit trade-off, not equivalent PR-time detection.

## Scan and evidence contract

- Scheduled (Thursday 03:23 UTC) and manual `workflow_dispatch(candidate_sha)` scans run both **Actions and Rust** with `build-mode: none`; no PR change-impact skip is reintroduced.
- Manual `candidate_sha` must be an exact 40-lowercase-hex commit that is either current protected `main` or an immutable ancestor of current protected `main`. The workflow definition itself always originates from current `main`; arbitrary fork/feature/diverged commits are rejected.
- Checkout pins the exact candidate SHA. Before and after analysis, GitHub compare metadata must prove `base_commit == merge_base == candidate` against current protected `main`.
- Each language retains **two** 90-day evidence artifacts: the unaltered original SARIF and a source-identity manifest binding repository, canonical workflow path/ref/SHA, event SHA, candidate SHA/tree, checkout SHA, run ID/attempt, and language.
- The current SARIF verifier parses the original bytes afresh. It requires SARIF 2.1.0, successful CodeQL invocations, matching language category/query pack, complete expected-vs-extracted source diagnostics, no Rust extraction warnings/tool errors, the reviewed **17 Actions + 17 Rust security rule IDs**, valid result-to-rule references, parseable security severities, and zero High/Critical findings (security severity >= 7.0). Raw SARIF SHA-256 is reported. A descriptor-count threshold is deliberately **not** used as a proxy for executed query coverage.
- Workflow success alone is insufficient. Missing/expired/ambiguous artifacts, missing language jobs, malformed SARIF/manifests, rerun ambiguity, source drift, reduced extraction/security coverage, or High/Critical findings fail closed.
- Check in GitHub Settings that **Default Setup is disabled** to avoid a second scanning authority; administration settings remain a manual control.

## Nonpublishing release-candidate preflight

`.github/workflows/release-codeql-preflight.yml` is a manual **nonpublishing** entrypoint. It grants `actions: write` only to its `preflight` job so `tools/ci/release_codeql_preflight.py` can:

1. prove the immutable candidate is on protected-main history;
2. reuse one eligible exact-candidate completed scan when current verification accepts all evidence;
3. join one already-active exact-candidate scan without dispatching a duplicate;
4. otherwise dispatch the trusted current-main `codeql.yml` with that candidate;
5. wait only a finite time for the selected run; and
6. independently download and verify both source manifests and original Actions/Rust SARIF.

Failed/cancelled scans, run-attempt ambiguity, duplicate active/auto-dispatched runs, API/dispatch/poll failures, selected-run identity drift/disappearance, invalid evidence, or timeout are terminal failures. The preflight has no GitHub Release, registry, signing, or asset-publication authority.

## Release boundary / explicit publication hold

Arkst still has **no managed GitHub Release publisher**. `.github/release-codeql-policy.toml` records that state and `tools/ci/verify_release_codeql_policy.py` rejects recognized repository-managed GitHub Release mutation paths unless a future publisher defines a preceding `codeql-preflight` job and makes every publication mutation depend on it. This is the executable #249 integration contract; it does not create `v0.1.0` or any public release.

Until #249 implements the publisher, **do not publish any GitHub Release via Actions, UI, or API** without operator verification that the intended immutable tag/commit/tree and exact protected-main candidate agree and the candidate preflight passes. Owner/admin UI/API publication can bypass repository workflows, so this remains an explicit governance limitation rather than a cryptographically non-bypassable control.

The future #249 publisher must invoke the same preflight **before the first external mutation** (release creation, asset upload, or draft/public visibility mutation), and must preserve tag/source identity checks. Never mistake a post-publication scan for a release gate.

## Trust boundaries and controls

Actions remain immutable-SHA pinned, checkout does not persist credentials, the scanner alone retains narrowly scoped `security-events: write`, and the nonpublishing preflight alone gains the narrowly scoped `actions: write` required to dispatch/join CodeQL. The merge-gate's trusted-base docs-only and compatibility routing remain fail-closed. Separate `.github/workflows/security.yml` cargo-deny auditing is not a CodeQL replacement. This work does not reopen performance investigation #585.

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
