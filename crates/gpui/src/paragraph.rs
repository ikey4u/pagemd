//! Native text shaping and mixed inline layout. Emphasis remains in one shaped
//! text run, so CJK kerning and wrapping never split at formatting boundaries.
use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};

use gpui_kit::{base::TextSelectionRun, *};

use crate::{
    model::{self, Paragraph},
    state::Frame,
    *,
};
type LinkClick = Arc<
    dyn Fn(&SharedString, &ClickEvent, &mut Window, &mut App) + Send + Sync,
>;

pub(crate) struct NativeParagraph {
    paragraph: Paragraph,
    source: Arc<str>,
    style: MarkdownStyle,
    extensions: MarkdownExtensions,
    link: Option<LinkClick>,
    image: Option<crate::view::ImageResolver>,
    highlights: Vec<RangeHighlight>,
    frame: Rc<RefCell<Frame>>,
    mono_font: SharedString,
}
impl NativeParagraph {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        paragraph: Paragraph,
        source: Arc<str>,
        style: MarkdownStyle,
        extensions: MarkdownExtensions,
        link: Option<LinkClick>,
        image: Option<crate::view::ImageResolver>,
        highlights: Vec<RangeHighlight>,
        frame: Rc<RefCell<Frame>>,
        mono_font: SharedString,
    ) -> Self {
        Self {
            paragraph,
            source,
            style,
            extensions,
            link,
            image,
            highlights,
            frame,
            mono_font,
        }
    }
    fn runs(
        &self,
        range: Range<usize>,
        typography: &TextStyle,
    ) -> Vec<TextRun> {
        self.paragraph
            .spans
            .iter()
            .filter_map(|span| {
                let part = span.range.start.max(range.start)
                    ..span.range.end.min(range.end);
                if part.is_empty() {
                    return None;
                }
                let mut style = typography.clone();
                if span.mark.bold {
                    style.font_weight = FontWeight::BOLD;
                }
                if span.mark.italic {
                    style.font_style = FontStyle::Italic;
                }
                if span.mark.strike {
                    style.strikethrough = Some(StrikethroughStyle::default());
                }
                if span.mark.link.is_some() {
                    style.color = self.style.link();
                }
                if span.mark.code {
                    style.font_family = self.mono_font.clone();
                    style = style.highlight(self.style.inline_code());
                }
                Some(style.to_run(part.len()))
            })
            .collect()
    }
    fn fragment(
        &self,
        range: Range<usize>,
        typography: &TextStyle,
    ) -> (AnyElement, TextLayout) {
        let text =
            StyledText::new(self.paragraph.text[range.clone()].to_owned())
                .with_runs(self.runs(range.clone(), typography));
        let layout = text.layout().clone();
        let links: Vec<_> =
            self.paragraph
                .spans
                .iter()
                .filter_map(|span| {
                    let r = span.range.start.max(range.start)
                        ..span.range.end.min(range.end);
                    span.mark.link.as_ref().filter(|_| !r.is_empty()).map(
                        |url| {
                            (
                                r.start - range.start..r.end - range.start,
                                url.clone(),
                            )
                        },
                    )
                })
                .collect();
        let element = NativeTextLeaf {
            text,
            range: self.paragraph.range.start + range.start
                ..self.paragraph.range.start + range.end,
            highlights: self.highlights.clone(),
            selection_color: self.style.selection(),
            frame: self.frame.clone(),
            links,
            callback: self.link.clone(),
            down: Rc::default(),
        }
        .into_any_element();
        (element, layout)
    }
}
impl IntoElement for NativeParagraph {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
pub(crate) struct ObjectLayout {
    element: Option<AnyElement>,
    range: Range<usize>,
    size: Size<Pixels>,
    baseline: Pixels,
}
#[derive(Clone)]
pub(crate) enum PlanKind {
    Text(Range<usize>),
    Object(usize),
}
#[derive(Clone)]
pub(crate) struct Plan {
    kind: PlanKind,
    origin: Point<Pixels>,
    size: Size<Pixels>,
}
pub(crate) enum ParagraphLayout {
    Text {
        element: AnyElement,
        layout: TextLayout,
    },
    Mixed {
        plans: Rc<RefCell<Vec<Plan>>>,
        objects: Vec<ObjectLayout>,
        typography: TextStyle,
    },
}
pub(crate) struct PaintedPiece {
    element: AnyElement,
    layout: Option<TextLayout>,
    range: Range<usize>,
    bounds: Bounds<Pixels>,
}
impl Element for NativeParagraph {
    type RequestLayoutState = ParagraphLayout;
    type PrepaintState = Vec<PaintedPiece>;
    fn id(&self) -> Option<ElementId> {
        Some(("pagemd-flow", self.paragraph.range.start).into())
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
    ) -> (LayoutId, ParagraphLayout) {
        let typography = window.text_style();
        let context = InlineRenderContext {
            style: typography.clone(),
            rem: window.rem_size(),
        };
        let mut objects = Vec::new();
        for object in &self.paragraph.objects {
            let presentation = self
                .extensions
                .inline(&object.node, &self.source, &context, window, cx)
                .or_else(|| match &object.node {
                    pagemd_parser::mdast::Node::Image(image) => {
                        Some(InlineElement::new(
                            img(self.image.as_ref().map_or_else(
                                || {
                                    ImageSource::from(SharedUri::from(
                                        image.url.clone(),
                                    ))
                                },
                                |resolve| {
                                    resolve(&SharedUri::from(image.url.clone()))
                                },
                            ))
                            .max_w(px(320.))
                            .max_h(px(240.)),
                        ))
                    }
                    _ => None,
                });
            if let Some(presentation) = presentation {
                let baseline = presentation.baseline;
                let mut wrapper = div().child(presentation.element);
                wrapper.style().text = typography.subtract(&Default::default());
                let mut element = wrapper.into_any_element();
                let measured = element.layout_as_root(
                    size(
                        AvailableSpace::MaxContent,
                        AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                if measured.width > px(0.) && measured.height > px(0.) {
                    objects.push(ObjectLayout {
                        element: Some(element),
                        range: object.range.clone(),
                        size: measured,
                        baseline: baseline.unwrap_or(measured.height),
                    });
                }
            }
        }
        if objects.is_empty() {
            let (mut element, layout) =
                self.fragment(0..self.paragraph.text.len(), &typography);
            let id = element.request_layout(window, cx);
            return (id, ParagraphLayout::Text { element, layout });
        }
        let plans = Rc::new(RefCell::new(Vec::new()));
        let target = plans.clone();
        let paragraph = self.paragraph.clone();
        let metrics: Vec<_> = objects
            .iter()
            .map(|o| (o.range.clone(), o.size, o.baseline))
            .collect();
        let runs = self.runs(0..paragraph.text.len(), &typography);
        let text_style = typography.clone();
        let rem = window.rem_size();
        let id = window.request_measured_layout(
            Default::default(),
            move |known, available, window, _| {
                let width = known
                    .width
                    .or(match available.width {
                        AvailableSpace::Definite(w) => Some(w),
                        _ => None,
                    })
                    .unwrap_or(px(100_000.));
                let (layout, measured) = wrap(
                    &paragraph.text,
                    &runs,
                    &metrics,
                    &text_style,
                    rem,
                    width.max(px(1.)),
                    window,
                );
                *target.borrow_mut() = layout;
                measured
            },
        );
        (
            id,
            ParagraphLayout::Mixed {
                plans,
                objects,
                typography,
            },
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut ParagraphLayout,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<PaintedPiece> {
        let mut pieces = vec![];
        match layout {
            ParagraphLayout::Text { element, layout } => {
                element.prepaint(window, cx);
                let range = self.paragraph.range.clone();
                self.frame.borrow_mut().runs.push((
                    range.clone(),
                    TextSelectionRun::new(
                        self.paragraph.text.clone(),
                        layout.clone(),
                        bounds,
                    )
                    .with_document_order(range.start as u64),
                ));
            }
            ParagraphLayout::Mixed {
                plans,
                objects,
                typography,
            } => {
                for plan in plans.borrow().iter() {
                    let (mut element, text_layout, range) = match &plan.kind {
                        PlanKind::Text(range) => {
                            let (element, layout) =
                                self.fragment(range.clone(), typography);
                            let mut wrapper =
                                div().w(plan.size.width).h(plan.size.height);
                            wrapper.style().text =
                                typography.subtract(&Default::default());
                            // These fragments are already wrapped by `wrap`.
                            // Apply nowrap after copying typography so that its
                            // whitespace refinement cannot restore wrapping.
                            wrapper = wrapper.whitespace_nowrap();
                            (
                                wrapper.child(element).into_any_element(),
                                Some(layout),
                                range.clone(),
                            )
                        }
                        PlanKind::Object(index) => {
                            let object = &mut objects[*index];
                            (
                                object.element.take().unwrap(),
                                None,
                                object.range.clone(),
                            )
                        }
                    };
                    if text_layout.is_some() {
                        element.layout_as_root(
                            size(
                                AvailableSpace::Definite(plan.size.width),
                                AvailableSpace::Definite(plan.size.height),
                            ),
                            window,
                            cx,
                        );
                    }
                    let origin = bounds.origin + plan.origin;
                    element.prepaint_at(origin, window, cx);
                    let global = self.paragraph.range.start + range.start
                        ..self.paragraph.range.start + range.end;
                    let piece_bounds = Bounds::new(origin, plan.size);
                    if let Some(layout) = &text_layout {
                        self.frame.borrow_mut().runs.push((
                            global.clone(),
                            TextSelectionRun::new(
                                self.paragraph.text[range.clone()].to_owned(),
                                layout.clone(),
                                piece_bounds,
                            )
                            .with_document_order(global.start as u64),
                        ));
                    } else {
                        self.frame
                            .borrow_mut()
                            .objects
                            .push((global.clone(), piece_bounds));
                    }
                    pieces.push(PaintedPiece {
                        element,
                        layout: text_layout,
                        range: global,
                        bounds: piece_bounds,
                    });
                }
            }
        }
        pieces
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut ParagraphLayout,
        pieces: &mut Vec<PaintedPiece>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected = self.frame.borrow().selected.clone();
        match layout {
            ParagraphLayout::Text { element, layout } => {
                let _ = layout;
                element.paint(window, cx);
            }
            ParagraphLayout::Mixed { .. } => {
                for piece in pieces {
                    if piece.layout.is_none()
                        && selected
                            .as_ref()
                            .is_some_and(|r| model::overlaps(r, &piece.range))
                    {
                        window.paint_quad(fill(
                            piece.bounds,
                            self.style.selection(),
                        ));
                    }
                    piece.element.paint(window, cx);
                }
            }
        }
    }
}
struct NativeTextLeaf {
    text: StyledText,
    range: Range<usize>,
    highlights: Vec<RangeHighlight>,
    selection_color: Hsla,
    frame: Rc<RefCell<Frame>>,
    links: Vec<(Range<usize>, SharedString)>,
    callback: Option<LinkClick>,
    down: Rc<RefCell<Option<MouseDownEvent>>>,
}
impl IntoElement for NativeTextLeaf {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for NativeTextLeaf {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;
    fn id(&self) -> Option<ElementId> {
        Some(("pagemd-text", self.range.start).into())
    }
    fn source_location(
        &self,
    ) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        if let Some(id) = id {
            self.down = window.with_element_state(
                id,
                |retained: Option<Rc<RefCell<Option<MouseDownEvent>>>>, _| {
                    let down = retained.unwrap_or_default();
                    (down.clone(), down)
                },
            );
        }
        self.text.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        // Prepaint the text directly, including offscreen measurement views.
        // A container may cull its children; reveal still needs their glyph geometry.
        self.text.prepaint(id, inspector, bounds, state, window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.text.layout();
        let _ = layout.paint_background(window, cx);
        for highlight in &self.highlights {
            paint_range(
                layout,
                self.range.clone(),
                highlight.range(),
                highlight.background(),
                window,
            );
        }
        if let Some(range) = self.frame.borrow().selected.clone() {
            paint_range(
                layout,
                self.range.clone(),
                range,
                self.selection_color,
                window,
            );
        }
        let _ = layout.paint_foreground(window, cx);
        if !self.links.is_empty() {
            let hovered_link =
                link_at_position(layout, &self.links, window.mouse_position());
            if hovered_link.is_some() {
                window.set_cursor_style(CursorStyle::PointingHand, hitbox);
            }
            // Moving within one text hitbox does not otherwise redraw it.
            // Refresh the cursor when entering or leaving an actual link.
            let hit = hitbox.clone();
            let links = self.links.clone();
            let hover_layout = layout.clone();
            let view = window.current_view();
            let mut hovered_link = hovered_link;
            window.on_mouse_event(
                move |event: &MouseMoveEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    let updated = if hit.is_hovered(window) {
                        link_at_position(&hover_layout, &links, event.position)
                    } else {
                        None
                    };
                    if updated != hovered_link {
                        hovered_link = updated;
                        cx.notify(view);
                    }
                },
            );
            let down = self.down.clone();
            let hit = hitbox.clone();
            window.on_mouse_event(
                move |event: &MouseDownEvent, phase, window, _| {
                    if phase.bubble()
                        && event.button == MouseButton::Left
                        && hit.is_hovered(window)
                    {
                        *down.borrow_mut() = Some(event.clone());
                    }
                },
            );
            let down = self.down.clone();
            let hit = hitbox.clone();
            let links = self.links.clone();
            let callback = self.callback.clone();
            let layout = layout.clone();
            window.on_mouse_event(
                move |event: &MouseUpEvent, phase, window, cx| {
                    if !phase.bubble()
                        || event.button != MouseButton::Left
                        || !hit.is_hovered(window)
                    {
                        return;
                    }
                    let Some(down) = down.borrow_mut().take() else {
                        return;
                    };
                    let (Ok(start), Ok(end)) = (
                        layout.index_for_position(down.position),
                        layout.index_for_position(event.position),
                    ) else {
                        return;
                    };
                    if let Some((_, url)) = links.iter().find(|(range, _)| {
                        range.contains(&start) && range.contains(&end)
                    }) {
                        if let Some(callback) = &callback {
                            callback(
                                url,
                                &ClickEvent::Mouse(MouseClickEvent {
                                    down,
                                    up: event.clone(),
                                }),
                                window,
                                cx,
                            );
                        } else {
                            cx.open_url(url);
                        }
                    }
                },
            );
        }
    }
}
fn link_at_position(
    layout: &TextLayout,
    links: &[(Range<usize>, SharedString)],
    position: Point<Pixels>,
) -> Option<usize> {
    let index = layout.index_for_position(position).ok()?;
    links.iter().position(|(range, _)| range.contains(&index))
}

#[cfg(test)]
#[path = "paragraph_tests.rs"]
mod tests;

fn paint_range(
    layout: &TextLayout,
    span: Range<usize>,
    range: Range<usize>,
    color: Hsla,
    window: &mut Window,
) {
    let range = span.start.max(range.start)..span.end.min(range.end);
    if range.is_empty() {
        return;
    }
    let (Some(start), Some(end)) = (
        layout.position_for_index(range.start - span.start),
        layout.position_for_index(range.end - span.start),
    ) else {
        return;
    };
    let height = layout.line_height();
    let bounds = layout.bounds();
    if start.y == end.y {
        window.paint_quad(fill(
            Bounds::from_corners(start, point(end.x, end.y + height)),
            color,
        ));
    } else {
        window.paint_quad(fill(
            Bounds::from_corners(
                start,
                point(bounds.right(), start.y + height),
            ),
            color,
        ));
        if end.y > start.y + height {
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(bounds.left(), start.y + height),
                    point(bounds.right(), end.y),
                ),
                color,
            ));
        }
        window.paint_quad(fill(
            Bounds::from_corners(
                point(bounds.left(), end.y),
                point(end.x, end.y + height),
            ),
            color,
        ));
    }
}
fn subruns(runs: &[TextRun], range: Range<usize>) -> Vec<TextRun> {
    let mut offset = 0;
    runs.iter()
        .filter_map(|run| {
            let span = offset..offset + run.len;
            offset += run.len;
            let part = span.start.max(range.start)..span.end.min(range.end);
            if part.is_empty() {
                None
            } else {
                let mut run = run.clone();
                run.len = part.len();
                Some(run)
            }
        })
        .collect()
}
fn wrap(
    text: &str,
    runs: &[TextRun],
    objects: &[(Range<usize>, Size<Pixels>, Pixels)],
    style: &TextStyle,
    rem: Pixels,
    width: Pixels,
    window: &mut Window,
) -> (Vec<Plan>, Size<Pixels>) {
    let font_size = style.font_size.to_pixels(rem);
    let line_height = style.line_height_in_pixels(rem);
    let measure = |range: Range<usize>, window: &Window| {
        window
            .text_system()
            .shape_line(
                text[range.clone()].to_owned().into(),
                font_size,
                &subruns(runs, range),
                None,
            )
            .width
    };
    let mut plans = Vec::<Plan>::new();
    let sample = window.text_system().shape_line(
        "M".into(),
        font_size,
        &[style.to_run(1)],
        None,
    );
    let text_baseline =
        (line_height - sample.ascent - sample.descent) / 2. + sample.ascent;
    let mut row_start = 0;
    let (mut offset, mut x, mut y, mut row_height, mut max_width) =
        (0, px(0.), px(0.), line_height, px(0.));
    while offset < text.len() {
        if let Some((index, (range, object_size, _))) = objects
            .iter()
            .enumerate()
            .find(|(_, (range, _, _))| range.start == offset)
        {
            if x > px(0.) && x + object_size.width > width {
                max_width = max_width.max(x);
                y += align_row(
                    &mut plans,
                    row_start,
                    y,
                    objects,
                    text_baseline,
                    line_height,
                );
                row_start = plans.len();
                x = px(0.);
                row_height = line_height;
            }
            plans.push(Plan {
                kind: PlanKind::Object(index),
                origin: point(x, y),
                size: *object_size,
            });
            x += object_size.width;
            row_height = row_height.max(object_size.height);
            offset = range.end;
            if range.is_empty() {
                offset +=
                    text[offset..].chars().next().map_or(0, char::len_utf8);
            }
            continue;
        }
        if text[offset..].starts_with('\n') {
            max_width = max_width.max(x);
            y += align_row(
                &mut plans,
                row_start,
                y,
                objects,
                text_baseline,
                line_height,
            );
            row_start = plans.len();
            x = px(0.);
            row_height = line_height;
            offset += 1;
            continue;
        }
        let end = objects
            .iter()
            .find(|(r, _, _)| r.start > offset)
            .map_or(text.len(), |(r, _, _)| r.start);
        let end = text[offset..end].find('\n').map_or(end, |n| offset + n);
        let remaining = width - x;
        let candidates: Vec<_> =
            unicode_linebreak::linebreaks(&text[offset..end])
                .map(|(i, _)| offset + i)
                .collect();
        let mut boundary = offset;
        for &candidate in &candidates {
            if measure(offset..candidate, window) <= remaining {
                boundary = candidate;
            } else {
                break;
            }
        }
        if boundary == offset {
            if x > px(0.) {
                max_width = max_width.max(x);
                y += align_row(
                    &mut plans,
                    row_start,
                    y,
                    objects,
                    text_baseline,
                    line_height,
                );
                row_start = plans.len();
                x = px(0.);
                row_height = line_height;
                continue;
            }
            // Unbreakable words still fit their glyph advances. Preserve UTF-8 boundaries.
            for (i, ch) in text[offset..end].char_indices() {
                let next = offset + i + ch.len_utf8();
                if measure(offset..next, window) > remaining
                    && boundary > offset
                {
                    break;
                }
                boundary = next;
            }
        }
        if boundary <= offset {
            break;
        }
        let advance = measure(offset..boundary, window);
        plans.push(Plan {
            kind: PlanKind::Text(offset..boundary),
            origin: point(x, y),
            size: size(advance, line_height),
        });
        x += advance;
        offset = boundary;
        if offset < end {
            max_width = max_width.max(x);
            y += align_row(
                &mut plans,
                row_start,
                y,
                objects,
                text_baseline,
                line_height,
            );
            row_start = plans.len();
            x = px(0.);
            row_height = line_height;
        }
    }
    max_width = max_width.max(x);
    let final_height = align_row(
        &mut plans,
        row_start,
        y,
        objects,
        text_baseline,
        line_height,
    );
    let _ = row_height;
    (plans, size(max_width.min(width), y + final_height))
}

fn align_row(
    plans: &mut [Plan],
    start: usize,
    y: Pixels,
    objects: &[(Range<usize>, Size<Pixels>, Pixels)],
    text_baseline: Pixels,
    line_height: Pixels,
) -> Pixels {
    let mut above = text_baseline;
    let mut below = line_height - text_baseline;
    for plan in &plans[start..] {
        let baseline = match plan.kind {
            PlanKind::Object(index) => {
                objects[index].2.clamp(px(0.), plan.size.height)
            }
            PlanKind::Text(_) => text_baseline,
        };
        above = above.max(baseline);
        below = below.max(plan.size.height - baseline);
    }
    for plan in &mut plans[start..] {
        let baseline = match plan.kind {
            PlanKind::Object(index) => {
                objects[index].2.clamp(px(0.), plan.size.height)
            }
            PlanKind::Text(_) => text_baseline,
        };
        plan.origin.y = y + above - baseline;
    }
    above + below
}
