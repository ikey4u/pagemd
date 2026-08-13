use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use super::cdp::CdpSession;
use super::snap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookKind {
    Clean,
    Extract,
    Navigate,
    Stop,
}

#[derive(Clone, Debug)]
pub struct PagmdScript {
    pub url_pattern: String,
    pub source: String,
    pub clean: Option<String>,
    pub extract: String,
    pub navigate: Option<String>,
    pub stop: Option<String>,
    /// Optional `const usage = "…"` blurb shown by `--usage`.
    pub usage: Option<String>,
    /// Parsed `const defaultParams = { … }` (empty object if absent/unparseable).
    pub default_params: Value,
    /// Optional `const paramHelp = { key: "description", … }`.
    pub param_help: Value,
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub max_pages: usize,
    pub delay_ms: (u64, u64),
    pub output: PathBuf,
    pub include_title: bool,
    pub include_source_url: bool,
    /// CLI / host overrides merged into script `params` at runtime.
    pub params: Value,
    /// Optional override for script `urlPattern` (CLI `--url-pattern`).
    pub url_pattern: Option<String>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            max_pages: 50,
            delay_ms: (800, 1600),
            output: PathBuf::from("pagemd-run"),
            include_title: true,
            include_source_url: true,
            params: json!({}),
            url_pattern: None,
        }
    }
}

