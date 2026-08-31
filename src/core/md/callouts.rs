use std::path::Path;

use anyhow::Result;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::core::md::footnotes::{ExtractedFootnote, FootnoteDisplay, FootnoteRegistry};
use crate::core::md::preprocess::{callout_label, FoldState};
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

fn render_nested_markdown(content: &str, ctx: &mut CalloutRenderContext<'_>) -> Result<String> {
    if ctx.depth >= 8 {
        Ok(format!("<p>{}</p>\n", html_escape(content.trim())))
    } else {
        Ok(render_markdown_with_depth(
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
        .html)
    }
}

fn chevron() -> &'static str {
    "<span class=\"md-fold-chevron\" aria-hidden=\"true\"></span>"
}

pub fn render_callout(
    kind: &str,
    title: &str,
    content: &str,
    fold: Option<FoldState>,
    ctx: &mut CalloutRenderContext<'_>,
) -> Result<String> {
    let body = render_nested_markdown(content, ctx)?;
    let title_text = if title.trim().is_empty() {
        callout_label(kind)
    } else {
        title.trim()
    };
    let title_html = html_escape(title_text);
    if let Some(fold) = fold {
        let open_attr = if matches!(fold, FoldState::Open) {
            " open"
        } else {
            ""
        };
        Ok(format!(
            "<details class=\"callout callout-{kind} callout-fold\"{open_attr}>\
<summary class=\"callout-title\">{chevron}<span>{title_html}</span></summary>\
<div class=\"callout-body\">{body}</div></details>\n",
            chevron = chevron(),
        ))
    } else {
        Ok(format!(
            "<div class=\"callout callout-{kind}\"><div class=\"callout-title\"><span>{title_html}</span></div><div class=\"callout-body\">{body}</div></div>\n"
        ))
    }
}

pub fn render_details(
    title: &str,
    content: &str,
    open: bool,
    ctx: &mut CalloutRenderContext<'_>,
) -> Result<String> {
    let body = render_nested_markdown(content, ctx)?;
    let title_text = if title.trim().is_empty() {
        "Details"
    } else {
        title.trim()
    };
    let open_attr = if open { " open" } else { "" };
    Ok(format!(
        "<details class=\"md-details\"{open_attr}>\
<summary class=\"md-details-summary\">{chevron}<span>{}</span></summary>\
<div class=\"md-details-body\">{body}</div></details>\n",
        html_escape(title_text),
        chevron = chevron(),
    ))
}
