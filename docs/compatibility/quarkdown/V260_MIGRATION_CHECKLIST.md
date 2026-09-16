# Quarkdown v2.6.0 migration checklist

Canonical tracker: #311.

This checklist fixes the migration scope so compatibility work does not expand opportunistically while individual slices are being implemented.

- [x] `.prepended` / `.appended` collection operations — merged in #319.
- [x] `.doclang` v2.6.0 English getter and release-scoped locale acceptance — implemented and documented in `V260_DOCLANG.md`.
- [x] `.code(callouts:)` binding, typed IR ownership, and renderer behavior — merged in #326; implementation/evidence is documented in `V260_CODE_CALLOUTS.md`.
- [x] `focus` layout theme acceptance and output behavior — merged in #330; implementation/evidence is documented in `V260_FOCUS_LAYOUT.md`.
- [x] `slides` H1/H2 automatic page-break behavior and `.autopagebreak` override — merged in #334; implementation/evidence is documented in `V260_AUTOPAGEBREAK.md`.
- [x] `.row` / `.column` omitted-alignment inheritance — merged in #340; implementation/evidence is documented in `V260_STACK_ALIGNMENT_INHERITANCE.md`.
- [x] PDF export Chromium-family adapter and `--chrome-path` / `QD_CHROME_PATH`; obsolete Node/npm controls removed from the Arkst-compatible surface where present — classified as not applicable to Arkst's current Typst-native PDF surface in #344; evidence and future ownership are documented in `V260_PDF_EXPORT_ADAPTER.md`.
- [x] `docs` wide-table horizontal scrolling behavior for HTML output where Arkst owns equivalent output — classified as not applicable to Arkst's current output surface in #349; future rendered-HTML conformance remains owned by #320/#347, with evidence documented in `V260_DOCS_WIDE_TABLE_SCROLLING.md`.
- [x] `slides` PDF fixes that are observable in Arkst-owned export behavior — implemented in candidate #358 with bounded #175/#178/#185 subcontracts; clean-room, adversarial, real-PDF, subprocess/in-process parity, WASM, and Linux/macOS/Windows evidence is documented in `V260_SLIDES_PDF_EXPORT.md`.
- [x] `quarkdown create` slides-tailored starter content for the Arkst CLI/project-template equivalent where that command is supported — classified in #356 as current-surface N/A because Arkst exposes no project-creation/scaffolding equivalent; the non-permanent ownership boundary and future clean-room conformance trigger are documented in `V260_CREATE_SLIDES_STARTER.md`.
- [ ] Full conformance suite and cross-platform CI.
- [ ] Compatibility documentation/provenance reconciliation.
- [ ] Repository-wide verified baseline promoted from v2.5.1 to v2.6.0 only after every row above is either implemented/evidenced or explicitly classified as not applicable with owner-bound evidence.

Clean-room rule: public user-facing documentation, official release metadata, independently authored fixtures, and black-box observations are allowed; Quarkdown implementation source is not used for Arkst implementation.
