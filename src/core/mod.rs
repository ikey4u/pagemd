//! Markdown → self-contained HTML conversion engine.
//!
//! This module is the full PageMD rendering stack (preprocess, extensions,
//! styling, and SingleFile-style embedding). The CLI is a thin front-end over
//! the same APIs.

pub mod authoring;
pub mod export;
pub mod ext;
pub mod import;
pub mod md;
pub mod model;
pub mod pipeline;
pub mod util;

pub use authoring::{diagram_help, markdown_help, DIAGRAM_HELP, MARKDOWN_HELP};
pub use export::html::{build_html, workspace_script_tag};
pub use export::html::{
    FootnoteDisplay, HtmlExportOptions, ScriptEmbed, ThemeMode, WorkspaceChrome,
};
pub use export::ExportOutput;
pub use export::OutputFormat;
pub use md::{normalize_footnote_definition_lines, ExtractedFootnote};
pub use pipeline::{RenderResources, ResolvedInputs};
pub use util::html_escape;

use std::path::{Path, PathBuf};

use anyhow::Result;

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
    pipeline::export_with_resources(opts, html_opts, resources, &resolved.files, title_hint)
}

/// Resolve `--input` / `--dir` / `--exclude` into a deduplicated file list.
pub fn resolve_inputs(opts: &ConvertOptions) -> Result<ResolvedInputs> {
    pipeline::resolve_inputs(opts)
}

pub use ext::typst::PAGEMD_LONG_ABOUT;
