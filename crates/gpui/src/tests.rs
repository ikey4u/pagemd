use core::prelude::v1::test;
use std::{cell::Cell, rc::Rc};

use gpui_kit::{component::Root, *};

use crate::*;

#[gpui_kit::test]
fn native_raw_html_and_soft_breaks_keep_readable_text(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for (source, expected) in [
        ("第一行\r\n第二行 &amp; &#x4e2d; &NotEqualTilde;", "第一行 第二行 & 中 ≂̸\n"),
        ("<div><h2>HTML 标题</h2><p>Hello <strong>world</strong>.</p><script>hidden()</script></div>", "HTML 标题\nHello world.\n"),
    ] {
        let state=cx.update(|cx|render(source,cx));
        cx.update(|cx| {
            let text=state.read(cx).rendered_text();assert_eq!(text.as_str(),expected);
            if let Some(start)=source.find("&NotEqualTilde;") {
                let range=text.range_for_source(start..start+"&NotEqualTilde;".len()).unwrap();assert_eq!(&text.as_str()[range],"≂̸");
            }
        });
    }
}
struct Formula;
impl MarkdownPlugin for Formula {
    fn name(&self) -> &str {
        "native-formula"
    }
    fn parse(
        &self,
        node: &pagemd_parser::mdast::Node,
        cx: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let pagemd_parser::mdast::Node::InlineMath(math) = node else {
            return None;
        };
        Some(
            MarkdownNode::new(self.name(), ())
                .text(math.value.clone())
                .markdown(cx.node_source(node).unwrap()),
        )
    }
    fn render_inline(
        &self,
        _: &MarkdownNode,
        context: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        assert_eq!(context.font_size(), px(16.));
        Some(
            InlineElement::new(div().w(px(36.)).h(px(40.)).bg(rgb(0x00ff00)))
                .with_baseline(px(30.)),
        )
    }
}
struct MixedHost {
    document: Entity<MarkdownViewState>,
    revealed: Rc<Cell<Bounds<Pixels>>>,
    hidden: bool,
}
impl Render for MixedHost {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        let reveal = self.revealed.clone();
        let child = div()
            .w(px(240.))
            .text_size(px(16.))
            .line_height(px(24.))
            .child(
                MarkdownView::new(&self.document)
                    .markdown_extensions(
                        MarkdownExtensions::default().plugin(Formula),
                    )
                    .on_reveal(move |b, _, _| reveal.set(b)),
            );
        div().size_full().child(if self.hidden {
            child.absolute().top(px(-1_000_000.)).into_any_element()
        } else {
            child.into_any_element()
        })
    }
}
#[gpui_kit::test]
fn mixed_inline_elements_share_a_baseline_and_hidden_reveals_work(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let revealed = Rc::new(Cell::new(Bounds::default()));
    let mut state = None;
    let mut hosted = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let document = render("Before $x$ after **中文**.", cx);
        state = Some(document.clone());
        let host = cx.new(|_| MixedHost {
            document,
            revealed: revealed.clone(),
            hidden: false,
        });
        hosted = Some(host.clone());
        Root::new(host, window, cx)
    });
    let state = state.unwrap();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| {
        let frame = state.read(cx).frame.borrow();
        let text = frame.runs.first().unwrap().1.layout().bounds();
        let object = frame.objects.first().unwrap().1;
        let style = window.text_style();
        let sample = window.text_system().shape_line(
            "M".into(),
            px(16.),
            &[style.to_run(1)],
            None,
        );
        let baseline =
            (px(24.) - sample.ascent - sample.descent) / 2. + sample.ascent;
        assert!(
            (text.top() + baseline - object.top() - px(30.)).abs() < px(1.)
        );
        assert!(state.read(cx).bounds().size.height >= px(40.));
    });
    hosted.unwrap().update(cx, |view, cx| {
        view.hidden = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        state.update(cx, |v, cx| v.reveal_range(0..0, cx).unwrap());
        window.draw(cx).clear(cx);
    });
    assert!(revealed.get().top() < px(-999_000.));
}

struct WrappedMixedHost {
    document: Entity<MarkdownViewState>,
    width: f32,
}
impl Render for WrappedMixedHost {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .w(px(self.width))
            .text_size(px(16.))
            .line_height(px(24.))
            .child(MarkdownView::new(&self.document).markdown_extensions(
                MarkdownExtensions::default().plugin(Formula),
            ))
    }
}

