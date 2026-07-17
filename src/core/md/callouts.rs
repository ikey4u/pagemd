use std::path::Path;

use anyhow::Result;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::core::md::footnotes::{ExtractedFootnote, FootnoteDisplay, FootnoteRegistry};
use crate::core::md::preprocess::callout_label;
use crate::core::md::render::render_markdown_with_depth;
use crate::core::util::html_escape;

pub struct CalloutRenderContext<'a> {
    pub base_dir: &'a Path,
    pub math_font_size: f64,
    pub font_dir: &'a str,
    pub ss: &'a SyntaxSet,
    pub ts: &'a ThemeSet,
    pub footnotes: &'a FootnoteRegistry,
    pub depth: usize,
    pub client_mermaid: bool,
    pub footnote_display: FootnoteDisplay,
    pub extracted_footnotes: &'a mut Vec<ExtractedFootnote>,
}

pub fn render_callout(
    kind: &str,
    title: &str,
    content: &str,
    ctx: &mut CalloutRenderContext<'_>,
) -> Result<String> {
    let body = if ctx.depth >= 8 {
        format!("<p>{}</p>\n", html_escape(content.trim()))
    } else {
        render_markdown_with_depth(
            content,
            ctx.base_dir,
            ctx.math_font_size,
            ctx.font_dir,
            ctx.ss,
            ctx.ts,
            Some(ctx.footnotes),
            ctx.depth + 1,
            ctx.client_mermaid,
            ctx.footnote_display,
            ctx.extracted_footnotes,
        )?
        .html
    };
    let title_text = if title.trim().is_empty() {
        callout_label(kind)
    } else {
        title.trim()
    };
    Ok(format!(
        "<div class=\"callout callout-{kind}\"><div class=\"callout-title\"><span>{}</span></div><div class=\"callout-body\">{}</div></div>\n",
        html_escape(title_text),
        body
    ))
}
