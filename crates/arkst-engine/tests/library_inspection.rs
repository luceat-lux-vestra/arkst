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

fn evaluate_plain(
    source: &str,
    source_id: SourceId,
) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new().evaluate(&document(source, source_id))
}

fn paragraph_texts(document: &IrDocument) -> Vec<String> {
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
        .collect()
}

#[derive(Default)]
struct FakeEnvironment {
    paths: HashMap<SourceId, String>,
    libraries: BTreeMap<String, LoadableLibrarySource>,
    library_requests: RefCell<Vec<String>>,
    text_requests: RefCell<Vec<(SourceId, String)>>,
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
        Err(ResourceAccessError::NotFound {
            path: reference.to_string(),
        })
    }

    fn read_source(
        &self,
        _source_id: SourceId,
        reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError> {
        Err(ResourceAccessError::NotFound {
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

fn evaluate_with_env(
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

#[test]
fn initial_registry_is_stdlib_only_and_unknowns_fail_closed() {
    let source_id = SourceId(1);
    let (result, diagnostics) = evaluate_plain(
        ".libraries\n.libexists {stdlib}\n.libexists {missing}\n.functionexists {size}\n.functionexists {paragraphstyle}\n.libfunctions {missing}",
        source_id,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec!["stdlib", "true", "false", "true", "false"]
    );
}

#[test]
fn logger_native_owners_are_visible_to_function_inspection() {
    let source_id = SourceId(1978);
    let (result, diagnostics) = evaluate_plain(
        ".functionexists {log}\n.functionexists {debug}\n.functionexists {error}",
        source_id,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["true", "true", "true"]);
}

#[test]
fn source_functions_publish_pseudo_libraries_in_declaration_order() {
    let source_id = SourceId(2);
    let source = ".function {local_first}\n    first\n.function {local_second}\n    second\n.libraries\n.libfunctions {__func__local_first}\n.functionexists {local_second}";
    let (result, diagnostics) = evaluate_plain(source, source_id);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec![
            "stdlib",
            "__func__local_first",
            "__func__local_second",
            "local_first",
            "true",
        ]
    );
}

#[test]
fn stdlib_enumeration_is_oracle_ordered_but_support_filtered() {
    let source_id = SourceId(3);
    let (result, diagnostics) = evaluate_plain(".libfunctions {stdlib}", source_id);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let names = paragraph_texts(&result);

    for supported in [
        "functionexists",
        "libexists",
        "libraries",
        "libfunctions",
        "size",
        "include",
        "code",
        "extend",
        "paragraphstyle",
        "pageformat",
    ] {
        assert!(
            names.iter().any(|name| name == supported),
            "missing {supported}: {names:?}"
        );
    }
    assert!(!names.iter().any(|name| name == "llmstxt"));

    let position = |needle: &str| {
        names
            .iter()
            .position(|name| name == needle)
            .unwrap_or_else(|| panic!("missing {needle}"))
    };
    assert!(position("functionexists") < position("libexists"));
    assert!(position("libexists") < position("libraries"));
    assert!(position("libraries") < position("pageformat"));
    assert!(position("pageformat") < position("libfunctions"));
}

#[test]
fn available_but_unloaded_library_is_invisible_and_inspection_never_queries_provider() {
    let main = SourceId(10);
    let alpha = SourceId(11);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "main.qd".into());
    env.libraries.insert(
        "alpha".into(),
        LoadableLibrarySource {
            name: "alpha".into(),
            source_id: alpha,
            text: ".function {alpha_only}\n    alpha".into(),
        },
    );

    let (result, diagnostics) = evaluate_with_env(
        ".libexists {alpha}\n.functionexists {alpha_only}\n.libraries",
        main,
        &env,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["false", "false", "stdlib"]);
    assert!(env.library_requests.borrow().is_empty());
}

#[test]
fn loaded_container_and_function_libraries_follow_registration_order() {
    let main = SourceId(20);
    let alpha = SourceId(21);
    let beta = SourceId(22);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "main.qd".into());
    env.libraries.insert(
        "alpha".into(),
        LoadableLibrarySource {
            name: "alpha".into(),
            source_id: alpha,
            text: ".function {alpha_first}\n    alpha-first\n.function {alpha_second}\n    alpha-second".into(),
        },
    );
    env.libraries.insert(
        "beta".into(),
        LoadableLibrarySource {
            name: "beta".into(),
            source_id: beta,
            text: ".function {beta_only}\n    beta-only".into(),
        },
    );

    let source = ".function {local_first}\n    local-first\n.function {local_second}\n    local-second\n.include {beta}\n.include {alpha}\n.libraries\n.libfunctions {beta}\n.libfunctions {__func__beta_only}\n.libexists {beta}\n.functionexists {alpha_second}";
    let (result, diagnostics) = evaluate_with_env(source, main, &env);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec![
            "stdlib",
            "__func__local_first",
            "__func__local_second",
            "beta",
            "__func__beta_only",
            "alpha",
            "__func__alpha_first",
            "__func__alpha_second",
            "beta_only",
            "true",
            "true",
        ]
    );
    assert_eq!(env.library_requests.borrow().as_slice(), &["beta", "alpha"]);
}

#[test]
fn failed_library_include_rolls_back_container_functions_and_pseudo_libraries() {
    let main = SourceId(30);
    let broken = SourceId(31);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "main.qd".into());
    env.libraries.insert(
        "broken".into(),
        LoadableLibrarySource {
            name: "broken".into(),
            source_id: broken,
            text: ".function {ghost}\n    ghost\n.read {missing.txt}".into(),
        },
    );

    let source = ".include {broken}\n.libexists {broken}\n.libexists {__func__ghost}\n.functionexists {ghost}\n.libraries";
    let (result, diagnostics) = evaluate_with_env(source, main, &env);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(broken)
    );
    assert_eq!(
        paragraph_texts(&result),
        vec!["false", "false", "false", "stdlib"]
    );
}

