//! PageMD analysis: translate pulldown-cmark's source-aware events into a native
//! syntax tree. References, tight lists, display math and positions are retained
//! for native selection/editing; no markdown-rs parser or source is involved.
use std::ops::Range;

use pulldown_cmark::{
    CodeBlockKind, Event, LinkType, MetadataBlockKind, Parser, Tag,
};

use crate::{
    mdast::*,
    unist::{Location, Position},
    ParseOptions,
};

struct Frame {
    tag: Option<Tag<'static>>,
    range: Range<usize>,
    children: Vec<Node>,
    value: String,
    checked: Option<bool>,
    spread: bool,
}
impl Frame {
    fn root(len: usize) -> Self {
        Self {
            tag: None,
            range: 0..len,
            children: vec![],
            value: String::new(),
            checked: None,
            spread: false,
        }
    }
    fn push(&mut self, node: Node) {
        if let Node::Text(next) = &node {
            if let Some(Node::Text(previous)) = self.children.last_mut() {
                if previous
                    .position
                    .as_ref()
                    .zip(next.position.as_ref())
                    .is_some_and(|(a, b)| a.end.offset == b.start.offset)
                {
                    previous.value.push_str(&next.value);
                    previous.position.as_mut().unwrap().end =
                        next.position.as_ref().unwrap().end.clone();
                    return;
                }
            }
        }
        self.children.push(node);
    }
    fn finish(mut self, source: &str, location: &Location) -> Node {
        let block = !matches!(
            self.tag,
            Some(
                Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Link { .. }
                    | Tag::Image { .. }
                    | Tag::TableCell
            )
        );
        let position = position(source, location, self.range.clone(), block);
        let children = std::mem::take(&mut self.children);
        match self.tag {
            None => Node::Root(Root { children, position }),
            Some(Tag::Paragraph) => {
                if children.len() == 1 && matches!(children[0], Node::Math(_)) {
                    return children.into_iter().next().unwrap();
                }
                Node::Paragraph(Paragraph { children, position })
            }
            Some(Tag::Heading { level, .. }) => Node::Heading(Heading {
                children,
                depth: level as u8,
                position,
            }),
            Some(Tag::BlockQuote(_)) => {
                Node::Blockquote(Blockquote { children, position })
            }
            Some(Tag::List(start)) => Node::List(List {
                children,
                position,
                ordered: start.is_some(),
                start: start.and_then(|n| n.try_into().ok()),
                spread: self.spread,
            }),
            Some(Tag::Item) => Node::ListItem(ListItem {
                children: wrap_tight_inline(children),
                position,
                checked: self.checked,
                spread: self.spread,
            }),
            Some(Tag::Table(aligns)) => Node::Table(Table {
                children,
                position,
                align: aligns
                    .into_iter()
                    .map(|a| match a {
                        pulldown_cmark::Alignment::Left => AlignKind::Left,
                        pulldown_cmark::Alignment::Right => AlignKind::Right,
                        pulldown_cmark::Alignment::Center => AlignKind::Center,
                        pulldown_cmark::Alignment::None => AlignKind::None,
                    })
                    .collect(),
            }),
            Some(Tag::TableHead | Tag::TableRow) => {
                Node::TableRow(TableRow { children, position })
            }
            Some(Tag::TableCell) => {
                Node::TableCell(TableCell { children, position })
            }
            Some(Tag::Emphasis) => {
                Node::Emphasis(Emphasis { children, position })
            }
            Some(Tag::Strong) => Node::Strong(Strong { children, position }),
            Some(Tag::Strikethrough) => {
                Node::Delete(Delete { children, position })
            }
            Some(Tag::CodeBlock(kind)) => {
                let (lang, meta) = match kind {
                    CodeBlockKind::Indented => (None, None),
                    CodeBlockKind::Fenced(info) => {
                        let mut parts =
                            info.trim().splitn(2, char::is_whitespace);
                        (
                            parts
                                .next()
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned),
                            parts
                                .next()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned),
                        )
                    }
                };
                Node::Code(Code {
                    value: strip_last_newline(&self.value).to_owned(),
                    lang,
                    meta,
                    position,
                })
            }
            Some(Tag::HtmlBlock) => Node::Html(Html {
                value: strip_last_newline(&self.value).to_owned(),
                position,
            }),
            Some(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                Node::Yaml(Yaml {
                    value: strip_last_newline(&self.value).to_owned(),
                    position,
                })
            }
            Some(Tag::MetadataBlock(MetadataBlockKind::PlusesStyle)) => {
                Node::Toml(Toml {
                    value: strip_last_newline(&self.value).to_owned(),
                    position,
                })
            }
            Some(Tag::FootnoteDefinition(id)) => {
                Node::FootnoteDefinition(FootnoteDefinition {
                    children,
                    position,
                    identifier: identifier(&id),
                    label: Some(id.to_string()),
                })
            }
            Some(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => match reference_kind(link_type) {
                Some(reference_kind) => Node::LinkReference(LinkReference {
                    children,
                    position,
                    identifier: identifier(&id),
                    label: Some(id.to_string()),
                    reference_kind,
                }),
                None => Node::Link(Link {
                    children,
                    position,
                    url: dest_url.to_string(),
                    title: optional(&title),
                }),
            },
            Some(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let alt = children
                    .iter()
                    .map(ToString::to_string)
                    .collect::<String>();
                match reference_kind(link_type) {
                    Some(reference_kind) => {
                        Node::ImageReference(ImageReference {
                            alt,
                            position,
                            identifier: identifier(&id),
                            label: Some(id.to_string()),
                            reference_kind,
                        })
                    }
                    None => Node::Image(Image {
                        alt,
                        position,
                        url: dest_url.to_string(),
                        title: optional(&title),
                    }),
                }
            }
            // These extensions are not enabled by the native Markdown options.
            Some(_) => Node::Paragraph(Paragraph { children, position }),
        }
    }
}
fn optional(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
}
fn identifier(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn reference_kind(kind: LinkType) -> Option<ReferenceKind> {
    match kind {
        LinkType::Reference | LinkType::ReferenceUnknown => {
            Some(ReferenceKind::Full)
        }
        LinkType::Collapsed | LinkType::CollapsedUnknown => {
            Some(ReferenceKind::Collapsed)
        }
        LinkType::Shortcut | LinkType::ShortcutUnknown => {
            Some(ReferenceKind::Shortcut)
        }
        _ => None,
    }
}
fn strip_last_newline(value: &str) -> &str {
    value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .or_else(|| value.strip_suffix('\r'))
        .unwrap_or(value)
}
fn position(
    source: &str,
    location: &Location,
    mut range: Range<usize>,
    trim: bool,
) -> Option<Position> {
    if trim {
        range.end -= source[range.clone()].len()
            - source[range.clone()].trim_end_matches(['\r', '\n']).len();
    }
    Some(Position {
        start: location.to_point(range.start)?,
        end: location.to_point(range.end)?,
    })
}
fn is_inline(node: &Node) -> bool {
    matches!(
        node,
        Node::Text(_)
            | Node::Strong(_)
            | Node::Emphasis(_)
            | Node::Delete(_)
            | Node::InlineCode(_)
            | Node::InlineMath(_)
            | Node::Link(_)
            | Node::LinkReference(_)
            | Node::Image(_)
            | Node::ImageReference(_)
            | Node::FootnoteReference(_)
            | Node::Break(_)
    )
}
fn wrap_tight_inline(children: Vec<Node>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut inline: Vec<Node> = Vec::new();
    fn flush(out: &mut Vec<Node>, inline: &mut Vec<Node>) {
        if inline.is_empty() {
            return;
        }
        let position = inline
            .first()
            .and_then(Node::position)
            .zip(inline.last().and_then(Node::position))
            .map(|(a, b)| Position {
                start: a.start.clone(),
                end: b.end.clone(),
            });
        out.push(Node::Paragraph(Paragraph {
            children: std::mem::take(inline),
            position,
        }));
    }
    for node in children {
        if is_inline(&node) {
            inline.push(node);
        } else {
            flush(&mut out, &mut inline);
            out.push(node);
        }
    }
    flush(&mut out, &mut inline);
    out
}

/// Analyze original or CJK-normalized Markdown with PageMD's parser.
pub(crate) fn parse(
    source: &str,
    options: &ParseOptions,
) -> (Node, Vec<crate::SourceEvent>) {
    let location = Location::new(source.as_bytes());
    let (masked, mut math) = display_math(source, options, &location);
    let masked_math_count = math.len();
    let parser_source = masked.as_deref().unwrap_or(source);
    let parser = Parser::new_ext(parser_source, options.pulldown());
    let definitions = parser
        .reference_definitions()
        .iter()
        .map(|(id, definition)| {
            let label = source
                .get(definition.span.clone())
                .and_then(|s| s.trim_start().strip_prefix('['))
                .and_then(|s| s.split_once(']'))
                .map(|(label, _)| label.to_owned());
            Node::Definition(Definition {
                identifier: identifier(id),
                label,
                url: definition.dest.to_string(),
                title: definition.title.as_ref().map(ToString::to_string),
                position: position(
                    source,
                    &location,
                    definition.span.clone(),
                    true,
                ),
            })
        })
        .collect::<Vec<_>>();
    let mut events: Vec<_> = parser
        .into_offset_iter()
        .map(|(event, range)| crate::SourceEvent {
            event: event.into_static(),
            range,
        })
        .collect();
    let mut stack = vec![Frame::root(source.len())];
    for crate::SourceEvent { event, range } in events.iter().cloned() {
        match event {
            Event::Start(tag) => {
                if matches!(tag, Tag::Paragraph)
                    && stack
                        .last()
                        .is_some_and(|f| matches!(f.tag, Some(Tag::Item)))
                {
                    stack.last_mut().unwrap().spread = true;
                }
                stack.push(Frame {
                    tag: Some(tag.into_static()),
                    range,
                    children: vec![],
                    value: String::new(),
                    checked: None,
                    spread: false,
                });
            }
            Event::End(_) => {
                let frame = stack.pop().unwrap();
                let spread = frame.spread;
                let node = frame.finish(source, &location);
                let parent = stack.last_mut().unwrap();
                if matches!(node, Node::ListItem(_)) {
                    parent.spread |= spread;
                }
                parent.push(node);
            }
            Event::TaskListMarker(checked) => {
                if let Some(item) = stack
                    .iter_mut()
                    .rev()
                    .find(|f| matches!(f.tag, Some(Tag::Item)))
                {
                    item.checked = Some(checked);
                }
            }
            Event::Text(text)
                if stack.last().is_some_and(|f| {
                    matches!(
                        f.tag,
                        Some(Tag::CodeBlock(_) | Tag::MetadataBlock(_))
                    )
                }) =>
            {
                stack.last_mut().unwrap().value.push_str(&text)
            }
            Event::Html(text)
                if stack
                    .last()
                    .is_some_and(|f| matches!(f.tag, Some(Tag::HtmlBlock))) =>
            {
                stack.last_mut().unwrap().value.push_str(&text)
            }
            event => {
                let pos = position(source, &location, range.clone(), false);
                let node = match event {
                    Event::Text(text) => Node::Text(Text {
                        value: text.to_string(),
                        position: pos,
                    }),
                    Event::Code(text) => Node::InlineCode(InlineCode {
                        value: text.to_string(),
                        position: pos,
                    }),
                    Event::InlineMath(text) if options.constructs.math_text => {
                        Node::InlineMath(InlineMath {
                            value: trim_math(&text),
                            position: pos,
                        })
                    }
                    Event::DisplayMath(text)
                        if options.constructs.math_flow
                            && stack.last().is_some_and(|f| {
                                f.range.start == range.start
                            }) =>
                    {
                        Node::Math(Math {
                            value: trim_math(&text),
                            meta: None,
                            position: pos,
                        })
                    }
                    Event::InlineMath(_) | Event::DisplayMath(_) => {
                        Node::Text(Text {
                            value: source[range].to_owned(),
                            position: pos,
                        })
                    }
                    Event::Html(text) | Event::InlineHtml(text) => {
                        Node::Html(Html {
                            value: text.to_string(),
                            position: pos,
                        })
                    }
                    Event::SoftBreak => Node::Text(Text {
                        value: "\n".into(),
                        position: pos,
                    }),
                    Event::HardBreak => Node::Break(Break { position: pos }),
                    Event::Rule => Node::ThematicBreak(ThematicBreak {
                        position: position(source, &location, range, true),
                    }),
                    Event::FootnoteReference(id) => {
                        Node::FootnoteReference(FootnoteReference {
                            identifier: identifier(&id),
                            label: Some(id.to_string()),
                            position: pos,
                        })
                    }
                    _ => continue,
                };
                stack.last_mut().unwrap().push(node);
            }
        }
    }
    let mut root = stack.pop().unwrap().finish(source, &location);
    if let Some(children) = root.children_mut() {
        children.extend(definitions);
        for node in &math {
            if let Node::Math(math) = node {
                if let Some(position) = &math.position {
                    events.push(crate::SourceEvent {
                        event: Event::DisplayMath(math.value.clone().into()),
                        range: position.start.offset..position.end.offset,
                    });
                }
            }
        }
        children.append(&mut math);
        children
            .sort_by_key(|node| node.position().map_or(0, |p| p.start.offset));
    }
    root.position_set(Some(Position {
        start: location.to_point(0).unwrap(),
        end: location.to_point(source.len()).unwrap(),
    }));
    let added = events.split_off(events.len() - masked_math_count);
    for item in added {
        let index = events
            .iter()
            .position(|event| event.range.start >= item.range.start)
            .unwrap_or(events.len());
        events.insert(index, item);
    }
    (root, events)
}
pub(crate) fn trim_math(value: &str) -> String {
    let mut value = strip_last_newline(value);
    value = value
        .strip_prefix("\r\n")
        .or_else(|| value.strip_prefix('\n'))
        .unwrap_or(value);
    if value.starts_with(' ') && value.ends_with(' ') && value.len() > 1 {
        value = &value[1..value.len() - 1];
    }
    value.to_owned()
}

/// PageMD's native display-math analysis preserves blank lines inside a fence.
/// Masking retains every byte offset and keeps global reference resolution intact.
fn display_math(
    source: &str,
    options: &ParseOptions,
    location: &Location,
) -> (Option<String>, Vec<Node>) {
    if !options.constructs.math_flow || !source.contains("$$") {
        return (None, vec![]);
    }
    let mut protected = Vec::new();
    for (event, range) in
        Parser::new_ext(source, options.pulldown()).into_offset_iter()
    {
        if matches!(
            event,
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock)
                | Event::Code(_)
                | Event::Html(_)
                | Event::InlineHtml(_)
        ) {
            protected.push(range);
        }
    }
    let mut lines = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        lines.push((offset, line));
        offset += line.len();
    }
    let mut nodes = Vec::new();
    let mut masked = source.as_bytes().to_vec();
    let mut i = 0;
    while i < lines.len() {
        let (start, line) = lines[i];
        let trimmed = line.trim();
        if trimmed != "$$"
            || protected.iter().any(|range| range.contains(&start))
        {
            i += 1;
            continue;
        }
        let body_start = start + line.len();
        let mut end_line = i + 1;
        while end_line < lines.len() && lines[end_line].1.trim() != "$$" {
            end_line += 1;
        }
        let body_end = lines
            .get(end_line)
            .map_or(source.len(), |(start, _)| *start);
        let end = lines.get(end_line).map_or(source.len(), |(start, line)| {
            start + line.trim_end_matches(['\r', '\n']).len()
        });
        let range = start..end;
        let node = Node::Math(Math {
            value: strip_last_newline(&source[body_start..body_end]).to_owned(),
            meta: None,
            position: position(source, location, range.clone(), true),
        });
        nodes.push(node);
        for byte in &mut masked[range] {
            if !matches!(*byte, b'\r' | b'\n') {
                *byte = b' ';
            }
        }
        i = end_line.saturating_add(1);
    }
    (
        (!nodes.is_empty()).then(|| String::from_utf8(masked).unwrap()),
        nodes,
    )
}
