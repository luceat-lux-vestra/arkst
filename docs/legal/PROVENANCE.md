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