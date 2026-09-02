# Quarkdown v2.5.1 Compatibility Impact

## Review record

- **Tracked target:** Quarkdown v2.5.1
- **Verified baseline before review:** v2.5.0
- **Latest stable at review:** v2.5.1 (`iamgio/quarkdown` release, published 2026-08-12)
- **Review mode:** clean-room public-evidence review
- **Black-box reference observation:** not used; the release note and public
  specifications were sufficient for the tested behavior
- **Rushdown revision:** `e5eb4e4446541ea0ed53111c1b37e779283ff57c` before and
  after review

This report records the v2.5.1 public delta and its effect on Arkst. It is
an evidence register, not a claim of full Quarkdown compatibility. Arkst's
long-term target remains complete public-language and document-observable
compatibility, while the verified baseline advances only for reviewed,
evidence-backed behavior.

Value categories, invocation binding, and conversion semantics are audited in
the separate canonical [`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md); this
release-delta report does not duplicate that matrix.

## Delta matrix

| Delta | Public source | Classification | Affected Arkst surface | Current behavior | Required change | Evidence | Disposition |
|---|---|---|---|---|---|---|---|
| D1. Embedding-friendly pipeline error handling | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Changed | `UPSTREAM_HOST_API_ONLY` | None; Arkst does not embed the Quarkdown JVM runtime or call `runQuarkdown` | Not applicable to the current host architecture | None | Architecture review and source inventory; no JVM embedding surface exists | `CODE_CHANGE: NONE`; `CONFORMANCE: NONE` |
| D2. Unbalanced parentheses in links | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed; [CommonMark link destination rules](https://spec.commonmark.org/current/#links); [CommonMark autolinks](https://spec.commonmark.org/current/#autolinks) | `DOCUMENT_LANGUAGE_RELEVANT` and `OUTPUT_SEMANTICS_RELEVANT` | Rushdown-backed Markdown conversion in `crates/arkst-markdown` | Rushdown accepts the relevant link kinds. The Arkst conversion layer previously exposed inline-link spans ending at the label, retained inline backslash escapes in destination values, and applied that normalization indiscriminately to Auto/Reference/Image values. | Restore complete source-backed inline-link spans through the closing delimiter, including empty destinations, and apply the required CommonMark backslash-escape semantics only to ordinary inline-link destinations. Preserve Auto, Reference, and Image destination representations and parser-owned autolink labels/spans. Keep literal trailing `)` and surrounding text outside the link. | `crates/arkst-markdown/tests/quarkdown_v2_5_1.rs` covers balanced, escaped, unbalanced, nested, empty, whitespace-empty, angle-bracket, double/single/parenthesized and multiline titles, trailing punctuation/text, Auto URI/email isolation, Reference/Image preservation, UTF-8, CRLF, `.md`, `.qd`, and QD body paths. | Adapted with a narrow frontend conversion/provenance fix; conformance PASS |
| D3. Deeply nested lists with 4-space indentation | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed; [CommonMark list rules](https://spec.commonmark.org/current/#lists) | `DOCUMENT_LANGUAGE_RELEVANT` and `OUTPUT_SEMANTICS_RELEVANT` | Rushdown-backed block AST conversion and Quarkdown body normalization | The pinned Rushdown substrate already produced the required three- and four-level list structure. No fixed-width Quarkdown body heuristic was introduced. | No parser/substrate change; retain the container-relative body indentation established by PR #54 and verify list structure independently. | `crates/arkst-markdown/tests/quarkdown_v2_5_1.rs` covers depth 3/4, siblings, dedent, following content, nested paragraph/list content, UTF-8, CRLF, `.md`, `.qd`, and QD body interaction. | No additional list implementation change; conformance PASS |
| D4. Subdocument links missing a trailing slash | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed; [Quarkdown subdocuments documentation](https://quarkdown.com/wiki/subdocuments/) | `NOT_APPLICABLE_TO_CURRENT_ARKST` | HTML/site generation, canonical URLs, sitemap, and client-side search index | Arkst currently has no HTML-site generator, subdocument routing, canonical URL, sitemap, or search-index surface | None in this adaptation; future HTML/site work must record this as compatibility debt before implementing that surface | Repository architecture/source inventory; no current HTML backend or site-output package exists | Deferred as future surface compatibility debt; no implementation added |
| D5. LSP completion crash on out-of-bounds cursor position | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed | `TOOLING_RELEVANT_NOT_IMPLEMENTED` | LSP completion and editor protocol | Arkst LSP is a deferred M6 surface and has no completion engine | None in this milestone; M6 must add bounds validation before completion processing | Roadmap/architecture review; no current LSP package exists | Recorded for M6; no implementation added |

## Additional public-delta review

The official v2.5.1 release notes were checked together with the public
Quarkdown wiki pages for [syntax of a function call](https://quarkdown.com/wiki/syntax-of-a-function-call/),
[Markdown content](https://quarkdown.com/wiki/markdown-content/),
[iterables](https://quarkdown.com/wiki/iterable/), and
[subdocuments](https://quarkdown.com/wiki/subdocuments/), plus the public
`quarkdown.com/docs` API surface. No additional behavior was identified as a
v2.5.1-specific public delta beyond D1-D5. Existing documented features that
remain absent from Arkst, including v2.5.0 built-ins and other deferred
Quarkdown syntax, remain compatibility debt and are not silently promoted by
this review.

### Target-specific `.html` contract audit

The `.html` function is not a v2.5.1 release-note delta; it is an existing
public-language gap that was previously insufficiently specified in the
compatibility record. This review closes that evidence ambiguity against the
fixed tag commit
`107ec3a9482f10d6f90d7580f8409b46a719d18e` and records the implementation
boundary in [ADR-0018](../../adr/0018-quarkdown-target-specific-native-content.md).
The v2.5.1 contract is one evaluated `String` argument, evaluator-time
`NativeContent` authorization, generic-node block/inline placement, verbatim
HTML-target output, and empty output for non-HTML visitors. This release-impact
review itself added no implementation; the subsequent current-main slice now
implements the closed `Html` semantic boundary with production IR, evaluator
capability checking, source-backed `E3004` denial, and silent Typst/PDF
omission. The HTML output backend remains future work, while ordinary mixed raw
HTML in `.qd`/`.arkst` remains `E8001` under the separate Markdown raw-HTML
policy.

## Clean-room and substrate record

- No Quarkdown implementation source, source diff, internal test, or upstream
  fixture was used.
- All v2.5.1 inputs are independently authored in the Arkst test file.
- The pinned Rushdown public/current integration contract was checked only for
  LinkKind, destination representation, and parser-owned autolink facts needed
  by this adapter correction; no Rushdown code, test, or fixture was changed.
- The Rushdown pin was not changed, forked, patched, vendored, or upgraded.
- The D2/D3 checks execute through the existing `.md`, `.qd`, and Quarkdown
  body frontend paths. They do not merge body indentation and Markdown list
  indentation into one absolute source-column rule.

## Baseline gate

The baseline promotion gate is evaluated by the adaptation PR after the full
validation matrix completes. D1-D5 are classified, the additional public
review is recorded, and the Arkst-relevant D2/D3 conformance evidence is
passing. The final manifest value is authoritative for whether promotion was
performed; this report does not equate a verified baseline with full
compatibility.
