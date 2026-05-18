mod live;
mod resources;
mod server;

use std::path::PathBuf;

pub use resources::{collect_initial_watch_paths, discover_watch_paths};
pub use server::{run, RenderRequest, RenderResult};

pub struct ViewOptions {
    pub host: String,
    pub port: u16,
    pub inputs: Vec<PathBuf>,
    pub watch_paths: Vec<PathBuf>,
    pub open_browser: bool,
    pub export_path: Option<PathBuf>,
}

pub fn validate_inputs(inputs: &[PathBuf]) -> anyhow::Result<()> {
    if inputs.is_empty() {
        anyhow::bail!("Missing required input. Pass --input <FILE>.");
    }
    for input in inputs {
        if !input.exists() {
            anyhow::bail!("Input file does not exist: {}", input.display());
        }
        if !input.is_file() {
            anyhow::bail!("Input is not a file: {}", input.display());
        }
    }
    Ok(())
}
