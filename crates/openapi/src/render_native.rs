// Shared source included by the PageMD and Sloud native adapters.
use gpui_kit::{
    component::{ActiveTheme as _, StyledExt as _},
    *,
};
use pagemd_openapi::{Document, Item};

fn item_list(items: &[Item], cx: &App) -> AnyElement {
    let mut list = div().flex().flex_col().w_full().min_w_0().gap_2().mt_2();
    for item in items {
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
                        .child(item.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(item.metadata.clone()),
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
                    .child(item.description.clone()),
            );
        }
        if !item.children.is_empty() {
            body = body.child(item_list(&item.children, cx));
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
        root = root.child("No API operations.");
    }
    for op in &document.operations {
        let mut request = div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .mt_4()
            .child(div().font_semibold().child("Request"));
        if op.request.is_empty() {
            request = request.child(
                div()
                    .mt_2()
                    .text_color(cx.theme().muted_foreground)
                    .child("No request parameters."),
            );
        } else {
            request = request.child(item_list(&op.request, cx));
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
                            .child(op.method.clone()),
                    )
                    .child(
                        div()
                            .font_family(cx.theme().mono_font_family.clone())
                            .whitespace_normal()
                            .child(op.path.clone()),
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
                    .child(div().font_semibold().child("Response"))
                    .child(item_list(&op.responses, cx)),
            );
        if !op.comment.is_empty() {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_w_0()
                    .mt_4()
                    .child(div().font_semibold().child("Comment"))
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_normal()
                            .child(op.comment.clone()),
                    ),
            );
        }
        root = root.child(card);
    }
    for warning in &document.warnings {
        root = root.child(
            div()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .whitespace_normal()
                .child(warning.clone()),
        );
    }
    root.into_any_element()
}
