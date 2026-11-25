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
#[gpui_kit::test]
fn native_blockquote_soft_breaks_preserve_lines_and_source(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    for (source, expected) in [
        (
            "> Owner: Sloud\n> Scope: `worker`\n> 依据: Rust &amp; API\n> Status: **needs changes**",
            "Owner: Sloud\nScope: worker\n依据: Rust & API\nStatus: needs changes\n",
        ),
        (
            "> outer\n> line\n>\n> > nested\n> > line\n>\n> after\n> nested\n\nnormal\nparagraph",
            "outer\nline\nnested\nline\nafter\nnested\nnormal paragraph\n",
        ),
        (
            "> - item\n>   continuation\n\n- ordinary\n  continuation",
            "item\ncontinuation\nordinary continuation\n",
        ),
        (
            "> first\r\n> second  \r\n> third\r\n\r\nnormal\r\nparagraph",
            "first\nsecond\nthird\nnormal paragraph\n",
        ),
    ] {
        let state = cx.update(|cx| render(source, cx));
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                let text = state.rendered_text();
                assert_eq!(text.as_str(), expected, "source: {source}");
                assert_eq!(text.source(), source);
                for word in ["Sloud", "worker", "依据", "needs changes"] {
                    if let Some(start) = source.find(word) {
                        let range = text
                            .range_for_source(start..start + word.len())
                            .unwrap();
                        assert_eq!(&text.as_str()[range], word);
                    }
                }
                // Soft breaks retain character-level mappings in both directions.
                for (start, _) in source.match_indices('\n') {
                    if let Some(range) = text.range_for_source(start..start + 1)
                    {
                        assert!(matches!(
                            &text.as_str()[range.clone()],
                            "\n" | " "
                        ));
                        assert!(
                            text.source_for_range(range)
                                .unwrap()
                                .contains(&start)
                        );
                    }
                }
                state.select_all(cx);
                assert_eq!(state.selected_text(), source);
                state.set_selection_format(SelectionFormat::Plain, cx);
                assert_eq!(state.selected_text(), expected);
            });
        });
    }
}

#[gpui_kit::test]
fn native_blockquote_fields_layout_on_separate_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut document = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let state = render(
            "> Owner: Sloud\n> Scope: `worker`\n> Status: **needs changes**",
            cx,
        );
        document = Some(state.clone());
        Root::new(
            cx.new(|_| MixedHost {
                document: state,
                revealed: Rc::new(Cell::new(Bounds::default())),
                hidden: false,
            }),
            window,
            cx,
        )
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let state = document.as_ref().unwrap().read(cx);
        let frame = state.frame.borrow();
        let layout = frame.runs[0].1.layout();
        let text = state.rendered_text();
        let mut previous = None;
        for label in ["Owner:", "Scope:", "Status:"] {
            let row = layout
                .position_for_index(text.as_str().find(label).unwrap())
                .unwrap();
            if let Some(y) = previous {
                assert!(row.y > y, "{label} must start on its own row");
            }
            previous = Some(row.y);
        }
    });
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
            Kind::Group(blocks)
            | Kind::Quote(blocks)
            | Kind::Boxed {
                children: blocks, ..
            } => blocks.iter().for_each(check_block),
            Kind::Details {
                summary, children, ..
            } => {
                check_paragraph(summary);
                children.iter().for_each(check_block);
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
            Kind::Metadata(rows) => {
                for (_, _, p) in rows {
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

#[gpui_kit::test]
fn source_across_styles_preserves_structure_and_plain_runs(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    for (source, selection, plain) in [
        ("plain **bold** tail", "bold", true),
        ("plain **bold** tail", "bold tail", false),
        ("plain **bold** tail", "plain bold", false),
        ("**bold _italic_ bold**", "bold italic", false),
        ("[link](https://example.com)", "link", true),
        ("# Heading", "Heading", true),
        ("```rust\nlet value = 1;\n```", "let value", true),
        ("first\n\nsecond", "first\nsecond", false),
        ("- first\n- second", "first\nsecond", false),
        ("| A | B |\n| - | - |\n| one | two |", "one two", false),
        ("before $x$ after", "x", false),
        ("before ![image](image.png) after", "image", false),
    ] {
        let state = cx.update(|cx| render(source, cx));
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                let rendered = state.rendered_text();
                let start = rendered.as_str().find(selection).expect(selection);
                let range = start..start + selection.len();
                state.frame.borrow_mut().selected = Some(range);
                state.set_selection_format(SelectionFormat::Source, cx);
                let original = state.selected_text();
                state.set_selection_format(
                    SelectionFormat::SourceAcrossStyles,
                    cx,
                );
                assert_eq!(
                    state.selected_text(),
                    if plain { selection } else { &original },
                    "{source}"
                );
                state.select_all(cx);
                assert_eq!(state.selected_text(), source);
            })
        });
    }
}

