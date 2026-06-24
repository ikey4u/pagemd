use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

pub fn normalize_filename(name: &str) -> String {
    let mut name = name.trim().trim_matches(['"', '\'']);
    if name.is_empty() {
        return "untitled.pagemd.js".to_string();
    }
    if name.ends_with(".pagemd.js") {
        return name.to_string();
    }
    if name.ends_with(".js") {
        name = name.strip_suffix(".js").unwrap_or(name);
    }
    format!("{name}.pagemd.js")
}

pub fn validate_pagemd_script(source: &str) -> Result<()> {
    let source = source.trim();
    if source.is_empty() {
        bail!("script content is empty");
    }
    if source.contains("import ") || source.contains("export ") {
        bail!("script must be plain JS (no ESM import/export)");
    }
    if !source.contains("urlPattern") {
        bail!("script must define a top-level urlPattern");
    }
    if !(source.contains("function extract") || source.contains("extract =")) {
        bail!("script must define extract() as a function declaration");
    }
    if source.contains("function clean") && !source.contains("removed") {
        bail!("clean() must return {{ removed: number }} — include a removed counter");
    }
    if !(source.contains("title") && source.contains("html")) {
        bail!("extract() must return an object with title and html fields");
    }
    Ok(())
}

pub fn save_script(export_dir: &Path, filename: &str, content: &str) -> Result<PathBuf> {
    validate_pagemd_script(content)?;
    std::fs::create_dir_all(export_dir)
        .with_context(|| format!("create {}", export_dir.display()))?;
    let filename = normalize_filename(filename);
    if filename.contains('/') || filename.contains('\\') {
        bail!("filename must not contain path separators");
    }
    let path = export_dir.join(&filename);
    std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

pub fn save_script_tool(export_dir: &Path, args: &serde_json::Value) -> Result<String> {
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("browser_save_script requires content")?;
    let filename = args
        .get("filename")
        .and_then(|v| v.as_str())
        .unwrap_or("untitled.pagemd.js");
    let path = save_script(export_dir, filename, content)?;
    Ok(format!(
        "Saved script -> {}\n({} bytes)",
        path.display(),
        content.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pagemd-{name}-{id}"))
    }

    #[test]
    fn normalize_filename_adds_suffix() {
        assert_eq!(normalize_filename("github"), "github.pagemd.js");
        assert_eq!(normalize_filename("x.pagemd.js"), "x.pagemd.js");
    }

    #[test]
    fn validate_requires_extract_and_url_pattern() {
        let ok = r#"
const urlPattern = "https://example.com/*";
function clean() { let removed = 0; document.querySelectorAll("nav").forEach(n => { n.remove(); removed++; }); return { removed }; }
function extract() { return { title: document.title, html: document.body.innerHTML }; }
"#;
        validate_pagemd_script(ok).unwrap();
        assert!(
            validate_pagemd_script("function extract() { return { title: 'a', html: 'b' }; }")
                .is_err()
        );
    }

    #[test]
    fn save_script_writes_under_export_dir() {
        let dir = temp_dir("export-script");
        std::fs::create_dir_all(&dir).unwrap();
        let source = r#"
const urlPattern = "https://example.com/*";
function extract() { return { title: "t", html: "<p>x</p>" }; }
"#;
        let path = save_script(&dir, "site", source).unwrap();
        assert_eq!(path, dir.join("site.pagemd.js"));
        assert!(path.is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