impl RunOptions {
    /// Effective urlPattern: CLI override, else script default.
    pub fn effective_url_pattern<'a>(&'a self, script: &'a PagmdScript) -> &'a str {
        self.url_pattern
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(script.url_pattern.as_str())
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RunPageResult {
    pub url: String,
    pub title: String,
    pub markdown: String,
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub pages: Vec<RunPageResult>,
    pub stop_reason: String,
    pub output: PathBuf,
}

pub fn normalize_filename(name: &str) -> String {
    let mut name = name.trim().trim_matches(['"', '\'']);
    if name.is_empty() {
        return "untitled.pagemd.js".to_string();
    }
    if name.ends_with(".pagemd.js") {
        return name.to_string();
    }
    if name.ends_with(".js") {
        name = name.strip_suffix(".js").unwrap_or(name);
    }
    format!("{name}.pagemd.js")
}

pub fn validate_pagemd_script(source: &str) -> Result<()> {
    parse_pagemd_script(source).map(|_| ())
}

pub fn parse_pagemd_script(source: &str) -> Result<PagmdScript> {
    let source = source.trim();
    if source.is_empty() {
        bail!("script content is empty");
    }
    if source.contains("import ") || source.contains("export ") {
        bail!("script must be plain JS (no ESM import/export)");
    }

    let url_pattern = parse_url_pattern(source)?;
    let extract = extract_function_declaration(source, "extract")
        .ok_or_else(|| anyhow!("script must define extract() as a function declaration"))?;
    if !extract.contains("title") || !extract.contains("html") {
        bail!("extract() must return an object with title and html fields");
    }

    let clean = extract_function_declaration(source, "clean");
    if let Some(ref clean_src) = clean {
        if !clean_src.contains("removed") {
            bail!("clean() must return {{ removed: number }} — include a removed counter");
        }
    }

    Ok(PagmdScript {
        url_pattern,
        source: source.to_owned(),
        clean,
        extract,
        navigate: extract_function_declaration(source, "navigate"),
        stop: extract_function_declaration(source, "stop"),
        usage: parse_const_string(source, "usage"),
        default_params: parse_const_object(source, "defaultParams").unwrap_or_else(|| json!({})),
        param_help: parse_const_object(source, "paramHelp").unwrap_or_else(|| json!({})),
    })
}

pub fn load_pagemd_script(path: &Path) -> Result<PagmdScript> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse_pagemd_script(&text).with_context(|| format!("invalid script {}", path.display()))
}

/// Human-readable usage for a script (params, hooks, example CLI).
pub fn format_script_usage(path: &Path, script: &PagmdScript) -> String {
    let mut out = String::new();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("script.pagemd.js");
    out.push_str(&format!("Usage: {name}\n"));
    out.push_str(&format!("  path:       {}\n", path.display()));
    out.push_str(&format!(
        "  urlPattern: {}  (override with --url-pattern)\n",
        script.url_pattern
    ));

    let mut hooks = Vec::new();
    if script.clean.is_some() {
        hooks.push("clean");
    }
    hooks.push("extract");
    if script.navigate.is_some() {
        hooks.push("navigate");
    }
    if script.stop.is_some() {
        hooks.push("stop");
    }
    out.push_str(&format!("  hooks:      {}\n", hooks.join(" · ")));

    if let Some(usage) = &script.usage {
        out.push('\n');
        for line in usage.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str("\nParameters (override with --param KEY=VALUE or --params '{…}'):\n");
    let defaults = script.default_params.as_object();
    let helps = script.param_help.as_object();
    let mut keys: Vec<String> = defaults
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    if let Some(helps) = helps {
        for k in helps.keys() {
            if !keys.iter().any(|x| x == k) {
                keys.push(k.clone());
            }
        }
    }
    keys.sort();
    if keys.is_empty() {
        out.push_str(
            "  (none — declare const defaultParams = { … } and/or const paramHelp = { … })\n",
        );
    } else {
        let key_width = keys.iter().map(|k| k.len()).max().unwrap_or(8).max(8);
        for key in &keys {
            let default = defaults
                .and_then(|m| m.get(key))
                .map(|v| compact_json(v))
                .unwrap_or_else(|| "—".into());
            let help = helps
                .and_then(|m| m.get(key))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            out.push_str(&format!(
                "  {key:<key_width$}  default: {default}\n",
                key = key,
                key_width = key_width,
                default = default
            ));
            if !help.is_empty() {
                out.push_str("  ");
                out.push_str(&" ".repeat(key_width));
                out.push_str("  ");
                out.push_str(help);
                out.push('\n');
            }
        }
    }

    out.push_str("\nExample:\n");
    out.push_str(&format!("  pagemd browser script {} \\\n", path.display()));
    out.push_str("    --url <start-url-matching-urlPattern> \\\n");
    out.push_str("    -o out-dir");
    if let Some(defaults) = defaults {
        if let Some((key, value)) = defaults.iter().next() {
            out.push_str(" \\\n");
            out.push_str(&format!("    --param {}={}", key, shell_param_value(value)));
        }
    }
    out.push('\n');
    out.push_str("  # use -o out.md to write one combined Markdown file instead\n");
    out
}

fn compact_json(value: &Value) -> String {
    match value {
        Value::String(s) => format!("{:?}", s),
        other => other.to_string(),
    }
}

fn shell_param_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn url_matches_pattern(url: &str, pattern: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    if !pattern.contains('*') && !pattern.contains('?') {
        return url == pattern || url.starts_with(pattern);
    }
    let mut url_chars = url.chars().peekable();
    let mut pat = pattern.chars().peekable();
    while let Some(pc) = pat.next() {
        match pc {
            '*' => {
                if pat.peek().is_none() {
                    return true;
                }
                let rest: String = pat.collect();
                // Longest suffix-friendly: try every remaining offset.
                let mut probe = url_chars.clone();
                loop {
                    let remaining: String = probe.clone().collect();
                    if url_matches_pattern(&remaining, &rest) {
                        return true;
                    }
                    if probe.next().is_none() {
                        return false;
                    }
                }
            }
            '?' => {
                if url_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                if url_chars.next() != Some(c) {
                    return false;
                }
            }
        }
    }
    url_chars.next().is_none()
}

/// Build a CDP expression that runs one hook with `params` available.
///
/// Runtime binding:
/// - Script may declare `const defaultParams = { … }` in the preamble.
/// - Host merges CLI/host overrides: `params = Object.assign({}, defaultParams, cliParams)`.
/// - Hooks read `params.*`. `stop(context)` also receives `context.params`.
pub fn compile_hook(script: &PagmdScript, kind: HookKind, params: &Value) -> String {
    let preamble = extract_preamble(&script.source);
    let mut blocks = Vec::new();
    if !preamble.is_empty() {
        blocks.push(preamble);
    }
    blocks.push(params_prelude(params));
    if let Some(ref clean) = script.clean {
        blocks.push(clean.clone());
    }
    blocks.push(script.extract.clone());
    if let Some(ref navigate) = script.navigate {
        blocks.push(navigate.clone());
    }
    if let Some(ref stop) = script.stop {
        blocks.push(stop.clone());
    }

    let invoke = match kind {
        HookKind::Clean => {
            r#"if (typeof clean !== "function") return null;
const __r = clean();
return __r && typeof __r === "object" ? __r : { removed: 0 };"#
        }
        HookKind::Extract => r#"return typeof extract === "function" ? extract() : null;"#,
        HookKind::Navigate => {
            r#"return typeof navigate === "function" ? navigate() : { success: false };"#
        }
        HookKind::Stop => {
            r#"const __ctx = Object.assign({}, context, { params });
return typeof stop === "function" ? stop(__ctx) : { shouldStop: false };"#
        }
    };

    let body = format!("{}\n{}", blocks.join("\n\n"), invoke);
    match kind {
        HookKind::Stop => format!("(function(context) {{\n{body}\n}})"),
        _ => format!("(function() {{\n{body}\n}})()"),
    }
}

fn params_prelude(params: &Value) -> String {
    let json = match params {
        Value::Object(_) => params.to_string(),
        Value::Null => "{}".to_owned(),
        other => {
            // Defensive: wrap non-objects so scripts still get an object.
            json!({ "_": other }).to_string()
        }
    };
    format!(
        "const __pagemdCliParams = {json};\n\
         const params = Object.assign(\n\
           {{}},\n\
           (typeof defaultParams !== \"undefined\" && defaultParams && typeof defaultParams === \"object\")\n\
             ? defaultParams : {{}},\n\
           __pagemdCliParams\n\
         );"
    )
}

/// Parse `key=value` (value: JSON if it parses, otherwise string).
pub fn parse_param_kv(raw: &str) -> Result<(String, Value)> {
    let (key, value) = raw
        .split_once('=')
        .ok_or_else(|| anyhow!("--param expects KEY=VALUE, got {raw:?}"))?;
    let key = key.trim();
    if key.is_empty() {
        bail!("--param key must be non-empty");
    }
    Ok((key.to_owned(), parse_param_value(value.trim())))
}

pub fn parse_param_value(raw: &str) -> Value {
    if raw.is_empty() {
        return Value::String(String::new());
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return v;
    }
    Value::String(raw.to_owned())
}

pub fn merge_params_object(base: &mut Value, patch: Value) -> Result<()> {
    let Value::Object(patch_map) = patch else {
        bail!("--params must be a JSON object");
    };
    if !base.is_object() {
        *base = json!({});
    }
    let map = base.as_object_mut().expect("object");
    for (k, v) in patch_map {
        map.insert(k, v);
    }
    Ok(())
}

pub fn save_script(export_dir: &Path, filename: &str, content: &str) -> Result<PathBuf> {
    validate_pagemd_script(content)?;
    std::fs::create_dir_all(export_dir)
        .with_context(|| format!("create {}", export_dir.display()))?;
    let filename = normalize_filename(filename);
    if filename.contains('/') || filename.contains('\\') {
        bail!("filename must not contain path separators");
    }
    let path = export_dir.join(&filename);
    std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

pub fn save_script_tool(export_dir: &Path, args: &serde_json::Value) -> Result<String> {
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("browser_save_script requires content")?;
    let filename = args
        .get("filename")
        .and_then(|v| v.as_str())
        .unwrap_or("untitled.pagemd.js");
    let path = save_script(export_dir, filename, content)?;
    Ok(format!(
        "Saved script -> {}\n({} bytes)",
        path.display(),
        content.len()
    ))
}

/// Run a validated `.pagemd.js` against the live CDP page (clean → extract → navigate loop).
pub async fn run_pagemd_script(
    session: &CdpSession,
    script: &PagmdScript,
    opts: &RunOptions,
) -> Result<RunReport> {
    let current = session.current_url().await.unwrap_or_default();
    let pattern = opts.effective_url_pattern(script);
    if !url_matches_pattern(&current, pattern) {
        bail!("current URL does not match urlPattern\n  url:     {current}\n  pattern: {pattern}");
    }

    let mut pages = Vec::new();
    let mut collected_urls = Vec::new();
    let mut collected_titles = Vec::new();
    let mut extract_errors = 0usize;
    let max_extract_errors = 3usize;
    let stop_reason;
    let dir_mode = !is_file_output(&opts.output);
    let mut used_names = std::collections::HashSet::<String>::new();
    if dir_mode {
        std::fs::create_dir_all(&opts.output)
            .with_context(|| format!("create output dir {}", opts.output.display()))?;
    }

    loop {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let page_url = session.current_url().await.unwrap_or_default();

        // True revisit: landed on a URL already extracted on a previous iteration.
        if collected_urls.iter().any(|u| u == &page_url) {
            stop_reason = format!("URL loop detected ({page_url})");
            break;
        }

        if let Some(ref _clean) = script.clean {
            let expr = compile_hook(script, HookKind::Clean, &opts.params);
            match eval_expression(session, &expr).await {
                Ok(value) => {
                    let removed = value.get("removed").and_then(|v| v.as_u64()).unwrap_or(0);
                    eprintln!("  clean: removed {removed}");
                }
                Err(err) => eprintln!("  clean warning: {err:#}"),
            }
        }

        let extract_expr = compile_hook(script, HookKind::Extract, &opts.params);
        let extract_value = match eval_expression(session, &extract_expr).await {
            Ok(v) => v,
            Err(err) => {
                extract_errors += 1;
                eprintln!("  extract failed ({extract_errors}/{max_extract_errors}): {err:#}");
                if extract_errors >= max_extract_errors {
                    stop_reason =
                        format!("consecutive extract failures reached {max_extract_errors}");
                    break;
                }
                if script.navigate.is_none() {
                    stop_reason = format!("extract failed: {err:#}");
                    break;
                }
                // try navigate onward
                if !try_navigate(session, script, &opts.params).await? {
                    stop_reason = "navigate: no more pages (after extract failure)".into();
                    break;
                }
                sleep_delay(opts.delay_ms).await;
                continue;
            }
        };

        if extract_value.is_null() {
            extract_errors += 1;
            eprintln!("  extract returned null ({extract_errors}/{max_extract_errors})");
            if extract_errors >= max_extract_errors {
                stop_reason = format!("consecutive extract nulls reached {max_extract_errors}");
                break;
            }
            if script.navigate.is_none() {
                stop_reason = "extract returned null".into();
                break;
            }
            if !try_navigate(session, script, &opts.params).await? {
                stop_reason = "navigate: no more pages (after null extract)".into();
                break;
            }
            sleep_delay(opts.delay_ms).await;
            continue;
        }

        extract_errors = 0;
        let title = extract_value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_owned();
        let html = extract_value
            .get("html")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        if html.trim().is_empty() {
            bail!("extract() returned empty html on {page_url}");
        }

        let body_md = snap::html_to_markdown(&html)?;
        let mut markdown = String::new();
        if opts.include_title && !title.is_empty() {
            markdown.push_str("# ");
            markdown.push_str(&title);
            markdown.push_str("\n\n");
        }
        if opts.include_source_url {
            markdown.push_str("> Source: ");
            markdown.push_str(&page_url);
            markdown.push_str("\n\n");
        }
        markdown.push_str(&body_md);
        if !markdown.ends_with('\n') {
            markdown.push('\n');
        }

        pages.push(RunPageResult {
            url: page_url.clone(),
            title: title.clone(),
            markdown: markdown.clone(),
        });
        collected_urls.push(page_url.clone());
        collected_titles.push(title.clone());
        eprintln!(
            "  [{}/{}] {}",
            pages.len(),
            opts.max_pages,
            if title.is_empty() { &page_url } else { &title }
        );

        if dir_mode {
            let file_name = unique_page_filename(pages.len(), &title, &page_url, &mut used_names);
            let path = opts.output.join(&file_name);
            std::fs::write(&path, markdown.as_bytes())
                .with_context(|| format!("write {}", path.display()))?;
        }

        if pages.len() >= opts.max_pages {
            stop_reason = format!("reached max pages ({})", opts.max_pages);
            break;
        }
        if script.navigate.is_none() {
            stop_reason = "single page mode (no navigate hook)".into();
            break;
        }

        if script.stop.is_some() {
            let stop_ctx = json!({
                "currentUrl": page_url,
                "pageIndex": pages.len(),
                "collectedUrls": collected_urls,
                "collectedTitles": collected_titles,
            });
            let stop_expr = compile_hook(script, HookKind::Stop, &opts.params);
            let stop_call = format!(
                "(function() {{ const __fn = {stop_expr}; return typeof __fn === 'function' ? __fn({ctx}) : __fn; }})()",
                ctx = stop_ctx
            );
            match eval_expression(session, &stop_call).await {
                Ok(value) => {
                    if value
                        .get("shouldStop")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        let reason = value
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("condition met");
                        stop_reason = format!("stop hook: {reason}");
                        break;
                    }
                }
                Err(err) => eprintln!("  stop warning: {err:#}"),
            }
        }

        let previous = page_url;
        eprintln!("  navigating…");
        if !try_navigate(session, script, &opts.params).await? {
            stop_reason = "navigate hook: no more pages".into();
            break;
        }
        sleep_delay(opts.delay_ms).await;
        wait_for_url_change(session, &previous, Duration::from_secs(30)).await?;
    }

    if !dir_mode {
        let mut combined = String::new();
        for (i, page) in pages.iter().enumerate() {
            if i > 0 {
                combined.push_str("\n\n---\n\n");
            }
            combined.push_str(&page.markdown);
        }
        if let Some(parent) = opts.output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
        }
        std::fs::write(&opts.output, combined.as_bytes())
            .with_context(|| format!("write {}", opts.output.display()))?;
    }

    Ok(RunReport {
        pages,
        stop_reason,
        output: opts.output.clone(),
    })
}

async fn try_navigate(session: &CdpSession, script: &PagmdScript, params: &Value) -> Result<bool> {
    let Some(_) = script.navigate else {
        return Ok(false);
    };
    let expr = compile_hook(script, HookKind::Navigate, params);
    let value = eval_expression(session, &expr).await?;
    Ok(value
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

async fn eval_expression(session: &CdpSession, expression: &str) -> Result<Value> {
    session.evaluate(expression, true).await
}

async fn sleep_delay(range: (u64, u64)) {
    let (lo, hi) = if range.0 <= range.1 {
        range
    } else {
        (range.1, range.0)
    };
    let ms = if lo == hi {
        lo
    } else {
        lo + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            % (hi - lo + 1))
    };
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

async fn wait_for_url_change(session: &CdpSession, previous: &str, max: Duration) -> Result<()> {
    let deadline = tokio::time::Instant::now() + max;
    loop {
        let current = session.current_url().await.unwrap_or_default();
        if current != previous && !current.is_empty() {
            // Give the new document a moment to settle.
            let _ = session.evaluate("document.readyState", false).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("timed out waiting for navigation away from {previous}");
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

fn parse_url_pattern(source: &str) -> Result<String> {
    parse_const_string(source, "urlPattern")
        .ok_or_else(|| anyhow!("missing urlPattern (expected: const urlPattern = \"...\")"))
}

fn parse_const_string(source: &str, name: &str) -> Option<String> {
    let markers = [
        format!("const {name}"),
        format!("let {name}"),
        format!("var {name}"),
    ];
    for marker in &markers {
        for line in source.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix(marker.as_str()) else {
                continue;
            };
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                continue;
            };
            let rest = rest.trim_start();
            let Some(quote) = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '\'' | '"' | '`'))
            else {
                continue;
            };
            let body = &rest[quote.len_utf8()..];
            let Some(end) = find_string_end(body, quote) else {
                continue;
            };
            return Some(body[..end].to_owned());
        }
    }

    // Multi-line: scan every `name = "…"` occurrence (skip false positives like `--usage`).
    let mut search_from = 0;
    while let Some(rel) = source[search_from..].find(name) {
        let idx = search_from + rel;
        // Prefer word-ish boundary: previous char not alphanumeric/_/-
        if idx > 0 {
            let prev = source[..idx].chars().next_back().unwrap_or('\0');
            if prev.is_ascii_alphanumeric() || prev == '_' || prev == '-' {
                search_from = idx + name.len();
                continue;
            }
        }
        let after = source[idx + name.len()..].trim_start();
        if let Some(after_eq) = after.strip_prefix('=') {
            let after_eq = after_eq.trim_start();
            if let Some(quote) = after_eq
                .chars()
                .next()
                .filter(|c| matches!(c, '\'' | '"' | '`'))
            {
                let body = &after_eq[quote.len_utf8()..];
                if let Some(end) = find_string_end(body, quote) {
                    return Some(body[..end].to_owned());
                }
            }
        }
        search_from = idx + name.len();
    }
    None
}

fn parse_const_object(source: &str, name: &str) -> Option<Value> {
    let patterns = [
        format!("const {name}"),
        format!("let {name}"),
        format!("var {name}"),
    ];
    for pat in &patterns {
        if let Some(idx) = source.find(pat.as_str()) {
            let after = source[idx + pat.len()..].trim_start();
            let Some(after) = after.strip_prefix('=') else {
                continue;
            };
            let after = after.trim_start();
            if !after.starts_with('{') {
                continue;
            }
            let open = source[idx + pat.len()..]
                .find('{')
                .map(|o| idx + pat.len() + o)?;
            let close = find_matching_brace(source, open)?;
            let literal = &source[open..=close];
            if let Some(value) = js_object_literal_to_json(literal) {
                return Some(value);
            }
        }
    }
    None
}

/// Best-effort conversion of a simple JS object literal to JSON.
fn js_object_literal_to_json(literal: &str) -> Option<Value> {
    let stripped = strip_js_comments(literal);
    let jsonish = quote_js_object_keys(&stripped)?;
    let no_trailing = strip_trailing_commas(&jsonish);
    serde_json::from_str(&no_trailing).ok()
}

fn strip_js_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            out.push(b as char);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        if b == b'\'' || b == b'"' || b == b'`' {
            in_str = Some(b);
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
        }
        out.push(b as char);
        i += 1;
    }
    out
}

