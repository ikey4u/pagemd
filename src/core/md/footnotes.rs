//! Section-wide footnote registry and rendering helpers.
//!
//! Callouts are re-parsed as separate documents, but footnotes are resolved at
//! section scope via `FootnoteRegistry`:
//!
//! 1. Scan the full preprocessed section for `[^label]:` definitions
//! 2. Before each parse unit, replace definition blocks outside code fences with slots
//! 3. Append stub `[^label]: .` lines so pulldown emits `FootnoteReference` events
//! 4. Render references and slots ourselves; ignore pulldown definition events

use std::collections::{HashMap, HashSet};

use crate::core::util::html_escape;

const FN_SLOT_MARKER: &str = "data-pagemd-fn-slot=\"";

pub struct FootnoteRegistry {
    definitions: HashMap<String, String>,
}

impl FootnoteRegistry {
    pub fn from_markdown(source: &str) -> Self {
        // Callers that need LLM dialect repair (concatenated defs, empty stubs)
        // should normalize upstream — Pack uses `canonicalize_intent_markdown`.
        let mut definitions: HashMap<String, String> = HashMap::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut index = 0usize;
        while index < lines.len() {
            let Some((label, first_line_body)) = parse_definition_start(lines[index]) else {
                index += 1;
                continue;
            };
            let mut body = first_line_body;
            index += 1;
            while index < lines.len() && is_definition_continuation(lines[index]) {
                let continuation = lines[index];
                let trimmed = continuation.trim();
                if !body.is_empty() && !body.ends_with('\n') {
                    body.push('\n');
                }
                body.push_str(trimmed);
                index += 1;
            }
            let existing_nonempty = definitions
                .get(&label)
                .is_some_and(|existing| !existing.trim().is_empty());
            // Keep a non-empty definition; allow a later non-empty body to
            // replace an earlier empty stub (`[^n]:` alone before a table).
            if existing_nonempty || (definitions.contains_key(&label) && body.trim().is_empty()) {
                continue;
            }
            definitions.insert(label, body);
        }
        Self { definitions }
    }

    pub fn definition(&self, label: &str) -> Option<&str> {
        self.definitions.get(label).map(String::as_str)
    }

    /// Remove footnote definition blocks present in `source`, insert render slots,
    /// and add stub definitions so pulldown emits `FootnoteReference` events for
    /// labels defined elsewhere in the section.
    ///
    /// Lines inside fenced code blocks are left untouched; callout bodies apply
    /// footnote preparation during their own nested parse.
    pub fn prepare_parse_unit(&self, source: &mut String) {
        let referenced = referenced_labels(source);
        let lines: Vec<&str> = source.lines().collect();
        let mut out = String::new();
        let mut index = 0usize;
        let mut in_code_fence = false;
        let mut fence_len = 0usize;
        let mut slotted: HashSet<String> = HashSet::new();

        while index < lines.len() {
            let line = lines[index];

            if let Some(mark_len) = fence_marker_length(line) {
                if !in_code_fence {
                    in_code_fence = true;
                    fence_len = mark_len;
                } else if mark_len >= fence_len && is_fence_closer(line, fence_len) {
                    in_code_fence = false;
                    fence_len = 0;
                }
                out.push_str(line);
                out.push('\n');
                index += 1;
                continue;
            }

            if in_code_fence {
                out.push_str(line);
                out.push('\n');
                index += 1;
                continue;
            }

            if let Some((label, _)) = parse_definition_start(line) {
                if self.definitions.contains_key(&label) {
                    // One end-note slot per label — drop duplicate def stubs.
                    if slotted.insert(label.clone()) {
                        out.push_str(&footnote_slot_html(&label));
                    }
                    index += 1;
                    while index < lines.len() && is_definition_continuation(lines[index]) {
                        index += 1;
                    }
                    continue;
                }
            }

            out.push_str(line);
            out.push('\n');
            index += 1;
        }

        for label in referenced {
            if self.definitions.contains_key(&label) {
                out.push('\n');
                out.push_str(&format!("[^{label}]: .\n"));
            }
        }

        *source = out;
    }
}

fn fence_marker_length(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if ch == '`' {
            count += 1;
        } else {
            break;
        }
    }
    if count >= 3 {
        Some(count)
    } else {
        None
    }
}

