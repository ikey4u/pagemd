// Version string for `pagemd --version`.

use std::{env, fs, path::Path, process::Command};

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

/// `0.9.1`, `0.9.1-<shortsha>`, or `0.9.1-<shortsha>+dirty`.
fn emit_version() {
    // The crate lives in client/cli; ask git where the repository's .git is.
    if let Some(git_dir) = git_output(&["rev-parse", "--absolute-git-dir"]) {
        let git_dir = Path::new(&git_dir);
        println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
        if let Ok(head) = fs::read_to_string(git_dir.join("HEAD")) {
            if let Some(git_ref) = head.trim().strip_prefix("ref: ") {
                println!(
                    "cargo:rerun-if-changed={}",
                    git_dir.join(git_ref).display()
                );
            }
        }
    }

    let pkg_version =
        env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut version = pkg_version.clone();
    if let Some(commit) = git_output(&["log", "-1", "--format=%h"]) {
        version = format!("{pkg_version}-{commit}");
        if git_dirty() {
            version.push_str("+dirty");
        }
    }
    println!("cargo:rustc-env=PAGEMD_VERSION={version}");
}

fn main() {
    emit_version();
}
