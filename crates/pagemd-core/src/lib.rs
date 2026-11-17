//! PageMD core library: convert Markdown into a self-contained, styled HTML document.
//!
//! The full rendering stack (callouts, footnotes, syntax highlighting, math,
//! Mermaid/PlantUML diagrams, asset inlining, and embedded CSS) lives in
//! this crate. The `pagemd` CLI is a thin front-end over the same APIs.
//!
//! # Quick start
//!
//! ```no_run
//! use pagemd_core::{render_to_html, RenderOptions};
//!
//! let html = render_to_html("# Hello\n\nFrom the library.", &RenderOptions::default())?;
//! std::fs::write("out.html", html)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! File-based conversion (same path as the CLI's `pagemd -i … -o …`) uses
//! [`ConvertOptions`] with [`export_to_file`] or [`export_with_resources`].
//!
//! For LLM / host prompts that should author PageMD-flavored Markdown, call
//! [`markdown_help`] (full short cheat-sheet) or [`diagram_help`] (figures only;
//! includes `diagram html` + Tailwind). Prefer these over CLI [`PAGEMD_LONG_ABOUT`].
//!
//! Host apps that embed HTML in a sandboxed iframe can use
//! [`HtmlExportOptions::embedded`] (content-only, no scripts, host-owned theme,
//! [`FootnoteDisplay::Tooltip`] so end-note dumps stay hidden without footnote JS).
//! For a citations dialog owned by the host, set [`FootnoteDisplay::Host`] and read
//! [`ExportOutput::footnotes`] from [`render`].
//!
//! ```no_run
//! use pagemd_core::{render_to_html, HtmlExportOptions, RenderOptions};
//!
//! let html = render_to_html(
//!     "# Hi\n\nBody[^1].\n\n[^1]: note",
//!     &RenderOptions {
//!         html: HtmlExportOptions::embedded(),
//!         ..Default::default()
//!     },
//! )?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod authoring;
mod cli_help;
pub mod export;
pub mod ext;
pub mod import;
pub mod md;
pub mod model;
pub mod pipeline;
pub mod util;

use std::path::{Path, PathBuf};

use anyhow::Result;
pub use authoring::{diagram_help, markdown_help, DIAGRAM_HELP, MARKDOWN_HELP};
pub use export::{
    html::{
        build_html, workspace_script_tag, FootnoteDisplay, HtmlExportOptions,
        ScriptEmbed, ThemeMode, WorkspaceChrome,
    },
    ExportOutput, OutputFormat,
};
pub use ext::math::install_katex_fonts;
pub use md::{normalize_footnote_definition_lines, ExtractedFootnote};
pub use pipeline::{render_file_section, RenderResources, ResolvedInputs};
pub use util::html_escape;

/// Options for converting Markdown files or directories into HTML.
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub inputs: Vec<PathBuf>,
    pub directories: Vec<PathBuf>,
    pub excludes: Vec<String>,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub math_font_size: f64,
    pub katex_fonts: Option<PathBuf>,
    #[allow(dead_code)]
    pub output_format: OutputFormat,
    /// When true, emit Mermaid source for browser-side mermaid.js (view mode).
    pub client_mermaid: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            directories: Vec::new(),
            excludes: Vec::new(),
            title: None,
            icon: None,
            math_font_size: 16.0,
            katex_fonts: None,
            output_format: OutputFormat::Html,
            client_mermaid: false,
        }
    }
}

/// Options for rendering an in-memory Markdown string into HTML.
///
/// Defaults match the CLI convert path: full styling, workspace script, native
/// Mermaid/math/diagram rendering (not the live-preview client runtime).
#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub math_font_size: f64,
    pub katex_fonts: Option<PathBuf>,
    /// Base directory used to resolve relative images and local assets.
    pub base_dir: PathBuf,
    /// When true, emit Mermaid source for browser-side mermaid.js.
    pub client_mermaid: bool,
    pub html: HtmlExportOptions,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            title: None,
            icon: None,
            math_font_size: 16.0,
            katex_fonts: None,
            base_dir: PathBuf::from("."),
            client_mermaid: false,
            html: HtmlExportOptions::default(),
        }
    }
}

/// Render Markdown source into a complete self-contained HTML document.
pub fn render(source: &str, opts: &RenderOptions) -> Result<ExportOutput> {
    pipeline::export_source(source, opts)
}

/// Convenience wrapper around [`render`] that returns only the HTML string.
pub fn render_to_html(source: &str, opts: &RenderOptions) -> Result<String> {
    Ok(render(source, opts)?.html)
}

/// Convert Markdown inputs to HTML and write the result to `output`.
pub fn export_to_file(
    opts: &ConvertOptions,
    html_opts: &HtmlExportOptions,
    output: &Path,
) -> Result<ExportOutput> {
    pipeline::export_to_file(opts, html_opts, output)
}

/// Load syntax themes and resolve KaTeX font directories once for repeated renders.
pub fn prepare_resources(opts: &ConvertOptions) -> Result<RenderResources> {
    pipeline::prepare_resources(opts)
}

/// Convert already-resolved Markdown files using preloaded [`RenderResources`].
pub fn export_with_resources(
    opts: &ConvertOptions,
    html_opts: &HtmlExportOptions,
    resources: &RenderResources,
    title_hint: Option<&Path>,
) -> Result<ExportOutput> {
    let resolved = pipeline::resolve_inputs(opts)?;
    pipeline::export_with_resources(
        opts,
        html_opts,
        resources,
        &resolved.files,
        title_hint,
    )
}

/// Resolve `--input` / `--dir` / `--exclude` into a deduplicated file list.
pub fn resolve_inputs(opts: &ConvertOptions) -> Result<ResolvedInputs> {
    pipeline::resolve_inputs(opts)
}

/// Headless Mermaid engine shared by static HTML and embedding applications.
pub use bezel_mermaid as mermaid;
pub use cli_help::PAGEMD_LONG_ABOUT;
/// Built-in complete HTML renderer.
pub use md::{FootnoteRegistry, HtmlRenderContext, HtmlRenderer};
/// Built-in native GPUI renderer (enable the `gpui` feature).
#[cfg(feature = "gpui")]
pub use pagemd_gpui as gpui;
/// Display-only OpenAPI projection shared by HTML, native and web hosts.
pub use pagemd_openapi as openapi;
/// Shared Markdown parsing and backend-neutral renderer interface.
pub use pagemd_parser as parser;
pub use pagemd_parser::{parse as analyze_markdown, ParsedDocument, Renderer};
/// Figure rendering with titles returned separately, outside the image.
pub mod mermaid_figure;

/// Browser scripts that exported HTML embeds, for hosts (such as the CLI's
/// live preview server) that serve them on their own.
pub mod scripts {
    pub use crate::export::html::{
        page::{DIAGRAM_HTML_TAILWIND_BROWSER_JS, MERMAID_BROWSER_JS},
        workspace::WORKSPACE_SCRIPT,
    };
}
