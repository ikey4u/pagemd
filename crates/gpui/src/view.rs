//! Render PageMD blocks with ordinary native GPUI elements.
use std::{ops::Range, rc::Rc, sync::Arc};

use gpui_kit::{
    base::{TextSelection, TextSelectionRegistration},
    component::ActiveTheme as _,
    *,
};

use crate::{
    model::{Block, Kind, Paragraph},
    paragraph::NativeParagraph,
    *,
};

type LinkClick = Arc<
    dyn Fn(&SharedString, &ClickEvent, &mut Window, &mut App) + Send + Sync,
>;
pub(crate) type ImageResolver =
    Arc<dyn Fn(&SharedUri) -> ImageSource + Send + Sync>;
type Reveal = Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App)>;
#[derive(Clone)]
pub enum ViewStyle {
    Native(MarkdownStyle),
    Component(gpui_kit::component::text::TextViewStyle),
}
impl From<MarkdownStyle> for ViewStyle {
    fn from(s: MarkdownStyle) -> Self {
        Self::Native(s)
    }
}
impl From<gpui_kit::component::text::TextViewStyle> for ViewStyle {
    fn from(s: gpui_kit::component::text::TextViewStyle) -> Self {
        Self::Component(s)
    }
}
#[derive(Clone)]
pub struct MarkdownView {
    id: ElementId,
    state: Option<Entity<MarkdownViewState>>,
    source: Option<SharedString>,
    style: StyleRefinement,
    markdown_style: Option<ViewStyle>,
    selectable: Option<bool>,
    format: Option<SelectionFormat>,
    scrollable: bool,
    extensions: MarkdownExtensions,
    link: Option<LinkClick>,
    image: Option<ImageResolver>,
    reveal: Option<Reveal>,
    max_lines: Option<usize>,
}
impl Styled for MarkdownView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl MarkdownView {
    pub fn new(state: &Entity<MarkdownViewState>) -> Self {
        let mut view = Self::markdown(state.entity_id().to_string(), "");
        view.state = Some(state.clone());
        view.source = None;
        view
    }
    pub fn markdown(
        id: impl Into<ElementId>,
        source: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            state: None,
            source: Some(source.into()),
            style: StyleRefinement::default(),
            markdown_style: None,
            selectable: None,
            format: None,
            scrollable: false,
            extensions: MarkdownExtensions::default(),
            link: None,
            image: None,
            reveal: None,
            max_lines: None,
        }
    }
    pub fn style(mut self, style: impl Into<ViewStyle>) -> Self {
        self.markdown_style = Some(style.into());
        self
    }
    pub fn selectable(mut self, value: bool) -> Self {
        self.selectable = Some(value);
        self
    }
    pub fn selection_format(mut self, format: SelectionFormat) -> Self {
        self.format = Some(format);
        self
    }
    pub fn scrollable(mut self, value: bool) -> Self {
        self.scrollable = value;
        self
    }
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines);
        self
    }
    pub fn markdown_extensions(
        mut self,
        extensions: MarkdownExtensions,
    ) -> Self {
        self.extensions = extensions;
        self
    }
    pub fn plugin<P: MarkdownPlugin>(mut self, plugin: P) -> Self {
        self.extensions = self.extensions.plugin(plugin);
        self
    }
    pub fn on_link_click(
        mut self,
        callback: impl Fn(&SharedString, &ClickEvent, &mut Window, &mut App)
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.link = Some(Arc::new(callback));
        self
    }
    pub fn image_source(
        mut self,
        resolve: impl Fn(&SharedUri) -> ImageSource + Send + Sync + 'static,
    ) -> Self {
        self.image = Some(Arc::new(resolve));
        self
    }
    pub fn on_reveal(
        mut self,
        callback: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.reveal = Some(Rc::new(callback));
        self
    }
    fn resolved_style(&self, cx: &App) -> MarkdownStyle {
        let theme = cx.theme();
        let themed = MarkdownStyle::default()
            .with_foreground(theme.foreground)
            .with_muted_foreground(theme.muted_foreground)
            .with_link(theme.link)
            .with_selection(theme.selection)
            .with_border(theme.border)
            .with_code_background(theme.muted)
            .with_inline_code(HighlightStyle {
                background_color: Some(theme.muted),
                ..Default::default()
            })
            .with_table_head(
                StyleRefinement::default()
                    .bg(theme.table_head)
                    .text_color(theme.table_head_foreground),
            )
            .with_dark(theme.is_dark());
        match &self.markdown_style {
            Some(ViewStyle::Native(style)) => style.clone(),
            Some(ViewStyle::Component(s)) => {
                let s = s.clone();
                let heading_size = s.heading_font_size.clone();
                let base = s.heading_base_font_size;
                themed
                    .with_paragraph_gap(s.paragraph_gap)
                    .with_code_block(s.code_block)
                    .with_table(s.table)
                    .with_table_head(s.table_head)
                    .with_table_cell(s.table_cell)
                    .with_inline_code(HighlightStyle {
                        background_color: s
                            .inline_code
                            .background_color
                            .or(Some(theme.muted)),
                        ..s.inline_code
                    })
                    .with_heading(move |level| {
                        let size = heading_size.as_ref().map_or_else(
                            || {
                                rems(match level {
                                    1 => 2.,
                                    2 => 1.5,
                                    3 => 1.25,
                                    _ => 1.,
                                })
                                .to_pixels(base)
                            },
                            |f| f(level, base),
                        );
                        StyleRefinement::default()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(size)
                    })
            }
            None => themed,
        }
    }
}
impl IntoElement for MarkdownView {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
pub struct ViewLayout {
    element: AnyElement,
    state: Entity<MarkdownViewState>,
    selectable: bool,
    reveal: Option<Reveal>,
}
impl Element for MarkdownView {
    type RequestLayoutState = ViewLayout;
    type PrepaintState = (Hitbox, gpui_kit::base::TouchHandleLayout);
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(
        &self,
    ) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ViewLayout) {
        let state = self.state.clone().unwrap_or_else(|| {
            let source = self.source.clone().unwrap_or_default();
            let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
                markdown_state(&source, cx)
            });
            state.update(cx, |v, cx| v.set_text(&source, cx));
            state
        });
        let style = self.resolved_style(cx);
        let v = state.read(cx);
        let selectable = self.selectable.unwrap_or(v.selectable);
        let scroll = v.scroll.clone();
        let focus = v.focus.clone();
        let mut root = div()
            .id("pagemd-document")
            .key_context("PageMdView")
            .track_focus(&focus)
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(style.paragraph_gap());
        root.style().refine(&self.style);
        if self.scrollable {
            root = root.overflow_y_scroll().track_scroll(&scroll);
        }
        if let Some(max) = self.max_lines {
            root = root
                .max_h(window.line_height() * max as f32)
                .overflow_hidden();
        }
        let blocks = v.blocks.clone();
        let source: Arc<str> = v.document.source().into();
        let frame = v.frame.clone();
        let highlights = v.highlights.clone();
        let selected = if selectable {
            frame.borrow().selected.clone()
        } else {
            None
        };
        *frame.borrow_mut() = crate::state::Frame {
            selected,
            ..Default::default()
        };
        let renderer = Blocks {
            mono_font: cx.theme().mono_font_family.clone(),
            source,
            style,
            extensions: self.extensions.clone(),
            link: self.link.clone(),
            image: self.image.clone(),
            highlights,
            frame,
        };
        for block in blocks.iter() {
            root = root.child(renderer.block(block, window, cx));
        }
        let for_copy = state.clone();
        let for_all = state.clone();
        root = root
            .on_action(
                move |_: &gpui_kit::component::input::Copy, window, cx| {
                    let selected = TextSelection::selected_text(window, cx);
                    let text = if selected.is_empty() {
                        for_copy.read(cx).selected_text()
                    } else {
                        selected
                    };
                    if !text.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                },
            )
            .on_action(
                move |_: &gpui_kit::component::input::SelectAll, _, cx| {
                    for_all.update(cx, |v, cx| v.select_all(cx));
                },
            );
        if let Some(format) = self.format {
            state.update(cx, |v, _| v.format = format);
        }
        let mut element = root.into_any_element();
        let layout = element.request_layout(window, cx);
        (
            layout,
            ViewLayout {
                element,
                state,
                selectable,
                reveal: self.reveal.clone(),
            },
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut ViewLayout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        layout.element.prepaint(window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        layout.state.update(cx, |v, cx| {
            let size_changed = v.frame.borrow().bounds.size != bounds.size;
            v.frame.borrow_mut().bounds = bounds;
            if size_changed {
                cx.notify();
            }
            if layout.selectable {
                v.refresh_selection(cx);
                let text_bounds = v
                    .frame
                    .borrow()
                    .runs
                    .iter()
                    .map(|(_, r)| r.layout().bounds())
                    .chain(v.frame.borrow().objects.iter().map(|(_, b)| *b))
                    .collect();
                let frame = v.frame.borrow();
                let mut registration =
                    TextSelectionRegistration::new(hitbox.clone(), bounds)
                        .with_text_bounds(text_bounds)
                        .with_rendered_element(&v.selection, window, cx);
                if let Some(selected) = &frame.selected {
                    let mut ends =
                        frame.runs.iter().filter_map(|(span, run)| {
                            if !crate::model::overlaps(span, selected) {
                                return None;
                            }
                            let start = run.layout().position_for_index(
                                selected.start.max(span.start) - span.start,
                            )?;
                            let end = run.layout().position_for_index(
                                selected.end.min(span.end) - span.start,
                            )?;
                            Some((
                                Bounds::new(
                                    start,
                                    size(px(1.), run.layout().line_height()),
                                ),
                                Bounds::new(
                                    end,
                                    size(px(1.), run.layout().line_height()),
                                ),
                            ))
                        });
                    if let Some((first, mut last)) = ends.next() {
                        for (_, end) in ends {
                            last = end;
                        }
                        registration =
                            registration.with_selection_edges(first, last);
                    }
                }
                drop(frame);
                v.selection.register(registration, window, cx);
            }
        });
        let handles = if layout.selectable {
            layout
                .state
                .read(cx)
                .selection
                .prepaint_touch_handles(window, cx)
        } else {
            Default::default()
        };
        finish_reveal(layout, window, cx);
        (hitbox, handles)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut ViewLayout,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        layout.element.paint(window, cx);
        if layout.selectable {
            let selection = layout.state.read(cx).selection.clone();
            selection.paint_touch_handles(
                &prepaint.1,
                cx.theme().selection,
                window,
                cx,
            );
        }
    }
}
fn finish_reveal(layout: &mut ViewLayout, window: &mut Window, cx: &mut App) {
    let reveal = layout.state.read(cx).reveal.clone();
    if let Some(range) = reveal {
        let frame = layout.state.read(cx).frame.borrow();
        let line = frame
            .runs
            .iter()
            .find_map(|(span, run)| {
                if !(crate::model::overlaps(span, &range)
                    || (range.is_empty()
                        && range.start >= span.start
                        && range.start <= span.end))
                {
                    return None;
                }
                let index = range.start.max(span.start) - span.start;
                let pos = run.layout().position_for_index(index)?;
                Some(Bounds::new(pos, size(px(1.), run.layout().line_height())))
            })
            .or_else(|| {
                frame
                    .objects
                    .iter()
                    .find(|(span, _)| {
                        crate::model::overlaps(span, &range)
                            || (range.is_empty()
                                && range.start >= span.start
                                && range.start <= span.end)
                    })
                    .map(|(_, b)| *b)
            });
        drop(frame);
        if let Some(line) = line {
            layout.state.update(cx, |v, _| v.reveal = None);
            if let Some(callback) = &layout.reveal {
                callback(line, window, cx);
            } else {
                window.request_autoscroll(line);
            }
        }
    }
}

