# Quarkdown Compatibility — Specification Sources

This file records all public specification sources used for Scribium's
Quarkdown-compatible feature implementation.

## Reference Baseline

- **Reference version:** Quarkdown **v2.5.1** (released 2026-08-12;
  `iamgio/quarkdown` tag `v2.5.1`)
- **Compatibility target:** complete public-language and document-observable semantic compatibility (ADR 0016)
- **Current verified baseline:** v2.5.1; current implementation is partial
- **Historical evidence retained:** v2.5.0 sources below remain part of the
  provenance record and are not deleted by this adaptation review

## Primary Sources

| Source                                        | Title / Citation                                   | Used For                                        | Date Accessed |
|-----------------------------------------------|----------------------------------------------------|-------------------------------------------------|---------------|
| GitHub release tag `v2.5.0`                   | https://github.com/iamgio/quarkdown/releases/tag/v2.5.0 | Reference baseline identification and v2.5.0 release additions such as `.markdown`, `.llmstxt`, `.code` and `.json` | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Dot-prefixed calls; positional, named, and mixed arguments; nested calls; block vs inline calls; indented bodies | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/conditional-statements/ | Conditional constructs: `.if`, `.ifnot`; boolean conditions; indented body semantics; nesting | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/boolean/ | Boolean literals: `true`/`yes`, `false`/`no` (case-insensitive) | 2026-08-08 |
| Quarkdown wiki — "Variables"                   | https://quarkdown.com/wiki/variables/ | Variable declaration (`.var`), reference (`.name`), reassignment (`.name {value}`), block variables, boolean use in conditionals | 2026-08-08 |
| Quarkdown wiki — "Syntax of a function call"   | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Documented-but-deferred v2.5.0 constructs: line continuation, `::` chaining, tight/brace-wrapped calls, multi-line arguments | 2026-08-08 |
| Quarkdown wiki — "Syntax of a function call" (v2.5.1 syntax review) | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Behavior specification for the #60 multiline-argument, continuation, chaining, tight-call, and block/inline boundary fixtures | 2026-08-14 |
| Quarkdown wiki — "Lambda" (v2.5.1)              | https://quarkdown.com/wiki/lambda/ | Headerless lambda implicit positional references (`.1`, `.2`, ...), nested scope behavior | 2026-08-16 |
| Quarkdown quickstart                          | https://quarkdown.com/                      | Call examples (`.pow {5} to:{2}`, `.align {center}` with an indented body) | 2026-08-08 |
| Quarkdown Core API — `Lambda` class           | https://quarkdown.com/docs/latest/quarkdown-core/com.quarkdown.core.function.value.data/-Lambda/index.html | Implicit positional references (`.1`, `.2`, ...): "If not present, parameter names are automatically set to `.1`, `.2`" | 2026-08-08 |
| Quarkdown stdlib API — `foreach` / `Flow`     | https://quarkdown.com/docs/latest/quarkdown-stdlib/com.quarkdown.stdlib.module.Flow/foreach.html | Iterative calls using implicit references (`**.1**`); iteration index starts at 1 | 2026-08-08 |
| GitHub release tag `v2.5.1`                   | https://github.com/iamgio/quarkdown/releases/tag/v2.5.1 | Release identification and D1-D5 public delta inventory | 2026-08-13 |
| CommonMark specification, current link rules  | https://spec.commonmark.org/current/#links | D2 balanced/escaped link destinations, literal trailing delimiters, and URI backslash-escape semantics | 2026-08-13 |
| CommonMark specification, current autolink rules | https://spec.commonmark.org/current/#autolinks | D2 autolink URI/email grammar and the rule that backslash escapes do not apply inside autolinks | 2026-08-13 |
| CommonMark specification, current list rules  | https://spec.commonmark.org/current/#lists | D3 nested list container and indentation semantics | 2026-08-13 |
| Quarkdown wiki — Markdown content            | https://quarkdown.com/wiki/markdown-content/ | Public body-content interaction for links, lists, and nested block content | 2026-08-13 |
| Quarkdown wiki — Iterable                   | https://quarkdown.com/wiki/iterable/ | Corroborating public scope for nested Markdown list document semantics | 2026-08-13 |
| Quarkdown wiki — Subdocuments                | https://quarkdown.com/wiki/subdocuments/ | Corroborating public scope for D4 local subdocument links and HTML output | 2026-08-13 |

The current evidence set covers the function-call syntax documented on the wiki
page above, plus the conditional constructs (`.if` / `.ifnot`) and implicit
positional references (`.1`, `.2`, ...), at the levels recorded in
`docs/compatibility/quarkdown/README.md`. This is the current verified evidence
baseline, not a permanent restriction on the complete public-language target.
Public features not yet covered remain compatibility debt.

The current verified baseline is **Quarkdown v2.5.1**. The v2.5.0-badged
*"Syntax of a function call"* wiki page is the primary public specification
source for the currently evidenced function-call behavior. Version provenance
is recorded per source:

- The **function-call syntax** page carries a `2.5.0` badge as of
  2026-08-08.
- The **Lambda** wiki page carries a `2.5.1` badge as of 2026-08-16 and
  documents headerless implicit positional references and nested lambda
  scopes.
- The **Conditional statements** wiki page carries a `2.5.0` badge as of
  2026-08-08 and documents `.if` / `.ifnot` conditional semantics.
- The **Boolean** wiki page carries a `2.5.0` badge as of 2026-08-08 and
  documents boolean literals (`true`/`yes`, `false`/`no`, case-insensitive).
- The **`docs/latest/…` API pages** are unversioned and are corroborating
  sources rather than evidence that a behavior was introduced in or
  uniquely belongs to v2.5.0.
- The **v2.5.1 release notes** are the primary source for release
  identification and D1-D5. The CommonMark links/autolinks/lists sections are
  corroborating public behavior specifications for D2/D3. The Markdown
  content and Subdocuments wiki pages corroborate scope and are not claims
  that those behaviors were introduced in v2.5.1.

The sources listed above are the sources consulted for this feature set and
the v2.5.1 impact review.

## Observational Method

- Implemented from public documentation and a permitted black-box probe of the
  official v2.5.1 macOS arm64 release; the probe checked successful `.1`
  binding and observed unresolved-reference failures for missing and
  zero-argument `.N` references
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
