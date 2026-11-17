use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct BrowserMcpArgs {
    #[arg(long, help = "PageMD browser workspace directory")]
    pub workspace: PathBuf,
}

pub fn run(args: BrowserMcpArgs) -> Result<()> {
    super::mcp::serve_stdio(&args.workspace)
}
