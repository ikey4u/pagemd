//! Compile-time embedded `@preview` packages and runtime package resolution.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use rust_embed::RustEmbed;
use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{package::PackageSpec as TypstPackageSpec, FileId, Source};
use typst_as_lib::file_resolver::FileResolver;

use super::package::{self, PackageSpec};

const MANIFEST: &str = include_str!("../../../../assets/typst-packages/manifest.toml");

/// Top-level `pagemd --help` text.
pub const PAGEMD_LONG_ABOUT: &str = concat!(
    "\
Convert Markdown to a SingleFile-style HTML document (default mode).

Usage:
  pagemd -i INPUT.md -o OUTPUT.html
  pagemd -i a.md -i b.md -o doc.html
  pagemd -i docs/ -o docs.html
  pagemd -d docs/ -o doc.html
  pagemd -d src/ --exclude drafts/** node_modules

Use `pagemd view --help` for live preview with hot reload.
Use `pagemd browser --help` for the Chrome REPL workflow.

Supported features
─────────────────────────────────

Core Markdown:
  Headings, paragraphs, emphasis, strikethrough, links, images, horizontal rules,
  ordered/unordered/task lists, tables, blockquotes, footnotes ([^id] with [^id]: body),
  and fenced code blocks with syntax highlighting (syntect).

Math:
  Inline: $E=mc^2$
  Display: a paragraph that is only $$...$$ on one line, or a fenced block:

  ```math
  \\int_0^1 x^2\\,dx = \\frac{1}{3}
  ```
  Alias fence language: latex

Diagrams — prefer these for AI-generated visuals:

  ```mermaid
  flowchart LR
    A[Input] --> B[PageMD] --> C[HTML]
  ```
  Fence languages: mermaid, mmd

  ```plantuml
  @startuml
  Alice -> Bob: hello
  @enduml
  ```
  Fence languages: plantuml, puml, uml

  ```diagram html
  <div class=\"rounded-3xl border border-slate-200 bg-white p-6\">
    <svg viewBox=\"0 0 640 240\" class=\"w-full\" role=\"img\" aria-label=\"Architecture\">
      <!-- nodes, connectors, labels -->
    </svg>
  </div>
  ```
  Raw HTML/SVG inside a diagram container. Tailwind utility classes work via an
  embedded @tailwindcss/browser runtime (included only when a document uses this fence).
  Best choice for architecture, UI mockups, and precise layouts — give explicit SVG/HTML.

Callouts / admonitions:
  GitHub-style blockquote marker:
    > [!NOTE] Optional title
    > Body supports **Markdown** and inline math.

  Fenced admonition:
    :::tip Optional title
    Body text
    :::

  Indented admonition:
    !!! warning \"Title\"
        Body text

  Kinds (and common aliases): note, info, tip, warning, danger, important, caution,
  question, success, failure, bug, example, quote, abstract; also hint, faq, error, …

Multi-file input:
  --input FILE (repeatable) and/or --dir DIR (recursive scan for .md/.markdown).
  Multiple files merge into one HTML document with a file-tree sidebar and per-file outline.
  --exclude PATTERN skips paths while scanning (name, glob, or drafts/** style).

Resources:
  Local images and assets referenced from Markdown or raw HTML are inlined when possible.
  Remote http(s) URLs in img/src/href/poster and CSS url(...) are fetched and embedded
  as data URIs when conversion succeeds.

Browser output:
  Self-contained HTML with embedded CSS. Footnote references show a hover hint with the
  note body. Use --title, --icon (2-char tab label), and --font-size for math scaling.

Example document
────────────────

",
    include_str!("../../../../examples/BASIC.md"),
);

#[derive(RustEmbed)]
#[folder = "assets/typst-packages/preview/"]
struct BundledTypstPreview;

fn bundled_index() -> &'static HashSet<(String, String)> {
    static INDEX: OnceLock<HashSet<(String, String)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        package::parse_manifest(MANIFEST)
            .expect("invalid assets/typst-packages/manifest.toml")
            .into_iter()
            .map(|s| (s.name, s.version))
            .collect()
    })
}

#[allow(dead_code)]
pub fn bundled_specs() -> &'static [PackageSpec] {
    static SPECS: OnceLock<Vec<PackageSpec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        package::parse_manifest(MANIFEST).expect("invalid assets/typst-packages/manifest.toml")
    })
}

fn is_bundled(package: &TypstPackageSpec) -> bool {
    bundled_index().contains(&(package.name.to_string(), package.version.to_string()))
}

pub fn runtime_package_cache_dir() -> PathBuf {
    if let Ok(path) = std::env::var("PAGEMD_TYPST_CACHE") {
        return PathBuf::from(path);
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pagemd")
        .join("typst")
        .join("packages")
}

fn not_found(id: FileId) -> FileError {
    FileError::NotFound(id.vpath().as_rootless_path().to_path_buf())
}

fn bytes_to_source(id: FileId, bytes: &[u8]) -> FileResult<Source> {
    let contents = std::str::from_utf8(bytes).map_err(|_| FileError::InvalidUtf8)?;
    let contents = contents.trim_start_matches('\u{feff}');
    Ok(Source::new(id, contents.to_owned()))
}

pub struct BundledPackageResolver;

impl BundledPackageResolver {
    fn resolve_bytes(id: FileId) -> FileResult<Vec<u8>> {
        let Some(package) = id.package() else {
            return Err(not_found(id));
        };
        if package.namespace.as_str() != "preview" || !is_bundled(package) {
            return Err(not_found(id));
        }

        let subdir = std::path::Path::new(package.name.as_str()).join(package.version.to_string());
        let Some(relative) = id.vpath().resolve(&subdir) else {
            return Err(not_found(id));
        };
        let embed_path = relative.to_string_lossy().replace('\\', "/");
        let file = BundledTypstPreview::get(&embed_path).ok_or_else(|| not_found(id))?;
        Ok(file.data.into_owned())
    }
}

impl FileResolver for BundledPackageResolver {
    fn resolve_binary(&self, id: FileId) -> FileResult<Cow<'_, Bytes>> {
        let bytes = Self::resolve_bytes(id)?;
        Ok(Cow::Owned(Bytes::new(bytes)))
    }

    fn resolve_source(&self, id: FileId) -> FileResult<Cow<'_, Source>> {
        let bytes = Self::resolve_bytes(id)?;
        Ok(Cow::Owned(bytes_to_source(id, &bytes)?))
    }
}

pub fn bundled_package_resolver() -> BundledPackageResolver {
    BundledPackageResolver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses() {
        assert_eq!(bundled_specs().len(), 3);
        assert!(bundled_index().contains(&("cetz".into(), "0.3.2".into())));
    }

    #[test]
    fn bundled_packages_are_embedded() {
        assert!(BundledTypstPreview::get("cetz/0.3.2/typst.toml").is_some());
        assert!(BundledTypstPreview::get("fletcher/0.5.8/typst.toml").is_some());
        assert!(BundledTypstPreview::get("codelst/2.0.2/typst.toml").is_some());
    }
}
