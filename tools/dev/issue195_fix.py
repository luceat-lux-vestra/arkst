from pathlib import Path

path = Path("crates/arkst-engine/src/evaluator.rs")
text = path.read_text()
old = '            || matches!(name, "pageformat" | "code" | "extend"))'
new = '            || matches!(name, "pageformat" | "code" | "extend" | "function"))'
count = text.count(old)
if count == 0 and new in text:
    print("issue195 ownership fix already present")
elif count != 1:
    raise SystemExit(f"expected exactly one ownership predicate anchor, found {count}")
else:
    path.write_text(text.replace(old, new))
    print("issue195 ownership fix applied")
