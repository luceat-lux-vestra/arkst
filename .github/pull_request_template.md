<!-- failure-triage:v1:start -->
## Failure remediation

Select exactly one. Required for human-authored PRs.

- [ ] Not remediation for an observed failure
- [ ] Remediation for an observed failure

If this PR is remediation, replace every placeholder. If root cause is still UNKNOWN / UNVERIFIED / INSUFFICIENT EVIDENCE, stop remediation and investigate first.

Observed:
<!-- What failed, where, and on which exact revision/run? -->

Classification:
<!-- Exactly one: implementation defect | test defect | evidence defect | workflow-policy drift | environment failure -->

Basis:
<!-- Why is this responsibility layer proven? Which plausible alternatives were rejected or remain unresolved? -->

Root cause:
<!-- Established cause; UNKNOWN / UNVERIFIED / INSUFFICIENT EVIDENCE / TBD are not remediation states. -->

Remediation:
<!-- Which owning layer changes, and why is this the minimum justified change? -->

Proof:
<!-- What will prove the cause is resolved without weakening tests/evidence/policy? -->
<!-- failure-triage:v1:end -->

## Summary

<!-- One-paragraph summary of the changes -->

## Motivation

<!-- Why is this change needed? -->

## Linked Issue

Closes #

## Design / ADR

<!-- If applicable, link the relevant ADR -->

## Changes

<!-- List of changes with crate names -->

- `arkst-core`: ...
- `arkst-typst`: ...
- `arkst-cli`: ...

## Tests

<!-- What testing was done? -->

- [ ] Unit tests added/updated
- [ ] Snapshot tests added/updated
- [ ] Golden tests added/updated
- [ ] End-to-end test

## Documentation

- [ ] README updated
- [ ] SYNTAX.md updated
- [ ] CHANGELOG updated
- [ ] ADR added/updated

## Compatibility Impact

- [ ] No Quarkdown compatibility change
- [ ] Compatibility profile updated
- [ ] Known divergence documented

## Security Impact

- [ ] No security impact
- [ ] THREAT_MODEL.md updated
- [ ] SECURITY.md updated

## Performance Impact

- [ ] No performance impact expected
- [ ] Benchmark results attached

## Checklist

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- [ ] Relevant tests pass
- [ ] Self-review completed