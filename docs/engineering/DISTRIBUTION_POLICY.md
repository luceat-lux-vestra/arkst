# Distribution policy

## Current decision

As of the `0cfb420dc8687ed0b61cb7e35318953e8737f7f3` baseline, Arkst has no
current public distribution contract. The repository is experimental, has no
release workflow, has no GitHub Releases or tags, and none of the 16 Cargo
workspace packages has been published under its workspace name on crates.io.
The Cargo package versions and metadata are development state, not a release
promise.

The canonical, machine-readable inventory is
[`../../.github/distribution-policy.toml`](../../.github/distribution-policy.toml).
Its `[[packages]]` entries are the complete crate-by-crate decision. Do not
maintain a second package inventory in this document.

The inventory covers the 14 `crates/*` packages and the two workspace tools,
`arkst-markdown-compat` and `arkst-upstream-watch`. The package set is
discovered from Cargo metadata; it is not a hand-maintained 14-package list.

## Artifact decisions

- Every workspace package is currently non-publishable. Compiler libraries
  are internal toolchain components, `arkst-test-support` is test-only,
  and the two compatibility/release-observer tools are repository tooling.
  Each manifest explicitly sets `publish = false`.
- `arkst-cli` and its `arkst` binary are useful from a repository
  checkout, but crates.io publication and a public `cargo install` contract
  are not currently intended. GitHub binary releases are also not currently
  intended.
- The `wasm32-unknown-unknown` check for `arkst-core` and `arkst-typst`
  is a buildability invariant. It is not a distributed WASM artifact, npm
  package, `wasm-bindgen` output, `wasm-pack` package, or browser bundle.
  WASM bindings remain future roadmap work.
- Internal compatibility and upstream-release observer tooling is not a
  public package or release artifact.

The README's `cargo install` wording describes the feature set of a Cargo
install/build from the checkout context; it is not a registry publication or
installation guarantee.

## Enforcement

`tools/ci/verify_distribution_policy.py` runs Cargo metadata and checks:

1. the exact workspace package set, manifest paths, versions, target-to-binary
   relationships, and workspace dependencies against the canonical inventory;
2. every package's actual Cargo `publish` metadata is `[]`, which is Cargo's
   resolved non-publishable form for `publish = false`; and
3. the CLI, crates.io, GitHub binary, WASM, and internal-tool classifications
   are known, explicit, and consistent.

The verifier's tests exercise the same production validation path with
mutations for accidental publication, new-package drift, stale entries,
policy/Cargo disagreement, omitted internal tools, and malformed artifact
classification. It runs inside the existing required `fmt` gate, so this
policy does not create another required status context.

## Enabling distribution later

A future distribution proposal must deliberately update the canonical policy,
the affected manifest's explicit Cargo publication setting, and the relevant
artifact/channel classification in one reviewable change. The package set,
manifest path, target relationship, version, and publish semantics must still
pass the verifier. The review must record the distribution decision and its
evidence; a forgotten `publish` field must never be enough to open publication.

Only after that decision is accepted should the separately scoped package,
dry-run, release-tag, or release-artifact work in #234/#235 be considered.
This issue does not add release workflows, publication, tags, GitHub Releases,
package dry-runs, SBOMs, or attestations.
