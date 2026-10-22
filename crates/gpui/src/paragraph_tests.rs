use core::prelude::v1::test;
use std::{cell::Cell, rc::Rc};

use gpui_kit::{component::Root, test::TestWindowExt as _, *};

use super::{link_at_position, link_bounds};
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
                links.iter().position(|(range, _)| link_bounds(
                    layout,
                    text,
                    range.clone()
                )
                .iter()
                .any(|bounds| bounds.contains(&at(index)))),
                links.iter().position(|(range, _)| range.contains(&index)),
                "tooltip geometry must follow wrapped glyphs at {index}",
            );
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
        assert!(links.iter().all(|(range, _)| link_bounds(
            layout,
            text,
            range.clone()
        )
        .iter()
        .all(|bounds| !bounds.contains(&gutter))));
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

#[gpui_kit::test]
fn wrapped_link_tooltips_show_destinations_without_changing_wrapping_or_clicks(
    cx: &mut TestAppContext,
) {
    use std::{
        cell::RefCell,
        rc::Rc,
        sync::{Arc, Mutex},
        time::Duration,
    };

    struct Host {
        document: Entity<MarkdownViewState>,
        revealed: Rc<RefCell<Option<Bounds<Pixels>>>>,
        clicked: Arc<Mutex<Vec<String>>>,
    }
    impl Render for Host {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> impl IntoElement {
            let revealed = self.revealed.clone();
            let clicked = self.clicked.clone();
            div().w(px(220.)).p_2().text_size(px(16.)).child(
                MarkdownView::new(&self.document)
                    .on_reveal(move |bounds, _, _| {
                        *revealed.borrow_mut() = Some(bounds)
                    })
                    .on_link_click(move |url, _, _, _| {
                        clicked.lock().unwrap().push(url.to_string())
                    }),
            )
        }
    }
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let revealed = Rc::new(RefCell::new(None));
    let clicked = Arc::new(Mutex::new(Vec::new()));
    let mut saved_document = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let document = crate::render("[**A long clickable link wrapping across several lines of text**](https://example.com/docs) 普通文字 [Second](</tmp/报告.md:42>)", cx);
        saved_document = Some(document.clone());
        Root::new(cx.new(|_| Host { document, revealed:revealed.clone(), clicked:clicked.clone() }), window, cx)
    });
    let document = saved_document.unwrap();
    let original = document
        .read_with(cx, |state, _| state.rendered_text().as_str().to_owned());
    let mut points = Vec::new();
    for word in ["long", "several", "普通", "Second"] {
        let index =
            original.find(word).unwrap() + if word.is_ascii() { 1 } else { 0 };
        cx.update(|window, cx| {
            document.update(cx, |document, cx| {
                document.reveal_range(index..index, cx).unwrap()
            });
            window.render_frame(cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.render_frame(cx));
        let bounds = revealed.borrow_mut().take().unwrap();
        points.push(bounds.center());
    }
    assert!(points[1].y > points[0].y, "the link must wrap across lines");
    for (position, url) in [
        (points[0], Some("https://example.com/docs")),
        (points[1], Some("https://example.com/docs")),
        (points[2], None),
        (points[3], Some("/tmp/报告.md:42")),
    ] {
        cx.simulate_mouse_move(position, None, Modifiers::default());
        cx.run_until_parked();
        cx.update(|window, cx| window.render_frame(cx));
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.render_frame(cx);
            for candidate in ["https://example.com/docs", "/tmp/报告.md:42"] {
                assert_eq!(
                    window
                        .try_find(SharedString::from(format!(
                            "markdown-link-destination:{candidate}"
                        )))
                        .is_some(),
                    url == Some(candidate)
                );
            }
        });
    }
    cx.simulate_mouse_down(points[3], MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(points[3], MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(*clicked.lock().unwrap(), vec!["/tmp/报告.md:42"]);
    assert_eq!(
        document.read_with(cx, |state, _| state
            .rendered_text()
            .as_str()
            .to_owned()),
        original
    );
}
