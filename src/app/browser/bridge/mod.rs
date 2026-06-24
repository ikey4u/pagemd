use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use super::cdp::CdpSession;
use super::runtime::BrowserRuntime;
use super::sandbox;
use super::session_md::SessionMarkdown;
use super::snap::format_snap;
use super::tools::{self, format_eval_result, parse_max_chars, truncate};
use super::undo::{DomTarget, UndoStack};

const DEFAULT_MAX_CHARS: usize = 50_000;
const CDP_LOCK_WAIT: Duration = Duration::from_secs(3);

#[derive(Clone)]
struct BridgeState {
    session: CdpSession,
    undo: Arc<Mutex<UndoStack>>,
    session_md: Arc<SessionMarkdown>,
    sandbox_enabled: Arc<AtomicBool>,
    cdp_lock: Arc<tokio::sync::Mutex<()>>,
    token: String,
    preferred_url: Option<String>,
}

async fn acquire_cdp(state: &BridgeState) -> Result<tokio::sync::MutexGuard<'_, ()>> {
    match tokio::time::timeout(CDP_LOCK_WAIT, state.cdp_lock.lock()).await {
        Ok(guard) => Ok(guard),
        Err(_) => Err(anyhow::anyhow!(
            "bridge busy: another CDP operation is still running. Retry the MCP tool; \
             do not curl runtime.json or kill the pagemd process."
        )),
    }
}

fn dom_target(state: &BridgeState) -> DomTarget {
    if sandbox::is_enabled(&state.sandbox_enabled) {
        DomTarget::Sandbox
    } else {
        DomTarget::Live
    }
}

fn disable_sandbox(state: &BridgeState) {
    state.sandbox_enabled.store(false, Ordering::SeqCst);
}

pub struct BrowserBridge {
    workspace: std::path::PathBuf,
    _server: JoinHandle<Result<()>>,
    shutdown: Option<oneshot::Sender<()>>,
    pub runtime: BrowserRuntime,
}

impl BrowserBridge {
    pub async fn start(
        workspace: &std::path::Path,
        cdp_port: u16,
        export_dir: &std::path::Path,
        session: CdpSession,
        undo: Arc<Mutex<UndoStack>>,
        session_md: Arc<SessionMarkdown>,
        sandbox_enabled: Arc<AtomicBool>,
        preferred_url: Option<String>,
    ) -> Result<Self> {
        let token = uuid::Uuid::new_v4().to_string();
        let state = BridgeState {
            session,
            undo,
            session_md,
            sandbox_enabled,
            cdp_lock: Arc::new(tokio::sync::Mutex::new(())),
            token: token.clone(),
            preferred_url,
        };

        let app = Router::new()
            .route("/health", get(health))
            .route("/v1/snap", post(snap))
            .route("/v1/html", post(html))
            .route("/v1/markdown", post(markdown))
            .route("/v1/markdown/save", post(markdown_save))
            .route("/v1/markdown/session", get(markdown_session))
            .route("/v1/markdown/original", get(markdown_original))
            .route("/v1/sandbox/begin", post(sandbox_begin))
            .route("/v1/eval", post(eval))
            .route("/v1/clean", post(clean_dom))
            .route("/v1/goto", post(goto))
            .route("/v1/reload", post(reload))
            .route("/v1/undo", post(undo_step))
            .route("/v1/url", get(url))
            .route("/v1/title", get(title))
            .route("/v1/undo-depth", get(undo_depth))
            .with_state(Arc::new(state));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("bind browser bridge")?;
        let addr = listener.local_addr().context("bridge local addr")?;
        let bridge_url = format!("http://{addr}");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .context("browser bridge server")
        });

        let runtime = BrowserRuntime {
            bridge_url: bridge_url.clone(),
            token,
            cdp_port,
            pid: std::process::id(),
            export_dir: export_dir.to_string_lossy().into_owned(),
        };
        runtime.write(workspace)?;

        Ok(Self {
            workspace: workspace.to_path_buf(),
            _server: server,
            shutdown: Some(shutdown_tx),
            runtime,
        })
    }
}

impl Drop for BrowserBridge {
    fn drop(&mut self) {
        BrowserRuntime::remove(&self.workspace);
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {token}"))
}

fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "missing or invalid bearer token").into_response()
}

async fn health(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    Json(json!({
        "ok": true,
        "pid": std::process::id(),
        "sandbox": sandbox::is_enabled(&state.sandbox_enabled),
    }))
    .into_response()
}

