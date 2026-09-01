# Reference JVM contract

Scribium deterministically reproduces the JVM-observable semantics required by
pinned Quarkdown v2.5.1 using Eclipse Temurin `25.0.4.1+1` as its reference
JVM, without requiring a JVM at runtime. Quarkdown is not asserted to require
JDK 25; this is Scribium's explicit compatibility oracle choice for behavior
that can vary with the JVM. A future reference-JVM change requires a separate
compatibility migration.

The machine-readable pin is [`reference-jvm.toml`](reference-jvm.toml). The
canonical generation asset is the Linux x64 JDK archive:

- release/tag: `jdk-25.0.4.1+1`
- filename: `OpenJDK25U-jdk_x64_linux_hotspot_25.0.4.1_1.tar.gz`
- URL: <https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.4.1%2B1/OpenJDK25U-jdk_x64_linux_hotspot_25.0.4.1_1.tar.gz>
- size: `141329719` bytes
- SHA-256: `dbb698396d478e7fa2b1e50f4103324b2a99b90569ee27c33f2261f9215cf41e`
- vendor/runtime: `Eclipse Adoptium`, `Temurin-25.0.4.1+1`
- Java version: `25.0.4.1`; locale provider: `CLDR`; Unicode baseline: `16.0.0`

The canonical asset reports:

```text
openjdk version "25.0.4.1" 2026-08-18 LTS
OpenJDK Runtime Environment Temurin-25.0.4.1+1 (build 25.0.4.1+1-LTS)
OpenJDK 64-Bit Server VM Temurin-25.0.4.1+1 (build 25.0.4.1+1-LTS, mixed mode, sharing)
```

The source revision is `520406d871955300957ef01e406ac2acd0f9b75c`, tagged
`jdk-25.0.4.1+1_adopt`, from
<https://github.com/adoptium/jdk25u.git>. The Temurin build revision is
`e6ba7dec3d07654074559310376a3ae89da5f4ac` from
<https://github.com/adoptium/temurin-build.git>.

## Phase A string semantics

Phase A migrates the already-merged #172 bounded string contract. The
generation-only Java helper calls the pinned public `Character` and `String`
APIs and captures:

- simple scalar upper/lower mappings for the complete Unicode scalar domain;
- simple mappings for every UTF-16 `Char` input;
- full upper/lower mappings and the Kotlin `Char.titlecase()` transformation
  needed by `.capitalize`; and
- `String.regionMatches(true, ...)`, which is the external oracle for
  `.startswith(ignorecase:true)`.

The helper is independently authored and is not runtime code. The transient
oracle output is not checked in. The checked-in generated Rust table is
`148058` bytes with SHA-256
`6b8f1a69a693357fd601b0908333370a2304edd642ac8ff6dbb520dbfb9211ad`.
The helper SHA-256 is
`e10e15f92ef6f996ed117e2d5e3d590a01df511abad7ffc583e349f59b76fa47`, and the
complete oracle output SHA-256 is
`f5efcfe8628d7794a459872a54f501c8541859a6daf2d0ed382af46cb6cdd862`.
The generated data contains `2933` non-identity scalar rows and all `65536`
UTF-16 code-unit rows.

Compared with the former JDK 17/Unicode 13 baseline, the direct Temurin
comparison found `134` newly non-identity scalar rows and `20` changed BMP
UTF-16 rows. The observed additions include Unicode 14-era Latin, Cyrillic,
Glagolitic, and Vithkuqi mappings and Unicode 16-era Garay mappings; the
exhaustive comparison found no additional relevant scalar case-mapping rows
from Unicode 15.0 or 15.1. The result is measured oracle behavior, not a
blind Unicode-version substitution. Existing full/simple distinctions remain
covered, including `ᾀ`, `ß`, `ŉ`, supplementary leading characters, and the
newly paired `Ꟑ`/`ꟑ` behavior.

