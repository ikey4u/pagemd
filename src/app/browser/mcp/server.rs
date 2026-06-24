use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use super::bridge_client::{call_tool, tool_definitions, BridgeClient};

pub fn serve_stdio(workspace: &Path) -> Result<()> {
    let client = BridgeClient::from_workspace(workspace)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.context("read MCP stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line).context("parse MCP request")?;
        if request.get("method").and_then(|v| v.as_str()) == Some("notifications/initialized") {
            continue;
        }
        let id = request.get("id").cloned();
        let response = match dispatch(&client, &request) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(err) if id.is_some() => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": err.to_string() } })
            }
            Err(_) => continue,
        };
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn dispatch(client: &BridgeClient, request: &Value) -> Result<Value> {
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "pagemd-browser", "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .context("tools/call missing name")?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let text = call_tool(client, name, &args)?;
            Ok(json!({
                "content": [{ "type": "text", "text": text }],
                "isError": false
            }))
        }
        "ping" => Ok(json!({})),
        _ => anyhow::bail!("unsupported MCP method: {method}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::app::browser::mcp::bridge_client::tool_definitions;

    #[test]
    fn tool_catalog_includes_snap_and_eval() {
        let tools = tool_definitions();
        let names: Vec<_> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()))
            .collect();
        assert!(names.contains(&"browser_snap"));
        assert!(names.contains(&"browser_clean"));
        assert!(names.contains(&"browser_begin_sandbox"));
        assert!(names.contains(&"browser_get_original_markdown"));
        assert!(names.contains(&"browser_save_markdown"));
        assert!(names.contains(&"browser_get_session_markdown"));
        assert!(names.contains(&"browser_save_script"));
    }
}
