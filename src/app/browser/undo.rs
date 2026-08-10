use anyhow::{bail, Result};
use serde_json::Value;

use super::cdp::CdpSession;

/// Command-style undo: record DOM mutations in-page via MutationObserver and
/// invert them. Never ships full-page HTML over CDP.
const INSTALL_JS: &str = r#"(() => {
  if (window.__PAGEMD_UNDO__ && window.__PAGEMD_UNDO__.__v === 2) {
    return { ok: true, already: true };
  }
  const api = {
    __v: 2,
    maxDepth: 50,
    stack: [],
    recording: null,
    applying: false,
    begin(doc) {
      if (!doc) throw new Error("undo begin: document required");
      if (this.recording) throw new Error("undo transaction already open");
      const records = [];
      const self = this;
      const observer = new MutationObserver((batch) => {
        if (self.applying) return;
        for (const m of batch) records.push(m);
      });
      observer.observe(doc.documentElement || doc, {
        subtree: true,
        childList: true,
        attributes: true,
        attributeOldValue: true,
        characterData: true,
        characterDataOldValue: true,
      });
      this.recording = { observer, records };
      return { ok: true, depth: this.stack.length };
    },
    commit() {
      const rec = this.recording;
      if (!rec) throw new Error("undo commit: no open transaction");
      for (const m of rec.observer.takeRecords()) rec.records.push(m);
      rec.observer.disconnect();
      this.recording = null;
      if (rec.records.length > 0) {
        this.stack.push({ records: rec.records });
        while (this.stack.length > this.maxDepth) this.stack.shift();
      }
      return { ok: true, depth: this.stack.length, recorded: rec.records.length };
    },
    cancel() {
      const rec = this.recording;
      if (!rec) return { ok: true, depth: this.stack.length, reverted: false };
      for (const m of rec.observer.takeRecords()) rec.records.push(m);
      rec.observer.disconnect();
      this.recording = null;
      if (rec.records.length > 0) {
        this._applyInverse(rec.records);
      }
      return { ok: true, depth: this.stack.length, reverted: rec.records.length > 0 };
    },
    undoOne() {
      if (this.recording) throw new Error("undo: finish open transaction first");
      const entry = this.stack.pop();
      if (!entry) return { changed: false, depth: 0 };
      this._applyInverse(entry.records);
      return { changed: true, depth: this.stack.length };
    },
    undoAll() {
      if (this.recording) throw new Error("undo: finish open transaction first");
      let n = 0;
      while (this.stack.length > 0) {
        const entry = this.stack.pop();
        this._applyInverse(entry.records);
        n++;
      }
      return { changed: n > 0, steps: n, depth: 0 };
    },
    reset() {
      if (this.recording) {
        this.recording.observer.disconnect();
        this.recording = null;
      }
      this.stack = [];
      return { ok: true, depth: 0 };
    },
    depth() {
      return this.stack.length;
    },
    _applyInverse(records) {
      this.applying = true;
      try {
        for (let i = records.length - 1; i >= 0; i--) {
          const m = records[i];
          if (m.type === "childList") {
            for (const node of m.addedNodes) {
              if (node.parentNode) node.parentNode.removeChild(node);
            }
            const parent = m.target;
            const anchor = m.nextSibling;
            for (const node of m.removedNodes) {
              parent.insertBefore(node, anchor);
            }
          } else if (m.type === "attributes") {
            const el = m.target;
            const name = m.attributeName;
            if (!name) continue;
            if (m.oldValue === null) {
              el.removeAttribute(name);
            } else {
              el.setAttribute(name, m.oldValue);
            }
          } else if (m.type === "characterData") {
            m.target.data = m.oldValue == null ? "" : m.oldValue;
          }
        }
      } finally {
        this.applying = false;
      }
    },
  };
  window.__PAGEMD_UNDO__ = api;
  return { ok: true, already: false };
})()"#;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DomTarget {
    #[default]
    Live,
    Sandbox,
}

