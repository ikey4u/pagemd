mod callouts;
pub(crate) mod footnotes;
mod preprocess;
mod render;

pub use footnotes::{
    normalize_footnote_definition_lines, sort_extracted_footnotes, ExtractedFootnote,
    FootnoteDisplay,
};
pub use render::render_markdown;
