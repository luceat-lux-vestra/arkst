# PROVENANCE — Arkst

This document records the origin and legal provenance of all code, documentation,
and assets in the Arkst repository.

Machine-checkable generated/reference-data integrity is defined by
[`reference-provenance.md`](../compatibility/reference-provenance.md) and
verified by the tool named by each specialized manifest. The manifests remain
the source of exact target values; this document records the legal context.

## Project Origin

- **Repository created:** 2026-08-02
- **Starting point:** Empty repository, no copied or migrated code
- **Source language:** Rust (independently authored)
- **Compatibility targets:** Quarkdown-compatible syntax (independently reimplemented)

## Compatibility provenance and historical source inspection

Arkst's clean-room policy has prohibited using Quarkdown implementation code as
an implementation reference since the repository bootstrap on 2026-08-02.
Historical v2.5.1 compatibility work nevertheless performed
**historical implementation-source inspection**, including review of upstream
implementation and test sources. That is **clean-room policy non-compliance**
and means the former absolute claim that all compatibility work was derived
solely from permitted clean-room evidence was inaccurate.

The 2026-09-25 first-pass audit did not identify direct Quarkdown
dependency/vendor inclusion, literal Quarkdown implementation-code copy, or a
confirmed Kotlin-to-Rust translation in the sampled production slices. Source
inspection alone **does not by itself establish GPL infringement**. Release
clearance therefore distinguishes observable/functional compatibility and
independently expressed source-influenced work from possible translation or
literal copy.

The machine-readable audit ledger is
[`.github/license-provenance-audit.toml`](../../.github/license-provenance-audit.toml),
and the finite clearance procedure is documented in
[`LICENSE_PROVENANCE_AUDIT.md`](LICENSE_PROVENANCE_AUDIT.md). Git history is
preserved; historical records are not rewritten to manufacture a clean-room
history.

The bounded historical production audit completed on 2026-09-25 with 27
source-exposed production PRs classified and no remaining
`POSSIBLE_TRANSLATION` or `LITERAL_COPY` finding. The conformance-fixture
ledger likewise contains no `REVIEW_REQUIRED` entry after the identified
transcendental fixture overlap was remediated. `coverage = "COMPLETE"` is a
permanent CI invariant rather than a one-time release waiver.

Future compatibility implementation remains subject to
[`CLEAN_ROOM_POLICY.md`](CLEAN_ROOM_POLICY.md): use public user-facing
documentation, independently authored specifications/fixtures, official binary
black-box observations, and other explicitly permitted evidence only. Do not
use Quarkdown implementation/test source as a new implementation reference.

## Third-Party Dependencies

See `deny.toml` for the dependency license policy and `THIRD_PARTY_LICENSES.md`
for the complete dependency license list (generated during builds).

## Reference JVM and generated Unicode compatibility data

The active JVM compatibility contract is the exact Eclipse Temurin
`25.0.4.1+1` release described in
[`docs/compatibility/quarkdown/reference-jvm.toml`](../compatibility/quarkdown/reference-jvm.toml).
Its Linux x64 archive is a pinned, generation-time input and is not bundled
with Arkst or required by the runtime. The archive URL, byte size,
SHA-256, source/build revisions, Java output, helper hash, and generated
artifact hash are recorded in that manifest.

`crates/arkst-engine/src/unicode_case.rs` contains independently generated
case-mapping values observed through the public Temurin/OpenJDK `Character`
and `String` APIs. `tools/dump_jdk25_unicode_data.java`, the corpus, and the
Python generator are independently authored Arkst tooling; no OpenJDK or
Quarkdown source or tests were copied. The generated values are compatibility
data, not a redistribution of the JDK implementation. The helper's exact
source hash and transient oracle hash are checked during regeneration.

