use std::path::Path;

use anyhow::{Context, Result};
use pulldown_cmark::{Event, Options, Parser as MdParser, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;

use crate::core::export::html::bundler::{image_to_data_uri, inline_raw_html_resources};
use crate::core::ext::diagram::{
    is_diagram_html_info, mermaid_client_html, mermaid_error_html, plantuml_error_html,
    render_diagram_html, render_mermaid, render_plantuml,
};
use crate::core::ext::math::latex_to_svg;
use crate::core::ext::typst;
use crate::core::md::callouts::{render_callout, CalloutRenderContext};
use crate::core::md::footnotes::{
    footnote_def_html, footnote_ref_html, footnote_slot_labels, plain_footnote_title,
    sort_extracted_footnotes, split_footnote_text, ExtractedFootnote, FootnoteDisplay,
    FootnoteRegistry, FootnoteTextSegment,
};
use crate::core::md::preprocess::{parse_internal_callout_info, preprocess_markdown_extensions};
use crate::core::model::{HeadingOutline, RenderedSection};
use crate::core::util::unique_heading_id;
use crate::core::util::{eprint_fence_render_error, html_escape};

struct PendingImage {
    src: String,
    title_attr: String,
    alt_buf: String,
}

fn push_plain_text(buf: &mut String, event: &Event<'_>) {
    match event {
        Event::Text(text) => buf.push_str(text),
        Event::Code(code) => buf.push_str(code),
        Event::InlineMath(math) => buf.push_str(math),
        Event::DisplayMath(math) => buf.push_str(math),
        Event::SoftBreak | Event::HardBreak => buf.push(' '),
        _ => {}
    }
}

fn current_target<'a>(
    html: &'a mut String,
    paragraph_html: &'a mut Option<String>,
) -> &'a mut String {
    match paragraph_html {
        Some(buf) => buf,
        None => html,
    }
}

fn is_escaped_byte(text: &str, index: usize) -> bool {
    let bytes = text.as_bytes();
    let mut count = 0usize;
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        if bytes[cursor] == b'\\' {
            count += 1;
        } else {
            break;
        }
    }
    count % 2 == 1
}

fn contains_cjk_text_or_punctuation(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{2E80}'..='\u{2EFF}'
                | '\u{2F00}'..='\u{2FDF}'
                | '\u{3000}'..='\u{303F}'
                | '\u{3040}'..='\u{30FF}'
                | '\u{3100}'..='\u{312F}'
                | '\u{31A0}'..='\u{31BF}'
                | '\u{31F0}'..='\u{31FF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{AC00}'..='\u{D7AF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{FF00}'..='\u{FFEF}'
        )
    })
}

fn is_invalid_inline_math_candidate(expr: &str) -> bool {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return true;
    }
    if contains_cjk_text_or_punctuation(trimmed) {
        return true;
    }
    matches!(
        trimmed.chars().last(),
        Some(
            '+' | '-'
                | '–'
                | '—'
                | '−'
                | '='
                | '/'
                | '*'
                | ':'
                | ';'
                | ','
                | '→'
                | '←'
                | '↔'
                | '⇒'
                | '⇐'
                | '⇔'
        )
    )
}

fn footnote_title_attr(
    label: &str,
    footnotes: &FootnoteRegistry,
    display: FootnoteDisplay,
) -> Option<String> {
    if !matches!(display, FootnoteDisplay::Tooltip | FootnoteDisplay::Host) {
        return None;
    }
    footnotes
        .definition(label)
        .map(plain_footnote_title)
        .filter(|title| !title.is_empty())
}

fn push_extracted_footnote(extracted: &mut Vec<ExtractedFootnote>, label: &str, body: &str) {
    if extracted.iter().any(|f| f.label == label) {
        return;
    }
    extracted.push(ExtractedFootnote {
        label: label.to_string(),
        plain: plain_footnote_title(body),
        html: render_footnote_body_markup(body),
    });
}

fn append_rich_text(
    buf: &mut String,
    text: &str,
    math_font_size: f64,
    font_dir: &str,
    footnotes: &FootnoteRegistry,
    display: FootnoteDisplay,
) {
    for segment in split_footnote_text(text) {
        match segment {
            FootnoteTextSegment::Plain(plain) => {
                append_inline_math_html(buf, plain, math_font_size, font_dir);
            }
            FootnoteTextSegment::Reference(label) => {
                let title = footnote_title_attr(label, footnotes, display);
                buf.push_str(&footnote_ref_html(label, title.as_deref()));
            }
        }
    }
}

