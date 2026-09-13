from pathlib import Path

p = Path("crates/arkst-typst/tests/code_callouts.rs")
text = p.read_text(encoding="utf-8")
old = 'assert_eq!(legacy, "```rust\\nalpha\\n```\\n");'
new = 'assert_eq!(legacy, "```rust\\nalpha\\n```\\n\\n");'
if text.count(old) != 1:
    raise SystemExit(f"expected one legacy assertion, got {text.count(old)}")
p.write_text(text.replace(old, new, 1), encoding="utf-8")
