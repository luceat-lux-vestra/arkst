# Quarkdown v2.7.0 Markdown `.csv` clean-room evidence

Issue: #716  
Date: 2026-11-05

## Public contract

The current public Quarkdown reference documents
`csv(path: String, mode: CsvParsingMode = PLAIN, caption: InlineMarkdownContent? = null, ref: String? = null) -> Node`.
#714 implemented PLAIN; #716 isolates explicit `mode:{markdown}` cell/header transformation.

## Reference artifact

Independent Arkst-authored probes used the official Quarkdown v2.7.0 Linux x64 release:
- SHA-256: `5ff4e78e391abd073e62030da816faef265d681d3b8ccfd05768cc9c586f7e85`
- runtime: `quarkdown version 2.7.0`
- successful evidence runs: `42010085395`, `42010109091`

No upstream implementation source, tests, fixtures, or documentation examples were copied or translated.

## Observed contract

- strong/emphasis in headers and cells become structured inline content;
- links preserve destinations and code spans preserve literal code;
- escaping preserves literal Markdown marker text;
- quoted fields may contain Markdown and commas;
- ordinary quoted-field newlines remain inline flow; two-space Markdown newlines emit a hard break;
- block-looking `# Heading`, `- item`, `> quote`, and `| A | B |` remain literal cell text;
- empty/header-only resources preserve the existing empty table node;
- nested sources resolve CSV paths relative to the active source;
- table-column extraction, sorting, and filtering consume the rich table through existing plain-text projection while returned rich cells remain structured;
- tablecompute preserves rich input cells; content-backed callback result stringification remains the separate nondeterministic #712 boundary;
- unknown modes fail strictly.

## Arkst disposition

The existing `IrTableCell { content: Vec<IrInline>, ... }` model is sufficient for these observations. #716 therefore uses the existing Quarkdown inline parser only at the CSV cell producer boundary and admits the independently evidenced closed subset: text, emphasis, strong, code, links, and soft/hard line breaks.

This is not generic String-to-Markdown conversion. Representable but unevidenced inline categories that reach the CSV closed-set guard (for example strikethrough) fail closed with `E8001`. Categories already rejected by the existing Quarkdown inline conversion boundary, such as image syntax in Quarkdown mode, retain that earlier `E3001` diagnostic instead of being reclassified by the CSV producer. Non-null caption/ref, numbering/index integration, pagination/repeated-header parity, exact renderer parity, global-read parity, remaining parser edges, and public WASM binding remain outside this slice. `.csv` remains `PARTIAL`.
