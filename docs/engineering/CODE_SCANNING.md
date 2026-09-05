# Code-scanning authority

Arkst uses one checked-in GitHub CodeQL advanced-setup workflow as its code-scanning authority: `.github/workflows/codeql.yml`.

## Scope

The workflow analyzes exactly two language domains on every pull request and on pushes to `main`:

- `rust` for the Rust workspace;
- `actions` for GitHub Actions workflow code.

Both analyses use CodeQL `build-mode: none`. Rust supports no-build extraction, and GitHub Actions is an interpreted workflow language. The workflow also runs on a weekly schedule and supports manual dispatch.

## Authority and non-duplication

Arkst must have one intentional CodeQL authority. Do not enable or add a second default/advanced CodeQL producer without first reconciling this document, `.github/gate-policy.toml`, and the live repository configuration.

The canonical gate inventory classifies the CodeQL job and pins its exact `language` matrix. Because every pull-request-time workflow job must be represented in `.github/gate-policy.toml`, deleting or renaming the CodeQL producer, changing its matrix without policy review, or adding another PR-time scanning producer cannot occur silently: the already-required `fmt` gate fails during merge-gate policy verification until the change is explicitly reconciled.

## Enforcement classification

CodeQL is currently **advisory**, not a required `Protect main` status context. This is deliberate:

- `clippy` remains the Rust lint/correctness gate;
- `license`/`cargo-deny` remains the dependency and supply-chain blocking authority;
- dependency review remains separate diff-scoped dependency evidence;
- CodeQL contributes static security analysis for Rust and GitHub Actions and does not replace those controls.

Promotion to a required context requires separate evidence that both CodeQL matrix contexts are reliably always present and stable, followed by an explicit live ruleset update and readback. A disappearing, path-scoped, or conditionally produced CodeQL context must never be registered as required.

## Workflow trust boundary

The checked-in workflow must preserve:

- immutable action commit SHAs with human-readable release comments;
- `persist-credentials: false` on checkout;
- least-privilege default permissions;
- `security-events: write` only where CodeQL result upload requires it;
- bounded job timeout;
- no execution of untrusted pull-request scripts as a privileged write-capable step.

Any authority change is repository-hardening work and is reviewed at the exact final PR HEAD before merge.
