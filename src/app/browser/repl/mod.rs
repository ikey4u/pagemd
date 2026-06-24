pub(crate) mod vendor;

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde_json::Value;

use self::vendor::CursorRelay;
use super::bridge::BrowserBridge;
use super::cdp::CdpSession;
use super::cli::BrowserArgs;
use super::export::build_export_prompt;
use super::pretty::PRETTY_PROMPT;
use super::runtime::BrowserRuntime;
use super::sandbox;
use super::session_md::SessionMarkdown;
use super::session_preview::{self, SessionPreview};
use super::snap::{self, format_snap};
use super::tools::format_eval_result;
use super::undo::{DomTarget, UndoStack};
use super::workspace::ensure_mcp_config;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

struct ReplContext<'a> {
    session: &'a CdpSession,
    undo: &'a mut UndoStack,
    sandbox_enabled: &'a Arc<AtomicBool>,
    vendor: Option<&'a CursorRelay>,
    _workspace: &'a Path,
    export_dir: &'a Path,
    session_md: &'a Arc<SessionMarkdown>,
    session_preview: &'a mut Option<SessionPreview>,
    session_preview_opened: &'a mut bool,
}

fn repl_dom_target(sandbox_enabled: &Arc<AtomicBool>) -> DomTarget {
    if sandbox::is_enabled(sandbox_enabled) {
        DomTarget::Sandbox
    } else {
        DomTarget::Live
    }
}

pub async fn run(
    args: BrowserArgs,
    mut chrome_proc: Option<super::chrome::ChromeProcess>,
    vendor: Option<CursorRelay>,
    workspace: std::path::PathBuf,
) -> Result<()> {
    eprintln!("Connecting to page…");
    let session = CdpSession::connect_with_hint(args.port, args.url.as_deref()).await?;
    let undo = Arc::new(Mutex::new(UndoStack::new(50)));
    let sandbox_enabled = Arc::new(AtomicBool::new(false));

    if let Some(url) = &args.url {
        let current = session.current_url().await.unwrap_or_default();
        if current.is_empty() || current == "about:blank" {
            session.navigate(url).await?;
            undo.lock()
                .await
                .capture_baseline(&session, DomTarget::Live)
                .await?;
        }
    }

    ensure_mcp_config(&workspace)?;
    let session_md = Arc::new(SessionMarkdown::new(&workspace));
    session_md.bind_to_page(&session).await?;
    let export_dir = std::env::current_dir()
        .context("read current working directory for /export")?
        .canonicalize()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| workspace.clone()));
    let bridge = BrowserBridge::start(
        &workspace,
        args.port,
        &export_dir,
        session.clone(),
        Arc::clone(&undo),
        Arc::clone(&session_md),
        Arc::clone(&sandbox_enabled),
        args.url.clone(),
    )
    .await?;

    let mut session_preview: Option<SessionPreview> = None;
    let mut session_preview_opened = false;

    let ai_forward = vendor.is_some();
    print_banner(
        &args,
        chrome_proc.as_ref().map(|p| p.user_data_dir()),
        vendor.as_ref(),
        &bridge.runtime,
    )?;
    print_status(&session, &undo, vendor.is_some(), ai_forward).await?;
    eprintln!("Export dir: {}", export_dir.display());

    let mut rl = DefaultEditor::new()?;
    let prompt = format!("{} ", args.prompt);
    let mut ai_forward = ai_forward;

    loop {
        let line = match rl.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(err) => return Err(err.into()),
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        rl.add_history_entry(trimmed)?;

        if trimmed == "/quit" || trimmed == "/exit" || trimmed == "/q" {
            break;
        }

        if trimmed.starts_with('/') {
            let mut local_undo = undo.lock().await;
            let mut ctx = ReplContext {
                session: &session,
                undo: &mut local_undo,
                sandbox_enabled: &sandbox_enabled,
                vendor: vendor.as_ref(),
                _workspace: &workspace,
                export_dir: &export_dir,
                session_md: &session_md,
                session_preview: &mut session_preview,
                session_preview_opened: &mut session_preview_opened,
            };
            match handle_slash(trimmed, &mut ctx).await {
                Ok(SlashOutcome::Continue { refresh_status }) => {
                    if refresh_status {
                        print_status(&session, &undo, vendor.is_some(), ai_forward).await?;
                    }
                    repl_prepare_next_prompt();
                }
                Ok(SlashOutcome::Quit) => break,
                Ok(SlashOutcome::SetAiForward(enabled)) => {
                    ai_forward = enabled;
                    print_status(&session, &undo, vendor.is_some(), ai_forward).await?;
                    repl_prepare_next_prompt();
                }
                Err(err) => {
                    eprintln!("error: {err:#}");
                    repl_prepare_next_prompt();
                }
            }
            continue;
        }

        if !ai_forward {
            println!("AI forwarding off (/ai to enable). Input ignored.");
            continue;
        }

        let Some(v) = vendor.as_ref() else {
            println!("No AI backend (--no-ai). Use slash commands only.");
            continue;
        };

        if let Err(err) = v.send_user_line(trimmed).await {
            eprintln!("agent error: {err:#}");
            repl_prepare_next_prompt();
        }
    }

    if let Some(v) = vendor {
        v.shutdown()?;
    }

    session_preview::SessionPreview::shutdown(session_preview).await;

    // Release CDP + bridge before asking Chrome to quit.
    drop(bridge);
    drop(session);

    if let Some(chrome) = chrome_proc.as_mut() {
        eprintln!("Closing Chrome…");
        chrome.shutdown_gracefully(args.port).await;
    }

    Ok(())
}