struct Blocks {
    mono_font: SharedString,
    source: Arc<str>,
    style: MarkdownStyle,
    extensions: MarkdownExtensions,
    link: Option<LinkClick>,
    image: Option<ImageResolver>,
    highlights: Vec<RangeHighlight>,
    frame: Rc<std::cell::RefCell<crate::state::Frame>>,
}
impl Blocks {
    fn paragraph(&self, paragraph: &Paragraph) -> NativeParagraph {
        NativeParagraph::new(
            paragraph.clone(),
            self.source.clone(),
            self.style.clone(),
            self.extensions.clone(),
            self.link.clone(),
            self.image.clone(),
            self.highlights.clone(),
            self.frame.clone(),
            self.mono_font.clone(),
        )
    }
    fn block(
        &self,
        block: &Block,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        if let Some(element) =
            self.extensions.block(&block.node, &self.source, window, cx)
        {
            return AtomicBlock {
                element,
                range: block.range.clone(),
                frame: self.frame.clone(),
            }
            .into_any_element();
        }
        match &block.kind {
            Kind::Paragraph(p) => div()
                .w_full()
                .min_w_0()
                .child(self.paragraph(p))
                .into_any_element(),
            Kind::Heading(level, p) => {
                let mut d =
                    div().w_full().min_w_0().font_weight(FontWeight::SEMIBOLD);
                d.style().refine(&self.style.heading(*level));
                d.child(self.paragraph(p)).into_any_element()
            }
            Kind::Code(lang, p) => {
                let mut d = div()
                    .w_full()
                    .min_w_0()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(self.style.border().opacity(0.5))
                    .bg(self.style.code_background())
                    .p_4()
                    .font_family(self.mono_font.clone())
                    .font_weight(FontWeight::NORMAL)
                    .text_size(cx.theme().mono_font_size)
                    .line_height(relative(1.65));
                d.style().text.font_style = Some(FontStyle::Normal);
                d.style().refine(self.style.code_block());
                // Keep code text's first line at the configured padding;
                // source-line navigation uses the same native geometry.
                let _ = lang;
                d.child(self.paragraph(p)).into_any_element()
            }

            Kind::Math(p) => div()
                .w_full()
                .min_w_0()
                .font_family(self.mono_font.clone())
                .child(self.paragraph(p))
                .into_any_element(),
            Kind::Group(children) => div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(self.style.paragraph_gap())
                .children(children.iter().map(|b| self.block(b, window, cx)))
                .into_any_element(),
            Kind::Quote(children) => div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .pl_4()
                .border_l_2()
                .border_color(self.style.border())
                .children(children.iter().map(|b| self.block(b, window, cx)))
                .into_any_element(),
            Kind::List {
                ordered,
                start,
                items,
            } => {
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(items.iter().enumerate().map(
                        |(i, (checked, blocks))| {
                            div()
                                .flex()
                                .items_start()
                                .gap_2()
                                .child(div().w(px(24.)).flex_none().child(
                                    match checked {
                                        Some(true) => "☑".to_owned(),
                                        Some(false) => "☐".to_owned(),
                                        None if *ordered => {
                                            format!("{}.", start + i as u32)
                                        }
                                        _ => "•".to_owned(),
                                    },
                                ))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .children(blocks.iter().map(|b| {
                                            self.block(b, window, cx)
                                        })),
                                )
                        },
                    ))
                    .into_any_element()
            }
            Kind::Table { rows, align } => {
                let mut table = div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(self.style.border());
                table.style().refine(self.style.table());
                for (i, row) in rows.iter().enumerate() {
                    let mut d = div().w_full().min_w_0().flex();
                    if i == 0 {
                        d.style().refine(self.style.table_head());
                    }
                    for (column, p) in row.iter().enumerate() {
                        let mut cell = div()
                            .flex_1()
                            .min_w_0()
                            .px_3()
                            .py_2()
                            .border_r_1()
                            .border_b_1()
                            .border_color(self.style.border());
                        cell.style().refine(self.style.table_cell());
                        cell = match align.get(column) {
                            Some(pagemd_parser::mdast::AlignKind::Center) => {
                                cell.text_center()
                            }
                            Some(pagemd_parser::mdast::AlignKind::Right) => {
                                cell.text_right()
                            }
                            _ => cell,
                        };
                        d = d.child(cell.child(self.paragraph(p)));
                    }
                    table = table.child(d);
                }
                table.into_any_element()
            }
            Kind::Rule => div()
                .w_full()
                .h(px(1.))
                .bg(self.style.border())
                .into_any_element(),
        }
    }
}
struct AtomicBlock {
    element: AnyElement,
    range: Range<usize>,
    frame: Rc<std::cell::RefCell<crate::state::Frame>>,
}
impl IntoElement for AtomicBlock {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for AtomicBlock {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(
        &self,
    ) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.element.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.prepaint(window, cx);
        self.frame
            .borrow_mut()
            .objects
            .push((self.range.clone(), bounds));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(window, cx);
    }
}
