from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    s = p.read_text()
    n = s.count(old)
    if n != 1:
        raise SystemExit(f"{path}: expected 1 occurrence, got {n}: {old[:100]!r}")
    p.write_text(s.replace(old, new, 1))

replace_once(
    "tools/markdown-compat/src/main.rs",
    "use arkst_typst::{TypstBackend, TypstInput};",
    "use arkst_typst::{TypstBackend, TypstInput, TypstTarget};",
)
replace_once(
    "tools/markdown-compat/src/main.rs",
    '''        match backend.compile(&TypstInput {
            source: typst,
            entry_path: spec.path.clone(),
        }) {
            Ok(output) => {
                let pdf = output.pdf.context("Typst backend returned no PDF")?;
''',
    '''        match backend.compile(
            &TypstInput {
                source: typst,
                entry_path: spec.path.clone(),
            },
            TypstTarget::Pdf,
        ) {
            Ok(output) => {
                let pdf = output
                    .single_artifact()
                    .context("Typst backend returned no PDF artifact")?
                    .bytes
                    .as_slice();
''',
)
