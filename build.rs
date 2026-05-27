// Bundled Typst packages must exist before compile (then embedded via rust-embed).

#[path = "src/typst/package.rs"]
mod typst_package;

use std::env;
use std::path::Path;
use std::process;

fn main() {
    let manifest_dir_path = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifest_dir = Path::new(&manifest_dir_path);
    println!("cargo:rerun-if-changed=assets/typst-packages/manifest.toml");

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
