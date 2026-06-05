//! Typst diagram embedding for PageMD.

mod embed;
mod package;

use anyhow::Result;
use typst::layout::{Abs, PagedDocument};
use typst_as_lib::{
    cached_file_resolver::IntoCachedFileResolver,
    package_resolver::{FileSystemCache, PackageResolver},
    typst_kit_options::TypstKitFontOptions,
    TypstEngine,
};
use typst_svg::{svg, svg_merged};

#[cfg(test)]
pub use embed::bundled_specs;
pub use embed::{bundled_package_resolver, PAGEMD_LONG_ABOUT};

fn typst_font_options() -> TypstKitFontOptions {
    TypstKitFontOptions::default()
        .include_system_fonts(true)
        .include_embedded_fonts(true)
}

fn normalize_typst_source(code: &str) -> String {
    let trimmed = code.trim();
    if trimmed.contains("#set page") {
        trimmed.to_string()
    } else {
        format!("#set page(width: auto, height: auto, margin: 8pt)\n{trimmed}\n")
    }
}

fn typst_engine(source: String) -> TypstEngine<typst_as_lib::TypstTemplateMainFile> {
    let runtime_resolver = PackageResolver::builder()
        .cache(FileSystemCache(embed::runtime_package_cache_dir()))
        .build()
        .into_cached();

    TypstEngine::builder()
        .main_file(source)
        .search_fonts_with(typst_font_options())
        .add_file_resolver(bundled_package_resolver())
        .add_file_resolver(runtime_resolver)
        .build()
}

pub fn render_typst(code: &str) -> Result<String> {
    let source = normalize_typst_source(code);
    let engine = typst_engine(source);
    let warned = engine.compile::<PagedDocument>();
    let doc = match warned.output {
        Ok(doc) => doc,
        Err(err) => {
            for warning in &warned.warnings {
                eprintln!("[pagemd] typst warning: {}", warning.message);
                for hint in &warning.hints {
                    eprintln!("  hint: {hint}");
                }
            }
            return Err(anyhow::anyhow!("{err}"));
        }
    };
    let svg_out = if doc.pages.len() == 1 {
        svg(&doc.pages[0])
    } else {
        svg_merged(&doc, Abs::pt(12.0))
    };
    Ok(format!(
        "<div class=\"typst-display\"><div class=\"typst-canvas\">{svg_out}</div></div>\n"
    ))
}

pub fn typst_error_html(code: &str) -> String {
    format!(
        "<div class=\"typst-display typst-error\"><strong>Typst render failed</strong><pre><code>{}</code></pre></div>\n",
        crate::core::util::html_escape(code)
    )
}
