use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use clap::{Args, Parser, Subcommand};
use mermaid_rs_renderer::{render_with_options, RenderOptions};
use plantuml_encoding::encode_plantuml_deflate;
mod typst;
use pulldown_cmark::{Event, Options, Parser as MdParser, Tag, TagEnd};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse as parse_latex;
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::math_style::MathStyle;
use regex::{Captures, Regex};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;

mod view;

#[derive(Parser, Debug)]
#[command(
    name = "pagemd",
    about = "Convert Markdown to a self-contained single HTML file",
    long_about = typst::PAGEMD_LONG_ABOUT,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    args: CliArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(
        about = "Live-preview Markdown in the browser",
        long_about = "Start a local HTTP server and open the rendered page in your browser.\n\
                      The page hot-reloads when you save the input Markdown or referenced local assets.\n\
                      Press Ctrl+C to stop.\n\n\
                      Usage:\n  \
                      pagemd view -i INPUT.md\n  \
                      pagemd view -i doc.md --port 8080 --no-open\n\n\
                      Export clean SingleFile HTML (no live-reload script) on each successful render:\n  \
                      pagemd view -i doc.md --export out.html\n  \
                      pagemd view -i doc.md -o out.html\n\n\
                      One-shot export without preview:\n  \
                      pagemd -i doc.md -o out.html"
    )]
    View(ViewArgs),
}

#[derive(Args, Debug, Clone)]
struct ViewArgs {
    #[command(flatten)]
    convert: CliArgs,

    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Preview server bind address"
    )]
    host: String,

    #[arg(
        long,
        default_value_t = 3847,
        help = "Preview server port (if busy, picks a random available port)"
    )]
    port: u16,

    #[arg(long = "no-open", help = "Do not open the default browser")]
    no_open: bool,

    #[arg(
        long = "export",
        value_name = "FILE",
        help = "Write clean SingleFile HTML on each successful render (same as -o/--output)"
    )]
    export: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct CliArgs {
    #[arg(
        short = 'i',
        long = "input",
        value_name = "FILE",
        num_args = 1..,
        help = "Markdown input file(s)"
    )]
    inputs: Vec<PathBuf>,

    #[arg(
        short = 'd',
        long = "dir",
        value_name = "DIR",
        num_args = 1..,
        help = "Directory/directories to scan recursively for Markdown files"
    )]
    directories: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        help = "Output HTML path (required for convert; in view, exports on each render)"
    )]
    output: Option<PathBuf>,

    #[arg(long = "title", value_name = "TITLE", help = "Document title")]
    title: Option<String>,

    #[arg(
        long = "icon",
        value_name = "XX",
        value_parser = parse_icon_arg,
        help = "Two-character favicon label (a-z, A-Z, 0-9); shown uppercase in the tab icon"
    )]
    icon: Option<String>,

    #[arg(long = "font-size", default_value = "16", help = "Math font size")]
    math_font_size: f64,

    #[arg(
        long = "katex-fonts",
        value_name = "DIR",
        help = "Directory containing KaTeX .ttf font files for glyph embedding"
    )]
    katex_fonts: Option<PathBuf>,
}

const KATEX_FONT_FILES: &[&str] = &[
    "KaTeX_Main-Regular.ttf",
    "KaTeX_Main-Bold.ttf",
    "KaTeX_Main-Italic.ttf",
    "KaTeX_Main-BoldItalic.ttf",
    "KaTeX_Math-Italic.ttf",
    "KaTeX_Math-BoldItalic.ttf",
    "KaTeX_AMS-Regular.ttf",
    "KaTeX_Caligraphic-Regular.ttf",
    "KaTeX_Caligraphic-Bold.ttf",
    "KaTeX_Fraktur-Regular.ttf",
    "KaTeX_Fraktur-Bold.ttf",
    "KaTeX_SansSerif-Regular.ttf",
    "KaTeX_SansSerif-Bold.ttf",
    "KaTeX_SansSerif-Italic.ttf",
    "KaTeX_Script-Regular.ttf",
    "KaTeX_Typewriter-Regular.ttf",
    "KaTeX_Size1-Regular.ttf",
    "KaTeX_Size2-Regular.ttf",
    "KaTeX_Size3-Regular.ttf",
    "KaTeX_Size4-Regular.ttf",
];

fn katex_font_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PAGEMD_CACHE_DIR") {
        return PathBuf::from(dir).join("katex-fonts");
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("pagemd/katex-fonts");
    }
    if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Caches/pagemd/katex-fonts");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache/pagemd/katex-fonts");
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("pagemd/katex-fonts");
    }
    std::env::temp_dir().join("pagemd-katex-fonts")
}

fn ensure_katex_font_cache() -> Result<PathBuf> {
    let dir = katex_font_cache_dir();
    if dir.join("KaTeX_Main-Regular.ttf").exists() {
        return Ok(dir);
    }
    fs::create_dir_all(&dir).context("failed to create KaTeX font cache directory")?;
    for filename in KATEX_FONT_FILES {
        let bytes = ratex_katex_fonts::ttf_bytes(filename)
            .with_context(|| format!("missing bundled KaTeX font {filename}"))?;
        fs::write(dir.join(filename), bytes.as_ref())
            .with_context(|| format!("failed to write KaTeX font {filename}"))?;
    }
    Ok(dir)
}

fn find_katex_fonts(hint: Option<&Path>) -> Result<String> {
    if let Some(p) = hint {
        if p.join("KaTeX_Main-Regular.ttf").exists() {
            return Ok(p.to_string_lossy().into_owned());
        }
        bail!("KaTeX fonts not found in {}", p.display());
    }

    let dir = ensure_katex_font_cache()?;
    Ok(dir.to_string_lossy().into_owned())
}

fn latex_to_svg(expr: &str, display: bool, font_size: f64, font_dir: &str) -> Result<String> {
    let ast = parse_latex(expr).map_err(|e| anyhow::anyhow!("LaTeX parse error: {}", e))?;
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let opts = LayoutOptions {
        style,
        ..LayoutOptions::default()
    };
    let lbox = layout(&ast, &opts);
    let dl = to_display_list(&lbox);
    let embed = !font_dir.is_empty();
    let effective_font_size = if display {
        font_size * 2.5
    } else {
        font_size * 1.15
    };
    let svg_opts = SvgOptions {
        font_size: effective_font_size,
        padding: if display { 2.0 } else { 0.5 },
        stroke_width: 1.5,
        embed_glyphs: embed,
        font_dir: font_dir.to_owned(),
    };
    Ok(render_to_svg(&dl, &svg_opts))
}

const MAX_INLINE_RESOURCE_BYTES: usize = 25 * 1024 * 1024;

fn http_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("pagemd/0.1")
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Log fenced-block render failures to stderr (convert and `pagemd view` both use this path).
fn eprint_fence_render_error(kind: &str, err: &(impl std::fmt::Display + ?Sized), source: &str) {
    eprintln!("\n[pagemd] {kind} block render failed");
    eprintln!("{err:#}");
    let trimmed = source.trim();
    if !trimmed.is_empty() {
        eprintln!("\n--- {kind} source ---");
        eprintln!("{trimmed}");
        eprintln!("--- end {kind} source ---\n");
    } else {
        eprintln!();
    }
}

fn render_mermaid(code: &str) -> Result<String> {
    let opts = RenderOptions::modern()
        .with_node_spacing(60.0)
        .with_rank_spacing(80.0);
    let svg = render_with_options(code.trim(), opts).context("Failed to render Mermaid diagram")?;
    Ok(format!(
        "<div class=\"mermaid-display\"><div class=\"mermaid-canvas\">{svg}</div></div>\n"
    ))
}

fn mermaid_error_html(code: &str) -> String {
    format!(
        "<div class=\"mermaid-display mermaid-error\"><strong>Mermaid render failed</strong><pre><code>{}</code></pre></div>\n",
        html_escape(code)
    )
}

fn plantuml_skinparams() -> &'static str {
    "skinparam backgroundColor transparent\nskinparam sequenceParticipantBackgroundColor white\nskinparam sequenceParticipantBorderColor #94a3b8\nskinparam actorBackgroundColor white\nskinparam actorBorderColor #94a3b8\nskinparam shadowing false"
}

fn normalize_plantuml_source(code: &str) -> String {
    let trimmed = code.trim();
    if trimmed.contains("@start") && trimmed.contains("@end") {
        if trimmed.contains("skinparam") {
            trimmed.to_string()
        } else if let Some(index) = trimmed.find('\n') {
            format!(
                "{}\n{}{}",
                &trimmed[..index],
                plantuml_skinparams(),
                &trimmed[index..]
            )
        } else {
            trimmed.to_string()
        }
    } else {
        format!("@startuml\n{}\n{trimmed}\n@enduml", plantuml_skinparams())
    }
}

fn render_plantuml(code: &str) -> Result<String> {
    let source = normalize_plantuml_source(code);
    let encoded = encode_plantuml_deflate(&source)
        .map_err(|err| anyhow::anyhow!("Failed to encode PlantUML diagram: {:?}", err))?;
    let url = format!("https://www.plantuml.com/plantuml/svg/{encoded}");
    let (bytes, mime) = fetch_remote_resource(&url)?;
    if mime.eq_ignore_ascii_case("image/svg+xml") || bytes.starts_with(b"<svg") {
        let svg = String::from_utf8(bytes).context("PlantUML server returned non-UTF-8 SVG")?;
        Ok(format!(
            "<div class=\"plantuml-display\"><div class=\"plantuml-canvas\">{svg}</div></div>\n"
        ))
    } else {
        let data_uri = data_uri_from_bytes(&mime, &bytes);
        Ok(format!(
            "<div class=\"plantuml-display\"><img class=\"plantuml-image\" src=\"{}\" alt=\"PlantUML diagram\" loading=\"lazy\"></div>\n",
            html_escape(&data_uri)
        ))
    }
}

fn plantuml_error_html(code: &str) -> String {
    format!(
        "<div class=\"plantuml-display plantuml-error\"><strong>PlantUML render failed</strong><pre><code>{}</code></pre></div>\n",
        html_escape(code)
    )
}

