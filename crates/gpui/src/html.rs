//! Interpret raw HTML nodes for native presentation without an HTML renderer.
use html5ever::{
    local_name, namespace_url, ns, parse_fragment, tendril::TendrilSink,
    QualName,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
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
            let mut content = children(handle);
            let node = match name {
                "script" | "style" | "iframe" | "head" => return vec![],
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
                "s" | "del" => Node::Delete(Delete {
                    children: content,
                    position: None,
                }),
                "a" => Node::Link(Link {
                    children: content,
                    url: attr("href").unwrap_or_default(),
                    title: attr("title"),
                    position: None,
                }),
                "img" => Node::Image(Image {
                    alt: attr("alt").unwrap_or_default(),
                    url: attr("src").unwrap_or_default(),
                    title: attr("title"),
                    position: None,
                }),
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
                "table" => Node::Table(Table {
                    align: vec![],
                    children: content,
                    position: None,
                }),
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
                "div" | "section" | "article" | "details" => {
                    return ensure_blocks(content)
                }
                _ => return std::mem::take(&mut content),
            };
            vec![node]
        }
        _ => children(handle),
    }
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
