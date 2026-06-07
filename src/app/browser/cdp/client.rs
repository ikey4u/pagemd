use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::targets::{
    format_target_choices, list_page_targets, pick_best_page_target, urls_match, PageTarget,
};

#[derive(Debug)]
enum IoCommand {
    Send {
        id: u64,
        payload: String,
        reply: oneshot::Sender<Result<Value>>,
    },
}

struct CdpConnection {
    tx: mpsc::UnboundedSender<IoCommand>,
}

#[derive(Clone)]
pub struct CdpSession {
    port: u16,
    connection: Arc<Mutex<CdpConnection>>,
    id_gen: Arc<AtomicU64>,
    attached_url: Arc<Mutex<String>>,
}

impl CdpSession {
    pub async fn connect(port: u16) -> Result<Self> {
        Self::connect_with_hint(port, None).await
    }

    pub async fn connect_with_hint(port: u16, preferred_url: Option<&str>) -> Result<Self> {
        let targets = list_page_targets(port).await?;
        let target = pick_best_page_target(&targets, preferred_url).with_context(|| {
            format!(
                "no suitable Chrome page target on port {port}\n{}",
                format_target_choices(&targets)
            )
        })?;
        Self::connect_to_target(port, target.clone()).await
    }

    async fn connect_to_target(port: u16, target: PageTarget) -> Result<Self> {
        let (tx, id_gen) = open_connection(&target.ws_url).await?;
        let session = Self {
            port,
            connection: Arc::new(Mutex::new(CdpConnection { tx })),
            id_gen,
            attached_url: Arc::new(Mutex::new(target.url)),
        };
        session.call("Runtime.enable", json!({})).await?;
        session.call("Page.enable", json!({})).await?;
        Ok(session)
    }

    /// Re-attach when the CDP session is on the wrong tab. Skips target listing when
    /// already attached to a usable HTTP(S) page matching `preferred`.
    pub async fn attach_to_best_tab(&self, preferred: Option<&str>) -> Result<String> {
        let current = self.current_url().await.unwrap_or_default();
        if Self::is_usable_page_url(&current) {
            let preferred_ok = preferred
                .map(|p| urls_match(&current, p))
                .unwrap_or(true);
            if preferred_ok {
                return Ok(current);
            }
        }

        let targets = list_page_targets(self.port).await?;
        let best = pick_best_page_target(&targets, preferred).with_context(|| {
            format!(
                "no suitable Chrome page target on port {}\n{}",
                self.port,
                format_target_choices(&targets)
            )
        })?;

        let current = self.current_url().await.unwrap_or_default();
        if urls_match(&current, &best.url) {
            return Ok(current);
        }

        self.reconnect(best.clone()).await?;
        Ok(self.current_url().await.unwrap_or_else(|_| best.url.clone()))
    }

    fn is_usable_page_url(url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    async fn reconnect(&self, target: PageTarget) -> Result<()> {
        let (tx, _) = open_connection(&target.ws_url).await?;
        {
            let mut conn = self.connection.lock().await;
            conn.tx = tx;
        }
        *self.attached_url.lock().await = target.url.clone();
        self.call("Runtime.enable", json!({})).await?;
        self.call("Page.enable", json!({})).await?;
        Ok(())
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.call_with_timeout(method, params, Duration::from_secs(30)).await
    }

    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        max_wait: Duration,
    ) -> Result<Value> {
        let id = self.id_gen.fetch_add(1, Ordering::Relaxed);
        let payload = json!({
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();

        let (reply_tx, reply_rx) = oneshot::channel();
        {
            let conn = self.connection.lock().await;
            conn.tx
                .send(IoCommand::Send {
                    id,
                    payload,
                    reply: reply_tx,
                })
                .map_err(|_| anyhow!("CDP IO task stopped"))?;
        }

        timeout(max_wait, reply_rx)
            .await
            .map_err(|_| anyhow!("CDP call timed out: {method}"))?
            .map_err(|_| anyhow!("CDP reply channel closed"))?
    }

    pub async fn evaluate(&self, expression: &str, await_promise: bool) -> Result<Value> {
        let result = self
            .call_with_timeout(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": await_promise,
                    "userGesture": true,
                }),
                Duration::from_secs(15),
            )
            .await?;

        if let Some(details) = result.get("exceptionDetails") {
            bail!("{}", format_js_exception(details));
        }

        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        self.call("Page.navigate", json!({ "url": url })).await?;
        self.wait_for_load(Duration::from_secs(30)).await?;
        *self.attached_url.lock().await = self.current_url().await.unwrap_or_else(|_| url.to_owned());
        Ok(())
    }

