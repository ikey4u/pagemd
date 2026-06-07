use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};
use serde_json::Value;

/// Read NDJSON lines from agent stdout and render progress in the pagemd terminal.
pub fn render_stream_json(reader: impl BufRead) -> Result<String> {
    let mut final_text = String::new();
    let mut streamed = false;
    let mut thinking = false;
    let mut assistant_open = false;

    for line in reader.lines() {
        let line = line.context("read agent stdout")?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse agent stream-json line: {line}"))?;

        match event.get("type").and_then(|v| v.as_str()) {
            Some("system") => {
                if event.get("subtype").and_then(|v| v.as_str()) == Some("init") {
                    let model = event
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    eprintln!("[agent] model: {model}");
                    io::stderr().flush()?;
                }
            }
            Some("thinking") => {
                if show_thinking() {
                    render_thinking(&event, &mut thinking)?;
                }
            }
            Some("assistant") => {
                if should_skip_assistant_event(&event) {
                    continue;
                }
                if let Some(delta) = assistant_delta_text(&event) {
                    if !assistant_open {
                        eprint!("\n[assistant] ");
                        assistant_open = true;
                    }
                    print!("{delta}");
                    io::stdout().flush()?;
                    streamed = true;
                }
            }
            Some("tool_call") => {
                if assistant_open {
                    println!();
                    assistant_open = false;
                }
                render_tool_call(&event)?;
            }
            Some("result") => {
                if assistant_open {
                    println!();
                    assistant_open = false;
                }
                if let Some(text) = event.get("result").and_then(|v| v.as_str()) {
                    final_text = text.to_string();
                }
                if !streamed {
                    if !final_text.is_empty() {
                        eprintln!("\n[assistant]");
                        println!("{final_text}");
                    }
                }
                if let Some(ms) = event.get("duration_ms").and_then(|v| v.as_u64()) {
                    eprintln!("[agent] done ({ms} ms)");
                } else {
                    eprintln!("[agent] done");
                }
                io::stderr().flush()?;
                if event.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
                    let msg = if final_text.trim().is_empty() {
                        "unknown"
                    } else {
                        final_text.as_str()
                    };
                    anyhow::bail!("agent error: {msg}");
                }
            }
            _ => {}
        }
    }

    Ok(final_text)
}

/// Per Cursor stream-json docs: only append deltas with `timestamp_ms` and no `model_call_id`.
fn should_skip_assistant_event(event: &Value) -> bool {
    if event.get("model_call_id").is_some() {
        return true;
    }
    event.get("timestamp_ms").is_none()
}

fn render_thinking(event: &Value, active: &mut bool) -> Result<()> {
    match event.get("subtype").and_then(|v| v.as_str()) {
        Some("delta") => {
            if let Some(text) = event.get("text").and_then(|v| v.as_str()) {
                if !*active {
                    eprint!("\n[thinking] ");
                    *active = true;
                }
                eprint!("{text}");
                io::stderr().flush()?;
            }
        }
        Some("completed") => {
            if *active {
                eprintln!();
                *active = false;
            }
        }
        _ => {}
    }
    Ok(())
}

fn render_tool_call(event: &Value) -> Result<()> {
    let subtype = event.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
    let Some((kind, variant)) = tool_call_variant(event) else {
        return Ok(());
    };
    let label = tool_call_label(kind, variant);

    match subtype {
        "started" => {
            eprint!("[agent] → {label}");
            if let Some(args) = tool_call_started_detail(kind, variant) {
                eprint!(" {args}");
            }
            eprintln!();
            if verbose_tools() {
                if let Some(detail) = tool_call_verbose_args(variant) {
                    for line in detail.lines() {
                        eprintln!("         {line}");
                    }
                }
            }
            io::stderr().flush()?;
        }
        "completed" => {
            let detail = tool_call_completed_detail(kind, variant);
            if detail.is_empty() {
                eprintln!("[agent] ← {label}");
            } else {
                eprintln!("[agent] ← {label} ({detail})");
            }
            if verbose_tools() {
                if let Some(detail) = tool_call_verbose_result(variant) {
                    for line in detail.lines() {
                        eprintln!("         {line}");
                    }
                }
            }
            io::stderr().flush()?;
        }
        _ => {}
    }
    Ok(())
}

