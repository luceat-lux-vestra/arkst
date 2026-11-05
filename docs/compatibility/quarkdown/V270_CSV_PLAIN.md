# Quarkdown v2.7.0 plain `.csv` clean-room evidence

Issue: #714  
Date: 2026-11-05

## Public contract

The current public Quarkdown reference documents:

```text
csv(
  path: String,
  mode: CsvParsingMode = CsvParsingMode.PLAIN,
  caption: InlineMarkdownContent? = null,
  ref: String? = null
) -> Node
```

#714 intentionally implements only the independently separable default/explicit
`PLAIN` producer. `MARKDOWN` cell materialization and non-null caption/ref
remain outside this slice.

## Reference artifact

Independent probes used the official Quarkdown v2.7.0 Linux x64 release:

- asset: `quarkdown-linux-x64.zip`
- SHA-256: `5ff4e78e391abd073e62030da816faef265d681d3b8ccfd05768cc9c586f7e85`
- runtime: `quarkdown version 2.7.0`
- final evidence workflow run: `41996949225`
- initial successful run: `41996708673`

The disposable workflow downloaded and digest-verified the release artifact, then
executed independently authored project files. No upstream implementation
source, tests, fixtures, or documentation examples were copied or translated.

## Observed bounded contract

Default mode and explicit `mode:{plain}` both produced ordinary tables with
trimmed String headers and cells in source order.

A PLAIN cell containing `**bold**` rendered those markers literally. The same
independent input under `mode:{markdown}` rendered a `<strong>bold</strong>`
element, proving that MARKDOWN is a semantically distinct mode and must not be
approximated by the bounded PLAIN adapter.

Clean quoted CSV input preserved a comma inside `"Harbor, Jr."` and decoded a
doubled quote to `a "quote"`. A deliberately different probe with whitespace
before an opening quote was rejected by the official parser; #714 does not use
that failure to widen or narrow the already-bounded #189 parser contract.

Header-only and empty resources both succeeded and emitted a table element with
no materialized rows/cells. This matches the existing #189 `CsvResourceData`
contract, which intentionally leaves `columns` empty until a data row exists.

Compiling `nested/input.qd` with `nested/data.csv` confirmed source-relative
resource resolution. Wrapping `.csv {data.csv}` in `.tablecompute` produced
the original rows plus the aggregate row, confirming that the public producer
is ordinary table content rather than a second representation.

An unknown mode failed strictly. Arkst additionally fails `mode:markdown` and
non-null caption/ref with structured E8001 diagnostics in #714, because those
cross into separately owned content/index behavior.

## Arkst disposition

The bounded implementation must:

- reuse `ResourceProvider` and `load_csv_resource`;
- map the existing evaluator materialization limit into the parser field bound;
- produce the existing `IrNode::Table` only;
- preserve literal PLAIN cell text, order, and atomic resource/parser failures;
- keep source-defined `csv` callables ahead of the newly added native; and
- introduce no host filesystem, network, new IR variant, or backend path.

MARKDOWN conversion, caption/ref/index integration, global-read parity,
pagination/repeated headers, and exact HTML/CSS renderer equivalence remain
outside this evidence claim.
