# Compatibility Documentation

Scribium keeps parser capability, language semantics, and backend capability as separate compatibility contracts.

## Cross-cutting policy

- [Raw HTML compatibility policy](RAW_HTML_POLICY.md) — defines the boundary between CommonMark/GFM raw HTML, Quarkdown v2.5.1 mixed-content policy, and Typst backend behavior.

## Markdown

- [Markdown/CommonMark+GFM baseline audit](markdown/README.md)
- [Known Markdown compatibility gaps](markdown/gaps.md)

## Quarkdown

- [Quarkdown compatibility specification](quarkdown/README.md)
- [Quarkdown v2.5.1 public-language gap inventory](quarkdown/GAP_INVENTORY.md)
- [Quarkdown v2.5.1 impact review](quarkdown/V2_5_1_IMPACT.md)
- [Quarkdown specification sources](quarkdown/SPEC_SOURCES.md)

## Typst

- [Typst backend compatibility](typst/README.md)

Compatibility claims must remain evidence-backed. A parser recognizing a construct is not sufficient evidence that Scribium supports its semantics or can successfully lower it through the selected backend.
