//! Independent completeness and classification guard for Issue #151's pinned inventory.

use std::collections::BTreeSet;

use scribium_core::ir::{IrInline, IrNode};
use scribium_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

const MANIFEST: &str =
    include_str!("../../../docs/compatibility/quarkdown/STDLIB_BUILTINS_AUDIT_MANIFEST.tsv");
const PINNED_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const EXPECTED_NAMES: &[&str] = &[
    "bibliography",
    "cite",
    "getat",
    "first",
    "second",
    "third",
    "last",
    "size",
    "sumall",
    "average",
    "distinct",
    "sorted",
    "reversed",
    "groupvalues",
    "pair",
    "read",
    "pathtoroot",
    "listfiles",
    "filename",
    "json",
    "csv",
    "dictionary",
    "get",
    "doctype",
    "docname",
    "docdescription",
    "docauthor",
    "docauthors",
    "dockeywords",
    "doclang",
    "theme",
    "numbering",
    "nonumbering",
    "font",
    "paragraphstyle",
    "captionposition",
    "texmacro",
    "pageformat",
    "pagemargin",
    "footer",
    "currentpage",
    "totalpages",
    "formatpagenumber",
    "resetpagenumber",
    "lastheading",
    "autopagebreak",
    "noautopagebreak",
    "marker",
    "navigation",
    "tableofcontents",
    "include",
    "includeall",
    "subdocument",
    "emoji",
    "allemojis",
    "if",
    "ifnot",
    "foreach",
    "repeat",
    "function",
    "extend",
    "var",
    "let",
    "node",
    "htmloptions",
    "html",
    "css",
    "cssproperties",
    "llmstxt",
    "icon",
    "container",
    "align",
    "center",
    "float",
    "row",
    "column",
    "grid",
    "landscape",
    "fullspan",
    "whitespace",
    "clip",
    "box",
    "todo",
    "collapse",
    "textcollapse",
    "numbered",
    "table",
    "libexists",
    "functionexists",
    "libraries",
    "libfunctions",
    "localization",
    "localize",
    "log",
    "debug",
    "error",
    "islower",
    "isgreater",
    "equals",
    "not",
    "markdown",
    "sum",
    "subtract",
    "multiply",
    "divide",
    "rem",
    "pow",
    "abs",
    "negate",
    "sqrt",
    "logn",
    "pi",
    "sin",
    "cos",
    "tan",
    "truncate",
    "round",
    "iseven",
    "range",
    "mermaid",
    "xychart",
    "subdocumentgraph",
    "filetree",
    "keybinding",
    "none",
    "isnone",
    "otherwise",
    "ifpresent",
    "takeif",
    "heading",
    "paragraph",
    "link",
    "image",
    "pagebreak",
    "code",
    "math",
    "figure",
    "env",
    "ref",
    "slides",
    "fragment",
    "speakernote",
    "string",
    "concatenate",
    "uppercase",
    "lowercase",
    "capitalize",
    "isempty",
    "isnotempty",
    "startswith",
    "plaintext",
    "tablesort",
    "tablefilter",
    "tablecompute",
    "tablecolumn",
    "tablecolumns",
    "tablebyrows",
    "text",
    "br",
    "codespan",
    "match",
    "loremipsum",
];

fn rows() -> Vec<Vec<&'static str>> {
    MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').collect())
        .collect()
}

fn compile_source(source: &str) -> (scribium_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn output_text(result: &scribium_core::CompileResult) -> String {
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
fn pinned_public_surface_is_complete_and_unique() {
    let rows = rows();
    assert_eq!(rows.len(), 162);
    assert!(rows.iter().all(|row| row.len() == 5));
    let names: BTreeSet<_> = rows.iter().map(|row| row[0]).collect();
    assert_eq!(names.len(), 162);
    assert_eq!(names, EXPECTED_NAMES.iter().copied().collect());
    assert!(rows.iter().all(|row| row[2].contains(PINNED_SHA)));
    assert!(rows.iter().all(|row| row[2].starts_with(
        "https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/"
    )));
}

#[test]
fn canonical_status_and_owner_counts_are_explicit() {
    let rows = rows();
    let owned = rows
        .iter()
        .filter(|row| row[3].starts_with("#151"))
        .collect::<Vec<_>>();
    assert_eq!(owned.len(), 60);
    assert_eq!(
        owned
            .iter()
            .filter(|row| row[4] == "SUPPORTED_SEMANTICS")
            .count(),
        43
    );
    assert_eq!(owned.iter().filter(|row| row[4] == "PARTIAL").count(), 6);
    for name in ["capitalize", "startswith"] {
        assert_eq!(
            owned.iter().find(|row| row[0] == name).map(|row| row[4]),
            Some("SUPPORTED_SEMANTICS"),
            "Unicode contract should be complete at the audited semantic boundary for {name}"
        );
    }
    assert_eq!(
        owned.iter().filter(|row| row[4] == "UNSUPPORTED").count(),
        10
    );
    assert_eq!(
        owned
            .iter()
            .filter(|row| row[4] == "NOT_APPLICABLE")
            .count(),
        1
    );
    for name in ["localization", "localize"] {
        let row = owned
            .iter()
            .find(|row| row[0] == name)
            .expect("localization row");
        assert_eq!(row[3], "#151");
        assert_eq!(row[4], "UNSUPPORTED");
    }
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row[3], "#150" | "#152" | "#153" | "#154" | "#155"))
            .count(),
        102
    );
    assert!(rows.iter().all(|row| matches!(
        row[4],
        "SUPPORTED_SEMANTICS" | "PARTIAL" | "UNSUPPORTED" | "NOT_APPLICABLE"
    )));
}

#[test]
fn representative_scalar_and_optionality_contracts_are_observable() {
    let source = ".sum {1} {2}\n.uppercase {Hello}\n.isnone {.none}\n";
    let (result, _) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "3\nHELLO\ntrue");
}

#[test]
fn unicode_string_semantics_are_observable_through_the_public_facade() {
    let source = ".capitalize {ǳabc}\n.startswith {Σigma} {ς} ignorecase:{true}\n";
    let (result, source_id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "ǲabc\ntrue");

    for node in &result.ir.nodes {
        let IrNode::Paragraph { content, span } = node else {
            panic!("expected scalar builtin paragraph, got {node:?}");
        };
        assert_eq!(span.source_id, source_id);
        assert!(content.iter().all(|inline| match inline {
            IrInline::Text { span, .. } => span.source_id == source_id,
            _ => true,
        }));
    }

    let invalid_source = ".startswith {Hello} {he} ignorecase:{maybe}\n";
    let (failed, source_id) = compile_source(invalid_source);
    assert_eq!(failed.diagnostics.len(), 1, "{failed:?}");
    assert_eq!(failed.diagnostics[0].code, "E3001");
    assert_eq!(
        failed.diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(failed.ir.nodes.is_empty(), "{failed:?}");
}

#[test]
fn representative_collection_and_failure_contracts_remain_fail_closed() {
    let source =
        ".var {values}\n    - 3\n    - 1\n    - 2\n\n.values::sorted\n.values::getat {9}\n";
    let (result, _) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "1\n2\n3\nNone");

    let (failed, source_id) = compile_source(".sum {true} {2}\n");
    assert_eq!(failed.diagnostics.len(), 1, "{failed:?}");
    assert_eq!(failed.diagnostics[0].code, "E3001");
    assert_eq!(
        failed.diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(failed.ir.nodes.is_empty(), "{failed:?}");
}
