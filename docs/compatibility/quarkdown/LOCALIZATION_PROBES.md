# Quarkdown v2.5.1 — Localization Black-Box Probe Evidence

This file records independently authored, reviewer-reproducible black-box
probes of pinned Quarkdown v2.5.1's `.localization`/`.localize` runtime
behavior. It exists because the repository's "Reference JVM" workflow
(`docs/compatibility/quarkdown/reference-jvm.md`,
`docs/compatibility/quarkdown/reference-jvm.toml`) only validates the
Temurin/JDK 25 Unicode-case and locale-table generation oracle described
there — it does not run the Quarkdown JAR itself, and does not exercise
`.localization`/`.localize`. It proves nothing about Quarkdown's own
localization behavior, and this file's evidence is not (currently) checked by
any CI job; it is a manually reproducible record, not an automated gate.

None of the probe inputs below are copied from the upstream `quarkdown`
repository's test suite or from its `/lib/localization.qd`. Each `.qd` file
is a short, independently authored program written specifically to observe
one behavior, listed in full inline so a reviewer can retype and rerun it
without fetching anything upstream.

## Release identity

- Release: `iamgio/quarkdown` tag `v2.5.1` (same pinned release as
  `docs/compatibility/quarkdown/upstream.toml`'s `supported_baseline`).
- Release API: `https://api.github.com/repos/iamgio/quarkdown/releases/tags/v2.5.1`
- Two platform archives were used (both fetched directly from the GitHub
  release; SHA-256 is the release API's reported asset `digest`, verified
  locally with `sha256sum` after download):

  | Archive | SHA-256 | Used by |
  |---|---|---|
  | `quarkdown-macos-aarch64.zip` | `3cbfb9a995e0ec9412a54b0667af609b6b0526a5f77a8dade9317a1f262b296c` | Original PR #269 author (macOS/Apple Silicon); this is the SHA-256 already reported in the PR body. |
  | `quarkdown-linux-x64.zip` | `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9` | This document's probes (Linux/aarch64 review host). |

  Both are the same `v2.5.1` release; the SHA-256 differs only because they
  are different per-platform archives of the identical tagged source. The
  macOS aarch64 digest matches the PR body's claim exactly, corroborating
  that the original probes were genuinely run against this release.

## Method

The `quarkdown-linux-x64.zip` archive and the pinned Linux x64 Temurin archive
were executed together inside a `linux/amd64` container because this review
host is aarch64. The bundled Quarkdown `runtime/` directory was moved aside so
the launcher used the exact pinned JDK below. `quarkdown --version` reported
`quarkdown version 2.5.1`.

The casing-sensitive probes in this document therefore use the exact pinned
runtime, not an ordinary host JDK:

- Quarkdown archive: `quarkdown-linux-x64.zip`, SHA-256
  `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`.
- JVM archive: `OpenJDK25U-jdk_x64_linux_hotspot_25.0.4.1_1.tar.gz`, SHA-256
  `dbb698396d478e7fa2b1e50f4103324b2a99b90569ee27c33f2261f9215cf41e`.
- Runtime: Eclipse Temurin `25.0.4.1+1-LTS`, vendor
  `Temurin-25.0.4.1+1`, Java version date `2026-08-18`, CLDR provider.
- Exact command used inside the container for each probe:

  ```sh
  JAVA_HOME=/tmp/jdk/jdk-25.0.4.1+1 \
  PATH=/tmp/jdk/jdk-25.0.4.1+1/bin:$PATH \
  /tmp/quarkdown/quarkdown/bin/quarkdown \
  compile --pipe --strict --allow=all /dev/stdin
  ```

The command was run with `docker run --platform linux/amd64` and the archive
was extracted with the matching pinned JDK's `jar` tool. This matters for
probe 1: `.localize` directly executes Kotlin/JVM `key.lowercase()`, including
context-sensitive Unicode casing. The non-casing probes remain supplementary
observations, but they were also rerun in this exact environment.

Each probe is compiled with:

```sh
quarkdown compile --pipe --strict --allow=all <file>.qd
```

`--pipe` prints only the rendered HTML to stdout; `--strict` exits non-zero
on any diagnostic instead of continuing; `--allow=all` avoids irrelevant
permission prompts. The observed result is the `<main>...</main>` HTML
fragment (for successful compiles) or the uncaught exception's class and
message (for failures — `--strict`/uncaught exceptions both surface on
stderr with a JVM stack trace; only the first line is quoted below).

## Probes

### 1. Lowercase-first-then-original key lookup (`entries[key.lowercase()] ?: entries[key]`)

This is the behavior issue #196's review required: `BaseContext.localize()`
tries the lowercased key first, then the original key, and is not general
case-insensitive matching.

```
.doclang {en}
.localization {t}
    - en
      - warning: lower
.localize {t:WARNING}
```
→ `<p>lower</p>` (lowercase-only entry, uppercase lookup: hits via `.lowercase()`)

```
.doclang {en}
.localization {t}
    - en
      - Warning: exact
.localize {t:Warning}
```
→ `<p>exact</p>` (exact key match, no lowercasing needed)

```
.doclang {en}
.localization {t}
    - en
      - warning: lower
      - Warning: exact
.localize {t:Warning}
```
→ `<p>lower</p>` — **both keys exist and differ; the lowercase-key entry wins**,
even though the requested key `Warning` is itself present verbatim. This is
the single most important observation: it rules out "try exact first, then
lowercase" or "prefer the requested casing" and confirms lowercase-first
order.

```
.doclang {en}
.localization {t}
    - en
      - Warning: exact
.localize {t:warning}
```
→ uncaught `com.quarkdown.core.localization.LocalizationKeyNotFoundException: Could not find localization key "warning" in table "t" for locale en`
— only `Warning` (not `warning`) exists; `"warning".lowercase()` is
`"warning"` (misses), and the original requested key `warning` also misses
`Warning`. **Not general case-insensitive lookup**: a case-different exact
key does not satisfy the lookup.

#### Contextual Greek final sigma

The pinned JVM's whole-string invariant lowercase maps `ΟΣ` (U+039F U+03A3)
to `ος` (U+03BF U+03C2), not `οσ` (U+03BF U+03C3), because the sigma is final
after a cased letter:

```
.doclang {en}
.localization {t}
    - en
      - ος: final
.localize {t:ΟΣ}
```
→ observed stdout body: `<main><p>final</p></main>`.

The same exact command produced observed stdout body
`<main><p>contextual</p></main>` when both entries were present, proving the
whole-string lowercase result wins before original-key fallback:

```
.doclang {en}
.localization {t}
    - en
      - ος: contextual
      - ΟΣ: original
.localize {t:ΟΣ}
```

An entry containing only the context-insensitive `οσ` key (U+03BF U+03C3)
failed with `LocalizationKeyNotFoundException`; it did not match the pinned
lowercase result `ος` or the original `ΟΣ`. This negative case prevents an
incorrect simple per-scalar sigma mapping from passing.

The repository's pinned Temurin differential additionally gates sequence-sensitive
word boundaries that a one-character sigma probe cannot prove. In particular,
`ΟΣ'Α` lowercases with ordinary sigma while `ΟΣ''Α` lowercases with final sigma;
the Reference JVM corpus checks both forms (and the analogous single/repeated
period forms) against the exact Temurin 25 runtime before `.localize` can rely on
the generated word-break DFA.

The same differential also covers supplementary-plane casing so the JVM's
UTF-16 representation cannot hide a scalar-conversion bug. For example,
`𐐀Σ` (U+10400 U+03A3) must lowercase to `𐐨ς` (U+10428 U+03C2), proving that
the supplementary Deseret scalar mapping and contextual Final_Sigma rule
compose correctly. This is pinned-JVM oracle evidence, not an additional
Quarkdown black-box claim.

### 2. Custom separator, first-boundary split

```
.doclang {en}
.localization {t}
    - en
      - a/b: nested value
.localize {t/a/b} separator:{/}
```
→ `<p>nested value</p>` — `t/a/b` splits at the *first* `/` only, giving
table `t` and key `a/b` (which itself contains the separator character);
the table lookup is not repeatedly split on every separator occurrence.

### 3. Empty separator

```
.localize {t} separator:{}
```
→ uncaught `com.quarkdown.core.localization.LocalizationTableNotFoundException: Could not find localization table ""`
— splitting `"t"` on an empty delimiter with a 2-limit split yields
`("", "t")` (table name `""`, key `"t"`), matching Kotlin's
`String.split(limit=2)` behavior with an empty delimiter. Rust's
`"t".split_once("")` (verified locally with `rustc`) returns
`Some(("", "t"))` — the same split point — so Arkst's existing
`key.split_once(&separator)` (unchanged by this fix) already reproduces this.

### 4. No locale set

```
.localize {std:warning}
```
→ uncaught `com.quarkdown.core.localization.LocaleNotSetException: Trying to localize from a document that does not have a locale set. Tip: .doclang {locale}`

### 5. Regional locale without implicit fallback

```
.doclang {en-US}
.localization {t}
    - en
      - key: value
.localize {t:key}
```
→ uncaught `com.quarkdown.core.localization.LocalizationLocaleNotFoundException: Could not find locale en-US in table "t"`
— the table has an `en` entry but the active locale is the regional tag
`en-US`; Quarkdown does **not** fall back from `en-US` to `en`. Locale
lookup is exact-tag, matching Arkst's unchanged `table.get(&locale.tag)`.

### 6. Numeric/Boolean values in a localization dictionary

```
.doclang {en}
.localization {t}
    - en
      - num: 42
      - flag: true
.localize {t:num}
.localize {t:flag}
```
→ `<p>42</p><p>true</p>` — Number and Boolean values are accepted and
stringified, matching Arkst's `scalar_string_conversion` boundary for
`contents` entry values (Arkst's evidence already covers this; this probe is
corroborating, independent confirmation).

### 7. Nested dictionary as an entry value

```
.doclang {en}
.localization {t}
    - en
      - key:
          - nested: value
.localize {t:key}
```
→ uncaught **`java.lang.ClassCastException: class java.util.LinkedHashMap cannot be cast to class java.lang.String`**
at `com.quarkdown.stdlib.LocalizationKt.buildLocalizationTable` — upstream
does not validate this and crashes with an internal JVM exception rather
than a structured diagnostic. Arkst deliberately does **not** reproduce this
crash: `validate_localization_dictionary` rejects a nested-Dictionary entry
value before publication with a structured `E3001` diagnostic
(`.localization: unsupported value category for target String for parameter
`contents``, see `localization_conversion_failure_is_source_backed` in
`crates/arkst-core/tests/quarkdown_localization.rs`). Both reject the input;
Arkst's failure mode is a documented, deliberate improvement over
upstream's uncaught-exception behavior, not a semantic gap.

### 8. Repeated dictionary key

```
.doclang {en}
.localization {t}
    - en
      - key: first
      - key: second
.localize {t:key}
```
→ `<p>second</p>` — last write wins, matching Arkst's `BTreeMap` insert
(later `entries.insert(key, value)` calls overwrite earlier ones for the
same key) and Kotlin's `mutableMapOf`-style sequential `put` construction.

### 9. Bounded seeded `std` entries

All ten seeded `std` locales independently reproduced against the pinned
release, immediately after `.doclang {<locale>}` with no other setup
(`.localize {std:warning}` / `.localize {std:error}`):

| Locale | `warning` | `error` |
|---|---|---|
| `zh` | 警告 | 错误 |
| `en` | Warning | Error |
| `fr` | Attention | Erreur |
| `de` | Warnung | Fehler |
| `it` | Attenzione | Errore |
| `ja` | 警告 | エラー |
| `pl` | Ostrzeżenie | Błąd |
| `pt` | Aviso | Erro |
| `ru` | Предупреждение | Ошибка |
| `uk` | Попередження | Помилка |

Every value is byte-for-byte identical to `seeded_localization_tables()` in
`crates/arkst-engine/src/evaluator.rs` and to
`seeded_std_table_covers_the_independently_evidenced_public_locales` in
`crates/arkst-core/tests/quarkdown_localization.rs`.

## Summary of implementation decisions this evidence supports

- `.localize` key lookup: lowercase-first, then original, not general
  case-insensitive matching (probe 1) — implemented by
  `crate::builtins::canonical_lowercase` plus the `entries.get(lowercase).or_else(|| entries.get(original))`
  fallback in `evaluate_localization_builtin` (`crates/arkst-engine/src/evaluator.rs`).
- Separator splits only at the first boundary, including on an empty
  separator (probes 2–3) — unchanged, already matched by
  `key.split_once(&separator)`.
- No locale-hierarchy fallback for regional tags (probe 5) — unchanged,
  already matched by exact `table.get(&locale.tag)`.
- Numeric/Boolean entry values remain accepted and stringified (probe 6) —
  unchanged, already matched by `scalar_string_conversion`.
- A nested-dictionary entry value is rejected, but with a structured
  diagnostic instead of reproducing upstream's uncaught crash (probe 7) —
  a deliberate, documented divergence, not a gap.
- Last-write-wins for a repeated dictionary key (probe 8) — unchanged,
  already matched by `BTreeMap::insert`.
- The bounded seeded `std` table's ten locale/`warning`/`error` triples are
  byte-for-byte correct (probe 9).
