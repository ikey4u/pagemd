use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as AsyncRwLock;

use super::cdp::CdpSession;
use super::snap;
use super::tools;

#[derive(Clone, Debug, Default)]
pub struct SessionMdSnapshot {
    pub markdown: String,
    pub page_url: String,
    pub title: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SessionMdMeta {
    page_url: String,
    title: String,
}

pub struct SessionMarkdown {
    workspace: PathBuf,
    /// Stable id derived from normalized page URL (`uuid` v5).
    active_key: RwLock<Option<String>>,
    inner: Arc<AsyncRwLock<SessionMdSnapshot>>,
}

impl SessionMarkdown {
    pub fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            active_key: RwLock::new(None),
            inner: Arc::new(AsyncRwLock::new(SessionMdSnapshot::default())),
        }
    }

    /// Bind reads/writes to the session slot for `session`'s current URL.
    /// Returns `true` when the active slot changed (e.g. after navigation).
    pub async fn bind_to_page(&self, session: &CdpSession) -> Result<bool> {
        let url = session.current_url().await.unwrap_or_default();
        self.bind_to_url(&url).await
    }

    /// Bind to a specific page URL (each URL gets its own on-disk session).
    pub async fn bind_to_url(&self, url: &str) -> Result<bool> {
        if should_skip_binding(url) {
            return Ok(false);
        }
        let key = session_key(url);
        let switched = {
            let mut active = self.active_key.write().unwrap();
            if active.as_deref() == Some(key.as_str()) {
                false
            } else {
                *active = Some(key);
                true
            }
        };
        if switched {
            *self.inner.write().await = SessionMdSnapshot::default();
        }
        self.ensure_loaded_from_disk().await?;
        Ok(switched)
    }

    pub fn active_page_url(&self) -> Option<String> {
        let key = self.active_key.read().unwrap().clone()?;
        self.read_meta_for_key(&key).map(|meta| meta.page_url)
    }

    pub fn file_path(&self) -> PathBuf {
        self.session_md_path_for_active()
    }

    pub fn original_md_path(&self) -> PathBuf {
        self.session_file_for_active("original.md")
    }

    pub fn original_html_path(&self) -> PathBuf {
        self.session_file_for_active("original.html")
    }

    pub fn has_original_baseline(&self) -> bool {
        self.original_md_path().is_file()
    }

    pub fn load_original_markdown(&self) -> Result<Option<String>> {
        let path = self.original_md_path();
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(text))
    }

    /// Snapshot the **visible** page before sandbox mutations (for /pmd --original).
    pub async fn capture_original_baseline(
        &self,
        session: &CdpSession,
        preferred_url: Option<&str>,
    ) -> Result<()> {
        self.bind_to_page(session).await?;
        if self.original_md_path().is_file() {
            return Ok(());
        }
        let html = snap::capture_body_html(session, preferred_url).await?;
        let md = snap::html_to_markdown(&html)?;
        let html_path = self.original_html_path();
        let md_path = self.original_md_path();
        if let Some(parent) = html_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&html_path, &html).with_context(|| format!("write {}", html_path.display()))?;
        std::fs::write(&md_path, &md).with_context(|| format!("write {}", md_path.display()))?;
        Ok(())
    }

    pub async fn capture_from_sandbox(&self, session: &CdpSession) -> Result<SessionMdSnapshot> {
        self.bind_to_page(session).await?;
        let markdown = super::sandbox::markdown_text(session, 500_000).await?;
        let page_url = session.current_url().await.unwrap_or_default();
        let title = session.current_title().await.unwrap_or_default();
        let snap = SessionMdSnapshot {
            markdown,
            page_url,
            title,
        };
        *self.inner.write().await = snap.clone();
        self.persist(&snap)?;
        Ok(snap)
    }

    pub async fn capture_from_live(
        &self,
        session: &CdpSession,
        preferred_url: Option<&str>,
    ) -> Result<SessionMdSnapshot> {
        self.bind_to_page(session).await?;
        let markdown = tools::markdown_text(session, preferred_url, 500_000).await?;
        let page_url = session.current_url().await.unwrap_or_default();
        let title = session.current_title().await.unwrap_or_default();
        let snap = SessionMdSnapshot {
            markdown,
            page_url,
            title,
        };
        *self.inner.write().await = snap.clone();
        self.persist(&snap)?;
        Ok(snap)
    }

    pub async fn load_from_disk(&self) -> Result<Option<SessionMdSnapshot>> {
        let path = self.session_md_path_for_active();
        if !path.is_file() {
            return Ok(None);
        }
        let markdown = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        if markdown.trim().is_empty() {
            return Ok(None);
        }
        let meta = self
            .read_meta_for_active()
            .unwrap_or_default();
        let snap = SessionMdSnapshot {
            markdown,
            page_url: meta.page_url,
            title: meta.title,
        };
        *self.inner.write().await = snap.clone();
        Ok(Some(snap))
    }

    pub async fn snapshot(&self) -> SessionMdSnapshot {
        self.inner.read().await.clone()
    }

    pub fn write_disk(&self, markdown: &str) -> Result<PathBuf> {
        let path = self.session_md_path_for_active();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, markdown).with_context(|| format!("write {}", path.display()))?;
        Ok(path)
    }

    fn session_md_path_for_active(&self) -> PathBuf {
        self.session_file_for_active("session.md")
    }

    fn session_file_for_active(&self, name: &str) -> PathBuf {
        let key = self
            .active_key
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "_unbound".to_string());
        session_dir(&self.workspace, &key).join(name)
    }

    fn meta_path_for_key(&self, key: &str) -> PathBuf {
        session_dir(&self.workspace, key).join("meta.json")
    }

    fn read_meta_for_active(&self) -> Option<SessionMdMeta> {
        let key = self.active_key.read().unwrap().clone()?;
        self.read_meta_for_key(&key)
    }

    fn read_meta_for_key(&self, key: &str) -> Option<SessionMdMeta> {
        let path = self.meta_path_for_key(key);
        let text = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&text).ok()
    }

    async fn ensure_loaded_from_disk(&self) -> Result<()> {
        if !self.inner.read().await.markdown.trim().is_empty() {
            return Ok(());
        }
        let _ = self.load_from_disk().await?;
        Ok(())
    }

    fn persist(&self, snap: &SessionMdSnapshot) -> Result<()> {
        self.write_disk(&snap.markdown)?;
        self.write_meta(&snap.page_url, &snap.title)
    }

    fn write_meta(&self, page_url: &str, title: &str) -> Result<()> {
        let key = self
            .active_key
            .read()
            .unwrap()
            .clone()
            .context("session not bound to a page URL")?;
        let path = self.meta_path_for_key(&key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let meta = SessionMdMeta {
            page_url: page_url.to_string(),
            title: title.to_string(),
        };
        let text = serde_json::to_string_pretty(&meta)?;
        std::fs::write(&path, format!("{text}\n")).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}

pub fn session_key(url: &str) -> String {
    let normalized = normalize_page_url(url);
    if normalized.is_empty() {
        return "_unbound".to_string();
    }
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, normalized.as_bytes()).to_string()
}

