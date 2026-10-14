//! Renderers consume parsed documents; backend context is independent of parsing.
use crate::{message::Message, CompileOptions, ParsedDocument};

pub trait Renderer {
    type Output;
    type Error;
    type Context<'a>;
    fn render<'a>(
        &mut self,
        document: &ParsedDocument,
        context: Self::Context<'a>,
    ) -> Result<Self::Output, Self::Error>;
}

/// Basic HTML fragments for parser clients. PageMD's complete HTML renderer adds
/// themes, syntax highlighting, diagrams, math SVG and embedded resources using
/// the same trait and parsed-document interface.
#[derive(Default)]
pub struct HtmlFragmentRenderer {
    options: CompileOptions,
}
impl HtmlFragmentRenderer {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }
}
impl Renderer for HtmlFragmentRenderer {
    type Output = String;
    type Error = Message;
    type Context<'a> = ();
    fn render<'a>(
        &mut self,
        document: &ParsedDocument,
        _: (),
    ) -> Result<String, Message> {
        use pulldown_cmark::{Event, Tag, TagEnd};
        let mut events: Vec<_> = document
            .events()
            .cloned()
            .map(|event| match event {
                Event::Html(raw) | Event::InlineHtml(raw)
                    if !self.options.allow_dangerous_html =>
                {
                    Event::Text(raw)
                }
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }) if !self.options.allow_dangerous_protocol => {
                    let scheme = dest_url
                        .split_once(':')
                        .map(|(scheme, _)| scheme.to_ascii_lowercase());
                    let safe = scheme.as_deref().is_none_or(|scheme| {
                        matches!(
                            scheme,
                            "http"
                                | "https"
                                | "mailto"
                                | "irc"
                                | "ircs"
                                | "xmpp"
                        )
                    });
                    Event::Start(Tag::Link {
                        link_type,
                        dest_url: if safe { dest_url } else { "".into() },
                        title,
                        id,
                    })
                }
                event => event,
            })
            .collect();
        for index in 1..events.len() {
            if matches!(events[index], Event::End(TagEnd::CodeBlock)) {
                if let Event::Text(text) = &mut events[index - 1] {
                    if !text.ends_with('\n') && !text.is_empty() {
                        *text = format!("{text}\n").into();
                    }
                }
            }
        }
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, events.into_iter());
        Ok(html.trim_end_matches('\n').to_owned())
    }
}
