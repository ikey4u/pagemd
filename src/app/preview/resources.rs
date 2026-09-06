use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

/// Local files pagemd actually inlines. Navigation links to source
/// (`.swift`, `.md`, …) are not part of the preview and must not be watched.
const EMBEDDABLE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "avif", "css", "js", "mjs", "woff",
    "woff2", "ttf", "otf", "mp4", "webm", "ogg",
];

const DISCOVER_SOURCE_LIMIT: usize = 64;

/// Minimum watch set that can change the rendered preview.
///
/// * `recursive` — corpus roots (`--dir` / directory inputs). Nested files are
///   already covered; they are never listed separately.
/// * `paths` — loose Markdown inputs and embeddable resources that live
///   **outside** those roots. Files only; their parent directories are not
///   attached (that is what pulled `ios/` into a `docs/` preview).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WatchPlan {
    pub recursive: Vec<PathBuf>,
    pub paths: Vec<PathBuf>,
}

impl WatchPlan {
    pub fn is_empty(&self) -> bool {
        self.recursive.is_empty() && self.paths.is_empty()
    }
}

fn markdown_image_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!\[[^\]]*\]\(\s*<?([^)>\s]+)").expect("markdown image regex"))
}

fn html_embed_attr_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?:src|poster)\s*=\s*["']([^"']+)["']"#).expect("html embed attr regex")
    })
}

fn html_link_href_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)<link\b[^>]*?\shref\s*=\s*["']([^"']+)["']"#)
            .expect("html link href regex")
    })
}

fn css_url_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"url\(\s*['"]?([^'")]+)['"]?\s*\)"#).expect("css url regex"))
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_embeddable_resource(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            EMBEDDABLE_EXTS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
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

fn covered_by_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .any(|root| path != root && path.starts_with(root))
}

fn resolve_local_path(reference: &str, base_dir: &Path) -> Option<PathBuf> {
    let reference = reference
        .trim()
        .split(['?', '#'])
        .next()
        .unwrap_or(reference);
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
    let path = canonical(&path);
    if path.exists() && is_embeddable_resource(&path) {
        Some(path)
    } else {
        None
    }
}

fn insert_discovered(paths: &mut HashSet<PathBuf>, reference: &str, base_dir: &Path) {
    if let Some(path) = resolve_local_path(reference, base_dir) {
        paths.insert(path);
    }
}

fn discover_embeddable_resources(sources: &[(PathBuf, String)]) -> Vec<PathBuf> {
    let mut paths = HashSet::new();

    for (input, source) in sources {
        let base_dir = input.parent().unwrap_or_else(|| Path::new("."));

        for cap in markdown_image_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                insert_discovered(&mut paths, reference.as_str(), base_dir);
            }
        }
        for cap in html_embed_attr_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                insert_discovered(&mut paths, reference.as_str(), base_dir);
            }
        }
        for cap in html_link_href_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                insert_discovered(&mut paths, reference.as_str(), base_dir);
            }
        }
        for cap in css_url_re().captures_iter(source) {
            if let Some(reference) = cap.get(1) {
                insert_discovered(&mut paths, reference.as_str(), base_dir);
            }
        }
    }

    paths.into_iter().collect()
}

fn load_sources(files: &[PathBuf]) -> Vec<(PathBuf, String)> {
    if files.len() > DISCOVER_SOURCE_LIMIT {
        return Vec::new();
    }
    files
        .iter()
        .filter_map(|input| {
            fs::read_to_string(input)
                .ok()
                .map(|source| (input.clone(), source))
        })
        .collect()
}

