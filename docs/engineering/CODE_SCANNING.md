# Code-scanning authority

Arkst uses one checked-in GitHub CodeQL advanced-setup workflow as its code-scanning authority: `.github/workflows/codeql.yml`.

## Scope

The workflow has two advisory status contexts:

- `codeql (actions)` for GitHub Actions workflow code;
- `codeql (rust)` for the Rust workspace.

Both contexts remain present on every non-draft pull request.

For pull requests:

- Actions analysis runs normally.
- Rust analysis is change-impact gated.
- The Rust job checks out the pull request's trusted base SHA into a dedicated path.
- It executes the base SHA's `tools/ci/codeql_scope.py`, not PR-head classifier code.
- If the classifier reports a Rust-impacting change, Rust CodeQL runs normally.
- If the classifier reports no Rust impact, the `codeql (rust)` context completes successfully without the expensive checkout/init/analyze steps.
- Any classifier/API/trusted-base uncertainty fails closed to a full Rust scan.

The Rust-impact classifier treats Rust source, Cargo manifests/lockfiles, build scripts, `.cargo/**`, Rust toolchain files, CodeQL Rust policy/configuration inputs, and relevant rename/delete cases as Rust-impacting.

Weekly scheduled scans and manual dispatch always run both Actions and Rust analysis.

Both analyses use CodeQL `build-mode: none`.

## Authority and non-duplication

Arkst must have one intentional CodeQL authority. Do not enable or add a second default/advanced CodeQL producer without first reconciling this document, `.github/gate-policy.toml`, and the live repository configuration.

The canonical gate inventory keeps both CodeQL contexts always present and advisory. The hardening audit also requires the trusted-base Rust scope contract, including base-SHA checkout, the checked-in classifier helper, fail-closed scope handling, and the conditional heavy Rust analysis guard.

Deleting or renaming the CodeQL producer, changing its matrix or trusted-scope topology without policy review, or adding another PR-time scanner cannot occur silently: the authoritative `fmt` component fails during merge-gate policy verification until the change is explicitly reconciled.

## Enforcement classification

CodeQL is currently **advisory**, not a required `Protect main` status context. This is deliberate:

- `clippy` remains the Rust lint/correctness gate;
- `license` / `cargo-deny` remains the dependency and supply-chain blocking authority;
- dependency review remains separate diff-scoped dependency evidence;
- CodeQL contributes static security analysis for Rust and GitHub Actions and does not replace those controls.

A conditionally executed analysis must never be promoted into merge authority merely because its status context is always present. Promotion would require separate evidence about analysis coverage, reliability, enforcement semantics, and live ruleset behavior.

## Workflow trust boundary

The checked-in workflow must preserve:

- immutable action commit SHAs with human-readable release comments;
- `persist-credentials: false` on checkout;
- least-privilege default permissions;
- `pull-requests: read` only for trusted change-impact metadata;
- `security-events: write` only where CodeQL result upload requires it;
- bounded job timeout;
- no execution of PR-head classifier code for the Rust scope decision;
- fail-closed fallback to a full Rust scan when trusted-base classification cannot be proven.

Any authority change is repository-hardening work and is reviewed at the exact final PR HEAD before merge.

## Offline Rust CodeQL timing evidence (non-authoritative)

For bounded Stage C performance research, `tools/ci/codeql_rust_log_timing.py` accepts
**one locally saved, successful Rust CodeQL job log**. It does not download logs,
invoke CodeQL, upload data, change scans, or perform security analysis.

```sh
python3 tools/ci/test_codeql_rust_log_timing.py
python3 tools/ci/codeql_rust_log_timing.py /path/to/local-rust-codeql-job.log
```

Its stdout is a fixed, numeric-only JSON summary of extraction, finalize, query
evaluation, upload tail, 37-query completion and the **largest gap between
consecutive query completion log records**. It refuses output with
`INCOMPLETE_DO_NOT_USE` when evidence is incomplete, duplicated, out of order,
has missing success/coverage, or has an unexpected CodeQL version/query pack.
The current frozen acceptance contract is CLI `2.27.2`, Rust pack `0.1.44`,
37 completed queries and 167 successfully extracted Rust files with zero errors.
An intentional CodeQL upgrade requires reviewing this contract, not silently
accepting an incompatible log.

A completion gap is **not isolated query wall time**. Even a reproducible
~2-minute 13th-to-14th completion gap cannot identify a hot QL predicate or
prove the cause; query executions can overlap and share evaluator work.
Do not rank, sum or remove queries using this metric. Compare matching
Rust-source-changing PRs and separate runner queue, Rust extraction, database
finalization, query processing, CodeQL/SARIF upload and the exact-head Gate tail.

The utility is deliberately not a CodeQL JSON profiler: it **does not parse**
the unverified nested `overall-summary.json` schema from oxide-batch #433 P7.
It never prints source paths, query paths, raw event lines, SARIF, credential
strings or arbitrary input. Keep downloaded raw job logs private; do not
commit, upload or attach them to evidence reports. This script is not a
substitute for the trusted Rust CodeQL scan, `Merge Gate`, or independently
validated privacy-safe evaluator telemetry.

Real historical examples (read-only first-attempt job logs):

| PR | Extraction | Finalize | Queries to shutdown | Maximum completion gap |
| --- | ---: | ---: | ---: | ---: |
| #581 | 180.510s | 29.629s | 229.281s | 118.271s after 13/37 |
| #584 | 188.305s | 30.616s | 237.603s | 123.115s after 13/37 |

These are **baseline observations**, not evidence of any optimization gain.
See [Arkst #585](https://github.com/luceat-lux-vestra/arkst/issues/585)
and [oxide-batch #433](https://github.com/luceat-lux-vestra/oxide-batch/issues/433).
