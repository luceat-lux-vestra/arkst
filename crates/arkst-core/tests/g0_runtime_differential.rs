use arkst_core::ir::{IrDocument, IrInline, IrNode};
use arkst_core::{
    compile, CompileOptions, CompileResult, Diagnostic, SourceId, VirtualProjectBuilder,
};
use arkst_engine::DocumentMetadataDefaults;
use arkst_markdown::Mode;

struct LegacyResult {
    ir: IrDocument,
    diagnostics: Vec<Diagnostic>,
}

fn build_project(source: &str) -> (arkst_core::VirtualProject, SourceId) {
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
    (project, source_id)
}

fn legacy_compile(
    project: &arkst_core::VirtualProject,
    source: &str,
    source_id: SourceId,
) -> LegacyResult {
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(
        parsed.diagnostics.is_empty(),
        "G0 differential fixtures must start from parser-clean source: {:?}",
        parsed.diagnostics
    );

    let (lowered, lowering_diagnostics) =
        arkst_engine::ast_to_ir::ast_to_ir_with_diagnostics_for_mode(
            &parsed.document,
            source_id,
            &DocumentMetadataDefaults::default(),
            Mode::Quarkdown,
        );
    assert!(
        lowering_diagnostics.is_empty(),
        "G0 differential fixture must be representable by the legacy AST-to-IR bridge: {lowering_diagnostics:?}"
    );

    let (ir, diagnostics) =
        arkst_core::evaluator::Evaluator::new().evaluate_project(project, source_id, &lowered);
    LegacyResult { ir, diagnostics }
}

fn diagnostic_signature(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{}|{:?}|{}|{:?}|{:?}|{:?}",
                diagnostic.code,
                diagnostic.severity,
                diagnostic.message,
                diagnostic.primary,
                diagnostic.secondary,
                diagnostic.hints
            )
        })
        .collect()
}

fn compile_and_assert_legacy_parity(source: &str) -> CompileResult {
    let (project, source_id) = build_project(source);
    let frontend = compile(&project, &CompileOptions::default());
    let legacy = legacy_compile(&project, source, source_id);

    assert_eq!(
        frontend.ir, legacy.ir,
        "normal direct-frontend result diverged from the legacy pre-lowered IR evaluator"
    );
    assert_eq!(
        diagnostic_signature(&frontend.diagnostics),
        diagnostic_signature(&legacy.diagnostics),
        "normal direct-frontend diagnostics diverged from the legacy pre-lowered IR evaluator"
    );
    frontend
}

fn paragraph_text(result: &CompileResult) -> String {
    result
        .ir
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
fn g0_s04_source_defined_direct_calls_match_legacy_ir_entrypoint() {
    let source = ".function {greet}\n    to from?:\n    Hello, .to from .from!\n\n.greet {world}\n.greet {world} from:{Jane}\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_text(&result),
        "Hello, world from None!\nHello, world from Jane!"
    );
}

#[test]
fn g0_s12_failed_extension_document_state_rollback_matches_legacy_ir_entrypoint() {
    let source = ".docname {before}\n.function {greet}\n    name:\n    .docname {changed}\n    .sum {true} {2}\n\n.extend {greet}\n    name:\n    .super\n\n.greet {world}\n.docname\n";
    let result = compile_and_assert_legacy_parity(source);

    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&result), "before");
}

#[test]
fn g0_s16_dynamic_and_static_string_conversion_match_legacy_ir_entrypoint() {
    let dynamic = ".var {numbertext} {.string {-3.5}}\n.abs {.numbertext}\n";
    let dynamic_result = compile_and_assert_legacy_parity(dynamic);
    assert!(dynamic_result.diagnostics.is_empty(), "{dynamic_result:?}");
    assert_eq!(paragraph_text(&dynamic_result), "3.5");

    let static_text = ".abs {.string {-3.5}}\n";
    let static_result = compile_and_assert_legacy_parity(static_text);
    assert_eq!(static_result.diagnostics.len(), 1, "{static_result:?}");
    assert_eq!(static_result.diagnostics[0].code, "E3001");
    assert!(static_result.ir.nodes.is_empty(), "{static_result:?}");
}

