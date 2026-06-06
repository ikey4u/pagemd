use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct ExcludeMatcher {
    patterns: Vec<String>,
}

impl ExcludeMatcher {
    pub(crate) fn new(patterns: &[String]) -> Self {
        Self {
            patterns: patterns
                .iter()
                .map(|pattern| pattern.trim().to_string())
                .filter(|pattern| !pattern.is_empty())
                .collect(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub(crate) fn should_skip_dir(&self, dir_path: &Path, scan_root: &Path) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let rel = relative_path(dir_path, scan_root);
        self.matches_path(&rel, true)
    }

    pub(crate) fn should_skip_file(&self, file_path: &Path, scan_root: &Path) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let rel = relative_path(file_path, scan_root);
        self.matches_path(&rel, false)
    }

    fn matches_path(&self, rel: &Path, _is_dir: bool) -> bool {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let name = rel
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        for pattern in &self.patterns {
            let pattern = pattern.replace('\\', "/");
            if pattern.contains('/') || pattern.contains('*') {
                if glob_match(&pattern, &rel_str) {
                    return true;
                }
                continue;
            }

            if name == pattern {
                return true;
            }

            if rel.components().any(|component| {
                matches!(component, Component::Normal(value) if value == pattern.as_str())
            }) {
                return true;
            }
        }

        false
    }
}

fn relative_path(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_segments(
        pattern.split('/').collect::<Vec<_>>(),
        text.split('/').collect::<Vec<_>>(),
    )
}

fn glob_match_segments(pattern: Vec<&str>, text: Vec<&str>) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some(segment), _) if *segment == "**" => {
            if pattern.len() == 1 {
                return true;
            }
            for index in 0..=text.len() {
                if glob_match_segments(pattern[1..].to_vec(), text[index..].to_vec()) {
                    return true;
                }
            }
            false
        }
        (Some(pattern_segment), Some(text_segment)) => {
            if segment_match(pattern_segment, text_segment) {
                glob_match_segments(pattern[1..].to_vec(), text[1..].to_vec())
            } else {
                false
            }
        }
        _ => false,
    }
}

fn segment_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    segment_match_chars(&pattern, &text)
}

fn segment_match_chars(pattern: &[char], text: &[char]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some('*'), None) => segment_match_chars(&pattern[1..], text),
        (Some('*'), Some(_)) => {
            segment_match_chars(&pattern[1..], text) || segment_match_chars(pattern, &text[1..])
        }
        (Some(expected), Some(actual)) if expected == actual => {
            segment_match_chars(&pattern[1..], &text[1..])
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_matches_name_and_glob_patterns() {
        let matcher = ExcludeMatcher::new(&[
            "node_modules".to_string(),
            "drafts/**".to_string(),
            "*.tmp.md".to_string(),
        ]);

        assert!(matcher.should_skip_dir(Path::new("/root/node_modules"), Path::new("/root")));
        assert!(
            matcher.should_skip_file(Path::new("/root/drafts/old/readme.md"), Path::new("/root"))
        );
        assert!(matcher.should_skip_file(Path::new("/root/notes.tmp.md"), Path::new("/root")));
        assert!(!matcher.should_skip_file(Path::new("/root/guide/readme.md"), Path::new("/root")));
    }

    #[test]
    fn exclude_name_pattern_matches_files_in_subdirectories() {
        let matcher = ExcludeMatcher::new(&["guide".to_string()]);
        assert!(matcher.should_skip_file(
            Path::new("/root/docs/guide/readme.md"),
            Path::new("/root/docs")
        ));
    }
}