fn is_fence_closer(line: &str, open_len: usize) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < open_len {
        return false;
    }
    if !trimmed.chars().take(open_len).all(|ch| ch == '`') {
        return false;
    }
    trimmed.chars().skip(open_len).all(char::is_whitespace)
}

pub fn referenced_labels(content: &str) -> HashSet<String> {
    let mut labels = HashSet::new();
    let bytes = content.as_bytes();
    let mut index = 0usize;
    while index + 3 < bytes.len() {
        if bytes[index] == b'[' && bytes[index + 1] == b'^' {
            let start = index + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b']' {
                end += 1;
            }
            if end < bytes.len()
                && end > start
                && !(end + 1 < bytes.len() && bytes[end + 1] == b':')
            {
                let label = &content[start..end];
                if !label.contains(['[', ']', ':']) {
                    labels.insert(label.to_string());
                }
            }
            index = end + 1;
        } else {
            index += 1;
        }
    }
    labels
}

fn parse_definition_start(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("[^") {
        return None;
    }
    let rest = &trimmed[2..];
    let closing = rest.find(']')?;
    let label = rest[..closing].to_string();
    if label.is_empty() || label.contains(['[', ']', ':']) {
        return None;
    }
    // Accept `[^n]:`, `[^n] :`, and fullwidth `[^n]：` (common LLM typos).
    let after = rest[closing + 1..].trim_start();
    let body = after
        .strip_prefix(':')
        .or_else(|| after.strip_prefix('：'))?
        .trim_start()
        .to_string();
    Some((label, body))
}

/// Opt-in LLM-dialect repair: split concatenated `[^n]:` defs, normalize spaced /
/// fullwidth colons, and rewrite empty `[^n]:` stubs into inline refs.
///
/// **Not** applied by default in [`FootnoteRegistry::from_markdown`] or the HTML
/// render path — Pack and similar hosts should canonicalize before calling PageMD.
pub fn normalize_footnote_definition_lines(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len().saturating_add(32));
    let mut in_fence = false;
    let mut fence_len = 0usize;

    for line in markdown.split_inclusive('\n') {
        let line_body = line.strip_suffix('\n').unwrap_or(line);
        let had_newline = line.len() != line_body.len();

        if let Some(mark_len) = fence_marker_length(line_body) {
            if !in_fence {
                in_fence = true;
                fence_len = mark_len;
            } else if mark_len >= fence_len && is_fence_closer(line_body, fence_len) {
                in_fence = false;
                fence_len = 0;
            }
            out.push_str(line_body);
            if had_newline {
                out.push('\n');
            }
            continue;
        }

        if in_fence {
            out.push_str(line_body);
            if had_newline {
                out.push('\n');
            }
            continue;
        }

        out.push_str(&normalize_footnote_defs_in_line(line_body));
        if had_newline {
            out.push('\n');
        }
    }
    repair_empty_footnote_definition_stubs(&out)
}

