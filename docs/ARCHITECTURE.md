# Architecture — Scribium

This document describes the accepted target architecture defined by ADR-0014
and ADR-0015. Some target crates are not yet physically extracted from
`scribium-core`; their current physical location during migration does not
change their architectural ownership. Implementation and migration status
must not be confused with target ownership.

## Context Diagram

```
User / CI
    |
    v
scribium-cli
    |
    +----> scribium-project
    |         |
    |         +---- constructs VirtualProject
    |
    +----> scribium-core
    |         |
    |         +---- compiles VirtualProject
    |                    |
    |                    +---- scribium-markdown
    |                    |          |
    |                    |          +----> scribium-quarkdown
    |                    |
    |                    +---- scribium-engine
    |                               |
    |                               +----> scribium-html
    |                               |          |
    |                               |          +---- HTML semantics / foreign content
    |                               v
    |                         normalized IrDocument
    |                               |
    |                               v
    |                         CompileResult
    |                         (normalized IR + shared diagnostics)
    |
    +----> scribium-typst
    |         |
    |         +---- lowers normalized IrDocument
    |         +---- Typst source
    |         +---- source map
    |         +---- lowering diagnostics
    |
    +----> scribium-typst-subprocess
              |
              +---- optional native Typst execution

Shared lower-level target crates:
  scribium-source       source identity, spans, source-map representation
  scribium-diagnostics  shared diagnostic representation
  scribium-compat       compatibility policy
  scribium-ir           backend-neutral document IR
  scribium-html         HTML interoperability adapter
```

The shared lower-level crates are dependencies of the stages that use them;
their implementations are not owned by `scribium-core`.
The native CLI/host composes `scribium-core` compilation, `scribium-typst`
lowering, and optional `scribium-typst-subprocess` execution. `scribium-core`
does not depend on `scribium-typst`.

## Compile Pipeline

```
VirtualProject
  │
  ▼
core selects entry source, metadata defaults, and compatibility options
  │
  ▼
Source text + SourceId
  │
  ▼
Markdown physical-line scanner / classifier
  └── front matter framing at document start
  │
  ▼
scribium-markdown BlockParser
  ├── Markdown blocks: headings, paragraphs, lists, code, tables, etc.
  ├── invokes scribium-quarkdown only for Quarkdown call/directive grammar
  └── owns document-context and body/container decisions
  │
  ▼
Markdown frontend AST
  │
  ▼
scribium-engine: AST normalization
  │
  ├── delegates raw HTML normalization to scribium-html
  │     ├── structured xberg result → Scribium semantics
  │     └── unsupported content → explicit foreign HTML content when needed
  │
  ▼
initial IrDocument
  │
  ▼
scribium-engine: semantic / evaluation / normalization passes
  ├── scope and name resolution
  ├── variables, function/component calls, and built-ins
  ├── conditional branching and iteration
  ├── compatibility policy application
  └── resource limit enforcement
  │
  ▼
CompileResult: normalized backend-neutral IrDocument + shared diagnostics
  │
  │ host composes the next stage
  ▼
scribium-typst lowering
  ├── Typst source
  ├── source map
  └── lowering diagnostics
  │
  ▼
optional scribium-typst-subprocess
  │
  ▼
Typst compiler output
```

## Markdown Frontend Boundary

The Markdown frontend uses a physical-line scanner/classifier as its lexical
layer. The implementation currently provides this through `SourceLine` and
`split_lines` in `syntax/markdown/parser.rs`; there is no separate generic
tokenizer or token stream.

ADR-0014 establishes the following frontend ownership target. PR #46 does
not change the physical crate layout or public parser API:

```text
source/span primitives
        ↓
scribium-markdown frontend
        ↓
scribium-markdown::BlockParser
  ├── one source position
  ├── one open-container stack
  ├── one open-leaf state
  └── one diagnostic/source-mapping sink
        ↓
pure Markdown + Quarkdown block candidates
        ↓
Markdown frontend AST
        ↓
scribium-engine
        ↓
initial IrDocument
        ↓
semantic / evaluation / normalization
        ↓
normalized IrDocument
```

