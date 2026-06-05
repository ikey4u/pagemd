fn canonical_callout_kind(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "note" => Some("note"),
        "abstract" | "summary" | "tldr" => Some("abstract"),
        "info" | "todo" => Some("info"),
        "tip" | "hint" => Some("tip"),
        "success" | "check" | "done" => Some("success"),
        "question" | "help" | "faq" => Some("question"),
        "warning" | "warn" | "attention" => Some("warning"),
        "failure" | "fail" | "missing" => Some("failure"),
        "danger" | "error" => Some("danger"),
        "bug" => Some("bug"),
        "example" => Some("example"),
        "quote" | "cite" => Some("quote"),
        "important" => Some("important"),
        "caution" => Some("caution"),
        _ => None,
    }
}

pub(crate) fn callout_label(kind: &str) -> &'static str {
    match kind {
        "note" => "Note",
        "abstract" => "Abstract",
        "info" => "Info",
        "tip" => "Tip",
        "success" => "Success",
        "question" => "Question",
        "warning" => "Warning",
        "failure" => "Failure",
        "danger" => "Danger",
        "bug" => "Bug",
        "example" => "Example",
        "quote" => "Quote",
        "important" => "Important",
        "caution" => "Caution",
        _ => "Note",
    }
}

fn parse_callout_marker(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    if !trimmed.starts_with("[!") {
        return None;
    }
    let end = trimmed.find(']')?;
    let raw_kind = trimmed.get(2..end)?.trim_end_matches(['+', '-']);
    let kind = canonical_callout_kind(raw_kind)?.to_string();
    let title = trimmed[end + 1..].trim().to_string();
    Some((kind, title))
}

fn parse_admonition_info(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let kind = canonical_callout_kind(parts.next()?)?.to_string();
    let title = parts.next().unwrap_or("").trim();
    let title = title
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| title.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(title)
        .to_string();
    Some((kind, title))
}

fn strip_blockquote_marker(line: &str) -> Option<&str> {
    let mut spaces = 0usize;
    for ch in line.chars() {
        if ch == ' ' && spaces < 4 {
            spaces += 1;
            continue;
        }
        if ch == '>' && spaces <= 3 {
            let rest = &line[spaces + 1..];
            return Some(rest.strip_prefix(' ').unwrap_or(rest));
        }
        return None;
    }
    None
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

fn max_backtick_run(text: &str) -> usize {
    let mut max_run = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            max_run = max_run.max(current);
        } else {
            current = 0;
        }
    }
    max_run
}

fn internal_callout_fence(kind: &str, title: &str, content: &str) -> String {
    let fence = "`".repeat(max_backtick_run(content).max(3) + 1);
    let title_suffix = if title.is_empty() {
        String::new()
    } else {
        format!(" {title}")
    };
    let mut out = format!("{fence}pagemd-callout {kind}{title_suffix}\n");
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

fn internal_math_fence(content: &str) -> String {
    let fence = "`".repeat(max_backtick_run(content).max(3) + 1);
    let mut out = format!("{fence}math\n");
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

pub(crate) fn preprocess_markdown_extensions(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = String::new();
    let mut i = 0usize;

    while i < lines.len() {
        if let Some(first) = strip_blockquote_marker(lines[i]) {
            if let Some((kind, title)) = parse_callout_marker(first) {
                i += 1;
                let mut content = String::new();
                while i < lines.len() {
                    if let Some(line) = strip_blockquote_marker(lines[i]) {
                        content.push_str(line);
                        content.push('\n');
                        i += 1;
                    } else {
                        break;
                    }
                }
                out.push_str(&internal_callout_fence(&kind, &title, &content));
                continue;
            }
        }

        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("$$") {
            let math_start = trimmed.trim_end();
            if math_start.len() >= 4 && math_start.ends_with("$$") {
                let inner = math_start[2..math_start.len() - 2].trim();
                if !inner.is_empty() && !inner.contains("$$") {
                    out.push_str(&internal_math_fence(inner));
                    i += 1;
                    continue;
                }
            } else if math_start == "$$" {
                let start_i = i;
                i += 1;
                let mut content = String::new();
                while i < lines.len() && lines[i].trim() != "$$" {
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                let found_closing = i < lines.len() && lines[i].trim() == "$$";
                if found_closing && !content.trim().is_empty() {
                    i += 1;
                    out.push_str(&internal_math_fence(content.trim()));
                    continue;
                }
                let end_i = if found_closing { i + 1 } else { i };
                for line in &lines[start_i..end_i] {
                    out.push_str(line);
                    out.push('\n');
                }
                i = end_i;
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix(":::") {
            if let Some((kind, title)) = parse_admonition_info(rest) {
                i += 1;
                let mut content = String::new();
                while i < lines.len() && lines[i].trim() != ":::" {
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }
                out.push_str(&internal_callout_fence(&kind, &title, &content));
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix("!!!") {
            if let Some((kind, title)) = parse_admonition_info(rest) {
                i += 1;
                let mut content = String::new();
                while i < lines.len() {
                    let line = lines[i];
                    if line.trim().is_empty() {
                        content.push('\n');
                        i += 1;
                    } else if let Some(stripped) = line.strip_prefix("    ") {
                        content.push_str(stripped);
                        content.push('\n');
                        i += 1;
                    } else if let Some(stripped) = line.strip_prefix('\t') {
                        content.push_str(stripped);
                        content.push('\n');
                        i += 1;
                    } else if leading_spaces(line) > 0 {
                        content.push_str(line.trim_start());
                        content.push('\n');
                        i += 1;
                    } else {
                        break;
                    }
                }
                out.push_str(&internal_callout_fence(&kind, &title, &content));
                continue;
            }
        }

        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }

    out
}

pub(crate) fn parse_internal_callout_info(info: &str) -> Option<(String, String)> {
    let mut parts = info.trim().splitn(3, char::is_whitespace);
    if parts.next()? != "pagemd-callout" {
        return None;
    }
    let kind = parts.next()?.to_string();
    let title = parts.next().unwrap_or("").trim().to_string();
    Some((kind, title))
}
