pub mod bundler;
pub mod favicon;
pub mod footnotes;
pub mod lightbox;
pub mod nav_tree;
pub mod page;
pub mod styles;
pub mod workspace;

pub use crate::core::md::FootnoteDisplay;
pub use favicon::resolve_icon_label;
pub use page::{
    build_html, render_document_html, section_label, HtmlExportOptions, ScriptEmbed, ThemeMode,
    WorkspaceChrome,
};
pub use workspace::workspace_script_tag;
