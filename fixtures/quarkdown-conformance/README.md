# Quarkdown Conformance Corpus

This directory contains Arkst's executable Quarkdown conformance cases.
`arkst-test-support` loads every case from the workspace test suite and
enforces the declared `compatibility_level`.

Historical v2.5.1 cases are undergoing provenance re-audit under issue #444.
Do not infer from a case's presence that its original input was independently
authored. The machine-readable status of every case is tracked in
`.github/license-provenance-audit.toml`; only cases marked `INDEPENDENT` or
`REMEDIATED` are cleared for release provenance.

The corpus is intentionally bounded. Its 20 core cases provide independent
parser, evaluator, document-state, value, and IR evidence for the slices listed
in the case metadata; many intentionally retain their original v2.5.1
specification-source identity as historical provenance for unchanged behavior.
They do not imply that every public surface in the current v2.6.0 verified
baseline is supported or output-equivalent. The v2.6.0 release-delta suites and
cross-platform backend evidence are summarized in
`docs/compatibility/quarkdown/V260_BASELINE_PROMOTION.md`. The
`localization-family` case covers the conformance-level localization slice;
Unicode casing and word-boundary edge cases are additionally gated by the
pinned Reference JVM differential rather than duplicated as corpus fixtures.
The canonical status for the complete audited surface is
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
| `Known divergence` | A non-empty `known_divergence` explanation and exact Arkst behavior assertion. It is not a verification bypass. | `ir.json` |

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
6. Run `cargo test -p arkst-test-support` to verify the case executes

## Clean-Room Policy

New or re-certified fixture inputs must be independently authored from permitted
public specification evidence and/or black-box observations. Do not copy inputs
from Quarkdown implementation source, test fixtures, or documentation examples
verbatim.

Historical cases pre-dating the #444 provenance audit are explicitly tracked in
`.github/license-provenance-audit.toml`. A `REVIEW_REQUIRED` status means the
fixture must be rechecked or independently re-authored before release; it is not
an infringement finding.

See `docs/legal/CLEAN_ROOM_POLICY.md` and
`docs/legal/LICENSE_PROVENANCE_AUDIT.md`.
