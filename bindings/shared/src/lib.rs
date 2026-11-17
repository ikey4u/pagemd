//! What pagemd's iOS and Android bindings share.
//!
//! [`render_html`] turns Markdown into a self-contained HTML document meant
//! for a web view inside an app: content only, the host page's theme, footnotes
//! in the host's style, and the scripts that interactive diagrams need.
//!
//! pagemd writes KaTeX fonts and other resources to a cache directory. Inside
//! an app sandbox the default (`$HOME/.cache`) is often not writable, so a
//! writable one is chosen: the one the app passes to [`set_cache_dir`], else
//! `PAGEMD_CACHE_DIR`, else the first writable of the usual sandbox locations.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pagemd::{render, FootnoteDisplay, HtmlExportOptions, RenderOptions, ScriptEmbed};

static CACHE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Uses `dir` for pagemd's cache (fonts, resources), e.g. the app's caches
/// directory. Fails if it can't be written.
pub fn set_cache_dir(dir: &Path) -> Result<(), String> {
    if !writable(dir) {
        return Err(format!("cache directory is not writable: {}", dir.display()));
    }
    *CACHE_DIR.lock().unwrap() = Some(dir.to_path_buf());
    std::env::set_var("PAGEMD_CACHE_DIR", dir);
    Ok(())
}

/// The cache directory in use, chosen on first use.
pub fn cache_dir() -> Result<PathBuf, String> {
    let mut slot = CACHE_DIR.lock().unwrap();
    if let Some(dir) = slot.as_ref() {
        return Ok(dir.clone());
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("PAGEMD_CACHE_DIR") {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(std::env::temp_dir().join("pagemd-cache"));
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join("Library/Caches/pagemd"));
        candidates.push(home.join("tmp/pagemd-cache"));
        candidates.push(home.join("cache/pagemd"));
    }
    let dir = candidates
        .into_iter()
        .find(|c| writable(c))
        .ok_or_else(|| "no writable cache directory; call set_cache_dir".to_string())?;
    // pagemd reads it from the environment.
    std::env::set_var("PAGEMD_CACHE_DIR", &dir);
    if std::env::var_os("XDG_CACHE_HOME").is_none() {
        std::env::set_var("XDG_CACHE_HOME", &dir);
    }
    *slot = Some(dir.clone());
    Ok(dir)
}

fn writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".pagemd_write_probe");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Markdown to a self-contained HTML document for a web view.
pub fn render_html(markdown: &str) -> Result<String, String> {
    let base_dir = cache_dir()?;
    let mut html = HtmlExportOptions::embedded();
    html.footnotes = FootnoteDisplay::Host;
    // `embedded()` leaves scripts out; interactive diagrams need them.
    html.scripts = ScriptEmbed::Full;
    let out = std::panic::catch_unwind(|| render(markdown, &RenderOptions { html, base_dir, ..Default::default() }))
        .map_err(|_| "pagemd panicked while rendering".to_string())?
        .map_err(|e| e.to_string())?;
    if out.html.is_empty() {
        return Err("pagemd returned empty HTML".into());
    }
    Ok(out.html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_markdown() {
        let html = render_html("# Hi\n\n**bold**").unwrap();
        assert!(html.contains("Hi") && html.contains("bold"));
    }

    #[test]
    fn keeps_the_scripts_diagrams_need() {
        let html = render_html("```diagram html\n<div class=\"rounded-xl bg-sky-50 p-4\">Node</div>\n```\n").unwrap();
        assert!(html.contains("diagram-html-display"));
        assert!(html.contains("tailwind") || html.contains("Tailwind"));
    }

    #[test]
    fn uses_the_cache_directory_given() {
        let dir = std::env::temp_dir().join(format!("pagemd-bindings-test-{}", std::process::id()));
        set_cache_dir(&dir).unwrap();
        assert_eq!(cache_dir().unwrap(), dir);
        assert!(set_cache_dir(Path::new("/proc/not-writable")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