#[gpui_kit::test]
fn mixed_inline_fragments_never_wrap_a_second_time(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for source in [
        "- **Handshake**:\n  - `hello { protocol, id, capabilities, last_epoch, cursors }` $\\rightarrow$ `welcome { protocol, epoch, capabilities, mode }`（确认后激活）。",
        "1. **多层转换（JSON $\\rightarrow$ HTTP SSE 文本字节流 $\\rightarrow$ 二进制 Chunk 隧道 $\\rightarrow$ 拼包解析 SSE 文本 $\\rightarrow$ 还原 JSON）**",
    ] {
        for width in [180., 240., 360., 600., 816.] {
            let mut document = None;
            let (_, cx) = cx.add_window_view(|window, cx| {
                let state = render(source, cx);
                document = Some(state.clone());
                Root::new(
                    cx.new(|_| WrappedMixedHost {
                        document: state,
                        width,
                    }),
                    window,
                    cx,
                )
            });
            let document = document.unwrap();
            cx.update(|window, cx| {
                window.draw(cx).clear(cx);
                let frame = document.read(cx).frame.borrow();
                assert!(!frame.objects.is_empty());
                for (_, run) in &frame.runs {
                    assert!(
                        !run.layout().wrapped_text().contains('\n'),
                        "a prewrapped fragment wrapped again at {width}px: {:?}",
                        run.layout().wrapped_text(),
                    );
                }
            });
        }
    }
}

struct CodeStyleHost(Entity<MarkdownViewState>);
impl Render for CodeStyleHost {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .w(px(500.))
            .text_size(px(24.))
            .font_weight(FontWeight::BOLD)
            .child(MarkdownView::new(&self.0).style(
                MarkdownStyle::default().with_inline_code(HighlightStyle {
                    background_color: Some(rgb(0xff00ff).into()),
                    ..Default::default()
                }),
            ))
    }
}
#[gpui_kit::test]
fn fenced_code_uses_block_typography_without_inline_backgrounds(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let source =
        "```text\nlookup_items(query, count)\nread_item(key) // 示例\n```";
    let mut state = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let document = render(source, cx);
        state = Some(document.clone());
        Root::new(cx.new(|_| CodeStyleHost(document)), window, cx)
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert!(
            !window
                .painted_quads()
                .iter()
                .any(|q| q.background == rgb(0xff00ff).into()),
            "fenced code must not paint inline-code chips"
        );
        let document = state.as_ref().unwrap();
        let view = document.read(cx);
        let frame = view.frame.borrow();
        let line = frame.runs[0].1.layout().line_layouts()[0].clone();
        use gpui_kit::component::ActiveTheme as _;
        assert_eq!(line.font_size(), cx.theme().mono_font_size);
        drop(frame);
        document.update(cx, |view, cx| {
            view.select_all(cx);
            assert_eq!(view.selected_text(), source);
        });
    });
}

#[test]
fn inline_html_spans_cover_each_utf8_byte_once() {
    fn check_paragraph(paragraph: &crate::model::Paragraph) {
        let mut cursor = 0;
        for span in &paragraph.spans {
            assert_eq!(
                span.range.start, cursor,
                "overlapping or missing span in {:?}",
                paragraph.text
            );
            assert!(paragraph.text.get(span.range.clone()).is_some());
            cursor = span.range.end;
        }
        assert_eq!(cursor, paragraph.text.len());
    }
    fn check_block(block: &crate::model::Block) {
        use crate::model::Kind;
        match &block.kind {
            Kind::Paragraph(p)
            | Kind::Heading(_, p)
            | Kind::Code(_, p)
            | Kind::Math(p) => check_paragraph(p),
            Kind::Group(blocks) | Kind::Quote(blocks) => {
                blocks.iter().for_each(check_block)
            }
            Kind::List { items, .. } => {
                for (_, blocks) in items {
                    blocks.iter().for_each(check_block);
                }
            }
            Kind::Table { rows, .. } => {
                for p in rows.iter().flatten() {
                    check_paragraph(p);
                }
            }
            Kind::Rule => {}
        }
    }
    for source in [
        "Before<br>挑战**修复**：保留 `sample.rs`。",
        "**Before<br/>挑战修复** after<br />结束。",
        "| Artifact | Verify |\n| --- | --- |\n| `sample.rs` | 完成<br>**挑战修复**：保留 UTF-8。 |",
        "<div><p>Before<br><strong>挑战修复</strong> after.</p></div>",
    ] {
        let document = pagemd_parser::parse(source, &GpuiRenderer::parse_options()).unwrap();
        let (blocks, _) = crate::model::build(&document);
        assert!(!blocks.is_empty());
        blocks.iter().for_each(check_block);
    }
}
