use std::{
    fs,
    path::{Path, PathBuf},
};

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
            return PathBuf::from(home)
                .join("Library/Caches/pagemd/katex-fonts");
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
    install_katex_fonts(&katex_font_cache_dir())
}

/// Writes the bundled KaTeX fonts into `dir` (unless already there) and
/// returns it, ready for `RenderOptions::katex_fonts`. Lets embedders such
/// as app sandboxes choose the font cache without setting `PAGEMD_CACHE_DIR`.
pub fn install_katex_fonts(dir: &Path) -> Result<PathBuf> {
    let dir = dir.to_path_buf();
    if dir.join("KaTeX_Main-Regular.ttf").exists() {
        return Ok(dir);
    }
    fs::create_dir_all(&dir)
        .context("failed to create KaTeX font cache directory")?;
    for filename in KATEX_FONT_FILES {
        let bytes =
            ratex_katex_fonts::ttf_bytes(filename).with_context(|| {
                format!("missing bundled KaTeX font {filename}")
            })?;
        fs::write(dir.join(filename), bytes.as_ref()).with_context(|| {
            format!("failed to write KaTeX font {filename}")
        })?;
    }
    Ok(dir)
}

pub fn find_katex_fonts(hint: Option<&Path>) -> Result<String> {
    if let Some(p) = hint {
        if p.join("KaTeX_Main-Regular.ttf").exists() {
            return Ok(p.to_string_lossy().into_owned());
        }
        bail!("KaTeX fonts not found in {}", p.display());
    }

    let dir = ensure_katex_font_cache()?;
    Ok(dir.to_string_lossy().into_owned())
}

pub fn latex_to_svg(
    expr: &str,
    display: bool,
    font_size: f64,
    font_dir: &str,
) -> Result<String> {
    let ast = parse_latex(expr)
        .map_err(|e| anyhow::anyhow!("LaTeX parse error: {}", e))?;
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
        // Display style already enlarges operators; keep near body text scale.
        font_size * 1.15
    } else {
        font_size
    };
    let padding = if display { 1.0 } else { 0.5 };
    let svg_opts = SvgOptions {
        font_size: effective_font_size,
        padding,
        stroke_width: 1.25,
        embed_glyphs: embed,
        font_dir: font_dir.to_owned(),
    };
    let svg = render_to_svg(&dl, &svg_opts);
    if display {
        return Ok(svg);
    }
    // Inline math is sized from its own metrics, in em of the surrounding
    // text, and sits on the text's baseline: a lone `x` stays letter-sized and
    // a fraction may rise above the line, instead of every formula being
    // squeezed to one height.
    let pad_em = padding / effective_font_size;
    let height_em = dl.height + dl.depth + 2.0 * pad_em;
    let drop_em = dl.depth + pad_em;
    Ok(svg.replacen(
        "<svg ",
        &format!(
            "<svg style=\"height:{height_em:.4}em;width:auto;vertical-align:-{drop_em:.4}em\" "
        ),
        1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline_metrics(expr: &str) -> (f64, f64) {
        let svg = latex_to_svg(expr, false, 16.0, "").unwrap();
        let style = svg
            .split("style=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let num = |key: &str| -> f64 {
            style
                .split(key)
                .nth(1)
                .unwrap()
                .trim_start_matches('-')
                .split("em")
                .next()
                .unwrap()
                .parse()
                .unwrap()
        };
        (num("height:"), num("vertical-align:"))
    }

    #[test]
    fn inline_math_is_sized_by_its_own_metrics() {
        let (letter, letter_drop) = inline_metrics("L");
        let (root, root_drop) = inline_metrics(r"T = 2\pi\sqrt{L/g}");
        // A lone letter is about a letter tall; a root over a fraction is taller.
        assert!(letter > 0.6 && letter < 1.0, "{letter}");
        assert!(root > letter * 1.4, "{root} vs {letter}");
        // Both drop below the baseline by their depth (and padding) only.
        assert!(letter_drop < 0.1, "{letter_drop}");
        assert!(root_drop > letter_drop, "{root_drop} vs {letter_drop}");
        // Display math keeps its pixel size.
        assert!(!latex_to_svg("x", true, 16.0, "").unwrap().contains("em;"));
    }
}
