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
fn cjk_latin_boundaries_preserve_source_events_and_utf8_positions() {
    let source = "前**中文。**EML\r\n\r\nASCII**「中文」**after";
    let parsed = pagemd_parser::parse(source, &ParseOptions::gfm()).unwrap();
    assert_eq!(parsed.source(), source);
    let location = pagemd_parser::Location::new(source.as_bytes());
    let mut strong = Vec::new();
    visit(parsed.ast(), &mut |node| {
        let position = node.position().unwrap();
        assert_eq!(
            location.to_point(position.start.offset).unwrap(),
            position.start
        );
        assert_eq!(
            location.to_point(position.end.offset).unwrap(),
            position.end
        );
        if matches!(node, Node::Strong(_)) {
            strong.push(&source[position.start.offset..position.end.offset]);
        }
    });
    assert_eq!(strong, ["**中文。**", "**「中文」**"]);
    for event in parsed.source_events() {
        assert!(source.get(event.range.clone()).is_some());
        if let pagemd_parser::events::Event::Text(text) = &event.event {
            assert!(!text.contains('\u{200b}'));
        }
    }
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

#[test]
fn currency_prices_in_tables_stay_prose_and_keep_original_positions() {
    let source = "| 服务 / 地域 / 档位 | 官方原始单价 | 1C2G × 1h | 2C4G × 1h | 状态、费用边界与规格条件 | 价格原始链接 |\n| --- | --- | --- | --- | --- | --- |\n| E2B Cloud | CPU $0.000014/vCPU·s；RAM $0.0000045/GiB·s | $0.082800 ≈ **¥0.5521** | $0.165600 ≈ **¥1.1042** | 可算；1C2G 的自定义资源权限见 Pro；Pro $150/月另收。Hobby 最长单次 1h，默认 2C4G；价格表与套餐权限须一起看 | [官方价格及套餐](https://e2b.dev/pricing) |";
    let mut options = ParseOptions::gfm();
    options.constructs.math_text = true;
    options.constructs.math_flow = true;
    let document = pagemd_parser::parse(source, &options).unwrap();
    let location = pagemd_parser::Location::new(source.as_bytes());
    let mut strong = Vec::new();
    visit(document.ast(), &mut |node| {
        assert!(!matches!(node, Node::InlineMath(_)), "{node:?}");
        if let Some(position) = node.position() {
            assert_eq!(
                location.to_point(position.start.offset).unwrap(),
                position.start
            );
            assert_eq!(
                location.to_point(position.end.offset).unwrap(),
                position.end
            );
            if matches!(node, Node::Strong(_)) {
                strong
                    .push(&source[position.start.offset..position.end.offset]);
            }
        }
    });
    assert_eq!(strong, ["**¥0.5521**", "**¥1.1042**"]);
    assert!(document
        .ast()
        .to_string()
        .contains("CPU $0.000014/vCPU·s；RAM $0.0000045/GiB·s"));
    assert!(!document.source_events().iter().any(|item| matches!(
        item.event,
        pagemd_parser::events::Event::InlineMath(_)
    )));
}

#[test]
fn currencies_coexist_with_numeric_padded_and_explicit_math() {
    let mut options = ParseOptions::gfm();
    options.constructs.math_text = true;
    options.constructs.math_flow = true;
    for source in [
        "$5 and $10; math $x^2$ and $ 2 + y $.",
        "Prices $5 and $10. Numeric math $2$ or $2 + x$.",
        "`$5 and $10` and [price](https://example.com/$5/$10), math $x$.",
        "前**「价格」**后 $5 and $10; math $x^2$.",
    ] {
        let document = pagemd_parser::parse(source, &options).unwrap();
        let mut math = Vec::new();
        visit(document.ast(), &mut |node| {
            if let Node::InlineMath(value) = node {
                math.push(value.value.clone());
            }
        });
        assert!(
            math.iter().all(|value| !value.contains("and")),
            "{source}: {math:?}"
        );
        assert!(!math.is_empty(), "real math must survive: {source}");
        assert_eq!(document.source(), source);
    }
}
