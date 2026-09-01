# Supply-chain check authority

Scribium uses three complementary dependency and supply-chain checks. They are intentionally not interchangeable.

## PR-time blocking authority

The `license` job in `.github/workflows/ci.yml` is the authoritative merge-time supply-chain gate. It runs on every pull request and executes full-graph `cargo deny check --all-features`, covering advisories, licenses, bans, and dependency sources. It is a required `Protect main` status context and must not be made path-scoped or advisory.

## Diff-scoped dependency review

`.github/workflows/dependency-review.yml` reviews dependency changes introduced by a pull request. It provides useful diff-specific license and vulnerability evidence, but it is advisory because the resulting Rust dependency graph is already checked unconditionally by the required `license` job. Its purpose is change attribution, not replacement of the full-graph gate.

## Post-merge and scheduled detection

The `audit` job in `.github/workflows/security.yml` also executes full-graph `cargo deny check --all-features`. It runs weekly, can be started manually, and runs when Cargo manifests/locks or the security workflow itself change. Its purpose is to detect advisories, license-policy violations, bans, or source-policy violations that become known after a change has already merged.

The scheduled `audit` job is deliberately not a required pull-request context: the always-present `license` job owns merge blocking. Visibility and ownership for failures discovered only by scheduled execution are handled separately by issue #229.

## Policy ownership

`deny.toml` remains the policy source for cargo-deny. In particular, its existing source restrictions and explicit Git dependency allowlist are not broadened by the scheduling policy above. Changes to the merge gate, scheduled audit, dependency review, or `deny.toml` must preserve the distinction between unconditional merge-time authority, diff-scoped evidence, and post-merge detection.
