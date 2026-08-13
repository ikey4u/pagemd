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
    html_to_markdown_rs::convert(html, None)
        .map(|md| cleanup_markdown(&md))
        .map_err(|e| anyhow::anyhow!("html to markdown: {e}"))
}

/// Strip BOM / zero-width chars common in CMS HTML (e.g. Tencent Cloud docs).
fn cleanup_markdown(md: &str) -> String {
    md.chars()
        .filter(|c| !matches!(c, '\u{feff}' | '\u{200b}' | '\u{200c}' | '\u{200d}'))
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_td_is_header_tables_to_markdown() {
        // Tencent Cloud docs use <td class="is-header"> instead of <th>/<thead>.
        let html = r#"
<table class="readonly">
  <colgroup><col width="14%"/><col width="17%"/><col width="38%"/></colgroup>
  <tbody>
    <tr>
      <td class="is-header"><div><span>动态分类</span></div></td>
      <td class="is-header"><div><span>动态名称</span></div></td>
      <td class="is-header"><div><span>相关文档</span></div></td>
    </tr>
    <tr>
      <td><div><span>沙箱实例</span></div></td>
      <td><div><span>Token 独立获取</span></div></td>
      <td><div><a href="/document/product/1814/132406">﻿沙箱访问 Token﻿</a></div></td>
    </tr>
  </tbody>
</table>
"#;
        let md = html_to_markdown(html).unwrap();
        assert!(
            md.contains("| 动态分类 | 动态名称 | 相关文档 |"),
            "expected markdown header row, got:\n{md}"
        );
        assert!(md.contains("| --- |"), "expected separator row, got:\n{md}");
        assert!(
            md.contains("| 沙箱实例 | Token 独立获取 |"),
            "expected data row, got:\n{md}"
        );
        assert!(
            md.contains("[沙箱访问 Token](/document/product/1814/132406)"),
            "expected cleaned link without BOM, got:\n{md}"
        );
        assert!(!md.contains('\u{feff}'));
    }

    #[test]
    fn converts_classic_th_tables() {
        let html = r#"
<table>
  <thead><tr><th>A</th><th>B</th></tr></thead>
  <tbody><tr><td>1</td><td>2</td></tr></tbody>
</table>
"#;
        let md = html_to_markdown(html).unwrap();
        assert!(md.contains("| A | B |"), "{md}");
        assert!(md.contains("| 1 | 2 |"), "{md}");
    }
}