#[gpui_kit::test]
fn explicit_selection_format_reads_only_the_selection_without_changing_copy_mode(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    for (source, selected) in [
        ("<p>before <strong>selected</strong> after</p>", "selected"),
        ("前文 **选中的中文🙂** 后文", "选中的中文🙂"),
        ("Intro `code **literal**` tail", "code **literal**"),
        ("left &amp; right", "&"),
        ("> first\n> selected\n> last", "selected"),
    ] {
        let state = cx.update(|cx| render(source, cx));
        cx.update(|cx| {
            state.update(cx, |state, cx| {
                let rendered = state.rendered_text();
                let start = rendered.as_str().find(selected).unwrap();
                state.frame.borrow_mut().selected =
                    Some(start..start + selected.len());
                let original = state.selected_text();
                if source.starts_with("<p>") {
                    assert_eq!(
                        original, source,
                        "HTML source maps the complete node"
                    );
                }
                assert_eq!(
                    state.selected_text_with_format(SelectionFormat::Plain),
                    selected
                );
                assert_eq!(
                    state.selected_text(),
                    original,
                    "reading must not change the clipboard mode"
                );
                state.select_all(cx);
                assert_eq!(
                    state.selected_text_with_format(SelectionFormat::Plain),
                    rendered.as_str()
                );
                assert_eq!(state.selected_text(), source);
                state.clear_selection(cx);
                assert_eq!(
                    state.selected_text_with_format(SelectionFormat::Plain),
                    ""
                );
            })
        });
    }
}

struct WideFormula;
impl MarkdownPlugin for WideFormula {
    fn name(&self) -> &str {
        "wide-formula"
    }
    fn parse(
        &self,
        node: &pagemd_parser::mdast::Node,
        cx: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        Formula.parse(node, cx)
    }
    fn render_inline(
        &self,
        _: &MarkdownNode,
        _: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        Some(InlineElement::new(
            div().w(px(600.)).h(px(30.)).bg(rgb(0x00ff00)),
        ))
    }
}
struct TableHost {
    document: Entity<MarkdownViewState>,
    width: f32,
}
impl Render for TableHost {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> impl IntoElement {
        div().size_full().child(
            div()
                .w(px(self.width))
                .text_size(px(16.))
                .line_height(px(24.))
                .child(MarkdownView::new(&self.document).markdown_extensions(
                    MarkdownExtensions::default().plugin(WideFormula),
                )),
        )
    }
}
#[gpui_kit::test]
fn wide_table_formula_scrolls_within_its_cell(cx: &mut TestAppContext) {
    use gpui_kit::test::TestWindowExt as _;
    cx.update(gpui_kit::init);
    let mut state = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let document = render(
            "| Formula | Neighbor |\n| --- | --- |\n| $x$ | keep visible |",
            cx,
        );
        state = Some(document.clone());
        Root::new(
            cx.new(|_| TableHost {
                document,
                width: 360.,
            }),
            window,
            cx,
        )
    });
    let document = state.unwrap();
    cx.run_until_parked();
    let (before, peer) = cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        let cell = window.find("pagemd-table-cell-0-1-0").bounds();
        let peer = window.find("pagemd-table-cell-0-1-1").bounds();
        assert!(cell.right() <= peer.left());
        let object = document.read(cx).frame.borrow().objects[0].1;
        assert_eq!(object.size.width, px(600.));
        assert!(object.size.width > cell.size.width);
        (object.left(), peer)
    });
    cx.update(|window, cx| {
        window.scroll(
            "pagemd-table-cell-0-1-0",
            ScrollDelta::Pixels(point(px(-120.), px(0.))),
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert!(
            document.read(cx).frame.borrow().objects[0].1.left() < before,
            "the entire formula must remain reachable by scrolling"
        );
        assert_eq!(window.find("pagemd-table-cell-0-1-1").bounds(), peer);
    });
}
#[gpui_kit::test]
fn native_table_currency_copy_and_mapping_remain_literal(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let source = "| Price | Result |\n| --- | --- |\n| CPU $0.000014/vCPU·s；RAM $0.0000045/GiB·s | $0.082800 ≈ **¥0.5521** |";
    let state = cx.update(|cx| render(source, cx));
    cx.update(|cx| {
        state.update(cx, |state, cx| {
            let text = state.rendered_text();
            for label in ["$0.000014", "$0.0000045", "$0.082800", "¥0.5521"] {
                assert!(text.as_str().contains(label));
                let at = source.find(label).unwrap();
                let range =
                    text.range_for_source(at..at + label.len()).unwrap();
                assert_eq!(&text.as_str()[range], label);
            }
            state.select_all(cx);
            assert_eq!(state.selected_text(), source);
        });
    });
}

