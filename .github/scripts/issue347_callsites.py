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

replace_once(
    "crates/arkst-typst-subprocess/tests/focus_layout.rs",
    "use arkst_typst::{TypstBackend, TypstInput};",
    "use arkst_typst::{TypstBackend, TypstInput, TypstTarget};",
)
replace_once(
    "crates/arkst-typst-subprocess/tests/focus_layout.rs",
    '''            let output = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "focus.qd".to_string(),
                })
                .expect("focus Typst must compile");
            let pdf = output.pdf.expect("PDF output must be present");
            assert!(pdf.starts_with(b"%PDF-"), "invalid PDF prefix");
''',
    '''            let output = backend
                .compile(
                    &TypstInput {
                        source: typst,
                        entry_path: "focus.qd".to_string(),
                    },
                    TypstTarget::Pdf,
                )
                .expect("focus Typst must compile");
            let pdf = output
                .single_artifact()
                .expect("PDF output must contain exactly one artifact");
            assert!(pdf.bytes.starts_with(b"%PDF-"), "invalid PDF prefix");
''',
)
replace_once(
    "crates/arkst-typst-subprocess/tests/focus_layout.rs",
    '''        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "focus.qd".to_string(),
            })
            .expect("paged control Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
''',
    '''        let output = backend
            .compile(
                &TypstInput {
                    source: typst,
                    entry_path: "focus.qd".to_string(),
                },
                TypstTarget::Pdf,
            )
            .expect("paged control Typst must compile");
        assert!(output
            .single_artifact()
            .expect("PDF output must contain exactly one artifact")
            .bytes
            .starts_with(b"%PDF-"));
''',
)
