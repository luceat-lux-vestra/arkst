from pathlib import Path
p = Path('tools/apply_v260_code.py')
text = p.read_text(encoding='utf-8')
old = '''            source: source.clone(),\n            span: *span,\n        }),'''
new = '''            source: source.clone(),\n            span: byte_to_source_span(span, source_id),\n        }),'''
assert text.count(old) == 1
p.write_text(text.replace(old, new, 1), encoding='utf-8')
