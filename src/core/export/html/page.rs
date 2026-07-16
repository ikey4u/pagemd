use std::path::{Path, PathBuf};

use crate::core::export::html::favicon::favicon_link_tag;
use crate::core::export::html::nav_tree::{
    build_nav_tree, common_path_prefix, nav_entries_have_tree, relativize_to_root,
    render_flat_nav_html, render_nav_tree_html,
};
use crate::core::model::RenderedSection;
use crate::core::util::{html_escape, script_escape};

use super::styles::CSS;

pub fn section_label(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn build_outline_nav(body_sections: &[RenderedSection]) -> String {
    body_sections
        .iter()
        .enumerate()
        .map(|(section_index, section)| {
            let doc_id = format!("doc-{}", section_index + 1);
            let active = if section_index == 0 { " is-active" } else { "" };
            let items = if section.outline.is_empty() {
                "<div class=\"doc-outline-empty\">No headings</div>\n".to_string()
            } else {
                section
                    .outline
                    .iter()
                    .map(|heading| {
                        let depth = heading.level.saturating_sub(1).min(5);
                        format!(
                            "<a class=\"doc-outline-link depth-{depth}\" href=\"#{}\" data-heading-target=\"{}\" title=\"{}\">{}</a>\n",
                            html_escape(&heading.id),
                            html_escape(&heading.id),
                            html_escape(&heading.text),
                            html_escape(&heading.text)
                        )
                    })
                    .collect()
            };
            format!(
                "<nav class=\"doc-outline-list{active}\" data-outline-for=\"{doc_id}\">\n{items}</nav>\n"
            )
        })
        .collect()
}

pub fn build_html(
    title: &str,
    body_sections: &[RenderedSection],
    icon_label: &str,
) -> String {
    build_html_with_nav(
        title,
        body_sections,
        icon_label,
        None,
        None,
        &HtmlExportOptions {
            embed_workspace_script: true,
            client_mermaid_runtime: false,
        },
    )
}

const DIAGRAM_HTML_MARKER: &str = "class=\"diagram-html-display\"";
const MERMAID_CLIENT_MARKER: &str = "data-mermaid-client";
const FOOTNOTE_MARKER: &str = crate::core::export::html::footnotes::FOOTNOTE_MARKER;
const DIAGRAM_HTML_TAILWIND_BROWSER_JS: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/diagram-html-tailwind-browser.js"
));
const MERMAID_INIT_JS: &str = include_str!("../../../../assets/mermaid-init.js");

fn diagram_html_tailwind_browser_js() -> &'static str {
    std::str::from_utf8(DIAGRAM_HTML_TAILWIND_BROWSER_JS)
        .expect("bundled diagram Tailwind browser runtime must be UTF-8")
}

fn mermaid_runtime_tags() -> String {
    // Bust browser cache when the bundled Mermaid version changes. Older Mermaid
    // builds fail to lex Chinese quadrantChart axis labels.
    format!(
        "<script src=\"/__assets/mermaid.min.js?v={}\" data-pagemd-mermaid></script>\n\
<script data-pagemd-mermaid-init>\n{}\n</script>\n",
        env!("PAGEMD_MERMAID_VERSION"),
        script_escape(MERMAID_INIT_JS)
    )
}

fn build_nav_entries(
    body_sections: &[RenderedSection],
    nav_labels: Option<&[String]>,
    input_paths: Option<&[PathBuf]>,
) -> Vec<(PathBuf, usize, String)> {
    let nav_root = input_paths.and_then(common_path_prefix);

    body_sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let label = nav_labels
                .and_then(|labels| labels.get(index))
                .cloned()
                .filter(|label| !label.trim().is_empty())
                .unwrap_or_else(|| {
                    if section.title.trim().is_empty() {
                        format!("Document {}", index + 1)
                    } else {
                        section.title.clone()
                    }
                });

            let rel_path = input_paths
                .and_then(|paths| paths.get(index))
                .and_then(|path| {
                    nav_root
                        .as_ref()
                        .and_then(|root| relativize_to_root(path, root))
                })
                .unwrap_or_else(|| PathBuf::from(&label));

            (rel_path, index, label)
        })
        .collect()
}

