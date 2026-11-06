// Shared source included by the PageMD and Sloud native adapters.
use gpui_kit::{
    base::{SelectableText, TestSupportExt as _},
    component::{ActiveTheme as _, StyledExt as _},
    *,
};
use pagemd_openapi::{Document, Item};

fn selectable(id: impl Into<SharedString>, text: impl Into<SharedString>) -> AnyElement {
    let id = id.into();
    div()
        .id(id.clone())
        .test_support()
        .cursor_text()
        .child(SelectableText::new(
            ElementId::NamedInteger(id, 0),
            text,
        ))
        .into_any_element()
}

fn item_list(items: &[Item], id: &str, cx: &App) -> AnyElement {
    let mut list = div().flex().flex_col().w_full().min_w_0().gap_2().mt_2();
    for (index, item) in items.iter().enumerate() {
        let id = format!("{id}-{index}");
        let mut body = div().flex().flex_col().flex_1().min_w_0().child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .items_baseline()
                .child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(12.))
                        .child(selectable(format!("{id}-name"), item.name.clone())),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(selectable(
                            format!("{id}-metadata"),
                            item.metadata.clone(),
                        )),
                ),
        );
        if !item.description.is_empty() {
            body = body.child(
                div()
                    .w_full()
                    .min_w_0()
                    .whitespace_normal()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(selectable(
                        format!("{id}-description"),
                        item.description.clone(),
                    )),
            );
        }
        if !item.children.is_empty() {
            body = body.child(item_list(&item.children, &id, cx));
        }
        list = list.child(
            div()
                .flex()
                .items_start()
                .gap_2()
                .w_full()
                .min_w_0()
                .child(
                    div()
                        .flex_shrink_0()
                        .text_color(cx.theme().muted_foreground)
                        .child("•"),
                )
                .child(body),
        );
    }
    list.into_any_element()
}

pub fn render_document(document: &Document, cx: &App) -> AnyElement {
    let mut root = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .gap_3()
        .text_size(px(13.))
        .text_color(cx.theme().foreground);
    if document.operations.is_empty() {
        root = root.child(selectable("openapi-empty", "No API operations."));
    }
    for (index, op) in document.operations.iter().enumerate() {
        let id = format!("openapi-operation-{index}");
        let mut request = div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .mt_4()
            .child(
                div()
                    .font_semibold()
                    .child(selectable(format!("{id}-request-heading"), "Request")),
            );
        if op.request.is_empty() {
            request = request.child(
                div()
                    .mt_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(selectable(
                        format!("{id}-request-empty"),
                        "No request parameters.",
                    )),
            );
        } else {
            request = request.child(item_list(
                &op.request,
                &format!("{id}-request"),
                cx,
            ));
        }
        let mut card = div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(7.))
            .p_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .items_baseline()
                    .child(
                        div()
                            .font_semibold()
                            .text_color(cx.theme().primary)
                            .child(selectable(
                                format!("{id}-method"),
                                op.method.clone(),
                            )),
                    )
                    .child(
                        div()
                            .font_family(cx.theme().mono_font_family.clone())
                            .whitespace_normal()
                            .child(selectable(format!("{id}-path"), op.path.clone())),
                    ),
            )
            .child(request)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_w_0()
                    .mt_4()
                    .child(
                        div()
                            .font_semibold()
                            .child(selectable(
                                format!("{id}-response-heading"),
                                "Response",
                            )),
                    )
                    .child(item_list(
                        &op.responses,
                        &format!("{id}-response"),
                        cx,
                    )),
            );
        if !op.comment.is_empty() {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_w_0()
                    .mt_4()
                    .child(
                        div()
                            .font_semibold()
                            .child(selectable(
                                format!("{id}-comment-heading"),
                                "Comment",
                            )),
                    )
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_normal()
                            .child(selectable(
                                format!("{id}-comment"),
                                op.comment.clone(),
                            )),
                    ),
            );
        }
        root = root.child(card);
    }
    for (index, warning) in document.warnings.iter().enumerate() {
        root = root.child(
            div()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .whitespace_normal()
                .child(selectable(
                    format!("openapi-warning-{index}"),
                    warning.clone(),
                )),
        );
    }
    root.into_any_element()
}
