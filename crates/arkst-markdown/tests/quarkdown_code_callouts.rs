use arkst_markdown::ast::{Block, CallArgument, Inline, Value};
use arkst_markdown::{parse_with_mode, Mode};

#[test]
fn code_callouts_own_marker_bearing_map_text_without_widening_e3010() {
    let source =
        ".code callouts:{\n    - 2: Second *item*\n    - 1: First\n}\n    alpha\n    beta\n";
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected .code diagnostics: {:?}",
        output.diagnostics
    );

    let Block::DirectiveCall {
        name, arguments, ..
    } = &output.document.nodes[0]
    else {
        panic!("expected .code block call, got {:?}", output.document.nodes);
    };
    assert_eq!(name, "code");
    let Some(CallArgument::Named {
        name,
        value: Value::Content(content),
        ..
    }) = arguments.first()
    else {
        panic!("expected named callouts content, got {arguments:?}");
    };
    assert_eq!(name, "callouts");
    assert!(matches!(
        content.as_slice(),
        [Inline::Text { content, .. }]
            if content == "\n    - 2: Second *item*\n    - 1: First\n"
    ));

    let ordinary = parse_with_mode(".foo value:{Second *item*}\n", Mode::Quarkdown);
    assert!(
        ordinary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3010"),
        "the .code(callouts:) exception must not widen globally: {:?}",
        ordinary.diagnostics
    );
}
