//! Shared, immutable results used by all PageMD renderers.
use std::{
    collections::VecDeque,
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};

use crate::{analysis, mdast, message::Message, ParseOptions, Prepared};

#[derive(Clone, Debug)]
pub struct SourceEvent {
    pub event: pulldown_cmark::Event<'static>,
    pub range: Range<usize>,
}
#[derive(Clone, Debug)]
pub struct ParsedDocument(Arc<DocumentData>);
#[derive(Debug)]
struct DocumentData {
    source: String,
    options: ParseOptions,
    ast: mdast::Node,
    events: Vec<SourceEvent>,
}
impl ParsedDocument {
    pub fn source(&self) -> &str {
        &self.0.source
    }
    pub fn options(&self) -> &ParseOptions {
        &self.0.options
    }
    pub fn ast(&self) -> &mdast::Node {
        &self.0.ast
    }
    pub fn source_events(&self) -> &[SourceEvent] {
        &self.0.events
    }
    pub fn events(
        &self,
    ) -> impl Iterator<Item = &pulldown_cmark::Event<'static>> {
        self.0.events.iter().map(|item| &item.event)
    }
}

// Renderers can reuse immutable documents without retaining view state or
// unbounded conversation history.
static DOCUMENTS: OnceLock<Mutex<VecDeque<ParsedDocument>>> = OnceLock::new();
const MAX_DOCUMENTS: usize = 32;
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub fn parse(
    source: &str,
    options: &ParseOptions,
) -> Result<ParsedDocument, Message> {
    options.validate()?;
    let documents = DOCUMENTS.get_or_init(|| Mutex::new(VecDeque::new()));
    {
        let mut cache =
            documents.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(index) = cache
            .iter()
            .position(|doc| doc.options() == options && doc.source() == source)
        {
            let doc = cache.remove(index).unwrap();
            cache.push_back(doc.clone());
            return Ok(doc);
        }
    }
    let (original, original_events) = analysis::parse(source, options);
    let (ast, events) = match Prepared::new(source, &original, options) {
        None => (original, original_events),
        Some(prepared) => {
            let (mut ast, mut events) =
                analysis::parse(&prepared.text, options);
            prepared.restore_node(source, &mut ast);
            for item in &mut events {
                item.range = prepared.original_offset(item.range.start)
                    ..prepared.original_offset(item.range.end);
                use pulldown_cmark::Event;
                match &mut item.event {
                    Event::Text(text) | Event::Code(text) => {
                        *text = text.replace(prepared.marker, "").into()
                    }
                    Event::InlineMath(text) | Event::DisplayMath(text) => {
                        *text = analysis::trim_math(
                            &text.replace(prepared.marker, ""),
                        )
                        .into()
                    }
                    _ => {}
                }
            }
            (ast, events)
        }
    };
    let document = ParsedDocument(Arc::new(DocumentData {
        source: source.to_owned(),
        options: options.clone(),
        ast,
        events,
    }));
    if source.len() <= MAX_SOURCE_BYTES {
        let mut cache =
            documents.lock().unwrap_or_else(|error| error.into_inner());
        while !cache.is_empty()
            && (cache.len() >= MAX_DOCUMENTS
                || cache.iter().map(|doc| doc.source().len()).sum::<usize>()
                    + source.len()
                    > MAX_SOURCE_BYTES)
        {
            cache.pop_front();
        }
        cache.push_back(document.clone());
    }
    Ok(document)
}
