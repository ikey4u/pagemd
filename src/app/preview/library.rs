//! Incremental section cache for `pagemd view` (parallel render + lazy shell).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

use crate::core::export::html::{outline_list_inner, resolve_icon_label, section_label};
use crate::core::export::{export_document, HtmlExportOptions, OutputFormat};
use crate::core::model::{Document, Section};
use crate::core::pipeline::{render_file_section, RenderResources};
use crate::core::{resolve_inputs, ConvertOptions};

#[derive(Clone)]
struct CachedSection {
    mtime: Option<SystemTime>,
    section: Section,
    label: String,
}

/// Shared view library: file list + per-file render cache.
pub struct PreviewLibrary {
    convert_opts: ConvertOptions,
    html_opts: HtmlExportOptions,
    resources: RenderResources,
    files: Vec<PathBuf>,
    cache: Vec<Option<CachedSection>>,
    title_hint: Option<PathBuf>,
}

#[derive(Serialize)]
pub struct SectionPayload {
    pub index: usize,
    pub id: String,
    pub title: String,
    pub html: String,
    pub outline_html: String,
}

impl PreviewLibrary {
    pub fn new(
        convert_opts: ConvertOptions,
        mut html_opts: HtmlExportOptions,
        resources: RenderResources,
        title_hint: Option<PathBuf>,
    ) -> Self {
        // Multi-file view uses lazy placeholders; single-file keeps eager body.
        html_opts.lazy_sections = true;
        Self {
            convert_opts,
            html_opts,
            resources,
            files: Vec::new(),
            cache: Vec::new(),
            title_hint,
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn set_client_mermaid(&mut self, enabled: bool) {
        self.convert_opts.client_mermaid = enabled;
    }

    /// Refresh the resolved Markdown file list; drop cache entries for removed/reordered paths.
    pub fn sync_files(&mut self) -> Result<()> {
        let resolved = resolve_inputs(&self.convert_opts)?;
        let new_files = resolved.files;
        if new_files == self.files {
            return Ok(());
        }

        let old = std::mem::take(&mut self.cache);
        let old_files = std::mem::take(&mut self.files);
        let mut new_cache = Vec::with_capacity(new_files.len());
        for path in &new_files {
            let reused = old_files
                .iter()
                .position(|old_path| same_path(old_path, path))
                .and_then(|idx| old.get(idx).cloned().flatten())
                .and_then(|entry| {
                    let mtime = file_mtime(path);
                    if entry.mtime == mtime {
                        Some(entry)
                    } else {
                        None
                    }
                });
            new_cache.push(reused);
        }
        self.files = new_files;
        self.cache = new_cache;
        self.html_opts.lazy_sections = self.files.len() > 1;
        Ok(())
    }

    /// Invalidate cache entries whose paths match (or are under) `changed`.
    pub fn invalidate(&mut self, changed: &[PathBuf]) {
        if changed.is_empty() {
            for slot in &mut self.cache {
                *slot = None;
            }
            return;
        }
        for (idx, path) in self.files.iter().enumerate() {
            if changed.iter().any(|c| path_matches(path, c)) {
                self.cache[idx] = None;
            }
        }
    }

    /// Ensure the given 0-based indices are rendered (in parallel for misses).
    pub fn ensure_indices(&mut self, indices: &[usize]) -> Result<()> {
        self.sync_files()?;
        let mut pending: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| i < self.files.len())
            .filter(|&i| {
                let stale = match &self.cache[i] {
                    None => true,
                    Some(entry) => entry.mtime != file_mtime(&self.files[i]),
                };
                stale
            })
            .collect();
        pending.sort_unstable();
        pending.dedup();
        if pending.is_empty() {
            return Ok(());
        }

        let opts = &self.convert_opts;
        let resources = &self.resources;
        let footnotes = self.html_opts.footnotes;
        let rendered: Vec<(usize, CachedSection)> = pending
            .par_iter()
            .map(|&index| {
                let path = &self.files[index];
                let section = render_file_section(opts, resources, path, footnotes)?;
                Ok((
                    index,
                    CachedSection {
                        mtime: file_mtime(path),
                        label: section_label(path),
                        section,
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        for (index, entry) in rendered {
            self.cache[index] = Some(entry);
        }
        Ok(())
    }

    pub fn ensure_all(&mut self) -> Result<()> {
        self.sync_files()?;
        let indices: Vec<usize> = (0..self.files.len()).collect();
        self.ensure_indices(&indices)
    }

    /// Build shell HTML. In lazy multi-file mode only `embed` indices include bodies.
    pub fn shell_html(&mut self, embed: &[usize]) -> Result<String> {
        self.sync_files()?;
        if self.files.is_empty() {
            anyhow::bail!("No Markdown files found.");
        }

        let embed: Vec<usize> = if self.files.len() == 1 {
            vec![0]
        } else if embed.is_empty() {
            vec![0]
        } else {
            embed.to_vec()
        };
        self.ensure_indices(&embed)?;

        let doc = self.document_for_shell(&embed);
        Ok(export_document(&doc, OutputFormat::Html, &self.html_opts)?.html)
    }

    /// Full SingleFile-style HTML (all sections embedded).
    pub fn full_html(&mut self) -> Result<String> {
        self.sync_files()?;
        self.ensure_all()?;
        let mut opts = self.html_opts.clone();
        opts.lazy_sections = false;
        let doc = self.document_for_shell(&(0..self.files.len()).collect::<Vec<_>>());
        Ok(export_document(&doc, OutputFormat::Html, &opts)?.html)
    }

    pub fn section_payload(&mut self, one_based: usize) -> Result<SectionPayload> {
        if one_based == 0 || one_based > self.files.len() {
            anyhow::bail!("section {one_based} out of range");
        }
        let index = one_based - 1;
        self.ensure_indices(&[index])?;
        let entry = self.cache[index]
            .as_ref()
            .context("section cache miss after ensure")?;
        let title = panel_title(&entry.section, index, &entry.label);
        Ok(SectionPayload {
            index: one_based,
            id: format!("doc-{one_based}"),
            title,
            html: entry.section.html.clone(),
            outline_html: outline_list_inner(&entry.section),
        })
    }

    fn document_for_shell(&self, embed: &[usize]) -> Document {
        let mut sections = Vec::with_capacity(self.files.len());
        let mut nav_labels = Vec::with_capacity(self.files.len());
        let mut doc_title = self.convert_opts.title.clone().unwrap_or_default();

        for (index, path) in self.files.iter().enumerate() {
            let label = self
                .cache
                .get(index)
                .and_then(|c| c.as_ref())
                .map(|c| c.label.clone())
                .unwrap_or_else(|| section_label(path));
            nav_labels.push(label.clone());

            let section = if embed.contains(&index) {
                if let Some(cached) = self.cache.get(index).and_then(|c| c.as_ref()) {
                    if doc_title.is_empty() && !cached.section.title.is_empty() {
                        doc_title = cached.section.title.clone();
                    }
                    cached.section.clone()
                } else {
                    stub_section(&label)
                }
            } else {
                // Lazy placeholder: empty html triggers data-lazy-section in page builder.
                stub_section(&label)
            };
            sections.push(section);
        }

        if doc_title.is_empty() {
            doc_title = self
                .title_hint
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .or_else(|| {
                    self.files
                        .first()
                        .and_then(|p| p.file_stem())
                        .and_then(|s| s.to_str())
                })
                .unwrap_or("Document")
                .to_string();
        }

        let icon_label = resolve_icon_label(&self.convert_opts, &self.files);
        Document {
            title: doc_title,
            icon_label,
            sections,
            nav_labels,
            input_paths: self.files.clone(),
        }
    }
}

fn stub_section(label: &str) -> Section {
    Section {
        title: label.to_string(),
        html: String::new(),
        outline: Vec::new(),
        footnotes: Vec::new(),
    }
}

fn panel_title(section: &Section, index: usize, label: &str) -> String {
    if !label.trim().is_empty() {
        return label.to_string();
    }
    let title = section.title.trim();
    if title.is_empty() {
        format!("Document {}", index + 1)
    } else {
        section.title.clone()
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

fn path_matches(file: &Path, changed: &Path) -> bool {
    if same_path(file, changed) {
        return true;
    }
    if changed.is_dir() {
        let file_c = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        let dir_c = changed
            .canonicalize()
            .unwrap_or_else(|_| changed.to_path_buf());
        return file_c.starts_with(&dir_c);
    }
    false
}

/// Thread-safe wrapper used by the preview server.
pub type SharedPreviewLibrary = std::sync::Arc<Mutex<PreviewLibrary>>;

pub fn lock_library(
    library: &SharedPreviewLibrary,
) -> Result<std::sync::MutexGuard<'_, PreviewLibrary>> {
    library
        .lock()
        .map_err(|_| anyhow::anyhow!("preview library lock poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ConvertOptions, HtmlExportOptions, OutputFormat};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pagemd-lib-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lazy_shell_embeds_only_requested_sections() {
        let dir = temp_dir("lazy");
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        fs::write(&a, "# Alpha\n\nbody-a\n").unwrap();
        fs::write(&b, "# Beta\n\nbody-b\n").unwrap();

        let convert_opts = ConvertOptions {
            inputs: vec![dir.clone()],
            directories: Vec::new(),
            excludes: Vec::new(),
            title: Some("Lib".into()),
            icon: None,
            math_font_size: 16.0,
            katex_fonts: None,
            output_format: OutputFormat::Html,
            client_mermaid: true,
        };
        let resources = crate::core::prepare_resources(&convert_opts).unwrap();
        let mut lib = PreviewLibrary::new(
            convert_opts,
            HtmlExportOptions {
                client_mermaid_runtime: true,
                embed_workspace_script: false,
                ..Default::default()
            },
            resources,
            None,
        );

        let shell = lib.shell_html(&[0]).unwrap();
        assert!(shell.contains("body-a"), "{shell}");
        assert!(!shell.contains("body-b"), "second section must stay lazy");
        assert!(shell.contains("data-lazy-section=\"2\""), "{shell}");

        let payload = lib.section_payload(2).unwrap();
        assert!(payload.html.contains("body-b"));
        assert!(payload.outline_html.contains("Beta") || payload.title.contains("b.md"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn incremental_skips_unchanged_files() {
        let dir = temp_dir("incr");
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        fs::write(&a, "# A\n").unwrap();
        fs::write(&b, "# B\n").unwrap();

        let convert_opts = ConvertOptions {
            inputs: vec![dir.clone()],
            ..ConvertOptions::default()
        };
        let resources = crate::core::prepare_resources(&convert_opts).unwrap();
        let mut lib =
            PreviewLibrary::new(convert_opts, HtmlExportOptions::default(), resources, None);
        lib.ensure_all().unwrap();
        assert!(lib.cache.iter().all(|c| c.is_some()));

        // Touch only B and invalidate it.
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&b, "# B2\n").unwrap();
        lib.invalidate(&[b.clone()]);
        assert!(lib.cache[0].is_some());
        assert!(lib.cache[1].is_none());
        lib.ensure_indices(&[1]).unwrap();
        assert!(lib.cache[1].as_ref().unwrap().section.html.contains("B2"));

        fs::remove_dir_all(dir).unwrap();
    }
}
