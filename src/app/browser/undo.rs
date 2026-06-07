use anyhow::{bail, Result};
use serde_json::json;

use super::cdp::CdpSession;
use super::sandbox;

/// Body HTML larger than this cannot be snapshotted for undo (CDP round-trip cost).
const MAX_UNDO_HTML_CHARS: usize = 1_500_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DomTarget {
    #[default]
    Live,
    Sandbox,
}

#[derive(Clone, Debug)]
struct UndoEntry {
    body_html: String,
    url: String,
}

pub struct UndoStack {
    baseline: Option<UndoEntry>,
    entries: Vec<UndoEntry>,
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            baseline: None,
            entries: Vec::new(),
            max_depth,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn reset(&mut self) {
        self.baseline = None;
        self.entries.clear();
    }

    pub async fn capture_baseline(&mut self, session: &CdpSession, target: DomTarget) -> Result<()> {
        if self.baseline.is_none() {
            self.baseline = Some(capture_entry(session, target).await?);
        }
        Ok(())
    }

    pub async fn push_before_mutate(&mut self, session: &CdpSession, target: DomTarget) -> Result<()> {
        self.capture_baseline(session, target).await?;
        let entry = capture_entry(session, target).await?;
        self.entries.push(entry);
        if self.entries.len() > self.max_depth {
            self.entries.remove(0);
        }
        Ok(())
    }

    pub async fn undo_one(&mut self, session: &CdpSession, target: DomTarget) -> Result<bool> {
        let Some(entry) = self.entries.pop() else {
            return Ok(false);
        };
        restore_entry(session, &entry, target).await?;
        Ok(true)
    }

    pub async fn undo_all(&mut self, session: &CdpSession, target: DomTarget) -> Result<bool> {
        let Some(baseline) = self.baseline.clone() else {
            return Ok(false);
        };
        restore_entry(session, &baseline, target).await?;
        self.entries.clear();
        Ok(true)
    }
}

async fn capture_entry(session: &CdpSession, target: DomTarget) -> Result<UndoEntry> {
    let (body_html, url) = match target {
        DomTarget::Live => {
            let value = session
                .evaluate(
                    r#"(() => ({
  bodyHtml: document.body ? document.body.innerHTML : "",
  url: location.href,
}))()"#,
                    false,
                )
                .await?;
            (
                value
                    .get("bodyHtml")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                value
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
            )
        }
        DomTarget::Sandbox => sandbox::capture_undo_entry(session).await?,
    };
    let chars = body_html.chars().count();
    if chars > MAX_UNDO_HTML_CHARS {
        bail!(
            "page body HTML is too large for undo snapshot ({chars} chars; max {MAX_UNDO_HTML_CHARS}). \
             Use browser_eval with record_undo=false for read-only checks, or clean in smaller steps."
        );
    }

    Ok(UndoEntry { body_html, url })
}

async fn restore_entry(session: &CdpSession, entry: &UndoEntry, target: DomTarget) -> Result<()> {
    match target {
        DomTarget::Sandbox => {
            sandbox::restore_body_html(session, &entry.body_html).await?;
            return Ok(());
        }
        DomTarget::Live => {
            let current_url = session.current_url().await.unwrap_or_default();
            if current_url != entry.url && !entry.url.is_empty() {
                session.navigate(&entry.url).await?;
            }

            let html_json = json!(entry.body_html);
            session
                .evaluate(
                    &format!(
                        r#"(() => {{
  const html = {html_json};
  if (!document.body) {{
    document.documentElement.innerHTML = "<head></head><body></body>";
  }}
  document.body.innerHTML = html;
  return true;
}})()"#
                    ),
                    false,
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_undo_limit_is_documented() {
        assert!(MAX_UNDO_HTML_CHARS >= 500_000);
    }
}
