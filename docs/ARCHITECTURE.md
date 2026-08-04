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
Lexer / Tokenizer
  │
  ▼
Parser (Markdown baseline + Quarkdown-compatible syntax)
  ├── Markdown blocks: headings, paragraphs, lists, code, tables, etc.
  ├── Quarkdown directives: @function, @function(args)[body]
  ├── Expressions: literals, variables, function calls, conditionals
  └── Front matter: YAML metadata block
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
    .add_source("chapter/intro.qd", "...")?
    .add_asset("fonts/main.otf", data)
    .build()?;

project.entry();
project.sources();
project.assets();
project.metadata();

pub fn compile(project: &VirtualProject) -> CompileResult;
```

- CLI builds `VirtualProject` from disk
- WASM builds `VirtualProject` from in-memory sources
- Core never touches filesystem

### Virtual Paths

Internal paths are logical, not OS paths (`"chapter/intro.qd"`).
The native CLI adapter converts `VirtualPathBuf` ↔ `PathBuf` at the boundary.

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
profile = "quarkdown-v0.9"
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