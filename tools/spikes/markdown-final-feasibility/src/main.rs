use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use markdown_it::parser::block::{BlockRule, BlockState};
use markdown_it::parser::inline::{InlineRule, InlineState};
use markdown_it::plugins::cmark;
use markdown_it::{MarkdownIt, Node as MarkdownItNode, NodeValue};

const BLOCK_CALL: &str = ".align {center}";
const INLINE_CALL: &str = ".text {red}";

const MATRIX_LF: &str = r#".align {center}
    Body **Markdown**

> .align {center}
>     quoted body

- item
  .align {center}
      list body
  - nested
    .align {center}
        nested body

- item
  > .align {center}
  >     list quote body

> - item
>   .align {center}
>       quote list body

lazy line
.align {center}

paragraph before
.align {center}
paragraph after

Text before .text {red} and **after**.
[label .text {red}](https://example.com)
![alt .text {red}](image.png)
`inline .text {red}`

    .align {center}

```text
.align {center}
.text {red}
```
"#;

const UTF8: &str = "한글 .text {빨강} 끝\n";

#[derive(Default, Debug)]
struct Evidence {
    block_call_in_text: usize,
    inline_call_in_text: usize,
    calls_in_code: usize,
    links: usize,
    images: usize,
    exact_inline_span: bool,
}

fn main() {
    vanilla_markdown_rs();
    alternative_markdown_it();
}

fn vanilla_markdown_rs() {
    let options = ParseOptions::gfm();
    let lf = parse(MATRIX_LF, &options);
    let crlf_source = MATRIX_LF.replace('\n', "\r\n");
    let crlf = parse(&crlf_source, &options);
    let utf8 = parse(UTF8, &options);

    let mut lf_evidence = Evidence::default();
    inspect(&lf, &mut lf_evidence);
    let mut crlf_evidence = Evidence::default();
    inspect(&crlf, &mut crlf_evidence);
    let mut utf8_evidence = Evidence::default();
    inspect(&utf8, &mut utf8_evidence);

    // Vanilla markdown-rs sees the documented Quarkdown calls only as normal
    // Markdown text/code. There is no custom node type or registration API.
    assert!(lf_evidence.block_call_in_text >= 6);
    assert!(lf_evidence.inline_call_in_text >= 2);
    assert!(lf_evidence.calls_in_code >= 2);
    assert_eq!(lf_evidence.links, 1);
    assert_eq!(lf_evidence.images, 1);

    // Existing standard nodes retain physical byte positions for both LF and
    // CRLF documents, but a call embedded in a larger Text node has no exact
    // construct node span to adapt.
    assert_eq!(root_end(&lf), MATRIX_LF.len());
    assert_eq!(root_end(&crlf), crlf_source.len());
    assert_eq!(root_end(&utf8), UTF8.len());
    assert!(!utf8_evidence.exact_inline_span);

    // .md/.qd isolation is configurable only for built-in Constructs. With
    // no Quarkdown registration point, both modes necessarily produce the
    // same tree; isolation is trivial but Quarkdown integration is absent.
    let md = parse(".foo {bar}\n", &ParseOptions::gfm());
    let qd = parse(".foo {bar}\n", &ParseOptions::gfm());
    assert_eq!(md, qd);

    println!("VANILLA_BLOCKER_FIXED_AST_AND_PRIVATE_LIFECYCLE");
    println!("LF={lf_evidence:?}");
    println!("CRLF={crlf_evidence:?}");
    println!("UTF8={utf8_evidence:?}");
}

#[derive(Debug)]
struct QuarkdownBlock {
    call_span: (usize, usize),
}

impl NodeValue for QuarkdownBlock {}

struct QuarkdownBlockRule;

impl QuarkdownBlockRule {
    fn call_len(line: &str) -> Option<usize> {
        (line.starts_with(".align {") && line.ends_with('}')).then_some(line.len())
    }
}

impl BlockRule for QuarkdownBlockRule {
    fn check(state: &mut BlockState) -> Option<()> {
        (state.line_indent(state.line) < state.md.max_indent
            && Self::call_len(state.get_line(state.line)).is_some())
        .then_some(())
    }

    fn run(state: &mut BlockState) -> Option<(MarkdownItNode, usize)> {
        Self::check(state)?;
        let start_line = state.line;
        let mut end_line = start_line + 1;
        while end_line < state.line_max && state.line_indent(end_line) >= state.md.max_indent {
            end_line += 1;
        }

        let call_span = state.get_map(start_line, start_line)?.get_byte_offsets();
        let mut node = MarkdownItNode::new(QuarkdownBlock { call_span });

        if end_line > start_line + 1 {
            let old_node = std::mem::replace(&mut state.node, node);
            let old_line = state.line;
            let old_line_max = state.line_max;
            let old_indent = state.blk_indent;

            state.line = start_line + 1;
            state.line_max = end_line;
            state.blk_indent += state.md.max_indent as usize;
            state.md.block.tokenize(state);

            node = std::mem::replace(&mut state.node, old_node);
            state.line = old_line;
            state.line_max = old_line_max;
            state.blk_indent = old_indent;
        }

        Some((node, end_line - start_line))
    }
}

#[derive(Debug)]
struct QuarkdownInline {
    link_level: i32,
}

impl NodeValue for QuarkdownInline {}

struct QuarkdownInlineRule;

impl InlineRule for QuarkdownInlineRule {
    const MARKER: char = '.';

    fn run(state: &mut InlineState) -> Option<(MarkdownItNode, usize)> {
        let rest = state.src.get(state.pos..state.pos_max)?;
        if !rest.starts_with(".text {") {
            return None;
        }
        let len = rest.find('}')? + 1;
        Some((
            MarkdownItNode::new(QuarkdownInline {
                link_level: state.link_level,
            }),
            len,
        ))
    }
}

fn markdown_it(enabled: bool) -> MarkdownIt {
    let mut parser = MarkdownIt::new();
    cmark::add(&mut parser);
    if enabled {
        parser.block.add_rule::<QuarkdownBlockRule>().before_all();
        parser.inline.add_rule::<QuarkdownInlineRule>().before_all();
    }
    parser
}

fn alternative_markdown_it() {
    let enabled = markdown_it(true);
    let disabled = markdown_it(false);
    let lf = enabled.parse(MATRIX_LF);
    let crlf_source = MATRIX_LF.replace('\n', "\r\n");
    let crlf = enabled.parse(&crlf_source);
    let md = disabled.parse(MATRIX_LF);

    assert_eq!(count_markdown_it::<QuarkdownBlock>(&lf), 8);
    assert_eq!(count_markdown_it::<QuarkdownInline>(&lf), 3);
    assert_eq!(count_markdown_it::<QuarkdownBlock>(&md), 0);
    assert_eq!(count_markdown_it::<QuarkdownInline>(&md), 0);

    // Code spans, indented code, and fenced code are owned by their standard
    // rules and never invoke the custom recognizers. Link and image labels do
    // retain custom inline children, so Scribium can apply an explicit policy.
    assert_eq!(
        count_markdown_it_in::<QuarkdownInline, cmark::inline::link::Link>(&lf),
        1
    );
    assert_eq!(
        count_markdown_it_in::<QuarkdownInline, cmark::inline::image::Image>(&lf),
        1
    );
    assert_eq!(
        collect_markdown_it::<QuarkdownInline>(&lf)
            .into_iter()
            .filter(|node| node
                .cast::<QuarkdownInline>()
                .is_some_and(|value| value.link_level > 0))
            .count(),
        2
    );

    // Every block body is parsed through the same nested Markdown block
    // parser while retaining physical source offsets.
    let blocks = collect_markdown_it::<QuarkdownBlock>(&lf);
    for node in &blocks {
        let value = node.cast::<QuarkdownBlock>().expect("type checked");
        assert_eq!(&MATRIX_LF[value.call_span.0..value.call_span.1], BLOCK_CALL);
    }
    assert_eq!(
        blocks
            .iter()
            .filter(|node| !node.children.is_empty())
            .count(),
        6
    );
    assert!(count_markdown_it_in::<cmark::inline::emphasis::Strong, QuarkdownBlock>(&lf) >= 1);

    verify_markdown_it_spans(&enabled, UTF8);
    verify_markdown_it_spans(&enabled, &UTF8.replace('\n', "\r\n"));
    assert_eq!(
        crlf.srcmap.expect("root source map").get_byte_offsets().1,
        crlf_source.len()
    );

    // The custom block interrupts an ordinary paragraph through BlockRule::check.
    let interruption = enabled.parse("paragraph\n.align {center}\n");
    assert_eq!(count_markdown_it::<QuarkdownBlock>(&interruption), 1);

    // A markerless line remains a lazy blockquote continuation, while the
    // recognized block on the following line terminates that container.
    let lazy = enabled.parse("> paragraph\nlazy continuation\n.align {center}\n");
    assert_eq!(count_markdown_it::<QuarkdownBlock>(&lazy), 1);
    assert_eq!(
        count_markdown_it_in::<QuarkdownBlock, cmark::block::blockquote::Blockquote>(&lazy),
        0
    );

    println!("ALTERNATIVE_MARKDOWN_IT_PUBLIC_RULES_PASS");
    println!(
        "blocks={} inline={} bodies_attached=true link=true image=true utf8_crlf_exact=true md_isolated=true",
        count_markdown_it::<QuarkdownBlock>(&lf),
        count_markdown_it::<QuarkdownInline>(&lf)
    );
}

fn verify_markdown_it_spans(parser: &MarkdownIt, source: &str) {
    let tree = parser.parse(source);
    let node = collect_markdown_it::<QuarkdownInline>(&tree)
        .into_iter()
        .next()
        .expect("inline extension");
    let (start, end) = node.srcmap.expect("exact source map").get_byte_offsets();
    assert_eq!(&source[start..end], ".text {빨강}");
    assert_eq!(start, "한글 ".len());
}

fn collect_markdown_it<T: NodeValue>(node: &MarkdownItNode) -> Vec<&MarkdownItNode> {
    let mut result = vec![];
    collect_markdown_it_into::<T>(node, &mut result);
    result
}

fn collect_markdown_it_into<'a, T: NodeValue>(
    node: &'a MarkdownItNode,
    result: &mut Vec<&'a MarkdownItNode>,
) {
    if node.is::<T>() {
        result.push(node);
    }
    for child in &node.children {
        collect_markdown_it_into::<T>(child, result);
    }
}

