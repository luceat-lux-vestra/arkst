use scribium_diagnostics::{Diagnostic, Severity};
use scribium_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    ResourceAccessError, ResourceProvider, ResourceText,
};
use scribium_ir::{IrDocument, IrInline, IrNode};
use scribium_markdown::Mode;
use scribium_source::{SourceId, SourceSpan};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Default)]
struct FakeResources {
    paths: HashMap<SourceId, String>,
    sources: HashMap<(SourceId, String), IncludedSource>,
    text: HashMap<(SourceId, String), Result<ResourceText, ResourceAccessError>>,
    source_requests: RefCell<Vec<(SourceId, String)>>,
    text_requests: RefCell<Vec<(SourceId, String)>>,
}

impl ResourceProvider for FakeResources {
    fn source_path(&self, source_id: SourceId) -> Option<String> {
        self.paths.get(&source_id).cloned()
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

fn document(source: &str, source_id: SourceId) -> IrDocument {
    let parsed = scribium_markdown::parse_with_mode(source, Mode::Quarkdown);
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
    resources: &FakeResources,
) -> (IrDocument, Vec<scribium_diagnostics::Diagnostic>) {
    scribium_engine::evaluator::Evaluator::new().evaluate_with_resources(
        resources,
        source_id,
        &document(source, source_id),
        &DocumentMetadataDefaults::default(),
    )
}

fn assert_exact_resource_diagnostic(
    diagnostic: &Diagnostic,
    code: &str,
    message: &str,
    source_id: SourceId,
    end: usize,
    hint: &str,
) {
    assert_eq!(diagnostic.code, code);
    assert!(matches!(diagnostic.severity, Severity::Error));
    assert_eq!(diagnostic.message, message);
    assert_eq!(diagnostic.primary, Some(SourceSpan::new(source_id, 0, end)));
    assert!(diagnostic.secondary.is_empty());
    assert_eq!(diagnostic.hints, vec![hint.to_string()]);
}

#[test]
fn nested_include_uses_included_source_identity_for_following_resources() {
    let main = SourceId(1);
    let first = SourceId(2);
    let nested = SourceId(3);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(first, "chapter/one.qd".into());
    resources.paths.insert(nested, "chapter/two.qd".into());
    resources.sources.insert(
        (main, "chapter/one.qd".into()),
        IncludedSource {
            path: "chapter/one.qd".into(),
            source_id: first,
            text: ".include {two.qd}".into(),
        },
    );
    resources.sources.insert(
        (first, "two.qd".into()),
        IncludedSource {
            path: "chapter/two.qd".into(),
            source_id: nested,
            text: ".read {data.txt}".into(),
        },
    );
    resources.text.insert(
        (nested, "data.txt".into()),
        Ok(ResourceText {
            path: "chapter/data.txt".into(),
            text: "nested text".into(),
        }),
    );

    let (result, diagnostics) = evaluate(".include {chapter/one.qd}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "chapter/one.qd".into()), (first, "two.qd".into()),]
    );
    assert_eq!(
        resources.text_requests.borrow().as_slice(),
        &[(nested, "data.txt".into())]
    );
    assert_eq!(result.nodes.len(), 1);
    let IrNode::Paragraph { content, .. } = &result.nodes[0] else {
        panic!("expected included read paragraph")
    };
    assert!(
        matches!(&content[0], IrInline::Text { content, span } if content == "nested text" && span.source_id == nested)
    );
}

#[test]
fn include_cycles_are_reported_without_suppressing_completed_includes() {
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
            text: ".include {main.qd}".into(),
        },
    );
    resources.sources.insert(
        (child, "main.qd".into()),
        IncludedSource {
            path: "main.qd".into(),
            source_id: main,
            text: "cycle".into(),
        },
    );

    let (_, diagnostics) = evaluate(".include {child.qd}", main, &resources);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E3001");
    assert!(diagnostics[0]
        .message
        .contains("main.qd -> child.qd -> main.qd"));
}

#[test]
fn completed_repeated_includes_are_not_globally_suppressed() {
    let main = SourceId(11);
    let child = SourceId(12);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(child, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: child,
            text: "repeated".into(),
        },
    );

    let (result, diagnostics) =
        evaluate(".include {child.qd}\n.include {child.qd}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(result.nodes.len(), 2);
}

#[test]
fn resource_failures_preserve_engine_diagnostic_categories_and_paths() {
    let source_id = SourceId(7);
    let mut resources = FakeResources::default();
    resources.paths.insert(source_id, "main.qd".into());
    resources.text.insert(
        (source_id, "unsupported".into()),
        Err(ResourceAccessError::UnsupportedReference {
            reference: "https://example.test/data".into(),
        }),
    );
    resources.text.insert(
        (source_id, "bad.bin".into()),
        Err(ResourceAccessError::InvalidUtf8 {
            path: "bad.bin".into(),
            message: "invalid utf-8 sequence".into(),
        }),
    );

    let (_, unsupported) = evaluate(".read {unsupported}", source_id, &resources);
    assert_exact_resource_diagnostic(
        &unsupported[0],
        "E8001",
        "`.read` does not support non-local resource reference `https://example.test/data`",
        source_id,
        19,
        "Only source-relative paths inside the supplied VirtualProject are available; network fetching is disabled.",
    );

    let (_, invalid_utf8) = evaluate(".read {bad.bin}", source_id, &resources);
    assert_exact_resource_diagnostic(
        &invalid_utf8[0],
        "E3001",
        "`.read` resource `bad.bin` is not valid UTF-8: invalid utf-8 sequence",
        source_id,
        15,
        "Text resource builtins require valid UTF-8 and do not perform lossy decoding.",
    );
}

#[test]
fn missing_resource_is_a_source_backed_failure() {
    let source_id = SourceId(9);
    let resources = FakeResources {
        paths: [(source_id, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };
    let (_, diagnostics) = evaluate(".read {missing.txt}", source_id, &resources);
    assert_exact_resource_diagnostic(
        &diagnostics[0],
        "E3001",
        "`.read` resource not found: `missing.txt`",
        source_id,
        19,
        "Add the logical resource to the VirtualProject supplied by the host.",
    );
}

#[test]
fn missing_include_source_keeps_its_legacy_diagnostic_hint() {
    let source_id = SourceId(10);
    let resources = FakeResources {
        paths: [(source_id, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };
    let (_, diagnostics) = evaluate(".include {missing.qd}", source_id, &resources);

    assert_exact_resource_diagnostic(
        &diagnostics[0],
        "E3001",
        "`.include` resource not found: `missing.qd`",
        source_id,
        21,
        "Add the target source to the VirtualProject supplied by the host.",
    );
}

#[test]
fn resource_builtin_without_provider_keeps_its_legacy_context_diagnostic() {
    let source_id = SourceId(11);
    let document = document(".read {missing.txt}", source_id);
    let (_, diagnostics) = scribium_engine::evaluator::Evaluator::new().evaluate(&document);

    assert_exact_resource_diagnostic(
        &diagnostics[0],
        "E8001",
        "Resource builtin requires a host-supplied VirtualProject",
        source_id,
        19,
        "Compile through the project API so logical resources are supplied explicitly.",
    );
}
