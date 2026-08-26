# Issue #201: Native Typst backend parity evidence

Status: implementation evidence on the #201 branch. The canonical executable
oracle is

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
boundary. It does not change the subprocess production error API or require
backend-specific diagnostic wording to match.

## Fixture matrix

The following values are observed by the named parity target on the local
macOS arm64 validation host at the current pinned base. Native Linux and
Windows results are supplied by the explicit CI step on the PR HEAD; they are
not inferred from this local run.

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
| package-denial | FAIL | FAIL | package/network fail-closed | error wording may differ |
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
parity target as a named step with SCRIBIUM_REQUIRE_TYPST=1. The final PR HEAD
evidence is recorded here after that run completes:

| Platform | Parity suite | Typst |
|---|---|---|
| Linux | pending PR CI | 0.15.1 required |
| macOS | pending PR CI | 0.15.1 required |
| Windows | pending PR CI | 0.15.1 required |

The corpus exercises:

- image resources and repeated image loads through the explicit project
  boundary;
- a project-supplied font fixture without system-font discovery assumptions;
- missing project-relative resources;
- ../../outside.svg traversal denial;
- @preview/not-present:1.0.0 package denial without URL leakage;
- logical generated/source paths and mapped/unmapped/ambiguous source spans;
  and
- rejection of temporary, runner, Unix absolute, Windows drive, UNC-like, or
  backslash path leakage in observed diagnostics.

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
