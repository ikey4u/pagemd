//! PageMD library: convert Markdown into a self-contained, styled HTML document.
//!
//! The full rendering stack (callouts, footnotes, syntax highlighting, math,
//! Mermaid/PlantUML/Typst diagrams, asset inlining, and embedded CSS) lives in
//! [`core`]. The `pagemd` CLI is a thin front-end over the same APIs.
//!
//! # Quick start
//!
//! ```no_run
//! use pagemd::{render_to_html, RenderOptions};
//!
//! let html = render_to_html("# Hello\n\nFrom the library.", &RenderOptions::default())?;
//! std::fs::write("out.html", html)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! File-based conversion (same path as `pagemd -i … -o …`) uses
//! [`ConvertOptions`] with [`export_to_file`] or [`export_with_resources`].

pub mod core;

mod app;

/// CLI / binary entrypoint (`pagemd` executable).
pub use app::run;

pub use core::{
    build_html, export_to_file, export_with_resources, html_escape, prepare_resources, render,
    render_to_html, resolve_inputs, workspace_script_tag, ConvertOptions, ExportOutput,
    HtmlExportOptions, OutputFormat, RenderOptions, RenderResources, ResolvedInputs,
    PAGEMD_LONG_ABOUT,
};

#[cfg(test)]
mod tests;
