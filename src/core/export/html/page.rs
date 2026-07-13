use std::path::{Path, PathBuf};

use crate::core::export::html::favicon::favicon_link_tag;
use crate::core::export::html::nav_tree::{
    build_nav_tree, common_path_prefix, nav_entries_have_tree, relativize_to_root,
    render_flat_nav_html, render_nav_tree_html,
};
use crate::core::model::RenderedSection;
use crate::core::util::{html_escape, script_escape};

use super::styles::CSS;

pub(crate) fn section_label(path: &Path) -> String {
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

pub(crate) fn build_html(
    title: &str,
    body_sections: &[RenderedSection],
    icon_label: &str,
) -> String {
    build_html_with_nav(title, body_sections, icon_label, None, None, true)
}

const DIAGRAM_HTML_MARKER: &str = "class=\"diagram-html-display\"";
const FOOTNOTE_MARKER: &str = crate::core::export::html::footnotes::FOOTNOTE_MARKER;
const DIAGRAM_HTML_TAILWIND_BROWSER_JS: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/diagram-html-tailwind-browser.js"
));

fn diagram_html_tailwind_browser_js() -> &'static str {
    std::str::from_utf8(DIAGRAM_HTML_TAILWIND_BROWSER_JS)
        .expect("bundled diagram Tailwind browser runtime must be UTF-8")
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
<div class=\"doc-sidebar-top\"><div class=\"doc-pane-header\">Files</div>\
<button type=\"button\" class=\"doc-nav-toggle doc-nav-toggle-panel\" data-nav-toggle aria-label=\"Hide files\">Hide</button></div>\n\
<div class=\"doc-sidebar-body\"><nav class=\"doc-nav\">\n{nav_items}</nav></div>\n\
</aside>\n\
<div class=\"doc-resizer doc-resizer-left\" role=\"separator\" aria-label=\"Resize file navigation\" data-resizer=\"left\"></div>\n"
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
    let nav_toggle_main = if use_file_sidebar {
        "<button type=\"button\" class=\"doc-nav-toggle doc-nav-toggle-main\" data-nav-toggle>Files</button>\n"
    } else {
        ""
    };
    (
        format!("<div class=\"{workspace_class}\" data-doc-workspace>\n"),
        "</div>\n".to_string(),
        format!(
            "{file_sidebar}<main class=\"doc-main\">\n{nav_toggle_main}\
<button type=\"button\" class=\"doc-outline-toggle doc-outline-toggle-main\" data-outline-toggle>Outline</button>\n"
        ),
        format!(
            "</main>\n<div class=\"doc-resizer doc-resizer-right\" role=\"separator\" aria-label=\"Resize outline\" data-resizer=\"right\"></div>\n<aside class=\"doc-outline doc-pane\" aria-label=\"Markdown outline\">\n<div class=\"doc-outline-top\"><div class=\"doc-pane-header\">Outline</div><button type=\"button\" class=\"doc-outline-toggle doc-outline-toggle-panel\" data-outline-toggle aria-label=\"Hide outline\">Hide</button></div>\n<div class=\"doc-outline-body\">\n{outline_nav}</div></aside>\n{workspace_script}"
        ),
    )
}

pub(crate) fn build_html_with_nav(
    title: &str,
    body_sections: &[RenderedSection],
    icon_label: &str,
    nav_labels: Option<&[String]>,
    input_paths: Option<&[PathBuf]>,
    embed_workspace_script: bool,
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
                format!(
                    "<section class=\"doc-section doc-panel{active}\" id=\"doc-{}\" data-doc-panel>\n{}</section>\n",
                    index + 1,
                    sec.html
                )
            })
            .collect()
    } else if use_outline_workspace {
        format!(
            "<section class=\"doc-section doc-panel is-active\" id=\"doc-1\" data-doc-panel>\n{}</section>\n",
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
            embed_workspace_script,
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

    let footnote_script = if body_sections
        .iter()
        .any(|section| section.html.contains(FOOTNOTE_MARKER))
    {
        super::footnotes::footnote_script_tag()
    } else {
        String::new()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
{favicon}
<style>
{css}
</style>
{diagram_script}
</head>
<body>
<div class="{container_class}">
{layout_open}{nav_html}
{body_html}
{script_html}{layout_close}
</div>
{footnote_script}
</body>
</html>"#,
        title = html_escape(title),
        favicon = favicon_link_tag(icon_label),
        css = CSS,
        diagram_script = diagram_script,
        layout_open = layout_open,
        nav_html = nav_html,
        body_html = body_html,
        script_html = script_html,
        layout_close = layout_close,
        container_class = container_class,
        footnote_script = footnote_script,
    )
}

pub(crate) struct HtmlExportOptions {
    pub embed_workspace_script: bool,
}

pub(crate) fn render_document_html(
    doc: &crate::core::model::Document,
    opts: &HtmlExportOptions,
) -> String {
    build_html_with_nav(
        &doc.title,
        &doc.sections,
        &doc.icon_label,
        Some(&doc.nav_labels),
        Some(&doc.input_paths),
        opts.embed_workspace_script,
    )
}
