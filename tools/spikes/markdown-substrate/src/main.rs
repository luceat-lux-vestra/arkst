use comrak::{parse_document, Arena};
use markdown::mdast::Node as MdastNode;
use markdown::{to_mdast, Constructs, Options as MarkdownOptions, ParseOptions};
use markdown_it::parser::block::{BlockRule, BlockState};
use markdown_it::parser::inline::{InlineRule, InlineState};
use markdown_it::plugins::{cmark, html};
use pulldown_cmark::{Event, Options as PulldownOptions, Parser, Tag};
use std::cell::Cell;
use std::fmt::Debug;

const UTF8: &str = "한글 **bold** text";
const FIXTURE: &str = "# Heading\r\n\r\nNormal **Markdown** and [link](https://example.com).\r\n.foo {bar}\r\nText before .foo {bar} and **after**.\r\n- item\r\n  .foo {bar}\r\n    body\r\n> .foo {bar}\r\n> continued **Markdown**\r\n`inline .foo {not-a-call}`\r\n```text\r\n.foo {not-a-call}\r\n```\r\n";
const EDGE_FIXTURE: &str = "- outer\r\n  - inner\r\n    body\r\n> outer quote\r\n> > nested quote\r\n> > - list in quote\r\n> >   .foo {bar}\r\n- item with quote\r\n  > quote inside list\r\n  > lazy continuation\r\n*before* .foo {bar} **after**\r\n[.foo {not-a-call}](https://example.com)\r\n<span>.foo {not-a-call}</span>\r\n.outer {.inner {value}}\r\n.align {center}\r\n  body **Markdown**\r\n";
const FRONT_MATTER: &str = "---\r\ntitle: Demo\r\n---\r\n\r\n# Heading\r\n";
const EXTENSION_FIXTURE: &str = "\
.foo {bar}\r\n\
- item\r\n\
  .foo {bar}\r\n\
> .foo {bar}\r\n\
*before* .foo {bar} **after**\r\n\
[.foo {bar}](https://example.com)\r\n\
![.foo {bar}](image.png)\r\n\
\\.foo {bar}\r\n\
&period;foo {bar}\r\n\
<span>.foo {bar}</span>\r\n\
`inline .foo {bar}`\r\n\
```text\r\n\
.foo {bar}\r\n\
```\r\n";

fn main() {
    markdown_rs();
    comrak();
    pulldown();
    markdown_it();
}

fn markdown_rs() {
    println!("=== markdown-rs 1.0.0 / markdown crate ===");
    let tree = to_mdast(UTF8, &MarkdownOptions::gfm().parse).expect("CommonMark does not error");
    println!("UTF8 AST: {tree:?}");
    let fixture = to_mdast(FIXTURE, &MarkdownOptions::gfm().parse).expect("GFM does not error");
    println!("fixture AST: {fixture:?}");
    let edge =
        to_mdast(EDGE_FIXTURE, &MarkdownOptions::gfm().parse).expect("edge fixture does not error");
    assert_debug_contains(&edge, "Blockquote");
    assert_debug_contains(&edge, "Link");
    assert_debug_contains(&edge, "Html");
    let frontmatter = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    };
    let frontmatter = to_mdast(FRONT_MATTER, &frontmatter).expect("front matter does not error");
    assert_debug_contains(&frontmatter, "Yaml");
    assert_debug_contains(&tree, "Strong");
    assert_debug_contains(&tree, "(7-15)");
    assert_debug_contains(&fixture, "InlineCode");
    assert_debug_contains(&fixture, "Code");
}

fn comrak() {
    println!("=== comrak 0.54.0 ===");
    let arena = Arena::new();
    let root = parse_document(&arena, UTF8, &comrak::Options::default());
    for node in root.descendants() {
        let data = node.data.borrow();
        println!("comrak UTF8 node: {:?} @ {}", data.value, data.sourcepos);
    }
    let arena = Arena::new();
    let root = parse_document(&arena, FIXTURE, &comrak::Options::default());
    for node in root.descendants() {
        let data = node.data.borrow();
        println!("comrak fixture node: {:?} @ {}", data.value, data.sourcepos);
    }
    let arena = Arena::new();
    let root = parse_document(&arena, EDGE_FIXTURE, &comrak::Options::default());
    let edge = format!("{root:?}");
    assert!(edge.contains("BlockQuote"));
    assert!(edge.contains("Link"));
    assert!(edge.contains("HtmlInline"));
    let mut options = comrak::Options::default();
    options.extension.front_matter_delimiter = Some("---".to_owned());
    let arena = Arena::new();
    let root = parse_document(&arena, FRONT_MATTER, &options);
    assert!(format!("{root:?}").contains("FrontMatter"));
}

