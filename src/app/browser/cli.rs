use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{ArgAction, Args, Subcommand};

use super::cdp::CdpSession;
use super::chrome::{self, ChromeProcess};
use super::provider_detect;
use super::repl;
use super::script::{
    default_output_for_script, format_script_usage, is_file_output, load_pagemd_script,
    merge_params_object, parse_delay, parse_param_kv, run_pagemd_script, RunOptions,
};
use serde_json::{json, Value};

#[derive(Subcommand, Debug, Clone)]
pub enum BrowserCommand {
    #[command(
        about = "Interactive Chrome REPL for authoring and tuning extraction scripts",
        long_about = "Launch (or connect to) Chrome with a dedicated profile and drive the page\n\
                      from a slash-command REPL (/goto, /eval, /undo, /snap, /md, /run, …).\n\n\
                      Usage:\n  \
                      pagemd browser dev\n  \
                      pagemd browser dev --url https://example.com\n  \
                      pagemd browser dev --connect --port 9222\n  \
                      pagemd browser dev --clean --url https://example.com"
    )]
    Dev(BrowserDevArgs),

    #[command(
        about = "Run a .pagemd.js script over CDP and write Markdown",
        long_about = "One-shot runner: open (or connect to) Chrome, navigate to --url, execute the\n\
                      script's clean/extract/navigate/stop hooks, and write Markdown.\n\n\
                      Scripts may declare `const defaultParams = { … }` and read `params` in hooks.\n\
                      Override with repeatable `--param KEY=VALUE` or `--params '{…}'`.\n\
                      Override which pages may run with `--filter GLOB` (full URL or path like `/document/*`).\n\
                      Print script help (params) without launching Chrome: `--usage`.\n\n\
                      Usage:\n  \
                      pagemd browser script site.pagemd.js --usage\n  \
                      pagemd browser script site.pagemd.js --url https://example.com/docs\n  \
                      pagemd browser script site.pagemd.js --url https://other.example/a \\\n    \
                      --filter '/document/*'\n  \
                      pagemd browser script site.pagemd.js --url https://example.com/a -o docs-out\n  \
                      pagemd browser script site.pagemd.js --url https://example.com/a \\\n    \
                      --param stopUrl=https://example.com/last --max-pages 10\n  \
                      pagemd browser script site.pagemd.js --url https://example.com/a --headless"
    )]
    Script(BrowserScriptArgs),
}

/// Shared Chrome launch / connect flags (no URL — each subcommand owns that).
#[derive(Args, Debug, Clone)]
pub struct ChromeArgs {
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
}

#[derive(Args, Debug, Clone)]
pub struct BrowserDevArgs {
    #[arg(long, help = "Navigate to this URL after Chrome starts")]
    pub url: Option<String>,

    #[command(flatten)]
    pub chrome: ChromeArgs,

    #[arg(long, default_value = "auto", help = "AI backend: auto | cursor")]
    pub provider: String,

    #[arg(long, help = "Disable Cursor agent; slash commands only")]
    pub no_ai: bool,

    #[arg(long, default_value = ">", help = "REPL input prompt")]
    pub prompt: String,
}

#[derive(Args, Debug, Clone)]
pub struct BrowserScriptArgs {
    #[arg(value_name = "SCRIPT", help = "Path to a .pagemd.js script")]
    pub script: PathBuf,

    #[arg(
        long,
        help = "Print script usage and params, then exit (does not launch Chrome)"
    )]
    pub usage: bool,

    #[arg(
        long,
        required_unless_present = "usage",
        help = "Start URL after Chrome opens"
    )]
    pub url: Option<String>,

    #[arg(
        long = "filter",
        value_name = "GLOB",
        help = "Optional URL allow filter (full URL or path glob, e.g. /document/*)"
    )]
    pub filter: Option<String>,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        help = "Output directory (default: <script-stem>-run/) or a .md file for one combined document"
    )]
    pub output: Option<PathBuf>,

    #[arg(
        long,
        value_name = "N",
        help = "Maximum pages to extract (default: unlimited; 0 also means unlimited)"
    )]
    pub max_pages: Option<usize>,

    #[arg(
        long,
        default_value = "800:1600",
        value_name = "MS|MIN:MAX",
        help = "Delay between navigations in milliseconds"
    )]
    pub delay: String,

    #[arg(long, help = "Do not prepend # title from extract()")]
    pub no_title: bool,

    #[arg(long, help = "Do not prepend > Source: <url>")]
    pub no_source: bool,

    #[arg(
        short = 'p',
        long = "param",
        value_name = "KEY=VALUE",
        action = ArgAction::Append,
        help = "Override a script param (repeatable). Value is JSON if parseable, else a string"
    )]
    pub param: Vec<String>,

    #[arg(
        long = "params",
        value_name = "JSON",
        help = "JSON object of script params (merged with --param)"
    )]
    pub params: Option<String>,

    #[command(flatten)]
    pub chrome: ChromeArgs,
}

