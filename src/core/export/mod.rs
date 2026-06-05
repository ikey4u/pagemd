pub(crate) mod html;

use anyhow::Result;

use crate::core::model::Document;

pub(crate) use html::HtmlExportOptions;

#[derive(Debug, Clone, Copy)]
pub(crate) enum OutputFormat {
    Html,
}

pub(crate) struct ExportOutput {
    pub html: String,
    #[allow(dead_code)]
    pub title: String,
    pub section_count: usize,
}

pub(crate) fn export_document(
    doc: &Document,
    format: OutputFormat,
    html_opts: &HtmlExportOptions,
) -> Result<ExportOutput> {
    match format {
        OutputFormat::Html => {
            let html = html::render_document_html(doc, html_opts);
            Ok(ExportOutput {
                html,
                title: doc.title.clone(),
                section_count: doc.sections.len(),
            })
        }
    }
}
