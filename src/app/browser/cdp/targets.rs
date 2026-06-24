use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PageTarget {
    pub url: String,
    pub title: String,
    pub ws_url: String,
}

pub async fn list_page_targets(port: u16) -> Result<Vec<PageTarget>> {
    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;
    let list_url = format!("http://127.0.0.1:{port}/json/list");
    let text = client
        .get(&list_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let targets: Vec<Value> = serde_json::from_str(&text)?;

    let mut pages = Vec::new();
    for target in targets {
        if target.get("type").and_then(|v| v.as_str()) != Some("page") {
            continue;
        }
        let Some(ws_url) = target.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) else {
            continue;
        };
        pages.push(PageTarget {
            url: target
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            title: target
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            ws_url: ws_url.to_owned(),
        });
    }
    Ok(pages)
}

pub fn pick_best_page_target<'a>(
    targets: &'a [PageTarget],
    preferred: Option<&str>,
) -> Option<&'a PageTarget> {
    if targets.is_empty() {
        return None;
    }
    targets
        .iter()
        .max_by_key(|t| score_target(t, preferred))
        .filter(|t| score_target(t, preferred) > i32::MIN / 2)
}

pub fn score_target(target: &PageTarget, preferred: Option<&str>) -> i32 {
    let mut score = 0;
    let url = target.url.as_str();

    if let Some(want) = preferred {
        if urls_match(url, want) {
            score += 10_000;
        }
    }

    if url.starts_with("https://") || url.starts_with("http://") {
        score += 1_000;
        score += (target.title.trim().len() as i32).min(200);
        score += (url.len() as i32).min(100);
    } else if url == "about:blank" {
        score -= 500;
    } else if url.starts_with("chrome://") || url.starts_with("chrome-untrusted://") {
        score -= 2_000;
    } else if url.starts_with("devtools://") {
        score -= 5_000;
    } else {
        score -= 100;
    }

    score
}

pub fn urls_match(a: &str, b: &str) -> bool {
    normalize_url(a) == normalize_url(b)
}

fn normalize_url(url: &str) -> String {
    let mut out = url.trim().to_ascii_lowercase();
    if let Some((base, _frag)) = out.split_once('#') {
        out = base.to_owned();
    }
    while out.ends_with('/') && out.len() > "https://x".len() {
        out.pop();
    }
    out
}

pub fn format_target_choices(targets: &[PageTarget]) -> String {
    if targets.is_empty() {
        return "(no page targets)".into();
    }
    targets
        .iter()
        .map(|t| format!("- {} | {}", t.title, t.url))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_http_over_chrome_internal() {
        let targets = vec![
            PageTarget {
                url: "chrome://newtab/".into(),
                title: "New Tab".into(),
                ws_url: "ws://a".into(),
            },
            PageTarget {
                url: "https://example.com/doc".into(),
                title: "Example Doc".into(),
                ws_url: "ws://b".into(),
            },
            PageTarget {
                url: "about:blank".into(),
                title: "".into(),
                ws_url: "ws://c".into(),
            },
        ];
        let best = pick_best_page_target(&targets, None).unwrap();
        assert_eq!(best.url, "https://example.com/doc");
    }

    #[test]
    fn preferred_url_wins() {
        let targets = vec![
            PageTarget {
                url: "https://a.test/".into(),
                title: "A".into(),
                ws_url: "ws://a".into(),
            },
            PageTarget {
                url: "https://b.test/page".into(),
                title: "B".into(),
                ws_url: "ws://b".into(),
            },
        ];
        let best = pick_best_page_target(&targets, Some("https://b.test/page#section")).unwrap();
        assert_eq!(best.url, "https://b.test/page");
    }
}
