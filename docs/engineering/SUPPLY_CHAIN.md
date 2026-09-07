# Supply-chain check authority

Arkst uses complementary dependency and supply-chain controls. They are intentionally not interchangeable: the production workspace is distribution authority, while disposable research Cargo roots are containment-controlled evidence.

## Production PR-time blocking authority

The `license` job in `.github/workflows/ci.yml` is the authoritative merge-time supply-chain gate for the production workspace. It runs on every pull request and executes full `cargo deny check --all-features`, covering advisories, licenses, bans, and dependency sources. It is a required `Protect main` status context and must not be made path-scoped or advisory.

The same required job also runs `advisories` and `sources` checks for `tools/spikes/typst-in-process/Cargo.toml`. That spike is a narrow exception because its checked-in research instructions keep it directly runnable and a previously committed research lockfile resolved the vulnerable pre-fix `citationberg -> quick-xml` line. The spike carries the same immutable `citationberg` patch as the production workspace until #203 removes that temporary security pin.

This extra scan does not make the spike production or distribution input. Production license and ban policy remains owned only by the production workspace.

## Research Cargo containment

`tools/spikes/` contains disposable research recipes rather than adopted dependencies. `tools/ci/verify_cargo_graph_ownership.py` is the fail-closed containment authority for those manifests. It requires that:

- every checked-in research `Cargo.toml` is explicitly registered;
- every research package sets `publish = false`;
- each research Cargo root is isolated either by its own containment-only `[workspace]` or by explicit root `workspace.exclude` ownership;
- research direct registry dependencies use complete exact semantic versions and Git dependencies use immutable 40-hex revisions;
- no `Cargo.lock` is committed below `tools/spikes/`;
- no production workspace member, path dependency, or path-based Cargo `[patch]`/`[replace]` override points into `tools/spikes/`;
- the runnable Typst spike's `citationberg` patch exactly matches the production patch;
- both production and research checks use the reviewed cargo-deny action identity `EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25`; an arbitrary different 40-hex action revision is not equivalent authority;
- the production and research cargo-deny authority steps remain unconditional and fail-hard (no step-level `if` or `continue-on-error`);
- the authority jobs themselves remain unconditional and fail-hard, the PR-time pair stays inside `ci.yml` job `deny` with status name `license`, and the scheduled pair stays inside `security.yml` job `audit`; and
- GitHub Actions cannot execute or package the `tools/spikes` root or descendants, except for exactly one literal cargo-deny `manifest-path` reference to the runnable Typst spike in each of `ci.yml` and `security.yml`.

The verifier and its adversarial self-tests run in the required `fmt` job. Adding an unregistered research manifest, making a spike publishable, committing a research lockfile, weakening a direct pin (including a partial exact pin such as `=0.15`), creating a production path dependency or path override into research, drifting the Typst security patch, changing the reviewed cargo-deny action identity, removing or soft-disabling a cargo-deny authority step or job, moving the Typst audit outside the required `license` job, changing the required status name, adding another cargo-deny authority, duplicating the otherwise allowed research manifest reference, or introducing another workflow reference to the research root or descendants fails closed.

Historical Markdown substrate PoCs deliberately remain research evidence. In particular, the recorded `markdown-it 0.6.1 -> mdurl 0.3.1 -> idna 0.3.0` advisory is not reclassified as safe and is not placed on an advisory allow-list. The manifests preserve complete exact direct-version research recipes while generated lockfiles remain uncommitted, matching the authority recorded in `docs/research/markdown-substrate-final-feasibility.md`.

## Diff-scoped dependency review

`.github/workflows/dependency-review.yml` reviews dependency changes introduced by a pull request. It provides useful diff-specific license and vulnerability evidence, but it is advisory because the production Rust dependency graph is already checked unconditionally by the required `license` job. Its purpose is change attribution, not replacement of the production full-graph gate.

## Post-merge and scheduled detection

The `audit` job in `.github/workflows/security.yml` mirrors merge-time supply-chain detection: it executes full configured cargo-deny checks for the production workspace plus `advisories` and `sources` for the directly runnable Typst research spike. It runs weekly, can be started manually, and runs when Cargo manifests/locks or the security workflow itself change.

The scheduled `audit` job is deliberately not a required pull-request context: the always-present `license` job owns merge blocking.

## Scheduled failure ownership

When a scheduled audit fails, `report-failure` owns one tracker issue titled `ci: scheduled supply-chain audit is failing`. The issue carries `type:bug`, `area:ci`, and `priority:normal` and contains an ownership marker so unrelated issues with a similar title are not mutated. A later failure comments on the existing issue; if the owned issue was closed, it is reopened before the new run is recorded. This keeps recurring failures visible without creating one issue per run.

The reporter has `issues: write` only at the `report-failure` job. The audit job and workflow default remain read-only. A manual `workflow_dispatch` with `force_failure: true` deliberately fails the audit after the real cargo-deny checks so the create/reopen path can be validated without altering dependency policy.

## Policy ownership

`deny.toml` remains the production cargo-deny policy source. Its source restrictions and explicit Git dependency allowlist are not broadened by research containment. #203 continues to own removal of the temporary production `citationberg` patch after a safe upstream release; this containment work does not implement #249 release/signing scope.

Changes to Cargo ownership, the merge gate, scheduled audit, dependency review, research direct pins, or workflow references must preserve the distinction between production distribution authority, explicitly audited runnable research, historical research evidence, post-merge detection, and failure ownership.
