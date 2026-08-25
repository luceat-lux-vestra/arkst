//! Offline completeness and semantic witnesses for Issue #152.

use std::collections::{BTreeSet, HashMap};

use scribium_core::ir::IrDocumentType;
use scribium_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

const MANIFEST: &str =
    include_str!("../../../docs/compatibility/quarkdown/DOCUMENT_STATE_AUDIT_MANIFEST.tsv");
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const BASE_SHA: &str = "1bd8cda073be4194ffce8e9e58ef4cfc4d742be1";
const STATUSES: &[&str] = &[
    "SUPPORTED_END_TO_END",
    "SUPPORTED_SEMANTICS",
    "PARSED_ONLY",
    "PARTIAL",
    "UNSUPPORTED",
    "DEFERRED",
    "BLOCKED",
    "NOT_APPLICABLE",
    "UNKNOWN",
];
const OWNED_NAMES: &[&str] = &[
    "docauthor",
    "docauthors",
    "docdescription",
    "dockeywords",
    "doclang",
    "docname",
    "doctype",
    "localization",
    "localize",
    "theme",
];

fn manifest_rows() -> Vec<Vec<&'static str>> {
    MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').collect())
        .collect()
}

fn declared_counts() -> HashMap<&'static str, usize> {
    MANIFEST
        .lines()
        .filter_map(|line| line.strip_prefix("# declared_"))
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name, value.parse().expect("numeric manifest declaration")))
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

#[test]
fn manifest_is_complete_and_machine_checkable() {
    let rows = manifest_rows();
    let declarations = declared_counts();
    assert_eq!(declarations.get("total"), Some(&rows.len()));
    assert_eq!(declarations.get("152_owned"), Some(&10));
    assert_eq!(declarations.get("cross_owned"), Some(&(rows.len() - 10)));

    let mut names = BTreeSet::new();
    let mut owned = BTreeSet::new();
    for row in &rows {
        assert_eq!(row.len(), 8, "manifest row has wrong column count: {row:?}");
        assert!(matches!(row[0], "owned" | "cross-owned"));
        assert!(names.insert(row[1]), "duplicate canonical name: {}", row[1]);
        assert!(STATUSES.contains(&row[4]), "invalid status: {}", row[4]);
        assert!(row[2] == "none" || row[2].split(';').all(|alias| !alias.is_empty()));
        assert!(matches!(row[3], "#152" | "#153" | "#154" | "#155"));
        assert!(
            row[5].contains(TARGET_SHA),
            "missing pinned provenance: {row:?}"
        );
        assert!(!row[6].is_empty() && !row[7].is_empty());
        if row[3] == "#152" {
            assert_eq!(row[0], "owned");
            assert!(owned.insert(row[1]));
        } else {
            assert_eq!(row[0], "cross-owned");
        }
    }
    assert_eq!(owned.into_iter().collect::<Vec<_>>(), OWNED_NAMES);
    assert_eq!(rows.len(), 43);
    assert_eq!(rows.iter().filter(|row| row[3] == "#153").count(), 20);
    assert_eq!(rows.iter().filter(|row| row[3] == "#154").count(), 3);
    assert_eq!(rows.iter().filter(|row| row[3] == "#155").count(), 10);
    assert!(MANIFEST.contains(BASE_SHA));
}

#[test]
fn document_state_witness_covers_defaults_and_mutation_contracts() {
    let source = ".docauthor {Alice}\n.docauthor {Bob}\n.docauthors\n    - Carol:\n        - email: carol@example.test\n.dockeywords\n    - one\n    - one\n.doclang {en-US}\n.theme {Dark} layout:{Minimal}\n";
    let (result, _) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let state = &result.ir.metadata.document_state;
    assert_eq!(state.document_type, IrDocumentType::Plain);
    assert_eq!(state.name, "");
    assert_eq!(state.description, "");
    assert_eq!(
        state
            .authors
            .iter()
            .map(|author| author.name.as_str())
            .collect::<Vec<_>>(),
        ["Alice", "Bob", "Carol"]
    );
    assert_eq!(state.keywords, ["one", "one"]);
    assert_eq!(
        state.locale.as_ref().map(|locale| locale.tag.as_str()),
        Some("en-US")
    );
    assert_eq!(
        state
            .theme
            .as_ref()
            .and_then(|theme| theme.color.as_deref()),
        Some("dark")
    );
    assert_eq!(
        state
            .theme
            .as_ref()
            .and_then(|theme| theme.layout.as_deref()),
        Some("minimal")
    );
}

#[test]
fn document_state_witness_covers_nested_visibility_and_atomic_failure() {
    let source = ".doclang {en}\n.doclang {.pair {.doclang {it}} {invalid}}\n.doclang\n";
    let (result, source_id) = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert_eq!(
        result.diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert_eq!(
        result
            .ir
            .metadata
            .document_state
            .locale
            .as_ref()
            .map(|locale| locale.tag.as_str()),
        Some("en")
    );
}
