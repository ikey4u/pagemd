//! PageMD's independent native renderer. The shared PageMD document is converted
//! directly into GPUI elements; GPUI Kit's Markdown TextView is never invoked.
mod extensions;
mod geometry;
pub use geometry::{text_index_at_point, text_range_bounds};
mod openapi;
pub use openapi::{render_openapi_document, OpenApiPlugin, OpenApiReader};
mod html;
mod model;
mod paragraph;
pub use paragraph::prepaint_link_tooltips;
mod state;
mod view;
pub use extensions::*;
pub use gpui_kit::base::text::TextViewStyle as MarkdownStyle;
use gpui_kit::{
    App, AppContext as _, Context, ElementId, Entity, SharedString,
};
pub use model::RenderedText;
pub use pagemd_parser::{ParsedDocument, Renderer};
pub use state::{
    MarkdownViewState, RangeHighlight, RangeHighlightError, SelectionFormat,
};
pub use view::{CodeBlockOptions, MarkdownView};

#[derive(Default)]
pub struct GpuiRenderer;
impl GpuiRenderer {
    pub fn parse_options() -> pagemd_parser::ParseOptions {
        let mut options = pagemd_parser::ParseOptions::gfm();
        options.constructs.math_flow = true;
        options.constructs.math_text = true;
        options
    }
}
impl Renderer for GpuiRenderer {
    type Output = Entity<MarkdownViewState>;
    type Error = std::convert::Infallible;
    type Context<'a> = &'a mut App;
    fn render<'a>(
        &mut self,
        document: &ParsedDocument,
        cx: Self::Context<'a>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(cx.new(|cx| MarkdownViewState::from_document(document, cx)))
    }
}
pub fn render(source: &str, cx: &mut App) -> Entity<MarkdownViewState> {
    let document = pagemd_parser::parse(source, &GpuiRenderer::parse_options())
        .expect("built-in parsing options are valid");
    GpuiRenderer.render(&document, cx).unwrap()
}
pub fn markdown_state(
    source: &str,
    cx: &mut Context<MarkdownViewState>,
) -> MarkdownViewState {
    let document = pagemd_parser::parse(source, &GpuiRenderer::parse_options())
        .expect("built-in parsing options are valid");
    MarkdownViewState::from_document(&document, cx)
}
pub fn markdown(
    id: impl Into<ElementId>,
    source: impl Into<SharedString>,
) -> MarkdownView {
    MarkdownView::markdown(id, source)
}

#[cfg(test)]
mod tests;
