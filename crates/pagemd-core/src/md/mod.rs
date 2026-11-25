mod callouts;
pub(crate) mod footnotes;
mod preprocess;
mod render;
pub mod writing;

pub use footnotes::{
    normalize_footnote_definition_lines, sort_extracted_footnotes,
    ExtractedFootnote, FootnoteDisplay, FootnoteRegistry,
};
pub use render::{
    render_markdown, render_markdown_with_typography, HtmlRenderContext,
    HtmlRenderer,
};
pub use writing::{RawHtml, Resources, Writing};