#[test]
fn g0_s13_nested_super_success_rolls_back_after_outer_failure() {
    let source = ".var {state} {before}\n.function {greet}\n    name:\n    base .name\n\n.extend {greet}\n    name:\n    .state {changed}\n    .super\n\n.function {outer}\n    name:\n    .greet {.name}\n    .sum {true} {2}\n\n.outer {world}\n.state\n";
    let result = compile_and_assert_legacy_parity(source);

    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&result), "before");
}

#[test]
fn g0_s14_extended_native_raw_body_stays_lazy_and_matches_legacy() {
    let source = ".var {state} {before}\n.extend {lowercase}\n    .super\n\n.lowercase\n    .state {changed}\n\n.state\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), ".state {changed}\nbefore");
}

#[test]
fn g0_s15_source_defined_nested_body_matches_legacy() {
    let source = ".function {echo}\n    value:\n    .value\n.text {.echo\n    nested}\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "nested");
}

#[test]
fn g0_s05_native_direct_fallback_matches_legacy_ir_entrypoint() {
    let success = ".sum {1} {2}\n";
    let success_result = compile_and_assert_legacy_parity(success);
    assert!(success_result.diagnostics.is_empty(), "{success_result:?}");
    assert_eq!(paragraph_text(&success_result), "3");

    let invalid_body = ".sum {1}\n    invalid\n";
    let invalid_result = compile_and_assert_legacy_parity(invalid_body);
    assert_eq!(invalid_result.diagnostics.len(), 1, "{invalid_result:?}");
    assert!(invalid_result.diagnostics[0]
        .message
        .contains("numeric arguments"));
}

#[test]
fn g0_s06_zero_argument_chain_tail_matches_legacy_ir_entrypoint() {
    let source = ".function {identity}\n    value:\n    .value\n\n.function {tail}\n    previous:\n    .previous\n\n.identity {7}::tail\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "7");
}

#[test]
fn g0_s07_flat_argument_chain_tail_matches_legacy_ir_entrypoint() {
    let source = ".function {identity}\n    value:\n    .value\n\n.function {tail}\n    previous second:\n    .second\n\n.identity {7}::tail {9}\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "9");
}

#[test]
fn g0_s08_multitail_terminal_body_source_native_source_matches_legacy() {
    // S08 / DYN02 bounded full-frontend proof: source-defined head,
    // native multiply middle tail, and source-defined terminal body owner.
    // Upstream Quarkdown reference is intentionally NOT claimed here.
    let source = ".function {identity}\n    value:\n    .value\n\n.function {tail}\n    previous content:\n    .content\n\n.identity {2}::multiply {3}::tail\n    terminal body\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "terminal body");
}

#[test]
fn g0_s08_middle_tail_failure_does_not_dispatch_stateful_terminal_tail() {
    // Successful terminal-tail dispatch is a control: this mutation must happen
    // when the native middle tail succeeds, and must not happen on failure.
    // This tests only Arkst frontend-vs-legacy parity, not official Quarkdown.
    let success = concat!(
        ".var {state} {before}\n",
        ".function {identity}\n",
        "    value:\n",
        "    .value\n",
        "\n",
        ".function {tail}\n",
        "    previous content:\n",
        "    .state {changed}\n",
        "    .content\n",
        "\n",
        ".identity {2}::multiply {3}::tail\n",
        "    terminal body\n",
        ".state\n",
    );
    let successful = compile_and_assert_legacy_parity(success);
    assert!(successful.diagnostics.is_empty(), "{successful:?}");
    assert_eq!(paragraph_text(&successful), "terminal body\nchanged");

    // The failing middle tail must stop before the state-mutating final
    // source-defined tail. Keep the same terminal body and control source.
    let failed = success.replace("::multiply {3}::tail", "::multiply {true}::tail");
    let failure = compile_and_assert_legacy_parity(&failed);
    assert_eq!(failure.diagnostics.len(), 1, "{failure:?}");
    assert!(failure.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&failure), "before");
}