fn quote_js_object_keys(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + 16);
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if q == b'\'' {
                // Convert single-quoted strings to JSON double-quoted.
                if escaped {
                    out.push('\\');
                    out.push(b as char);
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'\'' {
                    out.push('"');
                    in_str = None;
                } else if b == b'"' {
                    out.push_str("\\\"");
                } else {
                    out.push(b as char);
                }
            } else {
                out.push(b as char);
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == q {
                    in_str = None;
                }
            }
            i += 1;
            continue;
        }
        if b == b'\'' || b == b'"' {
            if b == b'\'' {
                out.push('"');
            } else {
                out.push('"');
            }
            in_str = Some(b);
            i += 1;
            continue;
        }
        if b == b'`' {
            // Unsupported in param objects for --usage parsing.
            return None;
        }
        // Unquoted identifier key: <ident><ws>:
        if (b as char).is_ascii_alphabetic() || b == b'_' || b == b'$' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let c = bytes[i] as char;
                if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                    i += 1;
                } else {
                    break;
                }
            }
            let ident = &input[start..i];
            let mut j = i;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                out.push('"');
                out.push_str(ident);
                out.push('"');
                // keep whitespace then ':' via normal path from i
                continue;
            }
            out.push_str(ident);
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    Some(out)
}

fn strip_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            out.push(b as char);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_str = Some(b);
            out.push('"');
            i += 1;
            continue;
        }
        if b == b',' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b'}' || bytes[j] == b']') {
                i += 1;
                continue;
            }
        }
        out.push(b as char);
        i += 1;
    }
    out
}

