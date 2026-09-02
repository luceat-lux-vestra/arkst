# Typst Backend Compatibility

Arkst does not reimplement the Typst language. The accepted backend path is:

```text
backend-neutral IrDocument
    -> arkst-typst
    -> generated Typst source
    -> concrete Typst compiler adapter
    -> official Typst compiler
```

Typst compatibility therefore means that Arkst's generated source and
backend adapter remain usable with the selected official Typst compiler. It
does not mean that Arkst owns a Typst parser or reproduces Typst's internal
implementation.

Source-language raw HTML is a separate compatibility concern. Typst HTML
facilities do not authorize Arkst to reinterpret arbitrary Markdown or
Quarkdown raw HTML at the backend boundary; see the
[raw HTML compatibility policy](../RAW_HTML_POLICY.md).

## Current evidence

CI currently validates the native subprocess path with Typst **0.15.1**. That
is the current verified Typst compiler evidence point, not a claim that every
Typst release is already compatible. The generated-source corpus and compiler
adapter behavior are the evidence that must advance when the verified version
changes.

Issue #200 promotes the optional native in-process adapter demonstrated by
#187 over `VirtualProject` and Typst 0.15.1's public `World`/compile/PDF APIs.
It is not the default backend: `arkst-cli` includes it only when built with
the `typst-inprocess` Cargo feature, and it is then selected explicitly with
the native CLI's `--backend in-process`. It is not part of the WASM lowering
boundary and is not a browser renderer. It uses `VirtualProject`-only
resources and does not silently fall back to subprocess when explicitly
selected. See
[`docs/research/typst-inprocess-187.md`](../../research/typst-inprocess-187.md)
and [ADR-0021](../../adr/0021-in-process-typst-backend-re-evaluation.md) for
the bounded evidence and promotion gates.

## Source/project context contract

The native subprocess adapter preserves the Arkst source context without
writing generated files into the source tree. `TypstInput.entry_path` is a
normalized, project-root-relative logical path such as `docs/main.qd`.
`TypstSourceContext.project_root` is an explicit physical read boundary; it is
not inferred from the process current directory.

For a context-backed compile, the adapter mirrors the project into a unique
temporary directory, writes generated Typst at the mirrored logical entry
directory as `main.typ`, and invokes the pinned CLI in the following form:

```text
typst compile --root <temporary-mirror> \
  <temporary-mirror>/docs/main.typ <temporary-build>/output.pdf
```

If `docs/main.typ` is already a source resource, the generated entry uses a
reserved collision-free `.typ` filename in the same directory instead.

Consequently, `#image("./assets/logo.svg")`, `#read("./data.txt")`, and
relative Typst imports are resolved from `docs/`, not from the OS temporary
directory. The mirror is a snapshot read context. The original project tree is
never modified, and the PDF remains an isolated temporary artifact until it is
returned by the backend.

The mirror canonicalizes source entries and symlink targets before copying.
Any final target outside the explicit project root is rejected, including file
and directory symlink escapes. A backend without a source context continues to
support self-contained generated Typst; it does not make its temporary
directory an implicit resource root.

## Target and verified version

- **Tracked backend target:** the latest stable Typst release automatically
  becomes the release Arkst must investigate and validate against.
- **Verified backend baseline:** the last Typst release for which the generated
  corpus and concrete adapter have passed the required checks.

The target may be ahead of the verified baseline. That lag is visible backend
compatibility debt and must not be represented as a successful baseline
promotion.

## Required validation surface

Tracking must cover at least:

- Typst syntax emitted by `arkst-typst`;
- changed or deprecated constructs used by lowering;
- compiler CLI behavior used by the subprocess adapter;
- output capabilities relevant to Arkst;
- compiler/backend APIs used by the optional in-process adapter; and
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
Arkst Typst parser merely to follow Typst syntax. Native `.typ` passthrough,
if implemented under its own accepted host policy, should normally be handled
by the selected official compiler rather than reproduced by Arkst.

This document records the target process only. A Typst watcher subsystem is
future implementation work and is not part of this policy PR.
