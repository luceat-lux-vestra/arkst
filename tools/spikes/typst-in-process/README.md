# Typst In-Process Compilation Spike

## Purpose

This spike investigates the feasibility of using the `typst` crate as an in-process backend for Scribium, instead of the current subprocess-based approach.

## Used Typst Version

- typst 0.15.1
- typst-layout 0.15.1

## Test Cases

1. **Simple rect (no fonts)** - Tests basic layout without font dependencies
2. **Text with font** - Tests font loading and text rendering
3. **Error fixture** - Tests structured diagnostic output for invalid code

## Run

```bash
cargo +1.92.0 run --manifest-path tools/spikes/typst-in-process/Cargo.toml
```

## Build

```bash
cargo +1.92.0 build --release --manifest-path tools/spikes/typst-in-process/Cargo.toml
```

## WASM Check

```bash
cargo +1.92.0 check --target wasm32-unknown-unknown --manifest-path tools/spikes/typst-in-process/Cargo.toml
```

## Test Fixtures

- `simple.typ`: Minimal rect (no fonts)
- `text.typ`: Text with fonts
- `error.typ`: Invalid function call

## Notes

This is an investigation spike, NOT production code. It is excluded from the main workspace via `workspace.exclude` in the root Cargo.toml.

## Results

### Compilation Tests

All three test cases pass:
- ✅ Simple rect (no fonts): 1 page, ~17ms
- ✅ Text with font: 1 page, ~8ms
- ✅ Error fixture: Expected failure with structured diagnostics

### Measurements

Environment:
- OS: macOS (Apple M1 Max)
- CPU: Apple M1 Max
- RAM: 64 GB
- Rust: 1.92.0
- Cargo: 1.92.0
- Typst crate: 0.15.1
- Profile: release
- Cold build (cargo clean before each run)
- 5 iterations each

Build Time (clean, 5 iterations):
- Baseline (subprocess CLI): 2.1s mean
- In-process spike: 18.5s mean
- **Increase: ~16.4s**

Binary Size (release):
- Subprocess CLI: 458 KB unstripped
- In-process spike: 38 MB unstripped / 31 MB stripped
- **Increase: ~37.5 MB absolute / ~84x**

Runtime Latency (20 runs, warm):
- In-process simple rect: 548-1340 µs (mean ~900 µs)
- In-process text with font: 550-1400 µs (mean ~950 µs)
- Subprocess simple rect: 52 ms
- Subprocess text with font: 2020 ms
- In-process is ~50-100x faster for these fixed synthetic fixtures

Process Spawn Overhead:
- Subprocess cold start (simple rect): ~52 ms
- In-process first run (simple rect): ~1.3 ms
- Difference: ~50 ms (not isolated process-spawn cost; includes CLI init, font discovery, etc.)

WASM Target Check:
- `cargo +1.92.0 check --target wasm32-unknown-unknown` passes
- This does **not** prove browser-ready Typst backend is operational

### Notes

- All measurements are spike-specific and must not be treated as production performance guarantees.
- Font fixture results (2020ms subprocess) may be affected by font discovery, cache state, or fixture composition.
- Full methodology and limitations documented in `docs/adr/0011-in-process-typst-backend-feasibility.md`.