/// Collapse inputs and referenced embeddable assets into the watch plan.
///
/// Callers pass the current corpus (`files` + scan `directories`). Markdown is
/// read here when the corpus is small enough; scan roots already cover nested
/// creates, so skipping discovery on huge trees does not miss in-tree edits.
pub fn collect_watch_plan(files: &[PathBuf], directories: &[PathBuf]) -> WatchPlan {
    let mut roots: Vec<PathBuf> = directories
        .iter()
        .map(|dir| canonical(dir))
        .filter(|dir| dir.is_dir())
        .collect();
    roots.sort_by_key(|path| path.components().count());
    let mut recursive = Vec::new();
    for root in roots {
        if !covered_by_root(&root, &recursive) {
            recursive.push(root);
        }
    }

    let mut paths = HashSet::new();
    for file in files {
        let file = canonical(file);
        if file.exists() && !covered_by_root(&file, &recursive) {
            paths.insert(file);
        }
    }

    for asset in discover_embeddable_resources(&load_sources(files)) {
        if !covered_by_root(&asset, &recursive) && !recursive.iter().any(|root| root == &asset) {
            paths.insert(asset);
        }
    }

    let mut paths: Vec<PathBuf> = paths.into_iter().collect();
    paths.sort();
    recursive.sort();
    WatchPlan { recursive, paths }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pagemd-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dir_preview_watches_scan_root_and_external_embeds_only() {
        let root = temp_dir("watch-plan");
        let docs = root.join("docs");
        let ios = root.join("ios");
        std::fs::create_dir_all(docs.join("guide")).unwrap();
        std::fs::create_dir_all(&ios).unwrap();

        let nested = docs.join("guide").join("intro.md");
        std::fs::write(&nested, "# Intro\n\n![local](./pic.png)\n").unwrap();
        std::fs::write(docs.join("guide/pic.png"), b"png").unwrap();

        let swift = ios.join("AppModel.swift");
        std::fs::write(&swift, "struct AppModel {}\n").unwrap();
        let shot = ios.join("screenshot.png");
        std::fs::write(&shot, b"png").unwrap();
        std::fs::write(
            docs.join("impl.md"),
            "# Impl\n\n\
             [AppModel.swift](../ios/AppModel.swift)\n\n\
             <a href=\"../ios/AppModel.swift\">source</a>\n\n\
             ![ui](../ios/screenshot.png)\n",
        )
        .unwrap();

        let plan = collect_watch_plan(&[nested, docs.join("impl.md")], &[docs.clone()]);
        let docs = docs.canonicalize().unwrap();
        let shot = shot.canonicalize().unwrap();
        let swift = swift.canonicalize().unwrap();
        let ios = ios.canonicalize().unwrap();

        assert_eq!(plan.recursive, vec![docs.clone()]);
        assert_eq!(plan.paths, vec![shot]);
        assert!(!plan.recursive.iter().any(|path| path == &ios));
        assert!(!plan.paths.iter().any(|path| path == &swift));
        assert!(!plan.paths.iter().any(|path| path == &ios));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_preview_watches_the_file_and_embeds_not_their_parents() {
        let root = temp_dir("watch-file");
        let other = root.join("other");
        std::fs::create_dir_all(&other).unwrap();
        let file = root.join("doc.md");
        let sibling = root.join("pic.png");
        let shot = other.join("shot.png");
        std::fs::write(&sibling, b"png").unwrap();
        std::fs::write(&shot, b"png").unwrap();
        std::fs::write(
            &file,
            "# Doc\n\n![a](./pic.png)\n\n![b](./other/shot.png)\n",
        )
        .unwrap();

        let plan = collect_watch_plan(&[file.clone()], &[]);
        let file = file.canonicalize().unwrap();
        let sibling = sibling.canonicalize().unwrap();
        let shot = shot.canonicalize().unwrap();
        let other = other.canonicalize().unwrap();
        let root = root.canonicalize().unwrap();

        assert!(plan.recursive.is_empty());
        assert!(plan.paths.contains(&file));
        assert!(plan.paths.contains(&sibling));
        assert!(plan.paths.contains(&shot));
        assert!(!plan.paths.contains(&root), "do not watch the file parent");
        assert!(
            !plan.paths.contains(&other),
            "do not watch the asset parent"
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
