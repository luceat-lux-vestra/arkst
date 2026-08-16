# Preserved but unsupported HTML

An unknown inline element is kept source-backed and rejected at the output boundary:
<span>x</span>.

Block HTML is also opaque:

<div>
Markdown-looking text with **no Markdown semantics**.
</div>

<!-- comment -->
