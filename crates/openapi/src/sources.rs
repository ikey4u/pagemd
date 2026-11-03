use crate::{decode, parse_with_refs, Document, MAX_BYTES};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::Future,
};
use url::Url;

fn rebase(
    value: &mut Value,
    base: &Url,
    external: bool,
    pending: &mut BTreeSet<String>,
) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if external || !reference.starts_with('#') {
                    let resolved = base.join(reference).map_err(|e| {
                        format!("Invalid reference {reference}: {e}")
                    })?;
                    *reference = resolved.to_string();
                    let mut document = resolved;
                    document.set_fragment(None);
                    pending.insert(document.to_string());
                }
            }
            for value in object.values_mut() {
                rebase(value, base, external, pending)?;
            }
        }
        Value::Array(array) => {
            for value in array {
                rebase(value, base, external, pending)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Bounded external-reference loading through a host-owned read capability.
/// Host policy controls allowed URLs/paths and credentials. Parsing does no I/O.
pub async fn parse_loaded<F, Fut>(
    source: &str,
    base: &Url,
    mut read: F,
) -> Result<Document, String>
where
    F: FnMut(Url) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let mut root = decode(source)?;
    let mut pending = BTreeSet::new();
    rebase(&mut root, base, false, &mut pending)?;
    let mut queue: VecDeque<_> = pending.into_iter().collect();
    let mut documents = BTreeMap::new();
    let mut failures = vec![];
    let mut attempted = BTreeSet::new();
    let mut total = source.len();
    while let Some(uri) = queue.pop_front() {
        if !attempted.insert(uri.clone()) {
            continue;
        }
        if attempted.len() > 32 {
            return Err(
                "OpenAPI external-reference limit reached (32 documents)"
                    .into(),
            );
        }
        let url = Url::parse(&uri).map_err(|e| e.to_string())?;
        let text = match read(url.clone()).await {
            Ok(text) => text,
            Err(error) => {
                failures.push(format!("Could not resolve {uri}: {error}"));
                continue;
            }
        };
        total += text.len();
        if total > MAX_BYTES * 4 {
            return Err(
                "OpenAPI references exceed the 8 MiB display limit".into()
            );
        }
        let mut document = decode(&text)?;
        let mut pending = BTreeSet::new();
        rebase(&mut document, &url, true, &mut pending)?;
        documents.insert(uri, document);
        queue.extend(pending);
    }
    let mut result = parse_with_refs(&root.to_string(), &documents)?;
    result.warnings.extend(failures);
    Ok(result)
}
