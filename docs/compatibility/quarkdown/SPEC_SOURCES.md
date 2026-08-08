# Quarkdown Compatibility — Specification Sources

This file records all public specification sources used for Scribium's
Quarkdown-compatible feature implementation.

## Primary Sources

| Source                                        | Title / Citation                           | Used For                                        | Date Accessed |
|-----------------------------------------------|--------------------------------------------|-------------------------------------------------|---------------|
| Public syntax documentation (deprecated)      | https://quarkdown.org/docs/syntax           | (unreachable — replaced by the wiki below)      | —             |
| Quarkdown wiki                                | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Dot-prefixed calls; positional, named, and mixed arguments; nested calls; block vs inline calls; indented bodies | 2026-08-08 |
| Quarkdown quickstart                          | https://quarkdown.com/                      | Call examples (`.pow {5} to:{2}`, `.align {center}` with an indented body) | 2026-08-08 |
| Public release notes / changelog              | public release notes                        | Confirmation that the basic dot-and-brace call grammar is stable for the 0.9.x target and unchanged in later series | 2026-08-08 |

The grammar implemented in Scribium is limited to the **function-call
syntax** documented on the wiki page above, restricted to the scope in
`docs/compatibility/quarkdown/README.md`. The project keeps the documented
0.9.x target version; the basic call grammar is valid for that target.

URLs other than those listed above were not consulted for this feature set.

## Observational Method

- Implemented exclusively from public documentation (no interactive
  reference sessions were run for this subset)
- No source code is read or copied
- The test inputs in `fixtures/` are independently authored from the
  specification documents above; they are not copied from reference inputs
- Each feature's provenance is recorded in
  `docs/compatibility/quarkdown/README.md`

## Prohibited Sources

The following are explicitly **not** used:

- Quarkdown implementation source code (any language)
- Quarkdown internal tests or test fixtures
- Quarkdown themes, CSS, HTML templates
- Quarkdown commit history or internal documentation
- quarkdown-wasm source code