#[gpui_kit::test]
fn frontmatter_keeps_literal_text_and_exact_value_source_mapping(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    for source in [
        "\u{feff}---\r\nname: reviewdoc\r\ndescription: 中文 **literal** $5 &amp; \\(x\\)\r\n...\r\n# Body",
        "+++\nname = 'reviewdoc'\n[metadata]\nenabled = true\n+++\n# Body",
        "{\"name\":\"reviewdoc\",\"nested\":{\"active\":true}}\n# Body",
    ] {
        let metadata = pagemd_parser::frontmatter::detect(source).unwrap();
        let value = source[metadata.value_range].replace("\r\n", "\n");
        let state = cx.update(|cx| render(source, cx));
        cx.update(|cx| state.update(cx, |state, cx| {
            let text = state.rendered_text();
            let _ = value;
            assert!(text.as_str().contains("reviewdoc"));
            for word in ["name", "reviewdoc", "中文", "**literal**", "$5", "&amp;", "Body"] {
                if let Some(start) = source.find(word) {
                    let rendered = text.range_for_source(start..start + word.len()).unwrap();
                    assert_eq!(&text.as_str()[rendered.clone()], word);
                    assert_eq!(text.source_for_range(rendered).unwrap(), start..start + word.len());
                }
            }
            state.select_all(cx);
            assert_eq!(state.selected_text(), source);
            state.set_selection_format(SelectionFormat::Plain, cx);
            assert!(state.selected_text().contains("reviewdoc"));
        }));
    }
}

#[gpui_kit::test]
fn labelled_serialization_fence_keeps_code_and_comment_mapping(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let source = "```yaml frontmatter\n# comment\nname: example\n```";
    let state = cx.update(|cx| render(source, cx));
    cx.update(|cx| {
        let text = state.read(cx).rendered_text();
        assert!(text.as_str().starts_with("# comment\nname: example"));
        let start = source.find("comment").unwrap();
        let range = text.range_for_source(start..start + 7).unwrap();
        assert_eq!(&text.as_str()[range], "comment");
    });
}

#[gpui_kit::test]
fn ambiguous_metadata_selection_never_returns_only_the_following_body(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let source = "---\nfirst: repeated\nsecond: repeated\n---\n\nBody";
    let state = cx.update(|cx| render(source, cx));
    cx.update(|cx| {
        let text = state.read(cx).rendered_text();
        assert!(text.source_for_range(0..text.len()).is_none());
        let start = text.as_str().find("Body").unwrap();
        assert_eq!(
            &source[text.source_for_range(start..start + 4).unwrap()],
            "Body"
        );
    });
    cx.update(|cx| {
        state.update(cx, |state, _| {
            let text = state.rendered_text().as_str().to_owned();
            state.frame.borrow_mut().selected = Some(0..text.len());
            assert_eq!(
                state.selected_text_with_format(SelectionFormat::Source),
                text
            );
        })
    });
}

