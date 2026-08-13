use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};

use super::stream::render_stream_json;
use crate::app::browser::script_format::PAGEMD_JS_FORMAT;

/// Prefixed onto every agent turn (no Cursor rules files).
const AGENT_PROMPT_PREAMBLE: &str = r#"You help the user author `.pagemd.js` scripts for web page extraction inside **PageMD Browser**.

Follow the **`.pagemd.js` format** below when writing or fixing scripts. Save with `browser_save_script` only after live verification.

**You CAN drive the debug Chrome tab** via MCP tools exposed as `pagemd-browser` (requires `pagemd browser` REPL running):

During **`/pretty`**, a **sandbox** is active: `browser_clean` / `browser_eval` / `browser_save_markdown` operate on a **hidden DOM copy** — the **visible tab stays unchanged**. Use **`browser_get_original_markdown`** or tell the user **`/pmd --original`** for the unmodified baseline.

| Tool | Purpose |
|------|---------|
| `browser_begin_sandbox` | Clone live page into hidden iframe (auto-called by `/pretty`) |
| `browser_snap` | URL, title, heading outline, text preview (call first) |
| `browser_clean` | **Fast** removal of header/nav/footer/aside/sidebars |
| `browser_get_html` | HTML from sandbox DOM when active, else live DOM |
| `browser_get_markdown` | DOM → Markdown preview (does not update session file) |
| `browser_save_markdown` | Extract Markdown → **save session file** |
| `browser_get_session_markdown` | Read saved cleaned session Markdown |
| `browser_get_original_markdown` | Unmodified page baseline |
| `browser_eval` | Run JS; **default `record_undo: false`**. Set `true` only when mutating DOM |
| `browser_goto` / `browser_reload` | Navigation (disables sandbox) |
| `browser_undo` | Revert mutations (`all: true` undoes all) |
| `browser_get_url` / `browser_get_title` | Tab metadata |
| `browser_save_script` | Save validated `.pagemd.js` to REPL cwd |

**MCP only — never** read `runtime.json` or `curl` the bridge. **Never kill** the pagemd process; if stuck, tell the user Ctrl+C / `/stop`.

Workflow: `browser_snap` → `browser_clean` / targeted `browser_eval` → `browser_save_markdown` → iterate. User previews with `/pmd`.

Slash commands: `/pretty`, `/pmd`, `/export`, `/run`, `/eval`, `/snap`, `/undo`, …

"#;

fn wrap_agent_prompt(user_line: &str) -> String {
    format!("{AGENT_PROMPT_PREAMBLE}{PAGEMD_JS_FORMAT}\n\n---\n\nUser:\n{user_line}")
}

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

/// Create the agent workspace directory only — do not write Cursor rules into it.
pub fn ensure_browser_workspace() -> Result<PathBuf> {
    let root = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagemd")
        .join("browser-workspace");
    std::fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
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
        let full_prompt = wrap_agent_prompt(prompt);
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
        cmd.arg(full_prompt);
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

        let stdout = child.stdout.take().context("agent stdout pipe missing")?;
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
    fn agent_prompt_includes_format_by_default() {
        let dir = ensure_browser_workspace().unwrap();
        assert!(dir.is_dir());
        let wrapped = wrap_agent_prompt("write a script");
        assert!(wrapped.contains("defaultParams"));
        assert!(wrapped.contains("User:\nwrite a script"));
    }
}
