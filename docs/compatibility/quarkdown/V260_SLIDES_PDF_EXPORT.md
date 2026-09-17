# Quarkdown v2.6 slides PDF export compatibility

Issue: #352. Parent migration checklist: #311. Disposable clean-room oracles: #353 and #355. Implementation candidate: #358.

## Decision

The Quarkdown v2.6 `slides` PDF-export changes are **applicable to Arkst's owned output surface**. The pre-implementation Arkst path was observably divergent from the v2.6 target because `.doctype {slides}` fell through to Typst's A4 portrait default and had no slides-specific document-layout contract.

PR #358 implements the bounded Arkst-owned contract needed for this migration row without importing Quarkdown renderer implementation details into generic compiler state:

- #175 bounded subcontract: typed complete `(width, height)` page geometry plus explicit `.pageformat` override ownership;
- #178 bounded subcontract: document-type-gated `.slides` state with nullable vertical centering;
- #185 bounded subcontract: typed explicit `.pagebreak` / `<<<` boundaries;
- Typst ownership: slides default PDF geometry, vertical-centering lowering, weak explicit page boundaries, and final PDF artifact production.

The broader #175/#178/#185 issues remain open for their unrelated baseline scope. This migration slice does not claim controls, speaker notes, transitions, partial/selector page-format semantics, math, or code presentation support.

The Chromium-export adapter remains current-surface N/A as documented in `V260_PDF_EXPORT_ADAPTER.md`; that does not make slides layout N/A because Arkst itself owns `.doctype {slides}` → Typst → PDF.

## Official v2.6 claim

The official Quarkdown v2.6.0 release notes describe improved `slides` PDF layout and correct spacing for blank/headerless slides. That statement is treated as an output-level compatibility claim only.

The exact complete meaning of **"more polished layout" remains UNKNOWN** beyond the clean-room observations below. Arkst does not infer unspecified CSS, DOM, browser, or theme behavior from that phrase.

## Clean-room evidence identity

No Quarkdown implementation source was inspected for this slice. Evidence was limited to:

- official v2.6.0 release notes;
- official distributed v2.5.1 and v2.6.0 Linux x64 binaries as black boxes;
- independently authored fixtures;
- generated presentation artifacts, observable runtime layout, and produced PDFs.

Pinned distributions:

| Version | Linux x64 SHA-256 |
|---|---|
| v2.5.1 | `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9` |
| v2.6.0 | `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4` |

### Initial oracle (#353)

Final HEAD: `5674cb3efb923034b54f504baa48a0cd0f8a82fa`.

Reference-version run:

- workflow run `35037462289` — success;
- artifact `10423593387`;
- artifact digest `sha256:f462a5c8c9d2dbfdd6a0df18b9b86d1e96c7a4f8a1924e1412656708ea6bcd8f`.

Current-Arkst run:

- workflow run `35037462367` — success;
- artifact `10423303799`;
- artifact digest `sha256:f281d649080bb6408675f352e7f641ab1dd941fa2d43a01fcb52286e7af6fbd5`.

### Adversarial follow-up (#355)

Final successful HEAD: `5ec5b46f0b788940eb1e19402ef3fc5ab1970c33`.

- workflow run `35044127244` — success;
- artifact `10426002679`;
- artifact digest `sha256:872d53115459c5018ce39eee618f8b55b6481d015d4f14b45e692362e4e8cdbe`.

Both disposable oracle PRs were closed unmerged after evidence capture.

## Reference observations

### Headerless/body-only spacing

For the bounded generated-presentation fixtures:

- v2.5.1 body-only/headerless sections expose computed `padding-top: 0px`;
- v2.6.0 body-only/headerless sections expose computed `padding-top: 48px`;
- two consecutive interior body-only sections independently show the same 0→48px runtime change;
- the v2.6 `focus` control also reports `48px`, while its final PDF marker position differs from the default layout.

`48px` is a **reference-renderer observation**, not a backend-neutral semantic value. PR #358 deliberately does not store it in Arkst IR or evaluator state.

### Explicit adjacent breaks

Two adjacent explicit `.pagebreak` calls produce two rendered sections in both v2.5.1 and v2.6.0, and the v2.6 native PDF contains two pages. The evidence therefore rejects an implementation that manufactures an empty page between adjacent breaks.

