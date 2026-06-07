use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use serde_json::Value;

use super::cdp::CdpSession;
use super::session_md::SessionMarkdown;
use super::snap;
use super::undo::{DomTarget, UndoStack};

const INIT_JS: &str = r#"(() => {
  let iframe = document.getElementById("pagemd-sandbox");
  if (!iframe) {
    iframe = document.createElement("iframe");
    iframe.id = "pagemd-sandbox";
    iframe.setAttribute("aria-hidden", "true");
    iframe.style.cssText =
      "position:fixed!important;left:-10000px!important;top:0!important;width:1366px!important;height:900px!important;visibility:hidden!important;pointer-events:none!important;border:0!important";
    document.documentElement.appendChild(iframe);
  }
  const doc = iframe.contentDocument;
  const win = iframe.contentWindow;
  if (!doc || !win) throw new Error("sandbox iframe unavailable");
  doc.open();
  doc.write("<!DOCTYPE html><html>" + document.documentElement.innerHTML + "</html>");
  doc.close();
  window.__PAGEMD_SANDBOX_DOC__ = doc;
  window.__PAGEMD_SANDBOX_WIN__ = win;
  return {
    ok: true,
    title: doc.title,
    bodyLen: doc.body ? doc.body.innerHTML.length : 0,
  };
})()"#;

const BODY_HTML_JS: &str = r#"(() => {
  const doc = window.__PAGEMD_SANDBOX_DOC__;
  return doc && doc.body ? doc.body.innerHTML : "";
})()"#;

const SNAP_JS: &str = r#"(() => {
  const doc = window.__PAGEMD_SANDBOX_DOC__;
  if (!doc) return null;
  const headings = [...doc.querySelectorAll("h1,h2,h3,h4")]
    .slice(0, 30)
    .map(el => `${el.tagName.toLowerCase()}: ${(el.textContent || "").trim().slice(0, 120)}`);
  const text = (doc.body?.innerText || "").replace(/\s+/g, " ").trim().slice(0, 800);
  return {
    url: location.href,
    title: doc.title,
    outline: headings,
    textPreview: text,
  };
})()"#;

pub fn is_enabled(flag: &Arc<AtomicBool>) -> bool {
    flag.load(Ordering::SeqCst)
}

pub fn set_enabled(flag: &Arc<AtomicBool>, on: bool) {
    flag.store(on, Ordering::SeqCst);
}

/// Clone the live page into a hidden iframe; save original baseline; keep the visible tab untouched.
pub async fn begin(
    session: &CdpSession,
    session_md: &SessionMarkdown,
    sandbox_enabled: &Arc<AtomicBool>,
    undo: &mut UndoStack,
) -> Result<Value> {
    if is_enabled(sandbox_enabled) {
        return Ok(serde_json::json!({ "ok": true, "already_active": true }));
    }

    session_md.bind_to_page(session).await?;
    session_md.capture_original_baseline(session, None).await?;
    let info = session.evaluate(INIT_JS, false).await?;
    set_enabled(sandbox_enabled, true);
    undo.capture_baseline(session, DomTarget::Sandbox).await?;
    Ok(info)
}

pub async fn eval_expression(session: &CdpSession, expression: &str) -> Result<Value> {
    let expr_json = serde_json::to_string(expression)?;
    let wrapped = format!(
        r#"(() => {{
  const win = window.__PAGEMD_SANDBOX_WIN__;
  if (!win) throw new Error("PageMD sandbox not active — call browser_begin_sandbox or /pretty first");
  return win.eval({expr_json});
}})()"#
    );
    session.evaluate(&wrapped, false).await
}

pub async fn capture_body_html(session: &CdpSession) -> Result<String> {
    let value = session.evaluate(BODY_HTML_JS, false).await?;
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("sandbox body HTML was not a string"))
}

pub async fn capture_page(session: &CdpSession) -> Result<Value> {
    session.evaluate(SNAP_JS, false).await
}

pub async fn markdown_text(session: &CdpSession, max_chars: usize) -> Result<String> {
    let html = capture_body_html(session).await?;
    let md = snap::html_to_markdown(&html)?;
    if md.trim().is_empty() {
        bail!(
            "sandbox markdown export is empty (HTML was {} chars)",
            html.len()
        );
    }
    Ok(super::tools::truncate(md, max_chars))
}

pub async fn capture_undo_entry(session: &CdpSession) -> Result<(String, String)> {
    let value = session
        .evaluate(
            r#"(() => ({
  bodyHtml: (window.__PAGEMD_SANDBOX_DOC__?.body?.innerHTML) || "",
  url: location.href,
}))()"#,
            false,
        )
        .await?;
    let body_html = value
        .get("bodyHtml")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let url = value
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    Ok((body_html, url))
}

pub async fn restore_body_html(session: &CdpSession, body_html: &str) -> Result<()> {
    let html_json = serde_json::to_string(body_html)?;
    session
        .evaluate(
            &format!(
                r#"(() => {{
  const doc = window.__PAGEMD_SANDBOX_DOC__;
  if (!doc) throw new Error("sandbox not active");
  if (!doc.body) {{
    const b = doc.createElement("body");
    doc.documentElement.appendChild(b);
  }}
  doc.body.innerHTML = {html_json};
  return true;
}})()"#
            ),
            false,
        )
        .await?;
    Ok(())
}

pub async fn run_clean(session: &CdpSession, extra_selectors: &[String]) -> Result<Value> {
    super::tools::run_clean_dom_sandbox(session, extra_selectors).await
}