fn pulldown() {
    println!("=== pulldown-cmark 0.13.4 ===");
    let mut options = PulldownOptions::empty();
    options.insert(PulldownOptions::ENABLE_GFM);
    let events: Vec<(Event<'_>, std::ops::Range<usize>)> =
        Parser::new_ext(UTF8, options).into_offset_iter().collect();
    for (event, range) in &events {
        println!("pulldown UTF8 event: {event:?} @ {range:?}");
    }
    assert!(events.iter().any(|(event, range)| {
        matches!(event, Event::Start(Tag::Strong)) && range.start == 7 && range.end == 15
    }));

    let events: Vec<(Event<'_>, std::ops::Range<usize>)> = Parser::new_ext(FIXTURE, options)
        .into_offset_iter()
        .collect();
    for (event, range) in events {
        println!("pulldown fixture event: {event:?} @ {range:?}");
    }
    let edge_events: Vec<(Event<'_>, std::ops::Range<usize>)> =
        Parser::new_ext(EDGE_FIXTURE, options)
            .into_offset_iter()
            .collect();
    assert!(edge_events
        .iter()
        .any(|(event, _)| matches!(event, Event::Start(Tag::BlockQuote(_)))));
    assert!(edge_events
        .iter()
        .any(|(event, _)| matches!(event, Event::InlineHtml(_))));
    let mut frontmatter_options = PulldownOptions::empty();
    frontmatter_options.insert(PulldownOptions::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    let frontmatter_events: Vec<_> = Parser::new_ext(FRONT_MATTER, frontmatter_options)
        .into_offset_iter()
        .collect();
    assert!(frontmatter_events
        .iter()
        .any(|(event, _)| matches!(event, Event::Start(Tag::MetadataBlock(_)))));
}

fn markdown_it() {
    println!("=== markdown-it-rust 0.6.1 / markdown-it crate ===");
    let md = &mut markdown_it::MarkdownIt::new();
    cmark::add(md);
    let tree = md.parse(UTF8);
    tree.walk(|node, depth| {
        println!("markdown-it UTF8 node: {node:?} depth={depth}");
    });
    let tree = md.parse(FIXTURE);
    tree.walk(|node, depth| {
        println!("markdown-it fixture node: {node:?} depth={depth}");
    });

    let md = &mut markdown_it::MarkdownIt::new();
    cmark::add(md);
    html::add(md);
    md.block.add_rule::<QuarkdownBlockRule>().before_all();
    md.inline.add_rule::<QuarkdownInlineRule>().before_all();
    let tree = md.parse(".foo {bar}\nText .foo {bar} and `inline .foo {not-a-call}`\n");
    tree.walk(|node, depth| {
        println!("markdown-it custom node: {node:?} depth={depth}");
    });
    let debug = format!("{tree:?}");
    assert!(debug.contains("QuarkdownBlock"));
    assert_eq!(debug.matches("QuarkdownInline").count(), 2);
    let edge = md.parse(EDGE_FIXTURE);
    let edge = format!("{edge:?}");
    assert!(edge.contains("Blockquote"));
    assert!(edge.contains("Link"));
    verify_markdown_it_extension_context(md);
    let frontmatter = md.parse(FRONT_MATTER);
    assert!(!format!("{frontmatter:?}").contains("FrontMatter"));
}

fn verify_markdown_it_extension_context(md: &markdown_it::MarkdownIt) {
    let tree = md.parse(EXTENSION_FIXTURE);
    let block_count = Cell::new(0);
    let inline_count = Cell::new(0);
    let inline_in_link_count = Cell::new(0);
    let exact_spans = Cell::new(0);

    tree.walk(|node, _| {
        if node.is::<QuarkdownBlock>() {
            block_count.set(block_count.get() + 1);
        }
        if let Some(value) = node.cast::<QuarkdownInline>() {
            inline_count.set(inline_count.get() + 1);
            if value.link_level > 0 {
                inline_in_link_count.set(inline_in_link_count.get() + 1);
            }
        }
        if node.srcmap.is_some_and(|map| {
            &EXTENSION_FIXTURE[map.get_byte_offsets().0..map.get_byte_offsets().1] == ".foo {bar}"
        }) {
            exact_spans.set(exact_spans.get() + 1);
        }
    });

    // The block rule participates after list/blockquote rules adjust their
    // nested BlockState, so all three standalone calls become custom blocks.
    assert_eq!(block_count.get(), 3);
    // Ordinary text, link text, image text, and HTML text currently invoke
    // this deliberately naive rule. Escaped/entity spelling and code
    // spans/fences do not. Production policy must use the exposed state and
    // precedence controls to narrow those contexts.
    assert_eq!(inline_count.get(), 4);
    assert_eq!(inline_in_link_count.get(), 2);
    assert_eq!(exact_spans.get(), 7);
}

#[derive(Debug)]
struct QuarkdownBlock;

impl markdown_it::NodeValue for QuarkdownBlock {}

struct QuarkdownBlockRule;

impl BlockRule for QuarkdownBlockRule {
    fn run(state: &mut BlockState) -> Option<(markdown_it::Node, usize)> {
        if state.get_line(state.line) == ".foo {bar}" {
            Some((markdown_it::Node::new(QuarkdownBlock), 1))
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct QuarkdownInline {
    link_level: i32,
}

impl markdown_it::NodeValue for QuarkdownInline {}

struct QuarkdownInlineRule;

impl InlineRule for QuarkdownInlineRule {
    const MARKER: char = '.';

    fn run(state: &mut InlineState) -> Option<(markdown_it::Node, usize)> {
        let input = &state.src[state.pos..state.pos_max];
        let call = ".foo {bar}";
        input.starts_with(call).then(|| {
            (
                markdown_it::Node::new(QuarkdownInline {
                    link_level: state.link_level,
                }),
                call.len(),
            )
        })
    }
}

fn assert_debug_contains<T: Debug>(value: &T, expected: &str) {
    let debug = format!("{value:?}");
    assert!(
        debug.contains(expected),
        "{expected:?} not found in {debug}"
    );
}

#[allow(dead_code)]
fn _keep_mdast_type_name(_: Option<MdastNode>) {}