async fn snap(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let _cdp = match acquire_cdp(&state).await {
        Ok(g) => g,
        Err(err) => return tool_error(err),
    };
    let result = if dom_target(&state) == DomTarget::Sandbox {
        sandbox::capture_page(&state.session)
            .await
            .map(|value| format_snap(&value))
    } else {
        let hint = state.preferred_url.as_deref();
        tools::snap_text(&state.session, hint).await
    };
    match result {
        Ok(text) => Json(json!({ "text": text })).into_response(),
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct HtmlRequest {
    body_only: Option<bool>,
    max_chars: Option<u64>,
}

async fn html(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(req): Json<HtmlRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let max_chars = match parse_max_chars(req.max_chars, DEFAULT_MAX_CHARS) {
        Ok(v) => v,
        Err(err) => return tool_error(err),
    };
    let body_only = req.body_only.unwrap_or(false);
    let result = if dom_target(&state) == DomTarget::Sandbox {
        if body_only {
            sandbox::capture_body_html(&state.session).await
        } else {
            sandbox::capture_body_html(&state.session)
                .await
                .map(|body| format!("<!DOCTYPE html><html><head></head><body>{body}</body></html>"))
        }
    } else {
        let hint = state.preferred_url.as_deref();
        tools::html_text(&state.session, hint, body_only, max_chars).await
    };
    match result {
        Ok(text) => {
            let text = if dom_target(&state) == DomTarget::Sandbox {
                truncate(text, max_chars)
            } else {
                text
            };
            Json(json!({ "text": text })).into_response()
        }
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct MarkdownRequest {
    max_chars: Option<u64>,
}

async fn markdown(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(req): Json<MarkdownRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let max_chars = match parse_max_chars(req.max_chars, DEFAULT_MAX_CHARS) {
        Ok(v) => v,
        Err(err) => return tool_error(err),
    };
    let result = if dom_target(&state) == DomTarget::Sandbox {
        sandbox::markdown_text(&state.session, max_chars).await
    } else {
        let hint = state.preferred_url.as_deref();
        tools::markdown_text(&state.session, hint, max_chars).await
    };
    match result {
        Ok(text) => Json(json!({ "text": text })).into_response(),
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct EvalRequest {
    expression: String,
    record_undo: Option<bool>,
}

async fn eval(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(req): Json<EvalRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    if req.expression.trim().is_empty() {
        return tool_error(anyhow::anyhow!("expression is required"));
    }
    let _cdp = match acquire_cdp(&state).await {
        Ok(g) => g,
        Err(err) => return tool_error(err),
    };
    let target = dom_target(&state);
    let record_undo = req.record_undo.unwrap_or(false);
    if record_undo {
        let mut undo = state.undo.lock().await;
        if let Err(err) = undo.push_before_mutate(&state.session, target).await {
            return tool_error(err);
        }
    }
    let eval_result = if target == DomTarget::Sandbox {
        sandbox::eval_expression(&state.session, &req.expression).await
    } else {
        state.session.evaluate(&req.expression, false).await
    };
    match eval_result {
        Ok(value) => {
            let undo_depth = state.undo.lock().await.len();
            Json(json!({
                "result": value,
                "text": format_eval_result(&value),
                "undo_depth": undo_depth,
                "sandbox": target == DomTarget::Sandbox,
            }))
            .into_response()
        }
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct CleanRequest {
    extra_selectors: Option<Vec<String>>,
}

async fn clean_dom(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(req): Json<CleanRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let _cdp = match acquire_cdp(&state).await {
        Ok(g) => g,
        Err(err) => return tool_error(err),
    };
    let extra = req.extra_selectors.unwrap_or_default();
    let target = dom_target(&state);
    let mut undo_skipped = false;
    {
        let mut undo = state.undo.lock().await;
        if let Err(err) = undo.push_before_mutate(&state.session, target).await {
            let msg = err.to_string();
            if msg.contains("too slow") || msg.contains("too large") {
                undo_skipped = true;
            } else {
                return tool_error(err);
            }
        }
    }
    let result = if target == DomTarget::Sandbox {
        sandbox::run_clean(&state.session, &extra).await
    } else {
        tools::run_clean_dom(&state.session, &extra).await
    };
    match result {
        Ok(value) => {
            let undo_depth = state.undo.lock().await.len();
            Json(json!({
                "result": value,
                "text": format_eval_result(&value),
                "undo_depth": undo_depth,
                "sandbox": target == DomTarget::Sandbox,
                "undo_skipped": undo_skipped,
            }))
            .into_response()
        }
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct GotoRequest {
    url: String,
}

async fn goto(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(req): Json<GotoRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    if req.url.trim().is_empty() {
        return tool_error(anyhow::anyhow!("url is required"));
    }
    disable_sandbox(&state);
    let mut undo = state.undo.lock().await;
    undo.reset();
    match state.session.navigate(&req.url).await {
        Ok(()) => {
            if let Err(err) = undo.capture_baseline(&state.session, DomTarget::Live).await {
                return tool_error(err);
            }
            drop(undo);
            let _ = state.session_md.bind_to_page(&state.session).await;
            Json(json!({ "ok": true, "url": req.url })).into_response()
        }
        Err(err) => tool_error(err),
    }
}

async fn reload(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    disable_sandbox(&state);
    let mut undo = state.undo.lock().await;
    undo.reset();
    match state.session.reload().await {
        Ok(()) => {
            if let Err(err) = undo.capture_baseline(&state.session, DomTarget::Live).await {
                return tool_error(err);
            }
            drop(undo);
            let _ = state.session_md.bind_to_page(&state.session).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct UndoRequest {
    all: Option<bool>,
}

async fn undo_step(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(req): Json<UndoRequest>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let target = dom_target(&state);
    let mut undo = state.undo.lock().await;
    let result = if req.all.unwrap_or(false) {
        undo.undo_all(&state.session, target).await
    } else {
        undo.undo_one(&state.session, target).await
    };
    match result {
        Ok(changed) => Json(json!({
            "changed": changed,
            "undo_depth": undo.len(),
            "sandbox": target == DomTarget::Sandbox,
        }))
        .into_response(),
        Err(err) => tool_error(err),
    }
}

async fn url(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    match state.session.current_url().await {
        Ok(url) => Json(json!({ "url": url })).into_response(),
        Err(err) => tool_error(err),
    }
}

async fn title(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    match state.session.current_title().await {
        Ok(title) => Json(json!({ "title": title })).into_response(),
        Err(err) => tool_error(err),
    }
}

async fn undo_depth(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let undo = state.undo.lock().await;
    Json(json!({
        "undo_depth": undo.len(),
        "sandbox": sandbox::is_enabled(&state.sandbox_enabled),
    }))
    .into_response()
}

async fn sandbox_begin(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let mut undo = state.undo.lock().await;
    match sandbox::begin(
        &state.session,
        &state.session_md,
        &state.sandbox_enabled,
        &mut undo,
    )
    .await
    {
        Ok(info) => Json(json!({
            "ok": true,
            "info": info,
            "text": "Sandbox active — visible tab unchanged; clean/eval operate on hidden DOM copy.",
        }))
        .into_response(),
        Err(err) => tool_error(err),
    }
}

async fn markdown_save(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let save_result = if dom_target(&state) == DomTarget::Sandbox {
        state.session_md.capture_from_sandbox(&state.session).await
    } else {
        let hint = state.preferred_url.as_deref();
        state
            .session_md
            .capture_from_live(&state.session, hint)
            .await
    };
    match save_result {
        Ok(snap) => {
            let path = state.session_md.file_path();
            let chars = snap.markdown.chars().count();
            Json(json!({
                "path": path.to_string_lossy(),
                "page_url": snap.page_url,
                "title": snap.title,
                "chars": chars,
                "text": truncate(snap.markdown, DEFAULT_MAX_CHARS),
                "sandbox": dom_target(&state) == DomTarget::Sandbox,
            }))
            .into_response()
        }
        Err(err) => tool_error(err),
    }
}

#[derive(Deserialize)]
struct SessionMdQuery {
    max_chars: Option<u64>,
}

async fn markdown_session(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<SessionMdQuery>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let max_chars = match parse_max_chars(query.max_chars, DEFAULT_MAX_CHARS) {
        Ok(v) => v,
        Err(err) => return tool_error(err),
    };
    if state.session_md.bind_to_page(&state.session).await.is_err() {
        return tool_error(anyhow::anyhow!(
            "failed to bind session Markdown to current page"
        ));
    }
    let snap = state.session_md.snapshot().await;
    if snap.markdown.trim().is_empty() {
        if let Ok(Some(loaded)) = state.session_md.load_from_disk().await {
            let path = state.session_md.file_path();
            return Json(json!({
                "path": path.to_string_lossy(),
                "page_url": loaded.page_url,
                "title": loaded.title,
                "chars": loaded.markdown.chars().count(),
                "text": truncate(loaded.markdown, max_chars),
            }))
            .into_response();
        }
        return tool_error(anyhow::anyhow!(
            "no session Markdown yet; call browser_save_markdown after optimizing the DOM (sandbox or live)"
        ));
    }
    let path = state.session_md.file_path();
    Json(json!({
        "path": path.to_string_lossy(),
        "page_url": snap.page_url,
        "title": snap.title,
        "chars": snap.markdown.chars().count(),
        "text": truncate(snap.markdown, max_chars),
    }))
    .into_response()
}

async fn markdown_original(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<SessionMdQuery>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return unauthorized();
    }
    let max_chars = match parse_max_chars(query.max_chars, DEFAULT_MAX_CHARS) {
        Ok(v) => v,
        Err(err) => return tool_error(err),
    };
    let _ = state.session_md.bind_to_page(&state.session).await;
    match state.session_md.load_original_markdown() {
        Ok(Some(md)) => {
            let path = state.session_md.original_md_path();
            Json(json!({
                "path": path.to_string_lossy(),
                "chars": md.chars().count(),
                "text": truncate(md, max_chars),
            }))
            .into_response()
        }
        Ok(None) => tool_error(anyhow::anyhow!(
            "no original baseline yet; run /pretty or browser_begin_sandbox first"
        )),
        Err(err) => tool_error(err),
    }
}

fn tool_error(err: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({ "error": err.to_string() })),
    )
        .into_response()
}
