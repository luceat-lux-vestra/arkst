//! Independent regression coverage for the bounded #175 selector-free
//! `.pageformat` standard size/orientation state slice.

use arkst_core::ir::{
    IrDocumentState, IrDocumentType, IrPageOrientation, IrPageSizeFormat, IrPageSizeSelection,
};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

#[test]
fn standard_page_size_domain_is_closed_and_typed() {
    for (raw, expected) in [
        ("a0", IrPageSizeFormat::A0),
        ("A1", IrPageSizeFormat::A1),
        ("a2", IrPageSizeFormat::A2),
        ("A3", IrPageSizeFormat::A3),
        ("a4", IrPageSizeFormat::A4),
        ("A5", IrPageSizeFormat::A5),
        ("a6", IrPageSizeFormat::A6),
        ("A7", IrPageSizeFormat::A7),
        ("a8", IrPageSizeFormat::A8),
        ("A9", IrPageSizeFormat::A9),
        ("a10", IrPageSizeFormat::A10),
        ("B0", IrPageSizeFormat::B0),
        ("b1", IrPageSizeFormat::B1),
        ("B2", IrPageSizeFormat::B2),
        ("b3", IrPageSizeFormat::B3),
        ("B4", IrPageSizeFormat::B4),
        ("b5", IrPageSizeFormat::B5),
        ("letter", IrPageSizeFormat::Letter),
        ("LEGAL", IrPageSizeFormat::Legal),
        ("Ledger", IrPageSizeFormat::Ledger),
    ] {
        let result = compile_source(&format!(".pageformat size:{{{raw}}}\n"));
        assert!(
            result.diagnostics.is_empty(),
            "{raw}: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.ir.metadata.document_state.page_size,
            Some(IrPageSizeSelection {
                format: expected,
                orientation: None,
                document_type: IrDocumentType::Plain,
            }),
            "{raw}"
        );
        assert!(result.ir.nodes.is_empty(), "setter must not emit content");
    }
}

#[test]
fn named_standard_size_with_orientation_is_supported() {
    let oriented =
        compile_source(".doctype {paged}\n.pageformat size:{a4} orientation:{landscape}\n");
    assert!(oriented.diagnostics.is_empty(), "{oriented:?}");
    assert_eq!(
        oriented.ir.metadata.document_state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        })
    );
}

#[test]
fn explicit_orientation_is_typed_and_document_type_basis_is_captured() {
    let result =
        compile_source(".doctype {paged}\n.pageformat size:{a4} orientation:{LANDSCAPE}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        })
    );

    let omitted = compile_source(".doctype {slides}\n.pageformat size:{letter}\n");
    assert!(omitted.diagnostics.is_empty(), "{omitted:?}");
    assert_eq!(
        omitted.ir.metadata.document_state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::Letter,
            orientation: None,
            document_type: IrDocumentType::Slides,
        }),
        "omitted orientation must preserve the call-time document-type basis"
    );
}

#[test]
fn later_global_standard_size_selection_replaces_the_previous_selection() {
    let result = compile_source(
        ".pageformat size:{a4} orientation:{portrait}\n.pageformat size:{legal} orientation:{landscape}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::Legal,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Plain,
        })
    );
}

#[test]
fn invalid_size_or_orientation_preserves_the_last_committed_selection() {
    for invalid in [
        ".pageformat size:{not-a-paper}\n",
        ".pageformat size:{a5} orientation:{diagonal}\n",
    ] {
        let result = compile_source(&format!(
            ".pageformat size:{{a4}} orientation:{{portrait}}\n{invalid}"
        ));
        assert!(!result.diagnostics.is_empty(), "{invalid}");
        assert_eq!(
            result.ir.metadata.document_state.page_size,
            Some(IrPageSizeSelection {
                format: IrPageSizeFormat::A4,
                orientation: Some(IrPageOrientation::Portrait),
                document_type: IrDocumentType::Plain,
            }),
            "failed conversion replaced committed state: {invalid:?}"
        );
    }
}

#[test]
fn unsupported_pageformat_shapes_do_not_claim_standard_size_state() {
    for unsupported in [
        ".pageformat orientation:{landscape}\n",
        ".pageformat {letter}\n",
        ".pageformat {a4} orientation:{landscape}\n",
        ".pageformat {1..2}\n",
        ".pageformat {letter} columns:{2}\n",
        ".pageformat size:{letter} width:{8in} height:{11in}\n",
        ".pageformat side:{left} size:{a5}\n",
        ".pageformat pages:{1..2} size:{a5}\n",
        ".pageformat size:{a5} columns:{2}\n",
    ] {
        let result = compile_source(&format!(".pageformat size:{{a4}}\n{unsupported}"));
        assert_eq!(
            result.ir.metadata.document_state.page_size,
            Some(IrPageSizeSelection {
                format: IrPageSizeFormat::A4,
                orientation: None,
                document_type: IrDocumentType::Plain,
            }),
            "unsupported shape mutated bounded state: {unsupported:?}; diagnostics={:?}; IR={:?}",
            result.diagnostics,
            result.ir
        );
    }
}

#[test]
fn semantic_none_preserves_the_previous_standard_size_selection() {
    let result =
        compile_source(".pageformat size:{a4} orientation:{landscape}\n.pageformat size:{.none}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Plain,
        }),
        "semantic None must preserve the previously committed effective state"
    );
}

#[test]
fn failed_size_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n.pageformat size:{a4}\n.function {badsize}\n    .autopagebreak maxdepth:{1}\n    notapaper\n\n.pageformat size:{.badsize}\n",
    );
    assert!(!result.diagnostics.is_empty(), "invalid size must fail");
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure leaked nested document-state mutation"
    );
    assert_eq!(
        result.ir.metadata.document_state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: None,
            document_type: IrDocumentType::Plain,
        })
    );
}

#[test]
fn source_defined_pageformat_shadows_the_bounded_standard_size_builtin() {
    let result = compile_source(
        ".function {pageformat}\n    size:\n    SHADOW-PAGEFORMAT-SIZE\n\n.pageformat size:{a4}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.page_size, None);
    assert!(
        format!("{:?}", result.ir).contains("SHADOW-PAGEFORMAT-SIZE"),
        "source-defined function must retain ownership of the name"
    );
}

#[test]
fn standard_size_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_size").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.page_size, None);

    let explicit = IrDocumentState {
        page_size: Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        }),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert!(value.get("page_size").is_some());
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored, explicit);
}
