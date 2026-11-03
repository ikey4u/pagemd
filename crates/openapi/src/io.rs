use crate::{parse_loaded, source_attribute, Document, MAX_BYTES};
use std::{io::Read, path::Path, time::Duration};
use url::Url;

/// Default file/HTTPS reader. A host can instead supply its own reader to
/// `parse_loaded`, for remote workspaces and application-specific permissions.
pub fn read_blocking(url: &Url, root: &Path) -> Result<String, String> {
    let mut bytes = vec![];
    match url.scheme() {
        "file" => {
            let path = url
                .to_file_path()
                .map_err(|_| "Invalid OpenAPI file path")?;
            let path = path.canonicalize().map_err(|e| e.to_string())?;
            let root = root.canonicalize().map_err(|e| e.to_string())?;
            if !path.starts_with(root) {
                return Err(
                    "OpenAPI source is outside the document folder".into()
                );
            }
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
        }
        "https" if url.username().is_empty() && url.password().is_none() => {
            let response = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?
                .get(url.clone())
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            response
                .take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
        }
        _ => {
            return Err(
                "OpenAPI sources require a document path or public HTTPS URL"
                    .into(),
            )
        }
    }
    if bytes.len() > MAX_BYTES {
        return Err("OpenAPI source exceeds the 2 MiB display limit".into());
    }
    String::from_utf8(bytes)
        .map_err(|_| "OpenAPI source is not UTF-8 text".into())
}

pub fn load_blocking(
    body: &str,
    meta: &str,
    folder: &Path,
) -> Result<Document, String> {
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    let source = source_attribute(meta, body)?;
    let base = Url::from_file_path(folder.join("__markdown__.md"))
        .map_err(|_| "Invalid document folder")?;
    let base = source
        .as_ref()
        .map(|s| base.join(s))
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(base);
    let text = if source.is_some() {
        read_blocking(&base, &folder)?
    } else {
        body.into()
    };
    futures_lite::future::block_on(parse_loaded(&text, &base, |url| {
        let result = read_blocking(&url, &folder);
        async move { result }
    }))
}
