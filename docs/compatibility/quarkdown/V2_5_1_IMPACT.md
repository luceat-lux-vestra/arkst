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

This report records the v2.5.1 public delta and its effect on Scribium. It is
an evidence register, not a claim of full Quarkdown compatibility. Scribium's
long-term target remains complete public-language and document-observable
compatibility, while the verified baseline advances only for reviewed,
evidence-backed behavior.

## Delta matrix

| Delta | Public source | Classification | Affected Scribium surface | Current behavior | Required change | Evidence | Disposition |
|---|---|---|---|---|---|---|---|
| D1. Embedding-friendly pipeline error handling | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Changed | `UPSTREAM_HOST_API_ONLY` | None; Scribium does not embed the Quarkdown JVM runtime or call `runQuarkdown` | Not applicable to the current host architecture | None | Architecture review and source inventory; no JVM embedding surface exists | `CODE_CHANGE: NONE`; `CONFORMANCE: NONE` |
| D2. Unbalanced parentheses in links | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed; [CommonMark link destination rules](https://spec.commonmark.org/current/#links) | `DOCUMENT_LANGUAGE_RELEVANT` and `OUTPUT_SEMANTICS_RELEVANT` | Rushdown-backed Markdown conversion in `crates/scribium-markdown` | Rushdown already recognized balanced and escaped destinations and did not create a link for the unbalanced case. The Scribium conversion layer previously exposed a link span ending at the label and retained backslash escapes in the destination value. | Restore the complete source-backed inline-link span through the closing delimiter and apply CommonMark backslash escapes to the destination value. Keep literal trailing `)` and surrounding text outside the link. | `crates/scribium-markdown/tests/quarkdown_v2_5_1.rs` covers balanced, escaped, unbalanced, nested, trailing punctuation/text, UTF-8, CRLF, `.md`, `.qd`, and QD body paths. | Adapted with a minimal frontend conversion fix; conformance PASS |
| D3. Deeply nested lists with 4-space indentation | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed; [CommonMark list rules](https://spec.commonmark.org/current/#lists) | `DOCUMENT_LANGUAGE_RELEVANT` and `OUTPUT_SEMANTICS_RELEVANT` | Rushdown-backed block AST conversion and Quarkdown body normalization | The pinned Rushdown substrate already produced the required three- and four-level list structure. No fixed-width Quarkdown body heuristic was introduced. | No parser/substrate change; retain the container-relative body indentation established by PR #54 and verify list structure independently. | `crates/scribium-markdown/tests/quarkdown_v2_5_1.rs` covers depth 3/4, siblings, dedent, following content, nested paragraph/list content, UTF-8, CRLF, `.md`, `.qd`, and QD body interaction. | No additional list implementation change; conformance PASS |
| D4. Subdocument links missing a trailing slash | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed; [Quarkdown subdocuments documentation](https://quarkdown.com/wiki/subdocuments/) | `NOT_APPLICABLE_TO_CURRENT_SCRIBIUM` | HTML/site generation, canonical URLs, sitemap, and client-side search index | Scribium currently has no HTML-site generator, subdocument routing, canonical URL, sitemap, or search-index surface | None in this adaptation; future HTML/site work must record this as compatibility debt before implementing that surface | Repository architecture/source inventory; no current HTML backend or site-output package exists | Deferred as future surface compatibility debt; no implementation added |
| D5. LSP completion crash on out-of-bounds cursor position | [Quarkdown v2.5.1 release notes](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1), Fixed | `TOOLING_RELEVANT_NOT_IMPLEMENTED` | LSP completion and editor protocol | Scribium LSP is a deferred M6 surface and has no completion engine | None in this milestone; M6 must add bounds validation before completion processing | Roadmap/architecture review; no current LSP package exists | Recorded for M6; no implementation added |

## Additional public-delta review

The official v2.5.1 release notes were checked together with the public
Quarkdown wiki pages for [syntax of a function call](https://quarkdown.com/wiki/syntax-of-a-function-call/),
[Markdown content](https://quarkdown.com/wiki/markdown-content/),
[iterables](https://quarkdown.com/wiki/iterable/), and
[subdocuments](https://quarkdown.com/wiki/subdocuments/), plus the public
`quarkdown.com/docs` API surface. No additional behavior was identified as a
v2.5.1-specific public delta beyond D1-D5. Existing documented features that
remain absent from Scribium, including v2.5.0 built-ins and other deferred
Quarkdown syntax, remain compatibility debt and are not silently promoted by
this review.

## Clean-room and substrate record

- No Quarkdown implementation source, source diff, internal test, or upstream
  fixture was used.
- All v2.5.1 inputs are independently authored in the Scribium test file.
- The Rushdown pin was not changed, forked, patched, vendored, or upgraded.
- The D2/D3 checks execute through the existing `.md`, `.qd`, and Quarkdown
  body frontend paths. They do not merge body indentation and Markdown list
  indentation into one absolute source-column rule.

## Baseline gate

The baseline promotion gate is evaluated by the adaptation PR after the full
validation matrix completes. D1-D5 are classified, the additional public
review is recorded, and the Scribium-relevant D2/D3 conformance evidence is
passing. The final manifest value is authoritative for whether promotion was
performed; this report does not equate a verified baseline with full
compatibility.
