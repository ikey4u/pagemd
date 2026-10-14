use pagemd_parser::{
    events::Event, parse, HtmlFragmentRenderer, ParsedDocument, Renderer,
};

struct PlainRenderer;
impl Renderer for PlainRenderer {
    type Output = String;
    type Error = std::convert::Infallible;
    type Context<'a> = &'a str;
    fn render(
        &mut self,
        document: &ParsedDocument,
        prefix: &str,
    ) -> Result<String, Self::Error> {
        Ok(format!("{prefix}{}", document.ast()))
    }
}

#[test]
fn custom_and_html_renderers_consume_one_immutable_document() {
    let source =
        "# Title\n\n前**「重点」**后 $ x^2 $。\n\n$$\nx + y\n\n+ z\n$$\n";
    let mut options = pagemd_parser::ParseOptions::gfm();
    options.constructs.math_text = true;
    options.constructs.math_flow = true;
    let document = parse(source, &options).unwrap();
    let ast = document.ast().clone();
    let html = HtmlFragmentRenderer::default()
        .render(&document, ())
        .unwrap();
    let plain = PlainRenderer.render(&document, "Text: ").unwrap();
    assert!(html.contains("<strong>「重点」</strong>"));
    assert!(plain.contains("前「重点」后 x^2。"));
    assert_eq!(document.source(), source);
    assert_eq!(document.ast(), &ast);
    assert!(std::ptr::eq(
        document.ast(),
        parse(source, &options).unwrap().ast()
    ));
    let mut tags = Vec::new();
    for item in document.source_events() {
        assert!(source.get(item.range.clone()).is_some(), "{:?}", item.range);
        match &item.event {
            Event::Start(tag) => tags.push(tag.to_end()),
            Event::End(tag) => assert_eq!(tags.pop(), Some(*tag)),
            Event::Text(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text) => assert!(!text.contains('\u{200b}')),
            _ => {}
        }
    }
    assert!(tags.is_empty());
}
