from pathlib import Path

p = Path("crates/arkst-markdown/src/parser.rs")
text = p.read_text(encoding="utf-8")

old = '''                    value: convert_arg_with_mode(
                        arg,
                        source,
                        base,
                        call_base,
                        diagnostics,
                        positional_body(call_name, index),
                        positional_body(call_name, index),
                    ),'''
new = '''                    value: convert_arg_with_mode(
                        arg,
                        source,
                        base,
                        call_base,
                        diagnostics,
                        positional_body(call_name, index),
                        positional_body(call_name, index),
                        call_name == "code" && index == 4,
                    ),'''
assert text.count(old) == 1, text.count(old)
text = text.replace(old, new, 1)

old = '''        value: convert_arg_with_mode(
            &arg.value,
            source,
            base,
            call_base,
            diagnostics,
            callback_lambda,
            false,
        ),'''
new = '''        value: convert_arg_with_mode(
            &arg.value,
            source,
            base,
            call_base,
            diagnostics,
            callback_lambda,
            false,
            call_name == Some("code") && arg.name == "callouts",
        ),'''
assert text.count(old) == 1, text.count(old)
text = text.replace(old, new, 1)

old = '''fn convert_arg_with_mode(
    arg: &Arg,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    allow_unmarked_lambda: bool,
    contextual_inline_body: bool,
) -> Value {'''
new = '''fn convert_arg_with_mode(
    arg: &Arg,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    allow_unmarked_lambda: bool,
    contextual_inline_body: bool,
    target_owns_opaque_markers: bool,
) -> Value {'''
assert text.count(old) == 1, text.count(old)
text = text.replace(old, new, 1)

text = text.replace(
    'let content = parse_original_content(source, span, base, diagnostics);',
    'let content = parse_original_content(\n                        source,\n                        span,\n                        base,\n                        diagnostics,\n                        target_owns_opaque_markers,\n                    );',
    1,
)
text = text.replace(
    'let body = parse_original_content(source, lambda.body, base, diagnostics);',
    'let body = parse_original_content(\n                        source,\n                        lambda.body,\n                        base,\n                        diagnostics,\n                        target_owns_opaque_markers,\n                    );',
    1,
)
text = text.replace(
    'Ok(None) => Value::Content(parse_original_content(source, span, base, diagnostics)),',
    'Ok(None) => Value::Content(parse_original_content(\n                    source,\n                    span,\n                    base,\n                    diagnostics,\n                    target_owns_opaque_markers,\n                )),',
    1,
)
text = text.replace(
    'Value::Content(parse_original_content(source, span, base, diagnostics))',
    'Value::Content(parse_original_content(\n                        source,\n                        span,\n                        base,\n                        diagnostics,\n                        target_owns_opaque_markers,\n                    ))',
    1,
)

old = '''fn parse_original_content(
    source: &str,
    span: ByteSpan,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {'''
new = '''fn parse_original_content(
    source: &str,
    span: ByteSpan,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    target_owns_opaque_markers: bool,
) -> Vec<Inline> {'''
assert text.count(old) == 1, text.count(old)
text = text.replace(old, new, 1)

old = '''    if content_requires_e3010(source, span) {
        diagnostics.push(ParserDiagnostic {'''
new = '''    if !target_owns_opaque_markers && content_requires_e3010(source, span) {
        diagnostics.push(ParserDiagnostic {'''
assert text.count(old) == 1, text.count(old)
text = text.replace(old, new, 1)

# Parser-local tests and any direct helper calls use the normal fail-closed
# policy unless they explicitly pass through the `.code(callouts:)` slot.
needle = 'parse_original_content(source, span, base, diagnostics)'
text = text.replace(needle, 'parse_original_content(source, span, base, diagnostics, false)')

p.write_text(text, encoding="utf-8")
