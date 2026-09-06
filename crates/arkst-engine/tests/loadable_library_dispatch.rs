use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    LoadableLibraryProvider, LoadableLibrarySource, ResourceAccessError, ResourceProvider,
    ResourceText,
};
use arkst_ir::{IrDocument, IrInline, IrNode};
use arkst_markdown::Mode;
use arkst_source::SourceId;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
struct FakeEnvironment {
    paths: HashMap<SourceId, String>,
    sources: HashMap<(SourceId, String), IncludedSource>,
    text: HashMap<(SourceId, String), Result<ResourceText, ResourceAccessError>>,
    libraries: BTreeMap<String, LoadableLibrarySource>,
    source_requests: RefCell<Vec<(SourceId, String)>>,
    text_requests: RefCell<Vec<(SourceId, String)>>,
    library_requests: RefCell<Vec<String>>,
}

impl ResourceProvider for FakeEnvironment {
    fn source_path(&self, source_id: SourceId) -> Option<String> {
        self.paths.get(&source_id).cloned().or_else(|| {
            self.libraries
                .values()
                .find(|library| library.source_id == source_id)
                .map(|library| format!("@library/{}", library.name))
        })
    }
    fn read_text(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        self.text_requests
            .borrow_mut()
            .push((source_id, reference.to_string()));
        self.text
            .get(&(source_id, reference.to_string()))
            .cloned()
            .unwrap_or_else(|| {
                Err(ResourceAccessError::NotFound {
                    path: reference.to_string(),
                })
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
impl LoadableLibraryProvider for FakeEnvironment {
    fn loadable_library(&self, name: &str) -> Option<LoadableLibrarySource> {
        self.library_requests.borrow_mut().push(name.to_string());
        self.libraries.get(name).cloned()
    }
}
fn document(source: &str, source_id: SourceId) -> IrDocument {
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (document, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
        &parsed.document,
        source_id,
        &DocumentMetadataDefaults::default(),
        Mode::Quarkdown,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    document
}
fn evaluate(
    source: &str,
    source_id: SourceId,
    env: &FakeEnvironment,
) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new().evaluate_with_resources_and_libraries_for_mode(
        env,
        env,
        source_id,
        Mode::Quarkdown,
        &document(source, source_id),
        &DocumentMetadataDefaults::default(),
    )
}
fn paragraph_text(document: &IrDocument) -> String {
    document
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
fn exact_library_name_precedes_file_lookup_and_ignores_sandbox() {
    let main = SourceId(1);
    let fallback = SourceId(2);
    let library = SourceId(100);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "main.qd".into());
    env.paths.insert(fallback, "Hello".into());
    env.sources.insert(
        (main, "hello".into()),
        IncludedSource {
            path: "hello".into(),
            source_id: fallback,
            text: "wrong file".into(),
        },
    );
    env.sources.insert(
        (main, "Hello".into()),
        IncludedSource {
            path: "Hello".into(),
            source_id: fallback,
            text: "file fallback".into(),
        },
    );
    env.libraries.insert(
        "hello".into(),
        LoadableLibrarySource {
            name: "hello".into(),
            source_id: library,
            text: ".var {libvalue} {library}".into(),
        },
    );
    let (result, diagnostics) = evaluate(
        ".include {hello} sandbox:{subdocument}\n.libvalue",
        main,
        &env,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(env.source_requests.borrow().is_empty());
    assert_eq!(env.library_requests.borrow().as_slice(), &["hello"]);
    assert_eq!(paragraph_text(&result), "library");

    env.source_requests.borrow_mut().clear();
    env.library_requests.borrow_mut().clear();
    let (result, diagnostics) = evaluate(".include {Hello}", main, &env);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(env.library_requests.borrow().as_slice(), &["Hello"]);
    assert_eq!(
        env.source_requests.borrow().as_slice(),
        &[(main, "Hello".into())]
    );
    assert_eq!(paragraph_text(&result), "file fallback");
}

#[test]
fn library_resource_access_keeps_includer_base_and_library_span_provenance() {
    let main = SourceId(11);
    let reader = SourceId(110);
    let broken = SourceId(111);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "docs/main.qd".into());
    env.text.insert(
        (main, "data.txt".into()),
        Ok(ResourceText {
            path: "docs/data.txt".into(),
            text: "caller data".into(),
        }),
    );
    env.libraries.insert(
        "reader".into(),
        LoadableLibrarySource {
            name: "reader".into(),
            source_id: reader,
            text: ".read {data.txt}".into(),
        },
    );
    env.libraries.insert(
        "broken".into(),
        LoadableLibrarySource {
            name: "broken".into(),
            source_id: broken,
            text: ".read {missing.txt}".into(),
        },
    );
    let (result, diagnostics) = evaluate(".include {reader} sandbox:{scope}", main, &env);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        env.text_requests.borrow().as_slice(),
        &[(main, "data.txt".into())]
    );
    assert_eq!(paragraph_text(&result), "caller data");

    env.text_requests.borrow_mut().clear();
    let (_, diagnostics) = evaluate(".include {broken}", main, &env);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        env.text_requests.borrow().as_slice(),
        &[(main, "missing.txt".into())]
    );
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(broken)
    );
}

#[test]
fn includeall_uses_library_dispatch_per_element_then_file_fallback() {
    let main = SourceId(21);
    let local = SourceId(22);
    let library = SourceId(120);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "main.qd".into());
    env.paths.insert(local, "local.qd".into());
    env.sources.insert(
        (main, "local.qd".into()),
        IncludedSource {
            path: "local.qd".into(),
            source_id: local,
            text: ".libvalue".into(),
        },
    );
    env.libraries.insert(
        "hello".into(),
        LoadableLibrarySource {
            name: "hello".into(),
            source_id: library,
            text: ".var {libvalue} {library}".into(),
        },
    );
    let (result, diagnostics) = evaluate(
        ".includeall\n    - hello\n    - local.qd\n.libvalue",
        main,
        &env,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        env.library_requests.borrow().as_slice(),
        &["hello", "local.qd"]
    );
    assert_eq!(
        env.source_requests.borrow().as_slice(),
        &[(main, "local.qd".into())]
    );
    assert!(paragraph_text(&result).contains("library"));
}
