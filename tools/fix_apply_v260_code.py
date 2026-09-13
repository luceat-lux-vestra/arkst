from pathlib import Path

p = Path("tools/apply_v260_code.py")
text = p.read_text(encoding="utf-8")

old_anchor = '''        } => Some(IrNode::CodeBlock {\n            language: language.clone(),\n            info: info.clone(),\n            source: source.clone(),\n            span: *span,\n        }),'''
new_anchor = '''        } => Some(IrNode::CodeBlock {\n            language: language.clone(),\n            info: info.clone(),\n            source: source.clone(),\n            span: byte_to_source_span(span, source_id),\n        }),'''
assert text.count(old_anchor) == 1, text.count(old_anchor)
text = text.replace(old_anchor, new_anchor, 1)

old_replacement = '''        } => Some(IrNode::CodeBlock {\n            language: language.clone(),\n            info: info.clone(),\n            source: source.clone(),\n            presentation: None,\n            span: *span,\n        }),'''
new_replacement = '''        } => Some(IrNode::CodeBlock {\n            language: language.clone(),\n            info: info.clone(),\n            source: source.clone(),\n            presentation: None,\n            span: byte_to_source_span(span, source_id),\n        }),'''
assert text.count(old_replacement) == 1, text.count(old_replacement)
text = text.replace(old_replacement, new_replacement, 1)

old_name = '.map_or("code", |parameter| parameter.name)'
new_name = '.map_or("code", |parameter| parameter.name.as_str())'
assert text.count(old_name) == 1, text.count(old_name)
text = text.replace(old_name, new_name, 1)

p.write_text(text, encoding="utf-8")
