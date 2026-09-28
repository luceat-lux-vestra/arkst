# Merge-gate policy

Arkst's canonical PR gate inventory is `.github/gate-policy.toml`. The policy is executable: `tools/ci/verify_gate_policy.py` compares every PR-time workflow job against the inventory, expands matrix job names into exact status contexts, rejects suppression of required producers, and can compare the accepted required-context set with the live `Protect main` ruleset.

The verifier runs inside the already-required `fmt` context. Arkst therefore does not add a second required status context merely to check the first set of contexts; a renamed, removed, newly unclassified, path-filtered, or job-conditioned required producer makes `fmt` fail.

## Required contexts

The accepted required set remains:

- `fmt`
- `clippy`
- `test (ubuntu-latest)`
- `test (macos-latest)`
- `test (windows-latest)`
- `docs`
- `license`
- `wasm`
- `compatibility`
- `msrv`
- `dependency-review`

The machine-readable authority is `.github/gate-policy.toml`. The live ruleset
must remain strict and contain exactly this set. Required contexts must be produced
on every pull request; top-level path filters are rejected for required producers, and job-level conditions are rejected except the exact `${{ always() }}` guard used to keep a required producer materialized after a shared prerequisite fails. A required workflow may leave `pull_request` types implicit, or it may declare exactly `opened`, `reopened`, `synchronize`, and `ready_for_review` so a Draft-to-Ready transition re-runs the complete final gate on the same candidate HEAD. Draft PRs may keep an always-present required context lightweight by deferring expensive internal steps. Ready PRs execute complete authoritative steps unless trusted-base classification proves the separately documented documentation-only scope; in that case required contexts remain materialized while non-applicable heavy work is skipped. The `license` job follows the exact supply-chain fast-path contract in `docs/engineering/SUPPLY_CHAIN.md`.

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

Required status contexts remain always present. The Rust-heavy CI jobs, MSRV,
and Dependency Review emit explicit successful fast-path steps instead of
running product/toolchain work when the trusted-base result is true.
`compatibility` keeps its existing independent fail-closed scope decision, so
compatibility-relevant documentation can still run the full compatibility
campaign. `docs/legal/` is deliberately excluded because legal/provenance
changes must continue through the license/provenance gate.

## Complementary required supply-chain controls

`dependency-review` is required because it provides diff-scoped admission for newly introduced dependency changes, including GitHub Actions references represented by GitHub's dependency graph. The required `license` job remains independently authoritative for the resulting Rust dependency graph through unconditional full-graph `cargo deny check --all-features`. Neither gate substitutes for the other.

## Failure handling

Observed failures still require evidence-backed root-cause classification before remediation. `UNKNOWN`, `UNVERIFIED`, and `INSUFFICIENT EVIDENCE` remain fail-closed engineering states, but Arkst no longer uses PR-body declaration metadata or a sticky classification reporter as merge authority.

## Non-required PR controls

The path-scoped spelling and security-audit jobs, the reference-JVM deep oracle, and PR metadata automation are classified explicitly in the canonical policy. Their presence in the inventory prevents silent job/context drift without promoting conditional or advisory jobs into required checks.

Arkst previously carried a custom advisory AI review workflow backed by GitHub Models. GitHub retired GitHub Models on July 30, 2026, so that dead integration and its review prompt were removed rather than preserved as a nonfunctional governance control. AI review is not part of merge authority; any future replacement requires a separate explicit trust-boundary review before being added to this inventory.

## Compatibility scope

The required `compatibility` context always exists. Its expensive campaign uses the same canonical policy for relevance decisions.

Paths are classified as `run` or `skip`. `run` is evaluated first so compatibility documentation and workflow paths can override broader documentation or `.github` skip classes. Any changed path that matches neither class fails the required `compatibility` job. New path classes therefore require an explicit policy decision instead of silently receiving a green no-op.

The `run` classes deliberately include all crates, tools, compatibility tests/corpora, fixtures, examples, compatibility documentation, and the policy/workflows that govern compatibility. This includes the JDK25 locale/unicode generators, oracle/reference data, `arkst-engine` locale semantics, and related #172/#173 assets.

## Live ruleset evidence

The repository ruleset is named `Protect main` and targets `refs/heads/main`. Besides the exact required-context set, independent admin-level readback must continue to verify deletion and non-fast-forward protection, linear history, squash-only merging, strict required checks, review-thread resolution, extra approval for unattributed changes, and no bypass actors.

A public/read-only ruleset response may omit bypass-actor details by GitHub API design. Absence of that field from a read-only CI response is not evidence that bypass actors do not exist; exact merge-gate review must use an authorized readback when validating that property.