#[gpui_kit::test]
fn list_markers_use_measured_columns_and_compact_body_gaps(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::TestWindowExt as _;
    cx.update(gpui_kit::init);
    for source in [
        "- Alpha\n- Beta",
        "1. Alpha\n2. Beta\n3. Gamma",
        "9. Alpha\n10. Beta",
        "- [x] Alpha\n- [ ] Beta",
    ] {
        let (_, cx) = cx.add_window_view(|window, cx| {
            let document = render(source, cx);
            Root::new(
                cx.new(|_| TableHost {
                    document,
                    width: 360.,
                }),
                window,
                cx,
            )
        });
        cx.simulate_resize(size(px(400.), px(240.)));
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.render_frame(cx);
            let first = window.find("pagemd-list-marker-0-0").bounds();
            let second = window.find("pagemd-list-marker-0-1").bounds();
            let body = window.find("pagemd-list-body-0-0").bounds();
            let next = window.find("pagemd-list-body-0-1").bounds();
            let gap = f32::from(body.left() - first.right());
            assert!((4.0..=8.0).contains(&gap), "{source:?}: gap={gap}");
            assert_eq!(first.right(), second.right());
            assert_eq!(body.left(), next.left());
            for i in 0..source.lines().count() {
                let marker = window
                    .find(SharedString::from(format!(
                        "pagemd-list-marker-0-{i}"
                    )))
                    .bounds();
                assert_eq!(
                    marker.size.height,
                    px(24.),
                    "{source:?}: marker {i} should fit on one line"
                );
            }
            if source.starts_with("- Alpha") {
                assert!(first.size.width < px(24.), "excessive bullet column");
            }
        });
    }
}

/// The HTML set as the HTML renderer shows it, natively: containers around
/// Markdown, boxes with their look, inline styles, captions and media.
mod html_parity {
    use super::*;
    use crate::model::{Align, Block, Kind, Size};

    /// The blocks, HTML's group wrappers looked through.
    fn blocks(source: &str) -> Vec<Block> {
        let document =
            pagemd_parser::parse(source, &GpuiRenderer::parse_options())
                .unwrap();
        fn flat(blocks: Vec<Block>) -> Vec<Block> {
            blocks
                .into_iter()
                .flat_map(|b| match b.kind {
                    Kind::Group(children) => flat(children),
                    _ => vec![b],
                })
                .collect()
        }
        flat(crate::model::build(&document).0)
    }

    #[test]
    fn details_around_markdown_fold_with_their_summary() {
        let b = blocks("<details><summary>More <b>here</b></summary>\n\nHidden **markdown**.\n\n</details>\n\nAfter.");
        match &b[0].kind {
            Kind::Details {
                open,
                summary,
                children,
            } => {
                assert!(!open);
                assert_eq!(summary.text.as_ref(), "More here");
                assert!(summary.spans.iter().any(|s| s.mark.bold));
                assert!(
                    matches!(&children[0].kind, Kind::Paragraph(p) if p.text.contains("Hidden markdown"))
                );
            }
            _ => panic!("not details"),
        }
        assert!(
            matches!(&b[1].kind, Kind::Paragraph(p) if p.text.contains("After"))
        );
        assert!(matches!(
            &blocks("<details open><summary>S</summary>\n\nx\n\n</details>")[0]
                .kind,
            Kind::Details { open: true, .. }
        ));
    }