enum SlashOutcome {
    Continue { refresh_status: bool },
    Quit,
    SetAiForward(bool),
}

const TERMINAL_PREVIEW_CHARS: usize = 1_200;

async fn handle_slash(line: &str, ctx: &mut ReplContext<'_>) -> Result<SlashOutcome> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("").to_ascii_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    match cmd.as_str() {
        "/help" | "/h" | "/?" => {
            print_help(ctx.vendor.is_some(), Some(ctx.export_dir));
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/quit" | "/exit" | "/q" => return Ok(SlashOutcome::Quit),
        "/goto" => {
            if rest.is_empty() {
                bail!("usage: /goto <url>");
            }
            ctx.sandbox_enabled.store(false, Ordering::SeqCst);
            ctx.undo.reset();
            ctx.session.navigate(rest).await?;
            ctx.undo
                .capture_baseline(ctx.session, DomTarget::Live)
                .await?;
            ctx.session_md.bind_to_page(ctx.session).await?;
            println!("navigated");
            return Ok(SlashOutcome::Continue {
                refresh_status: true,
            });
        }
        "/reload" => {
            ctx.sandbox_enabled.store(false, Ordering::SeqCst);
            ctx.undo.reset();
            ctx.session.reload().await?;
            ctx.undo
                .capture_baseline(ctx.session, DomTarget::Live)
                .await?;
            ctx.session_md.bind_to_page(ctx.session).await?;
            println!("reloaded");
            return Ok(SlashOutcome::Continue {
                refresh_status: true,
            });
        }
        "/back" => {
            ctx.undo
                .push_before_mutate(ctx.session, repl_dom_target(ctx.sandbox_enabled))
                .await?;
            ctx.session.evaluate("history.back()", false).await?;
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            ctx.sandbox_enabled.store(false, Ordering::SeqCst);
            ctx.undo.reset();
            ctx.session_md.bind_to_page(ctx.session).await?;
            println!("back");
            return Ok(SlashOutcome::Continue {
                refresh_status: true,
            });
        }
        "/forward" => {
            ctx.undo
                .push_before_mutate(ctx.session, repl_dom_target(ctx.sandbox_enabled))
                .await?;
            ctx.session.evaluate("history.forward()", false).await?;
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            ctx.sandbox_enabled.store(false, Ordering::SeqCst);
            ctx.undo.reset();
            ctx.session_md.bind_to_page(ctx.session).await?;
            println!("forward");
            return Ok(SlashOutcome::Continue {
                refresh_status: true,
            });
        }
        "/snap" if rest.eq_ignore_ascii_case("send") => {
            let snap_value = if sandbox::is_enabled(ctx.sandbox_enabled) {
                sandbox::capture_page(ctx.session).await?
            } else {
                snap::capture_page(ctx.session, None).await?
            };
            let text = format_snap(&snap_value);
            print!("{text}");
            let Some(v) = ctx.vendor else {
                bail!("no AI backend; ensure `agent` is installed");
            };
            v.send_context_block(&text).await?;
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/snap" => {
            let snap_value = if sandbox::is_enabled(ctx.sandbox_enabled) {
                sandbox::capture_page(ctx.session).await?
            } else {
                snap::capture_page(ctx.session, None).await?
            };
            print!("{}", format_snap(&snap_value));
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/eval" => {
            if rest.is_empty() {
                bail!("usage: /eval [--no-undo] <javascript expression>");
            }
            let no_undo = rest.split_whitespace().any(|t| t == "--no-undo");
            let expr = rest
                .split_whitespace()
                .filter(|t| *t != "--no-undo")
                .collect::<Vec<_>>()
                .join(" ");
            if expr.is_empty() {
                bail!("usage: /eval [--no-undo] <javascript expression>");
            }
            let target = repl_dom_target(ctx.sandbox_enabled);
            if !no_undo {
                ctx.undo.push_before_mutate(ctx.session, target).await?;
            }
            let eval_result = if target == DomTarget::Sandbox {
                sandbox::eval_expression(ctx.session, &expr).await
            } else {
                ctx.session.evaluate(&expr, false).await
            };
            match eval_result {
                Ok(value) => print_eval_result(&value),
                Err(err) => {
                    if !no_undo {
                        let _ = ctx.undo.undo_one(ctx.session, target).await;
                    }
                    eprintln!("eval error: {err:#}");
                }
            }
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/undo" => {
            let target = repl_dom_target(ctx.sandbox_enabled);
            if rest.eq_ignore_ascii_case("all") {
                if ctx.undo.undo_all(ctx.session, target).await? {
                    println!("undid all changes");
                } else {
                    println!("nothing to undo");
                }
            } else if rest.is_empty() {
                if ctx.undo.undo_one(ctx.session, target).await? {
                    println!("undid last step");
                } else {
                    println!("nothing to undo");
                }
            } else {
                bail!("usage: /undo [all]");
            }
            return Ok(SlashOutcome::Continue {
                refresh_status: true,
            });
        }
        "/html" => {
            let (opts, path) = parse_output_args(rest);
            eprintln!("Fetching page HTML…");
            let html = if opts.body_only {
                snap::capture_body_html(ctx.session, None).await?
            } else {
                snap::capture_html(ctx.session, None).await?
            };
            if let Some(path) = path {
                fs::write(&path, &html).with_context(|| format!("write {}", path.display()))?;
                println!("wrote {} ({} bytes)", path.display(), html.len());
            } else {
                print_terminal_preview(&html, TERMINAL_PREVIEW_CHARS, "/html -o file.html");
            }
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/md" => {
            let (_opts, path) = parse_output_args(rest);
            eprintln!("Converting body HTML to Markdown…");
            let html = snap::capture_body_html(ctx.session, None).await?;
            let md = snap::html_to_markdown(&html)?;
            if let Some(path) = path {
                fs::write(&path, &md).with_context(|| format!("write {}", path.display()))?;
                println!("wrote {} ({} bytes)", path.display(), md.len());
            } else {
                print_terminal_preview(&md, TERMINAL_PREVIEW_CHARS, "/md -o file.md");
            }
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/url" => {
            println!("{}", ctx.session.current_url().await?);
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/title" => {
            println!("{}", ctx.session.current_title().await?);
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/stop" => {
            let Some(v) = ctx.vendor else {
                bail!("no AI backend running");
            };
            v.interrupt()?;
            println!("interrupted Cursor agent");
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/provider" => {
            if ctx.vendor.is_some() {
                println!("cursor (agent -p stream)");
            } else {
                println!("none (--no-ai)");
            }
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/pmd" => return handle_pmd(rest, ctx).await,
        "/export" => {
            let Some(v) = ctx.vendor else {
                bail!("no AI backend; /export requires Cursor agent");
            };
            let prompt = build_export_prompt(
                ctx.export_dir,
                if rest.trim().is_empty() {
                    None
                } else {
                    Some(rest.trim())
                },
            );
            eprintln!(
                "[agent] exporting .pagemd.js → {}",
                ctx.export_dir.display()
            );
            io::stderr().flush()?;
            v.send_user_line(&prompt).await?;
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/pretty" => {
            let Some(v) = ctx.vendor else {
                bail!("no AI backend; /pretty requires Cursor agent");
            };
            sandbox::begin(ctx.session, ctx.session_md, ctx.sandbox_enabled, ctx.undo).await?;
            eprintln!("Sandbox active — visible tab unchanged; agent cleans hidden DOM copy.");
            eprintln!("[agent] page cleanup… (stream below; /stop to cancel)");
            io::stderr().flush()?;
            v.send_user_line(PRETTY_PROMPT).await?;
            return Ok(SlashOutcome::Continue {
                refresh_status: false,
            });
        }
        "/manual" => return Ok(SlashOutcome::SetAiForward(false)),
        "/ai" => {
            if ctx.vendor.is_none() {
                bail!("no AI backend; cannot enable /ai");
            }
            return Ok(SlashOutcome::SetAiForward(true));
        }
        other => bail!("unknown command: {other} (try /help)"),
    }
}

async fn handle_pmd(rest: &str, ctx: &mut ReplContext<'_>) -> Result<SlashOutcome> {
    let live = rest.split_whitespace().any(|t| t == "--live");
    let original = rest.split_whitespace().any(|t| t == "--original");
    let open_only = rest.eq_ignore_ascii_case("open");
    let no_open = rest.split_whitespace().any(|t| t == "--no-open");

    ctx.session_md.bind_to_page(ctx.session).await?;

    if original {
        if !ctx.session_md.has_original_baseline() {
            ctx.session_md
                .capture_original_baseline(ctx.session, None)
                .await?;
        }
        let md_path = ctx.session_md.original_md_path();
        let preview = session_preview::SessionPreview::ensure_at_path(
            ctx.session_preview,
            md_path,
            "PageMD original",
        )
        .await?;

        if !open_only {
            preview.trigger_render();
        }
        if (open_only || !*ctx.session_preview_opened) && !no_open {
            preview.open_browser()?;
            *ctx.session_preview_opened = true;
        }

        println!("Original preview: {}", preview.url());
        println!(
            "Original file: {}",
            ctx.session_md.original_md_path().display()
        );
        if let Some(md) = ctx.session_md.load_original_markdown()? {
            println!(
                "({} chars — unmodified page baseline; compare with /pmd for cleaned output)",
                md.chars().count()
            );
        }
        return Ok(SlashOutcome::Continue {
            refresh_status: false,
        });
    }

    if !open_only {
        if live {
            eprintln!("Extracting Markdown from live DOM…");
            ctx.session_md.capture_from_live(ctx.session, None).await?;
        } else if sandbox::is_enabled(ctx.sandbox_enabled) {
            eprintln!("Refreshing session Markdown from sandbox DOM…");
            ctx.session_md.capture_from_sandbox(ctx.session).await?;
        } else if ctx.session_md.load_from_disk().await?.is_none() {
            eprintln!("No session Markdown for this URL yet; extracting from live DOM…");
            ctx.session_md.capture_from_live(ctx.session, None).await?;
        }
    }

    let preview =
        session_preview::SessionPreview::ensure(ctx.session_preview, ctx.session_md).await?;

    if !open_only {
        preview.trigger_render();
    }

    if (open_only || !*ctx.session_preview_opened) && !no_open {
        preview.open_browser()?;
        *ctx.session_preview_opened = true;
    }

    let snap = ctx.session_md.snapshot().await;
    println!("Session preview: {}", preview.url());
    println!("Session file: {}", ctx.session_md.file_path().display());
    if let Some(url) = ctx.session_md.active_page_url() {
        println!("Page URL: {url}");
    }
    if snap.markdown.trim().is_empty() {
        println!("(empty — run /pretty or browser_save_markdown after DOM cleanup)");
    } else {
        let note = if sandbox::is_enabled(ctx.sandbox_enabled) {
            "sandbox cleaned"
        } else {
            "session file"
        };
        println!(
            "({} chars — {note}; use /pmd --original to compare with unmodified baseline)",
            snap.markdown.chars().count()
        );
    }

    Ok(SlashOutcome::Continue {
        refresh_status: false,
    })
}

fn print_terminal_preview(text: &str, max_chars: usize, save_hint: &str) {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        println!("{text}");
        return;
    }
    let preview: String = text.chars().take(max_chars).collect();
    println!("{preview}…\n[preview truncated, {char_count} chars total; use {save_hint}]");
}

