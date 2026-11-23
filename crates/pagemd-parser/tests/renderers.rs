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

#[test]
fn smart_punctuation_is_opt_in() {
    use pagemd_parser::{events::Event, parse, ParseOptions};
    let source = "\"Quoted\" -- it's here... and `\"code\"` stays.";
    let text = |options: &ParseOptions| {
        parse(source, options)
            .unwrap()
            .events()
            .filter_map(|event| match event {
                Event::Text(text) | Event::Code(text) => Some(text.to_string()),
                _ => None,
            })
            .collect::<String>()
    };
    assert_eq!(
        text(&ParseOptions::gfm()),
        "\"Quoted\" -- it's here... and \"code\" stays."
    );
    let mut smart = ParseOptions::gfm();
    smart.constructs.smart_punctuation = true;
    assert_eq!(text(&smart), "\u{201c}Quoted\u{201d} \u{2013} it\u{2019}s here\u{2026} and \"code\" stays.");
}
