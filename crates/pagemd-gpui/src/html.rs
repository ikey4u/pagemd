//! Interpret raw HTML nodes for native presentation without an HTML renderer:
//! the set `pagemd_parser::html` defines for every renderer. Scripts, styles,
//! frames and forms' fields go with what they hold; other elements outside
//! the set are unwrapped, their text kept, as the HTML renderer does.
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
            let size = |key: &str| {
                attr(key).or_else(|| {
                    attr("style").and_then(|s| filter_style(&s)).and_then(|s| {
                        style_property(&s, key).map(str::to_owned)
                    })
                })
            };
            let node = match name {
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
                "kbd" => Node::InlineCode(InlineCode {
                    value: text(handle),
                    position: None,
                }),
                // Video and audio: a link to play them where they are.
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
                    Node::Link(Link {
                        children: vec![Node::Text(Text {
                            value: label,
                            position: None,
                        })],
                        url: src,
                        title: None,
                        position: None,
                    })
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
                "div" | "center" | "figure" | "figcaption" | "details"
                | "summary" | "dl" | "dt" | "dd" | "caption" | "picture" => {
                    return ensure_blocks(content)
                }
                _ => return std::mem::take(&mut content),
            };
            vec![node]
        }
        _ => children(handle),
    }
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
