from pathlib import Path

p = Path("crates/arkst-typst/src/lowering.rs")
text = p.read_text(encoding="utf-8")
needle = "IrNode::CodeBlock {"
pos = 0
changed = 0
while True:
    start = text.find(needle, pos)
    if start < 0:
        break
    brace = text.find("{", start)
    depth = 0
    end = None
    for i in range(brace, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                end = i + 1
                break
    if end is None:
        raise SystemExit("unterminated CodeBlock occurrence")
    block = text[start:end]
    if "line_numbers" not in block and "span:" in block and "source:" in block:
        lines = block.splitlines(True)
        inserted = False
        for idx, line in enumerate(lines):
            stripped = line.lstrip()
            if stripped.startswith("span:"):
                indent = line[: len(line) - len(stripped)]
                lines[idx:idx] = [
                    f"{indent}line_numbers: None,\n",
                    f"{indent}callouts: vec![],\n",
                ]
                inserted = True
                changed += 1
                break
        if not inserted:
            raise SystemExit("CodeBlock initializer had no insertable span line")
        replacement = "".join(lines)
        text = text[:start] + replacement + text[end:]
        pos = start + len(replacement)
    else:
        pos = end

if changed != 4:
    raise SystemExit(f"expected four legacy CodeBlock test fixtures, changed {changed}")
p.write_text(text, encoding="utf-8")