/// Launch parameters passed to chrome spawn/connect helpers.
#[derive(Debug, Clone)]
pub struct ChromeLaunch {
    pub url: Option<String>,
    pub port: u16,
    pub chrome_path: Option<PathBuf>,
    pub user_data_dir: Option<PathBuf>,
    pub clean: bool,
    pub connect: bool,
    pub headless: bool,
}

impl ChromeArgs {
    pub fn with_url(&self, url: Option<String>) -> ChromeLaunch {
        ChromeLaunch {
            url,
            port: self.port,
            chrome_path: self.chrome_path.clone(),
            user_data_dir: self.user_data_dir.clone(),
            clean: self.clean,
            connect: self.connect,
            headless: self.headless,
        }
    }
}

impl BrowserDevArgs {
    pub fn launch(&self) -> ChromeLaunch {
        self.chrome.with_url(self.url.clone())
    }
}

impl BrowserScriptArgs {
    pub fn launch(&self) -> Result<ChromeLaunch> {
        let url = self
            .url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--url is required unless --usage"))?;
        Ok(self.chrome.with_url(Some(url)))
    }

    pub fn run_options(&self, cwd: &Path) -> Result<RunOptions> {
        let max_pages = match self.max_pages {
            Some(0) | None => None,
            Some(n) => Some(n),
        };
        let delay_ms = parse_delay(&self.delay)?;
        let script_path = if self.script.is_absolute() {
            self.script.clone()
        } else {
            cwd.join(&self.script)
        };
        let output = match &self.output {
            Some(path) if path.is_absolute() => path.clone(),
            Some(path) => cwd.join(path),
            None => default_output_for_script(&script_path, cwd),
        };
        let mut params = json!({});
        if let Some(raw) = &self.params {
            let patch: Value = serde_json::from_str(raw).context("invalid --params JSON")?;
            merge_params_object(&mut params, patch)?;
        }
        for raw in &self.param {
            let (key, value) = parse_param_kv(raw)?;
            merge_params_object(&mut params, json!({ key: value }))?;
        }
        Ok(RunOptions {
            max_pages,
            delay_ms,
            output,
            include_title: !self.no_title,
            include_source_url: !self.no_source,
            params,
            filter: self
                .filter
                .as_ref()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        })
    }
}

fn startup_status(msg: &str) {
    eprint!("\r{msg}");
    let _ = io::stderr().flush();
}

fn startup_status_done() {
    eprintln!();
}

pub fn run(cmd: BrowserCommand) -> Result<()> {
    match cmd {
        BrowserCommand::Dev(args) => run_dev(args),
        BrowserCommand::Script(args) => run_script(args),
    }
}

fn run_dev(args: BrowserDevArgs) -> Result<()> {
    startup_status("Preparing workspace…");
    let workspace = repl::vendor::ensure_browser_workspace()?;
    startup_status_done();

    let launch = args.launch();
    if launch.connect {
        startup_status("Connecting to Chrome CDP…");
    } else {
        startup_status("Starting Chrome…");
    }
    let chrome_proc = chrome::ensure_cdp(&launch)?;
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

fn run_script(args: BrowserScriptArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("read cwd")?;
    let script_path = if args.script.is_absolute() {
        args.script.clone()
    } else {
        cwd.join(&args.script)
    };
    let script = load_pagemd_script(&script_path)
        .with_context(|| format!("load {}", script_path.display()))?;

    if args.usage {
        print!("{}", format_script_usage(&script_path, &script));
        return Ok(());
    }

    let url = args
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--url is required unless --usage"))?;
    let opts = args.run_options(&cwd)?;

    let launch = args.launch()?;
    if launch.connect {
        startup_status("Connecting to Chrome CDP…");
    } else {
        startup_status("Starting Chrome…");
    }
    let mut chrome_proc = chrome::ensure_cdp(&launch)?;
    startup_status_done();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = rt.block_on(async {
        eprintln!("Connecting to page…");
        let session = CdpSession::connect_with_hint(launch.port, Some(url)).await?;
        let current = session.current_url().await.unwrap_or_default();
        if current.is_empty() || current == "about:blank" || current != url {
            session.navigate(url).await?;
        }
        // Let the document settle before the first clean/extract.
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        eprintln!(
            "Running {} (filter: {}, max-pages: {})…",
            script_path.display(),
            opts.filter_glob().unwrap_or("none"),
            opts.max_pages
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unlimited".into())
        );
        run_pagemd_script(&session, &script, &opts).await
    });

    // Drop Chrome before propagating errors so profiles unlock cleanly.
    drop_chrome(&mut chrome_proc);
    let report = result?;
    let dest = if is_file_output(&report.output) {
        report.output.display().to_string()
    } else {
        format!(
            "{}/",
            report.output.display().to_string().trim_end_matches('/')
        )
    };
    println!(
        "Done: {} page(s) → {}\nStop reason: {}",
        report.pages.len(),
        dest,
        report.stop_reason
    );
    Ok(())
}

fn drop_chrome(chrome_proc: &mut Option<ChromeProcess>) {
    if let Some(proc) = chrome_proc.take() {
        drop(proc);
    }
}
