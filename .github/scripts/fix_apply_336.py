from pathlib import Path

path = Path(".github/scripts/apply_336.py")
text = path.read_text()
old = '''replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    "    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);",
    "    assert_eq!(column.main_axis_alignment, None);",
)
'''
new = '''replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    """    assert_eq!(column.layout, IrStackedLayout::Column);\n    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);\n    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Center);""",
    """    assert_eq!(column.layout, IrStackedLayout::Column);\n    assert_eq!(column.main_axis_alignment, None);\n    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Center);""",
)
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one patcher replacement, found {count}")
path.write_text(text.replace(old, new, 1))
