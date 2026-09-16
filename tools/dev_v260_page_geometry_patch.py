from pathlib import Path
import subprocess

parser = Path("crates/arkst-markdown/src/parser.rs")
text = parser.read_text()
old = '''    if let Block::Paragraph { span, .. } = block {
        if source
            .get(span.start..span.end)
            .is_some_and(|text| text.trim() == "<<<")
        {
            return vec![Block::PageBreak { span: *span }];
        }
    }
'''
new = '''    if let Block::Paragraph { span, .. } = block {
        if let Some(spans) = explicit_page_break_spans(source, *span) {
            return spans
                .into_iter()
                .map(|span| Block::PageBreak { span })
                .collect();
        }
    }
'''
helper = '''fn explicit_page_break_spans(source: &str, span: ByteSpan) -> Option<Vec<ByteSpan>> {
    let text = source.get(span.start..span.end)?;
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for raw_line in text.split_inclusive('\\n') {
        let line = raw_line
            .strip_suffix('\\n')
            .unwrap_or(raw_line)
            .strip_suffix('\\r')
            .unwrap_or_else(|| raw_line.strip_suffix('\\n').unwrap_or(raw_line));
        if line.trim() != "<<<" {
            return None;
        }
        spans.push(ByteSpan::new(span.start + offset, span.start + offset + line.len()));
        offset += raw_line.len();
    }
    (!spans.is_empty()).then_some(spans)
}

'''
marker = "fn explicit_page_break_spans(source: &str, span: ByteSpan)"
if marker not in text:
    if old not in text:
        raise SystemExit("explicit pagebreak normalization anchor not found")
    text = text.replace(old, new, 1)
    anchor = "fn normalize_block(\n"
    if anchor not in text:
        raise SystemExit("normalize_block anchor not found")
    text = text.replace(anchor, helper + anchor, 1)
    parser.write_text(text)

commands = [
    ["cargo", "test", "-p", "arkst-markdown", "--test", "quarkdown_explicit_pagebreak", "--locked"],
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_explicit_pagebreak", "--locked"],
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_v260_slides", "--locked"],
    ["cargo", "test", "-p", "arkst-typst", "--test", "slides_document_prelude", "--locked"],
    ["cargo", "test", "-p", "arkst-typst-inprocess", "--test", "slides_pdf_contract", "--locked"],
]

for command in commands:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True)
