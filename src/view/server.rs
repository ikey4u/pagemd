use std::collections::HashSet;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use futures::stream::Stream;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tokio::sync::broadcast;

use super::live;
use super::ViewOptions;

pub struct RenderRequest {
    pub inputs: Vec<PathBuf>,
}

pub enum RenderResult {
    Ok {
        html: String,
        extra_watch_paths: Vec<PathBuf>,
    },
    Err {
        html: String,
    },
}

pub struct AppState {
    /// Clean HTML without the live-reload script (safe to export).
    html: RwLock<String>,
    version: AtomicU64,
    notify_tx: broadcast::Sender<u64>,
    export_path: Option<PathBuf>,
}

struct WatchState {
    debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    watched: HashSet<PathBuf>,
}

pub fn run(
    options: ViewOptions,
    render: impl Fn(RenderRequest) -> RenderResult + Send + Sync + 'static,
) -> Result<()> {
    let render = Arc::new(render);
    let (notify_tx, _) = broadcast::channel(64);

    let first = render(RenderRequest {
        inputs: options.inputs.clone(),
    });
    let (initial_html, initial_extra, export_initial) = match first {
        RenderResult::Ok { html, extra_watch_paths } => (html, extra_watch_paths, true),
        RenderResult::Err { html } => (html, Vec::new(), false),
    };

    let state = Arc::new(AppState {
        html: RwLock::new(initial_html),
        version: AtomicU64::new(0),
        notify_tx: notify_tx.clone(),
        export_path: options.export_path.clone(),
    });

    if export_initial {
        if let Ok(guard) = state.html.read() {
            if let Err(err) = write_export_if_configured(state.export_path.as_deref(), &guard) {
                eprintln!("Export error: {err:#}");
            }
        }
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let (render_tx, render_rx) = std::sync::mpsc::channel::<()>();

    let mut watch_paths = options.watch_paths.clone();
    watch_paths.extend(initial_extra);
    let watch_state = Arc::new(Mutex::new(setup_watcher(watch_paths, render_tx.clone())?));
    let watch_weak = Arc::downgrade(&watch_state);

    let render_worker = spawn_render_worker(
        render_rx,
        state.clone(),
        watch_weak,
        options.inputs.clone(),
        render,
        shutdown.clone(),
    );

    let addr: SocketAddr = format!("{}:{}", options.host, options.port)
        .parse()
        .context("Invalid host/port")?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to start async runtime")?;

    let url = format!("http://{addr}/");
    let serve_url = url.clone();

    rt.block_on(async {
        let app = Router::new()
            .route("/", get(index_handler))
            .route("/__events", get(events_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("Cannot bind to {addr}"))?;

        eprintln!("Preview server listening at {serve_url}");
        let watch_count = watch_state
            .lock()
            .map(|guard| guard.watched.len())
            .unwrap_or(0);
        eprintln!("Watching {watch_count} path(s) for changes");

        if options.open_browser {
            open_url(&serve_url)?;
        }

        let server = axum::serve(listener, app);
        tokio::select! {
            result = server => {
                result.context("HTTP server error")?;
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\nShutting down preview server...");
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    shutdown.store(true, Ordering::SeqCst);
    drop(watch_state);
    drop(render_tx);
    if let Err(err) = render_worker.join() {
        eprintln!("Render worker panicked: {err:?}");
    }

    eprintln!("Preview server stopped ({url}).");
    Ok(())
}

fn setup_watcher(
    paths: Vec<PathBuf>,
    render_tx: std::sync::mpsc::Sender<()>,
) -> Result<WatchState> {
    let watched = HashSet::new();

    let debouncer = new_debouncer(Duration::from_millis(300), move |result: DebounceEventResult| {
        let Ok(events) = result else {
            return;
        };
        if should_trigger_render(&events) {
            let _ = render_tx.send(());
        }
    })
    .context("Failed to create file watcher")?;

    let mut state = WatchState {
        debouncer,
        watched,
    };

    for path in paths {
        let recursive = path.is_dir();
        register_watch(&mut state, &path, recursive)?;
    }

    Ok(state)
}

fn should_trigger_render(events: &[notify_debouncer_mini::DebouncedEvent]) -> bool {
    !events.is_empty()
}

fn register_watch(state: &mut WatchState, path: &PathBuf, recursive: bool) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !canonical.exists() {
        return Ok(());
    }
    if !state.watched.insert(canonical.clone()) {
        return Ok(());
    }

    let mode = if canonical.is_dir() && recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    state
        .debouncer
        .watcher()
        .watch(&canonical, mode)
        .with_context(|| format!("Cannot watch {}", canonical.display()))?;

    eprintln!("  watch {}", canonical.display());

    // When watching a file, also watch its parent (shallow) for new sibling assets.
    if canonical.is_file() {
        if let Some(parent) = canonical.parent() {
            if !parent.as_os_str().is_empty() {
                register_watch(state, &parent.to_path_buf(), false)?;
            }
        }
    }

    Ok(())
}

fn register_extra_watches(state: &mut WatchState, paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        register_watch(state, path, false)?;
    }
    Ok(())
}

fn drain_render_triggers(render_rx: &std::sync::mpsc::Receiver<()>) {
    while render_rx.try_recv().is_ok() {}
}

fn spawn_render_worker(
    render_rx: std::sync::mpsc::Receiver<()>,
    state: Arc<AppState>,
    watch_state: std::sync::Weak<Mutex<WatchState>>,
    inputs: Vec<PathBuf>,
    render: Arc<dyn Fn(RenderRequest) -> RenderResult + Send + Sync>,
    shutdown: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            match render_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(()) => drain_render_triggers(&render_rx),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            let result = render(RenderRequest {
                inputs: inputs.clone(),
            });

            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            match result {
                RenderResult::Ok {
                    html,
                    extra_watch_paths,
                } => {
                    commit_html(&state, html, true);
                    if let Some(watch_state) = watch_state.upgrade() {
                        if let Ok(mut guard) = watch_state.lock() {
                            if let Err(err) = register_extra_watches(&mut guard, &extra_watch_paths) {
                                eprintln!("Watch registration error: {err:#}");
                            }
                        }
                    }
                }
                RenderResult::Err { html } => {
                    commit_html(&state, html, false);
                }
            }
        }
    })
}

fn commit_html(state: &Arc<AppState>, html: String, export: bool) {
    let changed = match state.html.write() {
        Ok(mut guard) => {
            if guard.as_str() == html.as_str() {
                false
            } else {
                *guard = html;
                true
            }
        }
        Err(_) => false,
    };

    if !changed {
        return;
    }

    if export {
        if let Ok(guard) = state.html.read() {
            if let Err(err) = write_export_if_configured(state.export_path.as_deref(), &guard) {
                eprintln!("Export error: {err:#}");
            }
        }
    }

    let version = state.version.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = state.notify_tx.send(version);
    eprintln!("Reloaded (v{version})");
}

fn write_export_if_configured(path: Option<&std::path::Path>, html: &str) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Cannot create {}", parent.display()))?;
        }
    }
    fs::write(path, html.as_bytes()).with_context(|| format!("Cannot export {}", path.display()))?;
    eprintln!("Exported -> {}", path.display());
    Ok(())
}

async fn index_handler(State(state): State<Arc<AppState>>) -> Html<String> {
    let html = state
        .html
        .read()
        .map(|guard| live::wrap_for_preview(guard.clone()))
        .unwrap_or_else(|_| live::wrap_for_preview("<p>Preview unavailable</p>".to_string()));
    Html(html)
}

async fn events_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let version = state.version.load(Ordering::SeqCst);
    let mut rx = state.notify_tx.subscribe();

    let stream = async_stream::stream! {
        yield Ok(Event::default().data(version.to_string()));
        loop {
            match rx.recv().await {
                Ok(v) => yield Ok(Event::default().data(v.to_string())),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream)
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    let status = command
        .status()
        .with_context(|| format!("Cannot open {url}"))?;
    if !status.success() {
        anyhow::bail!("Failed to open {url}");
    }
    Ok(())
}
