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

pub fn callout_label(kind: &str) -> &'static str {
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

pub fn code_fence_length(texts: &[&str]) -> usize {
    let max_run = texts
        .iter()
        .map(|text| max_backtick_run(text))
        .max()
        .unwrap_or(0);
    if max_run >= 3 {
        max_run + 1
    } else {
        3
    }
}

fn fence_safe_text(text: &str) -> String {
    text.replace('`', "'")
}

fn internal_callout_fence(kind: &str, title: &str, content: &str) -> String {
    let safe_title = fence_safe_text(title);
    let title_suffix = if safe_title.is_empty() {
        String::new()
    } else {
        format!(" {safe_title}")
    };
    let header = format!("pagemd-callout {kind}{title_suffix}");
    let fence = "`".repeat(code_fence_length(&[content, &header]));
    let mut out = format!("{fence}{header}\n");
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

fn internal_math_fence(content: &str) -> String {
    let fence = "`".repeat(code_fence_length(&[content]));
    let mut out = format!("{fence}math\n");
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

fn looks_like_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.contains('|')
}

fn next_non_empty_line<'a>(lines: &'a [&str], start: usize) -> Option<(usize, &'a str)> {
    let mut index = start;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if !trimmed.is_empty() {
            return Some((index, trimmed));
        }
        index += 1;
    }
    None
}

fn content_includes_table(content: &str) -> bool {
    content.lines().any(looks_like_table_line)
}

fn should_absorb_blank_inside_callout_table(content: &str, lines: &[&str], index: usize) -> bool {
    if !content_includes_table(content) {
        return false;
    }
    let Some((_, next)) = next_non_empty_line(lines, index + 1) else {
        return false;
    };
    looks_like_table_line(next)
}

fn attach_trailing_callout_structure(content: &mut String, lines: &[&str], i: &mut usize) {
    while *i < lines.len() {
        if lines[*i].trim().is_empty()
            && should_absorb_blank_inside_callout_table(content, lines, *i)
        {
            content.push('\n');
            *i += 1;
        } else if content_includes_table(content) && looks_like_table_line(lines[*i]) {
            content.push_str(lines[*i].trim());
            content.push('\n');
            *i += 1;
        } else {
            break;
        }
    }
}

fn collect_blockquote_callout_content(lines: &[&str], i: &mut usize) -> String {
    let mut content = String::new();
    while *i < lines.len() {
        if let Some(line) = strip_blockquote_marker(lines[*i]) {
            content.push_str(line);
            content.push('\n');
            *i += 1;
        } else if lines[*i].trim().is_empty()
            && should_absorb_blank_inside_callout_table(&content, lines, *i)
        {
            content.push('\n');
            *i += 1;
        } else if content_includes_table(&content) && looks_like_table_line(lines[*i]) {
            content.push_str(lines[*i].trim());
            content.push('\n');
            *i += 1;
        } else {
            break;
        }
    }
    content
}

