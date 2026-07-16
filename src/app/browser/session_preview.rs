use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::app::preview::error::{build_preview_error_html, preview_html_opts};
use crate::app::preview::{
    collect_initial_watch_paths, collect_render_watch_paths, HostedPreview, HostedPreviewOptions,
    RenderRequest, RenderResult,
};
use crate::core::{
    export_with_resources, prepare_resources, resolve_inputs, ConvertOptions, HtmlExportOptions,
    OutputFormat, RenderResources,
};

use super::session_md::SessionMarkdown;

struct SessionRenderContext {
    convert_opts: ConvertOptions,
    html_opts: HtmlExportOptions,
    resources: RenderResources,
    session_path: PathBuf,
}

pub struct SessionPreview {
    inner: HostedPreview,
    session_path: PathBuf,
}

impl SessionPreview {
    pub async fn ensure<'a>(
        slot: &'a mut Option<SessionPreview>,
        session_md: &SessionMarkdown,
    ) -> Result<&'a SessionPreview> {
        Self::ensure_at_path(slot, session_md.file_path(), "PageMD session").await
    }

    pub async fn ensure_at_path<'a>(
        slot: &'a mut Option<SessionPreview>,
        session_path: PathBuf,
        title: &str,
    ) -> Result<&'a SessionPreview> {
        let needs_recreate = slot
            .as_ref()
            .is_some_and(|existing| existing.session_path != session_path);
        if needs_recreate {
            Self::shutdown(slot.take()).await;
        }

        if slot.is_none() {
            if let Some(parent) = session_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            if !session_path.is_file() {
                fs::write(&session_path, "")
                    .with_context(|| format!("create {}", session_path.display()))?;
            }

            let source = fs::read_to_string(&session_path)
                .with_context(|| format!("read {}", session_path.display()))?;
            let inputs = vec![session_path.clone()];
            let watch_paths =
                collect_initial_watch_paths(&inputs, &[(session_path.clone(), source)]);

            let convert_opts = ConvertOptions {
                inputs: inputs.clone(),
                directories: Vec::new(),
                excludes: Vec::new(),
                title: Some(title.to_string()),
                icon: None,
                math_font_size: 1.0,
                katex_fonts: None,
                output_format: OutputFormat::Html,
                client_mermaid: true,
            };
            let resources = prepare_resources(&convert_opts)?;
            let html_opts = preview_html_opts();
            let ctx = Arc::new(SessionRenderContext {
                convert_opts,
                html_opts,
                resources,
                session_path: session_path.clone(),
            });

            let render_ctx = Arc::clone(&ctx);
            let hosted = HostedPreview::start(
                HostedPreviewOptions {
                    host: "127.0.0.1".to_string(),
                    port: 0,
                    inputs,
                    watch_paths,
                    export_path: None,
                },
                move |_request: RenderRequest| render_session(&render_ctx),
            )
            .await?;

            *slot = Some(SessionPreview {
                inner: hosted,
                session_path,
            });
        }
        Ok(slot.as_ref().expect("session preview"))
    }

    pub fn url(&self) -> &str {
        self.inner.url()
    }

    pub fn open_browser(&self) -> Result<()> {
        self.inner.open_browser()
    }

    pub fn trigger_render(&self) {
        self.inner.trigger_render();
    }

    pub async fn shutdown(preview: Option<Self>) {
        if let Some(preview) = preview {
            preview.inner.shutdown().await;
        }
    }
}

fn render_session(ctx: &SessionRenderContext) -> RenderResult {
    match export_with_resources(
        &ctx.convert_opts,
        &ctx.html_opts,
        &ctx.resources,
        Some(ctx.session_path.as_path()),
    ) {
        Ok(document) => {
            let extra_watch_paths = match resolve_inputs(&ctx.convert_opts) {
                Ok(resolved) => collect_render_watch_paths(&resolved.files, &resolved.directories),
                Err(err) => {
                    eprintln!("Watch path refresh warning: {err:#}");
                    Vec::new()
                }
            };
            RenderResult::Ok {
                html: document.html,
                extra_watch_paths,
            }
        }
        Err(err) => {
            eprintln!("Session preview render error: {err:#}");
            RenderResult::Err {
                html: build_preview_error_html(&err),
            }
        }
    }
}