The target `BlockParser` in `scribium-markdown` owns container continuation,
block interruption, paragraph/lazy continuation, fence lifecycle, body
collection, and source mapping. `scribium-markdown` recognizers classify
Markdown candidates and may invoke `scribium-quarkdown` for call grammar, but
neither recognizer owns parser state. `scribium-markdown` decides whether a
Quarkdown call participates as block or inline and owns any following body;
`scribium-quarkdown` owns only Quarkdown grammar and must not depend on
Markdown parser or AST types. This is a first-party Scribium integration, not
a plugin API or generic extension framework.

Raw inline and block HTML is recognized by `scribium-markdown` at the syntax
level. The frontend preserves the original HTML content, its block or inline
context, and its original `SourceSpan` in the frontend AST. It does not depend
on xberg, convert HTML to Typst, generate synthetic Markdown, or recursively
parse an HTML-to-Markdown string. HTML semantic normalization belongs to
`scribium-engine`'s delegation to `scribium-html`.

The target module layout and migration status are design work under
`docs/adr/0014-markdown-block-parser-foundation.md`; blockquote behavior is
intentionally not enabled by the foundation refactor.

## Crate Boundaries

| Crate                    | Target responsibility                                                    | WASM |
|--------------------------|--------------------------------------------------------------------------|------|
| scribium-source          | source identity, spans, source-map representation                        | Yes  |
| scribium-project         | VirtualProject, source/asset stores, project metadata                    | Yes  |
| scribium-quarkdown       | Quarkdown grammar                                                        | Yes  |
| scribium-markdown        | Markdown frontend, AST, BlockParser                                      | Yes  |
| scribium-diagnostics     | shared diagnostic representation                                         | Yes  |
| scribium-compat          | Quarkdown compatibility policy                                           | Yes  |
| scribium-ir              | backend-neutral document IR                                              | Yes  |
| scribium-engine          | AST→IR lowering, semantic/evaluation/normalization, built-ins            | Yes  |
| scribium-html            | HTML fragment→backend-neutral Scribium semantics/IR adapter             | Yes  |
| scribium-core            | public facade and compiler orchestration                                 | Yes  |
| scribium-typst           | pure IR→Typst lowering and source-map generation                         | Yes  |
| scribium-typst-subprocess | native Typst subprocess adapter                                          | No   |
| scribium-cli             | native host, filesystem/config/output composition                         | No   |
| scribium-test-support    | fixtures/test utilities                                                   | No   |

These are target architectural boundaries. Physical workspace extraction is
a subsequent migration.

## Platform Independence

All platform-independent compiler/library crates in the table marked Yes must
remain filesystem/network/process independent and compile for
`wasm32-unknown-unknown`.

Native host/adapter crates such as:

- `scribium-cli`
- `scribium-typst-subprocess`
- `scribium-test-support`

are not subject to that requirement.

### Forbidden in core crates

- `std::fs`, `std::process`, `std::env` — OS-specific APIs never used
- `TcpStream` — no network access
- System clock dependency
- Global mutable state
- `std::path::PathBuf` in public API — use `VirtualPathBuf` instead

### VirtualProject: I/O-Free Project Model

`scribium-project` owns the in-memory project model and its supporting types:

- `VirtualProject`
- `VirtualProjectBuilder`
- `SourceStore`
- `AssetStore`
- `VirtualPathBuf`
- `ProjectMetadata`

`VirtualProject` is the in-memory compilation project model. The native CLI or
another native host loads filesystem, configuration, and resource data and
constructs it. WASM and embedded hosts construct it directly from in-memory
inputs. `scribium-core` consumes an already constructed `VirtualProject`.
Project ownership does not prevent the core facade from accepting
`&VirtualProject`.

