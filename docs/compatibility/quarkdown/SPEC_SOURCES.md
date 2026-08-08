# Quarkdown Compatibility — Specification Sources

This file records all public specification sources used for Scribium's
Quarkdown-compatible feature implementation.

## Reference Baseline

- **Reference version:** Quarkdown **v2.5.0** (released 2026-08-04;
  `iamgio/quarkdown` tag `v2.5.0`)
- **Compatibility model:** documented feature subset (ADR 0012)
- **Full compatibility claim:** none

## Primary Sources

| Source                                        | Title / Citation                                   | Used For                                        | Date Accessed |
|-----------------------------------------------|----------------------------------------------------|-------------------------------------------------|---------------|
| GitHub release tag `v2.5.0`                   | https://github.com/iamgio/quarkdown/releases/tag/v2.5.0 | Reference baseline identification; release notes confirm the dot-and-brace call grammar and document v2.5.0 additions (`.json` data loading, `.markdown`, `.llmstxt`) | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Dot-prefixed calls; positional, named, and mixed arguments; nested calls; block vs inline calls; indented bodies | 2026-08-08 |
| Quarkdown wiki — "Syntax of a function call"  | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Documented-but-deferred v2.5.0 constructs: line continuation, `::` chaining, tight/brace-wrapped calls, multi-line arguments | 2026-08-08 |
| Quarkdown wiki — "Lambda"                      | https://quarkdown.com/wiki/lambda/ | Implicit positional references (`.1`, `.2`, ...) in lambdas | 2026-08-08 |
| Quarkdown quickstart                          | https://quarkdown.com/                      | Call examples (`.pow {5} to:{2}`, `.align {center}` with an indented body) | 2026-08-08 |
| Quarkdown Core API — `Lambda` class           | https://quarkdown.com/docs/latest/quarkdown-core/com.quarkdown.core.function.value.data/-Lambda/index.html | Implicit positional references (`.1`, `.2`, ...): "If not present, parameter names are automatically set to `.1`, `.2`" | 2026-08-08 |
| Quarkdown stdlib API — `foreach` / `Flow`     | https://quarkdown.com/docs/latest/quarkdown-stdlib/com.quarkdown.stdlib.module.Flow/foreach.html | Iterative calls using implicit references (`**.1**`); iteration index starts at 1 | 2026-08-08 |

The grammar implemented in Scribium is limited to the **function-call
syntax** documented on the wiki page above, restricted to the scope in
`docs/compatibility/quarkdown/README.md`, plus the **implicit positional
references** (`.1`, `.2`, ...). The basic dot-and-braces call grammar is
documented consistently across the wiki's release history; the reference
baseline is the v2.5.0 wiki (badge `2.5.0`).

The wiki pages carry a `2.5.0` version badge as of the access date. Where a
documentation page or release notes describe behavior, the claim recorded in
the compatibility matrix is limited to what the page states; no unverified
v2.5.0 behaviors are assumed. The `docs/latest/…` API pages above are
unversioned ("latest"); they are cited only for claims that the versioned
wiki also confirms (implicit references).

URLs other than those listed above were not consulted for this feature set.

## Observational Method

- Implemented from public documentation only (no interactive reference
  sessions were run for this subset)
- No Quarkdown source code is read or copied
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