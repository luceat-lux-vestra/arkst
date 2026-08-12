# Markdown Parser Foundation Regression Corpus

- **Status:** Design/fixture inventory; not activated by the foundation
- **Source:** `main` baseline plus the retained PR #45 branch
- **Rule:** preserve these cases before refactoring; activate blockquote
  expectations only in the later blockquote-v2 PR

## Existing main behavior to freeze

The foundation baseline must retain the current parser tests and snapshots for:

- empty/whitespace-only documents and line counts;
- paragraphs, adjacent paragraphs, soft breaks, two-space hard breaks, and
  backslash hard breaks;
- ATX headings 1-6, trailing hash closure, invalid heading spacing, and
  thematic breaks;
- unordered lists, ordered lists (`.` and `)`), marker/delimiter boundaries,
  non-one starts, 9-digit limit, nested list directions, item continuation,
  code blocks inside items, and exact list spans;
- fenced code, language info, indentation preservation, shorter/invalid
  closers, and unclosed fences;
- emphasis, strong, inline code spans, links, malformed inline recovery, and
  nested inline content;
- Quarkdown block/inline calls, scalar/named/content arguments, nested calls,
  indented bodies, body termination, malformed-call diagnostics, and tight
  Unicode boundaries;
- front matter valid/malformed/indented/duplicate/CRLF cases and metadata
  precedence;
- UTF-8/CJK byte spans, LF/CRLF normalization, malformed input recovery,
  snapshots, AST-to-IR, evaluator, Typst lowering, and integration tests.

## PR #45 blockquote cases to recover

### Container and interruption matrix

- basic and multiline blockquotes;
- marker with/without the optional space;
- 0, 1, 2, 3, and 4 leading spaces;
- nested blockquotes at multiple levels and partial marker depth;
- quote → paragraph, quote → heading, quote → list, and separate quotes;
- list → quote, quote → list, list → quote → paragraph;
- unordered/ordered list items inside quotes;
- nested list/list, ordered-list/list, list/quote, and mixed-depth transitions;
- same-list sibling item versus different-marker structural sibling;
- item-relative indentation and ordered marker-width changes.

### Continuation and blank-line matrix

- ordinary lazy paragraph continuation;
- CommonMark examples 237 (fence blocks are not lazy continuation),
  238 (four-space indentation remains paragraph text), 249 (quoted blank
  blocks continuation), 250 (deep nested lazy continuation), and 251
  (partial quote markers continue the active paragraph);
- quoted blank lines `>`, `> `, and `>   `;
- unquoted blank termination;
- lazy continuation after nested heading/list/fence is rejected;
- lazy continuation through an active list item is accepted only at its
  content column;
- open and closed fences inside quote/list containers do not reopen a
  paragraph;
- list-item fence opened after a paragraph and fence content across lines.

### Inline, source, and cross-layer matrix

- multiline code span, emphasis, strong, and link-label content inside a
  future quote container;
- two-space and backslash hard-break spans;
- LF and CRLF soft-break span differentials;
- exact outer block, paragraph, inline, and diagnostic source spans with
  indentation and marker stripping;
- UTF-8/CJK/emoji content;
- AST expectation, AST-to-IR expectation, evaluator expectation, Typst
  lowering/source-map expectation, and Markdown-to-PDF E2E fixture.

## Recovery policy

The foundation PR may add these cases as named fixtures or disabled corpus
records, but it must not add a `BlockQuote` AST/IR node or make them pass by
enabling blockquote syntax. The later `feat/m2-blockquotes-v2` PR owns the
activation and all expected semantic/lowering output changes.
