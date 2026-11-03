//! Native OpenAPI rendering with host-owned source readers.
use crate::{MarkdownNode, MarkdownParseContext, MarkdownPlugin};
use gpui_kit::{component::ActiveTheme as _, *};
use pagemd_parser::mdast::Node;
use std::{
    collections::HashMap, future::Future, path::PathBuf, pin::Pin, sync::Arc,
};

mod native {
    include!("../../openapi/src/render_native.rs");
}
pub use native::render_document as render_openapi_document;
pub type OpenApiReader = Arc<
    dyn Fn(
            url::Url,
        )
            -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
>;
type Notify = Arc<dyn Fn(&mut App) + Send + Sync>;

#[derive(Clone, Default)]
pub struct OpenApiPlugin {
    namespace: String,
    base: Option<url::Url>,
    reader: Option<OpenApiReader>,
    notify: Option<Notify>,
}
impl OpenApiPlugin {
    pub fn new(
        namespace: impl Into<String>,
        base: url::Url,
        reader: OpenApiReader,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            base: Some(base),
            reader: Some(reader),
            notify: None,
        }
    }
    pub fn local(folder: PathBuf) -> Self {
        let base =
            url::Url::from_file_path(folder.join("__markdown__.md")).ok();
        let namespace = format!("local:{folder:?}");
        let reader: OpenApiReader = Arc::new(move |url| {
            let folder = folder.clone();
            Box::pin(
                async move { pagemd_openapi::read_blocking(&url, &folder) },
            )
        });
        Self {
            namespace,
            base,
            reader: Some(reader),
            notify: None,
        }
    }
    pub fn on_loaded(
        mut self,
        notify: impl Fn(&mut App) + Send + Sync + 'static,
    ) -> Self {
        self.notify = Some(Arc::new(notify));
        self
    }
}
#[derive(Clone)]
struct Block {
    body: String,
    meta: String,
    pending: bool,
}
#[derive(Default)]
struct Cache(HashMap<String, Entity<Card>>);
impl Global for Cache {}
impl MarkdownPlugin for OpenApiPlugin {
    fn name(&self) -> &str {
        "pagemd-openapi"
    }
    fn is_block(&self) -> bool {
        true
    }
    fn parse(
        &self,
        node: &Node,
        cx: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let Node::Code(code) = node else { return None };
        if !code
            .lang
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("openapi"))
        {
            return None;
        }
        let source = cx.node_source(node).unwrap_or_default();
        Some(
            MarkdownNode::new(
                self.name().to_owned(),
                Block {
                    body: code.value.clone(),
                    meta: code.meta.clone().unwrap_or_default(),
                    pending: !pagemd_openapi::closed_fence(source),
                },
            )
            .text(code.value.clone())
            .markdown(source.to_owned()),
        )
    }
    fn render(
        &self,
        node: &MarkdownNode,
        _: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let block = node.data::<Block>().unwrap();
        if block.pending {
            return div()
                .child("Receiving API specification…")
                .into_any_element();
        }
        if !cx.has_global::<Cache>() {
            cx.set_global(Cache::default());
        }
        let key = format!(
            "{}:{:?}:{}:{}",
            self.namespace, self.base, block.meta, block.body
        );
        if let Some(card) = cx.global::<Cache>().0.get(&key) {
            return card.clone().into_any_element();
        }
        let block = block.clone();
        let base = self.base.clone();
        let reader = self.reader.clone();
        let notify = self.notify.clone();
        let card = cx.new(|cx| {
            let task = cx.background_spawn(async move {
                let source =
                    pagemd_openapi::source_attribute(&block.meta, &block.body)?;
                if let Some(base) = base {
                    let base = source
                        .as_ref()
                        .map(|src| base.join(src))
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .unwrap_or(base);
                    let reader =
                        reader.ok_or("OpenAPI source reader is unavailable")?;
                    let text = if source.is_some() {
                        reader(base.clone()).await?
                    } else {
                        block.body
                    };
                    pagemd_openapi::parse_loaded(&text, &base, |url| {
                        reader(url)
                    })
                    .await
                } else if let Some(source) = source {
                    let base = url::Url::parse(&source).map_err(|_| "OpenAPI file source needs a document folder")?;
                    if base.scheme() != "https" { return Err("OpenAPI sources require a document path or public HTTPS URL".into()); }
                    let read = |url: url::Url| async move {
                        if url.scheme() != "https" { return Err("HTTPS OpenAPI documents require HTTPS references".into()); }
                        pagemd_openapi::read_blocking(&url, std::path::Path::new("."))
                    };
                    let text = read(base.clone()).await?;
                    pagemd_openapi::parse_loaded(&text, &base, read).await
                } else {
                    pagemd_openapi::parse(&block.body)
                }
            });
            let task = cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this: &mut Card, cx| {
                    this.result = Some(result);
                    if let Some(notify) = notify {
                        notify(cx);
                    }
                    cx.notify();
                });
            });
            Card {
                result: None,
                _task: task,
            }
        });
        if cx.global::<Cache>().0.len() >= 48 {
            cx.global_mut::<Cache>().0.clear();
        }
        cx.global_mut::<Cache>().0.insert(key, card.clone());
        card.into_any_element()
    }
}
struct Card {
    result: Option<Result<pagemd_openapi::Document, String>>,
    _task: Task<()>,
}
impl Render for Card {
    fn render(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.result {
            Some(Ok(document)) => native::render_document(document, cx),
            Some(Err(error)) => div()
                .text_color(cx.theme().muted_foreground)
                .whitespace_normal()
                .child(format!("Couldn’t render API reference: {error}"))
                .into_any_element(),
            None => div()
                .text_color(cx.theme().muted_foreground)
                .child("Loading API reference…")
                .into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn plugin_preserves_source_ranges_and_waits_for_a_closed_fence() {
        for closed in [false, true] {
            let source = format!(
                "```openapi src=\"./api.yaml\"\n{}",
                if closed { "```" } else { "" }
            );
            let document = pagemd_parser::parse(
                &source,
                &pagemd_parser::ParseOptions::gfm(),
            )
            .unwrap();
            let Node::Root(root) = document.ast() else {
                panic!()
            };
            let plugin = OpenApiPlugin::default();
            let node = plugin
                .parse(
                    &root.children[0],
                    &MarkdownParseContext { source: &source },
                )
                .unwrap();
            assert_eq!(node.as_markdown(), source.trim_end());
            assert_eq!(node.data::<Block>().unwrap().pending, !closed);
        }
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use core::prelude::v1::test;
    use gpui_kit::component::{Root, Theme, ThemeMode};
    struct Host(Entity<crate::MarkdownViewState>);
    impl Render for Host {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> impl IntoElement {
            div().w(px(500.)).child(crate::MarkdownView::new(&self.0))
        }
    }
    #[gpui_kit::test]
    fn native_openapi_is_a_builtin_block_and_finishes_background_parsing(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let (_, cx) = cx.add_window_view(|window, cx| {
            Theme::change(ThemeMode::Light, Some(window), cx);
            let document = crate::render("```openapi\nopenapi: 3.1.0\ninfo: {title: Hidden, version: '1'}\npaths:\n  /pets:\n    get:\n      responses: {'200': {description: OK}}\n```", cx);
            Root::new(cx.new(|_| Host(document)), window, cx)
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            let cache = cx.global::<Cache>();
            assert_eq!(cache.0.len(), 1);
            let card = cache.0.values().next().unwrap().read(cx);
            let document = card
                .result
                .as_ref()
                .expect("background result")
                .as_ref()
                .unwrap();
            assert_eq!(document.operations[0].method, "GET");
            assert_eq!(document.operations[0].path, "/pets");
        });
    }
}
