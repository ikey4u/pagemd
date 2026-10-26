use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use bezel_mermaid::{render, Metrics, Options};

use crate::core::util::html_escape;

static MERMAID_DIAGRAM_ID: AtomicU64 = AtomicU64::new(0);

/// Headless bezel-mermaid render used by CLI / static convert.
pub fn render_mermaid(code: &str) -> Result<String> {
    let options = Options {
        id_prefix: format!(
            "pagemd-mermaid-{}",
            MERMAID_DIAGRAM_ID.fetch_add(1, Ordering::Relaxed)
        ),
        font_family: Some("Arial, system-ui, -apple-system, sans-serif".into()),
        font_size: 18.,
        responsive: Some(false),
        auto_direction: false,
        config: serde_json::from_value(serde_json::json!({
            "flowchart": {"nodeSpacing": 42, "rankSpacing": 48, "padding": 12}
        }))?,
        ..Default::default()
    };
    let drawing = render(code.trim(), &options, &Metrics)
        .context("Failed to render Mermaid diagram")?;
    Ok(format!(
        "<div class=\"mermaid-display\"><div class=\"mermaid-canvas\">{}</div></div>\n",
        drawing.svg
    ))
}

/// Client-side placeholder for `pagemd view` (official mermaid.js in the browser).
pub fn mermaid_client_html(code: &str) -> String {
    let escaped = html_escape(code.trim());
    format!(
        "<div class=\"mermaid-display\" data-mermaid-client data-mermaid-code=\"{escaped}\">\n\
<div class=\"mermaid-canvas\"><pre class=\"mermaid\">{escaped}</pre></div>\n\
</div>\n"
    )
}

pub fn mermaid_error_html(code: &str) -> String {
    format!(
        "<div class=\"mermaid-display mermaid-error\"><strong>Mermaid render failed</strong><pre><code>{}</code></pre></div>\n",
        html_escape(code)
    )
}