fn section_panel_title(
    section: &RenderedSection,
    index: usize,
    nav_labels: Option<&[String]>,
) -> String {
    nav_labels
        .and_then(|labels| labels.get(index))
        .cloned()
        .filter(|label| !label.trim().is_empty())
        .or_else(|| {
            let title = section.title.trim();
            if title.is_empty() {
                None
            } else {
                Some(section.title.clone())
            }
        })
        .unwrap_or_else(|| format!("Document {}", index + 1))
}

fn build_file_sidebar(
    body_sections: &[RenderedSection],
    nav_labels: Option<&[String]>,
    input_paths: Option<&[PathBuf]>,
) -> String {
    let entries = build_nav_entries(body_sections, nav_labels, input_paths);
    let nav_items = if nav_entries_have_tree(&entries) {
        render_nav_tree_html(&build_nav_tree(&entries), 0)
    } else {
        render_flat_nav_html(&entries, 0)
    };

    format!(
        "<aside class=\"doc-sidebar doc-pane\" aria-label=\"Markdown files\">\n\
<nav class=\"doc-nav\">\n{nav_items}</nav>\n\
</aside>\n\
<div class=\"doc-resizer doc-resizer-left\" role=\"separator\" aria-label=\"Resize file navigation\" data-resizer=\"left\"></div>\n"
    )
}

fn topbar_icon(kind: &str) -> &'static str {
    match kind {
        "files" => concat!(
            r#"<svg class="doc-topbar-icon" viewBox="0 0 16 16" aria-hidden="true">"#,
            r#"<path d="M2.5 2.5h4.5v11H2.5zM8.5 2.5h5v11h-5z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/>"#,
            r#"<path d="M7 2.5v11" fill="none" stroke="currentColor" stroke-width="1.25"/>"#,
            "</svg>"
        ),
        "outline" => concat!(
            r#"<svg class="doc-topbar-icon" viewBox="0 0 16 16" aria-hidden="true">"#,
            r#"<path d="M2.5 4h11M2.5 8h8M2.5 12h10" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#,
            "</svg>"
        ),
        "settings" => concat!(
            r#"<svg class="doc-topbar-icon" viewBox="0 0 16 16" aria-hidden="true">"#,
            r#"<path d="M6.7 1.8h2.6l.3 1.2 1.1.5 1.2-.2 1.3 1.3-.2 1.2.5 1.1 1.2.3v2.6l-1.2.3-.5 1.1.2 1.2-1.3 1.3-1.2-.2-1.1.5-.3 1.2H6.7l-.3-1.2-1.1-.5-1.2.2L2.8 11.7l.2-1.2-.5-1.1L1.3 9.1V6.5l1.2-.3.5-1.1-.2-1.2L4.1 2.6l1.2.2 1.1-.5.3-1.2z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>"#,
            r#"<circle cx="8" cy="8" r="2" fill="none" stroke="currentColor" stroke-width="1.2"/>"#,
            "</svg>"
        ),
        "sun" => concat!(
            r#"<svg class="doc-topbar-icon doc-theme-icon doc-theme-icon-sun" viewBox="0 0 16 16" aria-hidden="true">"#,
            r#"<circle cx="8" cy="8" r="2.4" fill="none" stroke="currentColor" stroke-width="1.25"/>"#,
            r#"<path d="M8 1.6v1.4M8 13v1.4M1.6 8h1.4M13 8h1.4M3.3 3.3l1 1M11.7 11.7l1 1M12.7 3.3l-1 1M4.3 11.7l-1 1" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>"#,
            "</svg>"
        ),
        "moon" => concat!(
            r#"<svg class="doc-topbar-icon doc-theme-icon doc-theme-icon-moon" viewBox="0 0 16 16" aria-hidden="true">"#,
            r#"<path d="M12.8 9.4A5.2 5.2 0 0 1 6.6 3.2 5.4 5.4 0 1 0 12.8 9.4z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/>"#,
            "</svg>"
        ),
        _ => "",
    }
}

