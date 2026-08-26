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

## Typst Integration Tests

Backend tests that invoke a **real** `typst` executable live in
`crates/scribium-typst-subprocess/tests/backend_integration.rs`, separate from
the subprocess unit tests in `crates/scribium-typst-subprocess/src/lib.rs`
(which use fake executable fixtures and never
require a Typst install, so the ordinary Rust suite is environment-independent).

The integration tests locate an executable via (in order) `SCRIBIUM_TYPST_PATH`,
`typst` on `PATH`, or the Homebrew default `/opt/homebrew/bin/typst`. When none
is found they **skip with a notice**; set `SCRIBIUM_REQUIRE_TYPST=1` to turn a
missing executable into a hard failure. CI installs a pinned Typst version
explicitly (see `.github/workflows/ci.yml`) and runs the suite with
`SCRIBIUM_REQUIRE_TYPST=1` on the Ubuntu/macOS/Windows matrix, so the variants
that produce and validate a real `%PDF-` PDF always run in CI without relying
on the runner image.

The native in-process adapter is covered by
`crates/scribium-typst-inprocess/tests/backend_integration.rs`. Its focused
suite validates generated Scribium Typst, multi-page PDFs, VirtualProject
images and fonts, repeated loads, missing/traversal/package denial, invalid
entry paths, deterministic diagnostics, and source-map handoff. CLI selection
tests cover the subprocess default, explicit values, invalid values, and an
explicit in-process PDF build; cross-platform parity corpus expansion remains
tracked by issue #201.

Issue #201's fixture-level semantic oracle is an independent test target:

~~~
SCRIBIUM_REQUIRE_TYPST=1 \
  cargo test -p scribium-typst-inprocess \
  --test backend_parity --all-features -- --nocapture
~~~

The target requires Typst 0.15.1, runs the generated Scribium source through
both native adapters, compares outcome classification and document-observable
PDF structure/page count, and checks logical diagnostics, source-map
availability, resource/font policy, traversal/package denial, and host-path
leakage. It does not compare PDF bytes. The CI native OS matrix runs this
target as a named step, in addition to the general workspace test, with the
Typst requirement enabled so a missing executable cannot produce a false
green result. See
[docs/research/typst-parity-201.md](research/typst-parity-201.md) for the
fixture matrix and intentional divergences.

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
reference_version: 2.5.0
feature: dot-prefixed-call
expected_level: output-equivalent
notes: basic case, single positional argument
```

The latest stable upstream release is the tracked adaptation target; the
reference/verified version is the last baseline supported by reviewed evidence.
Those values must remain distinguishable. A public feature documented upstream
but not yet implemented is compatibility debt, not evidence of support and not
a permanent exclusion.

Compatibility levels are executable policies enforced by
`scribium_test_support::ConformanceCase::verify()` for every corpus case:

- `Unsupported` — compares `expected/diagnostics.json` by diagnostic code,
  severity, primary span, and secondary spans, and requires a deliberate
  non-parser error.
- `Parsed` — requires only parser acceptance (no `E2xxx` diagnostic); evaluator
  and lowering diagnostics remain allowed.
- `Semantically supported` — requires no diagnostics and exact
  `expected/ir.json` equality with the compiled `IrDocument`.
- `Output-equivalent` — adds exact pure Typst lowering equality against
  `expected/typst.typ`; it does not invoke a Typst subprocess.
- `Known divergence` — requires a non-empty `known_divergence` explanation and
  an explicit expected IR assertion for Scribium's deliberate behavior.

Fixture loading fails closed for unknown levels, directory/metadata ID mismatch,
duplicate metadata IDs, and missing level-specific artifacts. The generic
`quarkdown_conformance_corpus_obeys_declared_levels` test runs the complete corpus
through the existing workspace test gate. Semantic goldens preserve IR node
structure and source spans; no automatic golden-update mode is provided.

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