For migration evidence only, the same helper under the former exact Temurin
`17.0.20.1+1` runtime emitted `68335` rows, `2685553` bytes, and SHA-256
`d771cf16704b8543fc4b1cb191a746b35ca78b7dd4b7c2587774bebc91031f5b`. The
Temurin 25 oracle emits `68469` rows, `2688843` bytes, and the SHA recorded
above. The JDK 17 values are historical comparison evidence, not an active
compatibility baseline.

The runtime has no JVM, filesystem, network, host-locale, or mutable-global
dependency. The generation/check path validates the exact archive, Java
properties, helper hash, oracle hash, and generated-source hash. It also runs
the complete transient oracle through the generated mappings and the public
engine builtin entry points, and the generator rejects generated Rust source
at or above its executable 1 MiB budget. The local development host used the
exact Temurin macOS arm64 build only because it cannot execute Linux x64 ELF; the
canonical Linux x64 asset remains the pinned provenance and CI verification
input.

Locale data and `.doclang` implementation are a separate Phase B workstream,
but they use this same active Temurin 25 baseline after the migration is
accepted. The Phase B snapshot is regenerated from the pinned JDK25 CLDR
oracle and does not carry forward the former JDK17 provider data. It remains a
bounded observable `.doclang` surface, not a claim of complete Java Locale
support.

## Phase B locale snapshot

PR #223's locale closure uses the same canonical archive and captures
`Locale.getAvailableLocales()`, `Locale.forLanguageTag()`, the Quarkdown
name-first lookup path, and the JDK25 `Locale.getDisplayName` provider result.
The checked-in locale snapshot contains 1,158 available-locale records and
1,157 canonical-tag records, including the blank-language root locale. Its
logical display oracle contains 453,459
records; semantic fallback-delta compaction retains 267,017 records in the
6,549,860-byte little-endian binary snapshot, with 320 profiles, 2,525 keys,
and 178,930 interned values. The logical locale source SHA-256 is
`85b704ef5648633ad0b22a6a326ce508109fa56348e5380460c4bc4d73271e16`, the
logical display source SHA-256 is
`96d43b0ff823a4505bdb69ddd80bfd3056867b2c7c0bc27b6a50fc822c116ab3`, and the
compact artifact SHA-256 is
`d086d29fbb3716624efb0066df9fe09cd6df21438931ed2b0f0ddc17743e68b1`.
The transient public Java differential emits 7,302 rows (4,984 tag-path and
2,318 name-path rows), with SHA-256
`7591c871e8cac354b29519bedad6a5cb3f389c94bce15ea44193e76299ff9ac4`; its
helper source SHA-256 is
`57e5e5dd3956ecf422d85cffc5fe0241e52a07911a8e820ebe9f095f7a5a63a1`.
The name-first table preserves a pinned raw-array capture of
`Locale.getAvailableLocales()` in
`tools/jdk25_available_locale_order.tsv` (SHA-256
`c4dd6cd7e83919d7236d3040c1ddc60ca21ff92e179b19a7d7d10fda7f9a815e`); the
JDK25 character-wise English-name collision audit found zero collision
classes. The root/private-use records preserve empty `code`/`shortTag`-equivalent
fields while retaining JDK `toLanguageTag()` serialization (`und`, or
`x-private` for private-use-only tags). The direct oracle includes `""`, `und`,
`x-private`, `en_US`, whitespace, and `en--US`; their JDK results are retained
without an oracle-side unresolved rewrite or blank-language filtering. The
generated Rust metadata is 502,655 bytes with
SHA-256
`5ebc94722c042340a84b03250ade2c9c716cfd228ad060c2186db2b41aa2ec2f` and
remains below the 1 MiB source budget.
The generator exhaustively reconstructs every logical display row, validates
the binary index, and checks both generated artifacts byte-for-byte. JDK25's
CLDR parent-locale, likely-script, and language-alias routing metadata is
captured separately from the public ResourceBundle candidate identity; no
JRE/COMPAT provider data is active.