/// Fix CommonMark emphasis flanking rules for CJK punctuation.
///
/// Problem: pulldown-cmark follows CommonMark strictly:
///   - `**` is left-flanking only if: when followed by Unicode punctuation (Ps/Pi),
///     the char before `**` must be whitespace or punctuation.
///   - `**` is right-flanking only if: when preceded by Unicode punctuation (Pe/Pf),
///     the char after `**` must be whitespace or punctuation.
///
/// CJK ideographs (category Lo) adjacent to `**` with Ps/Pe on the other side fail.
///
/// Fix: inject ZWSP (U+200B, category Cf — neither punctuation nor whitespace) on the
/// INNER side of `**` so the delimiter's adjacent char is no longer Ps/Pe. The ZWSP is
/// later stripped from all parsed events in `render_markdown_with_depth`, so it never
/// appears in final output.
///
/// - Opening: 是**「x → 是**\u{200B}「x  (char after ** becomes Cf, not Ps → flanking OK)
/// - Closing: x」**后 → x」\u{200B}**后  (char before ** becomes Cf, not Pe → flanking OK)
pub fn fix_emphasis_cjk_punctuation(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(source.len());
    let mut i = 0;

    while i < len {
        if chars[i] == '*' {
            let delim_start = i;
            while i < len && chars[i] == '*' {
                i += 1;
            }
            let delim_end = i;
            let delim_len = delim_end - delim_start;

            if delim_len >= 1 && delim_len <= 3 {
                let before = if delim_start > 0 {
                    Some(chars[delim_start - 1])
                } else {
                    None
                };
                let after = if delim_end < len {
                    Some(chars[delim_end])
                } else {
                    None
                };

                let before_is_cjk = before.map_or(false, is_cjk_letter);
                let after_is_open = after.map_or(false, is_open_punctuation);
                let after_is_cjk = after.map_or(false, is_cjk_letter);
                let before_is_close = before.map_or(false, is_close_punctuation);

                // Closing fix: Close_Punct ** CJK_Letter → ZWSP before **
                if before_is_close && after_is_cjk {
                    result.push('\u{200B}');
                }

                for _ in 0..delim_len {
                    result.push('*');
                }

                // Opening fix: CJK_Letter ** Open_Punct → ZWSP after **
                if before_is_cjk && after_is_open {
                    result.push('\u{200B}');
                }
            } else {
                for _ in 0..delim_len {
                    result.push('*');
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// CJK ideographs (Lo) — the characters that trigger the flanking issue when adjacent to `**`.
fn is_cjk_letter(ch: char) -> bool {
    matches!(ch,
        '\u{3400}'..='\u{4DBF}'     // CJK Unified Ext A
        | '\u{4E00}'..='\u{9FFF}'   // CJK Unified
        | '\u{F900}'..='\u{FAFF}'   // CJK Compatibility
        | '\u{20000}'..='\u{2A6DF}' // Ext B
        | '\u{2A700}'..='\u{2B73F}' // Ext C
        | '\u{2B740}'..='\u{2B81F}' // Ext D
        | '\u{2B820}'..='\u{2CEAF}' // Ext E
        | '\u{2CEB0}'..='\u{2EBEF}' // Ext F
        | '\u{30000}'..='\u{3134F}' // Ext G
        | '\u{3040}'..='\u{309F}'   // Hiragana
        | '\u{30A0}'..='\u{30FF}'   // Katakana
        | '\u{AC00}'..='\u{D7AF}'   // Hangul Syllables
    )
}

/// Unicode open punctuation (Ps/Pi) that triggers the flanking issue.
fn is_open_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '「' | '『'
            | '（'
            | '【'
            | '〔'
            | '〈'
            | '《'
            | '〖'
            | '〘'
            | '〚'
            | '"'
            | '\''
            | '⟨'
            | '⟪'
            | '('
            | '['
            | '{'
    )
}

/// Unicode close punctuation (Pe/Pf) that triggers the flanking issue.
fn is_close_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '」' | '』'
            | '）'
            | '】'
            | '〕'
            | '〉'
            | '》'
            | '〗'
            | '〙'
            | '〛'
            | '"'
            | '\''
            | '⟩'
            | '⟫'
            | ')'
            | ']'
            | '}'
    )
}

/// Strip a leading YAML frontmatter block (`---` … `---`) so it is not rendered.
///
/// Invalid YAML is ignored with a warning; the block is still stripped.
pub fn strip_yaml_frontmatter(source: &str) -> String {
    let body = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    let lines: Vec<&str> = body.lines().collect();
    if lines.first().map(|line| line.trim()) != Some("---") {
        return source.to_string();
    }

    let mut close_idx = None;
    for (idx, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            close_idx = Some(idx);
            break;
        }
    }
    let Some(close_idx) = close_idx else {
        return source.to_string();
    };

    let yaml = lines[1..close_idx].join("\n");
    if !yaml.trim().is_empty() {
        match serde_yaml::from_str::<serde_yaml::Value>(&yaml) {
            Ok(serde_yaml::Value::Mapping(_)) => {}
            Ok(_) => {
                // A leading thematic break followed by a later `---` is not frontmatter.
                return source.to_string();
            }
            Err(err) => {
                eprintln!("Warning: ignoring invalid YAML frontmatter: {err}");
            }
        }
    }

    let mut out = lines[close_idx + 1..].join("\n");
    if body.ends_with('\n') && (out.is_empty() || !out.ends_with('\n')) {
        out.push('\n');
    }
    out
}

