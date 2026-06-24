use anyhow::{bail, Result};
use serde_json::Value;

use super::cdp::CdpSession;
use super::snap::{self, format_snap};

const DEFAULT_CLEAN_SELECTORS: &[&str] = &[
    "header",
    "footer",
    "nav",
    "aside",
    ".sidebar",
    ".ads",
    ".cookie-notice",
    "[class*=\"sidebar\"]",
    "[class*=\"comment\"]",
    "[id*=\"comment\"]",
];

pub async fn snap_text(session: &CdpSession, preferred_url: Option<&str>) -> Result<String> {
    let value = snap::capture_page(session, preferred_url).await?;
    Ok(format_snap(&value))
}

pub async fn html_text(
    session: &CdpSession,
    preferred_url: Option<&str>,
    body_only: bool,
    max_chars: usize,
) -> Result<String> {
    let html = if body_only {
        snap::capture_body_html(session, preferred_url).await?
    } else {
        snap::capture_html(session, preferred_url).await?
    };
    Ok(truncate(html, max_chars))
}

pub async fn markdown_text(
    session: &CdpSession,
    preferred_url: Option<&str>,
    max_chars: usize,
) -> Result<String> {
    let html = snap::capture_body_html(session, preferred_url).await?;
    let md = snap::html_to_markdown(&html)?;
    if md.trim().is_empty() {
        bail!(
            "markdown export is empty (HTML was {} chars). The page may be mostly script/canvas or content lives in iframes.",
            html.len()
        );
    }
    Ok(truncate(md, max_chars))
}

pub async fn run_clean_dom(session: &CdpSession, extra_selectors: &[String]) -> Result<Value> {
    let mut selectors: Vec<&str> = DEFAULT_CLEAN_SELECTORS.to_vec();
    for s in extra_selectors {
        if !s.trim().is_empty() {
            selectors.push(s.as_str());
        }
    }
    let selectors_json = serde_json::to_string(&selectors)?;
    session
        .evaluate(
            &format!(
                r#"(() => {{
  const sels = {selectors_json};
  let removed = 0;
  for (const sel of sels) {{
    try {{
      document.querySelectorAll(sel).forEach(el => {{ el.remove(); removed++; }});
    }} catch (_) {{}}
  }}
  return {{ removed }};
}})()"#
            ),
            false,
        )
        .await
}

pub async fn run_clean_dom_sandbox(
    session: &CdpSession,
    extra_selectors: &[String],
) -> Result<Value> {
    let mut selectors: Vec<&str> = DEFAULT_CLEAN_SELECTORS.to_vec();
    for s in extra_selectors {
        if !s.trim().is_empty() {
            selectors.push(s.as_str());
        }
    }
    let selectors_json = serde_json::to_string(&selectors)?;
    let script = format!(
        r#"(() => {{
  const sels = {selectors_json};
  const doc = window.__PAGEMD_SANDBOX_DOC__;
  if (!doc) throw new Error("sandbox not active");
  let removed = 0;
  for (const sel of sels) {{
    try {{
      doc.querySelectorAll(sel).forEach(el => {{ el.remove(); removed++; }});
    }} catch (_) {{}}
  }}
  return {{ removed }};
}})()"#
    );
    super::sandbox::eval_expression(session, &script).await
}

pub fn format_eval_result(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

pub fn truncate(mut text: String, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text;
    }
    let kept: String = text.chars().take(max_chars).collect();
    text = kept;
    text.push_str(&format!(
        "\n\n[truncated at {max_chars} chars; use a smaller max_chars or write to disk via REPL /html -o]"
    ));
    text
}

pub fn parse_max_chars(value: Option<u64>, default: usize) -> Result<usize> {
    match value {
        None => Ok(default),
        Some(n) if n == 0 => bail!("max_chars must be > 0"),
        Some(n) if n > 500_000 => bail!("max_chars must be <= 500000"),
        Some(n) => Ok(n as usize),
    }
}
