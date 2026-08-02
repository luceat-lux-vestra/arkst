# Quarkdown Compatibility — Specification Sources

This file records all public specification sources used for Scribium's
Quarkdown-compatible feature implementation.

## Primary Sources

| Source                          | URL or Citation                        | Used For                          | Date Accessed |
|---------------------------------|----------------------------------------|-----------------------------------|---------------|
| Public syntax documentation    | https://quarkdown.org/docs/syntax     | Dot-prefixed calls, arguments     | TBD           |
| Public CLI documentation       | https://quarkdown.org/docs/cli        | CLI behavior                      | TBD           |
| Example repository (read-only) | https://github.com/quarkdown/examples | Input/output observation          | TBD           |

## Observational Method

- All observations are conducted as black-box input/output analysis
- No source code is read or copied
- Observations are recorded in `fixtures/compatibility/` independently authored inputs
- Each observation includes the reference implementation version

## Prohibited Sources

The following are explicitly **not** used:

- Quarkdown implementation source code (any language)
- Quarkdown internal tests or test fixtures
- Quarkdown themes, CSS, HTML templates
- Quarkdown commit history or internal documentation
- quarkdown-wasm source code