//! Interpret raw HTML nodes for native presentation without an HTML renderer:
//! the set `pagemd_parser::html` defines for every renderer. Scripts, styles,
//! frames and forms' fields go with what they hold; other elements outside
//! the set are unwrapped, their text kept, as the HTML renderer does.
//!
//! Containers (`div`, `center`, `figure`, `details`, `p align`, `dl`…) become
//! block elements (`MdxJsxFlowElement`) with their allowed attributes
//! (`style` filtered as the HTML renderer filters it, `align`, `open`), and
//! inline elements drawn with their own look (`sup`, `sub`, `small`, `abbr`,
//! `span style`, `u`, `mark`) inline ones (`MdxJsxTextElement`); the view
//! draws both (model.rs, view.rs).
use html5ever::{
    local_name, namespace_url, ns, parse_fragment, tendril::TendrilSink,
    QualName,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use pagemd_parser::html::{
    filter_style, is_allowed_element, is_dropped_with_content, safe_url,
    style_property,
};
use pagemd_parser::mdast::*;

pub(crate) fn nodes(source: &str) -> Vec<Node> {
    let dom = parse_fragment(
        RcDom::default(),
        Default::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Vec::new(),
    )
    .one(source);
    children(&dom.document)
}
fn children(handle: &Handle) -> Vec<Node> {
    handle.children.borrow().iter().flat_map(convert).collect()
}
fn convert(handle: &Handle) -> Vec<Node> {
    match &handle.data {
        NodeData::Text { contents } => vec![Node::Text(Text {
            value: contents.borrow().to_string(),
            position: None,
        })],
        NodeData::Element { name, attrs, .. } => {
            let name = name.local.as_ref();
            let attr = |key: &str| {
                attrs
                    .borrow()
                    .iter()
                    .find(|a| a.name.local.as_ref() == key)
                    .map(|a| a.value.to_string())
            };
            if is_dropped_with_content(name) {
                return vec![];
            }
            let mut content = children(handle);
            if !is_allowed_element(name) {
                return content;
            }
            // An element of the set the native view draws as its text in a
            // style (underline, highlight).
            let styled = |style: &str, children: Vec<Node>| {
                Node::MdxJsxTextElement(MdxJsxTextElement {
                    children,
                    name: Some(style.to_owned()),
                    attributes: vec![],
                    position: None,
                })
            };
            // A size as written on the element (`width="320"`) or in its
            // allowed style (`width:320px`).
            // The attributes the native view reads, as the HTML renderer
            // keeps them.
            let kept = |names: &[&str]| -> Vec<AttributeContent> {
                names
                    .iter()
                    .filter_map(|n| {
                        let value = attr(n)?;
                        let value = if *n == "style" {
                            filter_style(&value)?
                        } else {
                            value
                        };
                        Some(property(n, Some(value)))
                    })
                    .collect()
            };
            let block =
                |children: Vec<Node>, attributes: Vec<AttributeContent>| {
                    Node::MdxJsxFlowElement(MdxJsxFlowElement {
                        children,
                        name: Some(name.to_owned()),
                        attributes,
                        position: None,
                    })
                };
            let size = |key: &str| {
                attr(key).or_else(|| {
                    attr("style").and_then(|s| filter_style(&s)).and_then(|s| {
                        style_property(&s, key).map(str::to_owned)
                    })
                })
            };
            let node = match name {
                // A paragraph with its own alignment or look is a box around it.
                "p" if attr("align").is_some() || attr("style").is_some() => {
                    block(
                        vec![Node::Paragraph(Paragraph {
                            children: content,
                            position: None,
                        })],
                        kept(&["style", "align"]),
                    )
                }
                "p" => Node::Paragraph(Paragraph {
                    children: content,
                    position: None,
                }),
                "strong" | "b" => Node::Strong(Strong {
                    children: content,
                    position: None,
                }),
                "em" | "i" => Node::Emphasis(Emphasis {
                    children: content,
                    position: None,
                }),
                "s" | "del" | "strike" => Node::Delete(Delete {
                    children: content,
                    position: None,
                }),
                "a" => Node::Link(Link {
                    children: content,
                    url: attr("href")
                        .filter(|href| safe_url(href, false))
                        .unwrap_or_default(),
                    title: attr("title"),
                    position: None,
                }),
                "img" => Node::Image(Image {
                    alt: attr("alt").unwrap_or_default(),
                    url: attr("src")
                        .filter(|src| safe_url(src, true))
                        .unwrap_or_default(),
                    title: attr("title"),
                    width: size("width"),
                    height: size("height"),
                    position: None,
                }),
                "u" | "ins" => styled("underline", content),
                "mark" => styled("highlight", content),
                "sup" | "sub" | "small" | "abbr" | "span" => {
                    Node::MdxJsxTextElement(MdxJsxTextElement {
                        children: content,
                        name: Some(name.to_owned()),
                        attributes: kept(&["style", "title"]),
                        position: None,
                    })
                }
                "kbd" => Node::InlineCode(InlineCode {
                    value: text(handle),
                    position: None,
                }),
                // Video and audio: their poster (a video's), and a link to
                // play them where they are.
                "video" | "audio" => {
                    let src = attr("src")
                        .or_else(|| source_src(handle))
                        .filter(|src| safe_url(src, false))
                        .unwrap_or_default();
                    let label = format!(
                        "{} {}",
                        if name == "video" { "▶" } else { "♪" },
                        src.rsplit('/').next().unwrap_or(&src)
                    );
                    let mut children = vec![];
                    if let Some(poster) =
                        attr("poster").filter(|p| safe_url(p, true))
                    {
                        children.push(Node::Image(Image {
                            alt: label.clone(),
                            url: poster,
                            title: None,
                            width: size("width"),
                            height: size("height"),
                            position: None,
                        }));
                        children.push(Node::Break(Break { position: None }));
                    }
                    children.push(Node::Link(Link {
                        children: vec![Node::Text(Text {
                            value: label,
                            position: None,
                        })],
                        url: src,
                        title: None,
                        position: None,
                    }));
                    return children;
                }
                "br" => Node::Break(Break { position: None }),
                "hr" => Node::ThematicBreak(ThematicBreak { position: None }),
                "blockquote" => Node::Blockquote(Blockquote {
                    children: ensure_blocks(content),
                    position: None,
                }),
                "pre" => Node::Code(Code {
                    value: text(handle).trim_end_matches('\n').to_owned(),
                    lang: None,
                    meta: None,
                    position: None,
                }),
                "code" => Node::InlineCode(InlineCode {
                    value: text(handle),
                    position: None,
                }),
                "ul" | "ol" => Node::List(List {
                    children: content,
                    ordered: name == "ol",
                    start: attr("start").and_then(|s| s.parse().ok()),
                    spread: false,
                    position: None,
                }),
                "li" => Node::ListItem(ListItem {
                    children: ensure_blocks(content),
                    checked: None,
                    spread: false,
                    position: None,
                }),
                // A table's caption above it, and its columns aligned as its
                // first row's cells say.
                "table" => {
                    let mut caption = None;
                    let mut rows = vec![];
                    for node in content {
                        match node {
                            Node::MdxJsxFlowElement(e)
                                if e.name.as_deref() == Some("caption") =>
                            {
                                caption = Some(Node::MdxJsxFlowElement(e))
                            }
                            other => rows.push(other),
                        }
                    }
                    let align = first_row_alignment(handle);
                    let table = Node::Table(Table {
                        align,
                        children: rows,
                        position: None,
                    });
                    return caption.into_iter().chain([table]).collect();
                }
                "tr" => Node::TableRow(TableRow {
                    children: content,
                    position: None,
                }),
                "td" | "th" => Node::TableCell(TableCell {
                    children: content,
                    position: None,
                }),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    Node::Heading(Heading {
                        depth: name[1..].parse().unwrap(),
                        children: content,
                        position: None,
                    })
                }
                "div" | "center" | "figure" | "figcaption" | "details"
                | "summary" | "dl" | "dt" | "dd" | "caption" | "picture" => {
                    block(
                        ensure_blocks(content),
                        kept(&["style", "align", "open"]),
                    )
                }
                // Table sections: their rows.
                "thead" | "tbody" | "tfoot" => return content,
                _ => return std::mem::take(&mut content),
            };
            vec![node]
        }
        _ => children(handle),
    }
}
/// An attribute of an element node (`MdxJsx…Element`).
pub(crate) fn property(name: &str, value: Option<String>) -> AttributeContent {
    AttributeContent::Property(MdxJsxAttribute {
        name: name.to_owned(),
        value: value.map(AttributeValue::Literal),
    })
}

