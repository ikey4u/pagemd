use std::sync::OnceLock;

use regex::Regex;

/// Log fenced-block render failures to stderr (convert and `pagemd view` both use this path).
pub(crate) fn eprint_fence_render_error(
    kind: &str,
    err: &(impl std::fmt::Display + ?Sized),
    source: &str,
) {
    eprintln!("\n[pagemd] {kind} block render failed");
    eprintln!("{err:#}");
    let trimmed = source.trim();
    if !trimmed.is_empty() {
        eprintln!("\n--- {kind} source ---");
        eprintln!("{trimmed}");
        eprintln!("--- end {kind} source ---\n");
    } else {
        eprintln!();
    }
}
pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}
pub(crate) fn script_escape(script: &str) -> String {
    script.replace("</script", "<\\/script")
}
pub(crate) fn regex(pattern: &'static str) -> &'static Regex {
    static CACHE: OnceLock<
        std::sync::Mutex<std::collections::HashMap<&'static str, &'static Regex>>,
    > = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(value) = cache
        .lock()
        .expect("regex cache poisoned")
        .get(pattern)
        .copied()
    {
        return value;
    }
    let compiled = Box::leak(Box::new(Regex::new(pattern).expect("invalid regex")));
    cache
        .lock()
        .expect("regex cache poisoned")
        .insert(pattern, compiled);
    compiled
}

pub(crate) fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn unique_heading_id(
    text: &str,
    heading_ids: &mut std::collections::HashMap<String, usize>,
) -> String {
    let base = {
        let slug = slugify(text);
        if slug.is_empty() {
            "heading".to_string()
        } else {
            slug
        }
    };
    let count = heading_ids.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{}", *count)
    }
}
