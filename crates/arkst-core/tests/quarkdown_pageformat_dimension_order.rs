//! Regression coverage for the bounded selector-free page-dimension ordering
//! bridge owned by #175.

use arkst_core::ir::{
    IrDocumentState, IrDocumentType, IrPageDimensionLayer, IrPageGeometry, IrPageOrientation,
    IrPageSizeFormat, IrPageSizeSelection, IrSize, IrSizeUnit,
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
fn selector_free_dimension_layers_preserve_source_order_and_flat_compatibility() {
    let result = compile_source(
        ".doctype {paged}\n\
.pageformat size:{a4}\n\
.pageformat width:{10in} height:{5in}\n\
.pageformat size:{letter} orientation:{landscape}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

    let state = &result.ir.metadata.document_state;
    assert_eq!(
        state.page_dimension_layers,
        vec![
            IrPageDimensionLayer::StandardSize(IrPageSizeSelection {
                format: IrPageSizeFormat::A4,
                orientation: None,
                document_type: IrDocumentType::Paged,
            }),
            IrPageDimensionLayer::ExplicitGeometry(IrPageGeometry {
                width: IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::In,
                },
                height: IrSize {
                    value: 5.0,
                    unit: IrSizeUnit::In,
                },
            }),
            IrPageDimensionLayer::StandardSize(IrPageSizeSelection {
                format: IrPageSizeFormat::Letter,
                orientation: Some(IrPageOrientation::Landscape),
                document_type: IrDocumentType::Paged,
            }),
        ]
    );

    assert_eq!(
        state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::Letter,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        }),
        "legacy flat size remains populated"
    );
    assert_eq!(
        state.page_geometry,
        Some(IrPageGeometry {
            width: IrSize {
                value: 10.0,
                unit: IrSizeUnit::In,
            },
            height: IrSize {
                value: 5.0,
                unit: IrSizeUnit::In,
            },
        }),
        "legacy flat geometry remains populated"
    );
}

#[test]
fn reverse_order_ends_with_explicit_geometry() {
    let result = compile_source(
        ".doctype {paged}\n\
.pageformat size:{a4} orientation:{landscape}\n\
.pageformat width:{8in} height:{4in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

    assert!(matches!(
        result
            .ir
            .metadata
            .document_state
            .page_dimension_layers
            .last(),
        Some(IrPageDimensionLayer::ExplicitGeometry(geometry))
            if geometry.width.value == 8.0
                && geometry.width.unit == IrSizeUnit::In
                && geometry.height.value == 4.0
                && geometry.height.unit == IrSizeUnit::In
    ));
}

#[test]
fn failed_outer_conversion_rolls_back_nested_dimension_layer_write() {
    let result = compile_source(
        ".doctype {paged}\n\
.pageformat size:{a4}\n\
.function {badheight}\n\
    .pageformat size:{letter}\n\
    invalid\n\
\n\
.pageformat width:{8in} height:{.badheight}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "invalid nested height must fail conversion"
    );

    let state = &result.ir.metadata.document_state;
    assert_eq!(
        state.page_dimension_layers,
        vec![IrPageDimensionLayer::StandardSize(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: None,
            document_type: IrDocumentType::Paged,
        })],
        "nested dimension mutation must roll back with the failed outer invocation"
    );
    assert_eq!(
        state.page_size,
        Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: None,
            document_type: IrDocumentType::Paged,
        })
    );
    assert!(state.page_geometry.is_none());
}

#[test]
fn dimension_layer_wire_is_backward_compatible_and_roundtrips() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(
        legacy_shape.get("page_dimension_layers").is_none(),
        "empty ordered layer state must remain absent from legacy-shaped wire data"
    );
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert!(restored.page_dimension_layers.is_empty());

    let explicit = IrDocumentState {
        page_dimension_layers: vec![
            IrPageDimensionLayer::StandardSize(IrPageSizeSelection {
                format: IrPageSizeFormat::A5,
                orientation: Some(IrPageOrientation::Portrait),
                document_type: IrDocumentType::Paged,
            }),
            IrPageDimensionLayer::ExplicitGeometry(IrPageGeometry {
                width: IrSize {
                    value: 7.0,
                    unit: IrSizeUnit::In,
                },
                height: IrSize {
                    value: 3.5,
                    unit: IrSizeUnit::In,
                },
            }),
        ],
        ..IrDocumentState::default()
    };
    let wire = serde_json::to_value(&explicit).expect("serialize ordered layers");
    let roundtrip: IrDocumentState =
        serde_json::from_value(wire).expect("deserialize ordered layers");
    assert_eq!(roundtrip, explicit);
}