fn is_diagram_html_info(info: &str) -> bool {
    let mut parts = info.split_whitespace();
    let Some(kind) = parts.next() else {
        return false;
    };

    if matches!(
        kind.to_ascii_lowercase().as_str(),
        "diagram-html" | "diagram_html"
    ) {
        return true;
    }

    kind.eq_ignore_ascii_case("diagram")
        && parts
            .next()
            .is_some_and(|format| format.eq_ignore_ascii_case("html"))
}

fn render_diagram_html(code: &str, base_dir: &Path) -> String {
    let body = inline_raw_html_resources(code.trim(), base_dir);
    format!(
        "<div class=\"diagram-html-display\"><div class=\"diagram-html-canvas\">{body}</div></div>\n"
    )
}

fn canonical_callout_kind(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "note" => Some("note"),
        "abstract" | "summary" | "tldr" => Some("abstract"),
        "info" | "todo" => Some("info"),
        "tip" | "hint" => Some("tip"),
        "success" | "check" | "done" => Some("success"),
        "question" | "help" | "faq" => Some("question"),
        "warning" | "warn" | "attention" => Some("warning"),
        "failure" | "fail" | "missing" => Some("failure"),
        "danger" | "error" => Some("danger"),
        "bug" => Some("bug"),
        "example" => Some("example"),
        "quote" | "cite" => Some("quote"),
        "important" => Some("important"),
        "caution" => Some("caution"),
        _ => None,
    }
}

fn callout_label(kind: &str) -> &'static str {
    match kind {
        "note" => "Note",
        "abstract" => "Abstract",
        "info" => "Info",
        "tip" => "Tip",
        "success" => "Success",
        "question" => "Question",
        "warning" => "Warning",
        "failure" => "Failure",
        "danger" => "Danger",
        "bug" => "Bug",
        "example" => "Example",
        "quote" => "Quote",
        "important" => "Important",
        "caution" => "Caution",
        _ => "Note",
    }
}

fn parse_callout_marker(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    if !trimmed.starts_with("[!") {
        return None;
    }
    let end = trimmed.find(']')?;
    let raw_kind = trimmed.get(2..end)?.trim_end_matches(['+', '-']);
    let kind = canonical_callout_kind(raw_kind)?.to_string();
    let title = trimmed[end + 1..].trim().to_string();
    Some((kind, title))
}

fn parse_admonition_info(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let kind = canonical_callout_kind(parts.next()?)?.to_string();
    let title = parts.next().unwrap_or("").trim();
    let title = title
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| title.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(title)
        .to_string();
    Some((kind, title))
}

fn strip_blockquote_marker(line: &str) -> Option<&str> {
    let mut spaces = 0usize;
    for ch in line.chars() {
        if ch == ' ' && spaces < 4 {
            spaces += 1;
            continue;
        }
        if ch == '>' && spaces <= 3 {
            let rest = &line[spaces + 1..];
            return Some(rest.strip_prefix(' ').unwrap_or(rest));
        }
        return None;
    }
    None
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

fn max_backtick_run(text: &str) -> usize {
    let mut max_run = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            max_run = max_run.max(current);
        } else {
            current = 0;
        }
    }
    max_run
}

