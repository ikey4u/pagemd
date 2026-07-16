use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures::stream::Stream;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle as TokioJoinHandle;

use super::live;
use super::ViewOptions;

const MERMAID_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mermaid.min.js"));

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

#[derive(Clone)]
pub struct HostedPreviewOptions {
    pub host: String,
    pub port: u16,
    pub inputs: Vec<PathBuf>,
    pub watch_paths: Vec<PathBuf>,
    pub export_path: Option<PathBuf>,
}

impl From<ViewOptions> for HostedPreviewOptions {
    fn from(options: ViewOptions) -> Self {
        Self {
            host: options.host,
            port: options.port,
            inputs: options.inputs,
            watch_paths: options.watch_paths,
            export_path: options.export_path,
        }
    }
}

struct AppState {
    /// Clean HTML without the live-reload script (safe to export).
    html: RwLock<String>,
    version: AtomicU64,
    notify_tx: broadcast::Sender<u64>,
    export_path: Option<PathBuf>,
}

struct WatchState {
    debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    /// Canonical path → whether the active watch is recursive.
    watched: HashMap<PathBuf, bool>,
}

struct PreviewEngineOptions {
    inputs: Vec<PathBuf>,
    watch_paths: Vec<PathBuf>,
    export_path: Option<PathBuf>,
}

impl From<HostedPreviewOptions> for PreviewEngineOptions {
    fn from(options: HostedPreviewOptions) -> Self {
        Self {
            inputs: options.inputs,
            watch_paths: options.watch_paths,
            export_path: options.export_path,
        }
    }
}

impl From<ViewOptions> for PreviewEngineOptions {
    fn from(options: ViewOptions) -> Self {
        HostedPreviewOptions::from(options).into()
    }
}

struct PreviewEngine {
    state: Arc<AppState>,
    shutdown: Arc<AtomicBool>,
    render_tx: Option<std::sync::mpsc::Sender<()>>,
    render_worker: Option<JoinHandle<()>>,
    watch_state: Arc<Mutex<WatchState>>,
}

impl PreviewEngine {
    fn start(
        options: PreviewEngineOptions,
        render: Arc<dyn Fn(RenderRequest) -> RenderResult + Send + Sync>,
    ) -> Result<Self> {
        let (notify_tx, _) = broadcast::channel(64);

        let first = render(RenderRequest {
            inputs: options.inputs.clone(),
        });
        let (initial_html, initial_extra, export_initial) = match first {
            RenderResult::Ok {
                html,
                extra_watch_paths,
            } => (html, extra_watch_paths, true),
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

        Ok(Self {
            state,
            shutdown,
            render_tx: Some(render_tx),
            render_worker: Some(render_worker),
            watch_state,
        })
    }

    fn start_in_current_thread(
        options: PreviewEngineOptions,
        render: Arc<dyn Fn(RenderRequest) -> RenderResult + Send + Sync>,
    ) -> Result<Self> {
        Self::start(options, render)
    }

    fn router(&self) -> Router {
        Router::new()
            .route("/", get(index_handler))
            .route("/__events", get(events_handler))
            .route("/__assets/mermaid.min.js", get(mermaid_asset_handler))
            .with_state(Arc::clone(&self.state))
    }

    fn trigger_render(&self) {
        if let Some(tx) = &self.render_tx {
            let _ = tx.send(());
        }
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.render_tx.take();
    }
}

fn join_worker_off_runtime(worker: JoinHandle<()>) {
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(move || {
            let _ = worker.join();
        });
    } else {
        let _ = worker.join();
    }
}

impl Drop for PreviewEngine {
    fn drop(&mut self) {
        self.stop();
        if let Some(worker) = self.render_worker.take() {
            join_worker_off_runtime(worker);
        }
    }
}

pub struct HostedPreview {
    engine: PreviewEngine,
    preview_url: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    _server: TokioJoinHandle<Result<()>>,
}

impl HostedPreview {
    pub async fn start(
        options: HostedPreviewOptions,
        render: impl Fn(RenderRequest) -> RenderResult + Send + Sync + 'static,
    ) -> Result<Self> {
        let render = Arc::new(render);
        let engine = tokio::task::spawn_blocking({
            let options = options.clone();
            let render = Arc::clone(&render);
            move || PreviewEngine::start_in_current_thread(options.into(), render)
        })
        .await
        .context("preview engine task")??;

        let router = engine.router();

        let (listener, bound_addr) = bind_preview_listener(&options.host, options.port).await?;
        let preview_url = format!("http://{bound_addr}/");

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .context("hosted preview server")
        });

        Ok(Self {
            engine,
            preview_url,
            shutdown_tx: Some(shutdown_tx),
            _server: server,
        })
    }

    pub fn url(&self) -> &str {
        &self.preview_url
    }

    pub fn open_browser(&self) -> Result<()> {
        open_url(&self.preview_url)
    }

    pub fn trigger_render(&self) {
        self.engine.trigger_render();
    }

    pub async fn shutdown(self) {
        let _ = tokio::task::spawn_blocking(move || drop(self)).await;
    }
}

