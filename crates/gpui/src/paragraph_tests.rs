use core::prelude::v1::test;
use std::{cell::Cell, rc::Rc};

use gpui_kit::{component::Root, *};

use super::link_at_position;
use crate::{render, MarkdownView, MarkdownViewState};

struct LinkHost {
    document: Entity<MarkdownViewState>,
    renders: Rc<Cell<usize>>,
}
impl Render for LinkHost {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .w(px(220.))
            .p_2()
            .text_size(px(16.))
            .child(MarkdownView::new(&self.document))
    }
}

#[gpui_kit::test]
fn link_hover_follows_text_ranges_and_repaints_within_a_paragraph(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let renders = Rc::new(Cell::new(0));
    let mut document = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let state = render(
            "普通文字 [a long clickable link that wraps across several lines](https://example.com) 后面的文字 [第二个](https://example.org)。",
            cx,
        );
        document = Some(state.clone());
        Root::new(cx.new(|_| LinkHost {
            document: state,
            renders: renders.clone(),
        }), window, cx)
    });
    let document = document.unwrap();
    let (plain, link, gutter, outside) = cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let state = document.read(cx);
        let frame = state.frame.borrow();
        let layout = frame.runs[0].1.layout();
        let rendered = state.rendered_text();
        let text = rendered.as_str().trim_end_matches('\n');
        let first = text.find("a long").unwrap()..text.find(" 后面的").unwrap();
        let second = text.find("第二个").unwrap()..text.find('。').unwrap();
        let links = vec![
            (first.clone(), "https://example.com".into()),
            (second.clone(), "https://example.org".into()),
        ];
        let at = |index| {
            let position = layout.position_for_index(index).unwrap()
                + point(px(0.1), layout.line_height() / 2.);
            // A soft-wrap boundary has upstream affinity for caret placement;
            // hover targets need the glyph at the start of the next line.
            if layout.index_for_position(position) == Ok(index) {
                position
            } else {
                let position = point(
                    layout.bounds().left() + px(0.1),
                    position.y + layout.line_height(),
                );
                assert_eq!(layout.index_for_position(position), Ok(index));
                position
            }
        };
        for (index, _) in
            text.char_indices().filter(|(_, ch)| !ch.is_whitespace())
        {
            assert_eq!(
                link_at_position(layout, &links, at(index)),
                links.iter().position(|(range, _)| range.contains(&index)),
                "wrong hover target at {index}: {:?}",
                &text[index..],
            );
        }
        assert!(
            at(first.start).y < at(first.end - 1).y,
            "the link must wrap so continuation lines are covered",
        );
        let gutter = point(
            layout.bounds().right() - px(1.),
            at(text.len() - '。'.len_utf8()).y,
        );
        let outside =
            point(layout.bounds().left(), layout.bounds().bottom() + px(10.));
        assert_eq!(link_at_position(layout, &links, gutter), None);
        assert_eq!(link_at_position(layout, &links, outside), None);
        (at(0), at(first.start), gutter, outside)
    });
    for (step, position) in
        [plain, link, plain, link, gutter, link, outside, link]
            .into_iter()
            .enumerate()
    {
        let before = renders.get();
        cx.update(|window, cx| {
            window.dispatch_event(
                MouseMoveEvent {
                    position,
                    pressed_button: None,
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        if step > 0 {
            assert!(
                renders.get() > before,
                "hover transitions must refresh the cursor"
            );
        }
    }
}
