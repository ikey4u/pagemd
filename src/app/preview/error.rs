use crate::core::model::RenderedSection;
use crate::core::{build_html, html_escape, HtmlExportOptions};

pub(crate) fn build_preview_error_html(err: &anyhow::Error) -> String {
    let message = html_escape(&format!("{err:#}"));
    let body = format!(
        r#"<div class="callout callout-warning" role="alert">
<p><strong>Preview render failed</strong></p>
<pre><code>{message}</code></pre>
<p>Fix the source file and save again. The preview will update automatically.</p>
</div>"#
    );
    build_html(
        "Preview Error",
        &[RenderedSection {
            title: "Preview Error".to_string(),
            html: body,
            outline: Vec::new(),
        }],
        "ER",
    )
}

pub(crate) fn preview_html_opts() -> HtmlExportOptions {
    HtmlExportOptions {
        embed_workspace_script: false,
    }
}