/// LLMs often leave a bare `[^n]:` stub before a table/list/code block. That
/// steals the real end-note definition and renders as a floating marker in a
/// blank gap. Rewrite those stubs into an inline `[^n]` on the previous line.
fn repair_empty_footnote_definition_stubs(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    if lines.is_empty() {
        return markdown.to_string();
    }
    let mut owned: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();
    let mut remove = vec![false; owned.len()];
    let mut in_fence = false;
    let mut fence_len = 0usize;

    for i in 0..owned.len() {
        if let Some(mark_len) = fence_marker_length(&owned[i]) {
            if !in_fence {
                in_fence = true;
                fence_len = mark_len;
            } else if mark_len >= fence_len && is_fence_closer(&owned[i], fence_len) {
                in_fence = false;
                fence_len = 0;
            }
            continue;
        }
        if in_fence {
            continue;
        }

        let Some((label, body)) = parse_definition_start(&owned[i]) else {
            continue;
        };
        let next_continues = owned
            .get(i + 1)
            .is_some_and(|next| is_definition_continuation(next));
        if !body.trim().is_empty() || next_continues {
            continue;
        }

        remove[i] = true;
        let marker = format!("[^{label}]");
        let mut j = i;
        while j > 0 {
            j -= 1;
            if remove[j] || owned[j].trim().is_empty() {
                continue;
            }
            if parse_definition_start(&owned[j]).is_some() {
                continue;
            }
            if !owned[j].contains(&marker) {
                let trimmed = owned[j].trim_end().to_string();
                owned[j] = format!("{trimmed}{marker}");
            }
            break;
        }
    }

    let mut out = String::with_capacity(markdown.len());
    for (i, line) in owned.iter().enumerate() {
        if remove[i] {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !markdown.ends_with('\n') {
        while out.ends_with('\n') {
            out.pop();
        }
    }
    out
}

fn is_footnote_def_colon(ch: char) -> bool {
    ch == ':' || ch == '：'
}

fn normalize_footnote_defs_in_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len().saturating_add(8));
    let mut chars = line.chars().peekable();
    let mut emitted_def = false;

    while let Some(c) = chars.next() {
        if c == '[' && chars.peek() == Some(&'^') {
            let mut look = chars.clone();
            look.next(); // ^
            let mut label = String::new();
            let mut is_def = false;
            while let Some(&ch) = look.peek() {
                if ch == ']' {
                    look.next();
                    while matches!(look.peek(), Some(' ' | '\t')) {
                        look.next();
                    }
                    if look.peek().copied().is_some_and(is_footnote_def_colon) {
                        is_def = !label.is_empty() && !label.contains('[') && !label.contains(']');
                    }
                    break;
                }
                if ch == '[' || ch == '\n' || ch == '\r' {
                    break;
                }
                label.push(ch);
                look.next();
            }
            if is_def {
                if !out.is_empty() {
                    while out.ends_with(' ') || out.ends_with('\t') {
                        out.pop();
                    }
                    if emitted_def || !out.is_empty() {
                        out.push_str("\n\n");
                    }
                }
                out.push('[');
                out.push('^');
                chars.next(); // ^
                for _ in 0..label.chars().count() {
                    chars.next();
                }
                chars.next(); // ]
                while matches!(chars.peek(), Some(' ' | '\t')) {
                    chars.next();
                }
                chars.next(); // : or ：
                out.push_str(&label);
                out.push_str("]:");
                emitted_def = true;
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn is_definition_continuation(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}

/// A footnote definition extracted out of the HTML body for host UI
/// (e.g. a citations dialog). Populated when [`FootnoteDisplay::Host`] is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedFootnote {
    pub label: String,
    /// Single-line plain text suitable for tooltips / dialogs.
    pub plain: String,
    /// Definition body rendered as HTML (may be empty for plain-only hosts).
    pub html: String,
}

/// How footnote definitions appear in the exported HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FootnoteDisplay {
    /// Visible end-note list (CLI / full documents). Hover hints need footnote JS.
    #[default]
    EndList,
    /// Keep superscript refs; hide the end-note list. Refs get a plain-text `title`
    /// tooltip from the definition (works without scripts — Pack / sandboxed iframes).
    Tooltip,
    /// Inline superscript refs only; definitions are stripped from the body and
    /// returned via [`crate::ExportOutput::footnotes`] for the host to show in
    /// its own citations UI.
    Host,
}

/// Sort extracted footnotes by numeric label when possible, else lexicographically.
pub fn sort_extracted_footnotes(footnotes: &mut [ExtractedFootnote]) {
    footnotes.sort_by(
        |a, b| match (a.label.parse::<u32>(), b.label.parse::<u32>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => a.label.cmp(&b.label),
        },
    );
}

/// Flatten a footnote definition to a single-line plain tooltip string.
pub fn plain_footnote_title(body: &str) -> String {
    let mut out = body.to_string();
    // Strip HTML tags that would break parsing / tooltips.
    while let Some(start) = out.find('<') {
        if let Some(rel) = out[start..].find('>') {
            out.replace_range(start..start + rel + 1, " ");
        } else {
            out.replace_range(start.., " ");
            break;
        }
    }
    out = out
        .replace(['\n', '\r', '\t'], " ")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(180).collect()
}

pub fn footnote_ref_html(label: &str, title: Option<&str>) -> String {
    let escaped = html_escape(label);
    let title_attr = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| format!(" title=\"{}\"", html_escape(t)))
        .unwrap_or_default();
    format!(
        "<sup class=\"footnote-ref\"><a href=\"#fn-{escaped}\" class=\"footnote-ref-link\"{title_attr}>{escaped}</a></sup>"
    )
}

pub fn footnote_slot_html(label: &str) -> String {
    format!("<div {FN_SLOT_MARKER}{}\"></div>\n", html_escape(label))
}

pub fn footnote_def_html(label: &str, body_html: &str, display: FootnoteDisplay) -> String {
    let escaped = html_escape(label);
    let class = match display {
        FootnoteDisplay::EndList => "footnote",
        FootnoteDisplay::Tooltip => "footnote footnote--tooltip-source",
        // Host mode never emits definition HTML into the body.
        FootnoteDisplay::Host => return String::new(),
    };
    format!(
        "<div class=\"{class}\" id=\"fn-{escaped}\"><span class=\"footnote-marker\"><sup>{escaped}</sup></span><span class=\"footnote-content\">{body_html}</span></div>\n"
    )
}

pub fn footnote_slot_label(raw_html: &str) -> Option<String> {
    footnote_slot_labels(raw_html).into_iter().next()
}

/// All footnote slot labels in a raw HTML chunk (pulldown may merge adjacent slots).
pub fn footnote_slot_labels(raw_html: &str) -> Vec<String> {
    let mut labels = Vec::new();
    let mut rest = raw_html;
    while let Some(start) = rest.find(FN_SLOT_MARKER) {
        let after = &rest[start + FN_SLOT_MARKER.len()..];
        let Some(end) = after.find('"') else {
            break;
        };
        let label = after[..end].to_string();
        if !label.is_empty() {
            labels.push(label);
        }
        rest = &after[end + 1..];
    }
    labels
}

pub enum FootnoteTextSegment<'a> {
    Plain(&'a str),
    Reference(&'a str),
}

pub fn split_footnote_text(text: &str) -> Vec<FootnoteTextSegment<'_>> {
    let mut segments = Vec::new();
    let bytes = text.as_bytes();
    let mut plain_start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'[' && index + 1 < bytes.len() && bytes[index + 1] == b'^' {
            let label_start = index + 2;
            let mut label_end = label_start;
            while label_end < bytes.len() && bytes[label_end] != b']' {
                label_end += 1;
            }
            if label_end < bytes.len()
                && label_end > label_start
                && !(label_end + 1 < bytes.len() && bytes[label_end + 1] == b':')
            {
                let label = &text[label_start..label_end];
                if !label.contains(['[', ']', ':']) {
                    if plain_start < index {
                        segments.push(FootnoteTextSegment::Plain(&text[plain_start..index]));
                    }
                    segments.push(FootnoteTextSegment::Reference(label));
                    index = label_end + 1;
                    plain_start = index;
                    continue;
                }
            }
        }
        index += 1;
    }
    if plain_start < text.len() {
        segments.push(FootnoteTextSegment::Plain(&text[plain_start..]));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::{
        footnote_slot_label, plain_footnote_title, split_footnote_text, FootnoteDisplay,
        FootnoteRegistry, FootnoteTextSegment,
    };

    #[test]
    fn normalize_splits_concatenated_defs_and_skips_fences() {
        let raw = "## 引用\n\n[^1]: `a.toml` — one[^2]: `b.md` — two\n\n```\n[^x]: keep\n```\n";
        let normalized = super::normalize_footnote_definition_lines(raw);
        assert!(normalized.contains("\n\n[^2]:"));
        assert!(normalized.contains("[^x]: keep"));
        assert!(!normalized.contains("one[^2]:"));

        let html = render_md(&normalized);
        assert!(html.contains("id=\"fn-1\""));
        assert!(html.contains("id=\"fn-2\""));
        assert!(html.matches("class=\"footnote\"").count() >= 2);
    }

    #[test]
    fn empty_footnote_stub_becomes_inline_ref_on_previous_line() {
        let raw = "各臂按优先级从高到低\n\n[^12]:\n\n| 优先级 | 臂 |\n| --- | --- |\n| 1 | a |\n\n[^12]: real note about biased select\n";
        let normalized = super::normalize_footnote_definition_lines(raw);
        assert!(
            normalized.contains("各臂按优先级从高到低[^12]"),
            "stub should attach to previous prose: {normalized:?}"
        );
        assert!(
            !normalized.contains("\n[^12]:\n\n|"),
            "empty stub before table should be removed: {normalized:?}"
        );
        assert!(normalized.contains("[^12]: real note"));

        let html = render_md(&normalized);
        assert!(html.contains("class=\"footnote-ref\""));
        assert!(html.contains("id=\"fn-12\""));
        assert!(html.contains("real note about biased select"));
        assert!(
            !html.contains("<p><sup class=\"footnote-ref\""),
            "orphan ref paragraph: {html}"
        );
    }

    #[test]
    fn default_render_does_not_apply_llm_stub_repair() {
        // Empty stub stays in the source; hosts must canonicalize before render.
        let raw = "各臂按优先级从高到低\n\n[^12]:\n\n| a | b |\n| --- | --- |\n| 1 | x |\n\n[^12]: real note\n";
        let registry = FootnoteRegistry::from_markdown(raw);
        assert_eq!(registry.definition("12"), Some("real note"));
        let html = render_md(raw);
        assert!(html.contains("real note"));
        // Without normalize, the empty stub still produces a slot/ref in the gap.
        assert!(
            html.contains("id=\"fn-12\"") || html.contains("footnote-ref"),
            "def still renders: {html}"
        );
    }

    #[test]
    fn normalize_accepts_spaced_and_fullwidth_colons() {
        let raw = "## 引用\n\n[^1] : `a.toml` — one\n\n[^2]：`b.md` — two\n";
        let normalized = super::normalize_footnote_definition_lines(raw);
        assert!(normalized.contains("[^1]: `a.toml`"));
        assert!(normalized.contains("[^2]:"));
        assert!(normalized.contains("`b.md`"));
        assert!(!normalized.contains("[^1] :"));
        assert!(!normalized.contains('：'));

        // Spaced/fullwidth colons are still accepted by parse_definition_start
        // on the default path (general CJK tolerance, not LLM concat repair).
        let html = render_md(raw);
        assert!(html.contains("id=\"fn-1\""));
        assert!(html.contains("id=\"fn-2\""));
        assert!(html.contains("class=\"footnote\""));
        assert!(!html.contains("<p>[^1]"));
    }

    #[test]
    fn registry_collects_multiline_and_prepare_replaces_with_slot() {
        let source = "Ref[^a].\n\n[^a]: First line.\n    Second line.\n\nParagraph.\n";
        let registry = FootnoteRegistry::from_markdown(source);
        assert_eq!(registry.definition("a"), Some("First line.\nSecond line."));

        let mut unit = source.to_string();
        registry.prepare_parse_unit(&mut unit);
        assert!(!unit.contains("[^a]: First line."));
        assert!(unit.contains("data-pagemd-fn-slot=\"a\""));
        assert!(unit.contains("Ref[^a]."));
    }

    #[test]
    fn prepare_skips_footnote_lines_inside_code_fences() {
        let source = "```pagemd-callout note Title\n[^inline]: Keep me.\n```\n";
        let registry = FootnoteRegistry::from_markdown(source);
        let mut unit = source.to_string();
        registry.prepare_parse_unit(&mut unit);
        assert!(unit.contains("[^inline]: Keep me."));
        assert!(!unit.contains("data-pagemd-fn-slot"));
    }

    #[test]
    fn registry_first_definition_wins_for_duplicate_labels() {
        let source = "[^x]: One.\n[^x]: Two.\n";
        let registry = FootnoteRegistry::from_markdown(source);
        assert_eq!(registry.definition("x"), Some("One."));
    }

    #[test]
    fn split_text_skips_definition_syntax() {
        let segments = split_footnote_text("See[^note] and literal [^not-a-def]: text.");
        assert!(segments.iter().any(|segment| {
            matches!(segment, FootnoteTextSegment::Reference(label) if *label == "note")
        }));
        assert!(segments.iter().any(|segment| {
            matches!(segment, FootnoteTextSegment::Plain(text) if text.contains("[^not-a-def]: text."))
        }));
    }

    #[test]
    fn slot_label_is_extracted_from_placeholder_html() {
        let label = footnote_slot_label("<div data-pagemd-fn-slot=\"demo\"></div>");
        assert_eq!(label.as_deref(), Some("demo"));
    }

    fn render_md(source: &str) -> String {
        use std::path::Path;

        use syntect::highlighting::ThemeSet;
        use syntect::parsing::SyntaxSet;

        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        crate::core::md::render::render_markdown(
            source,
            Path::new("."),
            16.0,
            "",
            &ss,
            &ts,
            false,
            FootnoteDisplay::EndList,
        )
        .expect("render markdown")
        .html
    }

    #[test]
    fn prepare_injects_stub_definition_for_external_reference() {
        let registry = FootnoteRegistry::from_markdown("[^a]: Real def.\n");
        let mut unit = "See[^a].\n".to_string();
        registry.prepare_parse_unit(&mut unit);
        assert!(unit.contains("[^a]: ."));
    }

    #[test]
    fn pulldown_emits_footnote_reference_with_stub_definition() {
        use pulldown_cmark::{Event, Options, Parser as MdParser};

        let source = "Text[^a].\n\n[^a]: Footnote.\n";
        let registry = FootnoteRegistry::from_markdown(source);
        let mut parse_source = crate::core::md::preprocess::preprocess_markdown_extensions(source);
        registry.prepare_parse_unit(&mut parse_source);
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_FOOTNOTES);
        let parser = MdParser::new_ext(&parse_source, opts);
        let events: Vec<Event> = parser.collect();
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::FootnoteReference(label) if &**label == "a")));
    }

    #[test]
    fn split_finds_ref_in_text() {
        let segments = split_footnote_text("Text[^a].");
        assert!(segments.iter().any(|segment| {
            matches!(segment, FootnoteTextSegment::Reference(label) if *label == "a")
        }));
    }

    #[test]
    fn plain_footnote_reference_renders() {
        let source = "Text[^a].\n\n[^a]: Footnote.\n";
        let html = render_md(source);
        assert!(html.contains("class=\"footnote-ref\""));
        assert!(html.contains("id=\"fn-a\""));
    }

    #[test]
    fn plain_footnote_title_strips_html_tags() {
        let title = plain_footnote_title(
            "`README.md:1-80` — <source media=\"(prefers-color-scheme: dark)\" srcset=\"https://x\">",
        );
        assert!(title.contains("README.md:1-80"));
        assert!(!title.contains("<source"));
        assert!(!title.contains("srcset"));
    }

    #[test]
    fn callout_with_backtick_in_title_renders_as_callout() {
        let source =
            "> [!QUOTE] Trailing (no `>` after)\n> Reference[^fn].\n\n[^fn]: Definition.\n";
        let html = render_md(source);
        assert!(html.contains("class=\"callout callout-quote\""));
        assert!(!html.contains("pagemd-callout"));
        assert!(html.contains("class=\"footnote-ref\""));
        assert!(html.contains("id=\"fn-fn\""));
    }

    #[test]
    fn callout_renders_trailing_footnote_after_blockquote() {
        let source = "> [!NOTE] Title\n> Text[^a].\n\n[^a]: Footnote **bold**.\n";
        let html = render_md(source);
        assert!(html.contains("class=\"footnote-ref\""));
        assert!(html.contains("id=\"fn-a\""));
        assert!(html.contains("<strong>bold</strong>"));
        assert_eq!(html.matches("id=\"fn-a\"").count(), 1);
    }

    #[test]
    fn callout_renders_multiple_trailing_footnotes_with_orphan_between() {
        let source = "> [!NOTE] Title\n> Ref[^a] and [^b].\n\n[^a]: Def A.\n[^unused]: Orphan.\n[^b]: Def B.\n";
        let html = render_md(source);
        assert!(html.contains("id=\"fn-a\""));
        assert!(html.contains("id=\"fn-b\""));
        assert!(html.contains("id=\"fn-unused\""));
        assert_eq!(html.matches("class=\"footnote\"").count(), 3);
    }

    #[test]
    fn callout_table_with_trailing_footnote_renders() {
        let source = "> [!NOTE] Title\n> | Col | Ref |\n| --- | --- |\n| A | Item[^tbl]. |\n\n[^tbl]: Table footnote.\n";
        let html = render_md(source);
        assert!(html.contains("class=\"footnote-ref\""));
        assert!(html.contains("id=\"fn-tbl\""));
        assert!(html.contains("<table>"));
    }
}
