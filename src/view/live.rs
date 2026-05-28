const WORKSPACE_SCRIPT: &str = include_str!("../../assets/workspace.js");
const PREVIEW_SCRIPT: &str = include_str!("../../assets/preview.js");

pub fn workspace_script_tag() -> String {
    format!("<script data-pagemd-workspace>\n{WORKSPACE_SCRIPT}\n</script>\n")
}

/// Ensure exported HTML includes workspace interactivity when the layout uses it.
pub fn ensure_export_html(mut html: String) -> String {
    if !html.contains("data-doc-workspace") || html.contains("data-pagemd-workspace") {
        return html;
    }
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, &workspace_script_tag());
    } else {
        html.push_str(&workspace_script_tag());
    }
    html
}

/// Wrap clean HTML for browser preview (workspace + live-reload scripts only in the response).
pub fn wrap_for_preview(mut html: String) -> String {
    let scripts = format!(
        "{}{}",
        workspace_script_tag(),
        format!("<script data-pagemd-live-preview>\n{PREVIEW_SCRIPT}\n</script>\n")
    );
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, &scripts);
    } else {
        html.push_str(&scripts);
    }
    html
}