fn build_topbar(initial_title: &str, use_file_sidebar: bool) -> String {
    let files_btn = if use_file_sidebar {
        format!(
            "<button type=\"button\" class=\"doc-topbar-btn\" data-nav-toggle aria-label=\"Files\" title=\"Files\" aria-pressed=\"true\">{}</button>",
            topbar_icon("files")
        )
    } else {
        "<span class=\"doc-topbar-spacer\" aria-hidden=\"true\"></span>".to_string()
    };
    let escaped_title = html_escape(initial_title);
    format!(
        "<header class=\"doc-topbar\">\n\
<div class=\"doc-topbar-start\">{files_btn}</div>\n\
<div class=\"doc-topbar-title\" data-doc-title title=\"{escaped_title}\">{escaped_title}</div>\n\
<div class=\"doc-topbar-end\">\n\
<button type=\"button\" class=\"doc-topbar-btn\" data-outline-toggle aria-label=\"Outline\" title=\"Outline\" aria-pressed=\"false\">{outline}</button>\n\
<div class=\"doc-settings\">\n\
<button type=\"button\" class=\"doc-topbar-btn\" data-settings-toggle aria-label=\"Settings\" title=\"Settings\" aria-haspopup=\"true\" aria-expanded=\"false\">{settings}</button>\n\
<div class=\"doc-settings-panel\" data-settings-panel hidden>\n\
<div class=\"doc-settings-section\">\n\
<div class=\"doc-settings-label\">Theme</div>\n\
<button type=\"button\" class=\"doc-settings-action\" data-theme-toggle aria-label=\"Switch to dark theme\" title=\"Dark\" aria-pressed=\"false\">{moon}<span class=\"doc-settings-action-text\">Dark</span>{sun}<span class=\"doc-settings-action-text doc-settings-action-text-light\">Light</span></button>\n\
</div>\n\
<div class=\"doc-settings-section\" data-settings-export-slot></div>\n\
</div>\n\
</div>\n\
</div>\n\
</header>\n",
        outline = topbar_icon("outline"),
        settings = topbar_icon("settings"),
        moon = topbar_icon("moon"),
        sun = topbar_icon("sun"),
    )
}

fn build_workspace_layout(
    body_sections: &[RenderedSection],
    nav_labels: Option<&[String]>,
    input_paths: Option<&[PathBuf]>,
    use_file_sidebar: bool,
    embed_workspace_script: bool,
) -> (String, String, String, String) {
    let outline_nav = build_outline_nav(body_sections);
    let workspace_class = if use_file_sidebar {
        "doc-workspace outline-hidden"
    } else {
        "doc-workspace doc-workspace-single outline-hidden"
    };
    let file_sidebar = if use_file_sidebar {
        build_file_sidebar(body_sections, nav_labels, input_paths)
    } else {
        String::new()
    };
    let workspace_script = if embed_workspace_script {
        super::workspace::workspace_script_tag()
    } else {
        String::new()
    };
    let initial_title = section_panel_title(&body_sections[0], 0, nav_labels);
    let topbar = build_topbar(&initial_title, use_file_sidebar);
    (
        format!(
            "<div class=\"{workspace_class}\" data-doc-workspace>\n{topbar}<div class=\"doc-workspace-body\">\n"
        ),
        "</div>\n</div>\n".to_string(),
        format!("{file_sidebar}<main class=\"doc-main\">\n"),
        format!(
            "</main>\n\
<div class=\"doc-resizer doc-resizer-right\" role=\"separator\" aria-label=\"Resize outline\" data-resizer=\"right\"></div>\n\
<aside class=\"doc-outline doc-pane\" aria-label=\"Markdown outline\">\n{outline_nav}</aside>\n\
{workspace_script}"
        ),
    )
}

