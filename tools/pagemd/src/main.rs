use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use clap::Parser;
use pulldown_cmark::{Event, Options, Parser as MdParser, Tag, TagEnd};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse as parse_latex;
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::math_style::MathStyle;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;

#[derive(Parser, Debug)]
#[command(
    name = "pagemd",
    about = "Convert Markdown to a self-contained single HTML file",
    long_about = None,
)]
struct Cli {
    #[arg(short = 'i', long = "input", value_name = "FILE", num_args = 1.., required = true)]
    inputs: Vec<PathBuf>,

    #[arg(short = 'o', long = "output", value_name = "FILE")]
    output: PathBuf,

    #[arg(long = "title", value_name = "TITLE")]
    title: Option<String>,

    #[arg(long = "font-size", default_value = "16")]
    math_font_size: f64,

    #[arg(long = "katex-fonts", value_name = "DIR", help = "Directory containing KaTeX .ttf font files for glyph embedding")]
    katex_fonts: Option<PathBuf>,
}

fn find_katex_fonts(hint: Option<&Path>) -> Option<String> {
    if let Some(p) = hint {
        if p.join("KaTeX_Main-Regular.ttf").exists() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    let exe_dir = std::env::current_exe().ok().and_then(|p| {
        p.parent().map(|d| d.to_path_buf())
    });
    let candidates: Vec<PathBuf> = [
        "node_modules/katex/dist/fonts",
        "../node_modules/katex/dist/fonts",
        "../../node_modules/katex/dist/fonts",
    ]
    .iter()
    .map(PathBuf::from)
    .chain(exe_dir.iter().flat_map(|d| {
        [
            d.join("node_modules/katex/dist/fonts"),
            d.join("../node_modules/katex/dist/fonts"),
        ]
    }))
    .collect();

    for c in &candidates {
        if c.join("KaTeX_Main-Regular.ttf").exists() {
            return Some(c.to_string_lossy().into_owned());
        }
    }
    None
}

fn latex_to_svg(expr: &str, display: bool, font_size: f64, font_dir: &str) -> Result<String> {
    let ast = parse_latex(expr).map_err(|e| anyhow::anyhow!("LaTeX parse error: {}", e))?;
    let style = if display { MathStyle::Display } else { MathStyle::Text };
    let opts = LayoutOptions {
        style,
        ..LayoutOptions::default()
    };
    let lbox = layout(&ast, &opts);
    let dl = to_display_list(&lbox);
    let embed = !font_dir.is_empty();
    let svg_opts = SvgOptions {
        font_size: font_size * 2.5,
        padding: 2.0,
        stroke_width: 1.5,
        embed_glyphs: embed,
        font_dir: font_dir.to_owned(),
    };
    Ok(render_to_svg(&dl, &svg_opts))
}

fn image_to_data_uri(src: &str, base_dir: &Path) -> String {
    let path = if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_string();
    } else if src.starts_with('/') {
        PathBuf::from(src)
    } else {
        base_dir.join(src)
    };

    match std::fs::read(&path) {
        Ok(data) => {
            let mime = mime_from_ext(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
            format!("data:{};base64,{}", mime, B64.encode(&data))
        }
        Err(_) => src.to_string(),
    }
}

fn mime_from_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

struct RenderedSection {
    title: String,
    html: String,
}

fn render_markdown(
    source: &str,
    base_dir: &Path,
    math_font_size: f64,
    font_dir: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
) -> Result<RenderedSection> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_MATH);

    let parser = MdParser::new_ext(source, opts);
    let events: Vec<Event> = parser.collect();

    let mut html = String::new();
    let mut title = String::new();
    let mut first_heading = true;

    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next())
        .context("No theme found")?;

    #[derive(PartialEq)]
    enum Context {
        Normal,
        CodeBlock { lang: String, buf: String },
        Heading { level: u32, buf: String },
        Image { src: String, title_attr: String, alt_buf: String },
    }

    let mut ctx = Context::Normal;
    let mut in_table_head = false;
    let mut table_alignments: Vec<pulldown_cmark::Alignment> = Vec::new();
    let mut table_col_index: usize = 0;

    for event in &events {
        match &mut ctx {
            Context::CodeBlock { lang, buf } => match event {
                Event::Text(text) => buf.push_str(text),
                Event::End(TagEnd::CodeBlock) => {
                    let lang_str = lang.trim().to_owned();
                    let buf_str = buf.clone();
                    ctx = Context::Normal;
                    match lang_str.as_str() {
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

            Context::Heading { level, buf } => match event {
                Event::Text(text) => buf.push_str(text),
                Event::Code(code) => {
                    buf.push_str("<code>");
                    buf.push_str(&html_escape(code));
                    buf.push_str("</code>");
                }
                Event::Start(Tag::Emphasis) => buf.push_str("<em>"),
                Event::End(TagEnd::Emphasis) => buf.push_str("</em>"),
                Event::Start(Tag::Strong) => buf.push_str("<strong>"),
                Event::End(TagEnd::Strong) => buf.push_str("</strong>"),
                Event::Start(Tag::Strikethrough) => buf.push_str("<del>"),
                Event::End(TagEnd::Strikethrough) => buf.push_str("</del>"),
                Event::Start(Tag::Link { dest_url, title: link_title, .. }) => {
                    let title_attr = if link_title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", html_escape(link_title))
                    };
                    buf.push_str(&format!("<a href=\"{dest_url}\"{title_attr}>"));
                }
                Event::End(TagEnd::Link) => buf.push_str("</a>"),
                Event::Start(Tag::Image { dest_url, title: img_title, .. }) => {
                    let src = image_to_data_uri(dest_url, base_dir);
                    let title_attr = if img_title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", html_escape(img_title))
                    };
                    buf.push_str(&format!("<img src=\"{src}\"{title_attr} alt=\""));
                }
                Event::End(TagEnd::Image) => buf.push_str("\">"),
                Event::InlineMath(math) => {
                    if let Ok(svg) = latex_to_svg(math, false, math_font_size, font_dir) {
                        buf.push_str("<span class=\"math-inline\">");
                        buf.push_str(&svg);
                        buf.push_str("</span>");
                    }
                }
                Event::SoftBreak => buf.push(' '),
                Event::HardBreak => buf.push(' '),
                Event::End(TagEnd::Heading(_)) => {
                    let lvl = *level;
                    let id = slugify(&strip_html_tags(buf));
                    if first_heading && lvl == 1 {
                        title = strip_html_tags(buf);
                        first_heading = false;
                    }
                    html.push_str(&format!("<h{lvl} id=\"{id}\">{buf}</h{lvl}>\n"));
                    ctx = Context::Normal;
                }
                _ => {}
            },

            Context::Image { src, title_attr, alt_buf } => match event {
                Event::Text(text) => alt_buf.push_str(text),
                Event::End(TagEnd::Image) => {
                    let alt = html_escape(alt_buf);
                    html.push_str(&format!("<img src=\"{src}\" alt=\"{alt}\"{title_attr}>"));
                    ctx = Context::Normal;
                }
                _ => {}
            },

            Context::Normal => match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    let lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                    ctx = Context::CodeBlock { lang, buf: String::new() };
                }

                Event::Start(Tag::Heading { level, .. }) => {
                    ctx = Context::Heading { level: *level as u32, buf: String::new() };
                }

                Event::Start(Tag::Image { dest_url, title: img_title, .. }) => {
                    let src = image_to_data_uri(dest_url, base_dir);
                    let title_attr = if img_title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", html_escape(img_title))
                    };
                    ctx = Context::Image { src, title_attr, alt_buf: String::new() };
                }

                Event::InlineMath(math) => {
                    match latex_to_svg(math, false, math_font_size, font_dir) {
                        Ok(svg) => {
                            html.push_str("<span class=\"math-inline\">");
                            html.push_str(&svg);
                            html.push_str("</span>");
                        }
                        Err(_) => {
                            html.push_str("<code class=\"math-error\">");
                            html.push_str(&html_escape(math));
                            html.push_str("</code>");
                        }
                    }
                }
                Event::DisplayMath(math) => {
                    match latex_to_svg(math, true, math_font_size, font_dir) {
                        Ok(svg) => {
                            html.push_str("<div class=\"math-display\">");
                            html.push_str(&svg);
                            html.push_str("</div>\n");
                        }
                        Err(_) => {
                            html.push_str("<div class=\"math-error\"><code>");
                            html.push_str(&html_escape(math));
                            html.push_str("</code></div>\n");
                        }
                    }
                }

                Event::Start(Tag::Link { dest_url, title: link_title, .. }) => {
                    let title_attr = if link_title.is_empty() {
                        String::new()
                    } else {
                        format!(" title=\"{}\"", html_escape(link_title))
                    };
                    html.push_str(&format!("<a href=\"{dest_url}\"{title_attr}>"));
                }
                Event::End(TagEnd::Link) => html.push_str("</a>"),

                Event::Html(raw) => html.push_str(raw),
                Event::InlineHtml(raw) => html.push_str(raw),

                Event::Start(Tag::Paragraph) => html.push_str("<p>"),
                Event::End(TagEnd::Paragraph) => html.push_str("</p>\n"),

                Event::Start(Tag::BlockQuote(_)) => html.push_str("<blockquote>\n"),
                Event::End(TagEnd::BlockQuote(_)) => html.push_str("</blockquote>\n"),

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

                Event::Start(Tag::Emphasis) => html.push_str("<em>"),
                Event::End(TagEnd::Emphasis) => html.push_str("</em>"),
                Event::Start(Tag::Strong) => html.push_str("<strong>"),
                Event::End(TagEnd::Strong) => html.push_str("</strong>"),
                Event::Start(Tag::Strikethrough) => html.push_str("<del>"),
                Event::End(TagEnd::Strikethrough) => html.push_str("</del>"),

                Event::Code(code) => {
                    html.push_str("<code>");
                    html.push_str(&html_escape(code));
                    html.push_str("</code>");
                }

                Event::Text(text) => html.push_str(&html_escape(text)),

                Event::SoftBreak => html.push('\n'),
                Event::HardBreak => html.push_str("<br>\n"),
                Event::Rule => html.push_str("<hr>\n"),

                Event::TaskListMarker(checked) => {
                    if *checked {
                        html.push_str("<input type=\"checkbox\" checked disabled> ");
                    } else {
                        html.push_str("<input type=\"checkbox\" disabled> ");
                    }
                }

                Event::Start(Tag::FootnoteDefinition(label)) => {
                    html.push_str(&format!(
                        "<div class=\"footnote\" id=\"fn-{}\"><sup>{}</sup> ",
                        html_escape(label),
                        html_escape(label)
                    ));
                }
                Event::End(TagEnd::FootnoteDefinition) => html.push_str("</div>\n"),
                Event::FootnoteReference(label) => {
                    html.push_str(&format!(
                        "<sup><a href=\"#fn-{}\">{}</a></sup>",
                        html_escape(label),
                        html_escape(label)
                    ));
                }

                _ => {}
            },
        }
    }

    Ok(RenderedSection { title, html })
}

