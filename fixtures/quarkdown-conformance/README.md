# Quarkdown Conformance Corpus

This directory contains independently authored conformance cases for Scribium's
Quarkdown compatibility implementation. The corpus is an executable compatibility
claim: `scribium-test-support` loads every case from the workspace test suite and
enforces the declared `compatibility_level`.

The corpus is intentionally bounded. Its 17 cases provide independent parser,
evaluator, document-state, value, and IR evidence for the slices listed in the
case metadata; they do not imply that every public v2.5.1 surface is supported
or output-equivalent. The canonical status for the complete audited surface is
[`docs/compatibility/quarkdown/RECONCILIATION.md`](../../docs/compatibility/quarkdown/RECONCILIATION.md),
with detailed row evidence in the linked audit manifests. A missing fixture is
not positive support evidence, and unsupported/deferred rows must retain their
explicit issue, defer, or blocker.

## Structure

```
fixtures/quarkdown-conformance/
├── README.md              # This file
├── cases/                 # Individual test cases
│   ├── <case-id>/
│   │   ├── case.toml      # Case metadata
│   │   ├── input.qd       # Independently authored Quarkdown input
│   │   └── expected/
│   │       ├── ir.json          # Required for semantic/output/divergence levels
│   │       ├── typst.typ        # Required for Output-equivalent
│   │       └── diagnostics.json # Required for Unsupported
```

## Case Metadata Schema (`case.toml`)

```toml
# Required fields
id = "call-positional-basic"
feature = "positional-arguments"
compatibility_level = "Parsed"
specification_source = "quarkdown-function-call-syntax"
description = "Basic positional argument call"

# Optional fields
# known_divergence = "Description of known divergence"  # omit if none
```

### Fields

| Field | Description |
|-------|-------------|
| `id` | Unique identifier (kebab-case), used for test naming |
| `feature` | Feature name from the compatibility matrix (e.g., `dot-prefixed-call`, `positional-arguments`, `named-arguments`, `indented-body`, `conditionals`, `variables`) |
| `compatibility_level` | Exactly one of: `Unsupported`, `Parsed`, `Semantically supported`, `Output-equivalent`, `Known divergence` |
| `specification_source` | Short key referencing the specification source in `SPEC_SOURCES.md` |
| `description` | Human-readable description of what this case tests (required) |
| `known_divergence` | Omitted if none, or a description of a documented divergence |

Unknown compatibility levels fail during fixture loading. The case directory name
must equal `id`, and metadata IDs must be unique across the corpus.

## Executable level policy

`ConformanceCase::verify()` is the single public verification entry point:

| Level | Enforced contract | Required artifact |
|-------|-------------------|-------------------|
| `Parsed` | No parser `E2xxx` diagnostic. Evaluation and lowering diagnostics are allowed. | None |
| `Semantically supported` | No parser, evaluation, or lowering diagnostic; exact `IrDocument` equality against independently authored `expected/ir.json`. | `ir.json` |
| `Output-equivalent` | The semantic contract plus exact pure Typst lowering equality. No Typst subprocess runs. | `ir.json`, `typst.typ` |
| `Unsupported` | Exact diagnostic projection (code, severity, primary span, secondary spans), including a deliberate non-parser error. | `diagnostics.json` |
| `Known divergence` | A non-empty `known_divergence` explanation and exact Scribium behavior assertion. It is not a verification bypass. | `ir.json` |

Semantic IR goldens preserve node kinds, inline structure, and source spans. They
must not be replaced with flattened text, and there is no automatic golden-update
mode. For example, `br-line-break-family` retains the ordinary `.br` as an
`IrInline::HardBreak` while the `.plaintext` projection omits it.

`Unsupported` diagnostic goldens use this stable projection and intentionally omit
the brittle message text:

```json
[
  {
    "code": "E8001",
    "severity": "error",
    "primary": { "start": 10, "end": 20 },
    "secondary": []
  }
]
```

## Adding New Cases

1. Create a new directory under `cases/` with the case ID as name
2. Write `case.toml` with the metadata
3. Write `input.qd` with an independently authored Quarkdown input
4. **Do not** copy inputs from Quarkdown test suites or documentation examples
5. Add the required expected artifact for the declared level
6. Run `cargo test -p scribium-test-support` to verify the case executes

## Clean-Room Policy

All test inputs in this corpus are **independently authored** by Scribium contributors
based on public specification documentation only. No inputs are copied from:
- Quarkdown source code
- Quarkdown test fixtures
- Quarkdown documentation examples (verbatim)
- quarkdown-wasm or related repositories

See `docs/legal/CLEAN_ROOM_POLICY.md` for the full policy.
