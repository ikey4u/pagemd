use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::app::browser::runtime::BrowserRuntime;

pub struct BridgeClient {
    base: String,
    token: String,
    http: Client,
    export_dir: PathBuf,
}

impl BridgeClient {
    pub fn from_workspace(workspace: &Path) -> Result<Self> {
        let runtime = BrowserRuntime::read(workspace)
            .context("pagemd browser session not running (start `pagemd browser` first)")?;
        Ok(Self {
            base: runtime.bridge_url,
            token: runtime.token,
            export_dir: PathBuf::from(runtime.export_dir),
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .context("build bridge HTTP client")?,
        })
    }

    pub fn export_dir(&self) -> &Path {
        &self.export_dir
    }

    pub fn get_text(&self, path: &str) -> Result<String> {
        let value = self.request(reqwest::Method::GET, path, None)?;
        value
            .get("text")
            .or_else(|| value.get("url"))
            .or_else(|| value.get("title"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                if value.get("changed").is_some() || value.get("ok").is_some() {
                    Some(value.to_string())
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("unexpected bridge response: {value}"))
    }

    pub fn post_text(&self, path: &str, body: Value) -> Result<String> {
        let value = self.request(reqwest::Method::POST, path, Some(body))?;
        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
            return Ok(text.to_owned());
        }
        if let Some(text) = value.get("result").map(|v| v.to_string()) {
            return Ok(text);
        }
        Ok(value.to_string())
    }

    fn request(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let mut req = self
            .http
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", self.token));
        if let Some(body) = body {
            req = req.json(&body);
        }
        let resp = req
            .send()
            .with_context(|| format!("bridge request {url}"))?;
        let status = resp.status();
        let value: Value = resp.json().context("parse bridge JSON")?;
        if !status.is_success() {
            let err = value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("bridge request failed");
            anyhow::bail!("{err}");
        }
        Ok(value)
    }
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "browser_begin_sandbox",
            "Clone the live page into a hidden iframe sandbox. Visible tab stays unchanged. `/pretty` calls this automatically.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "browser_snap",
            "Fast page summary: URL, title, heading outline, text preview (~1s). Uses sandbox DOM when active. Call first before cleaning.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "browser_get_html",
            "Get HTML from sandbox DOM when active, else live DOM. Mutations from browser_eval are reflected immediately.",
            json!({
                "type": "object",
                "properties": {
                    "body_only": { "type": "boolean", "description": "Return document.body innerHTML only (default false)" },
                    "max_chars": { "type": "integer", "description": "Truncate output (default 50000)" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_get_markdown",
            "Extract Markdown from sandbox or live DOM (preview only; does not update session file).",
            json!({
                "type": "object",
                "properties": {
                    "max_chars": { "type": "integer", "description": "Truncate output (default 50000)" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_save_markdown",
            "Extract Markdown from sandbox/live DOM and save to the per-URL session file. Call once after DOM cleanup — do not write Markdown yourself.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "browser_get_session_markdown",
            "Read saved cleaned session Markdown for the current page URL.",
            json!({
                "type": "object",
                "properties": {
                    "max_chars": { "type": "integer", "description": "Truncate output (default 50000)" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_get_original_markdown",
            "Read unmodified page baseline Markdown (saved when sandbox begins). Compare with browser_get_session_markdown.",
            json!({
                "type": "object",
                "properties": {
                    "max_chars": { "type": "integer", "description": "Truncate output (default 50000)" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_clean",
            "Remove common page chrome (header, nav, footer, aside, sidebars, ads) in one fast call. Operates on sandbox when active.",
            json!({
                "type": "object",
                "properties": {
                    "extra_selectors": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Additional CSS selectors to remove"
                    }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_eval",
            "Run JavaScript in sandbox iframe when active, else live page MAIN world. Default record_undo=false (fast). Set record_undo=true only when mutating DOM.",
            json!({
                "type": "object",
                "properties": {
                    "expression": { "type": "string", "description": "JavaScript expression to evaluate" },
                    "record_undo": { "type": "boolean", "description": "Snapshot before eval for /undo (default false). Set true only when mutating DOM — slow on large pages." }
                },
                "required": ["expression"],
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_goto",
            "Navigate the debug Chrome tab to a URL.",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" }
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_reload",
            "Reload the current page and reset undo baseline.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "browser_undo",
            "Undo the last DOM mutation in sandbox or live DOM, or restore session baseline when all=true.",
            json!({
                "type": "object",
                "properties": {
                    "all": { "type": "boolean", "description": "Restore baseline DOM for this session" }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "browser_get_url",
            "Get the current tab URL.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "browser_get_title",
            "Get the current document title.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "browser_save_script",
            "Save a validated .pagemd.js script to the pagemd browser session export directory (REPL working directory at startup). Call only after live clean/extract verification passes.",
            json!({
                "type": "object",
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "File name ending in .pagemd.js (e.g. example-com.pagemd.js)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full plain-JS script source (urlPattern, extract(), optional clean())"
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

pub fn call_tool(client: &BridgeClient, name: &str, args: &Value) -> Result<String> {
    match name {
        "browser_begin_sandbox" => client.post_text("/v1/sandbox/begin", json!({})),
        "browser_snap" => client.post_text("/v1/snap", json!({})),
        "browser_get_html" => client.post_text(
            "/v1/html",
            json!({
                "body_only": args.get("body_only").and_then(|v| v.as_bool()),
                "max_chars": args.get("max_chars"),
            }),
        ),
        "browser_get_markdown" => client.post_text(
            "/v1/markdown",
            json!({ "max_chars": args.get("max_chars") }),
        ),
        "browser_save_markdown" => client.post_text("/v1/markdown/save", json!({})),
        "browser_get_session_markdown" => {
            let mut url = "/v1/markdown/session".to_string();
            if let Some(max) = args.get("max_chars") {
                url = format!("{url}?max_chars={max}");
            }
            client.get_text(&url)
        }
        "browser_get_original_markdown" => {
            let mut url = "/v1/markdown/original".to_string();
            if let Some(max) = args.get("max_chars") {
                url = format!("{url}?max_chars={max}");
            }
            client.get_text(&url)
        }
        "browser_clean" => client.post_text(
            "/v1/clean",
            json!({
                "extra_selectors": args.get("extra_selectors").cloned().unwrap_or(json!([])),
            }),
        ),
        "browser_eval" => client.post_text(
            "/v1/eval",
            json!({
                "expression": args.get("expression").and_then(|v| v.as_str()).unwrap_or(""),
                "record_undo": args.get("record_undo").and_then(|v| v.as_bool()),
            }),
        ),
        "browser_goto" => client.post_text(
            "/v1/goto",
            json!({ "url": args.get("url").and_then(|v| v.as_str()).unwrap_or("") }),
        ),
        "browser_reload" => client.post_text("/v1/reload", json!({})),
        "browser_undo" => client.post_text(
            "/v1/undo",
            json!({ "all": args.get("all").and_then(|v| v.as_bool()) }),
        ),
        "browser_get_url" => client.get_text("/v1/url"),
        "browser_get_title" => client.get_text("/v1/title"),
        "browser_save_script" => {
            crate::app::browser::script::save_script_tool(client.export_dir(), args)
        }
        other => anyhow::bail!("unknown tool: {other}"),
    }
}
