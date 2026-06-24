use anyhow::{bail, Result};
use serde_json::Value;

use super::cdp::CdpSession;

pub async fn capture_page(session: &CdpSession, preferred_url: Option<&str>) -> Result<Value> {
    session.attach_to_best_tab(preferred_url).await?;
    session
        .evaluate(
            r#"(() => {
  const headings = [...document.querySelectorAll("h1,h2,h3,h4")]
    .slice(0, 30)
    .map((el) => `${el.tagName.toLowerCase()}: ${(el.textContent || "").trim().slice(0, 120)}`);
  const text = (document.body?.innerText || "").replace(/\s+/g, " ").trim().slice(0, 800);
  return {
    url: location.href,
    title: document.title,
    outline: headings,
    textPreview: text,
  };
})()"#,
            false,
        )
        .await
}

pub fn format_snap(value: &Value) -> String {
    let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let mut out = format!("URL: {url}\nTitle: {title}\n");

    if let Some(outline) = value.get("outline").and_then(|v| v.as_array()) {
        if !outline.is_empty() {
            out.push_str("\nOutline:\n");
            for item in outline {
                if let Some(line) = item.as_str() {
                    out.push_str("  ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
    }

    if let Some(preview) = value.get("textPreview").and_then(|v| v.as_str()) {
        if !preview.is_empty() {
            out.push_str("\nText preview:\n  ");
            out.push_str(preview);
            out.push('\n');
        }
    }

    out
}

pub async fn capture_html(session: &CdpSession, preferred_url: Option<&str>) -> Result<String> {
    session.attach_to_best_tab(preferred_url).await?;
    let value = session
        .evaluate(
            "document.documentElement ? document.documentElement.outerHTML : ''",
            false,
        )
        .await?;
    let html = value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("page HTML was not a string"))?;
    ensure_non_empty_page(session, &html, "document HTML").await?;
    Ok(html)
}

pub async fn capture_body_html(
    session: &CdpSession,
    preferred_url: Option<&str>,
) -> Result<String> {
    session.attach_to_best_tab(preferred_url).await?;
    let value = session
        .evaluate("document.body ? document.body.innerHTML : ''", false)
        .await?;
    let html = value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("body HTML was not a string"))?;
    ensure_non_empty_page(session, &html, "body HTML").await?;
    Ok(html)
}

async fn ensure_non_empty_page(session: &CdpSession, html: &str, label: &str) -> Result<()> {
    if !html.trim().is_empty() {
        return Ok(());
    }
    let diag = session.page_diagnostics().await.unwrap_or(Value::Null);
    let url = diag.get("url").and_then(|v| v.as_str()).unwrap_or("?");
    let title = diag.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let ready = diag
        .get("readyState")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let text_len = diag
        .get("bodyTextLen")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    bail!(
        "{label} is empty on CDP tab `{url}` (title: {title}, readyState: {ready}, bodyTextLen: {text_len}). \
         pagemd may be attached to the wrong Chrome tab (about:blank / restore bubble) — click the real page tab or use /goto, then retry."
    );
}

pub fn html_to_markdown(html: &str) -> Result<String> {
    htmd::convert(html).map_err(|e| anyhow::anyhow!("html to markdown: {e}"))
}