impl Drop for HostedPreview {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn bind_preview_listener(
    host: &str,
    preferred_port: u16,
) -> Result<(tokio::net::TcpListener, SocketAddr)> {
    let preferred: SocketAddr = format!("{host}:{preferred_port}")
        .parse()
        .with_context(|| format!("Invalid host/port: {host}:{preferred_port}"))?;

    match tokio::net::TcpListener::bind(preferred).await {
        Ok(listener) => {
            let bound = listener
                .local_addr()
                .context("Read bound preview server address")?;
            return Ok((listener, bound));
        }
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {}
        Err(err) => {
            return Err(err).with_context(|| format!("Cannot bind preview server to {preferred}"));
        }
    }

    let ephemeral: SocketAddr = format!("{host}:0")
        .parse()
        .with_context(|| format!("Invalid host: {host}"))?;
    let listener = tokio::net::TcpListener::bind(ephemeral)
        .await
        .context("Cannot bind preview server to an available port")?;
    let bound = listener
        .local_addr()
        .context("Read bound preview server address")?;
    if preferred_port != 0 {
        eprintln!(
            "Port {preferred_port} in use, using port {} instead",
            bound.port()
        );
    }
    Ok((listener, bound))
}

pub fn run(
    options: ViewOptions,
    render: impl Fn(RenderRequest) -> RenderResult + Send + Sync + 'static,
) -> Result<()> {
    let render = Arc::new(render);
    let mut engine = PreviewEngine::start(options.clone().into(), render)?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to start async runtime")?;

    let host = options.host.clone();
    let start_port = options.port;
    let open_browser = options.open_browser;
    let router = engine.router();

    rt.block_on(async {
        let (listener, bound_addr) = bind_preview_listener(&host, start_port).await?;
        let serve_url = format!("http://{bound_addr}/");

        eprintln!("Preview server listening at {serve_url}");
        let watch_count = engine
            .watch_state
            .lock()
            .map(|guard| guard.watched.len())
            .unwrap_or(0);
        eprintln!("Watching {watch_count} path(s) for changes");

        if open_browser {
            open_url(&serve_url)?;
        }

        let server = axum::serve(listener, router);
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

    engine.stop();
    eprintln!("Preview server stopped.");
    Ok(())
}

fn setup_watcher(
    paths: Vec<PathBuf>,
    render_tx: std::sync::mpsc::Sender<()>,
) -> Result<WatchState> {
    let debouncer = new_debouncer(
        Duration::from_millis(300),
        move |result: DebounceEventResult| {
            let Ok(events) = result else {
                return;
            };
            if should_trigger_render(&events) {
                let _ = render_tx.send(());
            }
        },
    )
    .context("Failed to create file watcher")?;

    let mut state = WatchState {
        debouncer,
        watched: HashMap::new(),
    };
    sync_watches(&mut state, &paths)?;
    Ok(state)
}

fn should_trigger_render(events: &[notify_debouncer_mini::DebouncedEvent]) -> bool {
    !events.is_empty()
}

fn register_watch(state: &mut WatchState, path: &Path, recursive: bool) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !canonical.exists() {
        return Ok(());
    }

    let want_recursive = recursive && canonical.is_dir();
    if let Some(&already_recursive) = state.watched.get(&canonical) {
        if already_recursive || !want_recursive {
            return Ok(());
        }
        // File watches also attach their parent as NonRecursive. If that parent is
        // later registered as a scan root, upgrade so nested create/delete events
        // are delivered.
        if let Err(err) = state.debouncer.watcher().unwatch(&canonical) {
            eprintln!(
                "  watch upgrade unwatch warning for {}: {err}",
                canonical.display()
            );
        }
        state.watched.remove(&canonical);
    }

    let mode = if want_recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    state
        .debouncer
        .watcher()
        .watch(&canonical, mode)
        .with_context(|| format!("Cannot watch {}", canonical.display()))?;
    state.watched.insert(canonical.clone(), want_recursive);

    let mode_label = if want_recursive { "recursive" } else { "path" };
    eprintln!("  watch [{mode_label}] {}", canonical.display());

    // When watching a file, also watch its parent (shallow) for new sibling assets.
    // If the parent is already recursive (scan root), this is a no-op.
    if canonical.is_file() {
        if let Some(parent) = canonical.parent() {
            if !parent.as_os_str().is_empty() {
                register_watch(state, parent, false)?;
            }
        }
    }

    Ok(())
}

fn sync_watches(state: &mut WatchState, paths: &[PathBuf]) -> Result<()> {
    // Register directories first so recursive mode wins before file parents attach.
    let (dirs, files): (Vec<&PathBuf>, Vec<&PathBuf>) =
        paths.iter().partition(|path| path.is_dir());
    for path in dirs {
        register_watch(state, path, true)?;
    }
    for path in files {
        register_watch(state, path, false)?;
    }

    let stale: Vec<PathBuf> = state
        .watched
        .keys()
        .filter(|path| !path.exists())
        .cloned()
        .collect();
    for path in stale {
        let _ = state.debouncer.watcher().unwatch(&path);
        state.watched.remove(&path);
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
                            if let Err(err) = sync_watches(&mut guard, &extra_watch_paths) {
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
    let html = live::ensure_export_html(html.to_string());
    fs::write(path, html.as_bytes())
        .with_context(|| format!("Cannot export {}", path.display()))?;
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

async fn mermaid_asset_handler() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    (StatusCode::OK, headers, MERMAID_JS).into_response()
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

pub fn open_url(url: &str) -> Result<()> {
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

#[cfg(test)]
mod watch_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pagemd-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_parent_watch_can_upgrade_to_recursive_scan_root() {
        let root = temp_dir("watch-upgrade");
        let file = root.join("a.md");
        fs::write(&file, "# A\n").unwrap();

        let (tx, _rx) = std::sync::mpsc::channel();
        // Register the file first (same order as the old bug: parent becomes NonRecursive).
        let mut state = setup_watcher(vec![file.clone()], tx).unwrap();
        let root = root.canonicalize().unwrap();
        assert_eq!(state.watched.get(&root), Some(&false));

        sync_watches(&mut state, &[root.clone(), file]).unwrap();
        assert_eq!(
            state.watched.get(&root),
            Some(&true),
            "scan root must upgrade to recursive so nested create/delete events arrive"
        );

        fs::remove_dir_all(root).unwrap();
    }
}
