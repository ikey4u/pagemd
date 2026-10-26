//! Native display model and a character-level map back to the original source.
use std::{collections::HashMap, ops::Range, sync::Arc};

use gpui_kit::{EntityId, SharedString};
use pagemd_parser::{
    mdast::{self, Node},
    ParsedDocument,
};

#[derive(Clone, Default)]
pub(crate) struct Mark {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<SharedString>,
}
#[derive(Clone)]
pub(crate) struct Span {
    pub range: Range<usize>,
    pub mark: Mark,
}
#[derive(Clone)]
pub(crate) struct Object {
    pub range: Range<usize>,
    pub node: Node,
}
#[derive(Clone)]
pub(crate) struct Paragraph {
    pub text: SharedString,
    pub spans: Vec<Span>,
    pub objects: Vec<Object>,
    pub range: Range<usize>,
}
#[derive(Clone)]
pub(crate) enum Kind {
    Paragraph(Paragraph),
    Heading(u8, Paragraph),
    Code(Option<String>, Paragraph),
    Math(Paragraph),
    Group(Vec<Block>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u32,
        items: Vec<(Option<bool>, Vec<Block>)>,
    },
    Table {
        rows: Vec<Vec<Paragraph>>,
        align: Vec<mdast::AlignKind>,
    },
    Rule,
}
#[derive(Clone)]
pub(crate) struct Block {
    pub node: Node,
    pub range: Range<usize>,
    pub kind: Kind,
}
#[derive(Clone, Debug)]
struct SourceSpan {
    rendered: Range<usize>,
    source: Range<usize>,
}
#[derive(Debug)]
pub(crate) struct Index {
    text: String,
    source: String,
    mapping: Vec<SourceSpan>,
}
#[derive(Clone, Debug)]
pub struct RenderedText {
    owner: EntityId,
    revision: usize,
    index: Arc<Index>,
}
impl PartialEq for RenderedText {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.revision == other.revision
    }
}
impl Eq for RenderedText {}
impl RenderedText {
    pub(crate) fn new(
        owner: EntityId,
        revision: usize,
        index: Arc<Index>,
    ) -> Self {
        Self {
            owner,
            revision,
            index,
        }
    }
    pub fn as_str(&self) -> &str {
        &self.index.text
    }
    pub fn len(&self) -> usize {
        self.index.text.len()
    }
    pub fn is_empty(&self) -> bool {
        self.index.text.is_empty()
    }
    pub fn source(&self) -> &str {
        &self.index.source
    }
    pub fn range_for_source(
        &self,
        range: Range<usize>,
    ) -> Option<Range<usize>> {
        if range.start >= range.end
            || self.source().get(range.clone()).is_none()
        {
            return None;
        }
        hull(
            self.index
                .mapping
                .iter()
                .filter(|s| overlaps(&s.source, &range))
                .map(|s| s.rendered.clone()),
        )
    }
    pub(crate) fn source_for_range(
        &self,
        range: Range<usize>,
    ) -> Option<Range<usize>> {
        hull(
            self.index
                .mapping
                .iter()
                .filter(|s| overlaps(&s.rendered, &range))
                .map(|s| s.source.clone()),
        )
    }
}
fn hull(
    mut ranges: impl Iterator<Item = Range<usize>>,
) -> Option<Range<usize>> {
    let mut result = ranges.next()?;
    for range in ranges {
        result.start = result.start.min(range.start);
        result.end = result.end.max(range.end);
    }
    Some(result)
}
pub(crate) fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
pub(crate) fn build(document: &ParsedDocument) -> (Vec<Block>, Arc<Index>) {
    let mut builder = Builder {
        source: document.source(),
        text: String::new(),
        mapping: vec![],
        definitions: HashMap::new(),
        blockquote_depth: 0,
    };
    fn definitions(node: &Node, out: &mut HashMap<String, String>) {
        if let Node::Definition(n) = node {
            out.insert(n.identifier.to_lowercase(), n.url.clone());
        }
        if let Some(children) = node.children() {
            for child in children {
                definitions(child, out);
            }
        }
    }
    definitions(document.ast(), &mut builder.definitions);
    let blocks = document
        .ast()
        .children()
        .into_iter()
        .flatten()
        .filter_map(|n| builder.block(n))
        .collect();
    (
        blocks,
        Arc::new(Index {
            text: builder.text,
            source: document.source().to_owned(),
            mapping: builder.mapping,
        }),
    )
}
struct Builder<'a> {
    source: &'a str,
    text: String,
    mapping: Vec<SourceSpan>,
    definitions: HashMap<String, String>,
    blockquote_depth: usize,
}
impl Builder<'_> {
    fn block(&mut self, node: &Node) -> Option<Block> {
        let start = self.text.len();
        let kind = match node {
            Node::Paragraph(n) => Kind::Paragraph(self.paragraph(&n.children)),
            Node::Heading(n) => {
                Kind::Heading(n.depth, self.paragraph(&n.children))
            }
            Node::Code(n) => {
                Kind::Code(n.lang.clone(), self.literal(&n.value, node, true))
            }
            Node::Math(n) => Kind::Math(self.literal(&n.value, node, false)),
            Node::Html(n) => {
                let from = self.text.len();
                let children =
                    crate::html::ensure_blocks(crate::html::nodes(&n.value))
                        .iter()
                        .filter_map(|n| self.block(n))
                        .collect();
                if let Some(p) = node.position() {
                    self.mapping.push(SourceSpan {
                        rendered: from..self.text.len(),
                        source: p.start.offset..p.end.offset,
                    });
                }
                Kind::Group(children)
            }
            Node::Blockquote(n) => {
                self.blockquote_depth += 1;
                let children =
                    n.children.iter().filter_map(|c| self.block(c)).collect();
                self.blockquote_depth -= 1;
                Kind::Quote(children)
            }
            Node::List(n) => Kind::List {
                ordered: n.ordered,
                start: n.start.unwrap_or(1),
                items: n
                    .children
                    .iter()
                    .filter_map(|item| {
                        if let Node::ListItem(item) = item {
                            Some((
                                item.checked,
                                item.children
                                    .iter()
                                    .filter_map(|n| self.block(n))
                                    .collect(),
                            ))
                        } else {
                            None
                        }
                    })
                    .collect(),
            },
            Node::Table(n) => {
                let mut rows = Vec::new();
                for row in &n.children {
                    if let Node::TableRow(row) = row {
                        let mut cells = Vec::new();
                        for cell in &row.children {
                            if let Node::TableCell(cell) = cell {
                                if !cells.is_empty() {
                                    self.text.push(' ');
                                }
                                cells.push(self.paragraph(&cell.children));
                            }
                        }
                        self.text.push('\n');
                        rows.push(cells);
                    }
                }
                Kind::Table {
                    align: n.align.clone(),
                    rows,
                }
            }
            Node::ThematicBreak(_) => Kind::Rule,
            Node::FootnoteDefinition(n) => Kind::Quote(
                n.children.iter().filter_map(|n| self.block(n)).collect(),
            ),
            _ => return None,
        };
        if !matches!(kind, Kind::Group(_) | Kind::Quote(_) | Kind::List { .. })
            && !self.text.ends_with('\n')
        {
            self.text.push('\n');
        }
        Some(Block {
            node: node.clone(),
            range: start..self.text.len(),
            kind,
        })
    }
    fn literal(
        &mut self,
        value: &str,
        node: &Node,
        literal: bool,
    ) -> Paragraph {
        let start = self.text.len();
        self.append(value, node, literal);
        let end = self.text.len();
        Paragraph {
            text: value.to_owned().into(),
            spans: vec![Span {
                range: 0..value.len(),
                // Fenced code inherits its block typography. The inline-code
                // mark is reserved for spans inside prose, including its chip background.
                mark: Mark::default(),
            }],
            objects: vec![],
            range: start..end,
        }
    }
    fn paragraph(&mut self, nodes: &[Node]) -> Paragraph {
        let start = self.text.len();
        let mut spans = vec![];
        let mut objects = vec![];
        for node in nodes {
            self.inline(node, Mark::default(), start, &mut spans, &mut objects);
        }
        Paragraph {
            text: self.text[start..].to_owned().into(),
            spans,
            objects,
            range: start..self.text.len(),
        }
    }
    fn inline(
        &mut self,
        node: &Node,
        mut mark: Mark,
        start: usize,
        spans: &mut Vec<Span>,
        objects: &mut Vec<Object>,
    ) {
        match node {
            Node::Strong(_) => mark.bold = true,
            Node::Emphasis(_) => mark.italic = true,
            Node::Delete(_) => mark.strike = true,
            Node::Link(n) => mark.link = Some(n.url.clone().into()),
            Node::LinkReference(n) => {
                mark.link = self
                    .definitions
                    .get(&n.identifier.to_lowercase())
                    .cloned()
                    .map(Into::into)
            }
            _ => {}
        }
        if let Some(children) = node.children() {
            for child in children {
                self.inline(child, mark.clone(), start, spans, objects);
            }
            return;
        }
        let offset = self.text.len();
        match node {
            Node::Text(n) => {
                // Match the HTML renderer: quoted lines retain visible breaks,
                // while ordinary paragraph soft breaks collapse to spaces.
                let newline =
                    if self.blockquote_depth > 0 { "\n" } else { " " };
                self.append(
                    &n.value
                        .replace("\r\n", newline)
                        .replace(['\n', '\r'], newline),
                    node,
                    false,
                );
            }
            Node::InlineCode(n) => {
                mark.code = true;
                self.append(&n.value, node, true);
            }
            Node::InlineMath(n) => {
                self.append_atomic(&n.value, node);
                objects.push(Object {
                    range: offset - start..self.text.len() - start,
                    node: node.clone(),
                });
            }
            Node::Image(n) => {
                self.append_atomic(&n.alt, node);
                objects.push(Object {
                    range: offset - start..self.text.len() - start,
                    node: node.clone(),
                });
            }
            Node::ImageReference(n) => {
                self.append_atomic(&n.alt, node);
                objects.push(Object {
                    range: offset - start..self.text.len() - start,
                    node: node.clone(),
                });
            }
            Node::FootnoteReference(n) => {
                self.append_atomic(&format!("[{}]", n.identifier), node)
            }
            Node::Break(_) => self.append_atomic("\n", node),
            Node::Html(n) => {
                for child in crate::html::nodes(&n.value) {
                    self.inline(&child, mark.clone(), start, spans, objects);
                }
                // The converted children already own their text spans. Adding
                // another span for the HTML node duplicates those bytes and
                // can split a later UTF-8 character in GPUI's text runs.
                return;
            }
            _ => {}
        }
        if offset != self.text.len() {
            spans.push(Span {
                range: offset - start..self.text.len() - start,
                mark,
            });
        }
    }
    fn append_atomic(&mut self, value: &str, node: &Node) {
        let value = if value.is_empty() { "\u{fffc}" } else { value };
        let start = self.text.len();
        self.text.push_str(value);
        if let Some(p) = node.position() {
            self.mapping.push(SourceSpan {
                rendered: start..self.text.len(),
                source: p.start.offset..p.end.offset,
            });
        }
    }
    fn append(&mut self, value: &str, node: &Node, literal: bool) {
        let start = self.text.len();
        self.text.push_str(value);
        let Some(p) = node.position() else {
            return;
        };
        let range = p.start.offset..p.end.offset;
        let raw = &self.source[range.clone()];
        let mut cursor =
            if matches!(node, Node::InlineCode(_) | Node::InlineMath(_)) {
                raw.chars()
                    .take_while(|c| *c == '`' || *c == '$')
                    .map(char::len_utf8)
                    .sum()
            } else if matches!(node, Node::Code(_))
                && (raw.starts_with("```") || raw.starts_with("~~~"))
            {
                raw.find('\n').map_or(0, |i| i + 1)
            } else {
                0
            };
        let mut tokens = Vec::new();
        while cursor < raw.len() {
            let from = cursor;
            let c = raw[cursor..].chars().next().unwrap();
            cursor += c.len_utf8();
            let mut decoded = c.to_string();
            if !literal && c == '\\' {
                if let Some(next) = raw[cursor..]
                    .chars()
                    .next()
                    .filter(char::is_ascii_punctuation)
                {
                    cursor += next.len_utf8();
                    decoded = next.to_string();
                }
            } else if !literal && c == '&' {
                if let Some(end) = raw[from..].find(';').filter(|&i| i < 34) {
                    let entity = &raw[from..from + end + 1];
                    if let Some(&(first, second)) =
                        html5ever::data::NAMED_ENTITIES.get(&entity[1..])
                    {
                        decoded = [first, second]
                            .into_iter()
                            .filter(|&n| n != 0)
                            .filter_map(char::from_u32)
                            .collect();
                        cursor = from + end + 1;
                    } else {
                        let text = html_escape::decode_html_entities(entity);
                        if text != entity {
                            decoded = text.into_owned();
                            cursor = from + end + 1;
                        }
                    }
                }
            } else if c == '\r' && raw[cursor..].starts_with('\n') {
                cursor += 1;
                decoded = "\n".into();
            }
            for ch in decoded.chars() {
                tokens.push((ch, range.start + from..range.start + cursor));
            }
        }
        let mut tokens = tokens.into_iter();
        for (output, ch) in value.char_indices() {
            if let Some((_, source)) =
                tokens.find(|(c, _)| *c == ch || (ch == ' ' && *c == '\n'))
            {
                self.mapping.push(SourceSpan {
                    rendered: start + output..start + output + ch.len_utf8(),
                    source,
                });
            }
        }
    }
}
