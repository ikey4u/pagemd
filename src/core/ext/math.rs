use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse as parse_latex;
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::math_style::MathStyle;

const KATEX_FONT_FILES: &[&str] = &[
    "KaTeX_Main-Regular.ttf",
    "KaTeX_Main-Bold.ttf",
    "KaTeX_Main-Italic.ttf",
    "KaTeX_Main-BoldItalic.ttf",
    "KaTeX_Math-Italic.ttf",
    "KaTeX_Math-BoldItalic.ttf",
    "KaTeX_AMS-Regular.ttf",
    "KaTeX_Caligraphic-Regular.ttf",
    "KaTeX_Caligraphic-Bold.ttf",
    "KaTeX_Fraktur-Regular.ttf",
    "KaTeX_Fraktur-Bold.ttf",
    "KaTeX_SansSerif-Regular.ttf",
    "KaTeX_SansSerif-Bold.ttf",
    "KaTeX_SansSerif-Italic.ttf",
    "KaTeX_Script-Regular.ttf",
    "KaTeX_Typewriter-Regular.ttf",
    "KaTeX_Size1-Regular.ttf",
    "KaTeX_Size2-Regular.ttf",
    "KaTeX_Size3-Regular.ttf",
    "KaTeX_Size4-Regular.ttf",
];

fn katex_font_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PAGEMD_CACHE_DIR") {
        return PathBuf::from(dir).join("katex-fonts");
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("pagemd/katex-fonts");
    }
    if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Caches/pagemd/katex-fonts");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache/pagemd/katex-fonts");
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("pagemd/katex-fonts");
    }
    std::env::temp_dir().join("pagemd-katex-fonts")
}

fn ensure_katex_font_cache() -> Result<PathBuf> {
    let dir = katex_font_cache_dir();
    if dir.join("KaTeX_Main-Regular.ttf").exists() {
        return Ok(dir);
    }
    fs::create_dir_all(&dir).context("failed to create KaTeX font cache directory")?;
    for filename in KATEX_FONT_FILES {
        let bytes = ratex_katex_fonts::ttf_bytes(filename)
            .with_context(|| format!("missing bundled KaTeX font {filename}"))?;
        fs::write(dir.join(filename), bytes.as_ref())
            .with_context(|| format!("failed to write KaTeX font {filename}"))?;
    }
    Ok(dir)
}

pub(crate) fn find_katex_fonts(hint: Option<&Path>) -> Result<String> {
    if let Some(p) = hint {
        if p.join("KaTeX_Main-Regular.ttf").exists() {
            return Ok(p.to_string_lossy().into_owned());
        }
        bail!("KaTeX fonts not found in {}", p.display());
    }

    let dir = ensure_katex_font_cache()?;
    Ok(dir.to_string_lossy().into_owned())
}

pub(crate) fn latex_to_svg(
    expr: &str,
    display: bool,
    font_size: f64,
    font_dir: &str,
) -> Result<String> {
    let ast = parse_latex(expr).map_err(|e| anyhow::anyhow!("LaTeX parse error: {}", e))?;
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let opts = LayoutOptions {
        style,
        ..LayoutOptions::default()
    };
    let lbox = layout(&ast, &opts);
    let dl = to_display_list(&lbox);
    let embed = !font_dir.is_empty();
    let effective_font_size = if display {
        font_size * 2.5
    } else {
        font_size * 1.15
    };
    let svg_opts = SvgOptions {
        font_size: effective_font_size,
        padding: if display { 2.0 } else { 0.5 },
        stroke_width: 1.5,
        embed_glyphs: embed,
        font_dir: font_dir.to_owned(),
    };
    Ok(render_to_svg(&dl, &svg_opts))
}
