# Issue #201: Native Typst backend parity evidence

Status: implementation and parity evidence for PR #205. The current
code-bearing corrective implementation is commit
`4d29f1a452531221f3ed519673eaef15762e1e0f`. Its exact-head native matrix
evidence is CI run [33023853364](https://github.com/luceat-lux-vestra/scribium/actions/runs/33023853364),
which passed the named parity step on Linux, macOS, and Windows. This evidence
record is a documentation-only follow-up to that code-bearing commit; it
changes the PR head without changing the backend implementation or its
evidence.
The canonical executable oracle is

~~~
SCRIBIUM_REQUIRE_TYPST=1 \
  cargo test -p scribium-typst-inprocess \
  --test backend_parity --all-features -- --nocapture
~~~

The suite requires Typst 0.15.1. It constructs each logical fixture as a
VirtualProject, runs the real Scribium compile and lower_to_typst path,
and then observes both SubprocessBackend and InProcessBackend. The
subprocess observation receives a temporary native read-context populated from
that same virtual project; the temporary copy is adapter plumbing and is not a
second fixture source.

## Parity oracle

The oracle compares:

- success versus normalized failure classification;
- non-empty PDF, %PDF- header, %%EOF, and page-tree count;
- equal observable page count, with a two-page minimum for the multi-page case;
- resource and font success/failure behavior;
- logical diagnostic paths and the absence of native/temp path or remote URL
  leakage; and
- original SourceSpan only where the in-process source map is complete and
  unique.

PDF byte identity, metadata, object ordering, compression, and exporter
serialization are intentionally outside the oracle.

The test-only OutcomeClass normalizes subprocess text only at the oracle
boundary. The subprocess adapter retains its existing error type/API and does
not require backend-specific diagnostic wording to match. Before subprocess
execution, its package policy walks the active AST import/include graph from
the generated entry: static package specifications in any namespace and
dynamic module operands are denied; active references to `eval` (including
aliases and field accesses) are denied because they can construct imports
after this syntax-only preflight; static project-relative local modules are
resolved within the canonical project root and scanned recursively with cycle
protection. Unreachable project files and package-looking text in comments,
raw blocks, code blocks, strings, and inert `eval(...)` text remain inert.

## Fixture matrix

The following values are observed by the named parity target on the local
macOS arm64 validation host and by the explicit CI step on the PR branch. The
three CI job links below are the code-bearing cross-platform evidence for the
same fixture corpus; they are not inferred from the local run.

| Fixture | Subprocess | In-process | Oracle | Divergence |
|---|---|---|---|---|
| generated-simple | PASS | PASS | valid PDF, same page count | none observed |
| nested-inline-block | PASS | PASS | valid PDF, same page count | none observed |
| real-quarkdown | PASS | PASS | valid PDF, same page count | none observed |
| real-markdown | PASS | PASS | valid PDF, same page count | none observed |
| real-gfm | PASS | PASS | valid PDF, same page count | none observed |
| real-bounded-html | PASS | PASS | valid PDF, same page count | none observed |
| multi-page | PASS | PASS | valid PDF, page count >= 2 and equal | none observed |
| image-resource | PASS | PASS | project-relative asset, valid PDF | none observed |
| repeated-resource | PASS | PASS | repeated project asset, valid PDF | none observed |
| project-font | PASS | PASS | project-supplied font policy, valid PDF | no Scribium font semantic yet |
| missing-resource | FAIL | FAIL | resource failure, logical missing path | error wording may differ |
| traversal | FAIL | FAIL | project boundary denial | error wording may differ |
| package-denial-preview | FAIL | FAIL | package/network fail-closed | error wording may differ |
| package-denial-local | FAIL | FAIL | package/network fail-closed | error wording may differ |
| package-denial-arbitrary-namespace | FAIL | FAIL | package/network fail-closed | error wording may differ |
| package-looking-inert-text | PASS | PASS | inert package-looking text, valid PDF | none observed |
| runtime-eval-package-denial | FAIL | FAIL | runtime-generated package denied | error wording may differ |
| nested-local-module-package-preview | FAIL | FAIL | reachable local package denied | error wording may differ |
| nested-local-module-package-local | FAIL | FAIL | reachable local package denied | error wording may differ |
| nested-local-module-inert-package-looking-text | PASS | PASS | reachable inert module and unused file, valid PDF | none observed |
| invalid-generated | FAIL | FAIL | generated Typst compile failure | subprocess text versus structured in-process diagnostic |
| mapped-diagnostic | FAIL | FAIL | logical path plus original span in-process | subprocess has no structured span |
| ambiguous-diagnostic | FAIL | FAIL | logical path, no fabricated in-process span | subprocess has no structured span |

invalid-generated is also the unmapped-diagnostic case: the generated
diagnostic has no supplied source-map entry, so its in-process primary is
None. mapped-diagnostic adds a complete unique generated-range mapping to the
original project source. ambiguous-diagnostic adds two equally specific valid
mappings to different source identities, so primary remains None.

The fixture source markers are checked after lowering so the corpus does not
silently become a handwritten Typst-only suite. The font fixture has a
test-owned Typst text rule after real lowering because the current Scribium
semantic model has no font-selection construct; the font bytes themselves
remain a project-owned VirtualProject asset and no system font is required.

## Cross-platform and security evidence

The CI workflow installs Typst 0.15.1 on each native matrix job and runs the
parity target as a named step with SCRIBIUM_REQUIRE_TYPST=1. The code-bearing
exact-head evidence is recorded here from run 33023853364:

| Platform | Parity suite | Typst |
|---|---|---|
| Linux | [PASS](https://github.com/luceat-lux-vestra/scribium/actions/runs/33023853364/job/98360439482) | 0.15.1 required |
| macOS | [PASS](https://github.com/luceat-lux-vestra/scribium/actions/runs/33023853364/job/98360439404) | 0.15.1 required |
| Windows | [PASS](https://github.com/luceat-lux-vestra/scribium/actions/runs/33023853364/job/98360439590) | 0.15.1 required |

The corpus exercises:

- image resources and repeated image loads through the explicit project
  boundary;
- a project-supplied font fixture without system-font discovery assumptions;
- missing project-relative resources;
- ../../outside.svg traversal denial;
- @preview, @local, and arbitrary-namespace package denial without URL
  leakage, including packages reached through a project-local module, while
  package-looking comments/raw/code/string text, inert `eval(...)` text, and
  unused project files remain inert; runtime-generated package imports through
  active `eval` references are denied;
- logical generated/source paths and mapped/unmapped/ambiguous source spans;
  and
- rejection of temporary, runner, Unix absolute, Windows drive, UNC-like, or
  backslash path leakage in observed diagnostics.

The first Windows run exposed a native path spelling that the original
temporary-root replacement did not cover. The subprocess boundary now
normalizes native, slash, backslash, and extended-prefix forms and redacts
remaining absolute runner/temp tokens while retaining logical project paths.
The first complete parity run also showed that Typst CLI package resolution
could reach its network resolver. Static `import`/`include` package operands
for every namespace are now rejected before subprocess execution, including
when reached through a recursively scanned project-local module. The syntax
parser ignores comments, raw text, and inert strings, and the scanner is
cycle-safe without scanning unused files. Dynamic module operands are also
rejected because the adapter cannot prove they are project-local without
evaluating code. Active `eval` references are rejected for the same reason:
the runtime-generated source is outside this preflight's syntax-only boundary.
Ordinary literal relative imports remain supported.
The diagnostic retains the logical entry path. The sanitizer and parity oracle
treat relative components such as `target/` and `Users/` as ordinary logical
path components; only absolute/native/temporary path tokens are redacted.

## Intentional divergences and scope

The in-process adapter returns structured Scribium diagnostics and can retain
an original SourceSpan when the lowering map is reliable. The subprocess
adapter keeps its sanitized compiler text and does not expose an equivalent
structured span in this parity harness. This is an expected implementation
difference; success/failure classification, logical-path policy, and leakage
checks remain parity requirements.

The default backend remains SubprocessBackend, and this evidence does not
claim browser rendering or a WASM in-process backend. The platform-neutral
WASM check remains limited to scribium-core and scribium-typst.

This work does not implement #188 package/resource expansion, #190
environment capability, #191 browser/WASM rendering, or #203 dependency
cleanup. It does not remove subprocess or change the default selection.