fn find_string_end(body: &str, quote: char) -> Option<usize> {
    let mut escaped = false;
    for (i, ch) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(i);
        }
    }
    None
}

fn extract_function_declaration(source: &str, name: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r"(?m)function\s+{name}\s*\([^)]*\)\s*\{{"))
        .expect("hook regex");
    let m = re.find(source)?;
    let brace_start = source[m.start()..].find('{')? + m.start();
    let brace_end = find_matching_brace(source, brace_start)?;
    Some(source[m.start()..=brace_end].trim().to_owned())
}

fn find_matching_brace(source: &str, open_index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0isize;
    let mut i = open_index;
    let mut in_str: Option<u8> = None;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => in_str = Some(b),
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn extract_preamble(source: &str) -> String {
    let mut preamble = source.to_owned();
    for name in ["clean", "extract", "navigate", "stop"] {
        if let Some(decl) = extract_function_declaration(source, name) {
            preamble = preamble.replacen(&decl, "", 1);
        }
    }
    preamble.trim().to_owned()
}

/// Parse `/run` delay: `MS` or `MIN:MAX`.
pub fn parse_delay(raw: &str) -> Result<(u64, u64)> {
    if let Some((a, b)) = raw.split_once(':') {
        let lo: u64 = a.parse().context("invalid --delay min")?;
        let hi: u64 = b.parse().context("invalid --delay max")?;
        return Ok((lo, hi));
    }
    let ms: u64 = raw.parse().context("invalid --delay")?;
    Ok((ms, ms))
}

pub fn default_output_for_script(script_path: &Path, cwd: &Path) -> PathBuf {
    let stem = script_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("pagemd-run")
        .trim_end_matches(".pagemd")
        .to_owned();
    cwd.join(format!("{stem}-run"))
}

/// `-o out.md` → one combined file; otherwise treat as an output directory.
pub fn is_file_output(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "txt"))
        .unwrap_or(false)
}

