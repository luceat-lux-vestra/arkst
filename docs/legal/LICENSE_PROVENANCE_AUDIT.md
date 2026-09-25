# License and Provenance Release Clearance

Issue: #444

This document defines Arkst's finite engineering clearance process for the
historical Quarkdown source-inspection period. It is not a legal opinion and
the CI verifier does not adjudicate copyright infringement.

## What was established on 2026-09-25

Arkst's clean-room policy has prohibited using Quarkdown implementation code as
an implementation reference since the repository bootstrap on 2026-08-02.
Historical v2.5.1 compatibility work nevertheless inspected Quarkdown
implementation and test sources. That is **clean-room policy non-compliance**
and makes the prior absolute clean-room provenance claim inaccurate.

The audit did **not** identify any direct Quarkdown dependency/vendor inclusion,
literal Quarkdown implementation-code copy, or confirmed Kotlin-to-Rust
translation in the sampled production slices. Historical implementation-source
inspection therefore does not by itself establish GPL infringement. The
relevant release question is whether protectable implementation expression was
copied or adapted, not whether source was ever read.

## Classification model

Every historical source-exposed production slice is classified as exactly one
of:

- `FUNCTIONAL_CONTRACT` — only observable/functional compatibility is retained.
- `SOURCE_INFLUENCED_INDEPENDENT` — upstream implementation was historically
  consulted, but the reviewed Arkst expression is independently structured.
- `POSSIBLE_TRANSLATION` — similarity remains too close or insufficiently
  explained; clean rewrite/re-review is required before release.
- `LITERAL_COPY` — copied protected implementation expression was identified;
  remove/rewrite it and obtain focused legal review for historical distribution.

Only the last two classifications are engineering release blockers. Source
inspection alone is not a license-violation verdict.

Fixture provenance is tracked independently as `INDEPENDENT`,
`REVIEW_REQUIRED`, or `REMEDIATED`. A `REVIEW_REQUIRED` fixture is audit debt,
not an infringement finding.

The machine-readable source of truth is
[`../../.github/license-provenance-audit.toml`](../../.github/license-provenance-audit.toml).

## Completed 2026-09-25 audit

The bounded historical production audit is complete. The fail-closed inventory
contains 27 production PRs whose implementation evidence chain included, or is
conservatively treated as including, upstream implementation/test-source
consultation:

`#83, #85, #86, #87, #88, #89, #91, #92, #93, #94, #103, #104, #105,
#117, #129, #130, #136, #138, #140, #142, #144, #146, #206, #208, #219,
#221, #434`.

All 27 are classified `SOURCE_INFLUENCED_INDEPENDENT`. The audit found no
`POSSIBLE_TRANSLATION` and no `LITERAL_COPY`. The reviewed implementation
patterns consistently use Arkst-specific typed IR, evaluator, binding,
ownership/transaction, document-state, resource, or diagnostic structures
rather than an identified source-to-source translation.

The original `numeric-transcendental-family` fixture reused exact
`MathFunctionsTest.kt` expressions that Arkst's own historical records had
identified. Issue #444 replaces those expressions with independently composed
functional probes and marks the case `REMEDIATED`. Other historically
source-adjacent fixtures were individually provenance-reviewed; source-free or
later black-box-clean-room cases are `INDEPENDENT`, and source-adjacent cases
whose current authored inputs were cleared without an identified copied
upstream test input are recorded conservatively as `REMEDIATED`.

The audit inventory was widened from the first-pass 24 PRs to 27 after
fail-closed reconciliation identified #86, #117, and #138 as production work
whose evidence chain also needed classification. Source-heavy documentation or
audit-only PRs with no production semantic implementation are not falsely
counted as production translation candidates.

## CI semantics

The required `license` PR context runs the verifier unit tests, normal PR mode,
and the explicit `--release` mode. After #444 closure, both modes are required
evidence on every PR. The gate prevents regression by failing on:

- direct Quarkdown implementation-source references in production, test,
  fixture, or example paths;
- direct Quarkdown dependency/vendor markers in Cargo/submodule surfaces;
- malformed or incomplete audit-ledger coverage;
- an untracked conformance fixture; or
- canonical legal provenance reverting to disproven absolute clean-room claims.

While coverage is `INCOMPLETE`, normal PR mode permits explicitly tracked
historical debt so unrelated development is not frozen. Once the ledger is
`COMPLETE`, normal PR mode permanently enforces the same zero-pending,
zero-blocker, and zero-`REVIEW_REQUIRED` invariants as release mode. A future
PR therefore cannot silently reopen provenance debt after #444 closes.

The explicit release-clearance command is:

```text
python3 tools/ci/verify_license_provenance.py --release
```

It is also executed by the required PR-time `license` context, so the release
invariant cannot drift separately from ordinary merge authority.

Release mode additionally fails until:

1. historical source-exposed production coverage is `COMPLETE`;
2. `pending_prs` is empty;
3. `POSSIBLE_TRANSLATION` and `LITERAL_COPY` counts are zero; and
4. no fixture remains `REVIEW_REQUIRED`.

This is the finite unblock condition. With the #444 ledger at
`coverage = "COMPLETE"`, the required PR-time `license` context locks that
clear state for future changes. Git history is preserved; history must not be
rewritten to manufacture a clean-room record.
