use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use super::provider_detect;
use super::repl;

#[derive(Args, Debug, Clone)]
pub struct BrowserArgs {
    #[arg(long, help = "Navigate to this URL after Chrome starts")]
    pub url: Option<String>,

    #[arg(long, default_value_t = 9222, help = "Chrome remote debugging port")]
    pub port: u16,

    #[arg(long, value_name = "PATH", help = "Chrome executable path")]
    pub chrome_path: Option<PathBuf>,

    #[arg(
        long,
        value_name = "DIR",
        help = "Persistent Chrome user-data-dir (overrides default cache path)"
    )]
    pub user_data_dir: Option<PathBuf>,

    #[arg(
        long,
        help = "Use a fresh ephemeral profile directory for this session"
    )]
    pub clean: bool,

    #[arg(
        long,
        help = "Connect to an existing Chrome with remote-debugging-port (do not spawn)"
    )]
    pub connect: bool,

    #[arg(long, help = "Run Chrome headless")]
    pub headless: bool,

    #[arg(long, default_value = "auto", help = "AI backend: auto | cursor")]
    pub provider: String,

    #[arg(long, help = "Disable Cursor agent; slash commands only")]
    pub no_ai: bool,

    #[arg(long, default_value = ">", help = "REPL input prompt")]
    pub prompt: String,
}

fn startup_status(msg: &str) {
    eprint!("\r{msg}");
    let _ = io::stderr().flush();
}

fn startup_status_done() {
    eprintln!();
}

pub fn run(args: BrowserArgs) -> Result<()> {
    startup_status("Preparing workspace…");
    let workspace = repl::vendor::ensure_browser_workspace()?;
    startup_status_done();

    if args.connect {
        startup_status("Connecting to Chrome CDP…");
    } else {
        startup_status("Starting Chrome…");
    }
    let chrome_proc = super::chrome::ensure_cdp(&args)?;
    startup_status_done();

    let vendor = if args.no_ai {
        None
    } else {
        startup_status("Preparing Cursor agent…");
        let vendor = provider_detect::spawn_ai(&args, &workspace)?;
        startup_status_done();
        vendor
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(repl::run(args, chrome_proc, vendor, workspace))
}