fn page_file_stem(index: usize, title: &str, url: &str) -> String {
    let mut slug = crate::core::util::slugify(title);
    if slug.is_empty() {
        slug = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .map(crate::core::util::slugify)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "page".to_owned());
    }
    const MAX: usize = 80;
    if slug.chars().count() > MAX {
        slug = slug.chars().take(MAX).collect();
        slug = slug.trim_end_matches('-').to_owned();
    }
    format!("{index:03}-{slug}")
}

fn unique_page_filename(
    index: usize,
    title: &str,
    url: &str,
    used: &mut std::collections::HashSet<String>,
) -> String {
    let stem = page_file_stem(index, title, url);
    let mut name = format!("{stem}.md");
    let mut n = 2usize;
    while used.contains(&name) {
        name = format!("{stem}-{n}.md");
        n += 1;
    }
    used.insert(name.clone());
    name
}

#[derive(Clone, Debug)]
pub enum ParsedRunArgs {
    Usage { script: PathBuf },
    Run { script: PathBuf, opts: RunOptions },
}

/// Parse `/run` flags: `/run <file> [--usage] [-o out.md] [--param K=V]…`
pub fn parse_run_args(rest: &str, cwd: &Path) -> Result<ParsedRunArgs> {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.is_empty() {
        bail!(
            "usage: /run <file.pagemd.js> [--usage] [--url-pattern GLOB] [-o out-dir|.md] [--param KEY=VALUE]…"
        );
    }

    let mut script_path: Option<PathBuf> = None;
    let mut opts = RunOptions::default();
    let mut dump_usage = false;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "--usage" | "--help-script" => dump_usage = true,
            "--url-pattern" => {
                i += 1;
                let pat = tokens
                    .get(i)
                    .ok_or_else(|| anyhow!("--url-pattern requires a glob"))?;
                opts.url_pattern = Some((*pat).to_owned());
            }
            "-o" | "--output" => {
                i += 1;
                let path = tokens.get(i).ok_or_else(|| anyhow!("-o requires a path"))?;
                opts.output = PathBuf::from(*path);
            }
            "--max-pages" => {
                i += 1;
                let n = tokens
                    .get(i)
                    .ok_or_else(|| anyhow!("--max-pages requires a number"))?
                    .parse::<usize>()
                    .context("invalid --max-pages")?;
                if n == 0 {
                    bail!("--max-pages must be > 0");
                }
                opts.max_pages = n;
            }
            "--delay" => {
                i += 1;
                let raw = tokens
                    .get(i)
                    .ok_or_else(|| anyhow!("--delay requires MS or MIN:MAX"))?;
                opts.delay_ms = parse_delay(raw)?;
            }
            "--param" | "-p" => {
                i += 1;
                let raw = tokens
                    .get(i)
                    .ok_or_else(|| anyhow!("--param requires KEY=VALUE"))?;
                let (key, value) = parse_param_kv(raw)?;
                merge_params_object(&mut opts.params, json!({ key: value }))?;
            }
            "--params" => {
                i += 1;
                let raw = tokens
                    .get(i)
                    .ok_or_else(|| anyhow!("--params requires a JSON object"))?;
                let patch: Value = serde_json::from_str(raw).context("invalid --params JSON")?;
                merge_params_object(&mut opts.params, patch)?;
            }
            "--no-title" => opts.include_title = false,
            "--no-source" => opts.include_source_url = false,
            flag if flag.starts_with('-') => bail!("unknown flag: {flag}"),
            path => {
                if script_path.is_some() {
                    bail!("unexpected argument: {path}");
                }
                script_path = Some(PathBuf::from(path));
            }
        }
        i += 1;
    }

    let script_path =
        script_path.ok_or_else(|| anyhow!("usage: /run <file.pagemd.js> [--usage] [-o out.md]"))?;
    let script_path = if script_path.is_absolute() {
        script_path
    } else {
        cwd.join(script_path)
    };

    if dump_usage {
        return Ok(ParsedRunArgs::Usage {
            script: script_path,
        });
    }

    if opts.output.as_os_str() == "pagemd-run" || opts.output.as_os_str() == "pagemd-run.md" {
        opts.output = default_output_for_script(&script_path, cwd);
    } else if opts.output.is_relative() {
        opts.output = cwd.join(&opts.output);
    }

    Ok(ParsedRunArgs::Run {
        script: script_path,
        opts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pagemd-{name}-{id}"))
    }

    const SAMPLE: &str = r#"
const urlPattern = "https://example.com/*";
const X = 1;
function clean() { let removed = 0; return { removed }; }
function extract() { return { title: document.title, html: document.body.innerHTML }; }
function navigate() { return { success: false }; }
function stop(context) { return { shouldStop: false }; }
"#;

    #[test]
    fn normalize_filename_adds_suffix() {
        assert_eq!(normalize_filename("github"), "github.pagemd.js");
        assert_eq!(normalize_filename("x.pagemd.js"), "x.pagemd.js");
    }

    #[test]
    fn parse_requires_extract_and_url_pattern() {
        parse_pagemd_script(SAMPLE).unwrap();
        assert!(
            parse_pagemd_script("function extract() { return { title: 'a', html: 'b' }; }")
                .is_err()
        );
    }

    #[test]
    fn url_glob_matching() {
        assert!(url_matches_pattern(
            "https://example.com/a/b",
            "https://example.com/*"
        ));
        assert!(!url_matches_pattern(
            "https://other.com/a",
            "https://example.com/*"
        ));
        assert!(url_matches_pattern(
            "https://example.com/docs",
            "https://example.com/docs"
        ));
    }

    #[test]
    fn compile_includes_preamble_helpers() {
        let script = parse_pagemd_script(SAMPLE).unwrap();
        let js = compile_hook(&script, HookKind::Extract, &json!({}));
        assert!(js.contains("const X = 1"));
        assert!(js.contains("function extract"));
        assert!(js.contains("return typeof extract"));
        assert!(js.contains("const params = Object.assign"));
    }

    #[test]
    fn compile_merges_cli_params() {
        let source = r#"
const urlPattern = "https://example.com/*";
const defaultParams = { stopUrl: "https://example.com/end", noise: ["nav"] };
function extract() { return { title: params.stopUrl, html: "<p>x</p>" }; }
"#;
        let script = parse_pagemd_script(source).unwrap();
        let js = compile_hook(
            &script,
            HookKind::Extract,
            &json!({ "stopUrl": "https://example.com/override" }),
        );
        assert!(js.contains("defaultParams"));
        assert!(js.contains("https://example.com/override"));
        assert!(js.contains("__pagemdCliParams"));
    }

    #[test]
    fn parse_param_kv_json_and_string() {
        let (k, v) = parse_param_kv("count=3").unwrap();
        assert_eq!(k, "count");
        assert_eq!(v, json!(3));
        let (k, v) = parse_param_kv(r#"label=hello world"#).unwrap();
        assert_eq!(k, "label");
        assert_eq!(v, json!("hello world"));
        let (k, v) = parse_param_kv(r#"tags=["a","b"]"#).unwrap();
        assert_eq!(k, "tags");
        assert_eq!(v, json!(["a", "b"]));
    }

    #[test]
    fn parse_run_args_accepts_params() {
        let cwd = Path::new("/tmp");
        let parsed = parse_run_args(
            r#"site.pagemd.js --param stopUrl=https://x.test/end --params {"keepNav":true}"#,
            cwd,
        )
        .unwrap();
        let ParsedRunArgs::Run { opts, .. } = parsed else {
            panic!("expected run");
        };
        assert_eq!(opts.params["stopUrl"], json!("https://x.test/end"));
        assert_eq!(opts.params["keepNav"], json!(true));
    }

    #[test]
    fn parse_run_args_url_pattern_override() {
        let cwd = Path::new("/tmp");
        let parsed =
            parse_run_args("site.pagemd.js --url-pattern https://other.test/*", cwd).unwrap();
        let ParsedRunArgs::Run { opts, .. } = parsed else {
            panic!("expected run");
        };
        assert_eq!(opts.url_pattern.as_deref(), Some("https://other.test/*"));
        let script = parse_pagemd_script(
            r#"
const urlPattern = "https://example.com/*";
function extract() { return { title: "t", html: "<p>x</p>" }; }
"#,
        )
        .unwrap();
        assert_eq!(opts.effective_url_pattern(&script), "https://other.test/*");
    }

    #[test]
    fn parse_run_args_usage_flag() {
        let cwd = Path::new("/tmp");
        let parsed = parse_run_args("site.pagemd.js --usage", cwd).unwrap();
        match parsed {
            ParsedRunArgs::Usage { script } => {
                assert_eq!(script, PathBuf::from("/tmp/site.pagemd.js"));
            }
            ParsedRunArgs::Run { .. } => panic!("expected usage"),
        }
    }

    #[test]
    fn format_usage_lists_params() {
        let source = r#"
const urlPattern = "https://example.com/docs/*";
const usage = "Crawl docs into Markdown.";
const defaultParams = {
  stopUrl: "https://example.com/docs/end",
  contentSelector: "article",
};
const paramHelp = {
  stopUrl: "Stop after this page",
  contentSelector: "Main content selector",
};
function clean() { return { removed: 0 }; }
function extract() { return { title: document.title, html: "<p>x</p>" }; }
function navigate() { return { success: false }; }
function stop(context) { return { shouldStop: false }; }
"#;
        let script = parse_pagemd_script(source).unwrap();
        let path = Path::new("docs.pagemd.js");
        let text = format_script_usage(path, &script);
        assert!(text.contains("urlPattern:"));
        assert!(text.contains("stopUrl"));
        assert!(text.contains("--param"));
        assert_eq!(
            script.default_params.get("stopUrl"),
            Some(&json!("https://example.com/docs/end"))
        );
        assert_eq!(script.usage.as_deref(), Some("Crawl docs into Markdown."));
    }

    #[test]
    fn parse_run_args_defaults_output_from_stem() {
        let cwd = Path::new("/tmp");
        let parsed = parse_run_args("site.pagemd.js --max-pages 3", cwd).unwrap();
        let ParsedRunArgs::Run { script: path, opts } = parsed else {
            panic!("expected run");
        };
        assert_eq!(path, PathBuf::from("/tmp/site.pagemd.js"));
        assert_eq!(opts.max_pages, 3);
        assert_eq!(opts.output, PathBuf::from("/tmp/site-run"));
    }

    #[test]
    fn file_vs_dir_output_detection() {
        assert!(is_file_output(Path::new("out.md")));
        assert!(is_file_output(Path::new("/tmp/x.markdown")));
        assert!(!is_file_output(Path::new("out-dir")));
        assert!(!is_file_output(Path::new("out-dir/")));
        assert_eq!(
            page_file_stem(1, "Hello World!", "https://x/a"),
            "001-hello-world"
        );
    }

    #[test]
    fn parse_script_with_default_params_and_hooks() {
        let source = r#"
const urlPattern = "https://cloud.tencent.com/document/product/1095/*";
const defaultParams = {
  stopUrl: "https://cloud.tencent.com/document/product/1095/94313",
  contentSelector: "article",
};
function clean() { return { removed: 0 }; }
function extract() {
  const el = document.querySelector(params.contentSelector);
  return el ? { title: document.title, html: el.innerHTML } : null;
}
function navigate() { return { success: false }; }
function stop(context) { return { shouldStop: false }; }
"#;
        let script = parse_pagemd_script(source).expect("parse sample");
        assert!(script.url_pattern.contains("cloud.tencent.com"));
        assert!(script.clean.is_some());
        assert!(script.navigate.is_some());
        assert!(script.stop.is_some());
        assert!(!script.extract.is_empty());
        let js = compile_hook(&script, HookKind::Extract, &json!({ "stopUrl": "x" }));
        assert!(js.contains("defaultParams"));
        assert!(js.contains("params.contentSelector") || js.contains("params"));
    }

    #[test]
    fn save_script_writes_under_export_dir() {
        let dir = temp_dir("export-script");
        std::fs::create_dir_all(&dir).unwrap();
        let source = r#"
const urlPattern = "https://example.com/*";
function extract() { return { title: "t", html: "<p>x</p>" }; }
"#;
        let path = save_script(&dir, "site", source).unwrap();
        assert_eq!(path, dir.join("site.pagemd.js"));
        assert!(path.is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
