use anyhow::{Context, Result};
use mermaid_rs_renderer::{render_with_options, RenderOptions};

use crate::core::util::html_escape;

pub(crate) fn render_mermaid(code: &str) -> Result<String> {
    let opts = RenderOptions::modern()
        .with_node_spacing(60.0)
        .with_rank_spacing(80.0);
    let svg = render_with_options(code.trim(), opts).context("Failed to render Mermaid diagram")?;
    Ok(format!(
        "<div class=\"mermaid-display\"><div class=\"mermaid-canvas\">{svg}</div></div>\n"
    ))
}

pub(crate) fn mermaid_error_html(code: &str) -> String {
    format!(
        "<div class=\"mermaid-display mermaid-error\"><strong>Mermaid render failed</strong><pre><code>{}</code></pre></div>\n",
        html_escape(code)
    )
}
