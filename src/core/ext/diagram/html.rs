use std::path::Path;

use crate::core::export::html::bundler::inline_raw_html_resources;

pub fn is_diagram_html_info(info: &str) -> bool {
    let mut parts = info.split_whitespace();
    let Some(kind) = parts.next() else {
        return false;
    };

    if matches!(
        kind.to_ascii_lowercase().as_str(),
        "diagram-html" | "diagram_html"
    ) {
        return true;
    }

    kind.eq_ignore_ascii_case("diagram")
        && parts
            .next()
            .is_some_and(|format| format.eq_ignore_ascii_case("html"))
}

pub fn render_diagram_html(code: &str, base_dir: &Path) -> String {
    let body = inline_raw_html_resources(code.trim(), base_dir);
    format!(
        "<div class=\"diagram-html-display\"><div class=\"diagram-html-canvas\">{body}</div></div>\n"
    )
}
