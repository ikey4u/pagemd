use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::util::html_escape;

#[derive(Debug, Clone)]
pub(crate) enum NavTreeNode {
    Folder {
        id: String,
        name: String,
        children: Vec<NavTreeNode>,
    },
    File {
        section_index: usize,
        label: String,
    },
}

pub(crate) fn common_path_prefix(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }

    let canonical = paths
        .iter()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
        .collect::<Vec<_>>();

    let first = canonical[0].components().collect::<Vec<_>>();
    let mut prefix_len = first.len();

    for path in &canonical[1..] {
        let components = path.components().collect::<Vec<_>>();
        prefix_len = prefix_len.min(components.len());
        for index in 0..prefix_len {
            if first[index] != components[index] {
                prefix_len = index;
                break;
            }
        }
    }

    if prefix_len == 0 {
        return None;
    }

    let mut prefix = PathBuf::new();
    for index in 0..prefix_len {
        prefix.push(first[index].as_os_str());
    }

    Some(prefix)
}

pub(crate) fn relativize_to_root(path: &Path, root: &Path) -> Option<PathBuf> {
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    canonical_path
        .strip_prefix(&canonical_root)
        .ok()
        .map(Path::to_path_buf)
}

pub(crate) fn nav_entries_have_tree(entries: &[(PathBuf, usize, String)]) -> bool {
    entries
        .iter()
        .any(|(path, _, _)| path.components().count() > 1)
}

pub(crate) fn build_nav_tree(entries: &[(PathBuf, usize, String)]) -> Vec<NavTreeNode> {
    let mut grouped: BTreeMap<String, Vec<(PathBuf, usize, String)>> = BTreeMap::new();

    for (path, section_index, label) in entries {
        let key = path
            .parent()
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        grouped
            .entry(key)
            .or_default()
            .push((path.clone(), *section_index, label.clone()));
    }

    build_level(&grouped, "")
}

fn build_level(
    grouped: &BTreeMap<String, Vec<(PathBuf, usize, String)>>,
    prefix: &str,
) -> Vec<NavTreeNode> {
    let mut nodes = Vec::new();

    if let Some(files) = grouped.get(prefix) {
        for (path, section_index, label) in files {
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(label.as_str())
                .to_string();
            nodes.push(NavTreeNode::File {
                section_index: *section_index,
                label: if label.trim().is_empty() {
                    file_name
                } else {
                    label.clone()
                },
            });
        }
    }

    let folder_prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}/")
    };

    let mut child_folders = BTreeMap::<String, ()>::new();
    for key in grouped.keys() {
        if key == prefix || !key.starts_with(&folder_prefix) {
            continue;
        }
        let remainder = &key[folder_prefix.len()..];
        if remainder.is_empty() {
            continue;
        }
        let folder_name = remainder.split('/').next().unwrap_or(remainder);
        child_folders.insert(folder_name.to_string(), ());
    }

    for folder_name in child_folders.keys() {
        let folder_id = if prefix.is_empty() {
            folder_name.clone()
        } else {
            format!("{prefix}/{folder_name}")
        };

        nodes.push(NavTreeNode::Folder {
            id: folder_id.clone(),
            name: folder_name.clone(),
            children: build_level(grouped, &folder_id),
        });
    }

    nodes.sort_by(|left, right| node_sort_key(left).cmp(&node_sort_key(right)));
    nodes
}

fn node_sort_key(node: &NavTreeNode) -> (u8, String) {
    match node {
        NavTreeNode::Folder { name, .. } => (0, name.to_ascii_lowercase()),
        NavTreeNode::File { label, .. } => (1, label.to_ascii_lowercase()),
    }
}

pub(crate) fn render_nav_tree_html(nodes: &[NavTreeNode], active_index: usize) -> String {
    if nodes.is_empty() {
        return String::new();
    }

    let items = nodes
        .iter()
        .map(|node| render_nav_tree_node(node, active_index))
        .collect::<String>();

    format!("<ul class=\"doc-nav-tree\">\n{items}</ul>\n")
}

fn render_nav_tree_node(node: &NavTreeNode, active_index: usize) -> String {
    match node {
        NavTreeNode::Folder { id, name, children } => {
            let child_html = render_nav_tree_html(children, active_index);
            let escaped_id = html_escape(id);
            let escaped_name = html_escape(name);
            format!(
                "<li class=\"doc-nav-folder is-expanded\" data-nav-folder=\"{escaped_id}\">\n<div class=\"doc-nav-folder-row\"><button type=\"button\" class=\"doc-nav-folder-toggle\" aria-expanded=\"true\" aria-label=\"Toggle {escaped_name} folder\"><span class=\"doc-nav-folder-chevron\" aria-hidden=\"true\"></span></button><span class=\"doc-nav-folder-label\">{escaped_name}</span></div>\n{child_html}</li>\n"
            )
        }
        NavTreeNode::File {
            section_index,
            label,
        } => render_file_row(*section_index, label, active_index, true),
    }
}

