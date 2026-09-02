# Distribution policy

## Current decision

Arkst has an approved first public distribution contract for the `arkst` CLI
binary from Cargo package `arkst-cli` through GitHub Releases. The intended
first release is `v0.1.0`. This policy approves the artifact/channel contract
only; it does not itself add a release workflow, create a tag, or publish a
release.

The canonical, machine-readable inventory is
[`../../.github/distribution-policy.toml`](../../.github/distribution-policy.toml).
Its `[[packages]]` entries remain the complete crate-by-crate decision. Do not
maintain a second package inventory in this document.

The inventory covers all 16 Cargo workspace packages discovered from Cargo
metadata: the 14 `crates/*` packages plus `arkst-markdown-compat` and
`arkst-upstream-watch`.

## Artifact decisions

- `arkst-cli` / binary `arkst` is the sole currently intended public artifact.
  Its approved channel is GitHub Releases.
- Cargo registry publication remains disabled. Every workspace manifest keeps
  `publish = false`; crates.io and public `cargo install` are not part of the
  current contract.
- Compiler libraries remain internal toolchain components, `arkst-test-support`
  remains test-only, and compatibility/upstream-watch tools remain repository
  tooling.
- The `wasm32-unknown-unknown` check for `arkst-core` and `arkst-typst` remains
  a buildability invariant only. No WASM/npm/wasm-bindgen/wasm-pack artifact is
  approved for distribution.

## Enforcement

`tools/ci/verify_distribution_policy.py` runs Cargo metadata and verifies:

1. the exact workspace package set, manifest paths, versions, target identities,
   and workspace dependencies against the canonical inventory;
2. every package's actual Cargo `publish` metadata remains `[]`, which is Cargo's
   resolved non-publishable form for explicit `publish = false`;
3. GitHub Releases is enabled only for `arkst-cli` / `arkst`, while crates.io,
   public `cargo install`, WASM distribution, and internal-tool publication stay
   disabled; and
4. the CLI package entry is classified as `intended` through
   `github-releases`, while all other package classifications remain fail-closed.

The verifier's tests exercise the production validation path with negative
mutations for accidental Cargo publication, workspace inventory drift, stale
entries, GitHub Release contract drift, CLI channel mismatch, omitted internal
tools, and malformed artifact classification. It remains inside the existing
required `fmt` gate, so this change adds no new required status context.

## Next release gates

This contract intentionally does not create release machinery. Before the first
`v0.1.0` release:

1. #235 must establish and verify immutable `refs/tags/v*` protection with no
   bypass;
2. #249 must add the narrowly scoped GitHub Release workflow, native artifact
   matrix, deterministic archive names, and SHA-256 checksums; and
3. the exact release workflow/tag state must be re-read before the first tag is
   created.

#234 remains not planned while crates.io/public `cargo install` are deferred.
Any future registry or WASM distribution proposal must update this canonical
policy and its fail-closed verifier in a separately reviewed change.
