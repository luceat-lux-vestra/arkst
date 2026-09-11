# Quarkdown v2.6.0 migration checklist

Canonical tracker: #311.

This checklist fixes the migration scope so compatibility work does not expand opportunistically while individual slices are being implemented.

- [x] `.prepended` / `.appended` collection operations — merged in #319.
- [ ] `.doclang` v2.6.0 English getter and release-scoped locale acceptance.
- [ ] `.code(callouts:)` binding, typed IR ownership, and renderer behavior.
- [ ] `focus` layout theme acceptance and output behavior.
- [ ] `slides` H1/H2 automatic page-break behavior and `.autopagebreak` override.
- [ ] `.row` / `.column` omitted-alignment inheritance.
- [ ] PDF export Chromium-family adapter and `--chrome-path` / `QD_CHROME_PATH`; obsolete Node/npm controls removed from the Arkst-compatible surface where present.
- [ ] `docs` wide-table horizontal scrolling behavior for HTML output where Arkst owns equivalent output.
- [ ] `slides` PDF fixes that are observable in Arkst-owned export behavior.
- [ ] `quarkdown create` slides-tailored starter content for the Arkst CLI/project-template equivalent where that command is supported.
- [ ] Full conformance suite and cross-platform CI.
- [ ] Compatibility documentation/provenance reconciliation.
- [ ] Repository-wide verified baseline promoted from v2.5.1 to v2.6.0 only after every row above is either implemented/evidenced or explicitly classified as not applicable with owner-bound evidence.

Clean-room rule: public user-facing documentation, official release metadata, independently authored fixtures, and black-box observations are allowed; Quarkdown implementation source is not used for Arkst implementation.
