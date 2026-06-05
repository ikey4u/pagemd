use crate::core::workspace_script_tag;

const PREVIEW_SCRIPT: &str = include_str!("../../../assets/preview.js");

/// Ensure exported HTML includes workspace interactivity when the layout uses it.
pub fn ensure_export_html(mut html: String) -> String {
    if !html.contains("data-doc-workspace") || html.contains("data-pagemd-workspace") {
        return html;
    }
    let tag = workspace_script_tag();
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, &tag);
    } else {
        html.push_str(&tag);
    }
    html
}

/// Wrap clean HTML for browser preview (workspace + live-reload scripts only in the response).
pub fn wrap_for_preview(mut html: String) -> String {
    let scripts = format!(
        "{}<script data-pagemd-live-preview>\n{PREVIEW_SCRIPT}\n</script>\n",
        workspace_script_tag()
    );
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, &scripts);
    } else {
        html.push_str(&scripts);
    }
    html
}
