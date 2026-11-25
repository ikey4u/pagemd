//! Native display model and a character-level map back to the original source.
use std::{collections::HashMap, ops::Range, sync::Arc};

use gpui_kit::{EntityId, SharedString};
use pagemd_parser::{
    mdast::{self, Node},
    ParsedDocument,
};

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Mark {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// `<u>`, `<ins>`.
    pub underline: bool,
    /// `<mark>`.
    pub highlight: bool,
    /// A colour of the text, from a `style` (`0xRRGGBBAA`).
    pub color: Option<u32>,
    /// Its background, from a `style`.
    pub background: Option<u32>,
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
    Metadata(Vec<(usize, bool, Paragraph)>),
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
    /// An HTML container (`div`, `center`, `figure`, `p align`, `dd`…) with
    /// the look its tag and allowed `style` give it.
    Boxed {
        style: BoxStyle,
        children: Vec<Block>,
    },
    /// `<details>`: its summary line, open or folded (the reader toggles it).
    Details {
        open: bool,
        summary: Paragraph,
        children: Vec<Block>,
    },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Align {
    Start,
    Center,
    End,
    Between,
}
/// What an HTML container looks like, from its tag and allowed `style`.
#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct BoxStyle {
    pub align: Option<Align>,
    pub background: Option<u32>,
    pub border: Option<(f32, Option<u32>)>,
    pub radius: Option<f32>,
    /// Top, right, bottom, left.
    pub padding: Option<[f32; 4]>,
    pub margin: Option<(f32, f32)>,
    /// Left indent (`dd`).
    pub indent: f32,
    /// `display: flex`: children side by side.
    pub row: bool,
    pub gap: Option<f32>,
    pub justify: Option<Align>,
    pub wrap: bool,
    /// Captions: smaller and muted.
    pub caption: bool,
    pub width: Option<Size>,
    pub max_width: Option<Size>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Size {
    Px(f32),
    Fraction(f32),
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
    pub fn source_for_range(
        &self,
        range: Range<usize>,
    ) -> Option<Range<usize>> {
        self.index.text.get(range.clone())?;
        let mut covered = range.start;
        for span in self
            .index
            .mapping
            .iter()
            .filter(|span| overlaps(&span.rendered, &range))
        {
            let start = span.rendered.start.max(range.start);
            if start > covered
                && !self.index.text[covered..start].trim().is_empty()
            {
                return None;
            }
            covered = covered.max(span.rendered.end.min(range.end));
        }
        if covered < range.end
            && !self.index.text[covered..range.end].trim().is_empty()
        {
            return None;
        }
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
/// A selection contained in one leaf and one inline style can copy plain text.
/// Structural boundaries and inline objects retain the original source.
pub(crate) fn selection_has_one_style(
    blocks: &[Block],
    range: &Range<usize>,
) -> bool {
    blocks.iter().any(|block| match &block.kind {
        Kind::Paragraph(p) | Kind::Heading(_, p) | Kind::Code(_, p) => {
            paragraph_has_one_style(p, range)
        }
        Kind::Group(children)
        | Kind::Quote(children)
        | Kind::Boxed { children, .. } => {
            selection_has_one_style(children, range)
        }
        Kind::Details {
            summary, children, ..
        } => {
            paragraph_has_one_style(summary, range)
                || selection_has_one_style(children, range)
        }
        Kind::List { items, .. } => items
            .iter()
            .any(|(_, children)| selection_has_one_style(children, range)),
        Kind::Table { rows, .. } => rows
            .iter()
            .flatten()
            .any(|p| paragraph_has_one_style(p, range)),
        Kind::Math(_) | Kind::Rule | Kind::Metadata(_) => false,
    })
}
fn paragraph_has_one_style(p: &Paragraph, range: &Range<usize>) -> bool {
    if range.start >= range.end
        || range.start < p.range.start
        || range.end > p.range.end
    {
        return false;
    }
    let local = range.start - p.range.start..range.end - p.range.start;
    if p.objects
        .iter()
        .any(|object| overlaps(&object.range, &local))
    {
        return false;
    }
    let mut spans = p.spans.iter().filter(|span| overlaps(&span.range, &local));
    let Some(first) = spans.next() else {
        return false;
    };
    let mut end = first.range.end;
    if first.range.start > local.start {
        return false;
    }
    for span in spans {
        if span.mark != first.mark || span.range.start > end {
            return false;
        }
        end = end.max(span.range.end);
    }
    end >= local.end
}
pub(crate) fn build(document: &ParsedDocument) -> (Vec<Block>, Arc<Index>) {
    let mut builder = Builder {
        source: document.source(),
        text: String::new(),
        mapping: vec![],
        definitions: HashMap::new(),
        blockquote_depth: 0,
        base: Mark::default(),
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
    let root: Vec<Node> =
        document.ast().children().cloned().unwrap_or_default();
    let blocks = builder.blocks(&root);
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
    /// The text style containers give what they hold (`color`, weight…).
    base: Mark,
}
impl Builder<'_> {
    /// The blocks of a list of siblings, HTML containers written around
    /// Markdown (`<details>` … `</details>`) joined into one element first.
    fn blocks(&mut self, nodes: &[Node]) -> Vec<Block> {
        crate::html::pair_containers(nodes)
            .iter()
            .filter_map(|n| self.block(n))
            .collect()
    }
    fn block(&mut self, node: &Node) -> Option<Block> {
        let start = self.text.len();
        let kind = match node {
            Node::Paragraph(n) => Kind::Paragraph(self.paragraph(&n.children)),
            Node::Heading(n) => {
                Kind::Heading(n.depth, self.paragraph(&n.children))
            }
            Node::Code(n)
                if n.meta.as_deref() == Some("frontmatter")
                    && node
                        .position()
                        .and_then(|p| {
                            pagemd_parser::frontmatter::detect(
                                &self.source[p.start.offset..p.end.offset],
                            )
                        })
                        .is_some() =>
            {
                let value = pagemd_parser::frontmatter::presentation(
                    n.lang.as_deref().unwrap_or(""),
                    &n.value,
                )?;
                let mut rows = vec![];
                let mapping_start = self.mapping.len();
                self.metadata(&value, node, 0, &mut rows);
                // A decoded or ambiguous field must never enable a partial source edit.
                if rows.iter().any(|(_, _, p)| {
                    !p.text.is_empty()
                        && !self.mapping[mapping_start..].iter().any(|span| {
                            span.rendered.start == p.range.start
                                && span.rendered.end <= p.range.end
                        })
                }) {
                    self.mapping.truncate(mapping_start);
                }
                Kind::Metadata(rows)
            }
            Node::Code(n) => {
                Kind::Code(n.lang.clone(), self.literal(&n.value, node, true))
            }
            Node::Math(n) => Kind::Math(self.literal(&n.value, node, false)),
            Node::Html(n) => {
                let from = self.text.len();
                let children = self.blocks(&crate::html::ensure_blocks(
                    crate::html::nodes(&n.value),
                ));
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
                let children = self.blocks(&n.children);
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
                            Some((item.checked, self.blocks(&item.children)))
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
            Node::FootnoteDefinition(n) => {
                Kind::Quote(self.blocks(&n.children))
            }
            Node::MdxJsxFlowElement(n) => self.container(n),
            _ => return None,
        };
        if !matches!(
            kind,
            Kind::Group(_)
                | Kind::Quote(_)
                | Kind::List { .. }
                | Kind::Boxed { .. }
                | Kind::Details { .. }
        ) && !self.text.ends_with('\n')
        {
            self.text.push('\n');
        }
        Some(Block {
            node: node.clone(),
            range: start..self.text.len(),
            kind,
        })
    }
    /// An HTML container: `<details>` foldable, others boxed with their look;
    /// what they hold takes their text style.
    fn container(&mut self, n: &mdast::MdxJsxFlowElement) -> Kind {
        let name = n.name.as_deref().unwrap_or("div");
        if name == "details" {
            let open = crate::html::attribute(&n.attributes, "open").is_some();
            let mut rest = n.children.clone();
            let summary_at = rest.iter().position(|c| {
                matches!(c, Node::MdxJsxFlowElement(e) if e.name.as_deref() == Some("summary"))
            });
            let summary_nodes = match summary_at.map(|i| rest.remove(i)) {
                Some(Node::MdxJsxFlowElement(e)) => e
                    .children
                    .into_iter()
                    .flat_map(|c| match c {
                        Node::Paragraph(p) => p.children,
                        other => vec![other],
                    })
                    .collect(),
                _ => vec![Node::Text(mdast::Text {
                    value: "Details".into(),
                    position: None,
                })],
            };
            let summary = self.paragraph(&summary_nodes);
            self.text.push('\n');
            let children = self.blocks(&rest);
            return Kind::Details {
                open,
                summary,
                children,
            };
        }
        let (style, text) = box_style(name, &n.attributes, &self.base);
        let saved = std::mem::replace(&mut self.base, text);
        let children = self.blocks(&n.children);
        self.base = saved;
        Kind::Boxed { style, children }
    }
    fn metadata(
        &mut self,
        value: &pagemd_parser::frontmatter::Value,
        node: &Node,
        depth: usize,
        rows: &mut Vec<(usize, bool, Paragraph)>,
    ) {
        use pagemd_parser::frontmatter::Value;
        match value {
            Value::Object(fields) if fields.is_empty() => {
                self.metadata(&Value::Scalar("{}".into()), node, depth, rows)
            }
            Value::List(items) if items.is_empty() => {
                self.metadata(&Value::Scalar("[]".into()), node, depth, rows)
            }
            Value::Object(fields) => {
                for (key, value) in fields {
                    let paragraph = self.literal(key, node, true);
                    rows.push((depth, true, paragraph));
                    self.text.push('\n');
                    self.metadata(value, node, depth + 1, rows);
                }
            }
            Value::List(items) => {
                for value in items {
                    self.metadata(value, node, depth + 1, rows);
                }
            }
            Value::Scalar(value) => {
                let paragraph = self.literal(value, node, true);
                rows.push((depth, false, paragraph));
                self.text.push('\n');
            }
        }
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
        let nodes = with_image_attributes(&crate::html::pair_inline(nodes));
        for node in &nodes {
            self.inline(
                node,
                self.base.clone(),
                start,
                &mut spans,
                &mut objects,
            );
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
            Node::MdxJsxTextElement(n) => {
                match n.name.as_deref() {
                    Some("underline") => mark.underline = true,
                    Some("highlight") => mark.highlight = true,
                    // An abbreviation: underlined, its meaning in the source.
                    Some("abbr") => mark.underline = true,
                    _ => {}
                }
                if let Some(style) =
                    crate::html::attribute(&n.attributes, "style")
                {
                    mark = text_mark(&style, mark);
                }
                // Raised, lowered or smaller text: its own small run beside
                // the line (one shaped line has one size).
                if matches!(n.name.as_deref(), Some("sup" | "sub" | "small")) {
                    let offset = self.text.len();
                    self.append_atomic(&node.to_string(), node);
                    objects.push(Object {
                        range: offset - start..self.text.len() - start,
                        node: node.clone(),
                    });
                    return;
                }
            }
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
        if matches!(node, Node::Code(code) if code.meta.as_deref() == Some("frontmatter"))
            && pagemd_parser::frontmatter::detect(raw).is_some()
        {
            let candidates: Vec<_> = raw
                .match_indices(value)
                .filter(|(offset, _)| {
                    let line_start =
                        raw[..*offset].rfind('\n').map_or(0, |at| at + 1);
                    let prefix = &raw[line_start..*offset];
                    let line = raw[line_start..]
                        .split('\n')
                        .next()
                        .unwrap_or_default();
                    !line.trim_start().starts_with('#') && !prefix.contains('#')
                })
                .collect();
            if !value.is_empty() && candidates.len() == 1 {
                let source_start = range.start + candidates[0].0;
                for (offset, ch) in value.char_indices() {
                    self.mapping.push(SourceSpan {
                        rendered: start + offset
                            ..start + offset + ch.len_utf8(),
                        source: source_start + offset
                            ..source_start + offset + ch.len_utf8(),
                    });
                }
            }
            return;
        }

        let metadata = matches!(node, Node::Code(code) if code.meta.as_deref() == Some("frontmatter"))
            .then(|| pagemd_parser::frontmatter::detect(raw))
            .flatten();
        let mut cursor = if let Some(metadata) = metadata {
            metadata.value_range.start
        } else if matches!(node, Node::InlineCode(_) | Node::InlineMath(_)) {
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

/// `{width=320 …}` written right after an image (as the HTML renderer reads
/// it, `pagemd_parser::html`): the image's size, and the braces left out of
/// the text.
fn with_image_attributes(nodes: &[Node]) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    for node in nodes {
        if let (Some(Node::Image(image)), Node::Text(text)) =
            (out.last_mut(), node)
        {
            if let Some((attributes, rest)) =
                pagemd_parser::html::leading_image_attributes(&text.value)
            {
                if attributes.width.is_some() {
                    image.width = attributes.width.clone();
                }
                if attributes.height.is_some() {
                    image.height = attributes.height.clone();
                }
                if let Some(px) = attributes.width_px() {
                    image.width = Some(px.to_string());
                }
                if let Some(px) = attributes.height_px() {
                    image.height = Some(px.to_string());
                }
                if !rest.is_empty() {
                    out.push(Node::Text(pagemd_parser::mdast::Text {
                        value: rest.to_owned(),
                        position: text.position.clone(),
                    }));
                }
                continue;
            }
        }
        out.push(node.clone());
    }
    out
}

/// The text style a `style` gives (colour, background, weight, slant,
/// decoration), over `base`.
pub(crate) fn text_mark(style: &str, mut base: Mark) -> Mark {
    use pagemd_parser::html::{parse_color, style_declarations};
    for (property, value) in style_declarations(style) {
        let v = value.to_ascii_lowercase();
        match property.as_str() {
            "color" => base.color = parse_color(&v).or(base.color),
            "background-color" => {
                base.background = parse_color(&v).or(base.background)
            }
            "font-weight" => {
                base.bold = v == "bold"
                    || v == "bolder"
                    || v.parse::<u32>().is_ok_and(|w| w >= 600)
            }
            "font-style" => base.italic = v == "italic" || v == "oblique",
            "text-decoration" => {
                base.underline |= v.contains("underline");
                base.strike |= v.contains("line-through");
            }
            _ => {}
        }
    }
    base
}

/// A CSS length in pixels (`8`, `8px`, `1.5em`, `2rem` as 16px ems).
pub(crate) fn css_px(value: &str) -> Option<f32> {
    let v = value.trim().to_ascii_lowercase();
    if let Some(n) = v.strip_suffix("rem").or_else(|| v.strip_suffix("em")) {
        return n.trim().parse::<f32>().ok().map(|n| n * 16.);
    }
    pagemd_parser::html::length_px(&v).or_else(|| (v == "0").then_some(0.))
}

fn css_size(value: &str) -> Option<Size> {
    let v = value.trim();
    match v.strip_suffix('%') {
        Some(pct) => pct
            .trim()
            .parse::<f32>()
            .ok()
            .map(|p| Size::Fraction((p / 100.).clamp(0., 1.))),
        None => css_px(v).map(Size::Px),
    }
}

fn css_box(value: &str) -> Option<[f32; 4]> {
    let parts: Vec<f32> = value
        .split_whitespace()
        .map(css_px)
        .collect::<Option<_>>()?;
    Some(match parts.as_slice() {
        [a] => [*a; 4],
        [v, h] => [*v, *h, *v, *h],
        [t, h, b] => [*t, *h, *b, *h],
        [t, r, b, l] => [*t, *r, *b, *l],
        _ => return None,
    })
}

fn align_of(value: &str) -> Option<Align> {
    match value.trim().to_ascii_lowercase().as_str() {
        "center" => Some(Align::Center),
        "right" | "end" | "flex-end" => Some(Align::End),
        "left" | "start" | "flex-start" => Some(Align::Start),
        "space-between" | "space-around" | "space-evenly" => {
            Some(Align::Between)
        }
        _ => None,
    }
}

/// An HTML container's look and the text style it gives what it holds,
/// from its tag, `align` and allowed `style` (as the HTML renderer shows them).
pub(crate) fn box_style(
    name: &str,
    attributes: &[mdast::AttributeContent],
    base: &Mark,
) -> (BoxStyle, Mark) {
    use pagemd_parser::html::{parse_color, style_declarations};
    let mut style = BoxStyle::default();
    let mut text = base.clone();
    match name {
        "center" => style.align = Some(Align::Center),
        "dd" => style.indent = 24.,
        "dt" => text.bold = true,
        "figure" => style.margin = Some((8., 8.)),
        "figcaption" | "caption" => style.caption = true,
        _ => {}
    }
    if let Some(align) =
        crate::html::attribute(attributes, "align").and_then(|a| align_of(&a))
    {
        style.align = Some(align);
    }
    if let Some(css) = crate::html::attribute(attributes, "style") {
        text = text_mark(&css, text);
        for (property, value) in style_declarations(&css) {
            match property.as_str() {
                "text-align" => style.align = align_of(&value).or(style.align),
                "background-color" => style.background = parse_color(&value),
                "border" => {
                    let width =
                        value.split_whitespace().find_map(css_px).unwrap_or(1.);
                    let color = value.split_whitespace().find_map(parse_color);
                    if !value.trim().eq_ignore_ascii_case("none") {
                        style.border = Some((width, color));
                    }
                }
                "border-radius" => style.radius = css_px(&value),
                "padding" => style.padding = css_box(&value),
                "margin" => {
                    style.margin = css_box(&value).map(|m| (m[0], m[2]))
                }
                "margin-top" => {
                    style.margin = Some((
                        css_px(&value).unwrap_or(0.),
                        style.margin.map_or(0., |m| m.1),
                    ))
                }
                "margin-bottom" => {
                    style.margin = Some((
                        style.margin.map_or(0., |m| m.0),
                        css_px(&value).unwrap_or(0.),
                    ))
                }
                "margin-left" => {
                    style.indent = css_px(&value).unwrap_or(style.indent)
                }
                "display" => {
                    style.row =
                        value.trim() == "flex" || value.trim() == "inline-flex"
                }
                "gap" => {
                    style.gap =
                        css_px(value.split_whitespace().next().unwrap_or(""))
                }
                "justify-content" => style.justify = align_of(&value),
                "flex-wrap" => style.wrap = value.trim() == "wrap",
                "width" => style.width = css_size(&value),
                "max-width" => style.max_width = css_size(&value),
                // A centred block by `margin: 0 auto` reads as centred too.
                _ => {}
            }
        }
        if css.replace(' ', "").contains("margin:0auto")
            || css.contains("margin: auto")
        {
            style.align = Some(Align::Center);
        }
    }
    (style, text)
}
