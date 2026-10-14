use pagemd::{
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
    implements_renderer::<pagemd::gpui::GpuiRenderer>();
}
