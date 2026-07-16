use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::core::export::html::resolve_icon_label;
use crate::core::export::html::section_label;
use crate::core::export::{self, HtmlExportOptions, OutputFormat};
use crate::core::ext::math::find_katex_fonts;
use crate::core::md::render_markdown;
use crate::core::model::{Document, Section};
use crate::core::util::exclude::ExcludeMatcher;
use crate::core::ConvertOptions;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedInputs {
    pub files: Vec<PathBuf>,
    pub directories: Vec<PathBuf>,
}

pub(crate) struct RenderResources {
    pub ss: SyntaxSet,
    pub ts: ThemeSet,
    pub font_dir: String,
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn canonical_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn push_unique_file(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(canonical_key(&path)) {
        files.push(path);
    }
}

fn collect_markdown_files(
    dir: &Path,
    scan_root: &Path,
    exclude: &ExcludeMatcher,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory {}", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("Cannot list directory {}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Cannot inspect {}", path.display()))?;
        if file_type.is_dir() {
            if exclude.should_skip_dir(&path, scan_root) {
                continue;
            }
            collect_markdown_files(&path, scan_root, exclude, files)?;
        } else if file_type.is_file() && is_markdown_file(&path) {
            if !exclude.should_skip_file(&path, scan_root) {
                files.push(path);
            }
        }
    }

    Ok(())
}

fn nearest_scan_root(path: &Path, directories: &[PathBuf]) -> PathBuf {
    let canonical = canonical_key(path);
    directories
        .iter()
        .map(|dir| canonical_key(dir))
        .filter(|dir| canonical.starts_with(dir))
        .max_by_key(|dir| dir.components().count())
        .unwrap_or_else(|| {
            path.parent()
                .map(canonical_key)
                .unwrap_or_else(|| canonical.clone())
        })
}

pub(crate) fn resolve_inputs(opts: &ConvertOptions) -> Result<ResolvedInputs> {
    if opts.inputs.is_empty() && opts.directories.is_empty() {
        bail!("Missing required input. Pass --input <FILE> or --dir <DIR>.");
    }

    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut seen_files = HashSet::new();
    let mut seen_dirs = HashSet::new();

    let exclude = ExcludeMatcher::new(&opts.excludes);

    for dir in &opts.directories {
        if !dir.exists() {
            bail!("Input directory does not exist: {}", dir.display());
        }
        if !dir.is_dir() {
            bail!("Input is not a directory: {}", dir.display());
        }

        let canonical = canonical_key(dir);
        if seen_dirs.insert(canonical.clone()) {
            directories.push(canonical);
        }
    }

    for input in &opts.inputs {
        if !input.exists() {
            bail!("Input file does not exist: {}", input.display());
        }
        if !input.is_file() {
            bail!("Input is not a file: {}", input.display());
        }
        if !exclude.is_empty() {
            let scan_root = nearest_scan_root(input, &directories);
            if exclude.should_skip_file(input, &scan_root) {
                continue;
            }
        }
        push_unique_file(&mut files, &mut seen_files, input.clone());
    }

    for dir in &opts.directories {
        let mut dir_files = Vec::new();
        collect_markdown_files(dir, dir, &exclude, &mut dir_files)?;
        for path in dir_files {
            push_unique_file(&mut files, &mut seen_files, path);
        }
    }

    if files.is_empty() {
        bail!("No Markdown files found. Pass --input <FILE> or --dir <DIR> containing .md/.markdown files.");
    }

    Ok(ResolvedInputs { files, directories })
}

pub(crate) fn prepare_resources(opts: &ConvertOptions) -> Result<RenderResources> {
    let font_dir = find_katex_fonts(opts.katex_fonts.as_deref())?;
    Ok(RenderResources {
        ss: SyntaxSet::load_defaults_newlines(),
        ts: ThemeSet::load_defaults(),
        font_dir,
    })
}

fn build_document(
    opts: &ConvertOptions,
    title_hint: Option<&Path>,
    resources: &RenderResources,
    input_files: &[PathBuf],
) -> Result<Document> {
    let mut sections: Vec<Section> = Vec::new();
    let mut nav_labels: Vec<String> = Vec::new();
    let mut doc_title = opts.title.clone().unwrap_or_default();

    for input_path in input_files {
        let base_dir = input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let source = fs::read_to_string(input_path)
            .with_context(|| format!("Cannot read {}", input_path.display()))?;

        let section = render_markdown(
            &source,
            &base_dir,
            opts.math_font_size,
            &resources.font_dir,
            &resources.ss,
            &resources.ts,
            opts.client_mermaid,
        )
        .with_context(|| format!("Failed to render {}", input_path.display()))?;

        if doc_title.is_empty() && !section.title.is_empty() {
            doc_title = section.title.clone();
        }

        nav_labels.push(section_label(input_path));
        sections.push(section);
    }

    if doc_title.is_empty() {
        doc_title = title_hint
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .or_else(|| {
                input_files
                    .first()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
            })
            .unwrap_or("Document")
            .to_string();
    }

    let icon_label = resolve_icon_label(opts, input_files);

    Ok(Document {
        title: doc_title,
        icon_label,
        sections,
        nav_labels,
        input_paths: input_files.to_vec(),
    })
}

pub(crate) fn export_to_file(
    opts: &ConvertOptions,
    html_opts: &HtmlExportOptions,
    output: &Path,
) -> Result<export::ExportOutput> {
    let resolved = resolve_inputs(opts)?;
    let resources = prepare_resources(opts)?;
    let result = export_with_resources(opts, html_opts, &resources, &resolved.files, Some(output))?;
    fs::write(output, result.html.as_bytes())
        .with_context(|| format!("Cannot write {}", output.display()))?;
    Ok(result)
}

pub(crate) fn export_with_resources(
    opts: &ConvertOptions,
    html_opts: &HtmlExportOptions,
    resources: &RenderResources,
    input_files: &[PathBuf],
    title_hint: Option<&Path>,
) -> Result<export::ExportOutput> {
    let doc = build_document(opts, title_hint, resources, input_files)?;
    export::export_document(&doc, OutputFormat::Html, html_opts)
}
