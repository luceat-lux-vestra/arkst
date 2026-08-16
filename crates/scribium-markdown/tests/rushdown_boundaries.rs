use rushdown::ast::{Arena, CodeBlockKind, KindData, NodeRef, TextQualifier};
use rushdown::parser::{gfm, GfmOptions, Options, Parser};
use rushdown::text::{BasicReader, Lines};
use rushdown::util::resolve_numeric_references;
use scribium_markdown::{parse_md, Block, Inline};

fn parse(source: &str, gfm_mode: bool) -> (Arena, NodeRef) {
    let parser = if gfm_mode {
        Parser::with_extensions(Options::default(), gfm(GfmOptions::default()))
    } else {
        Parser::with_options(Options::default())
    };
    parser.parse(&mut BasicReader::new(source))
}

fn nodes_with_kind(arena: &Arena, node: NodeRef, kind: &'static str, out: &mut Vec<NodeRef>) {
    if arena[node].kind_data().kind_name() == kind {
        out.push(node);
    }
    for child in arena[node].children(arena) {
        nodes_with_kind(arena, child, kind, out);
    }
}

#[test]
fn empty_fences_have_no_source_backed_value_segment() {
    for (source, info) in [
        ("```\n", None),
        ("```\n```\n", None),
        ("````;\n````\n", Some(";")),
    ] {
        let (arena, root) = parse(source, false);
        let mut code_blocks = Vec::new();
        nodes_with_kind(&arena, root, "CodeBlock", &mut code_blocks);

        assert_eq!(code_blocks.len(), 1, "source: {source:?}");
        let code = match arena[code_blocks[0]].kind_data() {
            KindData::CodeBlock(code) => code,
            other => panic!("expected CodeBlock, got {other:?}"),
        };
        assert_eq!(code.code_block_kind(), CodeBlockKind::Fenced);
        assert!(matches!(code.value(), Lines::Segments(segments) if segments.is_empty()));
        assert_eq!(code.info_str(source), info);
        assert_eq!(arena[code_blocks[0]].pos(), Some(0));
    }
}

#[test]
fn blockquote_fence_boundary_has_positions_but_no_empty_value_segments() {
    let source = "> ```\nfoo\n```\n";
    let (arena, root) = parse(source, false);
    let mut code_blocks = Vec::new();
    nodes_with_kind(&arena, root, "CodeBlock", &mut code_blocks);

    assert_eq!(code_blocks.len(), 2);
    let positions: Vec<_> = code_blocks.iter().map(|node| arena[*node].pos()).collect();
    assert_eq!(positions, vec![Some(2), Some(10)]);
    assert!(code_blocks.iter().all(|node| {
        matches!(arena[*node].kind_data(), KindData::CodeBlock(code) if matches!(code.value(), Lines::Segments(segments) if segments.is_empty()))
    }));
}

#[test]
fn pinned_numeric_reference_is_adapted_at_the_frontend_boundary() {
    assert_eq!(resolve_numeric_references(b"&#0;").as_ref(), b"\0");

    let document = parse_md("&#0;\n");
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected a paragraph");
    };
    let Inline::Text { content, .. } = &content[0] else {
        panic!("expected a text node");
    };
    assert_eq!(content, "�");
}

#[test]
fn pinned_numeric_lf_stays_text_until_canonical_projection() {
    let source = "foo&#10;&#10;bar\n";
    let (arena, root) = parse(source, false);
    let mut text_nodes = Vec::new();
    nodes_with_kind(&arena, root, "Text", &mut text_nodes);

    assert_eq!(text_nodes.len(), 1);
    let text = match arena[text_nodes[0]].kind_data() {
        KindData::Text(text) => text,
        other => panic!("expected Text, got {other:?}"),
    };
    assert_eq!(
        text.index().map(|index| (index.start(), index.stop())),
        Some((0, 16))
    );
    assert!(!text.has_qualifiers(TextQualifier::SOFT_LINE_BREAK));
}

#[test]
fn pinned_gfm_autolink_boundaries_have_no_safe_frontend_linkifier() {
    for source in ["<http://foo.bar/baz bim>\n", "<foo\\+@bar.example.com>\n"] {
        let (arena, root) = parse(source, true);
        let mut links = Vec::new();
        nodes_with_kind(&arena, root, "Link", &mut links);
        assert!(links.is_empty(), "source: {source:?}");
    }

    let source = "Visit www.commonmark.org/a.b.\n";
    let (arena, root) = parse(source, true);
    let mut links = Vec::new();
    nodes_with_kind(&arena, root, "Link", &mut links);
    assert_eq!(links.len(), 1);
    let link = match arena[links[0]].kind_data() {
        KindData::Link(link) => link,
        other => panic!("expected Link, got {other:?}"),
    };
    assert_eq!(link.destination_str(source), "http://www.commonmark.org/a");
}
