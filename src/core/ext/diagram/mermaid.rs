use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use merman::render::HeadlessRenderer;
use merman::MermaidConfig;

use crate::core::util::html_escape;

static MERMAID_DIAGRAM_ID: AtomicU64 = AtomicU64::new(0);

fn mermaid_site_config() -> MermaidConfig {
    MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "fontSize": "18px",
            "fontFamily": "ui-sans-serif, system-ui, -apple-system, sans-serif"
        },
        "flowchart": {
            "htmlLabels": true,
            "useMaxWidth": false,
            "nodeSpacing": 42,
            "rankSpacing": 48,
            "padding": 12
        },
        "sequence": { "useMaxWidth": false },
        "class": { "useMaxWidth": false },
        "state": { "useMaxWidth": false },
        "er": { "useMaxWidth": false },
        "gantt": { "useMaxWidth": false },
        "pie": { "useMaxWidth": false },
        "journey": { "useMaxWidth": false }
    }))
}

/// Headless render (merman) used by CLI / static convert.
pub(crate) fn render_mermaid(code: &str) -> Result<String> {
    let id = format!(
        "pagemd-mermaid-{}",
        MERMAID_DIAGRAM_ID.fetch_add(1, Ordering::Relaxed)
    );
    let renderer = HeadlessRenderer::new()
        .with_diagram_id(&id)
        .with_site_config(mermaid_site_config());
    let svg = renderer
        .render_svg_sync(code.trim())
        .context("Failed to render Mermaid diagram")?
        .context("Mermaid diagram produced no SVG output")?;
    Ok(format!(
        "<div class=\"mermaid-display\"><div class=\"mermaid-canvas\">{svg}</div></div>\n"
    ))
}

/// Client-side placeholder for `pagemd view` (official mermaid.js in the browser).
pub(crate) fn mermaid_client_html(code: &str) -> String {
    let escaped = html_escape(code.trim());
    format!(
        "<div class=\"mermaid-display\" data-mermaid-client data-mermaid-code=\"{escaped}\">\n\
<div class=\"mermaid-canvas\"><pre class=\"mermaid\">{escaped}</pre></div>\n\
</div>\n"
    )
}

pub(crate) fn mermaid_error_html(code: &str) -> String {
    format!(
        "<div class=\"mermaid-display mermaid-error\"><strong>Mermaid render failed</strong><pre><code>{}</code></pre></div>\n",
        html_escape(code)
    )
}
