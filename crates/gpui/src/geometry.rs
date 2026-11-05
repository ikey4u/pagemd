//! Geometry shared by native selection, highlights and link tooltips.
use std::ops::Range;

use gpui_kit::*;

/// The glyph rows covered by a UTF-8 range in an already prepainted layout.
/// Soft-wrap carets have upstream affinity; glyph backgrounds must instead
/// start on the row where the character is actually painted.
pub fn text_range_bounds(
    layout: &TextLayout,
    text: &str,
    range: Range<usize>,
) -> Vec<Bounds<Pixels>> {
    let mut rows: Vec<Bounds<Pixels>> = Vec::new();
    let Some(text) = text.get(range.clone()) else {
        return rows;
    };
    let height = layout.line_height();
    for (offset, ch) in text.char_indices() {
        if ch == '\n' || ch == '\r' {
            continue;
        }
        let index = range.start + offset;
        let (Some(mut start), Some(end)) = (
            layout.position_for_index(index),
            layout.position_for_index(index + ch.len_utf8()),
        ) else {
            continue;
        };
        if layout.index_for_position(start + point(px(0.1), height / 2.))
            != Ok(index)
        {
            start = point(layout.bounds().left(), start.y + height);
        }
        if start.y != end.y || end.x <= start.x {
            continue;
        }
        match rows.last_mut() {
            Some(last)
                if last.top() == start.y
                    && (last.right() - start.x).abs() < px(1.) =>
            {
                last.size.width = end.x - last.left();
            }
            _ => rows.push(Bounds::new(start, size(end.x - start.x, height))),
        }
    }
    rows
}

/// Snap a pointer to the nearest UTF-8 character boundary in prepainted text.
pub fn text_index_at_point(
    layout: &TextLayout,
    text: &str,
    position: Point<Pixels>,
) -> usize {
    let bounds = layout.bounds();
    if position.y < bounds.top() {
        return 0;
    }
    if position.y >= bounds.bottom() {
        return text.len();
    }
    let index = match layout.index_for_position(position) {
        Ok(index) => index.min(text.len()),
        Err(index) => return index.min(text.len()),
    };
    // The platform's closest-index helper jumps to EOF throughout the
    // final glyph. Hit the glyph first and snap against its midpoint.
    let Some(ch) = text.get(index..).and_then(|s| s.chars().next()) else {
        return index;
    };
    let next = index + ch.len_utf8();
    match text_range_bounds(layout, text, index..next).first() {
        Some(glyph) if position.x >= glyph.center().x => next,
        _ => index,
    }
}

pub(crate) fn selection_range(
    layout: &TextLayout,
    text: &str,
    anchor: Point<Pixels>,
    cursor: Point<Pixels>,
) -> Option<Range<usize>> {
    let (a, b) = (
        text_index_at_point(layout, text, anchor),
        text_index_at_point(layout, text, cursor),
    );
    (a != b).then_some(a.min(b)..a.max(b))
}
