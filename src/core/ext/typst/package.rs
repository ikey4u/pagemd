//! Manifest fetch for `assets/typst-packages/` (build.rs) and manifest parsing for embed.
//!
//! Fetch helpers are only called from `build.rs` (via `#[path]`); the main crate uses `parse_manifest` only.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use tar::Archive;

const REGISTRY_BASE: &str = "https://packages.typst.org/preview";

#[derive(Debug, Clone, serde::Deserialize)]
struct Manifest {
    package: Vec<PackageEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PackageEntry {
    name: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageSpec {
    pub name: String,
    pub version: String,
}

impl PackageSpec {
    fn archive_url(&self) -> String {
        format!("{REGISTRY_BASE}/{}-{}.tar.gz", self.name, self.version)
    }
}

pub fn packages_root(workspace: &Path) -> PathBuf {
    workspace.join("assets/typst-packages")
}

pub fn preview_root(workspace: &Path) -> PathBuf {
    packages_root(workspace).join("preview")
}

pub fn package_install_dir(workspace: &Path, spec: &PackageSpec) -> PathBuf {
    preview_root(workspace).join(&spec.name).join(&spec.version)
}

pub fn is_installed(dir: &Path) -> bool {
    dir.join("typst.toml").is_file()
}

pub fn load_manifest_from_workspace(workspace: &Path) -> Result<Vec<PackageSpec>> {
    let path = packages_root(workspace).join("manifest.toml");
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read Typst package manifest {}", path.display()))?;
    parse_manifest(&text)
}

pub fn parse_manifest(text: &str) -> Result<Vec<PackageSpec>> {
    let manifest: Manifest = toml::from_str(text).context("parse typst-packages manifest.toml")?;
    Ok(manifest
        .package
        .into_iter()
        .map(|p| PackageSpec {
            name: p.name,
            version: p.version,
        })
        .collect())
}

fn fetch_package(spec: &PackageSpec, dest: &Path) -> Result<()> {
    let url = spec.archive_url();
    let client = reqwest::blocking::Client::builder()
        .user_agent("pagemd/0.1")
        .build()
        .context("build HTTP client")?;
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("download {url}"))?;
    if !response.status().is_success() {
        bail!(
            "download failed for {}@{}: HTTP {}",
            spec.name,
            spec.version,
            response.status()
        );
    }
    let bytes = response
        .bytes()
        .with_context(|| format!("read body from {url}"))?;

    if dest.exists() {
        fs::remove_dir_all(dest).with_context(|| format!("remove existing {}", dest.display()))?;
    }
    fs::create_dir_all(dest).with_context(|| format!("create {}", dest.display()))?;

    let decoder = GzDecoder::new(bytes.as_ref());
    let mut archive = Archive::new(decoder);
    archive
        .unpack(dest)
        .with_context(|| format!("extract {} into {}", url, dest.display()))?;
    Ok(())
}

/// Populate `assets/typst-packages/preview/` before compile (used by build.rs).
pub fn ensure_bundled(workspace: &Path, force: bool) -> Result<()> {
    let specs = load_manifest_from_workspace(workspace)?;
    if specs.is_empty() {
        bail!("no packages declared in assets/typst-packages/manifest.toml");
    }

    for spec in &specs {
        let dir = package_install_dir(workspace, spec);
        if !force && is_installed(&dir) {
            println!(
                "typst package {}@{}: already installed at {}",
                spec.name,
                spec.version,
                dir.display()
            );
            continue;
        }
        println!(
            "typst package {}@{}: fetching → {}",
            spec.name,
            spec.version,
            dir.display()
        );
        fetch_package(spec, &dir)?;
    }
    Ok(())
}
