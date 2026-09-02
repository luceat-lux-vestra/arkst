fn accepts_core_document(_: arkst_core::ir::IrDocument) {}
fn accepts_physical_document(_: arkst_ir::IrDocument) {}
fn accepts_core_metadata(_: arkst_core::ir::IrMetadata) {}
fn accepts_physical_metadata(_: arkst_ir::IrMetadata) {}
fn accepts_core_node(_: arkst_core::ir::IrNode) {}
fn accepts_physical_node(_: arkst_ir::IrNode) {}
fn accepts_core_inline(_: arkst_core::ir::IrInline) {}
fn accepts_physical_inline(_: arkst_ir::IrInline) {}
fn accepts_core_value(_: arkst_core::ir::IrValue) {}
fn accepts_physical_value(_: arkst_ir::IrValue) {}
fn accepts_source_map_entry(_: arkst_core::ir::SourceMapEntry) {}

#[test]
fn core_ir_facade_preserves_physical_type_identity() {
    let _: fn(arkst_core::ir::IrDocument) = accepts_physical_document;
    let _: fn(arkst_ir::IrDocument) = accepts_core_document;
    let _: fn(arkst_core::ir::IrMetadata) = accepts_physical_metadata;
    let _: fn(arkst_ir::IrMetadata) = accepts_core_metadata;
    let _: fn(arkst_core::ir::IrNode) = accepts_physical_node;
    let _: fn(arkst_ir::IrNode) = accepts_core_node;
    let _: fn(arkst_core::ir::IrInline) = accepts_physical_inline;
    let _: fn(arkst_ir::IrInline) = accepts_core_inline;
    let _: fn(arkst_core::ir::IrValue) = accepts_physical_value;
    let _: fn(arkst_ir::IrValue) = accepts_core_value;

    // SourceMapEntry remains available through the core facade after its
    // physical extraction to arkst-source.
    let _: fn(arkst_core::ir::SourceMapEntry) = accepts_source_map_entry;
}