fn tool_call_variant<'a>(event: &'a Value) -> Option<(&'a str, &'a Value)> {
    event
        .get("tool_call")?
        .as_object()?
        .iter()
        .next()
        .map(|(kind, variant)| (kind.as_str(), variant))
}

fn tool_call_label(kind: &str, variant: &Value) -> String {
    match kind {
        "mcpToolCall" => mcp_tool_label(variant).unwrap_or_else(|| "mcp".to_string()),
        "shellToolCall" => "shell".to_string(),
        "readToolCall" => "read".to_string(),
        "writeToolCall" => "write".to_string(),
        "editToolCall" => "edit".to_string(),
        "grepToolCall" => "grep".to_string(),
        "globToolCall" => "glob".to_string(),
        "lsToolCall" => "ls".to_string(),
        "deleteToolCall" => "delete".to_string(),
        "listMcpResourcesToolCall" => "list_mcp_resources".to_string(),
        "readMcpResourceToolCall" => "read_mcp_resource".to_string(),
        other => other.trim_end_matches("ToolCall").to_string(),
    }
}

fn tool_call_payload(variant: &Value) -> &Value {
    variant.get("args").unwrap_or(variant)
}

fn mcp_tool_label(variant: &Value) -> Option<String> {
    let payload = tool_call_payload(variant);
    let provider = payload
        .get("providerIdentifier")
        .or_else(|| payload.get("provider"))
        .or_else(|| payload.get("server"))
        .and_then(|v| v.as_str())
        .unwrap_or("mcp");
    let tool = payload
        .get("toolName")
        .or_else(|| payload.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("tool");
    Some(format!("{provider}.{tool}"))
}

fn tool_call_started_detail(kind: &str, variant: &Value) -> Option<String> {
    match kind {
        "mcpToolCall" => {
            let args = mcp_tool_arguments(variant)?;
            Some(format_mcp_arguments(&args))
        }
        "shellToolCall" => variant
            .pointer("/args/command")
            .and_then(|v| v.as_str())
            .map(|cmd| truncate_preview(cmd, 96)),
        "readToolCall" | "writeToolCall" | "editToolCall" | "deleteToolCall" => variant
            .pointer("/args/path")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        "grepToolCall" => {
            let pattern = variant
                .pointer("/args/pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let path = variant
                .pointer("/args/path")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            Some(format!("{pattern} @ {path}"))
        }
        "globToolCall" => {
            let pattern = variant
                .pointer("/args/globPattern")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            Some(pattern.to_string())
        }
        "lsToolCall" => variant
            .pointer("/args/path")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        "listMcpResourcesToolCall" => variant
            .pointer("/args/server")
            .or_else(|| variant.pointer("/args/providerIdentifier"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        "readMcpResourceToolCall" => {
            let uri = variant
                .pointer("/args/uri")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            Some(uri.to_string())
        }
        _ => generic_args_summary(tool_call_payload(variant)),
    }
}

fn tool_call_completed_detail(kind: &str, variant: &Value) -> String {
    match kind {
        "mcpToolCall" => mcp_result_summary(variant),
        "shellToolCall" => {
            if variant.pointer("/result/success/exitCode").is_some() {
                format!(
                    "exit {}",
                    variant
                        .pointer("/result/success/exitCode")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                )
            } else if variant.get("result").and_then(|v| v.get("error")).is_some() {
                "failed".to_string()
            } else {
                String::new()
            }
        }
        "readToolCall" => variant
            .pointer("/result/success/totalLines")
            .and_then(|v| v.as_u64())
            .map(|n| format!("{n} lines"))
            .unwrap_or_else(|| "failed".to_string()),
        "writeToolCall" => variant
            .pointer("/result/success/linesCreated")
            .and_then(|v| v.as_u64())
            .map(|n| format!("{n} lines written"))
            .unwrap_or_else(|| "failed".to_string()),
        "grepToolCall" => variant
            .pointer("/result/success/totalMatchedLines")
            .or_else(|| {
                variant
                    .pointer("/result/success/workspaceResults")
                    .and_then(|v| v.as_object())
                    .and_then(|obj| obj.values().next())
                    .and_then(|v| v.pointer("/content/totalMatchedLines"))
            })
            .and_then(|v| v.as_u64())
            .map(|n| format!("{n} matches"))
            .unwrap_or_else(|| "failed".to_string()),
        "globToolCall" => variant
            .pointer("/result/success/totalFiles")
            .and_then(|v| v.as_u64())
            .map(|n| format!("{n} files"))
            .unwrap_or_else(|| "failed".to_string()),
        "editToolCall" | "deleteToolCall" => {
            if variant.pointer("/result/success").is_some() {
                "ok".to_string()
            } else if variant.pointer("/result/rejected").is_some() {
                "rejected".to_string()
            } else {
                "failed".to_string()
            }
        }
        _ => generic_result_summary(variant),
    }
}

fn mcp_tool_arguments(variant: &Value) -> Option<&Value> {
    let payload = tool_call_payload(variant);
    payload
        .get("arguments")
        .or_else(|| payload.get("args"))
        .filter(|v| !v.is_null())
}

fn format_mcp_arguments(args: &Value) -> String {
    if let Some(expr) = args.get("expression").and_then(|v| v.as_str()) {
        return format!("expression={}", truncate_preview(expr, 72));
    }
    if let Some(url) = args.get("url").and_then(|v| v.as_str()) {
        return format!("url={}", truncate_preview(url, 72));
    }
    if let Some(filename) = args.get("filename").and_then(|v| v.as_str()) {
        return format!("filename={filename}");
    }
    if let Some(hint) = args.get("hint").and_then(|v| v.as_str()) {
        return format!("hint={}", truncate_preview(hint, 72));
    }
    if args.as_object().is_some_and(|obj| obj.is_empty()) {
        return "()".to_string();
    }
    generic_args_summary(args).unwrap_or_else(|| args.to_string())
}

fn mcp_result_summary(variant: &Value) -> String {
    let result = match variant.get("result") {
        Some(v) => v,
        None => return String::new(),
    };
    if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
        return format!("error: {}", truncate_preview(err, 80));
    }
    if result.get("rejected").is_some() {
        let reason = result
            .pointer("/rejected/reason")
            .and_then(|v| v.as_str())
            .unwrap_or("rejected");
        return format!("rejected: {reason}");
    }
    if let Some(success) = result.get("success") {
        if let Some(text) = extract_text_content(success) {
            return truncate_preview(&text, 96);
        }
        if success.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            return "ok".to_string();
        }
        if success.is_string() {
            return truncate_preview(success.as_str().unwrap_or(""), 96);
        }
        if success.is_object() && !success.as_object().unwrap().is_empty() {
            return "ok".to_string();
        }
    }
    String::new()
}

fn extract_text_content(value: &Value) -> Option<String> {
    if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        return Some(text.to_string());
    }
    if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
        let parts: Vec<String> = content
            .iter()
            .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
            .map(str::to_string)
            .collect();
        if !parts.is_empty() {
            return Some(parts.join("\n"));
        }
    }
    None
}

