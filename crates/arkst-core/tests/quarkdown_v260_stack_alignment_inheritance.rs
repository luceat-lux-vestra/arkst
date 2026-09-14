use arkst_core::ir::{
    IrComponent, IrDocumentAlignment, IrMainAxisAlignment, IrNode, IrStackedLayout,
};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

fn stacked(node: &IrNode) -> &arkst_core::ir::IrStackedComponent {
    let IrNode::Component {
        component: IrComponent::Stacked(component),
    } = node
    else {
        panic!("expected stacked component, got {node:?}");
    };
    component
}

#[test]
fn omitted_row_and_column_preserve_inherit_through_final_page_state() {
    let result = compile_source(
        ".row\n    BEFORE\n\n.pageformat alignment:{center}\n.column\n    AFTER\n\n.pageformat alignment:{end}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.page_alignment,
        Some(IrDocumentAlignment::End)
    );
    let [row, column] = result.ir.nodes.as_slice() else {
        panic!("expected row and column only: {:?}", result.ir.nodes);
    };
    assert_eq!(stacked(row).layout, IrStackedLayout::Row);
    assert_eq!(stacked(row).main_axis_alignment, None);
    assert_eq!(stacked(column).layout, IrStackedLayout::Column);
    assert_eq!(stacked(column).main_axis_alignment, None);
}

#[test]
fn explicit_stack_alignment_remains_authoritative_under_conflicting_global_state() {
    let result = compile_source(
        ".pageformat alignment:{center}\n.row alignment:{start}\n    A\n\n.column alignment:{spacebetween}\n    B\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let [row, column] = result.ir.nodes.as_slice() else {
        panic!("expected row and column only: {:?}", result.ir.nodes);
    };
    assert_eq!(
        stacked(row).main_axis_alignment,
        Some(IrMainAxisAlignment::Start)
    );
    assert_eq!(
        stacked(column).main_axis_alignment,
        Some(IrMainAxisAlignment::SpaceBetween)
    );
}

#[test]
fn justify_global_state_does_not_fabricate_a_stack_alignment() {
    let result = compile_source(".pageformat alignment:{justify}\n.row\n    A\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.page_alignment,
        Some(IrDocumentAlignment::Justify)
    );
    assert_eq!(stacked(&result.ir.nodes[0]).main_axis_alignment, None);
}

#[test]
fn align_and_center_wrappers_do_not_stamp_omitted_stack_alignment() {
    let aligned = compile_source(".align {center}\n    .row\n        A\n");
    assert!(aligned.diagnostics.is_empty(), "{:?}", aligned.diagnostics);
    let [IrNode::Component {
        component: IrComponent::Container(container),
    }] = aligned.ir.nodes.as_slice()
    else {
        panic!("expected align container: {:?}", aligned.ir.nodes);
    };
    assert_eq!(stacked(&container.children[0]).main_axis_alignment, None);

    let centered = compile_source(".center\n    .column\n        A\n");
    assert!(centered.diagnostics.is_empty(), "{:?}", centered.diagnostics);
    let [IrNode::Component {
        component: IrComponent::Container(container),
    }] = centered.ir.nodes.as_slice()
    else {
        panic!("expected center container: {:?}", centered.ir.nodes);
    };
    assert_eq!(stacked(&container.children[0]).main_axis_alignment, None);
}

#[test]
fn nested_omitted_and_explicit_stacks_remain_independent() {
    let result = compile_source(
        ".pageformat alignment:{center}\n.row\n    .column alignment:{end}\n        INNER\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let outer = stacked(&result.ir.nodes[0]);
    assert_eq!(outer.main_axis_alignment, None);
    let inner = stacked(&outer.children[0]);
    assert_eq!(
        inner.main_axis_alignment,
        Some(IrMainAxisAlignment::End)
    );
}

#[test]
fn source_defined_row_and_column_keep_ownership() {
    let result = compile_source(
        ".function {row}\n    SHADOW-ROW\n\n.function {column}\n    SHADOW-COLUMN\n\n.pageformat alignment:{center}\n.row\n.column\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let debug = format!("{:?}", result.ir.nodes);
    assert!(debug.contains("SHADOW-ROW"), "{debug}");
    assert!(debug.contains("SHADOW-COLUMN"), "{debug}");
    assert!(!debug.contains("Stacked"), "{debug}");
}