### Automatic H1/H2 boundaries

A default `slides` fixture containing one H1 and two H2 headings produces one rendered section in v2.5.1 and three sections in v2.6.0; the v2.6 native PDF contains three pages. This remains the already-implemented #332/#334 behavior and is regression evidence for #358 rather than a duplicate implementation.

### Native v2.6 PDF geometry

Representative official v2.6 results:

| Fixture | Pages | Page size |
|---|---:|---|
| `headerless-body` | 3 | `749.04 x 546 pt` |
| `heading-only` | 2 | `749.04 x 546 pt` |
| `short-long` | 2 | `749.04 x 546 pt` |
| `consecutive-headerless` | 4 | `749.04 x 546 pt` |
| `empty-double-break` | 2 | `749.04 x 546 pt` |
| `auto-headings` | 3 | `749.04 x 546 pt` |
| `focus-headerless` | 1 | `749.04 x 546 pt` |

The v2.5.1 manual distribution could not produce a usable native PDF in the runner because its Puppeteer dependency was unavailable. Native v2.5.1 PDF geometry therefore remains **UNKNOWN**. The evidence does not claim that page dimensions changed between v2.5.1 and v2.6.0.

## Pre-implementation Arkst divergence

Before #358, an independently authored Arkst fixture lowered to ordinary Typst content plus automatic weak heading breaks, with no slide page-geometry/layout prelude. Typst 0.15.1 therefore produced:

- 2 pages;
- `595.276 x 841.89 pt` A4 portrait;
- no target-specific slide page geometry.

That established a current Arkst → v2.6 target divergence. It did not establish any v2.5.1 → v2.6 page-size delta.

## Candidate contract in #358

### Backend-neutral state

Arkst now preserves only source-level semantics in shared IR:

- `IrPageGeometry { width: IrSize, height: IrSize }` for a complete explicit pair;
- `IrSlidesConfiguration { center: Option<bool> }` for the bounded `.slides` contract;
- `IrNode::PageBreak` for explicit semantic page boundaries.

Serialization is backward-compatible for older state shapes because the new document-state fields are optional/defaulted. Explicit state round-trips through the wire representation.

No `48px`, PDF coordinate, CSS class, Reveal-specific layout object, or renderer implementation detail is stored in generic state.

### `.pageformat` bounded ownership

For this migration slice, Arkst claims only a complete explicit width/height pair and its combination with the already-supported alignment field. A later complete pair replaces the previous pair.

Unsupported partial width/height, nullable component, named-size/orientation/selector combinations remain outside this bounded subcontract and do not silently mutate the last committed geometry. Failed conversion rolls back nested document-state writes.

Renderer resolution order is:

1. explicit complete Arkst page geometry, when present;
2. otherwise the Typst slides default evidenced from the v2.6 reference, `749.04pt x 546pt`.

### `.slides(center:)` bounded ownership

The native `.slides` initializer is accepted only when the final document type is `slides`. Its bounded centering contract is:

- omitted `center` → `None`, preserving renderer/default layout policy;
- explicit `.none` → `None`;
- `true` → typed `Some(true)` and Typst `#set align(horizon)`;
- `false` → typed `Some(false)` and Typst `#set align(top)`.

Malformed Boolean conversion fails closed and preserves the last committed state. Nested document-state writes are rolled back if the outer conversion fails. A source-defined function named `slides` retains precedence over the native initializer.

### Explicit break ownership

Both `.pagebreak` and Quarkdown `<<<` lower to typed `IrNode::PageBreak` and then to Typst `#pagebreak(weak: true)`.

Parser boundaries are deliberately narrow:

- `<<<` promotion applies only in Quarkdown mode;
- source spans are preserved;
- consecutive physical `<<<` lines become distinct boundaries;
- LF and CRLF marker lines retain bounded source spans without newline bytes;
- fenced code containing `<<<` is not promoted;
- Markdown `---` remains a thematic break;
- a mixed paragraph is not partially promoted;
- malformed `.pagebreak` arguments fail closed;
- a source-defined `pagebreak` function retains precedence.

