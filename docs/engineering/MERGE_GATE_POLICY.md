# Merge-gate policy

Arkst's canonical PR gate inventory is `.github/gate-policy.toml`. The policy is executable: `tools/ci/verify_gate_policy.py` compares every PR-time workflow job against the inventory, expands matrix job names into exact status contexts, validates the closed authoritative component graph, and can compare the accepted live required-context set with the `Protect main` ruleset.

The live ruleset requires exactly one GitHub Actions context: `Merge Gate`. The aggregate lives in `.github/workflows/ci.yml`, uses `${{ always() }}`, directly depends on every authoritative internal merge component, and succeeds only when every component result is exactly `success`. The verifier runs inside the internal `fmt` component and rejects missing/renamed/unclassified producers, component dependency drift, unsafe job conditions, or an aggregate that does not fail closed.

## Required context and authoritative components

Live required set:

- `Merge Gate`

Authoritative internal merge components:

- `fmt`
- `clippy`
- `test (ubuntu-24.04-arm)`
- `test (macos-latest)`
- `test (windows-latest)`
- `docs`
- `license`
- `wasm`
- `compatibility`
- `msrv`
- `dependency-review`

The machine-readable authority is `.github/gate-policy.toml`. All merge components are always-present PR producers inside the canonical CI graph. Top-level path filters are rejected for authoritative producers, and job-level conditions are rejected except the exact `${{ always() }}` guard used to keep a component or aggregate materialized after a prerequisite fails. The PR trigger may leave event types implicit, or declare exactly `opened`, `reopened`, `synchronize`, and `ready_for_review`. Draft PRs may keep component work lightweight; Ready PRs execute full authoritative work unless the trusted-base documentation-only classifier proves the separately documented bounded scope.

## Documentation-only fast path

The canonical allowlist lives in `.github/gate-policy.toml` under
`[docs_only_fast_path]`. The allowlist is intentionally closed: a new
repository-root Markdown file is **not** eligible merely because it ends in
`.md`; root files must be named explicitly in `exact_paths`. A pull request
may skip heavy product/toolchain work only when a classifier loaded from the
trusted base revision proves that the complete changed-file set is confined to:

- `README.md`
- `AGENTS.md`
- `CHANGELOG.md`
- `CODE_OF_CONDUCT.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- Markdown files under `docs/`, except `docs/legal/`

The classifier validates the PR-reported changed-file count, every file status,
rename/copy provenance, path normalization, and the trusted-base policy itself.
Missing classifier/policy data, malformed API output, unknown statuses, mixed
scope, duplicate records, count mismatch, or any non-allowlisted path falls
back to full validation. If the shared CI scope job itself fails, every required
CI producer uses an exact `${{ always() }}` job guard; the missing scope output
therefore selects full validation instead of skipping the required job.
Workflow/policy changes cannot authorize their own
fast path because both classifier code and allowlist are loaded from the base
revision and `.github/**` is outside the allowlist.

Authoritative component jobs remain always present, and the sole required
`Merge Gate` remains materialized above them. The Rust-heavy components, MSRV,
and Dependency Review emit explicit successful fast-path steps instead of
running product/toolchain work when the trusted-base result is true.
`compatibility` keeps its existing independent fail-closed scope decision, so
compatibility-relevant documentation can still run the full compatibility
campaign. `docs/legal/` is deliberately excluded because legal/provenance
changes must continue through the license/provenance gate.

## Complementary supply-chain components

`dependency-review` remains an authoritative merge component because it provides diff-scoped admission for newly introduced dependency changes, including GitHub Actions references represented by GitHub's dependency graph. The `license` component remains independently authoritative for the resulting Rust dependency graph through full-graph `cargo deny check --all-features` on substantive changes. `Merge Gate` requires both component results to succeed; neither control substitutes for the other.

## Failure handling

Observed failures still require evidence-backed root-cause classification before remediation. `UNKNOWN`, `UNVERIFIED`, and `INSUFFICIENT EVIDENCE` remain fail-closed engineering states, but Arkst no longer uses PR-body declaration metadata or a sticky classification reporter as merge authority.

## Non-required PR controls

The path-scoped spelling and security-audit jobs, the reference-JVM deep oracle, and PR metadata automation are classified explicitly in the canonical policy. Their presence in the inventory prevents silent job/context drift without promoting conditional or advisory jobs into required checks.

Arkst previously carried a custom advisory AI review workflow backed by GitHub Models. GitHub retired GitHub Models on July 30, 2026, so that dead integration and its review prompt were removed rather than preserved as a nonfunctional governance control. AI review is not part of merge authority; any future replacement requires a separate explicit trust-boundary review before being added to this inventory.

## Compatibility scope

The authoritative `compatibility` component always exists and is included in `Merge Gate`. Its expensive campaign uses the same canonical policy for relevance decisions.

Paths are classified as `run` or `skip`. `run` is evaluated first so compatibility documentation and workflow paths can override broader documentation or `.github` skip classes. Any changed path that matches neither class fails the authoritative `compatibility` component. New path classes therefore require an explicit policy decision instead of silently receiving a green no-op.

The `run` classes deliberately include all crates, tools, compatibility tests/corpora, fixtures, examples, compatibility documentation, and the policy/workflows that govern compatibility. This includes the JDK25 locale/unicode generators, oracle/reference data, `arkst-engine` locale semantics, and related #172/#173 assets.

## Live ruleset evidence

The repository ruleset is named `Protect main` and targets `refs/heads/main`. Besides the exact sole required context `Merge Gate`, independent admin-level readback must continue to verify deletion and non-fast-forward protection, linear history, squash-only merging, strict required checks, review-thread resolution, extra approval for unattributed changes, and no bypass actors.

A public/read-only ruleset response may omit bypass-actor details by GitHub API design. Absence of that field from a read-only CI response is not evidence that bypass actors do not exist; exact merge-gate review must use an authorized readback when validating that property.