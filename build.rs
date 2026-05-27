// Bundled Typst packages must exist before compile (then embedded via rust-embed).

#[path = "src/typst/package.rs"]
mod typst_package;

use std::env;
use std::fs;
use std::path::Path;
use std::process;

const DIAGRAM_TAILWIND_BROWSER_URL: &str =
    "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.3.0/dist/index.global.min.js";
const DIAGRAM_TAILWIND_BROWSER_OUT: &str = "diagram-html-tailwind-browser.js";

fn main() {
    let manifest_dir_path = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifest_dir = Path::new(&manifest_dir_path);
    println!("cargo:rerun-if-changed=assets/typst-packages/manifest.toml");
    prepare_diagram_tailwind_browser();

    if env::var_os("PAGEMD_SKIP_TYPST_PACKAGES").is_some() {
        eprintln!(
            "cargo:warning=PAGEMD_SKIP_TYPST_PACKAGES is set; bundled Typst packages must already exist under assets/typst-packages/preview/"
        );
    } else if let Err(err) = typst_package::ensure_bundled(manifest_dir, false) {
        eprintln!("error: failed to prepare bundled Typst packages for compile-time embed: {err}");
        eprintln!(
            "hint: check network, or populate assets/typst-packages/preview/ before building"
        );
        process::exit(1);
    }

    if let Ok(specs) = typst_package::load_manifest_from_workspace(manifest_dir) {
        for spec in &specs {
            let dir = typst_package::package_install_dir(manifest_dir, spec);
            println!("cargo:rerun-if-changed={}", dir.display());
            if !typst_package::is_installed(&dir) {
                eprintln!(
                    "error: bundled package {}@{} missing at {} (required for rust-embed)",
                    spec.name,
                    spec.version,
                    dir.display()
                );
                process::exit(1);
            }
        }
    }
}

fn prepare_diagram_tailwind_browser() {
    println!("cargo:rerun-if-env-changed=PAGEMD_TAILWIND_BROWSER_URL");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let out_path = Path::new(&out_dir).join(DIAGRAM_TAILWIND_BROWSER_OUT);
    let url = env::var("PAGEMD_TAILWIND_BROWSER_URL")
        .unwrap_or_else(|_| DIAGRAM_TAILWIND_BROWSER_URL.to_string());

    let bytes = reqwest::blocking::get(&url)
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
        .unwrap_or_else(|err| {
            eprintln!("error: failed to fetch Tailwind browser runtime from {url}: {err}");
            process::exit(1);
        });

    fs::write(&out_path, bytes.as_ref()).unwrap_or_else(|err| {
        eprintln!(
            "error: failed to write Tailwind browser runtime to {}: {err}",
            out_path.display()
        );
        process::exit(1);
    });
}
