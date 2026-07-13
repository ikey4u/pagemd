use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct HeadingOutline {
    pub level: u32,
    pub id: String,
    /// Visible heading text from markdown events (not HTML).
    /// Callers that embed this into HTML must escape it themselves.
    pub text: String,
}

pub(crate) struct Section {
    pub title: String,
    pub html: String,
    pub outline: Vec<HeadingOutline>,
}

pub(crate) struct Document {
    pub title: String,
    pub icon_label: String,
    pub sections: Vec<Section>,
    pub nav_labels: Vec<String>,
    pub input_paths: Vec<PathBuf>,
}

// Compatibility aliases for tests and transitional code.
pub(crate) type RenderedSection = Section;
