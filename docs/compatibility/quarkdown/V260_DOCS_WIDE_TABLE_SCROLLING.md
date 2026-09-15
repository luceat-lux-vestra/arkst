# Quarkdown v2.6 docs wide-table scrolling

Issue: #346  
Parent migration tracker: #311  
Future HTML-output owners: #320 and #347

## Decision

The Quarkdown v2.6 behavior that wide tables in `docs` documents scroll
horizontally is **not applicable to Arkst's current output surface**.

This is a current-surface classification, not a permanent compatibility
exclusion. Arkst currently exposes whole-document `typst` and `pdf` build
outputs only. Once Arkst exposes a first-class HTML target, the equivalent
observable table-overflow behavior becomes a conformance obligation of that
HTML output path under #320/#347.

No evaluator, backend-neutral IR, Typst lowering, CSS, DOM, or CLI behavior is
changed by this classification.

## Upstream v2.6 evidence

The official Quarkdown v2.6.0 release notes list the fix that wide tables in
`docs` documents scroll horizontally within their content area.

The clean-room black-box probe in disposable PR #348 used the official
v2.6.0 Linux x64 distribution only:

- asset: `quarkdown-linux-x64.zip`;
- SHA-256:
  `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`;
- observed binary identity: `quarkdown version 2.6.0`;
- independently authored fixtures: one deliberately wide `docs` table and one
  narrow `docs` control table.

Successful probe runs were `34923037835`, `34923115679`, and `34923203923`.
No Quarkdown implementation source was inspected.

### Observable artifact behavior

Both fixtures produced ordinary static `<table>` elements. The static HTML did
not encode a source-language distinction between a "wide table" and a narrow
table.

The generated `theme/global.css` was byte-identical for the wide and narrow
outputs, with SHA-256:

`3dc51d04733092bae1d61b7d0f863c444f82568224c7ebd531dc59e822756d3e`

That generated CSS contains the docs-specific rule:

```css
.quarkdown-docs .table-scroll-area {
  overflow-x: auto;
}
```

When the generated artifacts were loaded in headless Chromium, runtime DOM for
**both** fixtures wrapped the table in a `table-scroll-area` container. The
wide fixture was observed as:

```html
<div class="table-scroll-area"><table>
```

and the narrow control received the same wrapper. `overflow-x: auto` therefore
makes scrolling conditional on actual overflow at layout time; the compiler
does not need to classify a table as wide in source semantics or IR.

These observations establish the compatibility boundary without inferring how
Quarkdown implements it internally: the behavior is an HTML artifact/runtime
layout concern, not evidence for a new backend-neutral document semantic.

## Arkst current-surface audit

Fresh `main` at the time of this classification was
`5401b162e51a6d23f380a30d8fcce94aedbea022`.

The current Arkst host/output contract is explicit:

- `arkst build --format` documents `typst` and `pdf` as the implemented output
  formats and states that HTML/SVG/PNG output is not yet implemented;
- `crates/arkst-cli/src/commands.rs` accepts only `typst` and `pdf` in its
  current `SUPPORTED_FORMATS` boundary;
- existing CLI integration coverage rejects unsupported HTML/SVG/PNG output
  requests instead of silently producing another format;
- the current Typst subprocess path publishes PDF, while generated Typst
  source remains a separate Arkst lowering output;
- support for bounded native/foreign HTML content inside a document does not
  constitute a whole-document HTML renderer or HTML artifact contract.

Therefore Arkst has no current first-party output artifact on which the v2.6
browser scrolling behavior can be observed.

## Ownership

This behavior must **not** be represented by adding browser/CSS state to the
parser, evaluator, backend-neutral IR, or current PDF lowering.

Future ownership is split as follows:

- #347 owns the typed Typst compiler-target/capability and multi-artifact
  contract needed before rendered targets such as experimental Typst HTML are
  first-class Arkst outputs;
- #320 owns the HTML semantic/output architecture and the decision about the
  bounded HTML rendering/packaging path;
- once a whole-document HTML target is exposed, its docs-table conformance
  must verify an equivalent observable result: a table wider than its content
  area remains horizontally accessible without forcing the whole document
  layout wider, while a narrow table does not require visible horizontal
  scrolling.

The future conformance obligation is behavioral. Arkst does not need to copy
Quarkdown's `table-scroll-area` class name or DOM implementation if the same
observable result is provided by the selected HTML backend.

## Regression boundary

The existing current-surface regression contract is intentionally retained:
HTML is unsupported at the Arkst CLI build boundary and fails closed rather
than being accepted as a dead/no-op format. This classification adds no new
HTML flag, renderer shim, CSS asset, or semantic IR state.

When #320/#347 make HTML an actual output target, that change must replace the
current unsupported-format expectation with target-specific conformance tests,
including the wide-table `docs` fixture described above.

## Migration bookkeeping

This document resolves the ownership/classification work in #346. It does not
edit `V260_MIGRATION_CHECKLIST.md` in the same change. Per the v2.6 migration
process, the checklist row should be marked complete as an evidence-backed
current-surface N/A only in a separate post-merge bookkeeping PR after this
classification lands.
