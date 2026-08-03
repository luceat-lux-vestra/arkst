# Testing — Scribium

## Test Pyramid

```
        ╱╲
       ╱  ╲          End-to-end (CLI → PDF)
      ╱    ╲
     ╱──────╲        Integration (crate combinations)
    ╱        ╲
   ╱──────────╲      Golden + Snapshot (Typst output, diagnostics)
  ╱            ╲
 ╱──────────────╲    Unit (parser, span, value, lowering, escaping)
```

## Unit Tests

Target areas:

- Span conversion (byte ↔ line/column, both directions)
- Parser nodes (each AST construct)
- Escape functions (Typst text, code, string)
- Value operations (comparison, concatenation, type coercion)
- Evaluator (variable lookup, function call, conditional, iteration)
- Resource limits
- Lowering functions (each IR node → Typst)
- Diagnostic mapping (generated span → original span)
- Config merge (CLI args + config file + defaults)

## Snapshot Tests

Snapshot targets:

- AST output (`scribium inspect --emit ast`)
- Semantic tree (`scribium inspect --emit semantic`)
- IR output (`scribium inspect --emit ir`)
- Generated Typst (`scribium inspect --emit typst`)
- Diagnostics (error snapshots)
- CLI help text

**Snapshot update rules:**
- Change reason must be explained in the PR
- No batch updates without per-change review
- Normalize paths, timestamps, and versions before comparison
- Keep snapshots reviewable (not thousands of lines per change)

## Golden Tests

Golden test structure:

```
fixtures/lowering/
├── basic-paragraph/
│   ├── input.qd
│   ├── expected.typ
│   └── expected.diagnostics.json
├── function-call/
│   ├── input.qd
│   ├── expected.typ
│   └── expected.diagnostics.json
└── ...
```

Golden tests verify:
- Generated Typst output matches expected
- Typst compilation succeeds (exit code 0)
- Diagnostic JSON output matches expected
- Page count text extraction (when stable)

## End-to-End Tests

CLI integration tests:

- `scribium build input.qd` → output PDF exists
- `scribium check input.qd` → exit code reflects validity
- `scribium inspect input.qd --emit typst` → Typst output
- Invalid input → appropriate exit code + diagnostic
- Missing Typst backend → clear error
- Config discovery from parent dirs
- Project root detection
- Include resolution
- Cross-platform path handling
- Output overwrite protection

## Property Tests

Invariants:

- Parser does not panic on arbitrary UTF-8 input
- Source spans never exceed source bounds
- Lowering output is deterministic
- Source map ranges are non-overlapping and sorted
- Config serialization round-trips

## Compatibility Tests

Each compatibility fixture records:

```yaml
specification_source: public-docs
reference_version: 0.9.0
feature: dot-prefixed-call
expected_level: output-equivalent
notes: basic case, single positional argument
```

Compatibility levels:
- `Unsupported` — produces explicit diagnostic
- `Parsed` — syntactically accepted
- `Semantically supported` — semantics match spec
- `Output-equivalent` — Typst output matches reference
- `Known divergence` — deliberate behavioral difference

## CI Checks

| Check | What it runs | Gate |
|-------|-------------|------|
| fmt | `cargo fmt --all --check` | Merge |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Merge |
| test (macOS) | `cargo test --workspace --all-features` | Merge |
| test (Linux) | Same, on Ubuntu | Merge |
| test (Windows) | Same, on Windows | Merge |
| docs | `cargo doc --no-deps --all-features` | Merge |
| license | `cargo-deny check` | Merge |
| WASM build | `cargo check -p scribium-core -p scribium-typst --target wasm32-unknown-unknown` | Merge |

The WASM build check ensures core + lowering crates remain compatible with
browser deployment targets. It only checks that compilation passes — no WASM
test runner is required. If the check is slow, it may use `--target-dir`
caching.