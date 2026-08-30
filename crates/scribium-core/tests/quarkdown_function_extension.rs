use scribium_core::ir::{IrInline, IrNode};
use scribium_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

fn compile_source(source: &str) -> scribium_core::CompileResult {
    compile_source_with_id(source).0
}

fn compile_source_with_id(source: &str) -> (scribium_core::CompileResult, SourceId) {
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

fn paragraph_texts(result: &scribium_core::CompileResult) -> Vec<String> {
    result
        .ir
        .nodes
        .iter()
        .map(|node| {
            let IrNode::Paragraph { content, .. } = node else {
                panic!("expected paragraph, got {node:?}");
            };
            content
                .iter()
                .map(|inline| match inline {
                    IrInline::Text { content, .. } => content.as_str(),
                    other => panic!("expected text, got {other:?}"),
                })
                .collect()
        })
        .collect()
}

fn paragraph_text(result: &scribium_core::CompileResult) -> String {
    let texts = paragraph_texts(result);
    assert_eq!(
        texts.len(),
        1,
        "expected one paragraph: {:?}",
        result.ir.nodes
    );
    texts.into_iter().next().expect("one paragraph")
}

fn paragraph_node_text(node: &IrNode) -> String {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    content
        .iter()
        .map(|inline| match inline {
            IrInline::Text { content, .. } => content.as_str(),
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

#[test]
fn basic_extension_calls_super_with_the_original_parameters() {
    let source = ".function {greet}\n    name:\n    Hello, .name!\n\n.extend {greet}\n    name:\n    .super\n\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "Hello, world!");
}

#[test]
fn extension_without_header_forwards_explicit_target_parameters() {
    let source = ".function {greet}\n    name:\n    Hello, .name\n\n.extend {greet}\n    .super::uppercase\n\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "HELLO, WORLD");
}

#[test]
fn super_without_override_and_with_positional_override_preserve_parameter_names() {
    let source = ".function {greet}\n    name:\n    Hello, .name!\n\n.extend {greet}\n    name:\n    .super {overridden}\n\n.greet {original}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "Hello, overridden!");

    let no_override = source.replace(".super {overridden}", ".super");
    let result = compile_source(&no_override);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "Hello, original!");
}

#[test]
fn named_super_override_wins_without_rebinding_other_target_parameters() {
    let source = ".function {greet}\n    greeting name:\n    .greeting, .name!\n\n.extend {greet}\n    name:\n    .name, .super greeting:{Howdy}\n\n.greet {Hello} {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "world, Howdy, world!");
}

#[test]
fn chained_extensions_delegate_to_the_immediately_previous_callable() {
    let source = ".function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    first\n    .super\n\n.extend {greet}\n    name:\n    second\n    .super\n\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["first", "second", "base world"]);
}

#[test]
fn three_chained_extensions_keep_the_last_super_override() {
    let source = ".function {greet}\n    name:\n    .name\n\n.extend {greet}\n    name:\n    .super name:{blue}\n\n.extend {greet}\n    name:\n    .super name:{green}\n\n.extend {greet}\n    name:\n    .super name:{red}\n\n.greet {user}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "red");
}

#[test]
fn conditional_extensions_delegate_only_when_the_condition_is_false() {
    let source = ".function {greet}\n    name:\n    base .name\n\n.extend {greet} where:{name: .name::equals {world}}\n    name:\n    extended\n\n.greet {world}\n.greet {other}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["extended", "base other"]);
}

#[test]
fn conditional_extension_can_bind_a_partial_multi_parameter_condition() {
    let source = ".function {mysum}\n    a b:\n    .a::sum {.b}\n\n.extend {mysum} where:{a: .a::islower than:{10}}\n    a:\n    .a\n\n.mysum {10} {11} .mysum {4} {8}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "21 4");
}

#[test]
fn extension_target_failures_are_source_backed_and_do_not_fabricate_output() {
    for source in [
        ".extend {missing}\n    name:\n    body\n",
        ".extend {1}\n    name:\n    body\n",
    ] {
        let result = compile_source(source);
        assert!(result.ir.nodes.is_empty(), "{result:?}");
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert!(result.diagnostics[0].primary.is_some(), "{result:?}");
    }
}

#[test]
fn super_outside_extension_is_deterministic_failure() {
    let result = compile_source(".super\n");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert!(result.diagnostics[0]
        .message
        .contains("inside an extension"));
}

#[test]
fn source_defined_super_is_callable_outside_but_shadowed_inside_extensions() {
    let source = ".function {super}\n    value:\n    source .value\n\n.function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    .super\n\n.super {outside}\n.greet {inside}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["source outside", "base inside"]);
}

#[test]
fn extension_condition_failure_is_not_double_wrapped() {
    let source = ".function {greet}\n    name:\n    base .name\n\n.extend {greet} where:{name: .sum {true} {2}}\n    name:\n    extended\n\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert!(result.diagnostics[0]
        .message
        .contains("requires numeric arguments"));
}

#[test]
fn native_target_extension_keeps_native_precedence_and_body_policy() {
    let result =
        compile_source(".extend {lowercase}\n    .super::uppercase\n\n.lowercase {hello}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "HELLO");
}

#[test]
fn source_defined_target_precedes_a_same_named_native_builtin() {
    let source = ".function {lowercase}\n    text:\n    custom .text\n\n.extend {lowercase}\n    text:\n    .super\n\n.lowercase {hello}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "custom hello");
}

#[test]
fn native_extension_raw_body_stays_lazy_and_is_not_reparsed_as_nested_state() {
    let source = ".var {state} {before}\n.extend {lowercase}\n    .super\n\n.lowercase\n    .state {changed}\n\n.state\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), [".state {changed}", "before"]);
}

#[test]
fn implicit_user_callable_extension_preserves_positional_contract() {
    let source = ".function {identity}\n    .1\n\n.extend {identity}\n    .super::uppercase\n\n.identity {hello}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "HELLO");
}

#[test]
fn extensions_invoked_from_another_callable_keep_nested_super_scope() {
    let source = ".function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    wrapped\n    .super\n\n.function {call}\n    name:\n    .greet {.name}\n\n.call {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["wrapped", "base world"]);
}

#[test]
fn extension_preserves_recursion_through_the_current_callable_chain() {
    let source = ".function {count}\n    n:\n    .if {.n::equals {0}}\n        done\n    .ifnot {.n::equals {0}}\n        .count {.n::subtract {1}}\n\n.extend {count}\n    n:\n    .super\n\n.count {2}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "done");
}

#[test]
fn extension_can_write_a_caller_visible_variable_on_success() {
    let source = ".var {state} {before}\n.function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    .state {changed}\n    .super\n\n.greet {world}\n.state\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["base world", "changed"]);
}

#[test]
fn local_chained_extension_is_visible_until_the_callable_scope_ends() {
    let source = ".function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    first\n    .super\n\n.function {register}\n    name:\n    .extend {greet}\n        name:\n        local .super\n    .greet {.name}\n\n.register {world}\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_texts(&result),
        ["first", "local base world", "first", "base world"]
    );
}

#[test]
fn extension_call_uses_definition_and_caller_visible_scope_layers() {
    let source = ".var {prefix} {definition}\n.function {greet}\n    name:\n    .prefix\n    .suffix\n\n.extend {greet}\n    name:\n    .super\n\n.var {suffix} {caller}\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["definition", "caller"]);
}

#[test]
fn extension_local_parameters_and_original_parameter_reads_are_distinct() {
    let source = ".function {greet}\n    name:\n    original .name\n\n.extend {greet}\n    name:\n    local .name\n    .super\n\n.greet {world}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_texts(&result), ["local world", "original world"]);
}

#[test]
fn failed_extension_body_mutation_rolls_back_the_caller_variable() {
    let source = ".var {state} {before}\n.function {greet}\n    name:\n    .state {changed}\n    .sum {true} {2}\n\n.extend {greet}\n    name:\n    .super\n\n.greet {world}\n.state\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&result), "before");
}

#[test]
fn failed_extension_body_mutation_rolls_back_document_state() {
    let source = ".docname {before}\n.function {greet}\n    name:\n    .docname {changed}\n    .sum {true} {2}\n\n.extend {greet}\n    name:\n    .super\n\n.greet {world}\n.docname\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&result), "before");
}

#[test]
fn failed_extension_does_not_rollback_a_prior_successful_invocation() {
    let source = ".var {state} {before}\n.var {state} {outer}\n.extend {missing}\n    name:\n    body\n.state\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(paragraph_text(&result), "outer");
}

#[test]
fn failed_extension_body_rolls_back_function_creation_and_replacement() {
    let source = ".function {created}\n    value:\n    original .value\n\n.function {greet}\n    name:\n    original-greet .name\n\n.extend {greet}\n    name:\n    .function {new}\n        value:\n        new .value\n    .function {created}\n        value:\n        replacement .value\n    .sum {true} {2}\n\n.greet {world}\n.created {world}\n.new {world}\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(result.ir.nodes.len(), 2, "{result:?}");
    let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
        panic!(
            "expected original function output: {:?}",
            result.ir.nodes[0]
        );
    };
    let text = content
        .iter()
        .map(|inline| match inline {
            IrInline::Text { content, .. } => content.as_str(),
            other => panic!("expected text, got {other:?}"),
        })
        .collect::<String>();
    assert_eq!(text, "original world");
    assert!(matches!(
        &result.ir.nodes[1],
        IrNode::FunctionCall { name, .. } if name == "new"
    ));
}

#[test]
fn successful_nested_super_mutation_is_rolled_back_by_outer_failure() {
    let source = ".var {state} {before}\n.function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    .state {changed}\n    .super\n\n.function {outer}\n    name:\n    .greet {.name}\n    .sum {true} {2}\n\n.outer {world}\n.state\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&result), "before");
}

#[test]
fn unresolved_extension_is_preserved_and_does_not_publish_nested_state() {
    let source = ".var {state} {before}\n.function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    .var {nested} {changed}\n    .unknown\n\n.greet {world}\n.state\n.nested\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 3, "{result:?}");
    assert!(matches!(
        &result.ir.nodes[0],
        IrNode::FunctionCall { name, .. } if name == "greet"
    ));
    assert_eq!(paragraph_node_text(&result.ir.nodes[1]), "before");
    assert!(matches!(
        &result.ir.nodes[2],
        IrNode::FunctionCall { name, .. } if name == "nested"
    ));
}

#[test]
fn extension_parameter_diagnostics_keep_utf8_and_crlf_source_coordinates() {
    let source = ".var {prefix} {세계}\n.function {greet}\n    name:\n    Hello, .name!\n\n.extend {greet}\n    other:\n    .super\n"
        .replace('\n', "\r\n");
    let (result, source_id) = compile_source_with_id(&source);
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let parameter_start = source.find("other").expect("parameter span");
    assert_eq!(
        result.diagnostics[0].primary,
        Some(SourceSpan::new(
            source_id,
            parameter_start,
            parameter_start + "other".len(),
        ))
    );
    assert_eq!(result.diagnostics[0].secondary.len(), 1);
}