fn generic_args_summary(args: &Value) -> Option<String> {
    let obj = args.as_object()?;
    if obj.is_empty() {
        return Some("()".to_string());
    }
    let parts: Vec<String> = obj
        .iter()
        .take(4)
        .map(|(key, value)| format!("{key}={}", format_arg_value(value)))
        .collect();
    let mut summary = parts.join(", ");
    if obj.len() > 4 {
        summary.push_str(", …");
    }
    Some(summary)
}

fn format_arg_value(value: &Value) -> String {
    match value {
        Value::String(s) => truncate_preview(s, 48),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => format!("[{} items]", arr.len()),
        Value::Object(obj) => format!("{{{} fields}}", obj.len()),
    }
}

fn generic_result_summary(variant: &Value) -> String {
    let result = match variant.get("result") {
        Some(v) => v,
        None => return String::new(),
    };
    if result.get("success").is_some() {
        "ok".to_string()
    } else if result.get("error").is_some() {
        "error".to_string()
    } else {
        String::new()
    }
}

fn tool_call_verbose_args(variant: &Value) -> Option<String> {
    let args = mcp_tool_arguments(variant)?;
    serde_json::to_string_pretty(args)
        .ok()
        .map(|s| truncate_preview(&s, 800))
}

fn tool_call_verbose_result(variant: &Value) -> Option<String> {
    variant
        .get("result")
        .and_then(|v| serde_json::to_string_pretty(v).ok())
        .map(|s| truncate_preview(&s, 800))
}