/// An attribute's value on an element node; `Some("")` for a bare one.
pub(crate) fn attribute(
    attributes: &[AttributeContent],
    name: &str,
) -> Option<String> {
    attributes.iter().find_map(|a| match a {
        AttributeContent::Property(p) if p.name == name => {
            Some(match &p.value {
                Some(AttributeValue::Literal(v)) => v.clone(),
                _ => String::new(),
            })
        }
        _ => None,
    })
}

/// The alignment of a table's columns from its first row's `align`
/// attributes or `text-align` styles.
fn first_row_alignment(table: &Handle) -> Vec<AlignKind> {
    fn first_row(handle: &Handle) -> Option<Handle> {
        for child in handle.children.borrow().iter() {
            if let NodeData::Element { name, .. } = &child.data {
                match name.local.as_ref() {
                    "tr" => return Some(child.clone()),
                    "thead" | "tbody" | "tfoot" => {
                        if let Some(row) = first_row(child) {
                            return Some(row);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }
    let Some(row) = first_row(table) else {
        return vec![];
    };
    let cells = row.children.borrow();
    cells
        .iter()
        .filter_map(|cell| match &cell.data {
            NodeData::Element { name, attrs, .. }
                if matches!(name.local.as_ref(), "td" | "th") =>
            {
                let attrs = attrs.borrow();
                let get = |key: &str| {
                    attrs
                        .iter()
                        .find(|a| a.name.local.as_ref() == key)
                        .map(|a| a.value.to_string())
                };
                let align = get("align").or_else(|| {
                    get("style").and_then(|s| filter_style(&s)).and_then(|s| {
                        style_property(&s, "text-align").map(str::to_owned)
                    })
                });
                Some(match align.as_deref() {
                    Some("center") => AlignKind::Center,
                    Some("right") => AlignKind::Right,
                    Some("left") => AlignKind::Left,
                    _ => AlignKind::None,
                })
            }
            _ => None,
        })
        .collect()
}

/// The `src` of a media element's first `<source>`.
fn source_src(handle: &Handle) -> Option<String> {
    handle
        .children
        .borrow()
        .iter()
        .find_map(|child| match &child.data {
            NodeData::Element { name, attrs, .. }
                if name.local.as_ref() == "source" =>
            {
                attrs
                    .borrow()
                    .iter()
                    .find(|a| a.name.local.as_ref() == "src")
                    .map(|a| a.value.to_string())
            }
            _ => None,
        })
}
fn text(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => handle.children.borrow().iter().map(text).collect(),
    }
}
pub(crate) fn ensure_blocks(nodes: Vec<Node>) -> Vec<Node> {
    let mut result = vec![];
    let mut inline = vec![];
    for node in nodes {
        if matches!(
            node,
            Node::Paragraph(_)
                | Node::Heading(_)
                | Node::Code(_)
                | Node::Table(_)
                | Node::List(_)
                | Node::Blockquote(_)
                | Node::ThematicBreak(_)
                | Node::MdxJsxFlowElement(_)
        ) {
            if !inline.is_empty() {
                result.push(Node::Paragraph(Paragraph {
                    children: std::mem::take(&mut inline),
                    position: None,
                }));
            }
            result.push(node);
        } else {
            inline.push(node);
        }
    }
    if !inline.is_empty()
        && inline.iter().any(|n| {
            !n.to_string().trim().is_empty() || matches!(n, Node::Image(_))
        })
    {
        result.push(Node::Paragraph(Paragraph {
            children: inline,
            position: None,
        }));
    }
    result
}

#[cfg(test)]
mod shared_set_tests {
    use super::*;

    #[test]
    fn the_shared_set_converts_as_the_html_renderer_shows_it() {
        let nodes = nodes(
            r#"<u>u</u><mark>m</mark><kbd>K</kbd><img src="x.png" style="width:120px;position:fixed" height="40" onerror="x()"><script>gone()</script><section>kept</section><a href="javascript:x()">j</a>"#,
        );
        let styled = |n: &Node| match n {
            Node::MdxJsxTextElement(e) => e.name.clone(),
            _ => None,
        };
        assert_eq!(styled(&nodes[0]).as_deref(), Some("underline"));
        assert_eq!(styled(&nodes[1]).as_deref(), Some("highlight"));
        assert!(matches!(&nodes[2], Node::InlineCode(c) if c.value == "K"));
        match &nodes[3] {
            Node::Image(image) => {
                assert_eq!(image.url, "x.png");
                assert_eq!(image.width.as_deref(), Some("120px"));
                assert_eq!(image.height.as_deref(), Some("40"));
            }
            other => panic!("not an image: {other:?}"),
        }
        let text: String = nodes.iter().map(|n| n.to_string()).collect();
        assert!(
            !text.contains("gone"),
            "a script is dropped with its content"
        );
        assert!(
            text.contains("kept"),
            "an element outside the set is unwrapped"
        );
        assert!(
            matches!(nodes.last(), Some(Node::Link(l)) if l.url.is_empty())
        );
    }
}

/// Containers written around Markdown: Markdown gives `<details>` (or
/// `<div style>`, `<center>`…), what is inside and `</details>` as separate
/// siblings; joined, they are one element holding the Markdown, as the
/// HTML renderer shows them.
const CONTAINERS: &[&str] = &[
    "details",
    "div",
    "center",
    "figure",
    "dl",
    "dd",
    "dt",
    "blockquote",
    "p",
];

/// The opening and closing tags of `name` in `html`.
fn count_tags(html: &str, name: &str) -> (usize, usize) {
    let (mut opens, mut closes) = (0, 0);
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        rest = &rest[at..];
        match pagemd_parser::html::parse_tag(rest) {
            Some((tag, len)) => {
                if tag.name == name {
                    if tag.closing {
                        closes += 1;
                    } else if !tag.self_closing {
                        opens += 1;
                    }
                }
                rest = &rest[len..];
            }
            None => rest = &rest[1..],
        }
    }
    (opens, closes)
}

pub(crate) fn pair_containers(nodes: &[Node]) -> Vec<Node> {
    let mut out = Vec::with_capacity(nodes.len());
    let mut i = 0;
    while i < nodes.len() {
        let Some((tag, len, html)) = opening_container(&nodes[i]) else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        let (opens, closes) = count_tags(html, &tag.name);
        if opens <= closes {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        }
        // Find the sibling that closes it.
        let mut depth = opens - closes;
        let mut end = None;
        for (j, node) in nodes.iter().enumerate().skip(i + 1) {
            if let Node::Html(h) = node {
                let (o, c) = count_tags(&h.value, &tag.name);
                depth = depth + o - c.min(depth + o);
                if depth == 0 {
                    end = Some(j);
                    break;
                }
            }
        }
        let Some(end) = end else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        let trimmed = html.trim_start();
        let after_open = &trimmed[len..];
        let mut children = ensure_blocks(nodes_or_empty(after_open));
        children.extend(pair_containers(&nodes[i + 1..end]));
        let Node::Html(closer) = &nodes[end] else {
            unreachable!()
        };
        let close_tag = format!("</{}", tag.name);
        let (inside, after) =
            match closer.value.to_ascii_lowercase().rfind(&close_tag) {
                Some(at) => {
                    let rest = &closer.value[at..];
                    let gt = rest.find('>').map_or(rest.len(), |g| g + 1);
                    (&closer.value[..at], &rest[gt..])
                }
                None => ("", ""),
            };
        children.extend(ensure_blocks(nodes_or_empty(inside)));
        let attributes = pagemd_parser::html::safe_attributes(&tag)
            .into_iter()
            .filter(|(n, _)| matches!(n.as_str(), "style" | "align" | "open"))
            .map(|(n, v)| property(&n, v))
            .collect();
        let position = match (nodes[i].position(), nodes[end].position()) {
            (Some(a), Some(b)) => Some(pagemd_parser::unist::Position {
                start: a.start.clone(),
                end: b.end.clone(),
            }),
            _ => None,
        };
        let element = MdxJsxFlowElement {
            children,
            name: Some(tag.name.clone()),
            attributes,
            position,
        };
        // A `<p align>` holds inline content: its paragraph.
        out.push(Node::MdxJsxFlowElement(element));
        out.extend(ensure_blocks(nodes_or_empty(after)));
        i = end + 1;
    }
    out
}

fn nodes_or_empty(html: &str) -> Vec<Node> {
    if html.trim().is_empty() {
        vec![]
    } else {
        nodes(html)
    }
}

/// An HTML node starting with a container's opening tag: the tag, its
/// length and the node's HTML.
fn opening_container(
    node: &Node,
) -> Option<(pagemd_parser::html::Tag, usize, &str)> {
    let Node::Html(h) = node else { return None };
    let (tag, len) = pagemd_parser::html::parse_tag(h.value.trim_start())?;
    (!tag.closing
        && !tag.self_closing
        && CONTAINERS.contains(&tag.name.as_str()))
    .then_some((tag, len, h.value.as_str()))
}

/// Inline HTML written around Markdown text in a paragraph: Markdown gives
/// `<span style="…">`, the text and `</span>` as separate siblings; joined,
/// the element holds the text (styled, raised, linked…), as in HTML.
pub(crate) fn pair_inline(siblings: &[Node]) -> Vec<Node> {
    const HOLE: &str = "\u{e000}";
    let single_tag = |node: &Node| -> Option<pagemd_parser::html::Tag> {
        let Node::Html(h) = node else { return None };
        let value = h.value.trim();
        let (tag, len) = pagemd_parser::html::parse_tag(value)?;
        (len == value.len()).then_some(tag)
    };
    let mut out = Vec::with_capacity(siblings.len());
    let mut i = 0;
    while i < siblings.len() {
        let opener = single_tag(&siblings[i]).filter(|t| {
            !t.closing && !t.self_closing && is_allowed_element(&t.name)
        });
        if let Some(tag) = opener {
            let mut depth = 1;
            let mut end = None;
            for (j, node) in siblings.iter().enumerate().skip(i + 1) {
                if let Some(t) = single_tag(node).filter(|t| t.name == tag.name)
                {
                    if t.closing {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    } else if !t.self_closing {
                        depth += 1;
                    }
                }
            }
            if let (Some(end), Node::Html(h)) = (end, &siblings[i]) {
                let inner = pair_inline(&siblings[i + 1..end]);
                let shell =
                    nodes(&format!("{}{HOLE}</{}>", h.value.trim(), tag.name));
                out.extend(fill(shell, &inner, HOLE));
                i = end + 1;
                continue;
            }
        }
        out.push(siblings[i].clone());
        i += 1;
    }
    out
}

/// `nodes` with the placeholder text replaced by `inner`.
fn fill(nodes: Vec<Node>, inner: &[Node], hole: &str) -> Vec<Node> {
    nodes
        .into_iter()
        .flat_map(|node| match node {
            Node::Text(t) if t.value == hole => inner.to_vec(),
            Node::InlineCode(mut c) if c.value.contains(hole) => {
                let text: String =
                    inner.iter().map(|n| n.to_string()).collect();
                c.value = c.value.replace(hole, &text);
                vec![Node::InlineCode(c)]
            }
            mut other => {
                if let Some(children) = other.children_mut() {
                    let filled = fill(std::mem::take(children), inner, hole);
                    *children = filled;
                }
                vec![other]
            }
        })
        .collect()
}