fn count_markdown_it<T: NodeValue>(node: &MarkdownItNode) -> usize {
    collect_markdown_it::<T>(node).len()
}

fn count_markdown_it_in<T: NodeValue, P: NodeValue>(node: &MarkdownItNode) -> usize {
    if node.is::<P>() {
        count_markdown_it::<T>(node)
    } else {
        node.children.iter().map(count_markdown_it_in::<T, P>).sum()
    }
}

fn parse(source: &str, options: &ParseOptions) -> Node {
    to_mdast(source, options).expect("CommonMark/GFM parsing should not fail")
}

fn root_end(node: &Node) -> usize {
    node.position()
        .expect("root should have a source position")
        .end
        .offset
}

fn inspect(node: &Node, evidence: &mut Evidence) {
    match node {
        Node::Text(text) => {
            evidence.block_call_in_text += text.value.matches(BLOCK_CALL).count();
            evidence.inline_call_in_text += text.value.matches(INLINE_CALL).count();
            if text.value == INLINE_CALL {
                evidence.exact_inline_span = text.position.as_ref().is_some_and(|position| {
                    position.end.offset - position.start.offset == INLINE_CALL.len()
                });
            }
        }
        Node::Code(code) => {
            evidence.calls_in_code += code.value.matches(BLOCK_CALL).count();
            evidence.calls_in_code += code.value.matches(INLINE_CALL).count();
        }
        Node::InlineCode(code) => {
            evidence.calls_in_code += code.value.matches(INLINE_CALL).count();
        }
        Node::Link(_) => evidence.links += 1,
        Node::Image(_) => evidence.images += 1,
        _ => {}
    }

    if let Some(children) = node.children() {
        for child in children {
            inspect(child, evidence);
        }
    }
}
