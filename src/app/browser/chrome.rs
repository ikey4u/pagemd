use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use reqwest::blocking::Client;
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::cli::BrowserArgs;

pub struct ChromeProcess {
    child: Option<Child>,
    user_data_dir: PathBuf,
}

impl ChromeProcess {
    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_dir
    }

    /// Ask Chrome to quit via CDP, then wait for the child process to exit.
    pub async fn shutdown_gracefully(&mut self, port: u16) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        let _ = request_browser_close(port).await;

        if wait_for_child(&mut child, Duration::from_secs(10)) {
            return;
        }

        signal_terminate(child.id());
        if wait_for_child(&mut child, Duration::from_secs(4)) {
            return;
        }

        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for ChromeProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            signal_terminate(child.id());
            if wait_for_child(&mut child, Duration::from_secs(2)) {
                return;
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn request_browser_close(port: u16) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build HTTP client")?;
    let version_url = format!("http://127.0.0.1:{port}/json/version");
    let version: Value = client
        .get(&version_url)
        .send()
        .await
        .with_context(|| format!("GET {version_url}"))?
        .error_for_status()
        .context("CDP /json/version error")?
        .json()
        .await
        .context("parse /json/version")?;

    let ws_url = version
        .get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .context("browser webSocketDebuggerUrl missing in /json/version")?;

    let (mut ws, _) = connect_async(ws_url)
        .await
        .with_context(|| format!("connect browser CDP {ws_url}"))?;

    ws.send(Message::Text(
        r#"{"id":1,"method":"Browser.close","params":{}}"#.into(),
    ))
    .await
    .context("Browser.close send")?;

    let _ = tokio::time::timeout(Duration::from_secs(3), ws.next()).await;
    let _ = ws.close(None).await;
    Ok(())
}

fn wait_for_child(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return false,
        }
    }
    false
}

#[cfg(unix)]
fn signal_terminate(pid: u32) {
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
}

#[cfg(not(unix))]
fn signal_terminate(_pid: u32) {}

pub fn resolve_profile_dir(args: &BrowserArgs) -> Result<PathBuf> {
    if let Some(dir) = &args.user_data_dir {
        return Ok(dir.clone());
    }
    if args.clean {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pagemd")
            .join("profiles")
            .join(format!("ephemeral-{nanos}"));
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create profile dir {}", dir.display()))?;
        return Ok(dir);
    }
    let dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagemd")
        .join("chrome-profile");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create profile dir {}", dir.display()))?;
    Ok(dir)
}

pub fn find_chrome(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        bail!("Chrome not found at {}", path.display());
    }

    #[cfg(target_os = "macos")]
    {
        for candidate in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        ] {
            let path = PathBuf::from(candidate);
            if path.exists() {
                return Ok(path);
            }
        }
    }

    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chrome",
    ] {
        if let Ok(path) = which::which(name) {
            return Ok(path);
        }
    }

    bail!("Could not find Chrome. Install Chrome or pass --chrome-path /path/to/chrome");
}

pub fn spawn_chrome(args: &BrowserArgs) -> Result<ChromeProcess> {
    let chrome = find_chrome(args.chrome_path.as_deref())?;
    let profile = resolve_profile_dir(args)?;

    let mut cmd = Command::new(&chrome);
    cmd.arg(format!("--remote-debugging-port={}", args.port))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-background-networking")
        .arg("--disable-sync")
        .arg("--disable-translate")
        .arg("--metrics-recording-only")
        .arg("--disable-features=TranslateUI")
        .arg("--disable-session-crashed-bubble")
        .arg("--hide-crash-restore-bubble")
        .arg("--noerrdialogs")
        .arg("--disable-infobars")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if args.headless {
        cmd.arg("--headless=new");
    }

    if let Some(url) = &args.url {
        cmd.arg(url);
    } else {
        cmd.arg("about:blank");
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("spawn Chrome at {}", chrome.display()))?;

    wait_for_cdp(args.port, Duration::from_secs(30))?;

    Ok(ChromeProcess {
        child: Some(child),
        user_data_dir: profile,
    })
}

pub fn wait_for_cdp(port: u16, timeout: Duration) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("build HTTP client")?;
    let url = format!("http://127.0.0.1:{port}/json/version");
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                if let Ok(body) = resp.text() {
                    if body.contains("webSocketDebuggerUrl") {
                        return Ok(());
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    bail!("Timed out waiting for Chrome CDP on port {port}");
}

pub fn ensure_cdp(args: &BrowserArgs) -> Result<Option<ChromeProcess>> {
    if args.connect {
        wait_for_cdp(args.port, Duration::from_secs(5))?;
        return Ok(None);
    }
    spawn_chrome(args).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_profile_is_under_cache() {
        let args = BrowserArgs {
            url: None,
            port: 9222,
            chrome_path: None,
            user_data_dir: None,
            clean: true,
            connect: false,
            headless: true,
            provider: "auto".into(),
            no_ai: false,
            prompt: ">".into(),
        };
        let dir = resolve_profile_dir(&args).unwrap();
        assert!(dir.to_string_lossy().contains("ephemeral-"));
    }
}
