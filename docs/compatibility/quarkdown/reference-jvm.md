# Reference JVM contract

Arkst deterministically reproduces the JVM-observable semantics required by
pinned Quarkdown v2.5.1 using Eclipse Temurin `25.0.4.1+1` as its reference
JVM, without requiring a JVM at runtime. Quarkdown is not asserted to require
JDK 25; this is Arkst's explicit compatibility oracle choice for behavior
that can vary with the JVM. A future reference-JVM change requires a separate
compatibility migration.

The machine-readable pin is [`reference-jvm.toml`](reference-jvm.toml). The
repository-wide generated/reference-data rules are defined by the
[integrity contract](../reference-provenance.md) and enforced by its
deterministic verifier. This JDK-specific manifest remains the authoritative
source for the detailed archive, runtime, oracle, and locale values. The
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
  `.startswith(ignorecase:true)`; and
- `Character.isLowerCase`, `Character.isUpperCase`, and
  `Character.isTitleCase`, which provide the pinned `Cased` property used by
  invariant-locale contextual final-sigma lowering; and
- the exact `Locale.ROOT` `RuleBasedBreakIterator` forward DFA/category mapping
  used by JDK `ConditionalSpecialCasing.isFinalCased`, captured into a separate
  generated `word_break.rs` so sequence-sensitive word boundaries do not depend
  on a host JVM or locale at runtime.

The helper is independently authored and is not runtime code. The transient
oracle output is not checked in. The checked-in generated Rust table is
`159815` bytes with SHA-256
`0a27e755125cc7dcb9aa757fde0b0ef8bc46e3014a60c445931934e912d24f76`.
The helper SHA-256 is
`2b6459d6dfee9e2780c50e1734ac8302900bd9d601acae401b0af064c0ba568f`, and the
complete oracle output SHA-256 is
`df7748b6674398b726fa33f92c98ed9783f292173a257aa3fbb593016c9b38d9`.
The generated case data contains `2933` non-identity scalar rows and all
`65536` UTF-16 code-unit rows. Final-sigma word segmentation is separately
generated from the same pinned runtime's root word-break DFA; the Reference JVM
check regenerates both artifacts byte-for-byte.

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
Temurin 25 oracle emits `99997` rows, `3199082` bytes, and the SHA recorded
above; this includes `4578` generated `Cased` property records and `26950`
generated Final_Sigma context records in addition to the `68469` scalar/
UTF-16 mapping records. The JDK 17 values are historical comparison evidence,
not an active compatibility baseline.

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
oracle and uses only the active CLDR/FALLBACK provider data from this pin. It
remains a bounded observable `.doclang` surface, not a claim of complete Java
Locale support.

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
`2dc572125ce0e50854fc3ec538acde3358c5b0320e13b501162411a34dc36105`, the
logical display source SHA-256 is
`96d43b0ff823a4505bdb69ddd80bfd3056867b2c7c0bc27b6a50fc822c116ab3`, and the
compact artifact SHA-256 is
`d086d29fbb3716624efb0066df9fe09cd6df21438931ed2b0f0ddc17743e68b1`.
The transient public Java differential emits 7,440 rows (5,122 tag-path and
2,318 name-path rows), with SHA-256
`05b090ea11486c5f0487a40e6fd499f11e282e5e3490ede1688166c336c9faed`; its
helper source SHA-256 is
`80d7c48a4bc1d864e13b0ea4b327f92b6108d5e48f6846f8687e93613f91dc02`.
The locale generator source SHA-256 is recorded in the machine-readable
manifest.
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
generated Rust metadata is 814,472 bytes with
SHA-256
`c16d5d8c8bc96ec129bcfe222ae7acbf5ff91812108cde042477ef0b7b46ca34` and
remains below the 1 MiB source budget.
The generator exhaustively reconstructs every logical display row, validates
the binary index, and checks both generated artifacts byte-for-byte. JDK25's
CLDR parent-locale, likely-script, and language-alias routing metadata is
captured separately from the public ResourceBundle candidate identity; the
active secondary provider layer is the JDK FALLBACK root data.
