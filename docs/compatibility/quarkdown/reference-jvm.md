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

Locale data and `.doclang` implementation are Phase B work and remain
separate from #172 and this migration PR. In particular, this change does not
modify PR #223, regenerate locale data, or claim complete Java Locale support.
