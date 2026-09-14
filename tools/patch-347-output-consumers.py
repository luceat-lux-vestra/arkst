from pathlib import Path
import re

PATHS = [
    "crates/arkst-typst-subprocess/src/lib.rs",
    "crates/arkst-typst-subprocess/examples/measure_subprocess.rs",
    "crates/arkst-typst-subprocess/tests/backend_integration.rs",
    "crates/arkst-typst-subprocess/tests/focus_layout.rs",
    "crates/arkst-typst-subprocess/tests/code_callouts_integration.rs",
    "crates/arkst-typst-inprocess/tests/backend_integration.rs",
    "crates/arkst-typst-inprocess/tests/backend_parity.rs",
    "tools/markdown-compat/src/main.rs",
]

for name in PATHS:
    path = Path(name)
    text = path.read_text()

    # Move a historical owned PDF artifact out of the new artifact-set result.
    text = re.sub(
        r"\b(output)\.pdf\s*\.expect\(([^\n;]+)\)",
        r"\1.into_single_artifact().expect(\2).bytes",
        text,
    )

    # Borrow a single PDF artifact where ownership is not needed.
    text = re.sub(
        r"\b(output)\.pdf\.is_some_and\(\|pdf\| pdf\.starts_with\(([^\n]+?)\)\)",
        r"\1.single_artifact_bytes().is_some_and(|pdf| pdf.starts_with(\2))",
        text,
    )

    # Backend parity intentionally keeps an optional PDF observation.
    text = text.replace(
        "pdf: output.pdf.map(|pdf| pdf_observation(&pdf)),",
        "pdf: output.single_artifact_bytes().map(pdf_observation),",
    )

    # The markdown compatibility harness owns the returned PDF bytes.
    text = re.sub(
        r"\b(output)\.pdf\s*\.context\(([^\n]+)\)\?",
        r"\1.into_single_artifact().map(|artifact| artifact.bytes).context(\2)?",
        text,
    )

    # Benchmark examples only need successful PDF production.
    text = re.sub(
        r"\.pdf\s*\.expect\(\"subprocess PDF\"\);",
        '.into_single_artifact().expect("subprocess PDF");',
        text,
    )
    text = re.sub(
        r"\.pdf\s*\.expect\(\"subprocess multi-document PDF\"\);",
        '.into_single_artifact().expect("subprocess multi-document PDF");',
        text,
    )

    path.write_text(text)

# Fail closed if any known old TypstOutput field access remains in migrated files.
remaining = []
for name in PATHS:
    text = Path(name).read_text()
    for needle in ("output.pdf", "typst_output.pdf"):
        if needle in text:
            remaining.append(f"{name}: {needle}")
if remaining:
    raise SystemExit("unmigrated TypstOutput PDF access:\n" + "\n".join(remaining))