/// Thin Rust handle; mutation records and inverse ops live in the page.
pub struct UndoStack {
    depth: usize,
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            depth: 0,
            max_depth: max_depth.max(1),
        }
    }

    pub fn len(&self) -> usize {
        self.depth
    }

    /// Drop local depth after navigation (page JS is gone).
    pub fn reset(&mut self) {
        self.depth = 0;
    }

    /// Install in-page undo runtime and clear the command stack.
    pub async fn bind(&mut self, session: &CdpSession, _target: DomTarget) -> Result<()> {
        install(session).await?;
        set_max_depth(session, self.max_depth).await?;
        let value = call(session, "reset()").await?;
        self.depth = depth_from(&value);
        Ok(())
    }

    /// Start recording DOM mutations (command capture).
    pub async fn begin_record(&mut self, session: &CdpSession, target: DomTarget) -> Result<()> {
        install(session).await?;
        set_max_depth(session, self.max_depth).await?;
        let doc_expr = doc_expr(target);
        let value = session
            .evaluate(
                &format!(
                    r#"(() => {{
  const doc = {doc_expr};
  if (!doc) throw new Error("undo target document unavailable");
  return window.__PAGEMD_UNDO__.begin(doc);
}})()"#
                ),
                false,
            )
            .await?;
        if value.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            bail!("undo begin failed: {value}");
        }
        Ok(())
    }

    /// Finish recording and push a command if anything changed.
    pub async fn commit_record(&mut self, session: &CdpSession, _target: DomTarget) -> Result<()> {
        let value = call(session, "commit()").await?;
        self.depth = depth_from(&value);
        Ok(())
    }

    /// Revert in-progress mutations without pushing (e.g. failed eval).
    pub async fn cancel_record(&mut self, session: &CdpSession, _target: DomTarget) -> Result<()> {
        let value = call(session, "cancel()").await?;
        self.depth = depth_from(&value);
        Ok(())
    }

    pub async fn undo_one(&mut self, session: &CdpSession, _target: DomTarget) -> Result<bool> {
        install(session).await?;
        let value = call(session, "undoOne()").await?;
        self.depth = depth_from(&value);
        Ok(value
            .get("changed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    pub async fn undo_all(&mut self, session: &CdpSession, _target: DomTarget) -> Result<bool> {
        install(session).await?;
        let value = call(session, "undoAll()").await?;
        self.depth = depth_from(&value);
        Ok(value
            .get("changed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }
}

fn doc_expr(target: DomTarget) -> &'static str {
    match target {
        DomTarget::Live => "document",
        DomTarget::Sandbox => "window.__PAGEMD_SANDBOX_DOC__",
    }
}

async fn install(session: &CdpSession) -> Result<()> {
    session.evaluate(INSTALL_JS, false).await?;
    Ok(())
}

async fn set_max_depth(session: &CdpSession, max_depth: usize) -> Result<()> {
    session
        .evaluate(
            &format!(
                r#"(() => {{
  if (!window.__PAGEMD_UNDO__) return false;
  window.__PAGEMD_UNDO__.maxDepth = {max_depth};
  return true;
}})()"#
            ),
            false,
        )
        .await?;
    Ok(())
}

async fn call(session: &CdpSession, method_call: &str) -> Result<Value> {
    session
        .evaluate(
            &format!(
                r#"(() => {{
  if (!window.__PAGEMD_UNDO__) throw new Error("undo runtime not installed");
  return window.__PAGEMD_UNDO__.{method_call};
}})()"#
            ),
            false,
        )
        .await
}

fn depth_from(value: &Value) -> usize {
    value
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_starts_empty() {
        let s = UndoStack::new(50);
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn reset_clears_local_depth() {
        let mut s = UndoStack::new(10);
        s.depth = 3;
        s.reset();
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn install_js_is_v2_command_runtime() {
        assert!(INSTALL_JS.contains("__v === 2"));
        assert!(INSTALL_JS.contains("MutationObserver"));
        assert!(INSTALL_JS.contains("undoOne"));
        assert!(!INSTALL_JS.contains("innerHTML"));
    }
}