pub fn preprocess_markdown_extensions(source: &str) -> String {
    let source = strip_yaml_frontmatter(source);
    let source = fix_emphasis_cjk_punctuation(&source);
    let lines: Vec<&str> = source.lines().collect();
    let mut out = String::new();
    let mut i = 0usize;

    while i < lines.len() {
        if let Some(first) = strip_blockquote_marker(lines[i]) {
            if let Some((kind, title)) = parse_callout_marker(first) {
                i += 1;
                let mut content = collect_blockquote_callout_content(&lines, &mut i);
                attach_trailing_callout_structure(&mut content, &lines, &mut i);
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
                attach_trailing_callout_structure(&mut content, &lines, &mut i);
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
                attach_trailing_callout_structure(&mut content, &lines, &mut i);
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

pub fn parse_internal_callout_info(info: &str) -> Option<(String, String)> {
    let mut parts = info.trim().splitn(3, char::is_whitespace);
    if parts.next()? != "pagemd-callout" {
        return None;
    }
    let kind = parts.next()?.to_string();
    let title = parts.next().unwrap_or("").trim().to_string();
    Some((kind, title))
}

#[cfg(test)]
mod tests {
    use super::{
        fix_emphasis_cjk_punctuation, preprocess_markdown_extensions, strip_yaml_frontmatter,
    };

    #[test]
    fn yaml_frontmatter_is_stripped_before_render() {
        let input = "---\nposition: 2\npath: /docs/intro\ntitle: Hello\n---\n\n# Body\n";
        let out = strip_yaml_frontmatter(input);
        assert!(!out.contains("position:"));
        assert!(!out.contains("path:"));
        assert!(out.contains("# Body"));

        let pre = preprocess_markdown_extensions(input);
        assert!(!pre.contains("position:"));
        assert!(pre.contains("# Body"));
    }

    #[test]
    fn invalid_yaml_frontmatter_is_stripped_with_warning_path() {
        let input = "---\ntitle: [unterminated\n---\n\nVisible\n";
        let out = strip_yaml_frontmatter(input);
        assert!(!out.contains("title:"));
        assert!(out.contains("Visible"));
    }

    #[test]
    fn content_starting_with_thematic_break_is_kept() {
        let input = "---\n\n# Not frontmatter\n";
        let out = strip_yaml_frontmatter(input);
        assert_eq!(out, input);
    }

    #[test]
    fn thematic_break_with_later_rule_is_not_treated_as_frontmatter() {
        let input = "---\n\n# Title\n\n---\n\nAfter\n";
        let out = strip_yaml_frontmatter(input);
        assert_eq!(out, input);
    }

    #[test]
    fn empty_yaml_frontmatter_is_stripped() {
        let input = "---\n---\n\n# Body\n";
        let out = strip_yaml_frontmatter(input);
        assert_eq!(out, "\n# Body\n");
    }

    /// Helper: apply fix then parse with pulldown-cmark, strip ZWSP, return HTML.
    fn render_emphasis(input: &str) -> String {
        use pulldown_cmark::{html, Event, Options, Parser};

        let fixed = fix_emphasis_cjk_punctuation(input);
        let parser = Parser::new_ext(&fixed, Options::empty());
        let events: Vec<Event> = parser
            .map(|ev| match ev {
                Event::Text(ref t) if t.contains('\u{200B}') => {
                    Event::Text(t.replace('\u{200B}', "").into())
                }
                Event::Code(ref t) if t.contains('\u{200B}') => {
                    Event::Code(t.replace('\u{200B}', "").into())
                }
                other => other,
            })
            .collect();
        let mut html_out = String::new();
        html::push_html(&mut html_out, events.into_iter());
        html_out
    }

    #[test]
    fn callout_absorbs_table_rows_without_blockquote_prefix() {
        let input = "> [!NOTE] Title\n> | Col | Ref |\n| --- | --- |\n| A | Item[^tbl]. |\n\n[^tbl]: Footnote.\n";
        let out = preprocess_markdown_extensions(input);
        assert!(out.contains("| --- | --- |"));
        assert!(out.contains("Item[^tbl]."));
        assert!(out.contains("[^tbl]: Footnote."));
    }

    #[test]
    fn cjk_emphasis_with_corner_brackets() {
        // 是**「陷入无法恢复的状态」**—— should produce <strong>
        let html = render_emphasis("是**「陷入无法恢复的状态」**——\n");
        assert!(
            html.contains("<strong>「陷入无法恢复的状态」</strong>"),
            "expected bold with corner brackets, got: {html}"
        );
    }

    #[test]
    fn cjk_emphasis_with_fullwidth_parens() {
        let html = render_emphasis("是**（重要）**后\n");
        assert!(
            html.contains("<strong>（重要）</strong>"),
            "expected bold with fullwidth parens, got: {html}"
        );
    }

    #[test]
    fn cjk_emphasis_with_angle_brackets() {
        let html = render_emphasis("是**《标题》**后\n");
        assert!(
            html.contains("<strong>《标题》</strong>"),
            "expected bold with angle brackets, got: {html}"
        );
    }

    #[test]
    fn cjk_emphasis_with_square_brackets() {
        let html = render_emphasis("是**【注意】**后\n");
        assert!(
            html.contains("<strong>【注意】</strong>"),
            "expected bold with square brackets, got: {html}"
        );
    }

    #[test]
    fn normal_cjk_emphasis_unaffected() {
        // No open/close punct inside — should work without fix too
        let html = render_emphasis("这是**加粗**文本\n");
        assert!(
            html.contains("<strong>加粗</strong>"),
            "normal CJK bold broken: {html}"
        );
    }

    #[test]
    fn ascii_emphasis_unaffected() {
        let html = render_emphasis("This is **bold** text\n");
        assert!(
            html.contains("<strong>bold</strong>"),
            "ASCII bold broken: {html}"
        );
    }

    #[test]
    fn code_span_not_affected() {
        let html = render_emphasis("`是**「不加粗」**后`\n");
        assert!(
            html.contains("<code>是**「不加粗」**后</code>"),
            "code span should preserve raw text, got: {html}"
        );
    }

    #[test]
    fn no_zwsp_in_output() {
        let html = render_emphasis("是**「测试」**后\n");
        assert!(
            !html.contains('\u{200B}'),
            "ZWSP leaked into output: {html}"
        );
    }

    #[test]
    fn unmatched_stars_stay_literal() {
        let html = render_emphasis("是**「只有开头\n");
        assert!(
            !html.contains("<strong>"),
            "unmatched ** should not produce bold: {html}"
        );
    }

    #[test]
    fn digit_before_stars_not_triggered() {
        // '0' is not CJK letter — fix should NOT activate
        let html = render_emphasis("100**（税）**元\n");
        assert!(
            !html.contains("<strong>"),
            "digit before ** should not trigger fix: {html}"
        );
    }

    #[test]
    fn punctuation_before_stars_already_works() {
        // ，is fullwidth comma (Po) — flanking rule already satisfied without fix
        let html = render_emphasis("，**默认即装上熵增守卫**：\n");
        assert!(
            html.contains("<strong>默认即装上熵增守卫</strong>"),
            "punct-before case broken: {html}"
        );
    }

    #[test]
    fn nested_emphasis_inside_cjk_brackets() {
        let html = render_emphasis("是**「*斜体*」**后\n");
        assert!(
            html.contains("<strong>") && html.contains("<em>斜体</em>"),
            "nested emphasis broken: {html}"
        );
    }
}
