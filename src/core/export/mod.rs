pub mod html;

use anyhow::Result;

use crate::core::model::Document;

pub use html::HtmlExportOptions;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Html,
}

pub struct ExportOutput {
    pub html: String,
    #[allow(dead_code)]
    pub title: String,
    pub section_count: usize,
    /// Footnote definitions extracted when [`HtmlExportOptions::footnotes`] is
    /// [`crate::FootnoteDisplay::Host`]; empty otherwise.
    pub footnotes: Vec<crate::core::md::ExtractedFootnote>,
}

pub fn export_document(
    doc: &Document,
    format: OutputFormat,
    html_opts: &HtmlExportOptions,
) -> Result<ExportOutput> {
    match format {
        OutputFormat::Html => {
            let html = html::render_document_html(doc, html_opts);
            let mut footnotes: Vec<_> = doc
                .sections
                .iter()
                .flat_map(|section| section.footnotes.iter().cloned())
                .collect();
            crate::core::md::footnotes::sort_extracted_footnotes(&mut footnotes);
            Ok(ExportOutput {
                html,
                title: doc.title.clone(),
                section_count: doc.sections.len(),
                footnotes,
            })
        }
    }
}
