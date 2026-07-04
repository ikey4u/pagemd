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

pub(crate) struct FootnoteRegistry {
    definitions: HashMap<String, String>,
}

impl FootnoteRegistry {
    pub(crate) fn from_markdown(source: &str) -> Self {
        let mut definitions = HashMap::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut index = 0usize;
        while index < lines.len() {
            let Some((label, first_line_body)) = parse_definition_start(lines[index]) else {
                index += 1;
                continue;
            };
            if definitions.contains_key(&label) {
                index += 1;
                continue;
            }
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
            definitions.insert(label, body);
        }
        Self { definitions }
    }

    pub(crate) fn definition(&self, label: &str) -> Option<&str> {
        self.definitions.get(label).map(String::as_str)
    }

    /// Remove footnote definition blocks present in `source`, insert render slots,
    /// and add stub definitions so pulldown emits `FootnoteReference` events for
    /// labels defined elsewhere in the section.
    ///
    /// Lines inside fenced code blocks are left untouched; callout bodies apply
    /// footnote preparation during their own nested parse.
    pub(crate) fn prepare_parse_unit(&self, source: &mut String) {
        let referenced = referenced_labels(source);
        let lines: Vec<&str> = source.lines().collect();
        let mut out = String::new();
        let mut index = 0usize;
        let mut in_code_fence = false;
        let mut fence_len = 0usize;

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
                    out.push_str(&footnote_slot_html(&label));
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

pub(crate) fn referenced_labels(content: &str) -> HashSet<String> {
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
    let closing = trimmed.find("]:")?;
    let label = trimmed.get(2..closing)?.to_string();
    if label.is_empty() || label.contains(['[', ']', ':']) {
        return None;
    }
    let body = trimmed.get(closing + 2..)?.trim_start().to_string();
    Some((label, body))
}

fn is_definition_continuation(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}

pub(crate) fn footnote_ref_html(label: &str) -> String {
    let escaped = html_escape(label);
    format!(
        "<sup class=\"footnote-ref\"><a href=\"#fn-{escaped}\" class=\"footnote-ref-link\">{escaped}</a></sup>"
    )
}

pub(crate) fn footnote_slot_html(label: &str) -> String {
    format!("<div {FN_SLOT_MARKER}{}\"></div>\n", html_escape(label))
}

pub(crate) fn footnote_def_html(label: &str, body_html: &str) -> String {
    let escaped = html_escape(label);
    format!(
        "<div class=\"footnote\" id=\"fn-{escaped}\"><span class=\"footnote-marker\"><sup>{escaped}</sup></span><span class=\"footnote-content\">{body_html}</span></div>\n"
    )
}

pub(crate) fn footnote_slot_label(raw_html: &str) -> Option<String> {
    let start = raw_html.find(FN_SLOT_MARKER)? + FN_SLOT_MARKER.len();
    let rest = &raw_html[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub(crate) enum FootnoteTextSegment<'a> {
    Plain(&'a str),
    Reference(&'a str),
}

pub(crate) fn split_footnote_text(text: &str) -> Vec<FootnoteTextSegment<'_>> {
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
    use super::{footnote_slot_label, split_footnote_text, FootnoteRegistry, FootnoteTextSegment};

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
        crate::core::md::render::render_markdown(source, Path::new("."), 16.0, "", &ss, &ts)
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
