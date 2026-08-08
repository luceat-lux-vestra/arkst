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
| GitHub release tag `v2.5.0`                   | https://github.com/iamgio/quarkdown/releases/tag/v2.5.0 | Reference baseline identification and v2.5.0 release additions such as `.markdown`, `.llmstxt`, `.code` and `.json` | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Dot-prefixed calls; positional, named, and mixed arguments; nested calls; block vs inline calls; indented bodies | 2026-08-08 |
|| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/conditional-statements/ | Conditional constructs: `.if`, `.ifnot`; boolean conditions; indented body semantics; nesting | 2026-08-08 
|| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/boolean/ | Boolean literals: `true`/`yes`, `false`/`no` (case-insensitive) | 2026-08-08
| Quarkdown wiki — "Syntax of a function call"  | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Documented-but-deferred v2.5.0 constructs: line continuation, `::` chaining, tight/brace-wrapped calls, multi-line arguments | 2026-08-08 |
| Quarkdown wiki — "Lambda"                      | https://quarkdown.com/wiki/lambda/ | Implicit positional references (`.1`, `.2`, ...) in lambdas | 2026-08-08 |
| Quarkdown quickstart                          | https://quarkdown.com/                      | Call examples (`.pow {5} to:{2}`, `.align {center}` with an indented body) | 2026-08-08 |
| Quarkdown Core API — `Lambda` class           | https://quarkdown.com/docs/latest/quarkdown-core/com.quarkdown.core.function.value.data/-Lambda/index.html | Implicit positional references (`.1`, `.2`, ...): "If not present, parameter names are automatically set to `.1`, `.2`" | 2026-08-08 |
| Quarkdown stdlib API — `foreach` / `Flow`     | https://quarkdown.com/docs/latest/quarkdown-stdlib/com.quarkdown.stdlib.module.Flow/foreach.html | Iterative calls using implicit references (`**.1**`); iteration index starts at 1 | 2026-08-08 |

The grammar implemented in Scribium is limited to the **function-call
syntax** documented on the wiki page above, plus the **conditional
constructs** (`.if` / `.ifnot`) and **implicit positional references**
(`.1`, `.2`, ...), restricted to the scope in
`docs/compatibility/quarkdown/README.md`.

The reference baseline is **Quarkdown v2.5.0**. The v2.5.0-badged *"Syntax of
a function call"* wiki page is the primary public specification source for
the currently implemented function-call syntax subset. Version provenance is
recorded per source:

- The **function-call syntax** page carries a `2.5.0` badge as of
  2026-08-08.
- The **Lambda** wiki page carries a `2.4.1` badge as of 2026-08-08 and is
  used only as documentation for implicit positional references; it
  documents already existing behavior and is not evidence that the feature
  was introduced in v2.5.0.
- The **Conditional statements** wiki page carries a `2.5.0` badge as of
  2026-08-08 and documents `.if` / `.ifnot` conditional semantics.
- The **Boolean** wiki page carries a `2.5.0` badge as of 2026-08-08 and
  documents boolean literals (`true`/`yes`, `false`/`no`, case-insensitive).
- The **`docs/latest/…` API pages** are unversioned and are corroborating
  sources rather than evidence that a behavior was introduced in or
  uniquely belongs to v2.5.0.

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