pub fn session_dir(workspace: &Path, key: &str) -> PathBuf {
    workspace.join(".pagemd").join("sessions").join(key)
}

fn should_skip_binding(url: &str) -> bool {
    let url = url.trim();
    url.is_empty() || url == "about:blank"
}

fn normalize_page_url(url: &str) -> String {
    url.trim()
        .split_once('#')
        .map(|(base, _)| base)
        .unwrap_or(url.trim())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace(name: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pagemd-session-{name}-{id}"))
    }

    #[test]
    fn session_key_is_stable_for_same_url() {
        let a = session_key("https://example.com/docs?page=1");
        let b = session_key("https://example.com/docs?page=1");
        assert_eq!(a, b);
    }

    #[test]
    fn session_key_differs_for_different_urls() {
        assert_ne!(
            session_key("https://example.com/a"),
            session_key("https://example.com/b")
        );
    }

    #[test]
    fn normalize_strips_fragment() {
        assert_eq!(
            session_key("https://example.com/a#section"),
            session_key("https://example.com/a")
        );
    }

    #[tokio::test]
    async fn per_url_sessions_do_not_clobber_each_other() {
        let workspace = temp_workspace("isolated");
        let session_md = SessionMarkdown::new(&workspace);

        session_md.bind_to_url("https://example.com/a").await.unwrap();
        session_md
            .persist(&SessionMdSnapshot {
                markdown: "# page a\n".to_string(),
                page_url: "https://example.com/a".to_string(),
                title: "A".to_string(),
            })
            .unwrap();

        session_md.bind_to_url("https://example.com/b").await.unwrap();
        session_md
            .persist(&SessionMdSnapshot {
                markdown: "# page b\n".to_string(),
                page_url: "https://example.com/b".to_string(),
                title: "B".to_string(),
            })
            .unwrap();

        session_md.bind_to_url("https://example.com/a").await.unwrap();
        assert_eq!(session_md.snapshot().await.markdown, "# page a\n");

        session_md.bind_to_url("https://example.com/b").await.unwrap();
        assert_eq!(session_md.snapshot().await.markdown, "# page b\n");

        let key_a = session_key("https://example.com/a");
        let key_b = session_key("https://example.com/b");
        assert!(session_dir(&workspace, &key_a).join("session.md").is_file());
        assert!(session_dir(&workspace, &key_b).join("session.md").is_file());

        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn bind_is_noop_for_same_url_with_fragment() {
        let workspace = temp_workspace("fragment");
        let session_md = SessionMarkdown::new(&workspace);
        session_md.bind_to_url("https://example.com/a").await.unwrap();
        session_md
            .persist(&SessionMdSnapshot {
                markdown: "# same\n".to_string(),
                page_url: "https://example.com/a".to_string(),
                title: "A".to_string(),
            })
            .unwrap();

        assert!(!session_md
            .bind_to_url("https://example.com/a#top")
            .await
            .unwrap());
        assert_eq!(session_md.snapshot().await.markdown, "# same\n");
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[tokio::test]
    async fn fresh_instance_loads_existing_url_session() {
        let workspace = temp_workspace("reload");
        {
            let session_md = SessionMarkdown::new(&workspace);
            session_md.bind_to_url("https://example.com/x").await.unwrap();
            session_md
                .persist(&SessionMdSnapshot {
                    markdown: "# doc\n".to_string(),
                    page_url: "https://example.com/x".to_string(),
                    title: "Title".to_string(),
                })
                .unwrap();
        }

        let fresh = SessionMarkdown::new(&workspace);
        fresh.bind_to_url("https://example.com/x").await.unwrap();
        let loaded = fresh.load_from_disk().await.unwrap().unwrap();
        assert_eq!(loaded.page_url, "https://example.com/x");
        assert_eq!(loaded.title, "Title");
        assert_eq!(loaded.markdown, "# doc\n");
        std::fs::remove_dir_all(workspace).unwrap();
    }
}
