use std::{env, fs, path::PathBuf};

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

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let font_dir = out_dir.join("katex-fonts");
    fs::create_dir_all(&font_dir).expect("failed to create KaTeX font dir");

    for filename in KATEX_FONT_FILES {
        let bytes = ratex_katex_fonts::ttf_bytes(filename)
            .unwrap_or_else(|| panic!("missing bundled KaTeX font {filename}"));
        fs::write(font_dir.join(filename), bytes.as_ref())
            .unwrap_or_else(|err| panic!("failed to write bundled KaTeX font {filename}: {err}"));
    }

    println!("cargo:rustc-env=PAGEMD_KATEX_FONT_DIR={}", font_dir.display());
    println!("cargo:rerun-if-changed=build.rs");
}