fn render_footnote_body_markup(body: &str) -> String {
    use pulldown_cmark::html;

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let parser = MdParser::new_ext(body.trim(), opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out.trim().to_string()
}

fn render_footnote_slot(
    label: &str,
    footnotes: &FootnoteRegistry,
    display: FootnoteDisplay,
    extracted: &mut Vec<ExtractedFootnote>,
) -> String {
    let Some(body) = footnotes.definition(label) else {
        return String::new();
    };
    match display {
        FootnoteDisplay::Host => {
            push_extracted_footnote(extracted, label, body);
            String::new()
        }
        // Tooltip mode: plain escaped text only — citation quotes may contain raw HTML.
        FootnoteDisplay::Tooltip => {
            let body_html = format!("<p>{}</p>", html_escape(&plain_footnote_title(body)));
            footnote_def_html(label, &body_html, display)
        }
        FootnoteDisplay::EndList => {
            let body_html = render_footnote_body_markup(body);
            footnote_def_html(label, &body_html, display)
        }
    }
}

fn try_render_footnote_html(
    raw: &str,
    footnotes: &FootnoteRegistry,
    display: FootnoteDisplay,
    extracted: &mut Vec<ExtractedFootnote>,
) -> Option<String> {
    let labels = footnote_slot_labels(raw);
    if labels.is_empty() {
        return None;
    }
    let mut out = String::new();
    for label in labels {
        out.push_str(&render_footnote_slot(&label, footnotes, display, extracted));
    }
    // Host always replaces the slot(s) (including with empty), so defs leave the body.
    if display == FootnoteDisplay::Host {
        return Some(out);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn append_inline_math_html(buf: &mut String, text: &str, math_font_size: f64, font_dir: &str) {
    let bytes = text.as_bytes();
    let mut plain_start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'$' && !is_escaped_byte(text, i) {
            let prev_is_dollar = i > 0 && bytes[i - 1] == b'$';
            let next_is_dollar = i + 1 < bytes.len() && bytes[i + 1] == b'$';
            if prev_is_dollar || next_is_dollar {
                i += 1;
                continue;
            }

            let mut matched: Option<(usize, String)> = None;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'$' && !is_escaped_byte(text, j) {
                    let prev_close_is_dollar = j > 0 && bytes[j - 1] == b'$';
                    let next_close_is_dollar = j + 1 < bytes.len() && bytes[j + 1] == b'$';
                    if !prev_close_is_dollar && !next_close_is_dollar {
                        let expr = text[i + 1..j].trim();
                        if !expr.is_empty()
                            && !expr.contains('\n')
                            && !is_invalid_inline_math_candidate(expr)
                        {
                            if let Ok(svg) = latex_to_svg(expr, false, math_font_size, font_dir) {
                                matched = Some((j, svg));
                                break;
                            }
                        }
                    }
                }
                j += 1;
            }

            if let Some((end, svg)) = matched {
                buf.push_str(&html_escape(&text[plain_start..i]));
                buf.push_str("<span class=\"math-inline\">");
                buf.push_str(&svg);
                buf.push_str("</span>");
                plain_start = end + 1;
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }

    buf.push_str(&html_escape(&text[plain_start..]));
}

fn render_display_math_paragraph(
    plain: &str,
    math_font_size: f64,
    font_dir: &str,
) -> Option<String> {
    let trimmed = plain.trim();
    if trimmed.len() < 4 || !trimmed.starts_with("$$") || !trimmed.ends_with("$$") {
        return None;
    }

    let inner = trimmed[2..trimmed.len() - 2].trim();
    if inner.is_empty() || inner.contains("$$") {
        return None;
    }

    let svg = latex_to_svg(inner, true, math_font_size, font_dir).ok()?;
    Some(format!("<div class=\"math-display\">{svg}</div>"))
}

pub fn render_markdown(
    source: &str,
    base_dir: &Path,
    math_font_size: f64,
    font_dir: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
    client_mermaid: bool,
    footnote_display: FootnoteDisplay,
) -> Result<RenderedSection> {
    let mut extracted = Vec::new();
    let mut section = render_markdown_with_depth(
        source,
        base_dir,
        math_font_size,
        font_dir,
        ss,
        ts,
        None,
        0,
        client_mermaid,
        footnote_display,
        &mut extracted,
    )?;
    if footnote_display == FootnoteDisplay::Host {
        sort_extracted_footnotes(&mut extracted);
        section.footnotes = extracted;
    }
    Ok(section)
}

pub fn render_markdown_with_depth(
    source: &str,
    base_dir: &Path,
    math_font_size: f64,
    font_dir: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
    footnotes: Option<&FootnoteRegistry>,
    depth: usize,
    client_mermaid: bool,
    footnote_display: FootnoteDisplay,
    extracted_footnotes: &mut Vec<ExtractedFootnote>,
) -> Result<RenderedSection> {
    let preprocessed = preprocess_markdown_extensions(source);
    let owned_registry;
    let footnotes = match footnotes {
        Some(registry) => registry,
        None => {
            owned_registry = FootnoteRegistry::from_markdown(&preprocessed);
            &owned_registry
        }
    };
    let mut parse_source = preprocessed;
    footnotes.prepare_parse_unit(&mut parse_source);

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = MdParser::new_ext(&parse_source, opts);
    // Strip ZWSP (U+200B) injected by fix_emphasis_cjk_punctuation from all text content.
    // The ZWSP was only needed to satisfy pulldown-cmark's flanking rules during parsing;
    // it must not appear in final output.
    let events: Vec<Event> = parser
        .map(|event| match event {
            Event::Text(ref t) if t.contains('\u{200B}') => {
                Event::Text(t.replace('\u{200B}', "").into())
            }
            Event::Code(ref t) if t.contains('\u{200B}') => {
                Event::Code(t.replace('\u{200B}', "").into())
            }
            Event::InlineMath(ref t) if t.contains('\u{200B}') => {
                Event::InlineMath(t.replace('\u{200B}', "").into())
            }
            Event::DisplayMath(ref t) if t.contains('\u{200B}') => {
                Event::DisplayMath(t.replace('\u{200B}', "").into())
            }
            other => other,
        })
        .collect();

    let mut html = String::new();
    let mut title = String::new();
    let mut outline: Vec<HeadingOutline> = Vec::new();
    let mut heading_ids = std::collections::HashMap::new();
    let mut first_heading = true;

    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next())
        .context("No theme found")?;

    enum Context {
        Normal,
        CodeBlock {
            lang: String,
            buf: String,
        },
        Heading {
            level: u32,
            /// Escaped HTML content for the `<hN>` body.
            html: String,
            /// Unescaped visible text for outline / title / heading ids.
            /// Never derived from HTML — updated from markdown events in parallel.
            plain: String,
            image: Option<PendingImage>,
        },
        Image(PendingImage),
    }

    let mut ctx = Context::Normal;
    let mut in_table_head = false;
    let mut table_alignments: Vec<pulldown_cmark::Alignment> = Vec::new();
    let mut table_col_index: usize = 0;
    let mut paragraph_html: Option<String> = None;
    let mut paragraph_plain: Option<String> = None;
    let mut paragraph_is_plain = true;
    let mut blockquote_depth = 0usize;
    let mut skip_footnote_definition_depth = 0usize;

    for event in &events {
        if skip_footnote_definition_depth > 0 {
            match event {
                Event::Start(Tag::FootnoteDefinition(_)) => skip_footnote_definition_depth += 1,
                Event::End(TagEnd::FootnoteDefinition) => {
                    skip_footnote_definition_depth -= 1;
                }
                _ => {}
            }
            continue;
        }

        match &mut ctx {
            Context::CodeBlock { lang, buf } => match event {
                Event::Text(text) => buf.push_str(text),
                Event::End(TagEnd::CodeBlock) => {
                    let lang_info = lang.trim().to_string();
                    let lang_str = lang_info
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let buf_str = buf.clone();
                    ctx = Context::Normal;
                    match lang_str.as_str() {
                        "diagram" | "diagram-html" | "diagram_html"
                            if is_diagram_html_info(&lang_info) =>
                        {
                            html.push_str(&render_diagram_html(&buf_str, base_dir));
                        }
                        "math" | "latex" => {
                            match latex_to_svg(buf_str.trim(), true, math_font_size, font_dir) {
                                Ok(svg) => {
                                    html.push_str("<div class=\"math-display\">");
                                    html.push_str(&svg);
                                    html.push_str("</div>\n");
                                }
                                Err(_) => {
                                    html.push_str("<pre class=\"math-error\"><code>");
                                    html.push_str(&html_escape(&buf_str));
                                    html.push_str("</code></pre>\n");
                                }
                            }
                        }
                        "mermaid" | "mmd" => {
                            if client_mermaid {
                                html.push_str(&mermaid_client_html(&buf_str));
                            } else {
                                match render_mermaid(&buf_str) {
                                    Ok(rendered) => html.push_str(&rendered),
                                    Err(err) => {
                                        eprint_fence_render_error("Mermaid", &err, &buf_str);
                                        html.push_str(&mermaid_error_html(&buf_str));
                                    }
                                }
                            }
                        }
                        "plantuml" | "puml" | "uml" => match render_plantuml(&buf_str) {
                            Ok(rendered) => html.push_str(&rendered),
                            Err(err) => {
                                eprint_fence_render_error("PlantUML", &err, &buf_str);
                                html.push_str(&plantuml_error_html(&buf_str));
                            }
                        },
                        "typst" => match typst::render_typst(&buf_str) {
                            Ok(rendered) => html.push_str(&rendered),
                            Err(err) => {
                                eprint_fence_render_error("Typst", &err, &buf_str);
                                html.push_str(&typst::typst_error_html(&buf_str));
                            }
                        },
                        "pagemd-callout" => {
                            if let Some((kind, title)) = parse_internal_callout_info(&lang_info) {
                                match render_callout(
                                    &kind,
                                    &title,
                                    &buf_str,
                                    &mut CalloutRenderContext {
                                        base_dir,
                                        math_font_size,
                                        font_dir,
                                        ss,
                                        ts,
                                        footnotes,
                                        depth,
                                        client_mermaid,
                                        footnote_display,
                                        extracted_footnotes,
                                    },
                                ) {
                                    Ok(rendered) => html.push_str(&rendered),
                                    Err(_) => html
                                        .push_str(&highlight_code(&buf_str, &lang_str, ss, theme)),
                                }
                            } else {
                                html.push_str(&highlight_code(&buf_str, &lang_str, ss, theme));
                            }
                        }
                        _ => {
                            let highlighted = if lang_str.is_empty() {
                                format!("<pre><code>{}</code></pre>\n", html_escape(&buf_str))
                            } else {
                                highlight_code(&buf_str, &lang_str, ss, theme)
                            };
                            html.push_str(&highlighted);
                        }
                    }
                }
                _ => {}
            },

            Context::Heading {
                level,
                html: heading_html,
                plain: heading_plain,
                image,
            } => {
                if let Some(pending) = image {
                    match event {
                        Event::End(TagEnd::Image) => {
                            heading_plain.push_str(&pending.alt_buf);
                            let alt = html_escape(&pending.alt_buf);
                            heading_html.push_str(&format!(
                                "<img src=\"{}\" alt=\"{}\"{}>",
                                pending.src.as_str(),
                                alt,
                                pending.title_attr.as_str()
                            ));
                            *image = None;
                        }
                        _ => push_plain_text(&mut pending.alt_buf, event),
                    }
                } else {
                    match event {
                        Event::Text(text) => {
                            heading_plain.push_str(text);
                            append_rich_text(
                                heading_html,
                                text,
                                math_font_size,
                                font_dir,
                                footnotes,
                                footnote_display,
                            );
                        }
                        Event::Code(code) => {
                            heading_plain.push_str(code);
                            heading_html.push_str("<code>");
                            heading_html.push_str(&html_escape(code));
                            heading_html.push_str("</code>");
                        }
                        Event::Start(Tag::Emphasis) => heading_html.push_str("<em>"),
                        Event::End(TagEnd::Emphasis) => heading_html.push_str("</em>"),
                        Event::Start(Tag::Strong) => heading_html.push_str("<strong>"),
                        Event::End(TagEnd::Strong) => heading_html.push_str("</strong>"),
                        Event::Start(Tag::Strikethrough) => heading_html.push_str("<del>"),
                        Event::End(TagEnd::Strikethrough) => heading_html.push_str("</del>"),
                        Event::Start(Tag::Link {
                            dest_url,
                            title: link_title,
                            ..
                        }) => {
                            let title_attr = if link_title.is_empty() {
                                String::new()
                            } else {
                                format!(" title=\"{}\"", html_escape(link_title))
                            };
                            heading_html.push_str(&format!(
                                "<a href=\"{}\"{title_attr}>",
                                html_escape(dest_url)
                            ));
                        }
                        Event::End(TagEnd::Link) => heading_html.push_str("</a>"),
                        Event::Start(Tag::Image {
                            dest_url,
                            title: img_title,
                            ..
                        }) => {
                            let src = image_to_data_uri(dest_url, base_dir);
                            let title_attr = if img_title.is_empty() {
                                String::new()
                            } else {
                                format!(" title=\"{}\"", html_escape(img_title))
                            };
                            *image = Some(PendingImage {
                                src: html_escape(&src),
                                title_attr,
                                alt_buf: String::new(),
                            });
                        }
                        Event::InlineMath(math) => {
                            heading_plain.push_str(math);
                            if let Ok(svg) = latex_to_svg(math, false, math_font_size, font_dir) {
                                heading_html.push_str("<span class=\"math-inline\">");
                                heading_html.push_str(&svg);
                                heading_html.push_str("</span>");
                            }
                        }
                        Event::FootnoteReference(label) => {
                            heading_plain.push_str(label);
                            let title = footnote_title_attr(label, footnotes, footnote_display);
                            heading_html.push_str(&footnote_ref_html(label, title.as_deref()));
                        }
                        Event::Html(raw) => {
                            heading_html.push_str(&inline_raw_html_resources(raw, base_dir))
                        }
                        Event::InlineHtml(raw) => {
                            heading_html.push_str(&inline_raw_html_resources(raw, base_dir))
                        }
                        Event::SoftBreak | Event::HardBreak => {
                            heading_plain.push(' ');
                            heading_html.push(' ');
                        }
                        Event::End(TagEnd::Heading(_)) => {
                            let lvl = *level;
                            let plain = std::mem::take(heading_plain);
                            let body = std::mem::take(heading_html);
                            let id = unique_heading_id(&plain, &mut heading_ids);
                            if first_heading && lvl == 1 {
                                title = plain.clone();
                                first_heading = false;
                            }
                            outline.push(HeadingOutline {
                                level: lvl,
                                id: id.clone(),
                                text: plain,
                            });
                            html.push_str(&format!("<h{lvl} id=\"{id}\">{body}</h{lvl}>\n"));
                            ctx = Context::Normal;
                        }
                        _ => {}
                    }
                }
            }

            Context::Image(pending) => match event {
                Event::End(TagEnd::Image) => {
                    let alt = html_escape(&pending.alt_buf);
                    current_target(&mut html, &mut paragraph_html).push_str(&format!(
                        "<img src=\"{}\" alt=\"{}\"{}>",
                        pending.src.as_str(),
                        alt,
                        pending.title_attr.as_str()
                    ));
                    ctx = Context::Normal;
                }
                _ => push_plain_text(&mut pending.alt_buf, event),
            },

            Context::Normal => match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    let lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                    ctx = Context::CodeBlock {
                        lang,
                        buf: String::new(),
                    };
                }

                Event::Start(Tag::Heading { level, .. }) => {
                    ctx = Context::Heading {
                        level: *level as u32,
                        html: String::new(),
                        plain: String::new(),
                        image: None,
                    };
                }

                Event::Start(Tag::Image {
                    dest_url,
                    title: img_title,
                    ..
                }) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    let src = image_to_data_uri(dest_url, base_dir);
                    let title_attr = if img_title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", html_escape(img_title))
                    };
                    ctx = Context::Image(PendingImage {
                        src: html_escape(&src),
                        title_attr,
                        alt_buf: String::new(),
                    });
                }

                Event::InlineMath(math) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    let target = current_target(&mut html, &mut paragraph_html);
                    match latex_to_svg(math, false, math_font_size, font_dir) {
                        Ok(svg) => {
                            target.push_str("<span class=\"math-inline\">");
                            target.push_str(&svg);
                            target.push_str("</span>");
                        }
                        Err(_) => {
                            target.push_str("<code class=\"math-error\">");
                            target.push_str(&html_escape(math));
                            target.push_str("</code>");
                        }
                    }
                }
                Event::DisplayMath(math) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    let target = current_target(&mut html, &mut paragraph_html);
                    match latex_to_svg(math, true, math_font_size, font_dir) {
                        Ok(svg) => {
                            target.push_str("<div class=\"math-display\">");
                            target.push_str(&svg);
                            target.push_str("</div>\n");
                        }
                        Err(_) => {
                            target.push_str("<div class=\"math-error\"><code>");
                            target.push_str(&html_escape(math));
                            target.push_str("</code></div>\n");
                        }
                    }
                }

                Event::Start(Tag::Link {
                    dest_url,
                    title: link_title,
                    ..
                }) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    let title_attr = if link_title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", html_escape(link_title))
                    };
                    current_target(&mut html, &mut paragraph_html).push_str(&format!(
                        "<a href=\"{}\"{title_attr}>",
                        html_escape(dest_url)
                    ));
                }
                Event::End(TagEnd::Link) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("</a>");
                }

                Event::Html(raw) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    if let Some(rendered) = try_render_footnote_html(
                        raw,
                        footnotes,
                        footnote_display,
                        extracted_footnotes,
                    ) {
                        current_target(&mut html, &mut paragraph_html).push_str(&rendered);
                    } else {
                        current_target(&mut html, &mut paragraph_html)
                            .push_str(&inline_raw_html_resources(raw, base_dir));
                    }
                }
                Event::InlineHtml(raw) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    if let Some(rendered) = try_render_footnote_html(
                        raw,
                        footnotes,
                        footnote_display,
                        extracted_footnotes,
                    ) {
                        current_target(&mut html, &mut paragraph_html).push_str(&rendered);
                    } else {
                        current_target(&mut html, &mut paragraph_html)
                            .push_str(&inline_raw_html_resources(raw, base_dir));
                    }
                }

                Event::Start(Tag::Paragraph) => {
                    paragraph_html = Some(String::new());
                    paragraph_plain = Some(String::new());
                    paragraph_is_plain = true;
                }
                Event::End(TagEnd::Paragraph) => {
                    let rendered = paragraph_html.take().unwrap_or_default();
                    let plain = paragraph_plain.take().unwrap_or_default();
                    if paragraph_is_plain {
                        if let Some(display_html) =
                            render_display_math_paragraph(&plain, math_font_size, font_dir)
                        {
                            html.push_str(&display_html);
                            html.push('\n');
                        } else {
                            html.push_str("<p>");
                            html.push_str(&rendered);
                            html.push_str("</p>\n");
                        }
                    } else {
                        html.push_str("<p>");
                        html.push_str(&rendered);
                        html.push_str("</p>\n");
                    }
                    paragraph_is_plain = true;
                }

                Event::Start(Tag::BlockQuote(_)) => {
                    blockquote_depth += 1;
                    html.push_str("<blockquote>\n");
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    blockquote_depth = blockquote_depth.saturating_sub(1);
                    html.push_str("</blockquote>\n");
                }

                Event::Start(Tag::List(None)) => html.push_str("<ul>\n"),
                Event::End(TagEnd::List(false)) => html.push_str("</ul>\n"),
                Event::Start(Tag::List(Some(start))) => {
                    if *start == 1 {
                        html.push_str("<ol>\n");
                    } else {
                        html.push_str(&format!("<ol start=\"{start}\">\n"));
                    }
                }
                Event::End(TagEnd::List(true)) => html.push_str("</ol>\n"),

                Event::Start(Tag::Item) => html.push_str("<li>"),
                Event::End(TagEnd::Item) => html.push_str("</li>\n"),

                Event::Start(Tag::Table(aligns)) => {
                    table_alignments = aligns.to_vec();
                    table_col_index = 0;
                    html.push_str("<div class=\"table-wrap\"><table>\n");
                }
                Event::End(TagEnd::Table) => {
                    table_alignments.clear();
                    html.push_str("</table></div>\n");
                }
                Event::Start(Tag::TableHead) => {
                    in_table_head = true;
                    html.push_str("<thead>\n");
                }
                Event::End(TagEnd::TableHead) => {
                    in_table_head = false;
                    html.push_str("</thead>\n");
                }
                Event::Start(Tag::TableRow) => {
                    table_col_index = 0;
                    html.push_str("<tr>\n");
                }
                Event::End(TagEnd::TableRow) => html.push_str("</tr>\n"),
                Event::Start(Tag::TableCell) => {
                    let align_class = table_alignments
                        .get(table_col_index)
                        .map(|a| match a {
                            pulldown_cmark::Alignment::Left => " class=\"left\"",
                            pulldown_cmark::Alignment::Right => " class=\"right\"",
                            pulldown_cmark::Alignment::Center => " class=\"center\"",
                            pulldown_cmark::Alignment::None => "",
                        })
                        .unwrap_or("");
                    if in_table_head {
                        html.push_str(&format!("<th{align_class}>"));
                    } else {
                        html.push_str(&format!("<td{align_class}>"));
                    }
                }
                Event::End(TagEnd::TableCell) => {
                    if in_table_head {
                        html.push_str("</th>\n");
                    } else {
                        html.push_str("</td>\n");
                    }
                    table_col_index += 1;
                }

                Event::Start(Tag::Emphasis) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("<em>");
                }
                Event::End(TagEnd::Emphasis) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("</em>");
                }
                Event::Start(Tag::Strong) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("<strong>");
                }
                Event::End(TagEnd::Strong) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("</strong>");
                }
                Event::Start(Tag::Strikethrough) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("<del>");
                }
                Event::End(TagEnd::Strikethrough) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str("</del>");
                }

                Event::Code(code) => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push_str(code);
                        paragraph_is_plain = false;
                    }
                    let target = current_target(&mut html, &mut paragraph_html);
                    target.push_str("<code>");
                    target.push_str(&html_escape(code));
                    target.push_str("</code>");
                }

                Event::Text(text) => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push_str(text);
                    }
                    append_rich_text(
                        current_target(&mut html, &mut paragraph_html),
                        text,
                        math_font_size,
                        font_dir,
                        footnotes,
                        footnote_display,
                    );
                }

                Event::SoftBreak => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push('\n');
                    }
                    // CommonMark soft breaks collapse to spaces in HTML; inside
                    // blockquotes consecutive `>` lines are almost always meant
                    // as visible line breaks (same expectation as GFM breaks).
                    if blockquote_depth > 0 {
                        if paragraph_html.is_some() {
                            paragraph_is_plain = false;
                        }
                        current_target(&mut html, &mut paragraph_html).push_str("<br>\n");
                    } else if paragraph_html.is_some() {
                        current_target(&mut html, &mut paragraph_html).push('\n');
                    } else {
                        html.push('\n');
                    }
                }
                Event::HardBreak => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push('\n');
                        paragraph_is_plain = false;
                        current_target(&mut html, &mut paragraph_html).push_str("<br>\n");
                    } else {
                        html.push_str("<br>\n");
                    }
                }
                Event::Rule => html.push_str("<hr>\n"),

                Event::TaskListMarker(checked) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    let target = current_target(&mut html, &mut paragraph_html);
                    if *checked {
                        target.push_str("<input type=\"checkbox\" checked disabled> ");
                    } else {
                        target.push_str("<input type=\"checkbox\" disabled> ");
                    }
                }

                Event::FootnoteReference(label) => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push_str(label);
                        paragraph_is_plain = false;
                    }
                    let title = footnote_title_attr(label, footnotes, footnote_display);
                    current_target(&mut html, &mut paragraph_html)
                        .push_str(&footnote_ref_html(label, title.as_deref()));
                }

                Event::Start(Tag::FootnoteDefinition(_)) => {
                    skip_footnote_definition_depth += 1;
                }

                _ => {}
            },
        }
    }

    Ok(RenderedSection {
        title,
        html,
        outline,
        // Nested callouts share `extracted_footnotes`; only the top-level
        // `render_markdown` assigns the final list onto the section.
        footnotes: Vec::new(),
    })
}

fn highlight_code(
    code: &str,
    lang: &str,
    ss: &SyntaxSet,
    theme: &syntect::highlighting::Theme,
) -> String {
    let syntax = ss
        .find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_extension(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut hl = HighlightLines::new(syntax, theme);
    let mut out = String::from("<pre><code class=\"language-");
    out.push_str(&html_escape(lang));
    out.push_str("\">");

    for line in syntect::util::LinesWithEndings::from(code) {
        match hl.highlight_line(line, ss) {
            Ok(ranges) => {
                match styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No) {
                    Ok(html_line) => out.push_str(&html_line),
                    Err(_) => out.push_str(&html_escape(line)),
                }
            }
            Err(_) => out.push_str(&html_escape(line)),
        }
    }

    out.push_str("</code></pre>\n");
    out
}