    #[test]
    fn boxes_take_their_tag_align_and_style() {
        let b = blocks(
            r#"<div style="display:flex;gap:8px;justify-content:center;background-color:#eef;border:1px solid #ccc;border-radius:8px;padding:4px 8px;position:fixed"><img src="a.png" width="60"><img src="b.png" style="width:50%"></div>"#,
        );
        let Kind::Boxed { style, .. } = &b[0].kind else {
            panic!("not a box")
        };
        assert!(style.row);
        assert_eq!(style.gap, Some(8.));
        assert_eq!(style.justify, Some(Align::Center));
        assert_eq!(style.background, Some(0xeeeeffff));
        assert_eq!(style.border, Some((1., Some(0xccccccff))));
        assert_eq!(style.radius, Some(8.));
        assert_eq!(style.padding, Some([4., 8., 4., 8.]));
        let Kind::Boxed { style, .. } =
            &blocks(r#"<p align="center">Centred <b>text</b></p>"#)[0].kind
        else {
            panic!()
        };
        assert_eq!(style.align, Some(Align::Center));
        let Kind::Boxed { style, children } =
            &blocks("<center>\n\nMiddle **markdown**\n\n</center>")[0].kind
        else {
            panic!()
        };
        assert_eq!(style.align, Some(Align::Center));
        assert!(
            matches!(&children[0].kind, Kind::Paragraph(p) if p.text.contains("Middle markdown"))
        );
        let Kind::Boxed { style, .. } =
            &blocks(r#"<div style="max-width:50%">x</div>"#)[0].kind
        else {
            panic!()
        };
        assert_eq!(style.max_width, Some(Size::Fraction(0.5)));
        let Kind::Boxed { style, .. } =
            &blocks("<dl><dt>Term</dt><dd>Definition</dd></dl>")[0].kind
        else {
            panic!()
        };
        assert_eq!(style, &crate::model::BoxStyle::default());
    }

    #[test]
    fn inline_styles_and_raised_text() {
        let b = blocks(
            r#"<span style="color:#c00;background-color:yellow;font-weight:700;font-style:italic;text-decoration:underline">s</span> H<sub>2</sub>O x<sup>2</sup> <small>fine</small> <strike>old</strike> <abbr title="HyperText">HTML</abbr>"#,
        );
        let Kind::Paragraph(p) = &b[0].kind else {
            panic!("not a paragraph")
        };
        let span = p
            .spans
            .iter()
            .find(|s| &p.text[s.range.clone()] == "s")
            .unwrap();
        assert_eq!(span.mark.color, Some(0xcc0000ff));
        assert_eq!(span.mark.background, Some(0xffff00ff));
        assert!(span.mark.bold && span.mark.italic && span.mark.underline);
        let objects: Vec<_> = p
            .objects
            .iter()
            .map(|o| p.text[o.range.clone()].to_string())
            .collect();
        assert_eq!(
            objects,
            vec!["2", "2", "fine"],
            "sub, sup and small are their own runs"
        );
        assert!(p
            .spans
            .iter()
            .any(|s| s.mark.strike && &p.text[s.range.clone()] == "old"));
        assert!(p
            .spans
            .iter()
            .any(|s| s.mark.underline && &p.text[s.range.clone()] == "HTML"));
    }

    #[test]
    fn tables_keep_their_caption_and_alignment_media_their_poster() {
        let b = blocks(
            r#"<table><caption>Sizes</caption><thead><tr><th align="left">A</th><th style="text-align:center">B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>"#,
        );
        assert!(
            matches!(&b[0].kind, Kind::Boxed { style, .. } if style.caption)
        );
        let Kind::Table { rows, align } = &b[1].kind else {
            panic!()
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(
            align,
            &vec![
                pagemd_parser::mdast::AlignKind::Left,
                pagemd_parser::mdast::AlignKind::Center
            ]
        );
        let b = blocks(
            r#"<video src="/p/a/media/v.mp4" poster="/p/a/media/v.jpg" controls></video>"#,
        );
        let Kind::Paragraph(p) = &b[0].kind else {
            panic!()
        };
        assert!(p.objects.iter().any(|o| matches!(&o.node, pagemd_parser::mdast::Node::Image(i) if i.url == "/p/a/media/v.jpg")));
        assert!(p
            .spans
            .iter()
            .any(|s| s.mark.link.as_deref() == Some("/p/a/media/v.mp4")));
    }

    #[gpui_kit::test]
    fn details_open_and_fold_on_a_click(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt as _;
        cx.update(gpui_kit::init);
        let (_, cx) = cx.add_window_view(|window, cx| {
            let document = render(
                "<details><summary>More</summary>\n\nHidden.\n\n</details>",
                cx,
            );
            Root::new(
                cx.new(|_| TableHost {
                    document,
                    width: 360.,
                }),
                window,
                cx,
            )
        });
        cx.simulate_resize(size(px(400.), px(240.)));
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.render_frame(cx);
            assert!(
                window.try_find("pagemd-details-body-0").is_none(),
                "folded at first"
            );
            window.click("pagemd-details-summary-0", cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.render_frame(cx);
            assert!(
                window.try_find("pagemd-details-body-0").is_some(),
                "open after a click"
            );
        });
    }
}
