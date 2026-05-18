use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regex::Regex;

const ASSET_DIR_NAMES: &[&str] = &["assets", "images", "img", "static", "media", "figures"];

fn markdown_link_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!?\[[^\]]*\]\(([^)]+)\)").expect("markdown link regex"))
}

fn html_attr_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?:src|href|poster)\s*=\s*["']([^"']+)["']"#).expect("html attr regex")
    })
}

fn css_url_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"url\(\s*['"]?([^'")]+)['"]?\s*\)"#).expect("css url regex")
    })
}

fn is_local_reference(reference: &str) -> bool {
    let reference = reference.trim();
    !reference.is_empty()
        && !reference.starts_with('#')
        && !reference.starts_with("http://")
        && !reference.starts_with("https://")
        && !reference.starts_with("data:")
        && !reference.starts_with("mailto:")
        && !reference.starts_with("javascript:")
}

fn resolve_local_path(reference: &str, base_dir: &Path) -> Option<PathBuf> {
    let reference = reference.trim();
    if !is_local_reference(reference) {
        return None;
    }
    let path = Path::new(reference);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else if reference.starts_with('/') {
        PathBuf::from(reference)
    } else {
        base_dir.join(reference)
    };
    let path = path.canonicalize().unwrap_or(path);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Collect local resource paths referenced in Markdown / raw HTML fragments.
pub fn discover_from_sources(sources: &[(PathBuf, String)]) -> Vec<PathBuf> {
    let mut paths = HashSet::new();

    for (input, source) in sources {
        let base_dir = input.parent().unwrap_or_else(|| Path::new("."));

        for cap in markdown_link_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                if let Some(path) = resolve_local_path(reference.as_str(), base_dir) {
                    paths.insert(path);
                }
            }
        }

        for cap in html_attr_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                if let Some(path) = resolve_local_path(reference.as_str(), base_dir) {
                    paths.insert(path);
                }
            }
        }

        for cap in css_url_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                if let Some(path) = resolve_local_path(reference.as_str(), base_dir) {
                    paths.insert(path);
                }
            }
        }
    }

    paths.into_iter().collect()
}

/// Initial watch set: input files, sibling assets, common asset subdirectories, and discovered paths.
pub fn collect_initial_watch_paths(inputs: &[PathBuf], sources: &[(PathBuf, String)]) -> Vec<PathBuf> {
    let mut paths = HashSet::new();

    for input in inputs {
        let canonical = input.canonicalize().unwrap_or_else(|_| input.clone());
        paths.insert(canonical.clone());

        let parent = input.parent().unwrap_or_else(|| Path::new("."));
        if !parent.as_os_str().is_empty() {
            let parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            for name in ASSET_DIR_NAMES {
                let dir = parent.join(name);
                if dir.is_dir() {
                    paths.insert(dir.canonicalize().unwrap_or(dir));
                }
            }
        }
    }

    for path in discover_from_sources(sources) {
        paths.insert(path);
    }

    paths.into_iter().collect()
}

/// Additional paths to watch after a successful render (deduped by the caller).
pub fn discover_watch_paths(inputs: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    for input in inputs {
        let source = std::fs::read_to_string(input)
            .map_err(|err| std::io::Error::new(err.kind(), format!("Cannot read {}: {err}", input.display())))?;
        sources.push((input.clone(), source));
    }
    Ok(discover_from_sources(&sources))
}