fn highlight_code(code: &str, lang: &str, ss: &SyntaxSet, theme: &syntect::highlighting::Theme) -> String {
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn build_html(title: &str, body_sections: &[RenderedSection]) -> String {
    let body_html: String = if body_sections.len() == 1 {
        body_sections[0].html.clone()
    } else {
        body_sections
            .iter()
            .map(|sec| {
                format!(
                    "<section class=\"doc-section\">\n{}</section>\n",
                    sec.html
                )
            })
            .collect()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
{css}
</style>
</head>
<body>
<div class="container">
{body_html}
</div>
</body>
</html>"#,
        title = html_escape(title),
        css = CSS,
        body_html = body_html,
    )
}

const CSS: &str = r#"
*, *::before, *::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

:root {
  --color-bg: #ffffff;
  --color-text: #1a1a2e;
  --color-muted: #6b7280;
  --color-border: #e5e7eb;
  --color-code-bg: #f3f4f6;
  --color-blockquote-border: #3b82f6;
  --color-blockquote-bg: #eff6ff;
  --color-link: #2563eb;
  --color-link-hover: #1d4ed8;
  --color-table-header: #f9fafb;
  --color-table-row-alt: #f9fafb;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: "JetBrains Mono", "Fira Code", "Cascadia Code", Consolas, "Liberation Mono", monospace;
  --radius: 6px;
  --shadow-sm: 0 1px 3px rgba(0,0,0,0.08);
}

html {
  font-size: 16px;
  -webkit-text-size-adjust: 100%;
}

body {
  font-family: var(--font-sans);
  font-size: 1rem;
  line-height: 1.75;
  color: var(--color-text);
  background: var(--color-bg);
}

.container {
  max-width: 860px;
  margin: 0 auto;
  padding: 3rem 2rem 5rem;
}

.doc-section + .doc-section {
  margin-top: 4rem;
  padding-top: 3rem;
  border-top: 2px solid var(--color-border);
}

h1, h2, h3, h4, h5, h6 {
  font-weight: 700;
  line-height: 1.3;
  margin-top: 2rem;
  margin-bottom: 0.75rem;
  color: #0f172a;
}

h1 { font-size: 2.25rem; margin-top: 0; border-bottom: 2px solid var(--color-border); padding-bottom: 0.5rem; }
h2 { font-size: 1.5rem; border-bottom: 1px solid var(--color-border); padding-bottom: 0.35rem; }
h3 { font-size: 1.25rem; }
h4 { font-size: 1.1rem; }
h5 { font-size: 1rem; }
h6 { font-size: 0.9rem; color: var(--color-muted); }

p {
  margin-bottom: 1rem;
}

a {
  color: var(--color-link);
  text-decoration: none;
}
a:hover {
  color: var(--color-link-hover);
  text-decoration: underline;
}

strong { font-weight: 700; }
em { font-style: italic; }
del { text-decoration: line-through; color: var(--color-muted); }

code {
  font-family: var(--font-mono);
  font-size: 0.875em;
  background: var(--color-code-bg);
  border: 1px solid var(--color-border);
  border-radius: 3px;
  padding: 0.15em 0.4em;
}

pre {
  background: #1e2030;
  color: #c8d3f5;
  border-radius: var(--radius);
  padding: 1.25rem 1.5rem;
  overflow-x: auto;
  margin: 1.25rem 0;
  font-size: 0.875rem;
  line-height: 1.6;
  box-shadow: var(--shadow-sm);
}

pre code {
  background: none;
  border: none;
  padding: 0;
  font-size: inherit;
  color: inherit;
}

blockquote {
  border-left: 4px solid var(--color-blockquote-border);
  background: var(--color-blockquote-bg);
  padding: 0.75rem 1.25rem;
  margin: 1.25rem 0;
  border-radius: 0 var(--radius) var(--radius) 0;
  color: #374151;
}

blockquote p:last-child {
  margin-bottom: 0;
}

ul, ol {
  padding-left: 1.75rem;
  margin-bottom: 1rem;
}

ul { list-style-type: disc; }
ol { list-style-type: decimal; }

li {
  margin-bottom: 0.35rem;
}

li > ul, li > ol {
  margin-top: 0.35rem;
  margin-bottom: 0;
}

.table-wrap {
  overflow-x: auto;
  margin: 1.25rem 0;
  border-radius: var(--radius);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9375rem;
}

thead {
  background: var(--color-table-header);
}

th {
  font-weight: 600;
  text-align: left;
  padding: 0.65rem 1rem;
  border-bottom: 2px solid var(--color-border);
  white-space: nowrap;
  color: #374151;
}

td {
  padding: 0.6rem 1rem;
  border-bottom: 1px solid var(--color-border);
}

tr:last-child td {
  border-bottom: none;
}

tr:nth-child(even) {
  background: var(--color-table-row-alt);
}

col.left { text-align: left; }
col.right { text-align: right; }
col.center { text-align: center; }

th.left, td.left { text-align: left; }
th.right, td.right { text-align: right; }
th.center, td.center { text-align: center; }

hr {
  border: none;
  border-top: 2px solid var(--color-border);
  margin: 2.5rem 0;
}

img {
  max-width: 100%;
  height: auto;
  border-radius: var(--radius);
  display: block;
  margin: 1rem 0;
}

.math-inline {
  display: inline-flex;
  align-items: center;
  vertical-align: middle;
  margin: 0 0.1em;
}

.math-inline svg {
  vertical-align: middle;
}

.math-display {
  display: flex;
  justify-content: center;
  align-items: center;
  margin: 1.5rem 0;
  overflow-x: auto;
  padding: 0.5rem;
}

.math-error {
  color: #dc2626;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 3px;
  padding: 0.15em 0.4em;
}

.footnote {
  font-size: 0.875rem;
  color: var(--color-muted);
  border-top: 1px solid var(--color-border);
  margin-top: 0.35rem;
  padding-top: 0.35rem;
}

input[type="checkbox"] {
  vertical-align: middle;
  margin-right: 0.35rem;
}

@media (max-width: 640px) {
  .container {
    padding: 1.5rem 1rem 3rem;
  }
  h1 { font-size: 1.75rem; }
  h2 { font-size: 1.35rem; }
}

@media print {
  .container { max-width: 100%; padding: 0; }
  pre { white-space: pre-wrap; word-break: break-all; }
  a { color: var(--color-text); }
}
"#;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let font_dir = find_katex_fonts(cli.katex_fonts.as_deref()).unwrap_or_default();
    if font_dir.is_empty() {
        eprintln!("Warning: KaTeX fonts not found. Math glyphs may not render correctly.");
        eprintln!("  Install: npm install katex");
        eprintln!("  Or pass: --katex-fonts /path/to/katex/dist/fonts");
    } else {
        eprintln!("Using KaTeX fonts from: {}", font_dir);
    }

    let mut sections: Vec<RenderedSection> = Vec::new();
    let mut doc_title = cli.title.clone().unwrap_or_default();

    for input_path in &cli.inputs {
        let base_dir = input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let source = std::fs::read_to_string(input_path)
            .with_context(|| format!("Cannot read {}", input_path.display()))?;

        let section = render_markdown(&source, &base_dir, cli.math_font_size, &font_dir, &ss, &ts)
            .with_context(|| format!("Failed to render {}", input_path.display()))?;

        if doc_title.is_empty() && !section.title.is_empty() {
            doc_title = section.title.clone();
        }

        sections.push(section);
    }

    if doc_title.is_empty() {
        doc_title = cli
            .output
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Document")
            .to_string();
    }

    let html = build_html(&doc_title, &sections);

    std::fs::write(&cli.output, html.as_bytes())
        .with_context(|| format!("Cannot write {}", cli.output.display()))?;

    eprintln!(
        "Written {} section(s) -> {}",
        sections.len(),
        cli.output.display()
    );

    Ok(())
}
