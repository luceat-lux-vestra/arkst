use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn closed_type_domain_and_rich_title_boxes_compile_to_real_typst_pdf() {
    let path = if Command::new("typst")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        Some(PathBuf::from("typst"))
    } else {
        let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
        homebrew.is_file().then_some(homebrew)
    };
    let Some(path) = path else {
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
            panic!("Box integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };

    for (
        input,
        expected_stroke,
        expected_title,
        expected_inset,
        expected_background,
        expected_foreground,
    ) in [
        (
            ".box {Ship this}\n",
            "stroke: 1pt",
            None,
            "8pt",
            None,
            None,
        ),
        (
            ".box {Read *this*}\n    Ship this\n",
            "stroke: 1pt",
            Some("#strong[Read #emph[this]]"),
            "8pt",
            None,
            None,
        ),
        (
            ".box type:{tip}\n    Ship this\n",
            "stroke: (paint: rgb(46, 125, 50, 100%), thickness: 1pt)",
            None,
            "8pt",
            None,
            None,
        ),
        (
            ".box type:{note}\n    Ship this\n",
            "stroke: (paint: rgb(21, 101, 192, 100%), thickness: 1pt)",
            None,
            "8pt",
            None,
            None,
        ),
        (
            ".box type:{warning}\n    Ship this\n",
            "stroke: (paint: rgb(237, 108, 2, 100%), thickness: 1pt)",
            None,
            "8pt",
            None,
            None,
        ),
        (
            ".box type:{error}\n    Ship this\n",
            "stroke: (paint: rgb(198, 40, 40, 100%), thickness: 1pt)",
            None,
            "8pt",
            None,
            None,
        ),
        (
            ".box {Read *this*} type:{warning} padding:{1cm} background:{purple} foreground:{rgba(1,2,3,0.5)}\n    Ship this\n",
            "stroke: (paint: rgb(237, 108, 2, 100%), thickness: 1pt)",
            Some("#strong[Read #emph[this]]"),
            "1cm",
            Some("fill: rgb(128, 0, 128, 100%)"),
            Some("#set text(fill: rgb(1, 2, 3, 50%))"),
        ),
    ] {
        let project = VirtualProjectBuilder::new()
            .entry("box.qd")
            .expect("entry")
            .add_source("box.qd", input)
            .expect("source")
            .build()
            .expect("project");
        let result = compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{result:?}");

        let (source, source_map) = lower_to_typst(&result.ir);
        assert!(
            source.contains(&format!(
                "#block(width: 100%, inset: {expected_inset}, {expected_stroke}"
            )),
            "{source}"
        );
        match expected_background {
            Some(expected) => assert!(source.contains(expected), "{source}"),
            None => assert!(!source.contains(", fill: rgb("), "{source}"),
        }
        match expected_foreground {
            Some(expected) => assert!(source.contains(expected), "{source}"),
            None => assert!(!source.contains("#set text(fill:"), "{source}"),
        }
        assert!(source.contains("Ship this"), "{source}");
        assert!(!source.contains("TO DO"), "{source}");
        match expected_title {
            Some(expected) => assert!(source.contains(expected), "{source}"),
            None => assert!(!source.contains("#strong[Read"), "{source}"),
        }
        assert!(source_map.iter().any(|entry| {
            input
                .get(entry.original.start..entry.original.end)
                .is_some_and(|original| original.starts_with(".box"))
                && entry.generated_start < entry.generated_end
                && entry.generated_end <= source.len()
        }));

        let output = SubprocessBackend::new(path.clone())
            .compile(&TypstInput {
                source,
                entry_path: "box.qd".into(),
            })
            .expect("bounded callout box must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
