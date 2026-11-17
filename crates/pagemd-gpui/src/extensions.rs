//! Host extensions operate on PageMD syntax nodes and produce native elements.
use std::{any::Any, ops::Range, sync::Arc};

use gpui_kit::*;
use pagemd_parser::mdast::Node;

#[derive(Clone)]
pub struct MarkdownNode {
    name: SharedString,
    text: SharedString,
    markdown: SharedString,
    label: SharedString,
    range: Option<Range<usize>>,
    data: Arc<dyn Any + Send + Sync>,
}
impl MarkdownNode {
    pub fn new<T: Any + Send + Sync>(
        name: impl Into<SharedString>,
        data: T,
    ) -> Self {
        Self {
            name: name.into(),
            text: "".into(),
            markdown: "".into(),
            label: "".into(),
            range: None,
            data: Arc::new(data),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn as_text(&self) -> &str {
        &self.text
    }
    pub fn as_markdown(&self) -> &str {
        &self.markdown
    }
    pub fn source_range(&self) -> Option<Range<usize>> {
        self.range.clone()
    }
    pub fn accessibility_name(&self) -> &str {
        if self.label.is_empty() {
            &self.text
        } else {
            &self.label
        }
    }
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = text.into();
        self
    }
    pub fn markdown(mut self, text: impl Into<SharedString>) -> Self {
        self.markdown = text.into();
        self
    }
    pub fn accessibility_label(
        mut self,
        label: impl Into<SharedString>,
    ) -> Self {
        self.label = label.into();
        self
    }
    pub fn data<T: Any>(&self) -> Option<&T> {
        self.data.downcast_ref()
    }
    pub(crate) fn located(mut self, node: &Node) -> Self {
        self.range = node.position().map(|p| p.start.offset..p.end.offset);
        self
    }
}
pub struct MarkdownParseContext<'a> {
    pub(crate) source: &'a str,
}
impl<'a> MarkdownParseContext<'a> {
    pub fn source(&self) -> &'a str {
        self.source
    }
    pub fn offset(&self) -> usize {
        0
    }
    pub fn node_source(&self, node: &Node) -> Option<&'a str> {
        let p = node.position()?;
        self.source.get(p.start.offset..p.end.offset)
    }
}
pub struct InlineElement {
    pub(crate) element: AnyElement,
    pub(crate) baseline: Option<Pixels>,
}
impl InlineElement {
    pub fn new(element: impl IntoElement) -> Self {
        Self {
            element: element.into_any_element(),
            baseline: None,
        }
    }
    pub fn with_baseline(mut self, baseline: Pixels) -> Self {
        self.baseline = Some(baseline);
        self
    }
}
pub struct InlineRenderContext {
    pub(crate) style: TextStyle,
    pub(crate) rem: Pixels,
}
impl InlineRenderContext {
    pub fn text_style(&self) -> &TextStyle {
        &self.style
    }
    pub fn font_size(&self) -> Pixels {
        self.style.font_size.to_pixels(self.rem)
    }
    pub fn line_height(&self) -> Pixels {
        self.style.line_height_in_pixels(self.rem)
    }
    pub fn rem_size(&self) -> Pixels {
        self.rem
    }
}
pub trait MarkdownPlugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn is_block(&self) -> bool {
        false
    }
    fn parse(
        &self,
        node: &Node,
        cx: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode>;
    fn render(
        &self,
        node: &MarkdownNode,
        _window: &mut Window,
        _cx: &mut App,
    ) -> impl IntoElement {
        node.as_text().to_owned()
    }
    fn render_inline(
        &self,
        node: &MarkdownNode,
        _context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        Some(InlineElement::new(self.render(node, window, cx)))
    }
}
type Parse = Arc<
    dyn Fn(&Node, &MarkdownParseContext<'_>) -> Option<MarkdownNode>
        + Send
        + Sync,
>;
type RenderBlock = Arc<
    dyn Fn(&MarkdownNode, &mut Window, &mut App) -> AnyElement + Send + Sync,
>;
type RenderInline = Arc<
    dyn Fn(
            &MarkdownNode,
            &InlineRenderContext,
            &mut Window,
            &mut App,
        ) -> Option<InlineElement>
        + Send
        + Sync,
>;
#[derive(Clone, Default)]
pub struct MarkdownExtensions {
    blocks: Vec<(Parse, RenderBlock)>,
    inlines: Vec<(Parse, RenderInline)>,
}
impl MarkdownExtensions {
    pub fn plugin<P: MarkdownPlugin>(mut self, plugin: P) -> Self {
        let plugin = Arc::new(plugin);
        let parse = plugin.clone();
        let parser: Parse = Arc::new(move |node, cx| {
            parse.parse(node, cx).map(|n| n.located(node))
        });
        if plugin.is_block() {
            self.blocks.push((
                parser,
                Arc::new(move |node, window, cx| {
                    plugin.render(node, window, cx).into_any_element()
                }),
            ));
        } else {
            self.inlines.push((
                parser,
                Arc::new(move |node, context, window, cx| {
                    plugin.render_inline(node, context, window, cx)
                }),
            ));
        }
        self
    }
    pub(crate) fn block(
        &self,
        node: &Node,
        source: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let context = MarkdownParseContext { source };
        self.blocks.iter().find_map(|(parse, render)| {
            parse(node, &context).map(|node| render(&node, window, cx))
        })
    }
    pub(crate) fn inline(
        &self,
        node: &Node,
        source: &str,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let parse_context = MarkdownParseContext { source };
        self.inlines.iter().find_map(|(parse, render)| {
            parse(node, &parse_context)
                .and_then(|node| render(&node, context, window, cx))
        })
    }
}