#[test]
fn g0_s08_side_effectful_head_executes_once_on_success_and_middle_failure() {
    // A caller-visible numeric counter makes duplicate head evaluation
    // observable. The source-defined head succeeds and commits count=1 before
    // either the successful or failing native middle tail runs.
    //
    // Exact frontend-vs-legacy IR equality below means the final visible count
    // proves once-only execution in both entrypoints, not only in the normal
    // frontend path.
    let success = concat!(
        ".var {count} {0}\n",
        ".function {counted}\n",
        "    .var {count} {.count::sum {1}}\n",
        "    .sum {1} {1}\n",
        "\n",
        ".function {tail}\n",
        "    previous content:\n",
        "    .content\n",
        "\n",
        ".counted::multiply {3}::tail\n",
        "    terminal body\n",
        ".count\n",
    );
    let successful = compile_and_assert_legacy_parity(success);
    assert!(successful.diagnostics.is_empty(), "{successful:?}");
    assert_eq!(paragraph_text(&successful), "terminal body\n1");

    // Keep the same side-effectful head and terminal body, changing only the
    // native middle-tail input. The already-successful head invocation must
    // remain committed exactly once; the terminal owner must not run.
    let failed = success.replace("::multiply {3}::tail", "::multiply {true}::tail");
    let failure = compile_and_assert_legacy_parity(&failed);
    assert_eq!(failure.diagnostics.len(), 1, "{failure:?}");
    assert!(failure.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&failure), "1");
}

#[test]
fn g0_s09_chain_tail_shape_errors_preserve_once_only_head_effect() {
    // Both cases make head execution observable through a caller-visible
    // counter. Official Quarkdown v2.5.1 default-mode evidence shows the
    // source-defined head executes once before the terminal-tail binding
    // failure, so its committed effect remains visible as count=1.
    let prefix = concat!(
        ".var {count} {0}\n",
        ".function {counted}\n",
        "    .var {count} {.count::sum {1}}\n",
        "    .sum {1} {1}\n",
        "\n",
    );

    // The head first produces the implicit predecessor and commits count=1.
    // The terminal function's only parameter is then occupied by that
    // predecessor; supplying a terminal block body collides with the bound
    // slot and fails at the terminal tail.
    let invalid_terminal_body = format!(
        "{prefix}{}",
        concat!(
            ".function {tail}\n",
            "    previous:\n",
            "    .previous\n",
            "\n",
            ".counted::tail\n",
            "    rejected body\n",
            ".count\n",
        )
    );
    let body_failure = compile_and_assert_legacy_parity(&invalid_terminal_body);
    assert_eq!(body_failure.diagnostics.len(), 1, "{body_failure:?}");
    assert_eq!(paragraph_text(&body_failure), "1");

    // The committed head result occupies previous and the first authored
    // positional argument occupies second; one additional authored argument
    // is therefore excess. The terminal-tail failure must not duplicate or
    // roll back the already-successful head effect.
    let excess_positional_tail = format!(
        "{prefix}{}",
        concat!(
            ".function {tail}\n",
            "    previous second:\n",
            "    .second\n",
            "\n",
            ".counted::tail {9} {10}\n",
            ".count\n",
        )
    );
    let positional_failure = compile_and_assert_legacy_parity(&excess_positional_tail);
    assert_eq!(
        positional_failure.diagnostics.len(),
        1,
        "{positional_failure:?}"
    );
    assert_eq!(paragraph_text(&positional_failure), "1");
}

