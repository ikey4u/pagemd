use std::path::PathBuf;

#[derive(Clone)]
pub struct HeadingOutline {
    pub level: u32,
    pub id: String,
    /// Visible heading text from markdown events (not HTML).
    /// Callers that embed this into HTML must escape it themselves.
    pub text: String,
}

pub struct Section {
    pub title: String,
    pub html: String,
    pub outline: Vec<HeadingOutline>,
    /// Footnote definitions extracted when rendering with [`crate::FootnoteDisplay::Host`].
    pub footnotes: Vec<crate::core::md::ExtractedFootnote>,
}

pub struct Document {
    pub title: String,
    pub icon_label: String,
    pub sections: Vec<Section>,
    pub nav_labels: Vec<String>,
    pub input_paths: Vec<PathBuf>,
}

// Compatibility aliases for tests and transitional code.
pub type RenderedSection = Section;
