from pathlib import Path
import re

path = Path("crates/arkst-ir/src/lib.rs")
source = path.read_text()

# The semantic field is already implemented. Keep this temporary executor
# narrowly scoped to explicit test fixtures that must name every state field.
if "pub page_geometry: Option<IrPageGeometry>" not in source:
    raise SystemExit("page geometry implementation is missing")

pattern = re.compile(
    r"(?m)^(?P<indent>\s*)page_alignment: None,\n(?P=indent)slides: None,$"
)
source, replacements = pattern.subn(
    lambda match: (
        f"{match.group('indent')}page_alignment: None,\n"
        f"{match.group('indent')}page_geometry: None,\n"
        f"{match.group('indent')}slides: None,"
    ),
    source,
)

if replacements == 0:
    if source.count("page_geometry: None,") < 2:
        raise SystemExit("expected repaired page_geometry fixtures")
    print("page geometry IR fixtures already repaired")
elif replacements != 2:
    raise SystemExit(f"expected exactly 2 fixture repairs, found {replacements}")
else:
    path.write_text(source)
    print("repaired 2 explicit IrDocumentState fixtures")