#[test]
fn g0_s01_mixed_arguments_evaluate_left_to_right_matches_legacy() {
    // The two nested tick calls mutate one caller-visible counter and return
    // the value after each increment. The target combines the first result as
    // the tens digit and the second as the ones digit, so source-order
    // evaluation is externally visible as 12 followed by final count 2.
    let source = concat!(
        ".var {count} {0}\n",
        ".function {tick}\n",
        "    .var {count} {.count::sum {1}}\n",
        "    .count\n",
        "\n",
        ".function {target}\n",
        "    first second:\n",
        "    .sum {.multiply {.first} {10}} {.second}\n",
        "\n",
        ".target {.tick} second:{.tick}\n",
        ".count\n",
    );
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result), "12\n2");
}

#[test]
fn g0_s01_positional_after_named_preflights_before_nested_failure_in_arkst() {
    // Official Quarkdown v2.5.1 evaluates the earlier named candidate first
    // and therefore reports the nested numeric failure. Current Arkst instead
    // structurally rejects the later positional-after-named shape before
    // evaluating either candidate. This test freezes only Arkst
    // frontend-vs-legacy migration parity and the exact current provenance.
    let source = concat!(
        ".var {count} {0}\n",
        ".function {tick}\n",
        "    .var {count} {.count::sum {1}}\n",
        "    .count\n",
        "\n",
        ".function {target}\n",
        "    first second:\n",
        "    .sum {.first} {.second}\n",
        "\n",
        ".target first:{.sum {true} {2}} {.tick}\n",
        ".count\n",
    );
    let result = compile_and_assert_legacy_parity(source);

    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "E3003");
    assert_eq!(
        diagnostic.message,
        "positional argument after named argument is not allowed"
    );
    let primary = diagnostic.primary.expect("positional argument provenance");
    assert_eq!(&source[primary.start..primary.end], "{.tick}");
    let [secondary] = diagnostic.secondary.as_slice() else {
        panic!("expected one named-argument conflict span: {diagnostic:?}");
    };
    assert!(source[secondary.start..secondary.end].starts_with("first:"));
    assert_eq!(paragraph_text(&result), "0");
}

#[test]
fn g0_s01_duplicate_named_preflights_before_nested_failure_in_arkst() {
    // As above, official v2.5.1 surfaces the nested failure from the earlier
    // candidate before it reaches the later duplicate binding. Arkst's current
    // structural plan reports the duplicate first and does not evaluate either
    // nested candidate.
    let source = concat!(
        ".var {count} {0}\n",
        ".function {tick}\n",
        "    .var {count} {.count::sum {1}}\n",
        "    .count\n",
        "\n",
        ".function {target}\n",
        "    first second?:\n",
        "    .first\n",
        "\n",
        ".target first:{.sum {true} {2}} first:{.tick}\n",
        ".count\n",
    );
    let result = compile_and_assert_legacy_parity(source);

    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "E3003");
    assert_eq!(
        diagnostic.message,
        "named argument `first` was supplied more than once"
    );
    let primary = diagnostic.primary.expect("duplicate argument provenance");
    assert_eq!(&source[primary.start..primary.end], "first");
    let [secondary] = diagnostic.secondary.as_slice() else {
        panic!("expected one prior-binding span: {diagnostic:?}");
    };
    assert!(source[secondary.start..secondary.end].starts_with("first:"));
    assert_eq!(paragraph_text(&result), "0");
}

