use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::core::util::html_escape;

#[derive(Debug, Clone)]
pub enum NavTreeNode {
    Folder {
        id: String,
        name: String,
        children: Vec<NavTreeNode>,
    },
    File {
        section_index: usize,
        label: String,
        copy_path: String,
    },
}

pub fn nav_copy_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn common_path_prefix(paths: &[PathBuf]) -> Option<PathBuf> {
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

pub fn relativize_to_root(path: &Path, root: &Path) -> Option<PathBuf> {
    let canonical_path =
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_root =
        root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    canonical_path
        .strip_prefix(&canonical_root)
        .ok()
        .map(Path::to_path_buf)
}

pub fn build_nav_tree(
    entries: &[(PathBuf, usize, String)],
) -> Vec<NavTreeNode> {
    let mut grouped: BTreeMap<String, Vec<(PathBuf, usize, String)>> =
        BTreeMap::new();

    for (path, section_index, label) in entries {
        let key = path
            .parent()
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        grouped.entry(key).or_default().push((
            path.clone(),
            *section_index,
            label.clone(),
        ));
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
                copy_path: nav_copy_path(path),
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

/// Directory name shared by every file, or `fallback` when the files do not
/// share a parent directory. A multi-file document uses this as its single
/// navigation root.
pub fn nav_root_label(paths: Option<&[PathBuf]>, fallback: &str) -> String {
    let fallback = {
        let trimmed = fallback.trim();
        if trimmed.is_empty() {
            "Document"
        } else {
            trimmed
        }
    };
    let Some(paths) = paths.filter(|paths| !paths.is_empty()) else {
        return fallback.to_string();
    };
    let Some(prefix) = common_path_prefix(paths) else {
        return fallback.to_string();
    };
    // One path's common prefix is the file itself; the document root is its folder.
    let dir = if paths.len() == 1 {
        prefix.parent().map(Path::to_path_buf).unwrap_or(prefix)
    } else {
        prefix
    };
    dir.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

/// Wrap a forest in one folder so every document nav has a single root.
pub fn root_nav_tree(nodes: Vec<NavTreeNode>, name: &str) -> Vec<NavTreeNode> {
    if nodes.is_empty() {
        return nodes;
    }
    let name = {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            "Document"
        } else {
            trimmed
        }
    };
    vec![NavTreeNode::Folder {
        id: ".".to_string(),
        name: name.to_string(),
        children: nodes,
    }]
}

pub fn render_nav_tree_html(
    nodes: &[NavTreeNode],
    active_index: usize,
) -> String {
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
            let root_attr = if id == "." { " data-nav-root" } else { "" };
            format!(
                "<li class=\"doc-nav-folder is-expanded\" data-nav-node data-nav-folder=\"{escaped_id}\"{root_attr}>\n<div class=\"doc-nav-folder-row\"><button type=\"button\" class=\"doc-nav-folder-toggle\" aria-expanded=\"true\" aria-label=\"Toggle {escaped_name} folder\"><span class=\"doc-nav-folder-chevron\" aria-hidden=\"true\"></span></button><span class=\"doc-nav-folder-label\">{escaped_name}</span></div>\n{child_html}</li>\n"
            )
        }
        NavTreeNode::File {
            section_index,
            label,
            copy_path,
        } => render_file_row(*section_index, label, copy_path, active_index),
    }
}

fn render_file_row(
    section_index: usize,
    label: &str,
    copy_path: &str,
    active_index: usize,
) -> String {
    let doc_id = section_index + 1;
    let active = if section_index == active_index {
        " is-active"
    } else {
        ""
    };
    let escaped_label = html_escape(label);
    let escaped_copy_path = html_escape(copy_path);
    format!(
        "<li class=\"doc-nav-file\" data-nav-node><div class=\"doc-nav-row\"><a class=\"doc-nav-link{active}\" href=\"#doc-{doc_id}\" data-doc-target=\"doc-{doc_id}\" title=\"{escaped_copy_path}\"><span class=\"doc-nav-label\">{escaped_label}</span></a><button type=\"button\" class=\"doc-nav-copy\" data-copy-label=\"{escaped_copy_path}\" aria-label=\"Copy path {escaped_copy_path}\" title=\"Copy path\">Copy</button></div></li>\n"
    )
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

        let rooted = root_nav_tree(tree, "docs");
        assert_eq!(rooted.len(), 1);
        match &rooted[0] {
            NavTreeNode::Folder { id, name, children } => {
                assert_eq!(id, ".");
                assert_eq!(name, "docs");
                assert_eq!(children.len(), 2);
            }
            NavTreeNode::File { .. } => {
                panic!("document root must be a folder")
            }
        }
    }

    #[test]
    fn nav_root_label_uses_the_shared_directory() {
        let paths = vec![
            PathBuf::from("/project/docs/readme.md"),
            PathBuf::from("/project/docs/guide/start.md"),
        ];
        assert_eq!(nav_root_label(Some(&paths), "Title"), "docs");
        assert_eq!(
            nav_root_label(
                Some(&[PathBuf::from("a.md"), PathBuf::from("b.md")]),
                "Title"
            ),
            "Title"
        );
        assert_eq!(nav_root_label(None, "  "), "Document");
    }
}
