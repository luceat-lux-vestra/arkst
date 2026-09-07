use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn project(
    entry: &str,
    sources: &[(&str, &str)],
    assets: &[(&str, &[u8])],
) -> arkst_core::VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path");
    for (path, source) in sources {
        builder = builder
            .add_source(*path, *source)
            .expect("valid source path");
    }
    for (path, asset) in assets {
        builder = builder
            .add_asset(*path, asset.to_vec())
            .expect("valid asset path");
    }
    builder.build().expect("valid virtual project")
}

fn paragraph_text(result: &arkst_core::CompileResult) -> String {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn listfiles_name_sort_ascii_case_fold_precedes_alphanumeric_comparison_189() {
    let project = project(
        "main.qd",
        &[(
            "main.qd",
            ".listfiles {data} directories:{false} fullpath:{false} sortby:{name}::getat {1}\n.listfiles {data} directories:{false} fullpath:{false} sortby:{name}::getat {2}\n.listfiles {data} directories:{false} fullpath:{false} sortby:{name}::getat {3}\n",
        )],
        &[
            ("data/Beta2.txt", b"beta"),
            ("data/alpha10.txt", b"ten"),
            ("data/ALPHA2.txt", b"two"),
        ],
    );

    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        paragraph_text(&result),
        "ALPHA2.txt\nalpha10.txt\nBeta2.txt"
    );
}
