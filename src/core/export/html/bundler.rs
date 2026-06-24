use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use regex::Captures;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;

use crate::core::util::{html_escape, regex};

const MAX_INLINE_RESOURCE_BYTES: usize = 25 * 1024 * 1024;

fn http_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("pagemd/0.1")
            .build()
            .expect("failed to build HTTP client")
    })
}
fn is_remote_resource(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

fn is_already_embedded_or_non_fetchable(src: &str) -> bool {
    let lower = src.trim().to_ascii_lowercase();
    lower.is_empty()
        || lower.starts_with("data:")
        || lower.starts_with('#')
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("javascript:")
        || lower.starts_with("about:")
}

fn extension_from_reference(src: &str) -> &str {
    let clean = src.split(['?', '#']).next().unwrap_or(src);
    clean.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

pub(crate) fn data_uri_from_bytes(mime: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime, B64.encode(bytes))
}

pub(crate) fn fetch_remote_resource(url: &str) -> Result<(Vec<u8>, String)> {
    let response = http_client()
        .get(url)
        .send()
        .with_context(|| format!("Failed to fetch {url}"))?
        .error_for_status()
        .with_context(|| format!("Resource returned an error status: {url}"))?;
    let mime = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| mime_from_ext(extension_from_reference(url)).to_string());
    let bytes = response
        .bytes()
        .with_context(|| format!("Failed to read {url}"))?;
    if bytes.len() > MAX_INLINE_RESOURCE_BYTES {
        bail!("Resource is too large to inline: {url}");
    }
    Ok((bytes.to_vec(), mime))
}

fn local_resource_path(src: &str, base_dir: &Path) -> PathBuf {
    if src.starts_with('/') {
        PathBuf::from(src)
    } else {
        base_dir.join(src)
    }
}

fn resource_to_data_uri(src: &str, base_dir: &Path) -> Result<String> {
    let src = src.trim();
    if is_already_embedded_or_non_fetchable(src) {
        return Ok(src.to_string());
    }

    if is_remote_resource(src) {
        let (bytes, mime) = fetch_remote_resource(src)?;
        return Ok(data_uri_from_bytes(&mime, &bytes));
    }

    let path = local_resource_path(src, base_dir);
    let data = std::fs::read(&path).with_context(|| format!("Cannot read {}", path.display()))?;
    if data.len() > MAX_INLINE_RESOURCE_BYTES {
        bail!("Resource is too large to inline: {}", path.display());
    }
    let mime = mime_from_ext(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
    Ok(data_uri_from_bytes(mime, &data))
}

fn embedded_resource_error_data_uri(src: &str) -> String {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"120\" viewBox=\"0 0 640 120\"><rect width=\"640\" height=\"120\" fill=\"#fff7f7\"/><text x=\"24\" y=\"52\" font-family=\"system-ui,sans-serif\" font-size=\"16\" fill=\"#991b1b\">Resource could not be embedded</text><text x=\"24\" y=\"82\" font-family=\"monospace\" font-size=\"12\" fill=\"#7f1d1d\">{}</text></svg>",
        html_escape(src)
    );
    data_uri_from_bytes("image/svg+xml", svg.as_bytes())
}

pub(crate) fn image_to_data_uri(src: &str, base_dir: &Path) -> String {
    match resource_to_data_uri(src, base_dir) {
        Ok(value) => value,
        Err(_) => embedded_resource_error_data_uri(src),
    }
}

fn mime_from_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "html" | "htm" => "text/html",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn inline_resource_for_attr(src: &str, base_dir: &Path) -> String {
    match resource_to_data_uri(src, base_dir) {
        Ok(value) => value,
        Err(_) => data_uri_from_bytes(
            "text/plain",
            format!("Resource could not be embedded: {src}").as_bytes(),
        ),
    }
}

fn replace_attr_resources(input: &str, pattern: &'static str, base_dir: &Path) -> String {
    regex(pattern)
        .replace_all(input, |caps: &Captures<'_>| {
            let value = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            let suffix = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
            let embedded = inline_resource_for_attr(value, base_dir);
            format!("{}{}{}", &caps[1], html_escape(&embedded), suffix)
        })
        .into_owned()
}

fn is_fragment_reference(value: &str) -> bool {
    value.trim().starts_with('#')
}

fn inline_css_urls(input: &str, base_dir: &Path) -> String {
    let double = regex(r#"(?is)url\(\s*\"([^\"]*)\"\s*\)"#)
        .replace_all(input, |caps: &Captures<'_>| {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            if is_fragment_reference(value) {
                return caps[0].to_string();
            }
            let embedded = match resource_to_data_uri(value, base_dir) {
                Ok(value) => value,
                Err(_) => embedded_resource_error_data_uri(value),
            };
            format!("url(\"{}\")", html_escape(&embedded))
        })
        .into_owned();
    let single = regex(r#"(?is)url\(\s*'([^']*)'\s*\)"#)
        .replace_all(&double, |caps: &Captures<'_>| {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            if is_fragment_reference(value) {
                return caps[0].to_string();
            }
            let embedded = match resource_to_data_uri(value, base_dir) {
                Ok(value) => value,
                Err(_) => embedded_resource_error_data_uri(value),
            };
            format!("url(\"{}\")", html_escape(&embedded))
        })
        .into_owned();
    regex(r#"(?is)url\(\s*([^\s\)'\"]+)\s*\)"#)
        .replace_all(&single, |caps: &Captures<'_>| {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            if is_fragment_reference(value) {
                return caps[0].to_string();
            }
            let embedded = match resource_to_data_uri(value, base_dir) {
                Ok(value) => value,
                Err(_) => embedded_resource_error_data_uri(value),
            };
            format!("url(\"{}\")", html_escape(&embedded))
        })
        .into_owned()
}

pub(crate) fn inline_raw_html_resources(raw: &str, base_dir: &Path) -> String {
    let rewritten = inline_css_urls(raw, base_dir);
    let rewritten = replace_attr_resources(
        &rewritten,
        r#"(?is)(\s(?:src|poster)\s*=\s*\")([^\"]*)(\")"#,
        base_dir,
    );
    let rewritten = replace_attr_resources(
        &rewritten,
        r#"(?is)(\s(?:src|poster)\s*=\s*')([^']*)(')"#,
        base_dir,
    );
    let rewritten = replace_attr_resources(
        &rewritten,
        r#"(?is)(<link\b[^>]*?\shref\s*=\s*\")([^\"]*)(\")"#,
        base_dir,
    );
    replace_attr_resources(
        &rewritten,
        r#"(?is)(<link\b[^>]*?\shref\s*=\s*')([^']*)(')"#,
        base_dir,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_url_fragment_references_are_left_unchanged() {
        let input = r#"<path marker-end="url(#arr)" marker-start="url(#start)"/>"#;
        let output = inline_css_urls(input, Path::new("."));
        assert_eq!(output, input);
    }

    #[test]
    fn css_url_file_references_are_still_rewritten() {
        let dir = std::env::temp_dir().join(format!(
            "pagemd-bundler-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("icon.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        )
        .unwrap();
        let input = r#"<style>.x { background: url(icon.svg); }</style>"#;
        let output = inline_css_urls(input, &dir);
        assert!(output.contains("data:image/svg+xml;base64,"));
        assert!(!output.contains("url(icon.svg)"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
