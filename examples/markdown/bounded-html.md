# Bounded inline HTML example

Scribium deliberately supports only a small, attribute-free inline HTML subset where the parsed structure maps exactly onto existing backend-neutral IR.

<em>Emphasis</em>, <strong>strong text</strong>, <del>deleted text</del>, and <s>struck text</s> are supported.<br />
This line follows the supported HTML break.

Nested supported tags also work: <em>outer <strong>inner</strong></em>.

Other raw HTML remains source-backed but is rejected at the document-output boundary with `E8001`; this example intentionally contains only the supported subset.