pub(crate) fn render_flat_nav_html(
    entries: &[(PathBuf, usize, String)],
    active_index: usize,
) -> String {
    entries
        .iter()
        .map(|(_, section_index, label)| {
            render_file_row(*section_index, label, active_index, false)
        })
        .collect()
}

fn render_file_row(
    section_index: usize,
    label: &str,
    active_index: usize,
    wrap_li: bool,
) -> String {
    let doc_id = section_index + 1;
    let active = if section_index == active_index {
        " is-active"
    } else {
        ""
    };
    let escaped_label = html_escape(label);
    let row = format!(
        "<div class=\"doc-nav-row\"><a class=\"doc-nav-link{active}\" href=\"#doc-{doc_id}\" data-doc-target=\"doc-{doc_id}\" title=\"{escaped_label}\"><span class=\"doc-nav-label\">{escaped_label}</span></a><button type=\"button\" class=\"doc-nav-copy\" data-copy-label=\"{escaped_label}\" aria-label=\"Copy filename {escaped_label}\" title=\"Copy filename\">Copy</button></div>\n"
    );
    if wrap_li {
        format!("<li class=\"doc-nav-file\">{row}</li>\n")
    } else {
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_nav_tree_groups_nested_paths() {
        let entries = vec![
            (PathBuf::from("readme.md"), 0, "readme.md".to_string()),
            (PathBuf::from("guide/start.md"), 1, "start.md".to_string()),
            (
                PathBuf::from("guide/advanced/topic.md"),
                2,
                "topic.md".to_string(),
            ),
        ];

        let tree = build_nav_tree(&entries);
        assert_eq!(tree.len(), 2);
        assert!(matches!(tree[0], NavTreeNode::Folder { .. }));
        assert!(matches!(tree[1], NavTreeNode::File { .. }));
    }

    #[test]
    fn build_nav_tree_includes_readme_only_subdirectories() {
        let entries = vec![
            (PathBuf::from("readme.md"), 0, "readme.md".to_string()),
            (
                PathBuf::from("analysis/FLUT/README.md"),
                1,
                "README.md".to_string(),
            ),
            (
                PathBuf::from("analysis/OTHER/README.md"),
                2,
                "README.md".to_string(),
            ),
        ];

        let tree = build_nav_tree(&entries);
        assert_eq!(tree.len(), 2);

        let analysis = match &tree[0] {
            NavTreeNode::Folder { name, children, .. } => {
                assert_eq!(name, "analysis");
                children
            }
            other => panic!("expected analysis folder, got {other:?}"),
        };
        assert_eq!(analysis.len(), 2);
        assert!(analysis
            .iter()
            .all(|node| matches!(node, NavTreeNode::Folder { .. })));

        let flut = match &analysis[0] {
            NavTreeNode::Folder { name, children, .. } => {
                assert_eq!(name, "FLUT");
                children
            }
            other => panic!("expected FLUT folder, got {other:?}"),
        };
        assert!(matches!(
            flut.first(),
            Some(NavTreeNode::File {
                section_index: 1,
                ..
            })
        ));
    }

    #[test]
    fn build_nav_tree_includes_deep_paths_without_intermediate_files() {
        let entries = vec![
            (PathBuf::from("root.md"), 0, "root.md".to_string()),
            (
                PathBuf::from("nested/deep/only.md"),
                1,
                "only.md".to_string(),
            ),
        ];

        let tree = build_nav_tree(&entries);
        assert_eq!(tree.len(), 2);

        let nested = match &tree[0] {
            NavTreeNode::Folder { name, children, .. } => {
                assert_eq!(name, "nested");
                children
            }
            other => panic!("expected nested folder, got {other:?}"),
        };
        let deep = match &nested[0] {
            NavTreeNode::Folder { name, children, .. } => {
                assert_eq!(name, "deep");
                children
            }
            other => panic!("expected deep folder, got {other:?}"),
        };
        assert!(matches!(
            deep.first(),
            Some(NavTreeNode::File {
                section_index: 1,
                ..
            })
        ));
    }
}