fn repl_prepare_next_prompt() {
    let _ = writeln!(io::stdout());
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
}

struct OutputOpts {
    body_only: bool,
}

fn parse_output_args(rest: &str) -> (OutputOpts, Option<&Path>) {
    let mut body_only = false;
    let mut path: Option<&Path> = None;
    let mut tokens = rest.split_whitespace().peekable();

    while let Some(token) = tokens.peek().copied() {
        match token {
            "-o" | "--output" => {
                tokens.next();
                path = tokens.next().map(Path::new);
            }
            "--body" => {
                tokens.next();
                body_only = true;
            }
            _ if path.is_none()
                && (token.starts_with('-')
                    || token.ends_with(".md")
                    || token.ends_with(".html")) =>
            {
                if token.starts_with('-') {
                    break;
                }
                path = Some(Path::new(token));
                tokens.next();
            }
            _ => break,
        }
    }

    (OutputOpts { body_only }, path)
}

fn print_eval_result(value: &Value) {
    println!("{}", format_eval_result(value));
}

fn print_banner(
    args: &BrowserArgs,
    profile: Option<&Path>,
    vendor: Option<&CursorRelay>,
    runtime: &BrowserRuntime,
) -> Result<()> {
    println!("PageMD Browser — CDP REPL + Cursor agent");
    println!("  CDP port: {}", args.port);
    if args.connect {
        println!("  mode: connect (existing Chrome)");
    } else if args.clean {
        println!("  profile: ephemeral (--clean)");
    } else if let Some(dir) = &args.user_data_dir {
        println!("  profile: {}", dir.display());
    } else if let Some(dir) = profile {
        println!("  profile: {}", dir.display());
    }
    if vendor.is_some() {
        println!("  AI: cursor (agent -p stream + MCP browser tools)");
    } else if args.no_ai {
        println!("  AI: disabled (--no-ai)");
    } else {
        println!("  AI: none");
    }
    println!("  Type /help for commands, /quit to exit.");
    if vendor.is_some() {
        println!("  MCP bridge: {}", runtime.bridge_url);
        println!("  Tools: browser_snap, browser_eval, browser_get_html, browser_get_markdown, browser_undo, …");
    }
    println!();
    Ok(())
}

