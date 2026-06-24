use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserRuntime {
    pub bridge_url: String,
    pub token: String,
    pub cdp_port: u16,
    pub pid: u32,
    /// Directory where `/export` saves `.pagemd.js` (REPL cwd at startup).
    pub export_dir: String,
}

impl BrowserRuntime {
    pub fn path(workspace: &Path) -> PathBuf {
        workspace.join(".pagemd").join("runtime.json")
    }

    pub fn write(&self, workspace: &Path) -> Result<()> {
        let path = Self::path(workspace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, format!("{text}\n"))
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn read(workspace: &Path) -> Result<Self> {
        let path = Self::path(workspace);
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).context("parse browser runtime.json")
    }

    pub fn remove(workspace: &Path) {
        let _ = std::fs::remove_file(Self::path(workspace));
    }
}
