use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};

use super::stream::render_stream_json;

const WORKSPACE_RULES: &str = r#"---
description: PageMD browser — script authoring for live pages
alwaysApply: true
---

You help the user author `.pagemd.js` scripts for web page extraction inside **PageMD Browser**.

**You CAN drive the debug Chrome tab** via MCP tools exposed as `pagemd-browser` (requires `pagemd browser` REPL running):

During **`/pretty`**, a **sandbox** is active: `browser_clean` / `browser_eval` / `browser_save_markdown` operate on a **hidden DOM copy** — the **visible tab stays unchanged** for side-by-side comparison. Use **`browser_get_original_markdown`** or tell the user to run **`/pmd --original`** for the unmodified baseline.

| Tool | Purpose |
|------|---------|
| `browser_begin_sandbox` | Clone live page into hidden iframe (auto-called by `/pretty`) |
| `browser_snap` | URL, title, heading outline, text preview (call first) |
| `browser_clean` | **Fast** removal of header/nav/footer/aside/sidebars — prefer over custom eval for initial cleanup |
| `browser_get_html` | Full or body HTML from **sandbox DOM when active**, else live DOM |
| `browser_get_markdown` | DOM → Markdown preview (does not update session file) |
| `browser_save_markdown` | Sandbox/live DOM → extract Markdown → **save session file** |
| `browser_get_session_markdown` | Read saved cleaned session Markdown |
| `browser_get_original_markdown` | Read unmodified page baseline (before cleanup) |
| `browser_eval` | Run JS; **default `record_undo: false`** (fast probes). Set `true` only when mutating DOM |
| `browser_goto` / `browser_reload` | Navigation (disables sandbox) |
| `browser_undo` | Revert last sandbox/live mutation (`all: true` restores baseline) |
| `browser_get_url` / `browser_get_title` | Current tab metadata |
| `browser_save_script` | Save validated `.pagemd.js` to REPL cwd |

**MCP only — never** read `runtime.json` or `curl` the bridge from shell. All page operations go through `pagemd-browser` MCP tools above.

**Never kill or restart the pagemd process** (`kill`, `pkill`, stopping `cargo run`, etc.). If MCP tools time out or the bridge seems stuck, tell the user to press **Ctrl+C** once (interrupts the agent turn only) or run **`/stop`** — do **not** terminate the REPL yourself. Do **not** use shell to "fix" CDP; retry with `browser_eval` and `"record_undo": false` or ask the user to `/reload` the page tab.

Workflow: `browser_snap` → **`browser_clean`** (or one targeted `browser_eval` with undo) → **`browser_save_markdown`** → **`browser_get_session_markdown`** → iterate. User previews with **`/pmd`** (cleaned) and **`/pmd --original`** (baseline).

For read-only DOM checks use `browser_eval` with `"record_undo": false` — do not snapshot the whole page for probes like `document.querySelector(...)`.

When the user is satisfied, they run **`/export`** — verify `clean()`/`extract()` on the live tab, then **`browser_save_script`**. Do **not** save an unverified script.

Slash commands: `/pretty` (sandbox DOM cleanup), `/pmd` (cleaned Markdown), `/pmd --original` (baseline), **`/export`** (save `.pagemd.js`), `/eval`, `/snap`, `/undo`, …

Script contract (plain JS, not ESM):

- Top-level `const urlPattern = "…";` — prefer `https://<host>/*` for whole site
- **Top-level helpers** (`const`, `function`) above hooks — extension bundles them with each hook run
- **`function clean()`** (optional) — mutates live DOM, returns **`{ removed: number }`**
- **`function extract()`** (required) — returns **`{ title, html }`**
- Optional **`function navigate()`**, **`function stop(context)`**
- Use **`function name()`** declarations, not `const clean = () => …`

When suggesting hooks, output complete function bodies the user can paste into `/eval` or save later.
Prefer minimal, robust selectors; mention when the user should run `/eval` to verify on the live page.
"#;