async fn print_status(
    session: &CdpSession,
    undo: &Arc<Mutex<UndoStack>>,
    ai: bool,
    ai_forward: bool,
) -> Result<()> {
    let depth = undo.lock().await.len();
    let url = session.current_url().await.unwrap_or_else(|_| "?".into());
    let title = session.current_title().await.unwrap_or_else(|_| "?".into());
    let short_url = if url.len() > 72 {
        format!("{}…", &url[..72])
    } else {
        url
    };
    eprintln!("Tab: {title}");
    eprintln!("URL: {short_url}  |  undo: {depth}");
    if ai {
        eprintln!(
            "AI: cursor{}",
            if ai_forward {
                " (forwarding)"
            } else {
                " (manual — /ai to forward)"
            }
        );
    }
    Ok(())
}

fn print_help(ai: bool, export_dir: Option<&Path>) {
    println!("Commands:");
    println!("  /help                 Show this help");
    println!("  /quit                 Exit REPL and stop spawned Chrome");
    println!("  /goto <url>           Navigate");
    println!("  /reload               Reload page");
    println!("  /back  /forward       History navigation");
    println!("  /snap                 Page summary (URL, title, outline)");
    if ai {
        println!("  /snap send            Snap + forward context to Cursor agent");
        println!(
            "  /stop                 Interrupt in-flight agent turn (same as Ctrl+C during agent)"
        );
        println!("  /manual  /ai          Disable / enable natural-language forwarding");
        println!("  /provider             Show AI backend");
    }
    println!("  /eval [--no-undo] <js>  Run JavaScript (use --no-undo on large pages; errors stay in REPL)");
    println!("  /undo                 Undo last mutating step");
    println!("  /undo all             Restore baseline DOM for this session");
    println!("  /html [-o file]       Dump HTML (terminal preview; use -o for full output)");
    println!("  /md [-o file]         Convert body HTML to Markdown (preview; use -o for full)");
    println!("  /pmd [--live] [--original] [open]  Session or original Markdown preview");
    println!("  /url  /title          Print current URL or title");
    if ai {
        println!("  /pretty               Clean DOM in sandbox (visible tab unchanged) via Cursor");
        println!("  /export [name]        Export validated .pagemd.js to REPL cwd");
        println!();
        println!("Agent output: [thinking]/[assistant]; MCP as [agent] → pagemd-browser.browser_* (args + result)");
        println!("  Ctrl+C during agent output interrupts the turn; at the prompt Ctrl+C clears the line");
        println!("  Set PAGEMD_VERBOSE_TOOLS=1 for full MCP JSON args/result");
        if let Some(dir) = export_dir {
            println!("Export dir (cwd at startup): {}", dir.display());
        }
    }
}
