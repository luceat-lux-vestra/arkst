# Generated/reference-data integrity contract

Scribium's authoritative generated and retained compatibility data is governed
by the machine-readable manifests and verifier listed below. The manifests
remain the only source of target values; this document defines their shared
contract and applicability rules.

- JDK-derived Unicode and locale data: [`reference-jvm.toml`](quarkdown/reference-jvm.toml)
- Markdown/CommonMark/cmark/cmark-gfm data: [`references.toml`](../../tests/compat/references.toml)
- deterministic verifier: [`verify_reference_provenance.py`](../../tools/verify_reference_provenance.py)

Both manifests identify `scribium.reference-data-integrity` contract version
`1`. The verifier fails closed on missing fields, malformed identities, source
or artifact drift, unexpected size changes, missing legal attribution, and
independent-result mismatches. It checks exact artifact bytes and SHA-256
digests; bounded policies are explicit even when the current bound is exact.

## Source kinds

The contract intentionally preserves two valid provenance transports:

| Source kind | Applies to | Required source proof |
|---|---|---|
| `archive-backed` | Eclipse Temurin JDK25-derived data | exact archive URL, filename, byte count, SHA-256, runtime identity, and recorded JDK source/build revisions |
| `immutable-git` | CommonMark, cmark, and cmark-gfm references | HTTPS repository, exact version, full commit, checked-out `HEAD`, source corpus bytes/digest, and required source license files |

An archive SHA-256 is not required for the Markdown repositories. A git commit
is not substituted for the JDK archive proof. When requested, the verifier can
also resolve the JDK source tag and fetch the exact build-source commit in an
isolated temporary git repository.

## Contract layers

Each applicable manifest records:

1. source/vendor/project identity and the source kind;
2. generator, helper, preparation, extraction, and verifier paths with source
   digests where those tools affect output;
3. retained artifact paths, exact digests, structurally meaningful counts, and
   minimum/maximum byte policies;
4. legal identity, required source `LICENSE`/`COPYING` files where applicable,
   and required retained `NOTICE` markers; and
5. an independent semantic-result identity.

The JDK verifier retains the existing archive/runtime/generator checks and
validates the public locale oracle through the independent engine test. The
Markdown verifier checks the pinned checkout, source corpus, license files,
regenerated extracted JSON, and the byte-for-byte corpus comparison. The
differential report is checked by recomputing its case counts and result
classification; a producer-authored success field cannot turn a failed
independent result into a pass.

The exact checked-in artifact set is declared in each manifest's
`exact_artifact_globs`. An unlisted retained artifact in those scoped
directories is a contract failure. Changes to counts, hashes, or bounds require
explicit regeneration and review; the verifier never promotes a new baseline.

## Runtime boundary

All data preparation, archive access, git checkout, corpus extraction, and
semantic oracle execution are development/CI-only. Production Scribium uses
the checked-in Rust data and remains free of JVM, reference-repository,
generation-time filesystem, host-locale, and generation-time network
dependencies. The provenance verifier does not become a compiler runtime
dependency.