#[test]
fn inspection_is_case_sensitive() {
    let main = SourceId(40);
    let alpha = SourceId(41);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "main.qd".into());
    env.libraries.insert(
        "alpha".into(),
        LoadableLibrarySource {
            name: "alpha".into(),
            source_id: alpha,
            text: ".function {alpha_only}\n    alpha".into(),
        },
    );

    let (result, diagnostics) = evaluate_with_env(
        ".include {alpha}\n.libexists {alpha}\n.libexists {Alpha}\n.functionexists {alpha_only}\n.functionexists {ALPHA_ONLY}",
        main,
        &env,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec!["true", "false", "true", "false"]
    );
}

#[test]
fn malformed_arguments_are_source_backed_and_do_not_mutate_registry() {
    let source_id = SourceId(50);
    let (result, diagnostics) = evaluate_plain(".libexists\n.libraries", source_id);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert_eq!(paragraph_texts(&result), vec!["stdlib"]);
}

#[test]
fn pure_inspection_is_identical_with_or_without_resource_capabilities() {
    let source_id = SourceId(60);
    let source = ".libraries\n.libexists {stdlib}\n.functionexists {size}";
    let (plain, plain_diagnostics) = evaluate_plain(source, source_id);
    assert!(plain_diagnostics.is_empty(), "{plain_diagnostics:?}");

    let mut env = FakeEnvironment::default();
    env.paths.insert(source_id, "main.qd".into());
    let (resource_backed, resource_diagnostics) = evaluate_with_env(source, source_id, &env);
    assert!(resource_diagnostics.is_empty(), "{resource_diagnostics:?}");
    assert_eq!(paragraph_texts(&plain), paragraph_texts(&resource_backed));
    assert!(env.library_requests.borrow().is_empty());
    assert!(env.text_requests.borrow().is_empty());
}
