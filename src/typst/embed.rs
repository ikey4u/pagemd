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

const MANIFEST: &str = include_str!("../../assets/typst-packages/manifest.toml");

/// Top-level `pagemd --help` text. Typst package ids must stay in sync with `manifest.toml`.
pub const PAGEMD_LONG_ABOUT: &str = "\
Convert Markdown to a SingleFile-style HTML document (default mode).

Usage:
  pagemd -i INPUT.md -o OUTPUT.html
  pagemd --input a.md --input b.md --output doc.html

Use `pagemd view --help` for live preview with hot reload.

HTML diagram embedding
──────────────────────

Embed styled HTML/SVG diagrams in the output: use a Markdown fenced code block \
with language info \"diagram html\".

Example:

  ```diagram html
  <div class=\"rounded-3xl border border-slate-200 bg-white p-6\">
    <svg viewBox=\"0 0 640 240\" class=\"w-full\" role=\"img\" aria-label=\"Architecture\">
      <!-- Draw nodes, connectors, and labels here. -->
    </svg>
  </div>
  ```

`diagram html` blocks are inserted as raw HTML inside a diagram container. Local and \
remote resources referenced by common HTML attributes or CSS url(...) values are \
inlined when possible, like other raw HTML resources in PageMD.

Tailwind utility classes are supported by an embedded Tailwind browser runtime. \
The runtime is fetched at PageMD build time by build.rs, embedded into the pagemd \
binary with include_bytes!, and emitted into generated HTML only when a document \
contains a `diagram html` block. The released pagemd binary does not call the \
Tailwind CLI, npx, or any other external tool at render time.

Typst embedding
───────────────

Embed Typst diagrams in HTML: use a Markdown fenced code block with language \"typst\"; \
PageMD compiles it to inline SVG in the output.

Built-in @preview packages (embedded in pagemd, work offline):

  @preview/cetz:0.3.2       drawing / charts (CeTZ)
  @preview/fletcher:0.5.8   flowcharts / arrows
  @preview/codelst:2.0.2    code listings

Use those exact versions in #import lines in your Typst source.

Other @preview packages are downloaded on first render. They are stored as:

  <cache-root>/preview/<name>/<version>/...

Default <cache-root> (when PAGEMD_TYPST_CACHE is unset):
  Linux     ~/.cache/pagemd/typst/packages
  macOS     ~/Library/Caches/pagemd/typst/packages
  Windows   %LOCALAPPDATA%\\pagemd\\typst\\packages  (or the platform cache dir from dirs)

Set PAGEMD_TYPST_CACHE to a custom <cache-root>; preview/<name>/<version>/ is still used inside it.";

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

    #[test]
    fn bundled_help_lists_packages() {
        for spec in bundled_specs() {
            let id = format!("@preview/{}:{}", spec.name, spec.version);
            assert!(
                PAGEMD_LONG_ABOUT.contains(&id),
                "PAGEMD_LONG_ABOUT missing {id}"
            );
        }
    }
}
