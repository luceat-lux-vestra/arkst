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

## Current sampled findings

The first pass reviewed the higher-risk production slices #206, #208, #219,
#221, and #434. All five are currently classified
`SOURCE_INFLUENCED_INDEPENDENT`: their compatibility requirements were
source-influenced, but the sampled Rust implementation uses Arkst-specific
scanner, binding-plan, typed-IR, ownership/transaction, or explicit-error
structures rather than an identified literal/source-to-source translation.

The audit also found a concrete fixture-provenance contradiction:
`numeric-transcendental-family` includes short inputs that the repository itself
records as cases observed in upstream `MathFunctionsTest.kt`, while the corpus
README previously asserted that every fixture was independently authored.
Rather than debating copyrightability of short functional expressions, the
historical fixture corpus is now tracked explicitly and must be rechecked or
re-authored before release clearance.

## CI semantics

The required `license` PR context runs the license/provenance verifier in normal
mode. It prevents regression by failing on:

- direct Quarkdown implementation-source references in production, test,
  fixture, or example paths;
- direct Quarkdown dependency/vendor markers in Cargo/submodule surfaces;
- malformed or incomplete audit-ledger coverage;
- an untracked conformance fixture; or
- canonical legal provenance reverting to disproven absolute clean-room claims.

Normal PR mode deliberately permits **tracked historical debt** so unrelated
development is not frozen while #444 is completed.

For release clearance run:

```text
python3 tools/ci/verify_license_provenance.py --release
```

Release mode additionally fails until:

1. historical source-exposed production coverage is `COMPLETE`;
2. `pending_prs` is empty;
3. `POSSIBLE_TRANSLATION` and `LITERAL_COPY` counts are zero; and
4. no fixture remains `REVIEW_REQUIRED`.

This is the finite unblock condition. Git history is preserved; history must
not be rewritten to manufacture a clean-room record.
