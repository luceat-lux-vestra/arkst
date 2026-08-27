# Issue #201: Native Typst backend parity evidence

Status: corrective backend-contract work for PR #205. The subprocess and
in-process adapters intentionally do not provide the same package/network
capability contract. Exact-head implementation is commit
`94b9a38e7317ac8608d40573ef04edd53891c152`; its cross-platform CI evidence is
run [33034946513](https://github.com/luceat-lux-vestra/scribium/actions/runs/33034946513).
This evidence must not be inferred from an earlier green run.
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
not require backend-specific diagnostic wording to match. Its optional static
preflight walks the active AST import/include graph from the generated entry:
static package specifications in any namespace and dynamic module operands
may be rejected before CLI execution; static project-relative local modules
are resolved within the canonical project root and scanned recursively with
cycle protection. This is best-effort early validation, not a sandbox: it does
not prove that runtime execution cannot reach Typst's package resolver or
network capability. Runtime evaluation, including `eval` and aliases, is not
blocked by this syntax-only preflight. Unreachable project files and
package-looking text in comments, raw blocks, code blocks, and strings remain
inert.

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
| static-package-preflight-preview | REJECTED | DENIED | static validation versus World capability boundary | intentional architectural divergence; not package/network parity |
| static-package-preflight-local | REJECTED | DENIED | static validation versus World capability boundary | intentional architectural divergence; not package/network parity |
| static-package-preflight-arbitrary-namespace | REJECTED | DENIED | static validation versus World capability boundary | intentional architectural divergence; not package/network parity |
| package-looking-inert-text | PASS | PASS | inert package-looking text, valid PDF | none observed |
| nested-local-module-static-package-preflight-preview | REJECTED | DENIED | reachable static validation versus World capability boundary | intentional architectural divergence; not package/network parity |
| nested-local-module-static-package-preflight-local | REJECTED | DENIED | reachable static validation versus World capability boundary | intentional architectural divergence; not package/network parity |
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

The runtime-generated-package case is deliberately not part of the two-backend
security oracle. An InProcessBackend-only integration fixture verifies that a
runtime-generated package request is denied by the Scribium-owned World. No
subprocess fixture executes such a request to exercise a package or network
resolver.

## Cross-platform and security evidence

The CI workflow installs Typst 0.15.1 on each native matrix job and runs the
parity target as a named step with SCRIBIUM_REQUIRE_TYPST=1. The code-bearing
exact-head evidence is recorded here from run 33034946513 for commit
`94b9a38e7317ac8608d40573ef04edd53891c152`:

| Platform | Parity suite | Typst |
|---|---|---|
| Linux | [PASS](https://github.com/luceat-lux-vestra/scribium/actions/runs/33034946513/job/98395525597) | 0.15.1 required |
| macOS | [PASS](https://github.com/luceat-lux-vestra/scribium/actions/runs/33034946513/job/98395525656) | 0.15.1 required |
| Windows | [PASS](https://github.com/luceat-lux-vestra/scribium/actions/runs/33034946513/job/98395525579) | 0.15.1 required |

The corpus exercises:

- image resources and repeated image loads through the explicit project
  boundary;
- a project-supplied font fixture without system-font discovery assumptions;
- missing project-relative resources;
- ../../outside.svg traversal denial;
- static @preview, @local, and arbitrary-namespace package preflight without
  URL leakage, including packages reached through a project-local module,
  while package-looking comments/raw/code/string text and unused project files
  remain inert;
- logical generated/source paths and mapped/unmapped/ambiguous source spans;
  and
- rejection of temporary, runner, Unix absolute, Windows drive, UNC-like, or
  backslash path leakage in observed diagnostics.

The first Windows run exposed a native path spelling that the original
temporary-root replacement did not cover. The subprocess boundary now
normalizes native, slash, backslash, and extended-prefix forms and redacts
remaining absolute runner/temp tokens while retaining logical project paths.
The subprocess preflight is intentionally limited to static `import`/`include`
operands. It may reject package specifications in every namespace before CLI
execution, including packages reached through a recursively scanned
project-local module. The syntax parser ignores comments, raw text, and inert
strings, and the scanner is cycle-safe without scanning unused files. Dynamic
module operands may also be rejected because the adapter cannot prove they are
project-local without evaluating code. Runtime execution paths are outside
this analysis; in particular, `eval` is not an identifier deny-list. Ordinary
literal relative imports remain supported. None of these checks is a hard
package/network isolation guarantee for SubprocessBackend.
The diagnostic retains the logical entry path. The sanitizer and parity oracle
treat relative components such as `target/` and `Users/` as ordinary logical
path components; only absolute/native/temporary path tokens are redacted.

## Intentional divergences and scope

Package/network capability isolation is an intentional architectural
divergence, not a parity assertion:

| Backend | Contract |
|---|---|
| SubprocessBackend | default, compatibility-oriented CLI backend; explicit project-root staging and best-effort static preflight; no hard package/network isolation guarantee |
| InProcessBackend | optional explicit backend; Scribium-owned `World`/`VirtualProject` resource authority; package and network capability fail-closed |

The in-process adapter returns structured Scribium diagnostics and can retain
an original SourceSpan when the lowering map is reliable. The subprocess
adapter keeps its sanitized compiler text and does not expose an equivalent
structured span in this parity harness. Success/document behavior, resource
and project-boundary behavior, diagnostic path hygiene, and reliable source
map provenance remain parity requirements. Package/network capability does
not.

The default backend remains SubprocessBackend, and this evidence does not
claim browser rendering or a WASM in-process backend. The platform-neutral
WASM check remains limited to scribium-core and scribium-typst.

This work does not implement #188 package/resource expansion, #190
environment capability, #191 browser/WASM rendering, or #203 dependency
cleanup. It does not remove subprocess or change the default selection.