```rust
pub struct VirtualProject {
    entry: VirtualPathBuf,
    sources: SourceStore,
    assets: AssetStore,
    metadata: ProjectMetadata,
}

// Constructed only through the fluent builder:
VirtualProjectBuilder::new()
    .entry("main.qd")?
    .add_source("main.qd", "...")?
    .add_source("chapter/intro.qd", "...")?
    .add_asset("fonts/main.otf", data)
    .build()?;

project.entry();
project.sources();
project.assets();
project.metadata();

pub fn compile(
    project: &VirtualProject,
    options: &CompileOptions,
) -> CompileResult;
```

Ownership of the I/O boundary is explicit:

- filesystem discovery, reads, writes, and native output handling belong to
  the native CLI/host;
- OS-path canonicalization and symlink enforcement belong to the native
  CLI/host;
- `VirtualPathBuf`, `SourceStore`, `AssetStore`, and `VirtualProject` belong to
  `scribium-project`; and
- compiler orchestration belongs to `scribium-core`.

`scribium-project` and `scribium-core` remain filesystem-free. The host
acquires the required inputs and applies native filesystem policy before core
compilation.

- CLI builds `VirtualProject` from disk
- WASM builds `VirtualProject` from in-memory sources
- Core never touches filesystem
- SourceId assignment is deterministic (sources sorted by path before insertion)
- Front matter at document start is parsed and merged with project metadata
- Front matter is a flat, line-based `key: value` format, not full YAML:
  nested objects, arrays, and block strings are not supported
- Keys and values are split on the first colon; empty keys reject the block
- Metadata lines must start at column 0: indentation is not stripped and
  indented keys reject the whole block (no nested-object flattening)
- Duplicate keys use last-wins semantics (last occurrence wins)
- User-defined metadata is stored in the IR in deterministic
  (lexicographic key) order
- Malformed front matter blocks (indented delimiters, indented keys, lines
  without colons, empty keys) are rejected and treated as regular Markdown
- Supported CLI inputs are `.qd`, `.scrib`, `.md`; `.typ` is rejected until
  Typst passthrough is implemented. Extension matching is ASCII
  case-insensitive; files without an extension are rejected.
- Typst default output path replaces file extension with `.typ`; the build
  refuses to write an output that resolves to the same file as the input.
  Existing outputs are compared by file identity (device/inode on Unix, file
  index on Windows), so symlink and hard-link aliases of the input are also
  rejected; non-existent outputs are compared by canonicalized parent plus
  normalized file name. The check is repeated immediately before writing.
- Missing output parent directories are created (`create_dir_all`) before
  writing; the output path is then resolved against the real (canonicalized)
  parent and the same-file check runs against that resolved path immediately
  before the write, so `.`/`..` components and symlinks in the output path
  are interpreted after directory creation. Before that, a side-effect-free
  pre-validation resolves the requested output path in component order
  (left to right, starting from the real working directory), canonicalizing
  the path-so-far whenever it exists so symlinks resolve `as reached` and a
  `..` after a symlink moves to the symlink target's parent; only the
  non-existent suffix is kept on an in-memory stack (`..` canceling a
  non-existent component never creates anything). Output paths whose real
  resolution is the input (e.g. `new/../document.qd` or
  `a/b/../../document.qd`, even when the intermediate directories do not
  exist yet) are rejected *before* any directory is created, so a rejected
  build leaves no empty directories behind — while distinct targets behind
  a symlink (e.g. `link/../document.qd` with `link -> ../other/subdir`
  resolving to `other/document.qd`) are accepted. The canonicalized
  same-file check below remains the authoritative guard for symlink and
  hard-link aliases. Output is written
  atomically: the content goes to a uniquely named temporary file in the
  output directory — created exclusively with `create_new(true)`, retrying
  up to 32 candidate names (each includes the PID and an in-process counter)
  when a candidate is already taken, and touching only files this call
  created — is flushed and synced, then renamed over the output path; on an
  error return the temporary file is removed and any previous output is left
  untouched. On Unix the replacement is `rename(2)` (a symlink at the output
  path is replaced, not followed); on Windows it uses `MoveFileExW` with
  `MOVEFILE_REPLACE_EXISTING`, whose symlink replacement semantics differ —
  the output is verified not to alias the input source file before writing
  on both platforms.
