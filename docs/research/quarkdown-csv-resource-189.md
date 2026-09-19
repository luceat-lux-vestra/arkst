# Quarkdown v2.5.1 CSV resource/parser evidence (#189)

## Scope

This record closes one bounded #189 obligation only: deterministic loading and
parsing of a source-relative CSV resource into raw tabular strings.

It deliberately does **not** implement the public `.csv` callable or table
production. Those layers remain #183-owned:

- `mode:{plain|markdown}` content transformation;
- optional caption and `ref` binding;
- `IrNode::Table` materialization;
- numbering/reference integration;
- callable-facing diagnostics; and
- renderer/output equivalence.

Public WASM/embedder resource exposure remains #191-owned. Absolute/global
filesystem reads remain the accepted `POLICY_DIVERGENCE:global-read`.

## Pinned upstream source

Compatibility target:

- Quarkdown v2.5.1 commit
  `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- `quarkdown-stdlib/.../Data.kt` blob
  `79d8ba2f27835628de5838f7cb8ce1d7d4949631`
- CSV dependency
  `com.github.doyaaaaaken:kotlin-csv-jvm:1.10.0`

The pinned `csv` implementation resolves the path through the common
`file(context, path)` helper, opens the CSV with `csvReader()`, consumes
`readAllWithHeaderAsSequence()`, trims each header/cell, and only creates
table columns while iterating data rows.

That last detail is observable: empty and header-only CSV resources produce a
table with no materialized columns.

## Independent black-box evidence

Disposable PR #376 was closed without merge after running exact official Linux
x64 distributions.

| Target | Official archive SHA-256 |
| --- | --- |
| v2.5.1 | `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9` |
| v2.6.0 | `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4` |

The two versions agreed on the bounded parser semantics relevant here:

- first record supplies headers;
- header and cell contents are trimmed before table content is constructed;
- quoted commas and doubled quote escapes are accepted;
- CRLF is accepted;
- quoted fields may span physical lines;
- every data record must have the same field count as the header;
- short and long records fail;
- duplicate raw header names fail;
- a physical blank data line participates as a one-field record and therefore
  fails against a two-column header;
- empty input succeeds with no materialized columns;
- header-only input succeeds with no materialized columns.

The observed PLAIN output contained literal text. One MARKDOWN fixture confirmed
that Quarkdown transforms cell content before rendering, but that transformation
is #183 evidence and is intentionally not implemented by this #189 slice.

Normal-mode failures were reported without materializing a table while the
process exited successfully; strict mode exited 66. Exception package names and
row-number wording differ between v2.5.1 and v2.6.0, so Arkst does not treat
those strings as a stable compatibility contract.

## Arkst bounded contract

`arkst-engine::csv_resource` is an explicit platform-neutral parser/resource
boundary intended for downstream #183 table producers. It reuses the established
`ResourceProvider::read_text` boundary and returns pure data:

- canonical logical resource path;
- ordered columns;
- trimmed raw header text; and
- trimmed raw cell strings in row order.

The parser uses the workspace-pinned Rust `csv 1.4` crate for standard CSV
record handling and adds fail-closed validation around behavior where the Rust
crate is intentionally more permissive/different than the pinned Quarkdown
dependency.

The accepted subset covers:

- commas;
- RFC-style quoted fields;
- doubled quote escapes;
- LF and CRLF record terminators;
- multiline quoted fields;
- equal record widths; and
- raw duplicate-header rejection before trimming.

The following remain deliberately outside the claimed subset:

- non-standard quote placement and other unproven kotlin-csv leniencies;
- bare CR-only record separators, which were not established by the clean-room probes;
- generalized blank-record parity beyond the independently observed failure
  case; and
- any table/content/output semantics.

Those inputs fail closed rather than being guessed.

## Resource and safety invariants

The loader:

- receives the caller `SourceId` and logical reference;
- performs exactly one `ResourceProvider::read_text` operation;
- preserves the provider-returned canonical logical path;
- propagates provider boundary/not-found/UTF-8 failures without host fallback;
- has no `std::fs`, cwd, native path, process, temp, or network access; and
- requires an explicit `max_fields` bound that counts header plus data fields
  before continued materialization.

The explicit field bound is intentionally independent of host memory. When
#183 consumes this parser from evaluator dispatch, that caller can map the
existing evaluator materialization policy into `max_fields` without changing
the parser/resource ownership boundary.

## Ownership after this slice

After this bounded parser is merged:

- #189 still remains open for its other owned residuals, especially
  `.bibliography` and remaining `.listfiles` compatibility;
- the canonical `builtin:.csv` row is `PARTIAL`, not end-to-end supported;
- #183 owns the remaining public `.csv` callable/table producer path;
- #191 owns public WASM/embedder resource exposure; and
- the global-read incompatibility remains the previously accepted
  fail-closed policy divergence.