The JDK distribution licensing and Classpath Exception terms are maintained
by OpenJDK; see the [OpenJDK legal information](https://openjdk.org/legal/)
and [GPLv2 with Classpath Exception](https://openjdk.org/legal/gplv2+ce.html).
The Unicode 16.0-derived case-data provenance and applicable Unicode notice
are covered by the [Unicode copyright and terms](https://www.unicode.org/copyright.html).
These upstream notices apply to the generation sources/data provenance; the
Arkst generator and runtime integration remain Apache-2.0 independently
authored code.

## Reference locale snapshot

`crates/arkst-engine/data/jdk25_locale_display.bin` and the locale records
in `crates/arkst-engine/src/locale_data.rs` are generated compatibility
data, not copied JDK implementation code. Their source is the exact Eclipse
Temurin `25.0.4.1+1` Linux x64 archive pinned in
[`reference-jvm.toml`](../compatibility/quarkdown/reference-jvm.toml):
`OpenJDK25U-jdk_x64_linux_hotspot_25.0.4.1_1.tar.gz`, 141,329,719 bytes,
SHA-256
`dbb698396d478e7fa2b1e50f4103324b2a99b90569ee27c33f2261f9215cf41e`.
The locale provider is JDK25 CLDR, with JDK FALLBACK root values included only
where CLDR has no value. The locale, display, and public differential helpers
are
`tools/dump_jdk25_locale_data.java`,
`tools/dump_jdk25_locale_display_data.java`, and
`tools/dump_jdk25_locale_oracle.java`, whose SHA-256 fingerprints are recorded
in the generated metadata and generator constants. The name-first available
locale order is captured directly from the raw JDK array in
`tools/jdk25_available_locale_order.tsv`, SHA-256
`c4dd6cd7e83919d7236d3040c1ddc60ca21ff92e179b19a7d7d10fda7f9a815e`; the
JDK25 English-name collision audit found zero classes.

The locale snapshot covers available names/tags, display-language/script/
region/variant values, Unicode-extension names, currencies, and accepted
Unicode timezone identifiers. The timezone source is
`java.base/sun/util/cldr/CLDRBaseLocaleDataMetaInfo.java`, SHA-256
`dbddf061210b9086d820c4593c4921698b9d4ef15515fc2ac9a5336c626ce7c2`;
it contains 681 source `tzCanonicalIDMap.put` rows, 622 unique source keys,
and 468 accepted lowercase IDs. The generated logical display oracle has
453,459 rows (SHA-256
`96d43b0ff823a4505bdb69ddd80bfd3056867b2c7c0bc27b6a50fc822c116ab3`) and
the compact artifact has 267,017 records, 6,549,860 bytes, and SHA-256
`d086d29fbb3716624efb0066df9fe09cd6df21438931ed2b0f0ddc17743e68b1`.
The compact format is version 1, uses explicit little-endian numeric fields,
and is parsed as static read-only data without a JVM, filesystem, network,
host-locale database, unsafe code, or runtime decompression. The generator
exhaustively reconstructs every logical oracle row and `--check` compares both
generated artifacts byte-for-byte.

The JDK/CLDR-derived values remain subject to the OpenJDK GPLv2 with Classpath
Exception and upstream CLDR/Unicode notice terms described above. Arkst's
generator, compact-format parser, lookup algorithm, tests, and integration
are independently authored Apache-2.0 code and are not redistributed OpenJDK
implementation material.

## Trademarks

- **Arkst** is the current project name. Preliminary naming due diligence was
  completed on 2026-09-02 and the decision to proceed is recorded in
  [`TRADEMARKS.md`](TRADEMARKS.md). Registry availability must still be
  rechecked immediately before any first publication because registry names
  are not reserved by that earlier review.
- **Scribium** is the retired pre-release working name and is retained only in
  historical records where changing it would falsify the record.
- This project is not affiliated with, endorsed by, or sponsored by
  Typst GmbH or the Quarkdown project.

## External Notices

This product includes software developed by:
- The Rust Project Contributors (https://www.rust-lang.org)
- Typst GmbH (https://typst.app) — Typst compiler

See `NOTICE` for full attribution.
