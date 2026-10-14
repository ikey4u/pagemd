use pagemd_parser::{mdast::Node, ParseOptions};

fn visit(node: &Node, f: &mut impl FnMut(&Node)) {
    f(node);
    if let Some(children) = node.children() {
        for child in children {
            visit(child, f);
        }
    }
}

#[test]
fn cjk_ast_offsets_and_points_refer_to_the_original_crlf_source() {
    let source = "# 前**「标题」**后\r\n\r\n前*（重点）*后\r\n\n最后";
    let tree = pagemd_parser::to_mdast(source, &ParseOptions::gfm()).unwrap();
    let location = pagemd_parser::Location::new(source.as_bytes());
    let mut emphasis = Vec::new();
    visit(&tree, &mut |node| {
        let position = node.position().unwrap();
        assert_eq!(
            location.to_point(position.start.offset).unwrap(),
            position.start
        );
        assert_eq!(
            location.to_point(position.end.offset).unwrap(),
            position.end
        );
        assert!(source
            .get(position.start.offset..position.end.offset)
            .is_some());
        if matches!(node, Node::Strong(_) | Node::Emphasis(_)) {
            emphasis.push(&source[position.start.offset..position.end.offset]);
        }
    });
    assert_eq!(emphasis, ["**「标题」**", "*（重点）*"]);
    assert_eq!(tree.to_string(), "前「标题」后前（重点）后最后");
}

#[test]
fn user_zero_width_characters_and_entities_are_preserved() {
    for source in [
        "前**「重\u{200b}点」**后",
        "前**「重&#8203;点」**后",
        "前**「重&#xE000;点」**后",
    ] {
        let tree =
            pagemd_parser::to_mdast(source, &ParseOptions::gfm()).unwrap();
        let mut strong = false;
        visit(&tree, &mut |node| {
            strong |= matches!(node, Node::Strong(_));
        });
        assert!(strong, "{source}");
        let expected = if source.contains("E000") {
            '\u{e000}'
        } else {
            '\u{200b}'
        };
        assert!(tree.to_string().contains(expected));
        assert!(pagemd_parser::to_html(source).contains(expected));
    }
}

#[test]
fn normalization_skips_code_escapes_html_and_link_destinations() {
    for source in [
        "`前**「代码」**后`",
        "```md\n前**「代码」**后\n```",
        "    前**「代码」**后",
        "前\\*\\*「原文」\\*\\*后",
        "[链接](https://example.com/前**「路径」**后)",
        "<div title=\"前**「属性」**后\">content</div>",
    ] {
        assert_eq!(pagemd_parser::fix_emphasis_cjk_punctuation(source), source);
    }
}

#[test]
fn math_configuration_is_preserved_and_math_contents_are_literal() {
    let source =
        "前**「重点」**后 $前**「公式」**后$\n\n$$\n前**「公式」**后\n$$";
    let mut options = ParseOptions::gfm();
    options.constructs.math_text = true;
    options.constructs.math_flow = true;
    let tree = pagemd_parser::to_mdast(source, &options).unwrap();
    let mut values = Vec::new();
    visit(&tree, &mut |node| match node {
        Node::InlineMath(math) => values.push(math.value.clone()),
        Node::Math(math) => values.push(math.value.clone()),
        _ => {}
    });
    assert_eq!(values, ["前**「公式」**后", "前**「公式」**后"]);
}

#[test]
fn gfm_reference_links_and_tables_keep_their_source_positions() {
    let source = "| 内容 |\n| --- |\n| 前**「[链接][target]」**后 |\n\n[target]: https://example.com \"标题\"\n";
    let tree = pagemd_parser::to_mdast(source, &ParseOptions::gfm()).unwrap();
    let mut found = Vec::new();
    visit(&tree, &mut |node| {
        if matches!(
            node,
            Node::Strong(_) | Node::LinkReference(_) | Node::Definition(_)
        ) {
            let position = node.position().unwrap();
            found.push(&source[position.start.offset..position.end.offset]);
        }
    });
    assert_eq!(
        found,
        [
            "**「[链接][target]」**",
            "[链接][target]",
            "[target]: https://example.com \"标题\""
        ]
    );
    let html = pagemd_parser::to_html_with_options(
        source,
        &pagemd_parser::Options::gfm(),
    )
    .unwrap();
    assert!(html.contains("<strong>「<a href=\"https://example.com\" title=\"标题\">链接</a>」</strong>"));
}

#[test]
fn incomplete_streams_never_expose_artificial_markers() {
    let source = "前**「A &amp; B」**后";
    for end in (0..=source.len()).filter(|end| source.is_char_boundary(*end)) {
        let chunk = &source[..end];
        let tree =
            pagemd_parser::to_mdast(chunk, &ParseOptions::gfm()).unwrap();
        assert!(!tree.to_string().contains('\u{200b}'), "{chunk}");
        assert!(
            !pagemd_parser::to_html(chunk).contains('\u{200b}'),
            "{chunk}"
        );
    }
}

#[test]
fn adjacent_cjk_phrases_do_not_pair_the_middle_delimiters() {
    let source = "这是**“重要内容”**和**（另一个重点）**。";
    assert_eq!(pagemd_parser::to_html(source), "<p>这是<strong>“重要内容”</strong>和<strong>（另一个重点）</strong>。</p>");
    let fixed = pagemd_parser::fix_emphasis_cjk_punctuation(source);
    assert!(fixed.contains("”\u{200b}**和**\u{200b}（"));
}

#[test]
fn html_does_not_remove_user_character_references_from_image_alt_or_titles() {
    let source = "前**「重点」**后 ![&#8203;](image.png \"&#xE000;\")";
    let html = pagemd_parser::to_html(source);
    assert!(html.contains("alt=\"\u{200b}\""), "{html}");
    assert!(html.contains("title=\"\u{e000}\""), "{html}");
    assert!(html.contains("<strong>「重点」</strong>"));
}

#[test]
fn unsupported_mdx_is_reported_instead_of_using_another_parser() {
    let error = pagemd_parser::to_mdast(
        "<Component value={item} />",
        &ParseOptions::mdx(),
    )
    .unwrap_err();
    assert!(error.reason.contains("MDX"));
}

#[test]
fn padded_inline_math_has_original_positions_and_no_temporary_markers() {
    let source = "# Energy $ E=mc^2 $\n\n| Formula |\n| --- |\n| $ x + y $ |";
    let mut options = ParseOptions::gfm();
    options.constructs.math_text = true;
    let tree = pagemd_parser::to_mdast(source, &options).unwrap();
    let mut values = Vec::new();
    visit(&tree, &mut |node| {
        if let Node::InlineMath(math) = node {
            let position = math.position.as_ref().unwrap();
            assert!(source[position.start.offset..position.end.offset]
                .starts_with("$ "));
            assert!(!math.value.contains('\u{200b}'));
            values.push(math.value.clone());
        }
    });
    assert_eq!(values, ["E=mc^2", "x + y"]);
}
