# Recurring hardening drift audit

Arkst's hardening controls are enforced by their existing canonical policies and validators. The recurring audit composes those controls; it is not a second policy system and it does not auto-remediate repository state.

## Execution model

`.github/workflows/hardening-drift-audit.yml` runs weekly and supports manual dispatch. Its `detect` job is read-only. It produces one trusted JSON result with one of three classifications:

- `clean`: all continuously readable canonical controls agree with live state;
- `policy-drift`: a canonical control and current state disagree;
- `infrastructure-failure`: an API/readback/tooling failure prevents an authoritative verdict.

Controls that the scheduled low-privilege `GITHUB_TOKEN` cannot read authoritatively are listed under `manual_readback`. They are never silently treated as continuously verified.

The separate `report` job is the only hardening-audit job with `issues: write`. It runs only on `refs/heads/main`, consumes the detector result, and owns exactly one issue marked with:

`<!-- arkst-owned:hardening-drift-audit -->`

A non-clean result creates, updates, or reopens that issue. A later clean result records recovery and closes it. Multiple marker-owned issues are an error rather than an invitation to mutate an arbitrary tracker.

## Composed controls

The detector reuses the repository's existing validators and policies for:

- `.github/gate-policy.toml` and `tools/ci/verify_gate_policy.py`, including the live `Protect main` required-context ruleset;
- `tools/ci/verify_workflow_security.py` for immutable action refs, checkout credentials, permissions, and trust boundaries;
- `.github/distribution-policy.toml` and `tools/ci/verify_distribution_policy.py` for the GitHub Release CLI / no-crates.io / non-distributed-WASM contract;
- `tools/verify_reference_provenance.py` and its tests for generated/reference evidence;
- the upstream Quarkdown ingestion guards;
- the dedicated scheduled supply-chain audit and its owned failure reporter;
- the checked-in CodeQL advanced setup, which must remain exactly one `actions` + `rust` producer and remain advisory unless separately promoted;
- `SECURITY.md`, `CODE_OF_CONDUCT.md`, CODEOWNERS-sensitive paths, and managed label automation.

Live readback also verifies the repository merge policy, the `Protect main` ruleset, and `Protect immutable release tags`. The release-tag ruleset must remain active for `refs/tags/v*`, block deletion and update, and have no observed bypass authority.

## Readback boundary

The audit deliberately does not add a PAT or high-privilege secret. Repository fields and rulesets that the normal workflow token can read are automated. Security settings such as private vulnerability reporting and Dependency Graph are checked when the endpoint is available to that token; an authorization-limited response is recorded as manual coverage rather than fabricated as a pass.

The scheduled `GITHUB_TOKEN` may omit repository merge-policy fields such as allowed merge methods, delete-branch-on-merge, update-branch, and squash-message policy. `tools/ci/hardening_drift_live.py` preserves any values that are actually returned and validates them normally; only omitted merge-policy fields are recorded as explicit `manual_readback` controls. Stable public repository fields such as visibility, default branch, wiki, and discussions remain fail-closed automated checks. This boundary prevents both false infrastructure failures and false automated PASS claims.

Administration-only controls, including the disabled CodeQL default setup and the repository Actions token/fork/allowlist policy, remain explicit manual-readback items unless GitHub exposes an authoritative low-privilege read path. A network failure or malformed response from an endpoint classified as automated is an infrastructure failure, not policy drift and not clean.

## Validation and trust boundary

`tools/ci/test_hardening_drift_audit.py` contains safe negative fixtures for classification, repository settings, release-tag protection, CodeQL authority, supply-chain reporting, security/ownership documentation, and reporter create/update/reopen/recovery/duplicate behavior. `tools/ci/test_hardening_drift_live.py` separately proves the low-privilege repository readback boundary: omitted privilege-sensitive fields become manual obligations, observable wrong values remain drift, and missing stable fields remain fail-closed. The hardening-audit workflow executes both suites, and the existing required `fmt` job continues to execute the canonical hardening audit tests on every pull request.

The detector never executes untrusted pull-request code with write permissions. The reporter's write boundary is main-only, uses `persist-credentials: false`, and has no permission to alter repository settings. Changes to this audit do not add a new required status context; merge authority remains defined by `.github/gate-policy.toml` and the live `Protect main` ruleset.