Weak lowering is required by the observed adjacent-break and leading-page behavior: repeated boundaries do not fabricate an empty page when the current page is already empty.

### Typst prelude ordering

The Typst adapter owns the renderer-specific policy. Its order is pinned as:

1. explicit or slides-default page geometry;
2. explicit slides vertical-centering override, if any;
3. `focus` layout prelude, if selected;
4. document content and weak semantic boundaries.

This keeps default/focus interaction testable without conflating theme-specific coordinates with backend-neutral semantics.

## Regression and adversarial evidence

The substantive implementation candidate `aca8143a8f158bbe3411d4df13bf14303b822f0c` passed the bounded engineering proof before this evidence document was finalized:

| Evidence | Result |
|---|---:|
| page geometry semantic/rollback/serde tests | 6/6 pass |
| existing page-alignment regression tests | 6/6 pass |
| `.slides` gating/nullability/rollback/shadowing/serde tests | 6/6 pass |
| explicit-break parser boundary/adversarial tests | 6/6 pass |
| explicit-break IR/evaluator/fail-closed/shadowing/serde tests | 4/4 pass |
| Typst slides prelude/ordering/weak-break tests | 5/5 pass |
| existing automatic-pagebreak artifact regression | 5/5 pass |
| real in-process Typst PDF artifact matrix | 8/8 pass |
| subprocess/in-process slides backend parity with pinned Typst 0.15.1 | pass |

The 8-case real-PDF matrix covers:

- slides default geometry `749.04 x 546 pt`;
- explicit `10in x 5in` override producing `720 x 360 pt`;
- `.pagebreak` plus `<<<` producing three real pages;
- adjacent explicit breaks producing two pages rather than an empty intermediate page;
- heading-at-start plus H1/H2 automatic boundaries without a leading empty page;
- first body-only plus consecutive interior body-only slides;
- short and long body-only slides without accidental pagination;
- default layout, omitted/nullable `.slides`, explicit center true/false, and `focus` retaining the expected page contract.

The dedicated `slides_backend_parity` integration test compares in-process and subprocess Typst PDF page counts and MediaBox geometry across explicit boundaries, centered layout, `focus`, and explicit page geometry. Production CI ran it against pinned Typst 0.15.1 on Linux, macOS, and Windows.

## Production verification record

The substantive candidate `aca8143a8f158bbe3411d4df13bf14303b822f0c` passed the restored production workflows with no temporary development workflow or patch machinery present in the PR diff:

- CI run `35149202882` — success: fmt and repository policy/security/ruleset checks, clippy, docs, license, WASM, and Linux/macOS/Windows workspace tests; all three platforms installed pinned Typst 0.15.1 and passed the dedicated backend-parity step;
- Markdown compatibility run `35149202874` — success: checked-in corpora, generated/reference provenance regeneration, differential corpus and PDF smoke harness, and independent result identity;
- MSRV run `35149202921` — success on Rust 1.92.0 with locked workspace/all-target/all-feature check;
- Reference JVM run `35149202881` — success with the pinned Temurin 25 oracle and external differential verification;
- CodeQL run `35149202854` — success for both Actions and Rust analysis;
- Docs Checks run `35149202852` — success;
- Dependency review run `35149202853` — success.

The final documentation/checklist commit changes only evidence bookkeeping, so the strict merge gate is rerun on that resulting PR HEAD before merge. A HEAD move after that gate again invalidates the final gate evidence.

## Residual boundaries

Arkst has behavioral evidence for the bounded v2.6 slides PDF contract it owns. It does **not** claim Quarkdown's exact CSS, DOM, Chromium, theme-coordinate implementation, or every presentation feature. In particular:

- the unobserved remainder of the release-note phrase "more polished layout" remains `UNKNOWN` rather than guessed;
- native v2.5.1 PDF geometry remains `UNKNOWN`;
- the broader #175/#178/#185 baseline work remains separately owned and is not closed by this migration slice;
- browser/Chromium exporter internals remain outside Arkst's current Typst-native PDF surface as documented separately.

Those boundaries do not block the v2.6 migration row because the applicable Arkst-owned behavior is explicitly typed, regression-tested, artifact-tested, backend-parity-tested, and cross-platform verified.