#[test]
fn g0_s02_utf8_tight_nested_call_keeps_exact_source_spans_and_mode_isolation() {
    use arkst_markdown::ast::{Block, CallArgument, Inline, Value};

    let source = ".outer {앞{.inner {값}}뒤}\r\n";
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(parsed.diagnostics.is_empty(), "{parsed:?}");

    let [Block::DirectiveCall {
        name,
        arguments,
        span,
        ..
    }] = parsed.document.nodes.as_slice()
    else {
        panic!("expected one Quarkdown block call: {parsed:?}");
    };
    assert_eq!(name, "outer");
    assert_eq!(&source[span.start..span.end], ".outer {앞{.inner {값}}뒤}");
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));

    let [CallArgument::Positional {
        value: Value::Content(content),
        span: argument_span,
    }] = arguments.as_slice()
    else {
        panic!("expected one source-backed content argument: {arguments:?}");
    };
    assert_eq!(
        &source[argument_span.start..argument_span.end],
        "{앞{.inner {값}}뒤}"
    );

    let [Inline::Text {
        content: prefix,
        span: prefix_span,
    }, Inline::DirectiveCall {
        name: inner_name,
        name_span,
        head_span,
        arguments: inner_arguments,
        span: inner_span,
        ..
    }, Inline::Text {
        content: suffix,
        span: suffix_span,
    }] = content.as_slice()
    else {
        panic!("expected tight nested call surrounded by text: {content:?}");
    };
    assert_eq!(prefix, "앞");
    assert_eq!(&source[prefix_span.start..prefix_span.end], "앞");
    assert_eq!(inner_name, "inner");
    assert_eq!(&source[name_span.start..name_span.end], ".inner");
    assert_eq!(&source[head_span.start..head_span.end], ".inner {값}");
    assert_eq!(&source[inner_span.start..inner_span.end], "{.inner {값}}");
    let [CallArgument::Positional {
        value,
        span: inner_argument_span,
    }] = inner_arguments.as_slice()
    else {
        panic!("expected one inner argument: {inner_arguments:?}");
    };
    assert_eq!(
        &source[inner_argument_span.start..inner_argument_span.end],
        "{값}"
    );
    match value {
        Value::Identifier(value) => assert_eq!(value, "값"),
        Value::Content(content) => {
            let [Inline::Text {
                content: value,
                span,
            }] = content.as_slice()
            else {
                panic!("expected one UTF-8 inner text value: {content:?}");
            };
            assert_eq!(value, "값");
            assert_eq!(&source[span.start..span.end], "값");
        }
        other => panic!("unexpected inner value: {other:?}"),
    }
    assert_eq!(suffix, "뒤");
    assert_eq!(&source[suffix_span.start..suffix_span.end], "뒤");

    let markdown = arkst_markdown::parse_with_mode(source, Mode::Markdown);
    assert!(markdown.diagnostics.is_empty(), "{markdown:?}");
    assert!(markdown.document.nodes.iter().all(|node| match node {
        Block::DirectiveCall { .. } => false,
        Block::Paragraph { content, .. } => content
            .iter()
            .all(|inline| !matches!(inline, Inline::DirectiveCall { .. })),
        _ => true,
    }));
}

#[test]
fn g0_s02_crlf_escaped_delimiter_keeps_source_backing_and_markdown_literal() {
    use arkst_markdown::ast::{Block, CallArgument, Inline, Value};

    let source = "prefix .foo {한글 \\}\r\ntext} suffix\r\n";
    assert!(source.as_bytes().windows(2).any(|pair| pair == b"\r\n"));

    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(parsed.diagnostics.is_empty(), "{parsed:?}");
    let [Block::Paragraph { content, .. }] = parsed.document.nodes.as_slice() else {
        panic!("expected one inline paragraph: {parsed:?}");
    };
    let Inline::DirectiveCall {
        name,
        arguments,
        span,
        ..
    } = content
        .iter()
        .find(|inline| matches!(inline, Inline::DirectiveCall { name, .. } if name == "foo"))
        .expect("escaped-delimiter Quarkdown call")
    else {
        unreachable!()
    };
    assert_eq!(name, "foo");
    assert_eq!(&source[span.start..span.end], ".foo {한글 \\}\r\ntext}");
    let [CallArgument::Positional {
        value: Value::Content(argument_content),
        span: argument_span,
    }] = arguments.as_slice()
    else {
        panic!("expected source-backed escaped content: {arguments:?}");
    };
    assert_eq!(
        &source[argument_span.start..argument_span.end],
        "{한글 \\}\r\ntext}"
    );
    let rebuilt = argument_content
        .iter()
        .map(|inline| match inline {
            Inline::Text { span, .. } | Inline::SoftBreak { span } | Inline::HardBreak { span } => {
                &source[span.start..span.end]
            }
            other => panic!("unexpected escaped content node: {other:?}"),
        })
        .collect::<String>();
    assert_eq!(rebuilt, "한글 \\}\r\ntext");
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));

    let markdown = arkst_markdown::parse_with_mode(source, Mode::Markdown);
    assert!(markdown.diagnostics.is_empty(), "{markdown:?}");
    assert!(markdown.document.nodes.iter().all(|node| match node {
        Block::DirectiveCall { .. } => false,
        Block::Paragraph { content, .. } => content
            .iter()
            .all(|inline| !matches!(inline, Inline::DirectiveCall { .. })),
        _ => true,
    }));
}

