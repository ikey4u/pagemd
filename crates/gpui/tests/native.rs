use core::prelude::v1::test;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use gpui_kit::{
    base::{TextSelection, TextSelectionLayer},
    component::{ActiveTheme as _, Root, Theme, ThemeMode},
    test::TestWindowExt as _,
    *,
};
use pagemd_gpui::*;

#[gpui_kit::test]
fn code_presentation_preserves_source_selection_and_wrapped_geometry(
    cx: &mut TestAppContext,
) {
    use std::{cell::Cell, rc::Rc};
    struct CodeHost {
        document: Entity<MarkdownViewState>,
        wrap: Rc<Cell<bool>>,
        numbers: Rc<Cell<bool>>,
    }
    impl Render for CodeHost {
        fn render(
            &mut self,
            _: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl IntoElement {
            let wrap = self.wrap.clone();
            let numbers = self.numbers.clone();
            let color = cx.theme().primary;
            div().w_full().child(
                MarkdownView::new(&self.document)
                    .selectable(true)
                    .code_block_options(move |_, _, _| CodeBlockOptions {
                        soft_wrap: Some(wrap.get()),
                        line_numbers: numbers.get(),
                        highlights: vec![
                            (
                                0..2,
                                HighlightStyle {
                                    color: Some(color),
                                    ..Default::default()
                                },
                            ),
                            (4..5, HighlightStyle::default()),
                        ],
                    })
                    .decorate_code_blocks(|code, _, _, _| {
                        code.id("configured-code")
                            .test_support()
                            .into_any_element()
                    }),
            )
        }
    }
    cx.update(gpui_kit::init);
    let source = "```rust\nfn 你好() { let message = \"a deliberately long line to check wrapping, selection, line numbers and source mapping remain consistent across presentation settings\"; }\n```";
    let wrap = Rc::new(Cell::new(true));
    let numbers = Rc::new(Cell::new(true));
    let mut state = None;
    let (host, cx) = cx.add_window_view(|window, cx| {
        let document = cx.new(|cx| markdown_state(source, cx));
        state = Some(document.clone());
        let host = cx.new(|_| CodeHost {
            document,
            wrap: wrap.clone(),
            numbers: numbers.clone(),
        });
        Root::new(host, window, cx)
    });
    let document = state.unwrap();
    cx.simulate_resize(size(px(360.), px(500.)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.render_frame(cx);
        let wrapped = window.find("configured-code").bounds().size.height;
        let text = document.read(cx).rendered_text();
        let start = source.find("你好").unwrap();
        let range = text.range_for_source(start..start + "你好".len()).unwrap();
        assert_eq!(&text.as_str()[range], "你好");
        document.update(cx, |document, cx| {
            document.select_all(cx);
            document.set_selection_format(SelectionFormat::Plain, cx);
            assert_eq!(
                document.selected_text(),
                source.lines().nth(1).unwrap().to_owned() + "\n"
            );
        });
        wrap.set(false);
        numbers.set(false);
        cx.notify(host.entity_id());
        window.render_frame(cx);
        assert!(window.find("configured-code").bounds().size.height < wrapped);
        assert_eq!(document.read(cx).rendered_text().source(), source);
    });
}

#[gpui_kit::test]
fn shared_document_has_native_mapping_and_source_copy(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let source = "# 示例\n\n前**「重点」**后 &amp; \\* `代码` $ x^2 $。\n\n| A | B |\n|---|---|\n|甲|乙|\n";
    let document =
        pagemd_parser::parse(source, &GpuiRenderer::parse_options()).unwrap();
    let html = pagemd_parser::HtmlFragmentRenderer::default()
        .render(&document, ())
        .unwrap();
    assert!(html.contains("<strong>「重点」</strong>"));
    let state = cx.update(|cx| GpuiRenderer.render(&document, cx).unwrap());
    cx.update(|cx| {
        state.update(cx, |v, cx| {
            let rendered = v.rendered_text();
            assert!(
                rendered.as_str().contains("前「重点」后 & * 代码 x^2。"),
                "{}",
                rendered.as_str()
            );
            for word in ["重点", "代码", "甲", "乙"] {
                let start = source.find(word).unwrap();
                let r = rendered
                    .range_for_source(start..start + word.len())
                    .unwrap();
                assert_eq!(&rendered.as_str()[r], word);
            }
            let start = source.find("&amp;").unwrap();
            let r = rendered.range_for_source(start..start + 5).unwrap();
            assert_eq!(&rendered.as_str()[r], "&");
            v.select_all(cx);
            assert_eq!(v.selected_source_range(), Some(0..source.len()));
            assert_eq!(v.selected_text(), source);
            v.set_selection_format(SelectionFormat::Plain, cx);
            assert_eq!(v.selected_text(), rendered.as_str());
        })
    });
}

#[gpui_kit::test]
fn newest_background_update_wins_and_closes_streamed_emphasis(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let state = cx.update(|cx| render("前导**示例字段（编号 ①）", cx));
    cx.update(|cx| {
        state.update(cx, |v, cx| v.push_str("**进入 Example Handler。", cx))
    });
    cx.update(|cx| {
        assert_eq!(
            state.read(cx).rendered_text().as_str(),
            "前导示例字段（编号 ①）进入 Example Handler。\n"
        )
    });
    let old = format!("{}\n\nOLD", "旧段落。\n\n".repeat(2000));
    let newest = format!("{}\n\n前**「最新」**后", "新段落。\n\n".repeat(600));
    cx.update(|cx| {
        state.update(cx, |v, cx| {
            v.set_text(&old, cx);
            v.set_text(&newest, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let text = state.read(cx).rendered_text();
        assert_eq!(text.source(), newest);
        assert!(text.as_str().ends_with("前「最新」后\n"));
        assert!(!text.as_str().contains("OLD"));
    });
}

struct Host {
    document: Entity<MarkdownViewState>,
    clicks: Arc<AtomicUsize>,
    reveal: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
}
impl Render for Host {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        let clicks = self.clicks.clone();
        let reveal = self.reveal.clone();
        div()
            .size_full()
            .p_4()
            .text_size(px(16.))
            .line_height(px(24.))
            .child(TextSelectionLayer)
            .child(
                MarkdownView::new(&self.document)
                    .selection_format(SelectionFormat::Plain)
                    .on_reveal(move |bounds, _, _| reveal.set(bounds))
                    .on_link_click(move |_, _, _, _| {
                        clicks.fetch_add(1, Ordering::Relaxed);
                    }),
            )
    }
}
#[gpui_kit::test]
fn native_gestures_copy_across_blocks_and_links_still_click(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let mut state = None;
    let clicks = Arc::new(AtomicUsize::new(0));
    let reveal = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
    let (_, cx) = cx.add_window_view(|window, cx| {
        Theme::change(ThemeMode::Light, Some(window), cx);
        let document = render("First **中文** paragraph.\n\nSecond [link](https://example.com) paragraph.", cx);
        state = Some(document.clone());
        Root::new(cx.new(|_| Host { document, clicks: clicks.clone(), reveal: reveal.clone() }), window, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let state = state.unwrap();
    cx.update(|window, cx| {
        state.read(cx).focus_handle().clone().focus(window, cx);
        window.press("cmd-a", cx);
        window.draw(cx).clear(cx);
        window.press("cmd-c", cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "First 中文 paragraph.\nSecond link paragraph.\n"
        );
        TextSelection::clear(window, cx);
        state.update(cx, |v, cx| {
            v.clear_selection(cx);
            v.reveal_range(0..1, cx).unwrap();
        });
        window.draw(cx).clear(cx);
    });
    let first = reveal.get().origin + point(px(0.5), px(12.));
    cx.update(|window, cx| {
        let len = state.read(cx).rendered_text().len();
        state.update(cx, |v, cx| v.reveal_range(len - 2..len - 1, cx).unwrap());
        window.draw(cx).clear(cx);
    });
    let last = reveal.get().origin + point(px(8.), px(12.));
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(last, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let copied = TextSelection::selected_text(window, cx);
        assert!(copied.contains("中文 paragraph."), "{copied}");
        assert!(copied.contains("Second link paragraph."), "{copied}");
        TextSelection::clear(window, cx);
        state.update(cx, |v, cx| {
            v.clear_selection(cx);
            let r = v.rendered_text().as_str().find("link").unwrap();
            v.reveal_range(r..r + 4, cx).unwrap();
        });
        window.draw(cx).clear(cx);
    });
    let link = reveal.get().origin + point(px(8.), px(12.));
    cx.simulate_mouse_down(link, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(link, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(clicks.load(Ordering::Relaxed), 1);
}
