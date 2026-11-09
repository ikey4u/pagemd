use pagemd_parser::{
    extensions::preprocess_markdown_extensions, frontmatter::detect,
    mdast::Node, parse, to_html_with_options, Options, ParseOptions,
};

#[test]
fn common_metadata_formats_share_lossless_code_rendering() {
    for (source, language, value) in [
        ("---\nname: reviewdoc\ndescription: 中文 **literal** $5\nmetadata:\n  enabled: true\n  count: 2\n  optional: null\ntags: [review, docs]\n---\n# Body", "yaml", "name: reviewdoc"),
        ("\u{feff}---\r\ndescription: |\r\n  第一行\r\n  第二行\r\n...\r\n# Body", "yaml", "第一行\n  第二行"),
        ("+++\nname = 'reviewdoc'\ntags = ['review', 'docs']\n[metadata]\nenabled = true\n+++\n# Body", "toml", "enabled = true"),
        ("{\"name\":\"reviewdoc\",\"nested\":{\"braces\":\"} { \\\"\"},\"tags\":[1,true,null]}\n# Body", "json", "reviewdoc"),
        ("---\n---\n# Body", "yaml", ""),
    ] {
        let metadata = detect(source).unwrap();
        assert_eq!(metadata.language, language);
        let document = parse(source, &ParseOptions::gfm()).unwrap();
        assert_eq!(document.source(), source);
        let children = document.ast().children().unwrap();
        let Node::Code(code) = &children[0] else { panic!("{:?}", children[0]); };
        assert_eq!(code.lang.as_deref(), Some(language));
        assert_eq!(code.meta.as_deref(), Some("frontmatter"));
        assert!(code.value.contains(value), "{}", code.value);
        assert_eq!(code.position.as_ref().unwrap().start.offset, 0);
        let Node::Heading(heading) = &children[1] else { panic!("{children:?}"); };
        assert_eq!(heading.position.as_ref().unwrap().start.offset, source.find("# Body").unwrap());
        let html = to_html_with_options(source, &Options::gfm()).unwrap();
        assert!(html.contains("pagemd-metadata"), "{html}");
        assert!(html.contains("<h1>Body</h1>"));
        assert!(!html.contains("<strong>literal</strong>"));
    }
}

#[test]
fn incomplete_invalid_or_noninitial_metadata_remains_markdown() {
    for source in [
        "---\nname: reviewdoc",
        "+++\nname = 'reviewdoc'",
        "{\"name\":",
        "---\nname: [unterminated\n---\n# Body",
        "+++\nname = [\n+++\n# Body",
        "---\n\n# Title\n\n---\nAfter",
        "---\nplain text\n---\nAfter",
        "\n---\nname: value\n---",
        "# Body\n---\nname: value\n---",
        "{\"name\":\"value\"} same line prose",
        "[1, 2]",
        "{not json}",
    ] {
        assert!(detect(source).is_none(), "{source}");
        let document = parse(source, &ParseOptions::gfm()).unwrap();
        assert_eq!(document.source(), source);
        assert!(!document.ast().children().unwrap().iter().any(|n| matches!(n, Node::Code(c) if c.meta.as_deref() == Some("frontmatter"))), "{source}");
        assert!(!to_html_with_options(source, &Options::gfm())
            .unwrap()
            .is_empty());
    }
    let mut options = ParseOptions::gfm();
    options.constructs.frontmatter = false;
    assert!(!matches!(
        parse("---\nname: value\n---", &options)
            .unwrap()
            .ast()
            .children()
            .unwrap()[0],
        Node::Code(_)
    ));
}

#[test]
fn metadata_and_yaml_fences_protect_literal_extension_syntax() {
    let metadata = "---\nname: reviewdoc\ndescription: |\n  $$literal$$\n  !!! note\n  ::: details\n  > [!NOTE]\n  [hidden]: /metadata-only\n---\n";
    let source = format!(
        "{metadata}# Body\n\n[hidden]\n\n[visible]: /body\n\n[visible]\n"
    );
    let pre = preprocess_markdown_extensions(&source);
    assert!(pre.starts_with(metadata));
    let mut options = ParseOptions::gfm();
    options.constructs.math_flow = true;
    options.constructs.math_text = true;
    let document = parse(&pre, &options).unwrap();
    let Node::Code(code) = &document.ast().children().unwrap()[0] else {
        panic!()
    };
    assert!(code.value.contains("$$literal$$"));
    let html = to_html_with_options(&pre, &Options::gfm()).unwrap();
    assert!(!html.contains("href=\"/metadata-only\""));
    assert!(html.contains("href=\"/body\""));
    let fenced = "```yaml\nname: reviewdoc\ndescription: |\n  $$literal$$\n  !!! note\n  ::: details\n```\n";
    assert_eq!(preprocess_markdown_extensions(fenced), fenced);
}

#[test]
fn every_streaming_prefix_can_be_replaced_by_a_complete_document() {
    let source = "---\nname: reviewdoc\ndescription: 中文\n---\n\n# Body";
    for end in source
        .char_indices()
        .map(|(at, _)| at)
        .chain([source.len()])
    {
        let document = parse(&source[..end], &ParseOptions::gfm()).unwrap();
        assert_eq!(document.source(), &source[..end]);
    }
    let complete = parse(source, &ParseOptions::gfm()).unwrap();
    assert!(matches!(
        complete.ast().children().unwrap()[0],
        Node::Code(_)
    ));
}

#[test]
fn field_layout_preserves_empty_containers_and_escapes_values() {
    use pagemd_parser::frontmatter::render_html;
    let html = render_html("yaml", "object: {}\nlist: []\ntext: '<script>alert(1)</script>'").unwrap();
    assert!(html.contains("<span>{}</span>"));
    assert!(html.contains("<span>[]</span>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
    assert!(!render_html("yaml", "").unwrap().contains("null"));
}