#[test]
fn g0_s02_normal_compile_keeps_utf8_crlf_nested_failure_source_identity() {
    let source = "앞\r\n\r\n.sum {1} {.multiply {true} {2}}\r\n";
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
    let result = compile(&project, &CompileOptions::default());

    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    let failure_start = source.find("{true}").expect("nested boolean candidate");
    assert_eq!(
        result.diagnostics[0].primary,
        Some(arkst_source::SourceSpan::new(
            source_id,
            failure_start,
            failure_start + "{true}".len(),
        ))
    );
    let primary = result.diagnostics[0].primary.expect("primary source span");
    assert_eq!(&source[primary.start..primary.end], "{true}");
    assert!(source.is_char_boundary(primary.start));
    assert!(source.is_char_boundary(primary.end));
}

#[test]
fn g0_s03_nested_markdown_and_inline_call_preserve_order_spans_and_legacy_parity() {
    use arkst_markdown::ast::{Block, CallArgument, Inline, Value};

    fn contains_directive(inline: &Inline) -> bool {
        match inline {
            Inline::DirectiveCall { .. } => true,
            Inline::Emphasis { content, .. }
            | Inline::Strong { content, .. }
            | Inline::Link { content, .. }
            | Inline::Image { content, .. }
            | Inline::Strikethrough { content, .. } => content.iter().any(contains_directive),
            _ => false,
        }
    }

    let source = ".text {**앞 [.uppercase {world}](https://example.com) 뒤**}\r\n";
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(parsed.diagnostics.is_empty(), "{parsed:?}");

    let [Block::DirectiveCall { arguments, .. }] = parsed.document.nodes.as_slice() else {
        panic!("expected one top-level text call: {parsed:?}");
    };
    let [CallArgument::Positional {
        value: Value::Content(content),
        ..
    }] = arguments.as_slice()
    else {
        panic!("expected one rich content argument: {arguments:?}");
    };
    let [Inline::Strong {
        content: strong_content,
        span: strong_span,
    }] = content.as_slice()
    else {
        panic!("expected one strong wrapper: {content:?}");
    };
    assert_eq!(
        &source[strong_span.start..strong_span.end],
        "**앞 [.uppercase {world}](https://example.com) 뒤**"
    );

    let [Inline::Text {
        content: prefix,
        span: prefix_span,
    }, Inline::Link {
        content: link_content,
        destination,
        span: link_span,
        ..
    }, Inline::Text {
        content: suffix,
        span: suffix_span,
    }] = strong_content.as_slice()
    else {
        panic!("expected text/link/text inside strong: {strong_content:?}");
    };
    assert_eq!(prefix, "앞 ");
    assert_eq!(&source[prefix_span.start..prefix_span.end], "앞 ");
    assert_eq!(destination, "https://example.com");
    assert_eq!(
        &source[link_span.start..link_span.end],
        "[.uppercase {world}](https://example.com)"
    );
    assert_eq!(suffix, " 뒤");
    assert_eq!(&source[suffix_span.start..suffix_span.end], " 뒤");

    let [Inline::DirectiveCall {
        name,
        arguments: inner_arguments,
        span: inner_span,
        ..
    }] = link_content.as_slice()
    else {
        panic!("expected inline directive as link label: {link_content:?}");
    };
    assert_eq!(name, "uppercase");
    assert_eq!(
        &source[inner_span.start..inner_span.end],
        ".uppercase {world}"
    );
    let [CallArgument::Positional {
        span: inner_argument_span,
        ..
    }] = inner_arguments.as_slice()
    else {
        panic!("expected one uppercase argument: {inner_arguments:?}");
    };
    assert_eq!(
        &source[inner_argument_span.start..inner_argument_span.end],
        "{world}"
    );

    let markdown = arkst_markdown::parse_with_mode(source, Mode::Markdown);
    assert!(markdown.diagnostics.is_empty(), "{markdown:?}");
    assert!(markdown.document.nodes.iter().all(|node| match node {
        Block::DirectiveCall { .. } => false,
        Block::Paragraph { content, .. } => !content.iter().any(contains_directive),
        _ => true,
    }));

    let result = compile_and_assert_legacy_parity(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph {
        content: final_content,
        ..
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one final paragraph: {result:?}");
    };
    let [IrInline::Strong {
        content: final_strong,
        span: final_strong_span,
    }] = final_content.as_slice()
    else {
        panic!("expected final strong wrapper: {final_content:?}");
    };
    let [IrInline::Text {
        content: final_prefix,
        ..
    }, IrInline::Link {
        content: final_link,
        destination: final_destination,
        span: final_link_span,
        ..
    }, IrInline::Text {
        content: final_suffix,
        ..
    }] = final_strong.as_slice()
    else {
        panic!("expected final text/link/text ordering: {final_strong:?}");
    };
    assert_eq!(final_prefix, "앞 ");
    assert_eq!(final_destination, "https://example.com");
    assert!(matches!(
        final_link.as_slice(),
        [IrInline::Text { content, .. }] if content == "WORLD"
    ));
    assert_eq!(final_suffix, " 뒤");
    assert_eq!(
        &source[final_strong_span.start..final_strong_span.end],
        "**앞 [.uppercase {world}](https://example.com) 뒤**"
    );
    assert_eq!(
        &source[final_link_span.start..final_link_span.end],
        "[.uppercase {world}](https://example.com)"
    );
}

#[test]
fn g0_s03_indented_crlf_rich_body_preserves_arkst_markdown_structure() {
    let source = ".text\r\n    **앞 [.uppercase {world}](https://example.com) 뒤**\r\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one final paragraph: {result:?}");
    };
    let [IrInline::Strong {
        content: strong_content,
        span: strong_span,
    }] = content.as_slice()
    else {
        panic!("expected CRLF body to retain strong structure: {content:?}");
    };
    let [IrInline::Text {
        content: prefix, ..
    }, IrInline::Link {
        content: link_content,
        destination,
        span: link_span,
        ..
    }, IrInline::Text {
        content: suffix, ..
    }] = strong_content.as_slice()
    else {
        panic!("expected text/link/text ordering: {strong_content:?}");
    };
    assert_eq!(prefix, "앞 ");
    assert_eq!(destination, "https://example.com");
    assert!(matches!(
        link_content.as_slice(),
        [IrInline::Text { content, .. }] if content == "WORLD"
    ));
    assert_eq!(suffix, " 뒤");
    // Current Arkst keeps the intended Strong/link/inline-call semantics
    // under an indented CRLF body, but its final Strong span drops the authored
    // closing `**`. Freeze that migration behavior here without promoting it
    // to the target contract; #758 owns restoring the full source-backed span.
    assert_eq!(
        &source[strong_span.start..strong_span.end],
        "**앞 [.uppercase {world}](https://example.com) 뒤"
    );
    assert_eq!(
        &source[link_span.start..link_span.end],
        "[.uppercase {world}](https://example.com)"
    );
}
