use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn project(source: &str, assets: &[(&str, &[u8])]) -> arkst_core::CompileResult {
    let mut builder = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source");
    for &(path, bytes) in assets {
        builder = builder.add_asset(path, bytes.to_vec()).expect("asset");
    }
    compile(&builder.build().expect("project"), &CompileOptions::default())
}

fn output(result: &arkst_core::CompileResult) -> String {
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
fn regex_filters_full_bare_name_before_ascii_name_sort() {
    let result = project(
        ".listfiles {data} directories:{false} fullpath:{false} pattern:{alpha[0-9]+[.]txt} sortby:{name}::getat {1}\n.listfiles {data} directories:{false} fullpath:{false} pattern:{alpha[0-9]+[.]txt} sortby:{name}::getat {2}\n",
        &[
            ("data/alpha10.txt", b"ten"),
            ("data/alpha2.txt", b"two"),
            ("data/beta.txt", b"beta"),
        ],
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output(&result), "alpha2.txt\nalpha10.txt");
    let wire = serde_json::to_string(&result.ir).expect("serialize");
    let roundtrip: arkst_core::ir::IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn matching_is_full_string_not_substring_and_none_stays_supported() {
    let result = project(
        ".listfiles {data} directories:{false} fullpath:{false} pattern:{a}::size\n.listfiles {data} directories:{false} fullpath:{false} pattern:{a|b}::size\n.listfiles {data} directories:{false} fullpath:{false} pattern:{.}::size\n.listfiles {data} directories:{false} fullpath:{false} pattern:{.none}::size\n",
        &[("data/a", b"a"), ("data/b", b"b"), ("data/alpha", b"alpha")],
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output(&result), "1\n2\n2\n3");
}

#[test]
fn directory_selection_precedes_regex_and_recursive_names_remain_bare() {
    let result = project(
        ".listfiles {data} directories:{true} fullpath:{false} pattern:{nested}::size\n.listfiles {data} directories:{false} fullpath:{false} pattern:{nested}::size\n.listfiles {data} directories:{false} recursive:{true} fullpath:{false} pattern:{child[.]txt}::size\n",
        &[("data/nested/child.txt", b"x"), ("data/other.txt", b"y")],
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output(&result), "1\n0\n1");
}

#[test]
fn unsafe_or_different_regex_dialects_fail_closed_without_content() {
    for pattern in ["[", "(?=a)", "é", "(a)\\1"] {
        let source = format!(
            ".listfiles {{data}} directories:{{false}} fullpath:{{false}} pattern:{{{pattern}}}\n"
        );
        let result = project(&source, &[("data/a", b"x")]);
        assert_eq!(result.diagnostics.len(), 1, "{pattern:?}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001", "{pattern:?}");
        assert!(result.diagnostics[0].message.contains("pattern"), "{result:?}");
        assert!(result.ir.nodes.is_empty(), "{pattern:?}: {result:?}");
    }
}

#[test]
fn non_ascii_name_does_not_claim_jvm_regex_compatibility() {
    let result = project(
        ".listfiles {data} directories:{false} fullpath:{false} pattern:{.*}\n",
        &[("data/é.txt", b"x")],
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("non-ASCII"), "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}
