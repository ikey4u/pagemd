use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{json, Value};

pub fn ensure_mcp_config(workspace: &Path) -> Result<()> {
    let cursor_dir = workspace.join(".cursor");
    std::fs::create_dir_all(&cursor_dir)
        .with_context(|| format!("create {}", cursor_dir.display()))?;

    let pagemd = std::env::current_exe().context("resolve pagemd executable path")?;
    let workspace_str = workspace.to_string_lossy().to_string();
    let entry = json!({
        "command": pagemd,
        "args": ["browser-mcp", "--workspace", workspace_str]
    });

    let mcp_path = cursor_dir.join("mcp.json");
    let mut doc: Value = if mcp_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&mcp_path)?).unwrap_or(json!({}))
    } else {
        json!({})
    };

    if !doc.get("mcpServers").map(|v| v.is_object()).unwrap_or(false) {
        doc["mcpServers"] = json!({});
    }
    doc["mcpServers"]["pagemd-browser"] = entry;

    std::fs::write(
        &mcp_path,
        format!("{}\n", serde_json::to_string_pretty(&doc)?),
    )
    .with_context(|| format!("write {}", mcp_path.display()))?;
    Ok(())
}
