# PROVENANCE — Scribium

This document records the origin and legal provenance of all code, documentation,
and assets in the Scribium repository.

## Project Origin

- **Repository created:** 2026-08-02
- **Starting point:** Empty repository, no copied or migrated code
- **Source language:** Rust (independently authored)
- **Compatibility targets:** Quarkdown-compatible syntax (independently reimplemented)

## Clean-Room Policy

All Quarkdown-compatible features are implemented based solely on:

- Publicly available user-facing syntax documentation
- Publicly available CLI behavior
- Publicly available input/output examples
- Independently authored compatibility specifications
- Black-box conformance observations

No Quarkdown source code, internal AST structures, tests, themes, CSS,
templates, comments, error messages, or documentation text have been copied
or translated. See `CLEAN_ROOM_POLICY.md` for detailed rules.

## Third-Party Dependencies

See `deny.toml` for the dependency license policy and `THIRD_PARTY_LICENSES.md`
for the complete dependency license list (generated during builds).

## Reference JVM and generated Unicode compatibility data

The active JVM compatibility contract is the exact Eclipse Temurin
`25.0.4.1+1` release described in
[`docs/compatibility/quarkdown/reference-jvm.toml`](../compatibility/quarkdown/reference-jvm.toml).
Its Linux x64 archive is a pinned, generation-time input and is not bundled
with Scribium or required by the runtime. The archive URL, byte size,
SHA-256, source/build revisions, Java output, helper hash, and generated
artifact hash are recorded in that manifest.

`crates/scribium-engine/src/unicode_case.rs` contains independently generated
case-mapping values observed through the public Temurin/OpenJDK `Character`
and `String` APIs. `tools/dump_jdk25_unicode_data.java`, the corpus, and the
Python generator are independently authored Scribium tooling; no OpenJDK or
Quarkdown source or tests were copied. The generated values are compatibility
data, not a redistribution of the JDK implementation. The helper's exact
source hash and transient oracle hash are checked during regeneration.

The JDK distribution licensing and Classpath Exception terms are maintained
by OpenJDK; see the [OpenJDK legal information](https://openjdk.org/legal/)
and [GPLv2 with Classpath Exception](https://openjdk.org/legal/gplv2+ce.html).
The Unicode 16.0-derived case-data provenance and applicable Unicode notice
are covered by the [Unicode copyright and terms](https://www.unicode.org/copyright.html).
These upstream notices apply to the generation sources/data provenance; the
Scribium generator and runtime integration remain Apache-2.0 independently
authored code.

## Trademarks

- **Scribium** is a working name. A naming due diligence search must be
  completed before the first public release.
- This project is not affiliated with, endorsed by, or sponsored by
  Typst GmbH or the Quarkdown project.

## External Notices

This product includes software developed by:
- The Rust Project Contributors (https://www.rust-lang.org)
- Typst GmbH (https://typst.app) — Typst compiler

See `NOTICE` for full attribution.
