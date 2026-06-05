pub(crate) mod export;
pub(crate) mod ext;
pub(crate) mod import;
pub(crate) mod md;
pub(crate) mod model;
pub(crate) mod pipeline;
pub(crate) mod util;

pub(crate) use export::html::{build_html, workspace_script_tag};
pub(crate) use export::ExportOutput;
pub(crate) use export::HtmlExportOptions;
pub(crate) use export::OutputFormat;
pub(crate) use pipeline::RenderResources;
pub(crate) use util::html_escape;

use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone)]
pub(crate) struct ConvertOptions {
    pub inputs: Vec<PathBuf>,
    pub directories: Vec<PathBuf>,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub math_font_size: f64,
    pub katex_fonts: Option<PathBuf>,
    #[allow(dead_code)]
    pub output_format: OutputFormat,
}

pub(crate) fn export_to_file(
    opts: &ConvertOptions,
    html_opts: &HtmlExportOptions,
    output: &Path,
) -> Result<ExportOutput> {
    pipeline::export_to_file(opts, html_opts, output)
}

pub(crate) fn prepare_resources(opts: &ConvertOptions) -> Result<RenderResources> {
    pipeline::prepare_resources(opts)
}

pub(crate) fn export_with_resources(
    opts: &ConvertOptions,
    html_opts: &HtmlExportOptions,
    resources: &RenderResources,
    title_hint: Option<&Path>,
) -> Result<ExportOutput> {
    let resolved = pipeline::resolve_inputs(opts)?;
    pipeline::export_with_resources(opts, html_opts, resources, &resolved.files, title_hint)
}

pub(crate) fn resolve_inputs(opts: &ConvertOptions) -> Result<pipeline::ResolvedInputs> {
    pipeline::resolve_inputs(opts)
}

pub use ext::typst::PAGEMD_LONG_ABOUT;
