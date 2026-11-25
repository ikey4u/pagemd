//! What writing in Markdown needs beyond CommonMark and GFM, in the HTML
//! renderer, and how far a document's own HTML is trusted:
//!
//! - [`RawHtml`]: HTML written in the Markdown kept as written, or limited
//!   to the set every renderer shows (`pagemd_parser::html`).
//! - [`Resources`]: images and media embedded into the page (a portable
//!   file), or linked as written (a page served from a website, which must
//!   not read the server's files or fetch addresses for a visitor).
//! - `[TOC]`: a table of contents of the document's headings.
//! - `![alt](src){width=320 .wide style="…"}`: an image's size, classes and
//!   style (read by `pagemd_parser::html::parse_image_attributes`).
//! - `Table: caption` right after a table: the table's caption.
//!
//! Heading ids and classes (`## Title {#id .class}`) and definition lists
//! come from the parser.

use std::fmt::Write as _;

use pagemd_parser::html::{attribute_escape, leading_image_attributes};

use crate::model::HeadingOutline;

/// How HTML written in the Markdown is treated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RawHtml {
    /// As written, scripts included (documents one trusts, as a file of
    /// one's own).
    #[default]
    Keep,
    /// Only the set of `pagemd_parser::html`: text styles, sized images and
    /// media, centering, figures, details; no scripts, event handlers or
    /// `javascript:` addresses.
    Safe,
}

/// How images and other resources the Markdown refers to are given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resources {
    /// Read (local files, web addresses) and embedded as data.
    #[default]
    Embed,
    /// Left as written: nothing is read or fetched.
    Link,
}

/// The writing options a render carries down to nested Markdown (callouts,
/// details).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Writing {
    pub raw_html: RawHtml,
    pub resources: Resources,
}

/// `[TOC]` (or `[[TOC]]`, `[[_TOC_]]`) on a line of its own: the document's
/// headings as nested links, the title (a lone first-level heading at the
/// top) left out.
pub fn insert_table_of_contents(
    html: &str,
    outline: &[HeadingOutline],
) -> String {
    const MARKERS: &[&str] = &[
        "<p>[TOC]</p>",
        "<p>[[TOC]]</p>",
        "<p>[[_TOC_]]</p>",
        "<p>[toc]</p>",
        "<p>[[toc]]</p>",
    ];
    if !MARKERS.iter().any(|m| html.contains(m)) {
        return html.to_string();
    }
    let headings: Vec<&HeadingOutline> = match outline.first() {
        Some(first)
            if first.level == 1
                && outline.iter().filter(|h| h.level == 1).count() == 1 =>
        {
            outline.iter().skip(1).collect()
        }
        _ => outline.iter().collect(),
    };
    let nav = table_of_contents(&headings);
    let mut out = html.to_string();
    for marker in MARKERS {
        out = out.replace(marker, &nav);
    }
    out
}

fn table_of_contents(headings: &[&HeadingOutline]) -> String {
    if headings.is_empty() {
        return String::new();
    }
    let base = headings.iter().map(|h| h.level).min().unwrap_or(1);
    let mut out =
        String::from("<nav class=\"toc\" aria-label=\"Contents\">\n<ul>\n");
    let mut depth = 0usize;
    let mut open_item = false;
    for heading in headings {
        let level = (heading.level - base) as usize;
        if level > depth {
            for _ in depth..level {
                out.push_str("\n<ul>\n");
            }
        } else {
            if open_item {
                out.push_str("</li>\n");
            }
            for _ in level..depth {
                out.push_str("</ul>\n</li>\n");
            }
        }
        depth = level;
        let _ = write!(
            out,
            "<li><a href=\"#{}\">{}</a>",
            attribute_escape(&heading.id),
            text_escape(&heading.text)
        );
        open_item = true;
    }
    if open_item {
        out.push_str("</li>\n");
    }
    for _ in 0..depth {
        out.push_str("</ul>\n</li>\n");
    }
    out.push_str("</ul>\n</nav>\n");
    out
}

