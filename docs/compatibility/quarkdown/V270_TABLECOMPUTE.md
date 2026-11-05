# Quarkdown v2.7.0 `.tablecompute` clean-room evidence

Issue: #712  
Date: 2026-11-05

## Reference artifact

The probes used the official Quarkdown v2.7.0 Linux x64 release archive:

- asset: `quarkdown-linux-x64.zip`
- SHA-256: `5ff4e78e391abd073e62030da816faef265d681d3b8ccfd05768cc9c586f7e85`
- reported runtime: `quarkdown version 2.7.0`
- disposable evidence workflow run: `41967127988`

The workflow downloaded the release artifact directly from the GitHub release,
verified its digest, and executed independently authored inputs. No upstream
implementation source, tests, fixtures, or documentation examples were copied
or translated.

## Observed bounded contract

The selected body column is provided to the one-argument callback in source
order. A first/last probe over `Alpha`, `Beta`, `Gamma` produced
`Alpha` and `Gamma`, respectively.

Header-only input invokes the callback with an empty collection:
`.size` produced `0` and one computed bottom row. A ragged missing selected
cell is represented as an empty String: selecting the second element of
`[present, "", tail]` and applying `.isempty` produced `true`.

The original header and body rows stay intact. The computed row is appended
after them, has the table width, places its result only in the requested
one-based column, and leaves the other cells blank.

The same behavior was observed for:

- ordinary Markdown tables;
- rich selected cells projected to visible plain text before numeric
  aggregation;
- generated headerless `.tablebyrows` tables;
- a table nested in a blockquote; and
- inner `.tablefilter` followed by outer `.tablecompute`.

## Callback-result text projection

Stable observable result text from the official binary included:

| Result category | Observed cell text |
| --- | --- |
| Number | `19` |
| Boolean | `false` |
| closed Range | `2..4` |
| None | `None` |
| empty Collection | `[]` |
| outputless callback / Unit-shaped result | `kotlin.Unit` |

Content-backed results do not expose one deterministic value-model string that
Arkst can reproduce without importing JVM object identity. A `.join` result
rendered as a JVM class-plus-identity string, while a Markdown-shaped callback
result rendered its markers literally. Arkst therefore does not approximate
those categories in #712: they fail closed with E8001 until a deterministic
portable contract is established.

## Failure boundary

With `--strict`, an out-of-range column and an explicitly failing callback
both exit non-zero and produce no table result. Arkst keeps its existing
structured, source-backed diagnostics rather than claiming upstream exception
wording or stack-trace parity.

## Arkst disposition

The bounded #712 implementation may claim the stable behavior above. It must
continue to reuse the existing table IR, first-table traversal, table-cell
plain-text projection, callable/savepoint path, evaluation-depth limit, and
materialization limit. Captions/references, CSV production, repeated-header or
pagination behavior, exact HTML/CSS renderer equivalence, selected real-PDF
proof for this slice, and nondeterministic content-object result stringification
remain outside this evidence claim.