fn internal_callout_fence(kind: &str, title: &str, content: &str) -> String {
    let fence = "`".repeat(max_backtick_run(content).max(3) + 1);
    let title_suffix = if title.is_empty() {
        String::new()
    } else {
        format!(" {title}")
    };
    let mut out = format!("{fence}pagemd-callout {kind}{title_suffix}\n");
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

fn preprocess_markdown_extensions(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = String::new();
    let mut i = 0usize;

    while i < lines.len() {
        if let Some(first) = strip_blockquote_marker(lines[i]) {
            if let Some((kind, title)) = parse_callout_marker(first) {
                i += 1;
                let mut content = String::new();
                while i < lines.len() {
                    if let Some(line) = strip_blockquote_marker(lines[i]) {
                        content.push_str(line);
                        content.push('\n');
                        i += 1;
                    } else {
                        break;
                    }
                }
                out.push_str(&internal_callout_fence(&kind, &title, &content));
                continue;
            }
        }

        let trimmed = lines[i].trim_start();
        if let Some(rest) = trimmed.strip_prefix(":::") {
            if let Some((kind, title)) = parse_admonition_info(rest) {
                i += 1;
                let mut content = String::new();
                while i < lines.len() && lines[i].trim() != ":::" {
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }
                out.push_str(&internal_callout_fence(&kind, &title, &content));
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix("!!!") {
            if let Some((kind, title)) = parse_admonition_info(rest) {
                i += 1;
                let mut content = String::new();
                while i < lines.len() {
                    let line = lines[i];
                    if line.trim().is_empty() {
                        content.push('\n');
                        i += 1;
                    } else if let Some(stripped) = line.strip_prefix("    ") {
                        content.push_str(stripped);
                        content.push('\n');
                        i += 1;
                    } else if let Some(stripped) = line.strip_prefix('\t') {
                        content.push_str(stripped);
                        content.push('\n');
                        i += 1;
                    } else if leading_spaces(line) > 0 {
                        content.push_str(line.trim_start());
                        content.push('\n');
                        i += 1;
                    } else {
                        break;
                    }
                }
                out.push_str(&internal_callout_fence(&kind, &title, &content));
                continue;
            }
        }

        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }

    out
}

fn parse_internal_callout_info(info: &str) -> Option<(String, String)> {
    let mut parts = info.trim().splitn(3, char::is_whitespace);
    if parts.next()? != "pagemd-callout" {
        return None;
    }
    let kind = parts.next()?.to_string();
    let title = parts.next().unwrap_or("").trim().to_string();
    Some((kind, title))
}

fn render_callout(
    kind: &str,
    title: &str,
    content: &str,
    base_dir: &Path,
    math_font_size: f64,
    font_dir: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
    depth: usize,
) -> Result<String> {
    let body = if depth >= 8 {
        format!("<p>{}</p>\n", html_escape(content.trim()))
    } else {
        render_markdown_with_depth(
            content,
            base_dir,
            math_font_size,
            font_dir,
            ss,
            ts,
            depth + 1,
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

fn is_remote_resource(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

fn is_already_embedded_or_non_fetchable(src: &str) -> bool {
    let lower = src.trim().to_ascii_lowercase();
    lower.is_empty()
        || lower.starts_with("data:")
        || lower.starts_with('#')
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("javascript:")
        || lower.starts_with("about:")
}

fn extension_from_reference(src: &str) -> &str {
    let clean = src.split(['?', '#']).next().unwrap_or(src);
    clean.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

fn data_uri_from_bytes(mime: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime, B64.encode(bytes))
}

fn fetch_remote_resource(url: &str) -> Result<(Vec<u8>, String)> {
    let response = http_client()
        .get(url)
        .send()
        .with_context(|| format!("Failed to fetch {url}"))?
        .error_for_status()
        .with_context(|| format!("Resource returned an error status: {url}"))?;
    let mime = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| mime_from_ext(extension_from_reference(url)).to_string());
    let bytes = response
        .bytes()
        .with_context(|| format!("Failed to read {url}"))?;
    if bytes.len() > MAX_INLINE_RESOURCE_BYTES {
        bail!("Resource is too large to inline: {url}");
    }
    Ok((bytes.to_vec(), mime))
}

fn local_resource_path(src: &str, base_dir: &Path) -> PathBuf {
    if src.starts_with('/') {
        PathBuf::from(src)
    } else {
        base_dir.join(src)
    }
}

fn resource_to_data_uri(src: &str, base_dir: &Path) -> Result<String> {
    let src = src.trim();
    if is_already_embedded_or_non_fetchable(src) {
        return Ok(src.to_string());
    }

    if is_remote_resource(src) {
        let (bytes, mime) = fetch_remote_resource(src)?;
        return Ok(data_uri_from_bytes(&mime, &bytes));
    }

    let path = local_resource_path(src, base_dir);
    let data = std::fs::read(&path).with_context(|| format!("Cannot read {}", path.display()))?;
    if data.len() > MAX_INLINE_RESOURCE_BYTES {
        bail!("Resource is too large to inline: {}", path.display());
    }
    let mime = mime_from_ext(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
    Ok(data_uri_from_bytes(mime, &data))
}

fn embedded_resource_error_data_uri(src: &str) -> String {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"120\" viewBox=\"0 0 640 120\"><rect width=\"640\" height=\"120\" fill=\"#fff7f7\"/><text x=\"24\" y=\"52\" font-family=\"system-ui,sans-serif\" font-size=\"16\" fill=\"#991b1b\">Resource could not be embedded</text><text x=\"24\" y=\"82\" font-family=\"monospace\" font-size=\"12\" fill=\"#7f1d1d\">{}</text></svg>",
        html_escape(src)
    );
    data_uri_from_bytes("image/svg+xml", svg.as_bytes())
}

fn image_to_data_uri(src: &str, base_dir: &Path) -> String {
    match resource_to_data_uri(src, base_dir) {
        Ok(value) => value,
        Err(_) => embedded_resource_error_data_uri(src),
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
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "html" | "htm" => "text/html",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn regex(pattern: &'static str) -> &'static Regex {
    static CACHE: OnceLock<
        std::sync::Mutex<std::collections::HashMap<&'static str, &'static Regex>>,
    > = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(value) = cache
        .lock()
        .expect("regex cache poisoned")
        .get(pattern)
        .copied()
    {
        return value;
    }
    let compiled = Box::leak(Box::new(Regex::new(pattern).expect("invalid regex")));
    cache
        .lock()
        .expect("regex cache poisoned")
        .insert(pattern, compiled);
    compiled
}

fn inline_resource_for_attr(src: &str, base_dir: &Path) -> String {
    match resource_to_data_uri(src, base_dir) {
        Ok(value) => value,
        Err(_) => data_uri_from_bytes(
            "text/plain",
            format!("Resource could not be embedded: {src}").as_bytes(),
        ),
    }
}

fn replace_attr_resources(input: &str, pattern: &'static str, base_dir: &Path) -> String {
    regex(pattern)
        .replace_all(input, |caps: &Captures<'_>| {
            let value = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            let suffix = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
            let embedded = inline_resource_for_attr(value, base_dir);
            format!("{}{}{}", &caps[1], html_escape(&embedded), suffix)
        })
        .into_owned()
}

fn inline_css_urls(input: &str, base_dir: &Path) -> String {
    let double = regex(r#"(?is)url\(\s*\"([^\"]*)\"\s*\)"#)
        .replace_all(input, |caps: &Captures<'_>| {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let embedded = match resource_to_data_uri(value, base_dir) {
                Ok(value) => value,
                Err(_) => embedded_resource_error_data_uri(value),
            };
            format!("url(\"{}\")", html_escape(&embedded))
        })
        .into_owned();
    let single = regex(r#"(?is)url\(\s*'([^']*)'\s*\)"#)
        .replace_all(&double, |caps: &Captures<'_>| {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let embedded = match resource_to_data_uri(value, base_dir) {
                Ok(value) => value,
                Err(_) => embedded_resource_error_data_uri(value),
            };
            format!("url(\"{}\")", html_escape(&embedded))
        })
        .into_owned();
    regex(r#"(?is)url\(\s*([^\s\)'\"]+)\s*\)"#)
        .replace_all(&single, |caps: &Captures<'_>| {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let embedded = match resource_to_data_uri(value, base_dir) {
                Ok(value) => value,
                Err(_) => embedded_resource_error_data_uri(value),
            };
            format!("url(\"{}\")", html_escape(&embedded))
        })
        .into_owned()
}

fn inline_raw_html_resources(raw: &str, base_dir: &Path) -> String {
    let rewritten = inline_css_urls(raw, base_dir);
    let rewritten = replace_attr_resources(
        &rewritten,
        r#"(?is)(\s(?:src|poster)\s*=\s*\")([^\"]*)(\")"#,
        base_dir,
    );
    let rewritten = replace_attr_resources(
        &rewritten,
        r#"(?is)(\s(?:src|poster)\s*=\s*')([^']*)(')"#,
        base_dir,
    );
    let rewritten = replace_attr_resources(
        &rewritten,
        r#"(?is)(<link\b[^>]*?\shref\s*=\s*\")([^\"]*)(\")"#,
        base_dir,
    );
    replace_attr_resources(
        &rewritten,
        r#"(?is)(<link\b[^>]*?\shref\s*=\s*')([^']*)(')"#,
        base_dir,
    )
}

struct RenderedSection {
    title: String,
    html: String,
    outline: Vec<HeadingOutline>,
}

#[derive(Clone)]
struct HeadingOutline {
    level: u32,
    id: String,
    text: String,
}

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
        Event::FootnoteReference(label) => buf.push_str(label),
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

fn render_markdown(
    source: &str,
    base_dir: &Path,
    math_font_size: f64,
    font_dir: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
) -> Result<RenderedSection> {
    render_markdown_with_depth(source, base_dir, math_font_size, font_dir, ss, ts, 0)
}

fn render_markdown_with_depth(
    source: &str,
    base_dir: &Path,
    math_font_size: f64,
    font_dir: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
    depth: usize,
) -> Result<RenderedSection> {
    let source = preprocess_markdown_extensions(source);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = MdParser::new_ext(&source, opts);
    let events: Vec<Event> = parser.collect();

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
            buf: String,
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

    for event in &events {
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
                        "mermaid" | "mmd" => match render_mermaid(&buf_str) {
                            Ok(rendered) => html.push_str(&rendered),
                            Err(err) => {
                                eprint_fence_render_error("Mermaid", &err, &buf_str);
                                html.push_str(&mermaid_error_html(&buf_str));
                            }
                        },
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
                                    base_dir,
                                    math_font_size,
                                    font_dir,
                                    ss,
                                    ts,
                                    depth,
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

            Context::Heading { level, buf, image } => {
                if let Some(pending) = image {
                    match event {
                        Event::End(TagEnd::Image) => {
                            let alt = html_escape(&pending.alt_buf);
                            buf.push_str(&format!(
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
                            append_inline_math_html(buf, text, math_font_size, font_dir)
                        }
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
                            buf.push_str(&format!(
                                "<a href=\"{}\"{title_attr}>",
                                html_escape(dest_url)
                            ));
                        }
                        Event::End(TagEnd::Link) => buf.push_str("</a>"),
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
                            if let Ok(svg) = latex_to_svg(math, false, math_font_size, font_dir) {
                                buf.push_str("<span class=\"math-inline\">");
                                buf.push_str(&svg);
                                buf.push_str("</span>");
                            }
                        }
                        Event::Html(raw) => buf.push_str(&inline_raw_html_resources(raw, base_dir)),
                        Event::InlineHtml(raw) => {
                            buf.push_str(&inline_raw_html_resources(raw, base_dir))
                        }
                        Event::SoftBreak => buf.push(' '),
                        Event::HardBreak => buf.push(' '),
                        Event::End(TagEnd::Heading(_)) => {
                            let lvl = *level;
                            let plain = strip_html_tags(buf);
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
                            html.push_str(&format!("<h{lvl} id=\"{id}\">{buf}</h{lvl}>\n"));
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
                        buf: String::new(),
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
                    current_target(&mut html, &mut paragraph_html)
                        .push_str(&inline_raw_html_resources(raw, base_dir));
                }
                Event::InlineHtml(raw) => {
                    if paragraph_html.is_some() {
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html)
                        .push_str(&inline_raw_html_resources(raw, base_dir));
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
                    append_inline_math_html(
                        current_target(&mut html, &mut paragraph_html),
                        text,
                        math_font_size,
                        font_dir,
                    );
                }

                Event::SoftBreak => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push('\n');
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

                Event::Start(Tag::FootnoteDefinition(label)) => {
                    html.push_str(&format!(
                        "<div class=\"footnote\" id=\"fn-{}\"><sup>{}</sup> ",
                        html_escape(label),
                        html_escape(label)
                    ));
                }
                Event::End(TagEnd::FootnoteDefinition) => html.push_str("</div>\n"),
                Event::FootnoteReference(label) => {
                    if let Some(plain) = paragraph_plain.as_mut() {
                        plain.push_str(label);
                        paragraph_is_plain = false;
                    }
                    current_target(&mut html, &mut paragraph_html).push_str(&format!(
                        "<sup><a href=\"#fn-{}\">{}</a></sup>",
                        html_escape(label),
                        html_escape(label)
                    ));
                }

                _ => {}
            },
        }
    }

    Ok(RenderedSection {
        title,
        html,
        outline,
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

fn parse_icon_arg(s: &str) -> Result<String, String> {
    if s.chars().count() != 2 {
        return Err("icon must be exactly 2 characters".into());
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("icon must use only a-z, A-Z, and 0-9".into());
    }
    Ok(s.to_ascii_uppercase())
}

#[derive(Debug, Clone)]
struct ResolvedInputs {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn canonical_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn push_unique_file(
    files: &mut Vec<PathBuf>,
    seen: &mut std::collections::HashSet<PathBuf>,
    path: PathBuf,
) {
    if seen.insert(canonical_key(&path)) {
        files.push(path);
    }
}

fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory {}", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("Cannot list directory {}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Cannot inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if file_type.is_file() && is_markdown_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn resolve_inputs(args: &CliArgs) -> Result<ResolvedInputs> {
    if args.inputs.is_empty() && args.directories.is_empty() {
        bail!("Missing required input. Pass --input <FILE> or --dir <DIR>.");
    }

    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut seen_files = std::collections::HashSet::new();
    let mut seen_dirs = std::collections::HashSet::new();

    for input in &args.inputs {
        if !input.exists() {
            bail!("Input file does not exist: {}", input.display());
        }
        if !input.is_file() {
            bail!("Input is not a file: {}", input.display());
        }
        push_unique_file(&mut files, &mut seen_files, input.clone());
    }

    for dir in &args.directories {
        if !dir.exists() {
            bail!("Input directory does not exist: {}", dir.display());
        }
        if !dir.is_dir() {
            bail!("Input is not a directory: {}", dir.display());
        }

        let canonical = canonical_key(dir);
        if seen_dirs.insert(canonical.clone()) {
            directories.push(canonical);
        }

        let mut dir_files = Vec::new();
        collect_markdown_files(dir, &mut dir_files)?;
        for path in dir_files {
            push_unique_file(&mut files, &mut seen_files, path);
        }
    }

    if files.is_empty() {
        bail!("No Markdown files found. Pass --input <FILE> or --dir <DIR> containing .md/.markdown files.");
    }

    Ok(ResolvedInputs { files, directories })
}

/// Default favicon label from the input path (first two alphanumeric chars of the stem, uppercase).
fn default_icon_label_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let chars: Vec<char> = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(2)
        .collect();
    match chars.len() {
        0 => "PG".to_string(),
        1 => {
            let c = chars[0].to_ascii_uppercase();
            format!("{c}{c}")
        }
        _ => chars.into_iter().map(|c| c.to_ascii_uppercase()).collect(),
    }
}

fn resolve_icon_label(args: &CliArgs, resolved_inputs: &[PathBuf]) -> String {
    if let Some(icon) = &args.icon {
        return icon.clone();
    }
    resolved_inputs
        .first()
        .map(|p| default_icon_label_from_path(p))
        .unwrap_or_else(|| "PG".to_string())
}

fn hash_icon_label(label: &str) -> u32 {
    let mut hash: u32 = 5381;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    hash
}

/// Deterministic saturated background from icon text (HSL: hue from hash, fixed S/L).
fn icon_background_rgb(label: &str) -> (u8, u8, u8) {
    let hue = f64::from(hash_icon_label(label) % 360);
    hsl_to_rgb(hue, 0.62, 0.48)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = (h / 360.0).fract();
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h * 6.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn srgb_channel(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance.
fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
    let r = srgb_channel(rgb.0);
    let g = srgb_channel(rgb.1);
    let b = srgb_channel(rgb.2);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast_ratio(lighter: f64, darker: f64) -> f64 {
    (lighter + 0.05) / (darker + 0.05)
}

/// Pick black or white text for maximum contrast on the given background.
fn icon_foreground_rgb(background: (u8, u8, u8)) -> (u8, u8, u8) {
    let bg_l = relative_luminance(background);
    let white = 1.0;
    let black = 0.0;
    let on_white = contrast_ratio(white, bg_l);
    let on_black = contrast_ratio(bg_l, black);
    if on_white >= on_black {
        (255, 255, 255)
    } else {
        (17, 17, 17)
    }
}

fn icon_colors(label: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    let bg = icon_background_rgb(label);
    let fg = icon_foreground_rgb(bg);
    (bg, fg)
}

fn encode_svg_for_data_uri(svg: &str) -> String {
    svg.chars()
        .map(|c| match c {
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            '"' => "%22".to_string(),
            '\'' => "%27".to_string(),
            '&' => "%26".to_string(),
            '+' => "%2B".to_string(),
            ' ' => "%20".to_string(),
            _ if c.is_ascii() => c.to_string(),
            c => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                encoded.bytes().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}

fn favicon_link_tag(label: &str) -> String {
    let label = label.to_ascii_uppercase();
    let ((br, bg, bb), (fr, fg, fb)) = icon_colors(&label);
    const ICON_RX: u32 = 7;
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect width='32' height='32' rx='{ICON_RX}' ry='{ICON_RX}' fill='#{br:02x}{bg:02x}{bb:02x}'/><text x='16' y='16' text-anchor='middle' dominant-baseline='central' font-family='system-ui,-apple-system,sans-serif' font-size='18' font-weight='700' letter-spacing='1.5' fill='#{fr:02x}{fg:02x}{fb:02x}'>{label}</text></svg>"
    );
    format!(
        "<link rel=\"icon\" href=\"data:image/svg+xml,{}\">\n",
        encode_svg_for_data_uri(&svg)
    )
}

fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn unique_heading_id(
    text: &str,
    heading_ids: &mut std::collections::HashMap<String, usize>,
) -> String {
    let base = {
        let slug = slugify(text);
        if slug.is_empty() {
            "heading".to_string()
        } else {
            slug
        }
    };
    let count = heading_ids.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{}", *count)
    }
}

fn section_label(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn build_outline_nav(body_sections: &[RenderedSection]) -> String {
    body_sections
        .iter()
        .enumerate()
        .map(|(section_index, section)| {
            let doc_id = format!("doc-{}", section_index + 1);
            let active = if section_index == 0 { " is-active" } else { "" };
            let items = if section.outline.is_empty() {
                "<div class=\"doc-outline-empty\">No headings</div>\n".to_string()
            } else {
                section
                    .outline
                    .iter()
                    .map(|heading| {
                        let depth = heading.level.saturating_sub(1).min(5);
                        format!(
                            "<a class=\"doc-outline-link depth-{depth}\" href=\"#{}\" data-heading-target=\"{}\" title=\"{}\">{}</a>\n",
                            html_escape(&heading.id),
                            html_escape(&heading.id),
                            html_escape(&heading.text),
                            html_escape(&heading.text)
                        )
                    })
                    .collect()
            };
            format!(
                "<nav class=\"doc-outline-list{active}\" data-outline-for=\"{doc_id}\">\n{items}</nav>\n"
            )
        })
        .collect()
}

fn build_html(title: &str, body_sections: &[RenderedSection], icon_label: &str) -> String {
    build_html_with_nav(title, body_sections, icon_label, None)
}

const DIAGRAM_HTML_MARKER: &str = "class=\"diagram-html-display\"";
const DIAGRAM_HTML_TAILWIND_BROWSER_JS: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/diagram-html-tailwind-browser.js"
));

fn diagram_html_tailwind_browser_js() -> &'static str {
    std::str::from_utf8(DIAGRAM_HTML_TAILWIND_BROWSER_JS)
        .expect("bundled diagram Tailwind browser runtime must be UTF-8")
}

fn script_escape(script: &str) -> String {
    script.replace("</script", "<\\/script")
}

fn build_html_with_nav(
    title: &str,
    body_sections: &[RenderedSection],
    icon_label: &str,
    nav_labels: Option<&[String]>,
) -> String {
    let use_sidebar = body_sections.len() > 1;
    let body_html: String = if !use_sidebar {
        body_sections[0].html.clone()
    } else {
        body_sections
            .iter()
            .enumerate()
            .map(|(index, sec)| {
                let active = if index == 0 { " is-active" } else { "" };
                format!(
                    "<section class=\"doc-section doc-panel{active}\" id=\"doc-{}\" data-doc-panel>\n{}</section>\n",
                    index + 1,
                    sec.html
                )
            })
            .collect()
    };
    let (layout_open, layout_close, nav_html, script_html) = if use_sidebar {
        let nav_items: String = body_sections
            .iter()
            .enumerate()
            .map(|(index, section)| {
                let label = nav_labels
                    .and_then(|labels| labels.get(index))
                    .cloned()
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| {
                        if section.title.trim().is_empty() {
                            format!("Document {}", index + 1)
                        } else {
                            section.title.clone()
                        }
                    });
                let active = if index == 0 { " is-active" } else { "" };
                let escaped_label = html_escape(&label);
                format!(
                    "<div class=\"doc-nav-row\"><a class=\"doc-nav-link{active}\" href=\"#doc-{}\" data-doc-target=\"doc-{}\" title=\"{}\"><span class=\"doc-nav-label\">{}</span></a><button type=\"button\" class=\"doc-nav-copy\" data-copy-label=\"{}\" aria-label=\"Copy filename {}\" title=\"Copy filename\">Copy</button></div>\n",
                    index + 1,
                    index + 1,
                    escaped_label,
                    escaped_label,
                    escaped_label,
                    escaped_label
                )
            })
            .collect();
        let outline_nav = build_outline_nav(body_sections);
        (
            "<div class=\"doc-workspace outline-hidden\" data-doc-workspace>\n".to_string(),
            "</div>\n".to_string(),
            format!(
                "<aside class=\"doc-sidebar doc-pane\" aria-label=\"Markdown files\"><nav class=\"doc-nav\">\n{nav_items}</nav>\n</aside>\n<div class=\"doc-resizer doc-resizer-left\" role=\"separator\" aria-label=\"Resize file navigation\" data-resizer=\"left\"></div>\n<main class=\"doc-main\">\n<button type=\"button\" class=\"doc-outline-toggle\" data-outline-toggle>Outline</button>\n"
            ),
            format!(
                "</main>\n<div class=\"doc-resizer doc-resizer-right\" role=\"separator\" aria-label=\"Resize outline\" data-resizer=\"right\"></div>\n<aside class=\"doc-outline doc-pane\" aria-label=\"Markdown outline\">\n<div class=\"doc-pane-header\">Outline</div>\n{outline_nav}</aside>\n<script>\n{DOC_WORKSPACE_SCRIPT}\n</script>\n"
            ),
        )
    } else {
        (String::new(), String::new(), String::new(), String::new())
    };
    let container_class = if use_sidebar {
        "container container-with-sidebar"
    } else {
        "container"
    };
    let diagram_script = if body_sections
        .iter()
        .any(|section| section.html.contains(DIAGRAM_HTML_MARKER))
    {
        format!(
            "<script>\n{}\n</script>\n",
            script_escape(diagram_html_tailwind_browser_js())
        )
    } else {
        String::new()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
{favicon}
<style>
{css}
</style>
{diagram_script}
</head>
<body>
<div class="{container_class}">
{layout_open}{nav_html}
{body_html}
{script_html}{layout_close}
</div>
</body>
</html>"#,
        title = html_escape(title),
        favicon = favicon_link_tag(icon_label),
        css = CSS,
        diagram_script = diagram_script,
        layout_open = layout_open,
        nav_html = nav_html,
        body_html = body_html,
        script_html = script_html,
        layout_close = layout_close,
        container_class = container_class,
    )
}

const DOC_WORKSPACE_SCRIPT: &str = r##"(function () {
  var workspace = document.querySelector("[data-doc-workspace]");
  if (!workspace) return;

  var storageKey = "pagemd.workspace.v1.";
  function clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
  }
  function storageGet(name) {
    try {
      return window.localStorage ? localStorage.getItem(storageKey + name) : null;
    } catch (_) {
      return null;
    }
  }
  function storageSet(name, value) {
    try {
      if (window.localStorage) localStorage.setItem(storageKey + name, value);
    } catch (_) {}
  }
  function leftWidthBounds() {
    if (window.matchMedia("(min-width: 1600px)").matches) {
      return { min: 220, fallback: 280, max: 460 };
    }
    if (window.matchMedia("(min-width: 1200px)").matches) {
      return { min: 200, fallback: 240, max: 420 };
    }
    if (window.matchMedia("(min-width: 900px)").matches) {
      return { min: 170, fallback: 210, max: 340 };
    }
    return { min: 150, fallback: 180, max: 280 };
  }
  function rightWidthBounds() {
    if (window.matchMedia("(min-width: 1400px)").matches) {
      return { min: 240, fallback: 300, max: 440 };
    }
    return { min: 210, fallback: 260, max: 360 };
  }
  function loadNumber(name, fallback) {
    var raw = storageGet(name);
    var value = raw ? Number(raw) : NaN;
    return Number.isFinite(value) ? value : fallback;
  }
  function setWidth(name, value) {
    var rounded = Math.round(value);
    workspace.style.setProperty("--" + name, rounded + "px");
    storageSet(name, String(rounded));
  }
  function setOutlineVisible(visible) {
    workspace.classList.toggle("outline-hidden", !visible);
    storageSet("outlineVisible", visible ? "1" : "0");
    var toggle = document.querySelector("[data-outline-toggle]");
    if (toggle) {
      toggle.setAttribute("aria-expanded", visible ? "true" : "false");
      toggle.textContent = visible ? "Hide outline" : "Outline";
    }
  }
  function panelForId(id) {
    var panels = document.querySelectorAll("[data-doc-panel]");
    var current = document.querySelector("[data-doc-panel].is-active");
    if (id && current && window.CSS && CSS.escape && current.querySelector("#" + CSS.escape(id))) {
      return current;
    }
    var target = id ? document.getElementById(id) : null;
    if (target) {
      return target.matches("[data-doc-panel]") ? target : target.closest("[data-doc-panel]");
    }
    var storedId = storageGet("activeDoc");
    var stored = storedId ? document.getElementById(storedId) : null;
    return stored && stored.matches("[data-doc-panel]") ? stored : panels[0];
  }
  function activePanelFromHash() {
    return panelForId((window.location.hash || "").replace(/^#/, ""));
  }
  function activate(id) {
    var panels = document.querySelectorAll("[data-doc-panel]");
    var links = document.querySelectorAll("[data-doc-target]");
    var outlines = document.querySelectorAll("[data-outline-for]");
    var activePanel = id ? panelForId(id) : activePanelFromHash();
    if (!activePanel) return;
    panels.forEach(function (panel) {
      panel.classList.toggle("is-active", panel === activePanel);
    });
    links.forEach(function (link) {
      link.classList.toggle("is-active", link.getAttribute("data-doc-target") === activePanel.id);
    });
    outlines.forEach(function (outline) {
      outline.classList.toggle("is-active", outline.getAttribute("data-outline-for") === activePanel.id);
    });
    storageSet("activeDoc", activePanel.id);
    updateOutlineActive();
  }
  function updateOutlineActive() {
    var activePanel = document.querySelector("[data-doc-panel].is-active");
    if (!activePanel) return;
    var headings = activePanel.querySelectorAll("h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]");
    var current = headings[0] || null;
    headings.forEach(function (heading) {
      if (heading.getBoundingClientRect().top <= 140) {
        current = heading;
      }
    });
    var outline = document.querySelector('[data-outline-for="' + activePanel.id + '"]');
    if (!outline) return;
    outline.querySelectorAll("[data-heading-target]").forEach(function (link) {
      link.classList.toggle("is-active", !!current && link.getAttribute("data-heading-target") === current.id);
    });
  }
  function cssEscape(value) {
    if (window.CSS && CSS.escape) {
      return CSS.escape(value);
    }
    return String(value).replace(/[^a-zA-Z0-9_-]/g, "\\$&");
  }

  function scrollToHeading(id, panelId) {
    var activePanel = panelId ? panelForId(panelId) : activePanelFromHash();
    if (!activePanel) return false;
    var target = activePanel.querySelector("#" + cssEscape(id));
    if (!target) return false;
    activate(activePanel.id);
    target.scrollIntoView({ behavior: "smooth", block: "start" });
    history.replaceState(null, "", "#" + id);
    updateOutlineActive();
    return true;
  }

  var leftBounds = leftWidthBounds();
  var rightBounds = rightWidthBounds();
  setWidth("leftWidth", clamp(loadNumber("leftWidth", leftBounds.fallback), leftBounds.min, leftBounds.max));
  setWidth("rightWidth", clamp(loadNumber("rightWidth", rightBounds.fallback), rightBounds.min, rightBounds.max));
  setOutlineVisible(storageGet("outlineVisible") === "1");

  window.PageMDActivateDocumentFromHash = function () {
    activate((window.location.hash || "").replace(/^#/, ""));
  };

  var outlineToggle = document.querySelector("[data-outline-toggle]");
  if (outlineToggle) {
    outlineToggle.addEventListener("click", function (event) {
      event.preventDefault();
      event.stopPropagation();
      setOutlineVisible(workspace.classList.contains("outline-hidden"));
    });
  }

  function fallbackCopyText(text) {
    var textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.top = "-9999px";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.focus();
    textarea.select();
    var ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (_) {
      ok = false;
    }
    textarea.remove();
    return ok;
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text)
        .then(function () { return true; })
        .catch(function () { return fallbackCopyText(text); });
    }
    return Promise.resolve(fallbackCopyText(text));
  }

  function markCopyButton(button, ok) {
    var original = button.getAttribute("data-copy-original") || button.textContent;
    button.setAttribute("data-copy-original", original);
    button.classList.toggle("is-copied", ok);
    button.classList.toggle("is-copy-failed", !ok);
    button.textContent = ok ? "Copied" : "Failed";
    window.setTimeout(function () {
      button.classList.remove("is-copied", "is-copy-failed");
      button.textContent = original;
    }, 1400);
  }

  document.addEventListener("click", function (event) {
    var copyButton = event.target && event.target.closest
      ? event.target.closest("[data-copy-label]")
      : null;
    if (copyButton) {
      event.preventDefault();
      event.stopPropagation();
      var label = copyButton.getAttribute("data-copy-label") || "";
      copyText(label).then(function (ok) {
        markCopyButton(copyButton, ok);
      });
      return;
    }

    var navLink = event.target && event.target.closest
      ? event.target.closest("[data-doc-target]")
      : null;
    if (navLink) {
      event.preventDefault();
      var docId = navLink.getAttribute("data-doc-target");
      if (docId) {
        history.pushState(null, "", "#" + docId);
        activate(docId);
      }
      return;
    }
    var headingLink = event.target && event.target.closest
      ? event.target.closest("[data-heading-target]")
      : null;
    if (headingLink) {
      event.preventDefault();
      var outline = headingLink.closest("[data-outline-for]");
      var panelId = outline ? outline.getAttribute("data-outline-for") : null;
      scrollToHeading(headingLink.getAttribute("data-heading-target"), panelId);
      return;
    }
  });

  window.addEventListener("hashchange", window.PageMDActivateDocumentFromHash);
  window.addEventListener("scroll", updateOutlineActive, { passive: true });
  window.PageMDActivateDocumentFromHash();

  document.querySelectorAll("[data-resizer]").forEach(function (handle) {
    handle.addEventListener("mousedown", function (event) {
      event.preventDefault();
      var kind = handle.getAttribute("data-resizer");
      var startX = event.clientX;
      var leftBounds = leftWidthBounds();
      var rightBounds = rightWidthBounds();
      var startLeft = clamp(loadNumber("leftWidth", leftBounds.fallback), leftBounds.min, leftBounds.max);
      var startRight = clamp(loadNumber("rightWidth", rightBounds.fallback), rightBounds.min, rightBounds.max);
      document.body.classList.add("doc-resizing");
      function onMove(moveEvent) {
        if (kind === "left") {
          setWidth("leftWidth", clamp(startLeft + moveEvent.clientX - startX, leftBounds.min, leftBounds.max));
        } else {
          setWidth("rightWidth", clamp(startRight + startX - moveEvent.clientX, rightBounds.min, rightBounds.max));
          setOutlineVisible(true);
        }
      }
      function onUp() {
        document.body.classList.remove("doc-resizing");
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      }
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    });
  });
})();"##;

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
  --color-callout-bg: #f8fafc;
  --color-callout-title: #0f172a;
  --color-callout-note: #2563eb;
  --color-callout-info: #0891b2;
  --color-callout-tip: #16a34a;
  --color-callout-warning: #d97706;
  --color-callout-danger: #dc2626;
  --color-callout-muted: #64748b;
  --color-link: #2563eb;
  --color-link-hover: #1d4ed8;
  --color-table-header: #f9fafb;
  --color-table-row-alt: #f9fafb;
  --mermaid-bg: #ffffff;
  --mermaid-fg: #24292f;
  --mermaid-accent: #0969da;
  --mermaid-line: #57606a;
  --mermaid-muted: #57606a;
  --mermaid-surface: #f6f8fa;
  --mermaid-border: #d0d7de;
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

.container-with-sidebar {
  max-width: none;
  padding: 0;
}

.doc-workspace {
  --leftWidth: clamp(170px, 18vw, 240px);
  --rightWidth: clamp(220px, 20vw, 300px);
  min-height: 100vh;
  display: grid;
  grid-template-columns: var(--leftWidth) 8px minmax(0, 1fr) 8px var(--rightWidth);
  align-items: stretch;
  justify-content: center;
}

@media (min-width: 1200px) {
  .doc-workspace {
    --leftWidth: clamp(200px, 18vw, 260px);
  }
}

@media (min-width: 1600px) {
  .doc-workspace {
    --leftWidth: clamp(220px, 17vw, 300px);
    --rightWidth: clamp(260px, 18vw, 340px);
  }
}

.doc-workspace.outline-hidden {
  grid-template-columns: var(--leftWidth) 8px minmax(0, 1fr) 0 0;
}

.doc-pane {
  position: sticky;
  top: 0;
  height: 100vh;
  overflow-y: auto;
  background: #fbfcff;
}

.doc-sidebar {
  padding: 0.55rem 0.45rem;
  border-right: 1px solid var(--color-border);
}

.doc-outline {
  padding: 0.85rem 0.7rem;
  border-left: 1px solid var(--color-border);
}

.doc-workspace.outline-hidden .doc-outline,
.doc-workspace.outline-hidden .doc-resizer-right {
  display: none;
}

.doc-pane-header {
  position: sticky;
  top: 0;
  z-index: 1;
  margin: -1rem -0.75rem 0.65rem;
  padding: 0.95rem 1rem 0.7rem;
  border-bottom: 1px solid #e2e8f0;
  background: rgba(251, 252, 255, 0.94);
  backdrop-filter: blur(10px);
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--color-muted);
}

.doc-nav {
  display: flex;
  flex-direction: column;
  gap: 0.08rem;
}

.doc-nav-row {
  position: relative;
}

.doc-nav-link {
  position: relative;
  display: flex;
  align-items: center;
  overflow: hidden;
  min-width: 0;
  padding: 0.38rem 2.8rem 0.38rem 0.72rem;
  border-radius: 8px;
  color: #475569;
  font-size: 0.78rem;
  font-weight: 500;
  line-height: 1.25;
  transition: background 120ms ease, color 120ms ease, box-shadow 120ms ease;
}

.doc-nav-label {
  overflow: hidden;
  min-width: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-nav-link::before {
  content: "";
  position: absolute;
  left: 0.28rem;
  top: 0.45rem;
  bottom: 0.45rem;
  width: 2px;
  border-radius: 999px;
  background: transparent;
  transition: background 120ms ease;
}

.doc-nav-link:hover,
.doc-nav-row:hover .doc-nav-link {
  background: #f1f5f9;
  color: #0f172a;
  text-decoration: none;
}

.doc-nav-link.is-active {
  background: #eff6ff;
  color: #1d4ed8;
  font-weight: 650;
  box-shadow: inset 0 0 0 1px rgba(37, 99, 235, 0.10);
}

.doc-nav-link.is-active::before {
  background: #2563eb;
}

.doc-nav-copy {
  position: absolute;
  top: 50%;
  right: 0.35rem;
  transform: translateY(-50%);
  z-index: 1;
  max-width: 2.25rem;
  overflow: hidden;
  border: 1px solid transparent;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.86);
  color: #64748b;
  cursor: pointer;
  font: inherit;
  font-size: 0.62rem;
  font-weight: 700;
  line-height: 1;
  opacity: 0;
  padding: 0.22rem 0.34rem;
  text-overflow: clip;
  transition: opacity 120ms ease, color 120ms ease, border-color 120ms ease, background 120ms ease;
  white-space: nowrap;
}

.doc-nav-row:hover .doc-nav-copy,
.doc-nav-copy:focus-visible,
.doc-nav-copy.is-copied,
.doc-nav-copy.is-copy-failed {
  opacity: 1;
}

.doc-nav-copy:hover,
.doc-nav-copy:focus-visible {
  border-color: #bfdbfe;
  background: #ffffff;
  color: #1d4ed8;
  outline: none;
}

.doc-nav-copy.is-copied {
  max-width: none;
  border-color: #bbf7d0;
  color: #15803d;
}

.doc-nav-copy.is-copy-failed {
  max-width: none;
  border-color: #fecaca;
  color: #b91c1c;
}

.doc-resizer {
  cursor: col-resize;
  background: transparent;
  transition: background 120ms ease;
}

.doc-resizer:hover,
.doc-resizing .doc-resizer {
  background: #dbeafe;
}

.doc-resizing {
  cursor: col-resize;
  user-select: none;
}

.doc-main {
  max-width: 980px;
  width: 100%;
  min-width: 0;
  margin: 0 auto;
  padding: 3rem 3rem 5rem;
}

.doc-outline-toggle {
  position: fixed;
  top: 0.85rem;
  right: 0.9rem;
  z-index: 10;
  border: 1px solid #cbd5e1;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.92);
  color: #475569;
  cursor: pointer;
  font: inherit;
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1;
  padding: 0.38rem 0.62rem;
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.08);
}

.doc-outline-toggle:hover {
  border-color: #93c5fd;
  color: #1d4ed8;
}

.doc-panel {
  display: none;
}

.doc-panel.is-active {
  display: block;
}

.doc-outline-list {
  display: none;
}

.doc-outline-list.is-active {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
}

.doc-outline-link {
  display: block;
  overflow: hidden;
  padding: 0.35rem 0.35rem;
  border-radius: 8px;
  color: #64748b;
  font-size: 0.82rem;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-outline-link.depth-2 { padding-left: 0.85rem; }
.doc-outline-link.depth-3 { padding-left: 1.3rem; }
.doc-outline-link.depth-4,
.doc-outline-link.depth-5 { padding-left: 1.75rem; }

.doc-outline-link:hover {
  background: #f1f5f9;
  color: #1d4ed8;
  text-decoration: none;
}

.doc-outline-link.is-active {
  background: #eff6ff;
  color: #1d4ed8;
  font-weight: 700;
}

.doc-outline-empty {
  padding: 0.5rem 0.35rem;
  color: #94a3b8;
  font-size: 0.82rem;
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

.callout {
  --callout-accent: var(--color-callout-note);
  margin: 1.25rem 0;
  border: 1px solid color-mix(in srgb, var(--callout-accent) 26%, var(--color-border));
  border-left: 4px solid var(--callout-accent);
  border-radius: var(--radius);
  background: linear-gradient(135deg, color-mix(in srgb, var(--callout-accent) 7%, #fff), var(--color-callout-bg));
  box-shadow: var(--shadow-sm);
  overflow: hidden;
}

.callout-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem 0.35rem;
  color: var(--color-callout-title);
  font-weight: 700;
  line-height: 1.4;
}

.callout-title::before {
  content: "";
  width: 0.65rem;
  height: 0.65rem;
  border-radius: 999px;
  background: var(--callout-accent);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--callout-accent) 14%, transparent);
  flex: 0 0 auto;
}

.callout-body {
  padding: 0.25rem 1rem 0.85rem 2.15rem;
  color: #334155;
}

.callout-body > :last-child {
  margin-bottom: 0;
}

.callout-info,
.callout-abstract {
  --callout-accent: var(--color-callout-info);
}

.callout-tip,
.callout-success {
  --callout-accent: var(--color-callout-tip);
}

.callout-warning,
.callout-caution,
.callout-important,
.callout-question {
  --callout-accent: var(--color-callout-warning);
}

.callout-danger,
.callout-failure,
.callout-bug {
  --callout-accent: var(--color-callout-danger);
}

.callout-example,
.callout-quote {
  --callout-accent: var(--color-callout-muted);
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
  margin: 1.5rem 0;
  border-radius: 14px;
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.08), 0 1px 2px rgba(15, 23, 42, 0.06);
  border: 1px solid #e2e8f0;
  background: #ffffff;
}

table {
  width: 100%;
  min-width: 680px;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 0.925rem;
}

thead {
  background: linear-gradient(180deg, #f8fafc, #eef2ff);
}

th {
  font-weight: 700;
  text-align: left;
  padding: 0.8rem 1rem;
  border-bottom: 1px solid #cbd5e1;
  white-space: nowrap;
  color: #0f172a;
  letter-spacing: 0.015em;
}

td {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  color: #334155;
  vertical-align: top;
}

td code {
  white-space: nowrap;
  background: #eef2ff;
  border-color: #c7d2fe;
  color: #3730a3;
}

tr:last-child td {
  border-bottom: none;
}

tr:nth-child(even) {
  background: #f8fafc;
}

tr:hover td {
  background: #f1f5f9;
}

col.left { text-align: left; }
col.right { text-align: right; }
col.center { text-align: center; }

th.left, td.left { text-align: left; }
th.right, td.right { text-align: right; font-variant-numeric: tabular-nums; }
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
  vertical-align: -0.18em;
  margin: 0 0.08em;
  line-height: 1;
}

.math-inline svg {
  height: 1.25em;
  width: auto;
  max-width: none;
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

.mermaid-display {
  margin: 1.5rem 0;
  padding: 0;
  overflow-x: auto;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.mermaid-canvas {
  min-width: max-content;
  display: flex;
  justify-content: center;
  padding: 0.25rem 0;
  border-radius: 0;
  background: transparent;
}

.mermaid-display svg {
  max-width: 100%;
  height: auto;
  font-family: var(--font-sans);
  color: var(--mermaid-fg);
}

.mermaid-display svg text,
.mermaid-display svg tspan {
  fill: var(--mermaid-fg);
  font-family: var(--font-sans);
}

.mermaid-display svg path,
.mermaid-display svg line,
.mermaid-display svg polyline {
  stroke-linecap: round;
  stroke-linejoin: round;
}

.mermaid-display svg .node rect,
.mermaid-display svg .node circle,
.mermaid-display svg .node ellipse,
.mermaid-display svg .node polygon,
.mermaid-display svg .node path {
  fill: #ffffff;
  stroke: #94a3b8;
  stroke-width: 1.5px;
  filter: none;
}

.mermaid-display svg .edgePath path,
.mermaid-display svg .flowchart-link,
.mermaid-display svg .relationshipLine,
.mermaid-display svg .messageLine0,
.mermaid-display svg .messageLine1 {
  stroke: var(--mermaid-line);
  stroke-width: 1.8px;
}

.mermaid-display svg marker path,
.mermaid-display svg marker polygon {
  fill: var(--mermaid-accent);
  stroke: var(--mermaid-accent);
}

.mermaid-display svg .edgeLabel,
.mermaid-display svg .labelBkg,
.mermaid-display svg .messageText,
.mermaid-display svg .actor,
.mermaid-display svg .cluster rect {
  color: var(--mermaid-muted);
}

.mermaid-display svg .cluster rect {
  fill: transparent;
  stroke: #cbd5e1;
  stroke-dasharray: 5 5;
}

.mermaid-error {
  color: #991b1b;
  background: linear-gradient(135deg, #fff7f7, #fff);
  border-color: #fecaca;
}

.mermaid-error pre {
  margin: 0.75rem 0 0;
  background: #450a0a;
}

.plantuml-display {
  margin: 1.5rem 0;
  padding: 0;
  overflow-x: auto;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  text-align: center;
}

.plantuml-canvas {
  min-width: max-content;
  display: flex;
  justify-content: center;
  padding: 0.25rem 0;
  border-radius: 0;
  background: transparent;
}

.plantuml-canvas svg {
  max-width: 100%;
  height: auto;
  background: transparent !important;
}

.plantuml-canvas svg rect[fill='#E2E2F0'],
.plantuml-canvas svg polygon[fill='#E2E2F0'],
.plantuml-canvas svg ellipse[fill='#E2E2F0'],
.plantuml-canvas svg circle[fill='#E2E2F0'] {
  fill: #ffffff !important;
}

.plantuml-image {
  display: inline-block;
  max-width: 100%;
  height: auto;
  margin: 0;
  border-radius: 0;
  background: transparent;
}

.plantuml-error {
  color: #991b1b;
  background: linear-gradient(135deg, #fff7f7, #fff);
  border-color: #fecaca;
  text-align: left;
}

.plantuml-error pre {
  margin: 0.75rem 0 0;
  background: #450a0a;
}

.typst-display {
  margin: 1.5rem 0;
  padding: 0;
  overflow-x: auto;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  text-align: center;
}

.typst-canvas {
  min-width: max-content;
  display: flex;
  justify-content: center;
  padding: 0.25rem 0;
  border-radius: 0;
  background: transparent;
}

.typst-canvas svg {
  max-width: 100%;
  height: auto;
  background: transparent !important;
}

.typst-error {
  color: #991b1b;
  background: linear-gradient(135deg, #fff7f7, #fff);
  border-color: #fecaca;
  text-align: left;
}

.typst-error pre {
  margin: 0.75rem 0 0;
  background: #450a0a;
}

.diagram-html-display {
  margin: 1.5rem 0;
  overflow-x: auto;
}

.diagram-html-canvas {
  min-width: 0;
  width: 100%;
  padding: 0.25rem 0;
}

.diagram-html-canvas svg {
  display: block;
  max-width: 100%;
  height: auto;
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
  .container-with-sidebar {
    max-width: 100%;
  }
  .doc-workspace {
    display: block;
  }
  .doc-pane {
    position: static;
    height: auto;
    max-height: none;
    border: 1px solid var(--color-border);
    margin: 1rem;
  }
  .doc-outline {
    display: none;
  }
  .doc-resizer {
    display: none;
  }
  .doc-main {
    max-width: 100%;
    padding: 1.5rem 1rem 3rem;
  }
  .doc-outline-toggle {
    display: none;
  }
  h1 { font-size: 1.75rem; }
  h2 { font-size: 1.35rem; }
}

@media print {
  .container { max-width: 100%; padding: 0; }
  .doc-workspace { display: block; }
  .doc-sidebar,
  .doc-outline,
  .doc-resizer,
  .doc-outline-toggle { display: none; }
  .doc-main { max-width: 100%; padding: 0; }
  .doc-panel { display: block; }
  .doc-section + .doc-section {
    margin-top: 2rem;
    padding-top: 2rem;
  }
  pre { white-space: pre-wrap; word-break: break-all; }
  a { color: var(--color-text); }
}
"#;

struct RenderedDocument {
    html: String,
    section_count: usize,
}

struct RenderResources {
    ss: SyntaxSet,
    ts: ThemeSet,
    font_dir: String,
}

fn prepare_render_resources(args: &CliArgs) -> Result<RenderResources> {
    let font_dir = find_katex_fonts(args.katex_fonts.as_deref())?;
    eprintln!("Using KaTeX fonts from: {}", font_dir);
    Ok(RenderResources {
        ss: SyntaxSet::load_defaults_newlines(),
        ts: ThemeSet::load_defaults(),
        font_dir,
    })
}

fn render_document(args: &CliArgs, title_hint: Option<&Path>) -> Result<RenderedDocument> {
    let resolved = resolve_inputs(args)?;
    let resources = prepare_render_resources(args)?;
    render_document_from_files(args, title_hint, &resources, &resolved.files)
}

fn render_document_with_resources(
    args: &CliArgs,
    title_hint: Option<&Path>,
    resources: &RenderResources,
) -> Result<RenderedDocument> {
    let resolved = resolve_inputs(args)?;
    render_document_from_files(args, title_hint, resources, &resolved.files)
}

fn render_document_from_files(
    args: &CliArgs,
    title_hint: Option<&Path>,
    resources: &RenderResources,
    input_files: &[PathBuf],
) -> Result<RenderedDocument> {
    let mut sections: Vec<RenderedSection> = Vec::new();
    let mut nav_labels: Vec<String> = Vec::new();
    let mut doc_title = args.title.clone().unwrap_or_default();

    for input_path in input_files {
        let base_dir = input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let source = std::fs::read_to_string(input_path)
            .with_context(|| format!("Cannot read {}", input_path.display()))?;

        let section = render_markdown(
            &source,
            &base_dir,
            args.math_font_size,
            &resources.font_dir,
            &resources.ss,
            &resources.ts,
        )
        .with_context(|| format!("Failed to render {}", input_path.display()))?;

        if doc_title.is_empty() && !section.title.is_empty() {
            doc_title = section.title.clone();
        }

        nav_labels.push(section_label(input_path));
        sections.push(section);
    }

    if doc_title.is_empty() {
        doc_title = title_hint
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .or_else(|| {
                input_files
                    .first()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
            })
            .unwrap_or("Document")
            .to_string();
    }

    let section_count = sections.len();
    let icon_label = resolve_icon_label(args, input_files);
    let html = build_html_with_nav(&doc_title, &sections, &icon_label, Some(&nav_labels));

    Ok(RenderedDocument {
        html,
        section_count,
    })
}

fn write_document(args: &CliArgs, output: &Path, title_hint: Option<&Path>) -> Result<usize> {
    let document = render_document(args, title_hint)?;
    std::fs::write(output, document.html.as_bytes())
        .with_context(|| format!("Cannot write {}", output.display()))?;
    Ok(document.section_count)
}

fn run_convert(args: &CliArgs) -> Result<()> {
    let output = args
        .output
        .as_deref()
        .context("Missing required output. Pass --output <FILE>.")?;
    let section_count = write_document(args, output, Some(output))?;

    eprintln!(
        "Written {} section(s) -> {}",
        section_count,
        output.display()
    );

    Ok(())
}

fn build_preview_error_html(err: &anyhow::Error) -> String {
    let message = html_escape(&format!("{err:#}"));
    let body = format!(
        r#"<div class="callout callout-warning" role="alert">
<p><strong>Preview render failed</strong></p>
<pre><code>{message}</code></pre>
<p>Fix the source file and save again. The preview will update automatically.</p>
</div>"#
    );
    build_html(
        "Preview Error",
        &[RenderedSection {
            title: "Preview Error".to_string(),
            html: body,
            outline: Vec::new(),
        }],
        "ER",
    )
}

fn run_view(args: &ViewArgs) -> Result<()> {
    let export_path = args.export.clone().or_else(|| args.convert.output.clone());

    let resolved = resolve_inputs(&args.convert)?;
    view::validate_inputs(&resolved.files)?;

    let resources = prepare_render_resources(&args.convert)?;
    let convert = args.convert.clone();
    let title_hint = resolved.files.first().cloned();

    let sources: Vec<(PathBuf, String)> = resolved
        .files
        .iter()
        .map(|input| {
            let source = fs::read_to_string(input)
                .with_context(|| format!("Cannot read {}", input.display()))?;
            Ok((input.clone(), source))
        })
        .collect::<Result<_>>()?;

    let mut watch_paths = view::collect_initial_watch_paths(&resolved.files, &sources);
    watch_paths.extend(resolved.directories.clone());

    view::run(
        view::ViewOptions {
            host: args.host.clone(),
            port: args.port,
            inputs: resolved.files.clone(),
            watch_paths,
            open_browser: !args.no_open,
            export_path,
        },
        move |request: view::RenderRequest| match render_document_with_resources(
            &convert,
            title_hint.as_deref(),
            &resources,
        ) {
            Ok(document) => {
                let _current_preview_inputs = request.inputs;
                let extra_watch_paths = match resolve_inputs(&convert)
                    .and_then(|resolved| view::discover_watch_paths(&resolved.files))
                {
                    Ok(paths) => paths,
                    Err(err) => {
                        eprintln!("Resource watch discovery warning: {err:#}");
                        Vec::new()
                    }
                };
                view::RenderResult::Ok {
                    html: document.html,
                    extra_watch_paths,
                }
            }
            Err(err) => {
                eprintln!("Render error: {err:#}");
                view::RenderResult::Err {
                    html: build_preview_error_html(&err),
                }
            }
        },
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::View(args)) => run_view(&args),
        None => run_convert(&cli.args),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn render_html_at(source: &str, base_dir: &Path) -> String {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        render_markdown(source, base_dir, 16.0, "", &ss, &ts)
            .unwrap()
            .html
    }

    fn render_html(source: &str) -> String {
        render_html_at(source, Path::new("."))
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pagemd-{name}-{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn math_inline_count(html: &str) -> usize {
        html.matches("class=\"math-inline\"").count()
    }

    fn mermaid_count(html: &str) -> usize {
        html.matches("class=\"mermaid-display\"").count()
    }

    fn plantuml_count(html: &str) -> usize {
        html.matches("plantuml-display").count()
    }

    fn typst_count(html: &str) -> usize {
        html.matches("class=\"typst-display\"").count()
    }

    fn diagram_html_count(html: &str) -> usize {
        html.matches("class=\"diagram-html-display\"").count()
    }

    fn callout_count(html: &str) -> usize {
        html.matches("class=\"callout callout-").count()
    }

    fn test_args(inputs: Vec<PathBuf>, directories: Vec<PathBuf>) -> CliArgs {
        CliArgs {
            inputs,
            directories,
            output: None,
            title: None,
            icon: None,
            math_font_size: 16.0,
            katex_fonts: None,
        }
    }

    #[test]
    fn parse_icon_arg_validates_and_uppercases() {
        assert_eq!(parse_icon_arg("ab").unwrap(), "AB");
        assert_eq!(parse_icon_arg("x9").unwrap(), "X9");
        assert!(parse_icon_arg("a").is_err());
        assert!(parse_icon_arg("abc").is_err());
        assert!(parse_icon_arg("a!").is_err());
        assert!(parse_icon_arg("中文").is_err());
    }

    #[test]
    fn default_icon_label_from_path_rules() {
        assert_eq!(default_icon_label_from_path(Path::new("readme.md")), "RE");
        assert_eq!(default_icon_label_from_path(Path::new("a.md")), "AA");
        assert_eq!(default_icon_label_from_path(Path::new("my-doc.md")), "MY");
        assert_eq!(default_icon_label_from_path(Path::new("笔记.md")), "PG");
    }

    #[test]
    fn build_html_embeds_two_char_favicon() {
        let html = build_html(
            "Title",
            &[RenderedSection {
                title: String::new(),
                html: "<p>x</p>".to_string(),
                outline: Vec::new(),
            }],
            "ab",
        );
        assert!(html.contains("rel=\"icon\""));
        assert!(html.contains("data:image/svg+xml,"));
        assert!(html.contains("AB</text>") || html.contains("AB%3C/text"));
        assert!(html.contains("rx='7'") || html.contains("rx=%277%27"));
    }

    #[test]
    fn directory_inputs_collect_markdown_and_dedup_files() {
        let dir = temp_test_dir("dir-inputs");
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let first = dir.join("a.md");
        let second = nested.join("b.markdown");
        let ignored = nested.join("c.txt");
        std::fs::write(&first, "# A").unwrap();
        std::fs::write(&second, "# B").unwrap();
        std::fs::write(&ignored, "# C").unwrap();

        let args = test_args(vec![first.clone()], vec![dir.clone(), dir.clone()]);
        let resolved = resolve_inputs(&args).unwrap();

        assert_eq!(resolved.files.len(), 2);
        assert!(resolved.files.iter().any(|path| path.ends_with("a.md")));
        assert!(resolved
            .files
            .iter()
            .any(|path| path.ends_with("b.markdown")));
        assert!(!resolved.files.iter().any(|path| path.ends_with("c.txt")));
        assert_eq!(resolved.directories.len(), 1);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn multi_file_html_includes_standalone_sidebar() {
        let html = build_html_with_nav(
            "Title",
            &[
                RenderedSection {
                    title: "A".to_string(),
                    html: "<h1>A</h1>".to_string(),
                    outline: vec![HeadingOutline {
                        level: 1,
                        id: "a".to_string(),
                        text: "A".to_string(),
                    }],
                },
                RenderedSection {
                    title: "B".to_string(),
                    html: "<h1>B</h1>".to_string(),
                    outline: vec![HeadingOutline {
                        level: 1,
                        id: "b".to_string(),
                        text: "B".to_string(),
                    }],
                },
            ],
            "PG",
            Some(&["a.md".to_string(), "b.md".to_string()]),
        );

        assert!(html.contains("data-doc-workspace"));
        assert!(html.contains("class=\"doc-sidebar doc-pane\""));
        assert!(html.contains("data-doc-target=\"doc-1\""));
        assert!(html.contains("class=\"doc-nav-label\""));
        assert!(html.contains("class=\"doc-nav-copy\""));
        assert!(html.contains("data-copy-label=\"a.md\""));
        assert!(html.contains("navigator.clipboard"));
        assert!(html.contains("fallbackCopyText"));
        assert!(html.contains("activeDoc"));
        assert!(html.contains("data-doc-panel"));
        assert!(html.contains("class=\"doc-outline"));
        assert!(html.contains("data-heading-target=\"a\""));
        assert!(html.contains("PageMDActivateDocumentFromHash"));
    }

    #[test]
    fn duplicate_heading_ids_are_unique_for_outline_links() {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let section = render_markdown(
            "# Repeat\n\n## Repeat\n\n# Repeat\n",
            Path::new("."),
            16.0,
            "",
            &ss,
            &ts,
        )
        .unwrap();

        assert!(section.html.contains("id=\"repeat\""));
        assert!(section.html.contains("id=\"repeat-2\""));
        assert!(section.html.contains("id=\"repeat-3\""));
        assert_eq!(
            section
                .outline
                .iter()
                .map(|heading| heading.id.as_str())
                .collect::<Vec<_>>(),
            vec!["repeat", "repeat-2", "repeat-3"]
        );
    }

    #[test]
    fn icon_colors_are_deterministic_and_readable() {
        let bg1 = icon_background_rgb("BX");
        let bg2 = icon_background_rgb("BX");
        assert_eq!(bg1, bg2);
        assert_ne!(icon_background_rgb("AB"), icon_background_rgb("BX"));

        for label in ["AB", "BX", "PG", "Z9", "00", "XY"] {
            let (bg, fg) = icon_colors(label);
            let bg_l = relative_luminance(bg);
            let fg_l = relative_luminance(fg);
            let (hi, lo) = if bg_l > fg_l {
                (bg_l, fg_l)
            } else {
                (fg_l, bg_l)
            };
            let ratio = contrast_ratio(hi, lo);
            assert!(
                ratio >= 4.5,
                "label {label} contrast {ratio:.2} bg={bg:?} fg={fg:?}"
            );
        }
    }

    #[test]
    fn currency_sentence_with_cjk_text_stays_plain() {
        let html = render_html("（$21 发行价，融资 $2.08 亿）");
        assert!(html.contains("<p>（$21 发行价，融资 $2.08 亿）</p>"));
        assert_eq!(math_inline_count(&html), 0);
    }

    #[test]
    fn currency_and_bold_currency_do_not_merge_into_math() {
        let html = render_html("但合并营业利润因 $738M 减值几乎归零；OCF **$710M**");
        assert!(html.contains("$738M"));
        assert!(html.contains("<strong>$710M</strong>"));
        assert_eq!(math_inline_count(&html), 0);
    }

    #[test]
    fn currency_range_stays_plain() {
        let html = render_html("品牌溢价（售价 $40–$60）");
        assert!(html.contains("<p>品牌溢价（售价 $40–$60）</p>"));
        assert_eq!(math_inline_count(&html), 0);
    }

    #[test]
    fn eps_sequence_with_arrows_stays_plain() {
        let html = render_html("（EPS：$8.71→$12.79→$15.88）");
        assert!(html.contains("<p>（EPS：$8.71→$12.79→$15.88）</p>"));
        assert_eq!(math_inline_count(&html), 0);
    }

    #[test]
    fn actual_inline_and_display_math_still_render() {
        let html = render_html("真公式 $x+y$\n\n**$x+y$**\n\n$$x+y$$");
        assert_eq!(math_inline_count(&html), 2);
        assert_eq!(html.matches("class=\"math-display\"").count(), 1);
        assert!(html.contains("<strong><span class=\"math-inline\">"));
    }

    #[test]
    fn mermaid_code_block_renders_svg() {
        let html = render_html("```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n");
        assert_eq!(mermaid_count(&html), 1);
        assert!(html.contains("<svg"));
        assert!(!html.contains("language-mermaid"));
    }

    #[test]
    fn plantuml_code_block_renders_self_contained_output() {
        let html = render_html("```plantuml\n@startuml\nAlice -> Bob: Hi\n@enduml\n```\n");
        assert_eq!(plantuml_count(&html), 1);
        assert!(!html.contains("https://www.plantuml.com/plantuml/svg/"));
        assert!(html.contains("<svg") || html.contains("PlantUML render failed"));
        assert!(!html.contains("language-plantuml"));
    }

    #[test]
    fn typst_code_block_renders_svg() {
        let html = render_html(
            "```typst\n#circle(radius: 30pt, fill: blue.lighten(30%))\n#text(size: 14pt)[Hello Typst]\n```\n",
        );
        assert_eq!(typst_count(&html), 1);
        assert!(html.contains("<svg") || html.contains("Typst render failed"));
        assert!(!html.contains("language-typst"));
    }

    #[test]
    fn diagram_html_code_block_renders_raw_html() {
        let html = render_html(
            "```diagram html\n<div class=\"rounded-xl bg-sky-50 p-4\">Graph node</div>\n```\n",
        );
        assert_eq!(diagram_html_count(&html), 1);
        assert!(html.contains("rounded-xl bg-sky-50 p-4"));
        assert!(html.contains("Graph node"));
        assert!(!html.contains("language-diagram"));
    }

    #[test]
    fn diagram_html_tailwind_browser_runtime_is_embedded_when_needed() {
        let section = RenderedSection {
            title: String::new(),
            html: render_html("```diagram html\n<div class=\"rounded-xl\">Node</div>\n```\n"),
            outline: Vec::new(),
        };
        let html = build_html("Title", &[section], "PG");
        assert!(html.contains("<script>"));
        assert!(html.contains("tailwind"));
        assert!(html.contains("diagram-html-display"));
    }

    #[test]
    fn bundled_typst_packages_are_embedded() {
        assert_eq!(typst::bundled_specs().len(), 3);
    }

    #[test]
    fn typst_cetz_package_renders_svg() {
        let html = render_html(
            "```typst\n#import \"@preview/cetz:0.3.2\"\n#cetz.canvas({\n  import cetz.draw: *\n  circle((0, 0), radius: 1)\n})\n```\n",
        );
        assert_eq!(typst_count(&html), 1);
        assert!(
            html.contains("<svg"),
            "expected cetz diagram SVG, got: {}",
            &html[..html.len().min(500)]
        );
        assert!(!html.contains("Typst render failed"));
    }

    #[test]
    fn github_callout_renders_admonition() {
        let html = render_html("> [!NOTE] Custom title\n> This is **important**.\n");
        assert_eq!(callout_count(&html), 1);
        assert!(html.contains("class=\"callout callout-note\""));
        assert!(html.contains("Custom title"));
        assert!(html.contains("<strong>important</strong>"));
        assert!(!html.contains("<blockquote>"));
    }

    #[test]
    fn fenced_admonition_renders_nested_markdown() {
        let html = render_html(":::warning Pay attention\nUse `pagemd` safely.\n:::\n");
        assert_eq!(callout_count(&html), 1);
        assert!(html.contains("class=\"callout callout-warning\""));
        assert!(html.contains("Pay attention"));
        assert!(html.contains("<code>pagemd</code>"));
    }

    #[test]
    fn local_markdown_images_are_embedded_as_data_uris() {
        let dir = temp_test_dir("local-image");
        std::fs::write(
            dir.join("tiny.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>",
        )
        .unwrap();
        let html = render_html_at("![tiny](tiny.svg)\n", &dir);
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(!html.contains("src=\"tiny.svg\""));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn raw_html_resources_are_embedded() {
        let dir = temp_test_dir("raw-html");
        std::fs::write(
            dir.join("tiny.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>",
        )
        .unwrap();
        std::fs::write(dir.join("style.css"), "body { color: #111; }").unwrap();
        let html = render_html_at(
            "<img src=\"tiny.svg\"><link rel=\"stylesheet\" href=\"style.css\"><style>.x{background:url('tiny.svg')}</style>",
            &dir,
        );
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(html.contains("data:text/css;base64,"));
        assert!(!html.contains("src=\"tiny.svg\""));
        assert!(!html.contains("href=\"style.css\""));
        assert!(!html.contains("url('tiny.svg')"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
