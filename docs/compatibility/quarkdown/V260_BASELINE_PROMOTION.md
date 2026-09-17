# Quarkdown v2.6.0 verified-baseline promotion

Canonical migration tracker: #311.

## Promotion decision

Arkst's verified Quarkdown compatibility baseline is promoted from **v2.5.1**
to **v2.6.0**. This is a release-identity and evidence-baseline promotion, not
a claim that every public Quarkdown surface is implemented. The compatibility
matrix remains authoritative: only rows with their stated evidence level are
support claims, and existing unsupported/partial rows remain compatibility
debt.

The v2.6.0 clean-room release identity used by the adaptation work is:

- upstream tag: `v2.6.0`
- upstream tag commit: `22f3c1169d0b1356fb51d8f43e1833d3d815aab1`
- official Linux x64 archive SHA-256:
  `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`

No Quarkdown v2.6 implementation source is used as implementation evidence for
this migration. Public user-facing documentation, official release metadata,
independently authored fixtures, and black-box observations are the permitted
v2.6 evidence sources.

## Release-delta closure

The canonical `V260_MIGRATION_CHECKLIST.md` records each public v2.6 delta and
its bounded owner/evidence:

- collection `.prepended` / `.appended`: #319;
- `.doclang` English display-name and release-scoped locale acceptance:
  `V260_DOCLANG.md`;
- `.code(callouts:)`: #326 / `V260_CODE_CALLOUTS.md`;
- `focus` layout: #330 / `V260_FOCUS_LAYOUT.md`;
- slides H1/H2 auto page breaks and override: #334 /
  `V260_AUTOPAGEBREAK.md`;
- row/column omitted-alignment inheritance: #340 /
  `V260_STACK_ALIGNMENT_INHERITANCE.md`;
- Chromium-family PDF-export adapter delta: current Typst-native surface N/A,
  #344 / `V260_PDF_EXPORT_ADAPTER.md`;
- docs wide-table HTML scrolling: current output surface N/A, #349 /
  `V260_DOCS_WIDE_TABLE_SCROLLING.md`;
- slides PDF geometry/layout/boundaries: #358 /
  `V260_SLIDES_PDF_EXPORT.md`;
- `quarkdown create` slides starter: current CLI surface N/A, #356 /
  `V260_CREATE_SLIDES_STARTER.md`.

Current-surface N/A classifications are not permanent exclusions; their
documents name the future ownership trigger.

## Full conformance and cross-platform gate

The final slides migration candidate was reviewed at PR #358 head
`c1684e4341ece05e121837df7b49a7dba717113a` and squash-merged as main commit
`5723af8ec49806f09bc7c79a0b4a76acd5cd081b`. On that exact candidate, the
production CI matrix passed `fmt`, `clippy`, `docs`, `license`, `wasm`, and the
full workspace tests on Ubuntu, macOS, and Windows. Each OS installed pinned
Typst 0.15.1 and passed the subprocess/in-process backend-parity step. The same
head also passed Markdown compatibility/provenance, MSRV, Reference JVM,
CodeQL, Docs Checks, and dependency review.

After the squash merge, all six main-push workflows for
`5723af8ec49806f09bc7c79a0b4a76acd5cd081b` completed successfully, providing
post-merge regression evidence before baseline metadata is changed.

The dedicated v2.6 suites additionally exercise release-specific malformed
inputs, source-defined shadowing, transactional state behavior, serde/backward
compatibility where state/IR changed, WASM-sensitive backend-neutral changes,
and real PDF page-count/geometry/output behavior for the applicable slices.

## Provenance reconciliation

Promotion changes only declarations that mean **current verified baseline**:

- `docs/compatibility/quarkdown/upstream.toml`;
- `crates/arkst-compat` default compatibility-profile label;
- root/compatibility active status documentation and architecture example;
- the v2.6 migration checklist and this promotion record.

Historical v2.5.1 audits, ADRs, research records, regression-test names,
conformance `specification_source` identifiers, source links, and changelog
entries are deliberately retained. A v2.5.1 citation remains correct evidence
for an unchanged slice; rewriting it to v2.6 would falsify provenance. Stale
cross-references that described v2.5.1 as the *current* `supported_baseline` are
reworded as historical evidence.

## Gate rule

The promotion PR must run the complete protected-branch gate again after these
metadata/documentation changes. A changed promotion HEAD invalidates prior gate
evidence. The baseline is considered promoted only after that exact-head gate
passes and the promotion PR is squash-merged.

This closes the v2.6 release migration only. It does **not** mark M3 or the
broader long-term full-compatibility program complete.
