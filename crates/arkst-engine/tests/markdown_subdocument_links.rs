use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    ResourceAccessError, ResourceProvider, ResourceRoot, ResourceText,
};
use arkst_ir::{IrDocument, IrInline, IrNode};
use arkst_markdown::Mode;
use arkst_source::SourceId;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Default)]
struct FakeResources {
    paths: HashMap<SourceId, String>,
    sources: HashMap<(SourceId, String), IncludedSource>,
    source_requests: RefCell<Vec<(SourceId, String)>>,
}

impl ResourceProvider for FakeResources {
    fn source_path(&self, source_id: SourceId) -> Option<String> {
        self.paths.get(&source_id).cloned()
    }

    fn relative_path_to_root(
        &self,
        source_id: SourceId,
        _root: ResourceRoot,
    ) -> Result<String, ResourceAccessError> {
        Err(ResourceAccessError::UnknownSource { source_id })
    }

    fn read_text(
        &self,
        _source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        Err(ResourceAccessError::NotFound {
            path: reference.to_string(),
        })
    }

    fn read_source(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError> {
        self.source_requests
            .borrow_mut()
            .push((source_id, reference.to_string()));
        self.sources
            .get(&(source_id, reference.to_string()))
            .cloned()
            .ok_or_else(|| ResourceAccessError::NotFound {
                path: reference.to_string(),
            })
    }
}

fn document(source: &str, source_id: SourceId, mode: Mode) -> IrDocument {
    let parsed = arkst_markdown::parse_with_mode(source, mode);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (document, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
        &parsed.document,
        source_id,
        &DocumentMetadataDefaults::default(),
        mode,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    document
}

fn evaluate(
    source: &str,
    source_id: SourceId,
    mode: Mode,
    resources: &FakeResources,
) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new().evaluate_with_resources_for_mode(
        resources,
        source_id,
        mode,
        &document(source, source_id, mode),
        &DocumentMetadataDefaults::default(),
    )
}

fn link_destinations(document: &IrDocument) -> Vec<&str> {
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => Some(content),
            _ => None,
        })
        .flatten()
        .filter_map(|inline| match inline {
            IrInline::Link { destination, .. } => Some(destination.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn quarkdown_literal_links_classify_only_pinned_suffixes_after_anchor_stripping() {
    let main = SourceId(1);
    let qd = SourceId(2);
    let md = SourceId(3);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(qd, "child.qd".into());
    resources.paths.insert(md, "notes.MD".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: qd,
            text: "not evaluated".into(),
        },
    );
    resources.sources.insert(
        (main, "notes.MD".into()),
        IncludedSource {
            path: "notes.MD".into(),
            source_id: md,
            text: "not evaluated".into(),
        },
    );

    let source = "[QD](child.qd#intro) [MD](notes.MD#Top) [Query](child.qd?x=1) [Bare](child) [MDX](child.mdx) [Local](#local)";
    let (result, diagnostics) = evaluate(source, main, Mode::Quarkdown, &resources);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "child.qd".into()), (main, "notes.MD".into())]
    );
    assert_eq!(
        link_destinations(&result),
        vec![
            "child.qd#intro",
            "notes.MD#Top",
            "child.qd?x=1",
            "child",
            "child.mdx",
            "#local",
        ]
    );
}

#[test]
fn explicit_markdown_mode_keeps_document_links_ordinary_even_for_qd_provider_path() {
    let main = SourceId(1);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    let source = "[QD](child.qd) [MD](other.md#x)";

    let (result, diagnostics) = evaluate(source, main, Mode::Markdown, &resources);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(resources.source_requests.borrow().is_empty());
    assert_eq!(link_destinations(&result), vec!["child.qd", "other.md#x"]);
}

#[test]
fn explicit_quarkdown_mode_classifies_document_links_even_for_md_provider_path() {
    let main = SourceId(1);
    let child = SourceId(2);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.md".into());
    resources.paths.insert(child, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: child,
            text: "target".into(),
        },
    );

    let (result, diagnostics) =
        evaluate("[Child](child.qd#intro)", main, Mode::Quarkdown, &resources);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "child.qd".into())]
    );
    assert_eq!(link_destinations(&result), vec!["child.qd#intro"]);
}

#[test]
fn missing_static_subdocument_target_is_a_deterministic_resource_error() {
    let main = SourceId(1);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());

    let (result, diagnostics) =
        evaluate("[Missing](missing.qd)", main, Mode::Quarkdown, &resources);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3001");
    assert_eq!(
        diagnostics[0].message,
        "Markdown subdocument link resource not found: `missing.qd`"
    );
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(main)
    );
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "missing.qd".into())]
    );
    assert!(link_destinations(&result).is_empty());
}

#[test]
fn nested_include_uses_literal_link_defining_source_for_resolution() {
    let main = SourceId(1);
    let child = SourceId(2);
    let target = SourceId(3);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(child, "chapter/one.qd".into());
    resources.paths.insert(target, "chapter/two.qd".into());
    resources.sources.insert(
        (main, "chapter/one.qd".into()),
        IncludedSource {
            path: "chapter/one.qd".into(),
            source_id: child,
            text: "[Two](two.qd#part)".into(),
        },
    );
    resources.sources.insert(
        (child, "two.qd".into()),
        IncludedSource {
            path: "chapter/two.qd".into(),
            source_id: target,
            text: ".read {missing.txt}".into(),
        },
    );

    let (result, diagnostics) = evaluate(
        ".include {chapter/one.qd}",
        main,
        Mode::Quarkdown,
        &resources,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "chapter/one.qd".into()), (child, "two.qd".into())]
    );
    assert_eq!(link_destinations(&result), vec!["two.qd#part"]);
}

#[test]
fn included_markdown_source_uses_quarkdown_mode_for_static_subdocument_links() {
    let main = SourceId(1);
    let included = SourceId(2);
    let target = SourceId(3);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(included, "docs/part.md".into());
    resources.paths.insert(target, "docs/child.qd".into());
    resources.sources.insert(
        (main, "docs/part.md".into()),
        IncludedSource {
            path: "docs/part.md".into(),
            source_id: included,
            text: "[Child](child.qd#intro)".into(),
        },
    );
    resources.sources.insert(
        (included, "child.qd".into()),
        IncludedSource {
            path: "docs/child.qd".into(),
            source_id: target,
            text: "not evaluated".into(),
        },
    );

    let (result, diagnostics) =
        evaluate(".include {docs/part.md}", main, Mode::Quarkdown, &resources);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "docs/part.md".into()), (included, "child.qd".into())]
    );
    assert_eq!(link_destinations(&result), vec!["child.qd#intro"]);
}

#[test]
fn native_subdocument_output_link_is_not_reclassified_as_static_markdown() {
    let main = SourceId(1);
    let child = SourceId(2);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(child, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: child,
            text: "target".into(),
        },
    );

    let (result, diagnostics) = evaluate(
        ".subdocument {child.qd} {Child}",
        main,
        Mode::Quarkdown,
        &resources,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "child.qd".into())]
    );
    assert_eq!(link_destinations(&result), vec!["child.qd"]);
}