fn assistant_delta_text(event: &Value) -> Option<String> {
    let content = event.pointer("/message/content")?.as_array()?;
    let mut text = String::new();
    for item in content {
        if item.get("type").and_then(|v| v.as_str()) != Some("text") {
            continue;
        }
        if let Some(part) = item.get("text").and_then(|v| v.as_str()) {
            text.push_str(part);
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn truncate_preview(text: &str, max: usize) -> String {
    let compact: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max {
        compact
    } else {
        format!("{}…", compact.chars().take(max).collect::<String>())
    }
}

fn show_thinking() -> bool {
    match std::env::var("PAGEMD_SHOW_THINKING") {
        Ok(v) if v == "0" || v.eq_ignore_ascii_case("false") => false,
        Ok(v) if v == "1" || v.eq_ignore_ascii_case("true") => true,
        Ok(_) => true,
        Err(_) => true,
    }
}

fn verbose_tools() -> bool {
    match std::env::var("PAGEMD_VERBOSE_TOOLS") {
        Ok(v) if v == "0" || v.eq_ignore_ascii_case("false") => false,
        Ok(v) if v == "1" || v.eq_ignore_ascii_case("true") => true,
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn streams_assistant_deltas_and_finalizes() {
        let input = "\
{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Hi\"}]},\"timestamp_ms\":1}\n\
{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Hi there\"}]}}\n\
{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"Hi there\",\"duration_ms\":10}\n";
        let text = render_stream_json(Cursor::new(input)).unwrap();
        assert_eq!(text, "Hi there");
    }

    #[test]
    fn skips_buffered_duplicate_assistant_events() {
        let input = "\
{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"[Image #1]\"}]},\"timestamp_ms\":1}\n\
{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"[Image #1]\"}]},\"timestamp_ms\":2,\"model_call_id\":\"call_1\"}\n\
{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"[Image #1]\"}]}}\n\
{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"[Image #1]\"}\n";
        let text = render_stream_json(Cursor::new(input)).unwrap();
        assert_eq!(text, "[Image #1]");
    }

    #[test]
    fn renders_mcp_tool_call_with_args_and_result() {
        let input = "\
{\"type\":\"tool_call\",\"subtype\":\"started\",\"tool_call\":{\"mcpToolCall\":{\"args\":{\"providerIdentifier\":\"pagemd-browser\",\"toolName\":\"browser_eval\",\"arguments\":{\"expression\":\"document.title\"}}}}}\n\
{\"type\":\"tool_call\",\"subtype\":\"completed\",\"tool_call\":{\"mcpToolCall\":{\"args\":{\"providerIdentifier\":\"pagemd-browser\",\"toolName\":\"browser_eval\",\"arguments\":{\"expression\":\"document.title\"}},\"result\":{\"success\":{\"content\":[{\"type\":\"text\",\"text\":\"雪球 - 文章标题\"}]}}}}}\n\
{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"done\"}\n";
        let text = render_stream_json(Cursor::new(input)).unwrap();
        assert_eq!(text, "done");
    }

    #[test]
    fn mcp_tool_label_includes_provider_and_tool() {
        let event: Value = serde_json::from_str(
            r#"{"tool_call":{"mcpToolCall":{"args":{"providerIdentifier":"pagemd-browser","toolName":"browser_snap"}}}}"#,
        )
        .unwrap();
        let (_, variant) = tool_call_variant(&event).unwrap();
        assert_eq!(
            mcp_tool_label(variant).as_deref(),
            Some("pagemd-browser.browser_snap")
        );
    }
}
