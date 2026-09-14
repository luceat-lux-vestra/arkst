# Quarkdown v2.6.0 `.code(callouts:)` compatibility

Arkst's bounded Quarkdown v2.6.0 `.code(callouts:)` behavior is based on public v2.6 documentation/release metadata and clean-room black-box observations from the official Linux x64 v2.6.0 distribution. No Quarkdown implementation source was used for this slice.

- Upstream tag commit: `22f3c1169d0b1356fb51d8f43e1833d3d815aab1`
- Official `quarkdown-linux-x64.zip` SHA-256: `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`
- Canonical tracker: #311

## Bounded observable contract

The public v2.6 surface adds `callouts` to `.code` as a map-shaped option. The independently observed boundary used by Arkst is:

- callout keys denote one-based source lines and must be positive integers;
- key `0` and non-integer keys fail instead of being silently ignored;
- a positive key beyond the current code line count is accepted;
- map insertion order does not control output order: callouts are normalized by ascending source line;
- visible callout marker numbers are assigned `1..N` after that line ordering;
- `linenumbers:{no}` remains valid when callouts are present;
- descriptions use the value's visible/plain-text representation at this boundary, including formatted text such as `Third *item*` becoming `Third item`; and
- a source-defined `.code` callable retains precedence over the native primitive.

The official black-box probes also covered empty callouts, invalid keys, out-of-range positive keys, line-number suppression, input-order reversal, scalar description values, formatted descriptions, and source-defined shadowing.

## Arkst ownership

Arkst keeps ordinary Markdown fenced code unchanged. Native `.code` evaluation produces the same backend-neutral `IrNode::CodeBlock` carrier with two optional compatibility fields:

- `line_numbers: Option<bool>` distinguishes legacy fenced code (`None`) from native `.code` line-number state; and
- `callouts: Vec<IrCodeCallout>` stores validated one-based line numbers and visible description text in normalized order.

Both fields have serde defaults so pre-callout CodeBlock JSON remains readable. `IrCodeCallout` is backend-neutral and contains only `line` and `description`.

The frontend exception is intentionally narrow. Marker-bearing source text is preserved without `E3010` only for the `.code` `callouts` slot so the evaluator-owned map conversion can consume it. Other Quarkdown content arguments retain the established `E3010` fallback policy.

The Typst backend renders the bounded native form with line rows, callout markers, and a description list while preserving the exact legacy fenced-code lowering path when the new metadata is absent. A real Typst 0.15.1 subprocess integration test compiles this generated source to a `%PDF-` document in CI.

## Explicit non-claims

This slice does **not** promote the complete historical `.code` primitive to full compatibility. Existing baseline options outside the v2.6 callout delta, including `caption`, `focus`, and `ref`, remain explicitly deferred here rather than receiving guessed semantics. Their eventual implementation requires their own public evidence and owner-bound compatibility work.

Likewise, this document does not rewrite the historical v2.5.1 audit evidence. It records only the v2.6.0 delta owned by #311.

## Regression evidence

Durable coverage is kept in:

- `crates/arkst-markdown/tests/quarkdown_code_callouts.rs` — target-owned marker preservation and non-widening `E3010` regression;
- `crates/arkst-core/tests/quarkdown_code_callouts.rs` — binding, typed IR, ordering, key validation, line-number behavior, source-defined shadowing, and legacy fence behavior;
- `crates/arkst-ir/tests/code_callouts_serde.rs` — populated roundtrip and pre-callout JSON defaults;
- `crates/arkst-typst/tests/code_callouts.rs` — deterministic lowering and legacy fence preservation; and
- `crates/arkst-typst-subprocess/tests/code_callouts_integration.rs` — generated Typst compiled by the pinned real Typst executable.