pub fn agent_executable() -> PathBuf {
    std::env::var("PAGEMD_CURSOR_AGENT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| which::which("agent").unwrap_or_else(|_| PathBuf::from("agent")))
}

pub fn detect_cursor() -> bool {
    if let Ok(path) = std::env::var("PAGEMD_CURSOR_AGENT") {
        return PathBuf::from(path).is_file();
    }
    which::which("agent").is_ok()
}

pub fn ensure_browser_workspace() -> Result<PathBuf> {
    let root = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagemd")
        .join("browser-workspace");
    std::fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;

    let rules_dir = root.join(".cursor").join("rules");
    std::fs::create_dir_all(&rules_dir)?;
    let rules_path = rules_dir.join("pagemd-browser.mdc");
    std::fs::write(&rules_path, WORKSPACE_RULES)?;

    let cursorignore = root.join(".cursorignore");
    std::fs::write(
        &cursorignore,
        "# PageMD bridge secrets — use pagemd-browser MCP tools only\n.pagemd/runtime.json\n",
    )?;

    Ok(root)
}

fn trust_marker_path(workspace: &Path) -> PathBuf {
    workspace.join(".pagemd").join("trusted")
}

fn bootstrap_workspace_trust(agent: &Path, workspace: &Path) -> Result<()> {
    let marker = trust_marker_path(workspace);
    if marker.is_file() {
        return Ok(());
    }

    eprintln!("Trusting browser workspace (one-time, may take a few seconds)…");
    let output = Command::new(agent)
        .arg("-p")
        .arg("--trust")
        .arg("--output-format")
        .arg("text")
        .arg("--workspace")
        .arg(workspace)
        .arg("ok")
        .output()
        .with_context(|| format!("bootstrap workspace trust ({})", agent.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("trust browser workspace failed (try `agent login`)\n{stderr}");
    }

    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, "1")?;
    Ok(())
}

struct CursorAgentSession {
    agent: PathBuf,
    workspace: PathBuf,
    running: Mutex<Option<std::process::Child>>,
    interrupted: AtomicBool,
}

impl CursorAgentSession {
    fn run_turn(&self, prompt: &str) -> Result<()> {
        self.interrupted.store(false, Ordering::SeqCst);
        self.interrupt_child_only();

        let workspace = self.workspace.to_string_lossy().into_owned();
        let mut cmd = Command::new(&self.agent);
        cmd.args([
            "-p",
            "--trust",
            "--continue",
            "--approve-mcps",
            "--output-format",
            "stream-json",
            "--stream-partial-output",
            "--workspace",
            workspace.as_str(),
        ]);
        cmd.arg(prompt);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawn agent ({})", self.agent.display()))?;

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        eprintln!("[agent] {line}");
                    }
                }
            });
        }

        let stdout = child
            .stdout
            .take()
            .context("agent stdout pipe missing")?;
        {
            let mut guard = self
                .running
                .lock()
                .map_err(|_| anyhow::anyhow!("agent session lock poisoned"))?;
            *guard = Some(child);
        }

        let result = render_stream_json(BufReader::new(stdout));

        let mut guard = self
            .running
            .lock()
            .map_err(|_| anyhow::anyhow!("agent session lock poisoned"))?;
        if let Some(mut child) = guard.take() {
            let status = child.wait().context("wait for agent")?;
            if self.interrupted.load(Ordering::SeqCst) {
                return Ok(());
            }
            if !status.success() && result.is_ok() {
                anyhow::bail!("agent exited with {status}");
            }
        }

        if self.interrupted.load(Ordering::SeqCst) {
            return Ok(());
        }

        result?;
        Ok(())
    }

    fn interrupt_child_only(&self) {
        if let Ok(mut guard) = self.running.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn interrupt(&self) {
        self.interrupted.store(true, Ordering::SeqCst);
        self.interrupt_child_only();
    }
}

#[derive(Clone)]
pub struct CursorRelay(Arc<CursorAgentSession>);

impl CursorRelay {
    pub async fn send_user_line(&self, line: &str) -> Result<()> {
        let session = Arc::clone(&self.0);
        let line = line.to_owned();
        let mut join = tokio::task::spawn_blocking(move || session.run_turn(&line));

        tokio::select! {
            res = &mut join => {
                res.context("agent task join")??;
                Ok(())
            }
            _ = tokio::signal::ctrl_c() => {
                self.0.interrupt();
                let _ = join.await;
                eprintln!("\n[agent] interrupted (Ctrl+C)");
                Ok(())
            }
        }
    }

    pub async fn send_context_block(&self, block: &str) -> Result<()> {
        self.send_user_line(block).await
    }

    pub fn interrupt(&self) -> Result<()> {
        self.0.interrupt();
        Ok(())
    }

    pub fn shutdown(self) -> Result<()> {
        self.0.interrupt();
        Ok(())
    }
}

pub fn spawn_cursor(workspace: &Path) -> Result<CursorRelay> {
    let agent = agent_executable();
    bootstrap_workspace_trust(&agent, workspace)?;

    Ok(CursorRelay(Arc::new(CursorAgentSession {
        agent,
        workspace: workspace.to_path_buf(),
        running: Mutex::new(None),
        interrupted: AtomicBool::new(false),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_rules_created() {
        let dir = ensure_browser_workspace().unwrap();
        assert!(dir.join(".cursor/rules/pagemd-browser.mdc").exists());
        let ignore = std::fs::read_to_string(dir.join(".cursorignore")).unwrap();
        assert!(ignore.contains("runtime.json"));
    }
}
