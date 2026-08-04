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

### Measurements (to be completed)

- [ ] Clean build time (5 iterations)
- [ ] Binary size (stripped/unstripped)
- [ ] Runtime latency (20 iterations)
- [ ] WASM target check