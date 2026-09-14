# Quarkdown v2.6 PDF export host-adapter compatibility

Issue: #342. Parent migration tracker: #311. Future HTML output architecture: #320.

## Decision

The Quarkdown v2.6 Chromium-family PDF host-adapter migration is **not applicable to Arkst's current PDF output surface**.

Arkst currently lowers its backend-neutral document IR to Typst and produces PDF with the official Typst compiler, either through the default native subprocess adapter or the explicit optional in-process adapter. The trusted CLI host selects the subprocess compiler with `--typst-path`. Arkst does not currently implement an HTML-to-browser PDF backend, so adding `--chrome-path` or `QD_CHROME_PATH` now would create a dead or misleading compatibility control rather than reproduce an owned behavior.

This classification does not claim Quarkdown HTML-to-PDF implementation parity. If Arkst later owns a browser-backed HTML/PDF path, that capability must follow the HTML output architecture decision in #320 (or a bounded follow-up) and may then define its own truthful host controls.

## Clean-room evidence

Only public release information, the exact official v2.6.0 distribution as a black-box oracle, independently authored wrappers, and observable CLI/process behavior were used. Quarkdown implementation source was not inspected or translated.

Pinned oracle:

- Quarkdown release: `v2.6.0`
- upstream tag commit: `22f3c1169d0b1356fb51d8f43e1833d3d815aab1`
- release asset: `quarkdown-linux-x64.zip`
- SHA-256: `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`
- binary identity observed after verification: `quarkdown version 2.6.0`
- disposable evidence PR: #343, closed unmerged
- successful oracle run: `34886270606`
- evidence artifact: `probe-342-pdf-export-oracle-34886270606` (artifact id `10364758758`)

The official v2.6.0 release notes describe the PDF-export migration from Node.js/npm/Puppeteer to direct control of a Chromium-family browser, introduce `--chrome-path` and `QD_CHROME_PATH`, and state that `--node-path`, `--npm-path`, `QD_NPM_PREFIX`, and `NODE_PATH` are no longer used by that upstream exporter.

## Black-box observations

The independently authored probe used fake browser executables that only recorded their invocation and then failed. This proved path selection without depending on successful browser rendering.

| Case | Observable result |
| --- | --- |
| `quarkdown c --help` | exposes `--chrome-path=<text>` as a Chromium-family browser path; also exposes `--pdf` and `--pdf-no-sandbox`; does not expose `--node-path` or `--npm-path` |
| explicit `--chrome-path <fake-cli>` | selected executable is invoked with `--version`; Quarkdown exits 70 after the fake browser fails |
| `QD_CHROME_PATH=<fake-env>`, no CLI path | environment-selected executable is invoked with `--version`; exit 70 after fake-browser failure |
| both CLI path and `QD_CHROME_PATH` | only the CLI-selected executable is invoked; explicit option has precedence |
| bogus `QD_NPM_PREFIX` and `NODE_PATH` plus explicit Chrome path | explicit Chromium executable is still selected; legacy environment values do not displace it |
| nonexistent `--chrome-path` | exit 70; diagnostic says the Chrome executable cannot be found and directs the user to `--chrome-path` or `QD_CHROME_PATH` |
| `--node-path` | rejected during CLI parsing, exit 1 |
| `--npm-path` | rejected during CLI parsing, exit 1 |

The fake-browser cases intentionally fail after proving selection, so their exit 70 result is not a browser-rendering conformance claim.

## Arkst current-surface audit

Fresh `main` at the start of #342 was `ea7bc2e6553cfcbc477171a677ecdd76df844eb1`.

Arkst's current CLI contract is materially different from Quarkdown's HTML-to-browser PDF exporter:

- `arkst build --format pdf` uses generated Typst and the official Typst compiler;
- the default PDF adapter is `arkst-typst-subprocess` and accepts `--typst-path` at the trusted CLI host boundary;
- the optional `in-process` backend invokes the official Typst compiler API without a browser;
- HTML output is not currently implemented by `arkst build`;
- repository search found no owned `--node-path`, `--npm-path`, `QD_NPM_PREFIX`, or `NODE_PATH` compatibility surface to remove;
- repository search found no owned `--chrome-path` or `QD_CHROME_PATH` surface outside the migration checklist itself;
- #320 remains the architecture owner for future HTML output and any later HTML/browser packaging path.

Browser/process/environment authority therefore has no reason to enter Arkst's backend-neutral IR, evaluator, or other platform-neutral compiler layers for this migration item.

## Compatibility contract

For the current Arkst product surface:

1. `--typst-path` remains the truthful host control for subprocess PDF compilation.
2. `--chrome-path`, `--node-path`, and `--npm-path` are not Arkst `build` options and must not be accepted as silent/no-op aliases.
3. `QD_CHROME_PATH`, `QD_NPM_PREFIX`, and `NODE_PATH` are not Arkst PDF backend configuration contracts.
4. `--chrome-path` must not be reinterpreted as `--typst-path`.
5. The existing Typst-native PDF backend must not be replaced merely to mirror Quarkdown's exporter implementation choice.
6. Any future browser-backed output capability requires an independently justified host adapter after #320 selects the relevant HTML architecture.

CLI regression tests pin items 1 and 2 so this N/A classification cannot silently turn into a misleading compatibility surface.

## Migration status

This slice is complete when #342's classification lands and the separate migration-checklist bookkeeping PR records the row as **current-surface N/A with evidence**, rather than claiming that Arkst implements Quarkdown's Chromium PDF exporter.