    pub async fn reload(&self) -> Result<()> {
        self.call("Page.reload", json!({ "ignoreCache": false }))
            .await?;
        self.wait_for_load(Duration::from_secs(30)).await
    }

    pub async fn current_url(&self) -> Result<String> {
        let value = self
            .evaluate("location.href", false)
            .await
            .context("read location.href")?;
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("location.href was not a string"))
    }

    pub async fn current_title(&self) -> Result<String> {
        let value = self.evaluate("document.title", false).await?;
        Ok(value.as_str().unwrap_or("").to_owned())
    }

    pub async fn page_diagnostics(&self) -> Result<Value> {
        self.evaluate(
            r#"(() => ({
  url: location.href,
  title: document.title,
  readyState: document.readyState,
  bodyHtmlLen: document.body?.innerHTML?.length ?? 0,
  bodyTextLen: (document.body?.innerText || "").trim().length,
}))()"#,
            false,
        )
        .await
    }

    async fn wait_for_load(&self, max: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + max;
        loop {
            let state = self
                .evaluate(
                    "document.readyState === 'complete' ? 'complete' : document.readyState",
                    false,
                )
                .await?;
            if state.as_str() == Some("complete") {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("Timed out waiting for page load");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

pub fn format_js_exception(details: &Value) -> String {
    if let Some(desc) = details
        .get("exception")
        .and_then(|e| e.get("description"))
        .and_then(|v| v.as_str())
    {
        return desc.to_string();
    }
    if let Some(text) = details.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(line) = details.get("lineNumber").and_then(|v| v.as_u64()) {
        if let Some(col) = details.get("columnNumber").and_then(|v| v.as_u64()) {
            return format!("Uncaught exception at line {line}, column {col}");
        }
    }
    details.to_string()
}

async fn open_connection(ws_url: &str) -> Result<(mpsc::UnboundedSender<IoCommand>, Arc<AtomicU64>)> {
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .with_context(|| format!("connect CDP websocket {ws_url}"))?;

    let (mut write, mut read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<IoCommand>();
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let pending_reader = Arc::clone(&pending);
    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                            let mut map = pending_reader.lock().await;
                            if let Some(reply) = map.remove(&id) {
                                let result = if let Some(err) = value.get("error") {
                                    Err(anyhow!("CDP error: {err}"))
                                } else {
                                    Ok(value.get("result").cloned().unwrap_or(Value::Null))
                                };
                                let _ = reply.send(result);
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
        let mut map = pending_reader.lock().await;
        for (_, reply) in map.drain() {
            let _ = reply.send(Err(anyhow!("CDP connection closed")));
        }
    });

    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                IoCommand::Send {
                    id,
                    payload,
                    reply,
                } => {
                    pending.lock().await.insert(id, reply);
                    if write.send(Message::Text(payload.into())).await.is_err() {
                        let mut map = pending.lock().await;
                        if let Some(reply) = map.remove(&id) {
                            let _ = reply.send(Err(anyhow!("CDP write failed")));
                        }
                        break;
                    }
                }
            }
        }
        let _ = write.close().await;
    });

    Ok((tx, Arc::new(AtomicU64::new(1))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_js_exception_prefers_description() {
        let details = json!({
            "exception": { "description": "ReferenceError: Data is not defined" },
            "text": "Uncaught"
        });
        assert_eq!(
            format_js_exception(&details),
            "ReferenceError: Data is not defined"
        );
    }
}
