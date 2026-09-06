use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    ResourceAccessError, ResourceProvider, ResourceText,
};
use arkst_ir::IrDocument;
use arkst_markdown::Mode;
use arkst_source::SourceId;
use std::cell::RefCell;

#[derive(Debug, Default)]
struct CountingResources {
    text_requests: RefCell<Vec<(SourceId, String)>>,
    source_requests: RefCell<Vec<(SourceId, String)>>,
}

impl CountingResources {
    fn clear_requests(&self) {
        self.text_requests.borrow_mut().clear();
        self.source_requests.borrow_mut().clear();
    }
}

impl ResourceProvider for CountingResources {
    fn source_path(&self, _source_id: SourceId) -> Option<String> {
        Some("main.qd".into())
    }

    fn read_text(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        self.text_requests
            .borrow_mut()
            .push((source_id, reference.to_string()));
        Err(ResourceAccessError::UnsupportedReference {
            reference: reference.to_string(),
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
        Err(ResourceAccessError::UnsupportedReference {
            reference: reference.to_string(),
        })
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
    resources: &CountingResources,
) -> Vec<arkst_diagnostics::Diagnostic> {
    let (_, diagnostics) = arkst_engine::evaluator::Evaluator::new().evaluate_with_resources(
        resources,
        source_id,
        &document(source, source_id),
        &DocumentMetadataDefaults::default(),
    );
    diagnostics
}

#[test]
fn host_filesystem_references_fail_before_resource_provider_access() {
    let source_id = SourceId(501);
    let resources = CountingResources::default();
    let cases = [
        (".read {/etc/passwd}", ".read", "/etc/passwd"),
        (
            ".json {C:/Users/private/data.json}",
            ".json",
            "C:/Users/private/data.json",
        ),
        (
            r".include {\\server\share\child.qd}",
            ".include",
            r"\\server\share\child.qd",
        ),
        (
            r".subdocument {safe/..\private\child.qd}",
            ".subdocument",
            r"safe/..\private\child.qd",
        ),
        (
            ".includeall\n    - D:private.qd\n    - local.qd",
            ".include",
            "D:private.qd",
        ),
    ];

    for (source, subject, secret_path) in cases {
        resources.clear_requests();
        let diagnostics = evaluate(source, source_id, &resources);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "E8001", "{source}: {diagnostic:?}");
        assert_eq!(
            diagnostic.message,
            format!("`{subject}` cannot access host filesystem paths")
        );
        assert_eq!(
            diagnostic.hints,
            vec!["Use a source-relative logical project path; Arkst does not expose host filesystem access or a `global-read` capability.".to_string()]
        );
        let rendered = format!("{diagnostic:?}");
        assert!(
            !rendered.contains(secret_path),
            "host path leaked through diagnostic: {rendered}"
        );
        assert!(
            resources.text_requests.borrow().is_empty(),
            "text provider was consulted for {source}"
        );
        assert!(
            resources.source_requests.borrow().is_empty(),
            "source provider was consulted for {source}"
        );
    }
}

#[test]
fn logical_traversal_and_uri_references_still_reach_the_provider() {
    let source_id = SourceId(502);
    let resources = CountingResources::default();

    for (source, reference) in [
        (".read {../../outside.txt}", "../../outside.txt"),
        (".read {data:text/plain,abc}", "data:text/plain,abc"),
        (".read {chapter/../data.txt}", "chapter/../data.txt"),
    ] {
        resources.clear_requests();
        let diagnostics = evaluate(source, source_id, &resources);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(
            resources.text_requests.borrow().as_slice(),
            &[(source_id, reference.to_string())]
        );
        assert!(resources.source_requests.borrow().is_empty());
    }
}
