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

The live ruleset must remain strict and contain exactly this set. Required contexts must be produced on every pull request; top-level path filters and job-level `if` conditions are rejected for required producers.

## Non-required PR controls

`dependency-review` remains advisory rather than required. It is diff-scoped evidence about newly introduced dependency changes. The required `license` job independently runs full-graph `cargo deny check --all-features` on every PR and remains authoritative for the resulting Rust dependency graph. Keeping both controls distinct avoids making a second blocking context mandatory without weakening either check.

The path-scoped spelling and security-audit jobs, the reference-JVM deep oracle, PR metadata automation, and advisory AI review are classified explicitly in the canonical policy. Their presence in the inventory prevents silent job/context drift without promoting conditional or advisory jobs into required checks.

## Compatibility scope

The required `compatibility` context always exists. Its expensive campaign uses the same canonical policy for relevance decisions.

Paths are classified as `run` or `skip`. `run` is evaluated first so compatibility documentation and workflow paths can override broader documentation or `.github` skip classes. Any changed path that matches neither class fails the required `compatibility` job. New path classes therefore require an explicit policy decision instead of silently receiving a green no-op.

The `run` classes deliberately include all crates, tools, compatibility tests/corpora, fixtures, examples, compatibility documentation, and the policy/workflows that govern compatibility. This includes the JDK25 locale/unicode generators, oracle/reference data, `arkst-engine` locale semantics, and related #172/#173 assets.

## Live ruleset evidence

The repository ruleset is named `Protect main` and targets `refs/heads/main`. Besides the exact required-context set, independent admin-level readback must continue to verify deletion and non-fast-forward protection, linear history, squash-only merging, strict required checks, review-thread resolution, extra approval for unattributed changes, and no bypass actors.

A public/read-only ruleset response may omit bypass-actor details by GitHub API design. Absence of that field from a read-only CI response is not evidence that bypass actors do not exist; exact merge-gate review must use an authorized readback when validating that property.
