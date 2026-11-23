use pagemd_core::{
    parser, FootnoteDisplay, FootnoteRegistry, HtmlRenderContext, HtmlRenderer,
    Renderer,
};
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

#[test]
fn complete_html_renderer_consumes_shared_parser_results() {
    let source = "# PageMD\n\n前**「重点」**后。\n\n```rust\nlet n = 1;\n```\n";
    let document = parser::parse(source, &parser::ParseOptions::gfm()).unwrap();
    let registry = FootnoteRegistry::from_markdown(document.source());
    let syntax = SyntaxSet::load_defaults_newlines();
    let themes = ThemeSet::load_defaults();
    let mut extracted = vec![];
    let section = HtmlRenderer
        .render(
            &document,
            HtmlRenderContext {
                base_dir: std::path::Path::new("."),
                math_font_size: 16.,
                font_dir: "",
                ss: &syntax,
                ts: &themes,
                footnotes: &registry,
                depth: 0,
                client_mermaid: false,
                footnote_display: FootnoteDisplay::default(),
                smart_punctuation: false,
                extracted_footnotes: &mut extracted,
            },
        )
        .unwrap();
    assert_eq!(section.title, "PageMD");
    assert!(section.html.contains("<strong>「重点」</strong>"));
    assert!(section.html.contains("language-rust"));
    assert_eq!(document.source(), source);
}

#[cfg(feature = "gpui")]
#[test]
fn built_in_backends_share_the_renderer_contract() {
    fn implements_renderer<R: Renderer>() {}
    implements_renderer::<HtmlRenderer>();
    implements_renderer::<pagemd_core::gpui::GpuiRenderer>();
}

#[test]
fn complete_html_renderer_displays_frontmatter_without_consuming_body() {
    let syntax = SyntaxSet::load_defaults_newlines();
    let themes = ThemeSet::load_defaults();
    for (metadata, language) in [
        ("---\nname: reviewdoc\ndescription: 中文\n---\n", "yaml"),
        ("+++\nname = 'reviewdoc'\n+++\n", "toml"),
        ("{\"name\":\"reviewdoc\",\"enabled\":true}\n", "json"),
    ] {
        let source = format!("{metadata}# Body\n\nVisible.");
        let source =
            parser::extensions::preprocess_markdown_extensions(&source);
        let document =
            parser::parse(&source, &parser::ParseOptions::gfm()).unwrap();
        let registry = FootnoteRegistry::from_markdown(document.source());
        let mut extracted = vec![];
        let section = HtmlRenderer
            .render(
                &document,
                HtmlRenderContext {
                    base_dir: std::path::Path::new("."),
                    math_font_size: 16.,
                    font_dir: "",
                    ss: &syntax,
                    ts: &themes,
                    footnotes: &registry,
                    depth: 0,
                    client_mermaid: false,
                    footnote_display: FootnoteDisplay::default(),
                    smart_punctuation: false,
                    extracted_footnotes: &mut extracted,
                },
            )
            .unwrap();
        assert_eq!(section.title, "Body");
        assert!(section.html.contains("reviewdoc"));
        assert!(section.html.contains("pagemd-metadata"), "{language}");
        assert!(!section.html.contains("<pre"));
        assert!(section.html.contains("Visible."));
    }
}

#[test]
fn smart_punctuation_is_an_opt_in_render_option() {
    let source =
        "> [!NOTE]\n> A \"note\" -- here...\n\n\"Quoted\" text, `\"code\"`.\n";
    let plain = pagemd_core::render_to_html(
        source,
        &pagemd_core::RenderOptions::default(),
    )
    .unwrap();
    assert!(
        plain.contains("&quot;Quoted&quot;") || plain.contains("\"Quoted\""),
        "unchanged by default"
    );
    let smart = pagemd_core::render_to_html(
        source,
        &pagemd_core::RenderOptions {
            smart_punctuation: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(smart.contains("\u{201c}Quoted\u{201d}"), "prose");
    assert!(
        smart.contains("\u{201c}note\u{201d} \u{2013} here\u{2026}"),
        "callouts too"
    );
    assert!(
        smart.contains("&quot;code&quot;") || smart.contains("\"code\""),
        "code keeps its text"
    );
}
