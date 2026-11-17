use crate::{Document, Item};
pub const CSS: &str = include_str!("openapi.css");
pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn items(output: &mut String, entries: &[Item]) {
    if entries.is_empty() {
        return;
    }
    output.push_str("<ul class=\"pagemd-openapi-items\">");
    for item in entries {
        output.push_str(&format!("<li><code>{}</code>", escape(&item.name)));
        if !item.metadata.is_empty() {
            output.push_str(&format!(
                " <span class=\"pagemd-openapi-meta\">{}</span>",
                escape(&item.metadata)
            ));
        }
        if !item.description.is_empty() {
            output.push_str(&format!("<p>{}</p>", escape(&item.description)));
        }
        items(output, &item.children);
        output.push_str("</li>");
    }
    output.push_str("</ul>");
}
pub fn render_html(document: &Document) -> String {
    let mut output =
        format!("<style>{CSS}</style><div class=\"pagemd-openapi\">");
    if document.operations.is_empty() {
        output.push_str(
            "<p class=\"pagemd-openapi-empty\">No API operations.</p>",
        );
    }
    for operation in &document.operations {
        output.push_str(&format!("<article class=\"pagemd-openapi-operation\" aria-label=\"{} {}\"><h3><span class=\"pagemd-openapi-method\">{}</span> <code>{}</code></h3>", escape(&operation.method), escape(&operation.path), escape(&operation.method), escape(&operation.path)));
        if !operation.comment.is_empty() {
            let comment = pagemd_parser::to_html_with_options(
                &operation.comment,
                &pagemd_parser::Options::gfm(),
            )
            .unwrap_or_else(|_| {
                format!("<p>{}</p>", escape(&operation.comment))
            });
            output.push_str("<div class=\"pagemd-openapi-comment\">");
            output.push_str(&comment);
            output.push_str("</div>");
        }
        output.push_str("<section><h4>Request</h4>");
        if operation.request.is_empty() {
            output.push_str("<p>No request parameters.</p>");
        } else {
            items(&mut output, &operation.request);
        }
        output.push_str("</section><section><h4>Response</h4>");
        items(&mut output, &operation.responses);
        output.push_str("</section>");
        output.push_str("</article>");
    }
    for warning in &document.warnings {
        output.push_str(&format!(
            "<p class=\"pagemd-openapi-warning\">{}</p>",
            escape(warning)
        ));
    }
    output.push_str("</div>");
    output
}
