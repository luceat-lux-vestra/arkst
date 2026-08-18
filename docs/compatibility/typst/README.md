# Typst Backend Compatibility

Scribium does not reimplement the Typst language. The accepted backend path is:

```text
backend-neutral IrDocument
    -> scribium-typst
    -> generated Typst source
    -> concrete Typst compiler adapter
    -> official Typst compiler
```

Typst compatibility therefore means that Scribium's generated source and
backend adapter remain usable with the selected official Typst compiler. It
does not mean that Scribium owns a Typst parser or reproduces Typst's internal
implementation.

Source-language raw HTML is a separate compatibility concern. Typst HTML
facilities do not authorize Scribium to reinterpret arbitrary Markdown or
Quarkdown raw HTML at the backend boundary; see the
[raw HTML compatibility policy](../RAW_HTML_POLICY.md).

## Current evidence

CI currently validates the native subprocess path with Typst **0.15.1**. That
is the current verified Typst compiler evidence point, not a claim that every
Typst release is already compatible. The generated-source corpus and compiler
adapter behavior are the evidence that must advance when the verified version
changes.

## Target and verified version

- **Tracked backend target:** the latest stable Typst release automatically
  becomes the release Scribium must investigate and validate against.
- **Verified backend baseline:** the last Typst release for which the generated
  corpus and concrete adapter have passed the required checks.

The target may be ahead of the verified baseline. That lag is visible backend
compatibility debt and must not be represented as a successful baseline
promotion.

## Required validation surface

Tracking must cover at least:

- Typst syntax emitted by `scribium-typst`;
- changed or deprecated constructs used by lowering;
- compiler CLI behavior used by the subprocess adapter;
- output capabilities relevant to Scribium;
- compiler/backend APIs if a future in-process adapter is accepted; and
- diagnostic and source-map implications where relevant.

The intended future pipeline is:

```text
new stable Typst release
    -> release/change detection
    -> official release notes and documented compatibility changes
    -> generated-Typst corpus validation
    -> compile with the new stable compiler
        -> success: compatibility evidence
        -> failure: classify lowering/adapter impact and prepare an adaptation PR
```

The future watcher should add a machine-readable verified baseline, stable
release detection, generated-source corpus execution, deduplicated failure
reporting, and eventually adaptation-PR preparation. It must not create a
Scribium Typst parser merely to follow Typst syntax. Native `.typ` passthrough,
if implemented under its own accepted host policy, should normally be handled
by the selected official compiler rather than reproduced by Scribium.

This document records the target process only. A Typst watcher subsystem is
future implementation work and is not part of this policy PR.
