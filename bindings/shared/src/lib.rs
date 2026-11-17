//! What pagemd's iOS and Android bindings share.
//!
//! [`render_html`] turns Markdown into a self-contained HTML document meant
//! for a web view inside an app: content only, the host page's theme, footnotes
//! in the host's style, and the scripts that interactive diagrams need.
//!
//! pagemd writes KaTeX fonts to a cache directory. Inside an app sandbox the
//! default (`$HOME/.cache`) is often not writable, so a writable one is
//! chosen: the one the app passes to [`set_cache_dir`], else
//! `PAGEMD_CACHE_DIR`, else the first writable of the usual sandbox locations.
//!
//! The choice is handed to pagemd through [`RenderOptions`], never through the
//! process environment: `setenv` races with other threads reading the
//! environment, and apps always have other threads.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use pagemd_core::{
    install_katex_fonts, render, FootnoteDisplay, HtmlExportOptions,
    RenderOptions, ScriptEmbed,
};

static CACHE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The cache slot. A panic elsewhere must not poison it: a second panic here
/// would cross the FFI boundary and abort the app.
fn slot() -> MutexGuard<'static, Option<PathBuf>> {
    CACHE_DIR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Uses `dir` for pagemd's cache (fonts), e.g. the app's caches directory.
/// Fails if it can't be written.
pub fn set_cache_dir(dir: &Path) -> Result<(), String> {
    if !writable(dir) {
        return Err(format!(
            "cache directory is not writable: {}",
            dir.display()
        ));
    }
    *slot() = Some(dir.to_path_buf());
    Ok(())
}

/// The cache directory in use, chosen on first use.
pub fn cache_dir() -> Result<PathBuf, String> {
    let mut slot = slot();
    if let Some(dir) = slot.as_ref() {
        return Ok(dir.clone());
    }
    let dir =
        candidates()
            .into_iter()
            .find(|c| writable(c))
            .ok_or_else(|| {
                "no writable cache directory; call set_cache_dir".to_string()
            })?;
    *slot = Some(dir.clone());
    Ok(dir)
}

fn candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
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
    candidates
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
    let katex_fonts = install_katex_fonts(&base_dir.join("katex-fonts"))
        .map_err(|e| format!("{e:#}"))?;
    let mut html = HtmlExportOptions::embedded();
    html.footnotes = FootnoteDisplay::Host;
    // `embedded()` leaves scripts out; interactive diagrams need them.
    html.scripts = ScriptEmbed::Full;
    let opts = RenderOptions {
        html,
        base_dir,
        katex_fonts: Some(katex_fonts),
        ..Default::default()
    };
    let out = std::panic::catch_unwind(|| render(markdown, &opts))
        .map_err(|_| "pagemd panicked while rendering".to_string())?
        .map_err(|e| format!("{e:#}"))?;
    if out.html.is_empty() {
        return Err("pagemd returned empty HTML".into());
    }
    Ok(out.html)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests share the process-wide cache slot; run them one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn temp_cache(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("pagemd-bindings-{name}-{}", std::process::id()))
    }

    #[test]
    fn renders_markdown() {
        let _guard = serial();
        let html = render_html("# Hi\n\n**bold**").unwrap();
        assert!(html.contains("Hi") && html.contains("bold"));
    }

    #[test]
    fn keeps_the_scripts_diagrams_need() {
        let _guard = serial();
        let html = render_html("```diagram html\n<div class=\"rounded-xl bg-sky-50 p-4\">Node</div>\n```\n").unwrap();
        assert!(html.contains("diagram-html-display"));
        assert!(html.contains("tailwind") || html.contains("Tailwind"));
    }

    #[test]
    fn writes_math_fonts_to_the_cache_directory_given() {
        let _guard = serial();
        let dir = temp_cache("math");
        set_cache_dir(&dir).unwrap();
        let html = render_html("$$x^2$$").unwrap();
        assert!(html.contains("<svg"));
        assert!(dir.join("katex-fonts/KaTeX_Main-Regular.ttf").exists());
        assert!(std::env::var_os("XDG_CACHE_HOME")
            .is_none_or(|v| Path::new(&v) != dir));
    }

    #[test]
    fn uses_the_cache_directory_given() {
        let _guard = serial();
        let dir = temp_cache("set");
        set_cache_dir(&dir).unwrap();
        assert_eq!(cache_dir().unwrap(), dir);
        assert!(set_cache_dir(Path::new("/proc/not-writable")).is_err());
        assert_eq!(
            cache_dir().unwrap(),
            dir,
            "a failed set keeps the previous directory"
        );
    }
}