fn text_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `{width=320 height=200 .class #id style="…" align=center}` right after an
/// image: its attributes. Rendered text holds the braces as text.
pub fn apply_image_attributes(html: &str) -> String {
    if !html.contains("<img ") || !html.contains('{') {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find("<img ") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(close) = rest.find('>') else { break };
        let (tag, after) = rest.split_at(close + 1);
        if let Some((attributes, remaining)) = leading_image_attributes(after) {
            let self_closing = tag.ends_with("/>");
            let head =
                tag.trim_end_matches('>').trim_end_matches('/').trim_end();
            out.push_str(head);
            out.push_str(&attributes.to_html());
            out.push_str(if self_closing { " />" } else { ">" });
            rest = remaining;
            continue;
        }
        out.push_str(tag);
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A paragraph `Table: …` right after a table: its `<caption>`.
pub fn apply_table_captions(html: &str) -> String {
    const LEAD: &str = "<p>Table: ";
    if !html.contains(LEAD) {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find(LEAD) {
        let before = &rest[..at];
        let trimmed = before.trim_end();
        let after_lead = &rest[at + LEAD.len()..];
        let Some(end) = after_lead.find("</p>") else {
            break;
        };
        let ends_table = trimmed.ends_with("</table></div>")
            || trimmed.ends_with("</table>");
        match (ends_table, before.rfind("<table")) {
            (true, Some(table_at)) => {
                let open_end = before[table_at..]
                    .find('>')
                    .map(|e| table_at + e + 1)
                    .unwrap_or(table_at);
                out.push_str(&before[..open_end]);
                let _ =
                    write!(out, "\n<caption>{}</caption>", &after_lead[..end]);
                out.push_str(&before[open_end..]);
                rest = after_lead[end + 4..].trim_start_matches('\n');
            }
            _ => {
                out.push_str(&rest[..at + LEAD.len()]);
                rest = after_lead;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heading(level: u32, id: &str, text: &str) -> HeadingOutline {
        HeadingOutline {
            level,
            id: id.into(),
            text: text.into(),
        }
    }

    #[test]
    fn table_of_contents_nests_headings() {
        let outline = vec![
            heading(1, "guide", "Guide"),
            heading(2, "start", "Start"),
            heading(3, "install", "Install <fast>"),
            heading(2, "use", "Use"),
        ];
        let html = insert_table_of_contents(
            "<h1 id=\"guide\">Guide</h1>\n<p>[TOC]</p>\n",
            &outline,
        );
        assert!(html.contains("<nav class=\"toc\""));
        assert!(!html.contains("#guide"), "the title is not listed: {html}");
        assert!(
            html.contains(
                "<li><a href=\"#start\">Start</a>\n<ul>\n<li><a href=\"#install\">Install &lt;fast&gt;</a></li>\n</ul>\n</li>\n<li><a href=\"#use\">Use</a></li>"
            ),
            "{html}"
        );
        assert_eq!(
            insert_table_of_contents("<p>no marker</p>", &outline),
            "<p>no marker</p>"
        );
    }

    #[test]
    fn image_attributes_after_images() {
        let html = r#"<p><img src="a.png" alt="A">{width=320 .wide #fig-1 style=&quot;border-radius:8px&quot;}</p>"#;
        assert_eq!(
            apply_image_attributes(html),
            r#"<p><img src="a.png" alt="A" width="320" id="fig-1" style="border-radius:8px" class="wide"></p>"#
        );
        assert_eq!(
            apply_image_attributes(r#"<img src="a.png" alt="">{see below}"#),
            r#"<img src="a.png" alt="">{see below}"#
        );
    }

    #[test]
    fn table_captions() {
        let html = "<div class=\"table-wrap\"><table>\n<thead></thead>\n</table></div>\n<p>Table: Sizes of <em>things</em></p>\n<p>Next</p>";
        assert_eq!(
            apply_table_captions(html),
            "<div class=\"table-wrap\"><table>\n<caption>Sizes of <em>things</em></caption>\n<thead></thead>\n</table></div>\n<p>Next</p>"
        );
        assert_eq!(
            apply_table_captions("<p>Table: not after a table</p>"),
            "<p>Table: not after a table</p>"
        );
    }
}
