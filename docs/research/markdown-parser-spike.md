# Markdown Parser Spike

## Objective

Evaluate Markdown parser candidates for the Scribium parser baseline.
Candidates must support CommonMark/GFM, preserve source spans, and allow
extension syntax for Quarkdown-compatible directives.

## Candidates

| Candidate               | License   | Span support | Extensibility | WASM      |
|-------------------------|-----------|-------------|---------------|-----------|
| `markdown-rs` (pulldown-cmark) | MIT | Naive byte offset | No hook for directives | Yes |
| `comrak`                | MIT       | Yes (byte spans) | Custom inline/block parsers | Partial   |
| Custom parser           | Apache-2.0| Full control | Full control  | Yes       |

## Recommendation

For M1, use a **custom parser** that handles only the Markdown subset needed
for the vertical slice (heading, paragraph, emphasis, strong, list, fenced code).
This avoids dependency decisions while we evaluate `pulldown-cmark` and `comrak`
for the M2 full Markdown baseline.

## Conclusion

Custom parser for M1. Evaluate `pulldown-cmark` before M2. The span model is
designed to be parser-agnostic.