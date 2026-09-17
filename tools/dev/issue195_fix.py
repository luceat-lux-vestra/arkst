from pathlib import Path


def replace_exact(text: str, old: str, new: str, expected: int, label: str) -> str:
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: expected {expected} anchor(s), found {count}")
    return text.replace(old, new)


evaluator_path = Path("crates/arkst-engine/src/evaluator.rs")
evaluator = evaluator_path.read_text()
old_owner = '            || matches!(name, "pageformat" | "code" | "extend"))'
new_owner = '            || matches!(name, "pageformat" | "code" | "extend" | "function"))'
if old_owner in evaluator:
    evaluator = replace_exact(evaluator, old_owner, new_owner, 1, "special function owner")
elif new_owner not in evaluator:
    raise SystemExit("special function owner predicate is in an unknown state")
evaluator_path.write_text(evaluator)

registry_path = Path("crates/arkst-engine/src/library_inspection.rs")
registry = registry_path.read_text()
registry = replace_exact(
    registry,
    '\npub(crate) fn function_name_from_library(library: &str) -> Option<&str> {\n    library.strip_prefix(FUNCTION_LIBRARY_PREFIX)\n}\n',
    "\n",
    1,
    "unused pseudo-library parser",
)
registry = replace_exact(
    registry,
    '        assert_eq!(function_name_from_library("__func__hello"), Some("hello"));\n',
    "",
    1,
    "unused pseudo-library parser test",
)
registry_path.write_text(registry)

readme_path = Path("docs/compatibility/quarkdown/README.md")
readme = readme_path.read_text()
old_readme = """The remaining #151 `UNSUPPORTED` families have cohesive implementation owners:
[#195](https://github.com/luceat-lux-vestra/arkst/issues/195) for library
inspection, and
[#197](https://github.com/luceat-lux-vestra/arkst/issues/197) for logger /
diagnostic builtins. For #154, `.match` is #198-owned,
"""
new_readme = """The bounded library-inspection family is implemented by
[#195](https://github.com/luceat-lux-vestra/arkst/issues/195); its clean-room
v2.6 contract and non-goals are recorded in
[`LIBRARY_INSPECTION.md`](LIBRARY_INSPECTION.md). The remaining #151
`UNSUPPORTED` family owner is
[#197](https://github.com/luceat-lux-vestra/arkst/issues/197) for logger /
diagnostic builtins. For #154, `.match` is #198-owned,
"""
readme = replace_exact(readme, old_readme, new_readme, 1, "README #195 status")
readme_path.write_text(readme)

print("issue195 final cleanup patch applied")
