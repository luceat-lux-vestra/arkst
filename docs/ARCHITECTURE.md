# Architecture — Scribium

## Context Diagram

```
┌────────────────────────────────────────────────────────┐
│                      User / CI                          │
│  scribium build | check | inspect | watch              │
└────────────────────┬───────────────────────────────────┘
                     │ CLI arguments, config (scribium.toml)
                     ▼
┌────────────────────────────────────────────────────────┐
│                     scribium-cli                        │
│  Command dispatch, config loading, filesystem I/O       │
│  Exit codes, human/JSON diagnostics output             │
└────────────────────┬───────────────────────────────────┘
                     │ CompileRequest
                     ▼
┌────────────────────────────────────────────────────────┐
│                     scribium-core                       │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │  Source      │  │  Semantic    │  │  Evaluator    │ │
│  │  Abstraction │  │  Analysis    │  │               │ │
│  │  Spans       │  │  Scope       │  │  Built-ins    │ │
│  └──────┬───────┘  └──────┬───────┘  └───────┬───────┘ │
│         │                 │                   │         │
│  ┌──────▼─────────────────▼───────────────────▼───────┐ │
│  │                    IR (Intermediate Representation) │ │
│  └──────────────────────┬─────────────────────────────┘ │
│                         │                               │
│  ┌──────────────────────▼─────────────────────────────┐ │
│  │              Source Map                             │ │
│  │  Original positions ↔ Generated positions           │ │
│  └────────────────────────────────────────────────────┘ │
│                                                         │
│  ┌────────────────────────────────────────────────────┐ │
│  │  compatibility/ (profile, divergence, diagnostics) │ │
│  └────────────────────────────────────────────────────┘ │
└────────────────────┬───────────────────────────────────┘
                     │ TypstDocument
                     ▼
┌────────────────────────────────────────────────────────┐
│                     scribium-typst                      │
│  ┌──────────────────────────────────────────────────┐  │
│  │  TypstLowering                                    │  │
│  │  IR → Typst source code                           │  │
│  │  Source map updates                               │  │
│  └──────────────────────┬───────────────────────────┘  │
│                         │ Typst source (.typ)           │
│  ┌──────────────────────▼───────────────────────────┐  │
│  │  TypstBackend (trait)                             │  │
│  │  Subprocess adapter | InProcess adapter (future) │  │
│  │  → Typst compiler                                 │  │
│  │  → PDF / HTML / SVG / PNG                        │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

## Compile Pipeline

```
Source Text (String)
  │
  ▼
Source Abstraction (SourceId + SourceText)
  │
  ▼
Physical-line scanner / classifier
  (SourceLine + source/span primitives)
  │
  ▼
Markdown block parser
  (current recursive parser; BlockParser state proposed in ADR-0014)
  ├── Markdown blocks: headings, paragraphs, lists, code, tables, etc.
  ├── Quarkdown directives: .function, .function {arg} name:{value}
      (indented block bodies)
  ├── Expressions: literals, variables, function calls, conditionals
  └── Front matter: flat key-value metadata block
  │
  ▼
Semantic Analysis
  ├── Scope resolution
  ├── Name binding
  ├── Type checking (basic)
  └── Compatibility profile application
  │
  ▼
Evaluator
  ├── Literal evaluation
  ├── Variable lookup
  ├── Function/component call
  ├── Conditional branching
  ├── Iteration
  └── Resource limit enforcement
  │
  ▼
IR (Document IR)
  ├── Typst-oriented nodes
  ├── Content blocks
  ├── Metadata
  └── Source span annotations
  │
  ▼
Typst Lowering
  ├── IR → Typst code generation
  ├── Source map recording
  └── Escape handling
  │
  ▼
Typst Backend (trait)
  └── Subprocess adapter → typst compile
      └── PDF / HTML / SVG / PNG + diagnostics
```

## Markdown Frontend Boundary

The Markdown frontend uses a physical-line scanner/classifier as its lexical
layer. The implementation currently provides this through `SourceLine` and
`split_lines` in `syntax/markdown/parser.rs`; there is no separate generic
tokenizer or token stream.

ADR-0014 proposes the following internal target without changing the crate
boundary or public parser API:

```text
source/span primitives
        ↓
markdown::block::line
        ↓
markdown::block::parser::BlockParser
  ├── one source position
  ├── one open-container stack
  ├── one open-leaf state
  └── one diagnostic/source-mapping sink
        ↓
pure Markdown + Quarkdown block candidates
        ↓
Markdown AST
        ↓
ast_to_ir → evaluator → IR
```

The target `BlockParser` owns container continuation, block interruption,
paragraph/lazy continuation, fence lifecycle, body collection, and source
mapping. Markdown and Quarkdown recognizers classify candidates but do not own
parser state. Quarkdown call grammar remains in `syntax/quarkdown`; standalone
call/body ownership remains in the block parser. Quarkdown syntax is part of
Scribium core, not a plugin.

The target module layout and migration status are design work under
`docs/adr/0014-markdown-block-parser-foundation.md`; blockquote behavior is
intentionally not enabled by the foundation refactor.

## Crate Boundaries

| Crate               | Responsibility                                          | WASM? |
|---------------------|---------------------------------------------------------|-------|
| scribium-core       | Source abstraction, parsing, semantic analysis,         | Yes   |
|                     | evaluator, built-ins, IR, source map, compatibility     |       |
| scribium-typst      | Typst lowering, TypstBackend trait, backend adapters    | Yes ¹ |
| scribium-cli        | CLI dispatch, config, filesystem, output formatting     | No    |
| scribium-test-support | Fixture loading, golden test utilities, temp projects | No    |

¹ Lowering layer only (not subprocess backend).

## Platform Independence

`scribium-core` and `scribium-typst` (lowering only) MUST compile for
`wasm32-unknown-unknown`. CI enforces this on every push.

### Forbidden in core crates

- `std::fs`, `std::process`, `std::env` — OS-specific APIs never used
- `TcpStream` — no network access
- System clock dependency
- Global mutable state
- `std::path::PathBuf` in public API — use `VirtualPathBuf` instead

### VirtualProject: I/O-Free Core

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

Core compilation is synchronous. Host loads dependencies asynchronously.

```rust
pub enum CompileStatus {
    Complete(CompileOutput),
    NeedsSources(Vec<VirtualPathBuf>),
}
```

### WASM Editions

| Edition | Scope | Status |
|---------|-------|--------|
| Frontend WASM | Parse → evaluate → lower to Typst source | Guaranteed |
| Full browser compile | + Typst compiler in WASM → PDF | M7+ feasibility gate |

## Source Span Model

- **SourceId**: unique identifier for each source file
- **ByteSpan**: byte offset + length in source text
- **LineColumn**: line (1-indexed) + column (1-indexed, byte offset within line)
- **Span conversion functions**: byte ↔ line/column, both directions
- **Span attachment**: every AST node, IR node, and diagnostic carries its source span

## IR Model

The IR is a Typst-oriented tree:

```
Document
├── Metadata (title, author, date, etc.)
├── Content
│   ├── Heading (level, body)
│   ├── Paragraph (inline content)
│   ├── List (ordered/unordered, items)
│   ├── CodeBlock (language, source)
│   ├── Table (header, rows)
│   ├── Math (display/inline, source)
│   ├── RawTypst (raw Typst block)
│   ├── FunctionCall (name, args, body)
│   └── NativeBlock (evaluated Typst output)
```

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