pub fn build_html_with_nav(
    title: &str,
    body_sections: &[RenderedSection],
    icon_label: &str,
    nav_labels: Option<&[String]>,
    input_paths: Option<&[PathBuf]>,
    opts: &HtmlExportOptions,
) -> String {
    let use_file_sidebar = body_sections.len() > 1;
    let use_outline_workspace = use_file_sidebar
        || body_sections
            .first()
            .is_some_and(|section| !section.outline.is_empty());
    let body_html: String = if use_file_sidebar {
        body_sections
            .iter()
            .enumerate()
            .map(|(index, sec)| {
                let active = if index == 0 { " is-active" } else { "" };
                let panel_title = html_escape(&section_panel_title(sec, index, nav_labels));
                format!(
                    "<section class=\"doc-section doc-panel{active}\" id=\"doc-{}\" data-doc-panel data-panel-title=\"{panel_title}\">\n{}</section>\n",
                    index + 1,
                    sec.html
                )
            })
            .collect()
    } else if use_outline_workspace {
        let panel_title = html_escape(&section_panel_title(&body_sections[0], 0, nav_labels));
        format!(
            "<section class=\"doc-section doc-panel is-active\" id=\"doc-1\" data-doc-panel data-panel-title=\"{panel_title}\">\n{}</section>\n",
            body_sections[0].html
        )
    } else {
        body_sections[0].html.clone()
    };
    let (layout_open, layout_close, nav_html, script_html) = if use_outline_workspace {
        build_workspace_layout(
            body_sections,
            nav_labels,
            input_paths,
            use_file_sidebar,
            opts.embed_workspace_script,
        )
    } else {
        (String::new(), String::new(), String::new(), String::new())
    };
    let container_class = if use_outline_workspace {
        "container container-with-sidebar"
    } else {
        "container"
    };
    let diagram_script = if body_sections
        .iter()
        .any(|section| section.html.contains(DIAGRAM_HTML_MARKER))
    {
        format!(
            "<script>\n{}\n</script>\n",
            script_escape(diagram_html_tailwind_browser_js())
        )
    } else {
        String::new()
    };

    let mermaid_script = if opts.client_mermaid_runtime
        && body_sections
            .iter()
            .any(|section| section.html.contains(MERMAID_CLIENT_MARKER))
    {
        mermaid_runtime_tags()
    } else {
        String::new()
    };

    let footnote_script = if body_sections
        .iter()
        .any(|section| section.html.contains(FOOTNOTE_MARKER))
    {
        super::footnotes::footnote_script_tag()
    } else {
        String::new()
    };

    let lightbox_script = super::lightbox::diagram_lightbox_script_tag();

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<script>
(function () {{
  try {{
    var theme = localStorage.getItem("pagemd.workspace.v1.theme");
    if (theme === "dark" || theme === "light") {{
      document.documentElement.setAttribute("data-theme", theme);
    }}
  }} catch (_) {{}}
}})();
</script>
<title>{title}</title>
{favicon}
<style>
{css}
</style>
{diagram_script}{mermaid_script}
</head>
<body>
<div class="{container_class}">
{layout_open}{nav_html}
{body_html}
{script_html}{layout_close}
</div>
{footnote_script}{lightbox_script}
</body>
</html>"#,
        title = html_escape(title),
        favicon = favicon_link_tag(icon_label),
        css = CSS,
        diagram_script = diagram_script,
        mermaid_script = mermaid_script,
        layout_open = layout_open,
        nav_html = nav_html,
        body_html = body_html,
        script_html = script_html,
        layout_close = layout_close,
        container_class = container_class,
        footnote_script = footnote_script,
        lightbox_script = lightbox_script,
    )
}

#[derive(Debug, Clone)]
pub struct HtmlExportOptions {
    pub embed_workspace_script: bool,
    /// Serve official mermaid.js for client-side diagram rendering (view mode).
    pub client_mermaid_runtime: bool,
}

impl Default for HtmlExportOptions {
    fn default() -> Self {
        Self {
            // Match CLI convert / SingleFile export defaults.
            embed_workspace_script: true,
            client_mermaid_runtime: false,
        }
    }
}

pub fn render_document_html(
    doc: &crate::core::model::Document,
    opts: &HtmlExportOptions,
) -> String {
    build_html_with_nav(
        &doc.title,
        &doc.sections,
        &doc.icon_label,
        Some(&doc.nav_labels),
        Some(&doc.input_paths),
        opts,
    )
}
