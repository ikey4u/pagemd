use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use crate::app::browser;
use crate::app::convert::run_convert;
use crate::app::preview;
use crate::app::preview::error::{build_preview_error_html, preview_html_opts};
use crate::core::{self, export_with_resources, prepare_resources, resolve_inputs, ConvertOptions};

#[derive(Parser, Debug)]
#[command(
    name = "pagemd",
    about = "Convert Markdown to a self-contained single HTML file",
    long_about = core::PAGEMD_LONG_ABOUT,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    args: CliArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(
        about = "Live-preview Markdown in the browser",
        long_about = "Start a local HTTP server and open the rendered page in your browser.\n\
                      The page hot-reloads when you save the input Markdown or referenced local assets.\n\
                      Press Ctrl+C to stop.\n\n\
                      Usage:\n  \
                      pagemd view -i INPUT.md\n  \
                      pagemd view -i doc.md --port 8080 --no-open\n\n\
                      Export clean SingleFile HTML (no live-reload script) on each successful render:\n  \
                      pagemd view -i doc.md --export out.html\n  \
                      pagemd view -i doc.md -o out.html\n\n\
                      One-shot export without preview:\n  \
                      pagemd -i doc.md -o out.html"
    )]
    View(ViewArgs),
    #[command(
        about = "Interactive browser REPL via Chrome DevTools Protocol",
        long_about = "Launch (or connect to) Chrome with a dedicated profile and drive the page\n\
                      from a slash-command REPL (/goto, /eval, /undo, /snap, /md, …).\n\n\
                      Usage:\n  \
                      pagemd browser\n  \
                      pagemd browser --url https://example.com\n  \
                      pagemd browser --connect --port 9222\n  \
                      pagemd browser --clean --url https://example.com"
    )]
    Browser(browser::cli::BrowserArgs),
    #[command(
        name = "browser-mcp",
        hide = true,
        about = "MCP stdio bridge for pagemd browser"
    )]
    BrowserMcp(browser::mcp_cli::BrowserMcpArgs),
}

#[derive(Args, Debug, Clone)]
struct ViewArgs {
    #[command(flatten)]
    convert: CliArgs,

    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Preview server bind address"
    )]
    host: String,

    #[arg(
        long,
        default_value_t = 3847,
        help = "Preview server port (if busy, picks a random available port)"
    )]
    port: u16,

    #[arg(long = "no-open", help = "Do not open the default browser")]
    no_open: bool,

    #[arg(
        long = "export",
        value_name = "FILE",
        help = "Write clean SingleFile HTML on each successful render (same as -o/--output)"
    )]
    export: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CliArgs {
    #[arg(
        short = 'i',
        long = "input",
        value_name = "FILE",
        num_args = 1..,
        help = "Markdown input file(s)"
    )]
    pub(crate) inputs: Vec<PathBuf>,

    #[arg(
        short = 'd',
        long = "dir",
        value_name = "DIR",
        num_args = 1..,
        help = "Directory/directories to scan recursively for Markdown files"
    )]
    pub(crate) directories: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        help = "Output HTML path (required for convert; in view, exports on each render)"
    )]
    pub(crate) output: Option<PathBuf>,

    #[arg(
        short = 'x',
        long = "exclude",
        value_name = "PATTERN",
        num_args = 1..,
        help = "Exclude files or directories while scanning (name, path, or glob such as drafts/**)"
    )]
    pub(crate) excludes: Vec<String>,

    #[arg(long = "title", value_name = "TITLE", help = "Document title")]
    pub(crate) title: Option<String>,

    #[arg(
        long = "icon",
        value_name = "XX",
        value_parser = parse_icon_arg,
        help = "Two-character favicon label (a-z, A-Z, 0-9); shown uppercase in the tab icon"
    )]
    pub(crate) icon: Option<String>,

    #[arg(long = "font-size", default_value = "16", help = "Math font size")]
    pub(crate) math_font_size: f64,

    #[arg(
        long = "katex-fonts",
        value_name = "DIR",
        help = "Directory containing KaTeX .ttf font files for glyph embedding"
    )]
    pub(crate) katex_fonts: Option<PathBuf>,
}

pub(crate) fn parse_icon_arg(s: &str) -> Result<String, String> {
    if s.chars().count() != 2 {
        return Err("icon must be exactly 2 characters".into());
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("icon must use only a-z, A-Z, and 0-9".into());
    }
    Ok(s.to_ascii_uppercase())
}

fn run_view(args: &ViewArgs) -> Result<()> {
    let export_path = args.export.clone().or_else(|| args.convert.output.clone());

    let convert_opts = ConvertOptions::from(&args.convert);
    let resolved = resolve_inputs(&convert_opts)?;
    preview::validate_inputs(&resolved.files)?;

    let resources = prepare_resources(&convert_opts)?;
    let title_hint = resolved.files.first().cloned();
    let html_opts = preview_html_opts();

    let sources: Vec<(PathBuf, String)> = resolved
        .files
        .iter()
        .map(|input| {
            let source = fs::read_to_string(input)
                .with_context(|| format!("Cannot read {}", input.display()))?;
            Ok((input.clone(), source))
        })
        .collect::<Result<_>>()?;

    // Prefer scan roots first so they are registered recursively before file
    // watches attach their parents as non-recursive.
    let mut watch_paths = resolved.directories.clone();
    watch_paths.extend(preview::collect_initial_watch_paths(
        &resolved.files,
        &sources,
    ));

    preview::run(
        preview::ViewOptions {
            host: args.host.clone(),
            port: args.port,
            inputs: resolved.files.clone(),
            watch_paths,
            open_browser: !args.no_open,
            export_path,
        },
        move |request: preview::RenderRequest| match export_with_resources(
            &convert_opts,
            &html_opts,
            &resources,
            title_hint.as_deref(),
        ) {
            Ok(document) => {
                let _current_preview_inputs = request.inputs;
                let extra_watch_paths = match resolve_inputs(&convert_opts) {
                    Ok(resolved) => {
                        preview::collect_render_watch_paths(&resolved.files, &resolved.directories)
                    }
                    Err(err) => {
                        eprintln!("Watch path refresh warning: {err:#}");
                        Vec::new()
                    }
                };
                preview::RenderResult::Ok {
                    html: document.html,
                    extra_watch_paths,
                }
            }
            Err(err) => {
                eprintln!("Render error: {err:#}");
                preview::RenderResult::Err {
                    html: build_preview_error_html(&err),
                }
            }
        },
    )
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::View(args)) => run_view(&args),
        Some(Commands::Browser(args)) => browser::run(args),
        Some(Commands::BrowserMcp(args)) => browser::mcp_cli::run(args),
        None => run_convert(&cli.args),
    }
}
