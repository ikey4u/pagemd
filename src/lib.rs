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
//! use pagemd::{render_to_html, HtmlExportOptions, RenderOptions};
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

pub mod app;
pub mod core;

/// CLI / binary entrypoint (`pagemd` executable).
pub use app::run;

pub use core::{
    build_html, diagram_help, export_to_file, export_with_resources, html_escape, markdown_help,
    normalize_footnote_definition_lines, prepare_resources, render, render_to_html, resolve_inputs,
    workspace_script_tag, ConvertOptions, ExportOutput, ExtractedFootnote, FootnoteDisplay,
    HtmlExportOptions, OutputFormat, RenderOptions, RenderResources, ResolvedInputs, ScriptEmbed,
    ThemeMode, WorkspaceChrome, DIAGRAM_HELP, MARKDOWN_HELP, PAGEMD_LONG_ABOUT,
};