- Atomicity scope: the rename guarantees readers never observe partial
  content, but this is *not* a crash-durability guarantee — the output
  directory is not fsynced, so power loss may not preserve the newest file,
  and an abrupt process kill can leave a temporary file behind (normal
  error-return paths remove it).
- Permissions (Unix): the temporary file is created with `OpenOptions` plus
  `create_new(true)`, which applies the standard `0666 & !umask` mode (same as
  `std::fs::write`). When an output file already exists, its permission bits
  are copied to the replacement first, so re-running a build never silently
  changes an existing output mode (e.g. from `0640` to a temp file's `0600`).
  Windows has no Unix mode semantics and is left untouched.
### Virtual Paths

Internal paths are logical, not OS paths (`"chapter/intro.qd"`).
The native CLI adapter resolves OS paths (canonicalization, symlink resolution)
and maps them into project-relative `VirtualPathBuf` values.
Symlink handling is a CLI adapter responsibility; the core only sees virtual paths.

### Symlink Security Boundary

The CLI adapter enforces a strict symlink containment policy:

* **Logical project root**: Derived from the user-provided input path (before canonicalization).
* **Physical project root**: Canonicalized logical project root.
* **Symlink containment check**: Before reading a file, the CLI canonicalizes the input path and verifies it lies within the canonicalized physical project root. If a symlink points outside the project root, the operation fails with a clear error message.
* **Output path**: Computed from the user-provided logical path, preserving the original filename and directory structure. Symlinks do not affect output location.

This design ensures:

* A WASM frontend (which has no filesystem access) is inherently immune to symlink escape attacks.
* Native CLI users are protected from accidental or malicious symlink escapes.
* The `VirtualProject` abstraction remains purely logical, with no OS path leakage.

### Synchronous Core, Async Host

The host gathers all required filesystem, network, and resource input before
core compilation. It constructs or updates the complete in-memory
`VirtualProject`, then `scribium-core` performs synchronous, deterministic
compilation over that project. Host-side acquisition may itself be
asynchronous, but lower compiler crates do not request missing sources through
callbacks or asynchronous compiler APIs.

### WASM Editions

| Edition | Scope | Status |
|---------|-------|--------|
| Compiler/library WASM | In-memory `VirtualProject` → frontend → engine (including `scribium-html` HTML normalization) → normalized IR → pure Typst lowering | Guaranteed target |
| Full browser compile | Above + Typst compiler running in WASM → PDF/output | M7+ feasibility gate |

The guaranteed compiler/library path includes pure `scribium-typst` lowering;
it does not include `scribium-typst-subprocess`. Subprocess execution is
native-only. Running the Typst compiler in a browser remains a later
feasibility decision; this architecture does not introduce a browser adapter
or an in-process backend.

## Source Span Model

`scribium-source` owns the source-location primitives and the backend-neutral
source-map representation. Its target responsibilities include:

- `SourceId`, the identity of an original source;
- `ByteSpan`, a byte range within source text;
- `SourceSpan`, an original-source identity plus byte range;
- `LineColumn`, the line and column view of a source position;
- project-independent byte/span conversion primitives;
- generated-output range primitives; and
- the backend-neutral representation of source maps.

`SourceSpan` identifies an original source location using `SourceId` plus a byte
range. AST nodes and IR nodes preserve their original source spans through the
frontend, engine, and lowering stages. Diagnostics use `SourceSpan` when an
original source location is available. A primary source span is not mandatory
for every diagnostic: project, backend, and internal diagnostics may have no
corresponding original source range.

The location types remain distinct:

```
original source location
    -> SourceId + SourceSpan

generated backend output location
    -> generated range

source map
    -> generated range -> original SourceSpan
```

Source-map entries are created when backend lowering emits generated output.
The source-map representation belongs to `scribium-source`; generated mappings
do not belong to `scribium-ir`. This section describes the model without
defining exact Rust structs.

## IR Model

Scribium IR is a backend-neutral document representation. Its purpose is to
separate Scribium language semantics from any concrete output backend.

The pipeline has one IR model in the current target architecture:

```
Markdown frontend AST
        ↓
scribium-engine
        ↓
initial IrDocument
        ↓
semantic / evaluation / normalization
        ↓
normalized IrDocument
        ↓
backend lowering
```

An `IrDocument` may therefore be at an earlier or later stage of semantic
normalization; IR values are not inherently all already evaluated. The target
architecture does not introduce HIR/MIR or separate evaluated and unevaluated
IR crates.

`scribium-ir` owns the backend-neutral IR model, including the architectural
equivalents of `IrDocument`, `IrMetadata`, `IrNode`, `IrInline`, `IrListItem`,
and `IrValue`. Illustrative Scribium semantic/document concepts represented by
the IR include:

- headings and paragraphs;
- ordered and unordered lists;
- code blocks and thematic breaks;
- math and links;
- inline formatting;
- semantic function/component calls;
- resolved values; and
- document metadata.

This list is illustrative, not a requirement to add missing variants. The
target IR contains Scribium semantics, not backend-specific output fragments:

```
IrDocument
├── IrMetadata
└── semantic content
    ├── IrNode
    ├── IrInline
    ├── IrListItem
    └── IrValue
```

A semantic function/component-call node represents a Scribium/Quarkdown
semantic operation, not pre-generated Typst source. The Typst backend may lower
that semantic node into an appropriate Typst construct; retaining the semantic
operation does not make the IR Typst-specific.

IR nodes preserve their original `SourceSpan`, but generated-output source-map
entries are not stored in `IrDocument` or `IrNode`:

```
IrDocument
    |
    | original SourceSpan values only
    v
scribium-typst lowering
    |
    +---- generated Typst source
    +---- source-map entries
```

The source-map entries use the representation owned by `scribium-source` and
are created as lowering emits generated output. A backend-specific source
fragment must not cross backward into `scribium-ir`; `scribium-ir` expresses
Scribium semantics and `scribium-typst` translates those semantics into Typst.

### Migration Note

The current physical implementation still contains `IrNode::RawTypst`. This is
a migration artifact only and does not represent accepted target ownership. It
must be removed or eliminated during the later physical crate/IR migration;
PR #46 does not decide or implement that code migration.

## HTML Interoperability Policy

`scribium-html` is the target first-party HTML interoperability boundary. It
converts raw HTML fragments preserved by `scribium-markdown` into
backend-neutral Scribium semantics/IR; it is not a renderer, a Typst-specific
crate, or a generator of Typst source. HTML normalization occurs before
rendering/backend code generation:

```text
Markdown / Quarkdown source
        ↓
scribium-markdown
        ↓ raw HTML content + block/inline context + original SourceSpan
frontend AST
        ↓
scribium-engine
        ↓ delegates HTML normalization
scribium-html
        ↓
backend-neutral Scribium IR
        ↓
scribium-typst
```

The frontend recognizes CommonMark/Markdown syntax and preserves raw HTML
syntax and provenance. It does not depend on xberg, convert HTML to Typst,
reconstruct Markdown strings, or recursively parse synthetic Markdown. The
engine invokes `scribium-html` for HTML requiring semantic normalization.

The selected implementation library is `xberg-io/html-to-markdown`, isolated
inside `scribium-html`. The adapter consumes its structured conversion result
or equivalent structural API, including semantic document structure and
visitor/customization facilities, and translates it directly into Scribium
semantics. The architecture forbids an HTML → xberg Markdown string →
`scribium-markdown` parser round-trip. xberg types do not cross the
`scribium-html` public boundary.

Supported HTML is mapped to existing backend-neutral concepts where the
mapping is faithful, including concepts equivalent to paragraphs, headings,
strong/emphasis, code, links, lists, tables, and line breaks. The supported-tag
matrix and exact Rust API are deferred. When faithful normalization is not
possible, the IR may represent foreign input content conceptually as:

```text
ForeignContent
    format = Html
    original content
    original provenance/span
```

This is allowed for HTML input but does not introduce `RawTypst`, `BackendRaw`,
or a generic backend-code escape hatch; those remain forbidden in
backend-neutral IR. `scribium-typst` must handle unsupported foreign HTML
explicitly according to the eventual compatibility/lowering policy. It must
never paste HTML into Typst source, interpret HTML as Typst syntax, or silently
discard it. The exact diagnostic code is not defined here.

The original HTML fragment's `SourceSpan` remains authoritative. Child nodes
produced by third-party normalization must not claim fabricated byte-precise
spans when xberg offsets do not correspond to the original `.qd` source;
fragment-level provenance is permitted until a later source-mapping enhancement.
Mixed inline Markdown/HTML must preserve existing Markdown children and HTML
provenance without guessed ranges. If faithful conversion is unavailable,
foreign HTML and the appropriate compatibility/lowering diagnostic preserve the
meaning.

Target dependencies are:

```text
scribium-engine -> scribium-html
scribium-html -> scribium-source
scribium-html -> scribium-ir
scribium-html -> scribium-diagnostics
scribium-html -> xberg-io/html-to-markdown  (implementation only)
```

`scribium-html` must not depend on `scribium-engine`, `scribium-markdown`,
`scribium-core`, `scribium-project`, `scribium-typst`, or
`scribium-typst-subprocess`; `scribium-markdown` and `scribium-typst` must not
depend on xberg. `scribium-html` is part of the WASM-compatible compiler path
and must remain free of native filesystem, process, and network requirements.

Pandoc is an optional externally installed development/compatibility oracle,
not a Scribium dependency. It may provide differential evidence, native
AST/JSON comparisons, expected-output investigation, or compatibility
fixtures. Pandoc behavior is reference evidence rather than the Scribium
specification; accepted CommonMark, Quarkdown, and Scribium ADR contracts win
when they conflict. Pandoc is not linked, vendored, required to build, needed
for normal unit tests, used at runtime, or used as a production subprocess.
Any future oracle tests must be isolated from the normal deterministic suite
and use an explicitly controlled/pinned Pandoc version. Pandoc is not part of
the WASM path.

## Typst Backend Interface

```rust
pub trait TypstBackend {
    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, TypstError>;
    fn version(&self) -> Result<String, TypstError>;
}

pub struct TypstInput {
    pub source: String,
    pub entry_path: PathBuf,
    pub assets: Vec<Asset>,
    pub fonts: Vec<Font>,
    pub packages: Packages,
}

pub struct TypstOutput {
    pub pdf: Option<Vec<u8>>,
    pub html: Option<String>,
    pub svg: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
    pub diagnostics: Vec<TypstDiagnostic>,
    pub duration: Duration,
}
```

## Error Model

```
Diagnostic
├── code: "E0001" (stable diagnostic code)
├── severity: Error | Warning | Hint
├── message: "concise description"
├── primary_span: SourceSpan
├── secondary_spans: Vec<SourceSpan>
├── hints: Vec<String>
├── include_stack: Vec<SourceLocation>
└── expansion_stack: Vec<CallLocation>

Error codes:
  E1xxx - Syntax
  E2xxx - Semantic
  E3xxx - Evaluation
  E4xxx - Lowering
  E5xxx - Typst backend
  E6xxx - Project/config
  E7xxx - IO/assets
  E8xxx - Compatibility
  E9xxx - Internal invariant
```

## Configuration Model

```toml
# scribium.toml (project-level)
[project]
name = "my-doc"
root = "."
entry = "src/main.qd"

[output]
targets = ["pdf", "html"]
dir = "out"

[typst]
backend = "subprocess"
packages = []

[resources]
max_source_size = "10MB"
max_include_depth = 16
max_evaluation_steps = 100000
max_loop_iterations = 10000
max_recursion_depth = 64

[compatibility]
profile = "quarkdown-v2.5"
strict = false
```

## Security Boundaries

- No shell execution from document source
- No network access by default
- Filesystem access scoped to project root
- Absolute includes denied by default
- Symlink escape denied
- Resource limits enforced at evaluation time
- No hidden global mutable state
- Generated Typst output is deterministic
