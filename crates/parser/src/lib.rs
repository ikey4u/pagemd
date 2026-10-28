//! Shared PageMD Markdown analysis and rendering contracts.
//! The library owns syntax, dialect preprocessing and source coordinates.
//! Output backends consume immutable parsed documents through `Renderer`.

mod analysis;
mod configuration;
pub mod extensions;
pub mod mdast;
pub mod message;
pub mod unist;
use std::ops::Range;

pub use configuration::{CompileOptions, Constructs, Options, ParseOptions};
use unicode_general_category::{get_general_category, GeneralCategory};
pub use unist::Location;

mod document;
mod renderer;
pub use document::{parse, ParsedDocument, SourceEvent};
pub use renderer::{HtmlFragmentRenderer, Renderer};
/// Event types consumed by renderers; only this crate owns parser construction.
pub mod events {
    pub use pulldown_cmark::{
        Alignment, CodeBlockKind, Event, LinkType, Tag, TagEnd,
    };
}

/// Convenience entry point returning PageMD's syntax tree.
pub fn to_mdast(
    source: &str,
    options: &ParseOptions,
) -> Result<mdast::Node, message::Message> {
    Ok(parse(source, options)?.ast().clone())
}

/// Convenience HTML fragment rendering through the shared renderer trait.
pub fn to_html(source: &str) -> String {
    to_html_with_options(source, &Options::default())
        .expect("default parsing options are valid")
}
pub fn to_html_with_options(
    source: &str,
    options: &Options,
) -> Result<String, message::Message> {
    let document = parse(source, &options.parse)?;
    HtmlFragmentRenderer::new(options.compile.clone()).render(&document, ())
}

/// Prose-only CJK normalization shared with PageMD's HTML renderer.
pub fn fix_emphasis_cjk_punctuation(source: &str) -> String {
    let (root, _) = analysis::parse(source, &ParseOptions::gfm());
    let offsets = insertion_offsets(source, &root);
    prepare(source, &offsets, '\u{200b}').0
}

struct Prepared {
    text: String,
    marker: char,
    /// Byte ranges occupied by artificial characters in the prepared source.
    inserted: Vec<Range<usize>>,
}

impl Prepared {
    fn new(
        source: &str,
        original: &mdast::Node,
        options: &ParseOptions,
    ) -> Option<Self> {
        let currency = if options.constructs.math_text {
            currency_math_offsets(source, original)
        } else {
            Vec::new()
        };
        let mut offsets = insertion_offsets(source, original);
        if options.constructs.math_text {
            offsets.extend(padded_math_offsets(source, original, &currency));
        }
        offsets.sort_unstable();
        offsets.dedup();
        if offsets.is_empty() && currency.is_empty() {
            return None;
        }
        // Use a character absent from decoded prose as well as the raw source:
        // user-authored ZWSP and character references must survive unchanged.
        let decoded = original.to_string();
        let fields = format!("{original:?}");
        let marker = std::iter::once('\u{200b}')
            .chain('\u{e000}'..='\u{f8ff}')
            .chain('\u{f0000}'..='\u{ffffd}')
            .find(|marker| {
                !source.contains(*marker)
                    && !decoded.contains(*marker)
                    && !fields.contains(&marker.escape_debug().to_string())
            })?;
        let mut insertions: Vec<_> =
            offsets.into_iter().map(|at| (at, marker)).collect();
        insertions.extend(currency.into_iter().map(|at| (at, '\\')));
        insertions.sort_by_key(|&(at, _)| at);
        let (text, inserted) = prepare_insertions(source, &insertions);
        Some(Self {
            text,
            marker,
            inserted,
        })
    }

    fn original_offset(&self, offset: usize) -> usize {
        let index = self.inserted.partition_point(|range| range.end <= offset);
        let removed: usize =
            self.inserted[..index].iter().map(|range| range.len()).sum();
        let partial = self.inserted.get(index).map_or(0, |range| {
            offset.saturating_sub(range.start).min(range.len())
        });
        offset - removed - partial
    }

    fn restore_point(&self, location: &Location, point: &mut unist::Point) {
        if let Some(original) =
            location.to_point(self.original_offset(point.offset))
        {
            *point = original;
        }
    }

    fn restore_stops(&self, stops: &mut [mdast::Stop]) {
        for (_, absolute) in stops {
            *absolute = self.original_offset(*absolute);
        }
    }

    fn restore_attributes(&self, attributes: &mut [mdast::AttributeContent]) {
        for attribute in attributes {
            match attribute {
                mdast::AttributeContent::Expression(expression) => {
                    self.restore_stops(&mut expression.stops)
                }
                mdast::AttributeContent::Property(property) => {
                    if let Some(mdast::AttributeValue::Expression(expression)) =
                        &mut property.value
                    {
                        self.restore_stops(&mut expression.stops);
                    }
                }
            }
        }
    }

    fn restore_node(&self, source: &str, root: &mut mdast::Node) {
        fn walk(
            prepared: &Prepared,
            location: &Location,
            node: &mut mdast::Node,
        ) {
            if let Some(position) = node.position_mut() {
                prepared.restore_point(location, &mut position.start);
                prepared.restore_point(location, &mut position.end);
            }
            match node {
                mdast::Node::Text(text) => {
                    text.value = text.value.replace(prepared.marker, "")
                }
                mdast::Node::InlineMath(math) => {
                    math.value = analysis::trim_math(
                        &math.value.replace(prepared.marker, ""),
                    );
                }
                mdast::Node::MdxjsEsm(expression) => {
                    prepared.restore_stops(&mut expression.stops)
                }
                mdast::Node::MdxFlowExpression(expression) => {
                    prepared.restore_stops(&mut expression.stops)
                }
                mdast::Node::MdxTextExpression(expression) => {
                    prepared.restore_stops(&mut expression.stops)
                }
                mdast::Node::MdxJsxFlowElement(element) => {
                    prepared.restore_attributes(&mut element.attributes)
                }
                mdast::Node::MdxJsxTextElement(element) => {
                    prepared.restore_attributes(&mut element.attributes)
                }
                _ => {}
            }
            if let Some(children) = node.children_mut() {
                for child in children {
                    walk(prepared, location, child);
                }
            }
        }
        walk(self, &Location::new(source.as_bytes()), root);
    }
}

fn prepare(
    source: &str,
    offsets: &[usize],
    marker: char,
) -> (String, Vec<Range<usize>>) {
    prepare_insertions(
        source,
        &offsets.iter().map(|&at| (at, marker)).collect::<Vec<_>>(),
    )
}

fn prepare_insertions(
    source: &str,
    insertions: &[(usize, char)],
) -> (String, Vec<Range<usize>>) {
    let mut text = String::with_capacity(source.len() + insertions.len() * 3);
    let mut inserted = Vec::with_capacity(insertions.len());
    let mut previous = 0;
    for &(offset, character) in insertions {
        text.push_str(&source[previous..offset]);
        let start = text.len();
        text.push(character);
        inserted.push(start..text.len());
        previous = offset;
    }
    text.push_str(&source[previous..]);
    (text, inserted)
}

fn insertion_offsets(source: &str, root: &mdast::Node) -> Vec<usize> {
    fn prose_ranges(node: &mdast::Node, ranges: &mut Vec<Range<usize>>) {
        if let mdast::Node::Text(text) = node {
            if let Some(position) = &text.position {
                ranges.push(position.start.offset..position.end.offset);
            }
        }
        // A strict parse may incorrectly pair the middle delimiters of two
        // neighboring CJK phrases (e.g. **“one”**和**（two）**). Include the
        // already-paired delimiters so normalization can repair both pairs.
        if matches!(node, mdast::Node::Strong(_) | mdast::Node::Emphasis(_)) {
            if let Some(position) = node.position() {
                let width = if matches!(node, mdast::Node::Strong(_)) {
                    2
                } else {
                    1
                };
                ranges
                    .push(position.start.offset..position.start.offset + width);
                ranges.push(position.end.offset - width..position.end.offset);
            }
        }
        if let Some(children) = node.children() {
            for child in children {
                prose_ranges(child, ranges);
            }
        }
    }
    let mut ranges = Vec::new();
    prose_ranges(root, &mut ranges);
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(previous) = merged
            .last_mut()
            .filter(|previous| range.start <= previous.end)
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    let ranges = merged;
    let bytes = source.as_bytes();
    let mut offsets = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'*' {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index] == b'*' {
            index += 1;
        }
        let end = index;
        if end - start > 3 {
            continue;
        }
        // Only raw prose delimiters may be altered, never code or a URL/title.
        let prose = ranges.partition_point(|range| range.end <= start);
        if !ranges
            .get(prose)
            .is_some_and(|range| range.start <= start && end <= range.end)
        {
            continue;
        }
        let slashes = bytes[..start]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count();
        if slashes % 2 != 0 {
            continue;
        }
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if before.is_some_and(is_punctuation)
            && (after.is_some_and(is_cjk_letter)
                || (before.is_some_and(is_cjk_punctuation)
                    && after.is_some_and(char::is_alphanumeric)))
        {
            offsets.push(start);
        }
        if (before.is_some_and(is_cjk_letter)
            || (before.is_some_and(char::is_alphabetic)
                && after.is_some_and(is_cjk_punctuation)))
            && after.is_some_and(is_punctuation)
        {
            offsets.push(end);
        }
    }
    offsets
}

fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            get_general_category(ch),
            GeneralCategory::ConnectorPunctuation
                | GeneralCategory::DashPunctuation
                | GeneralCategory::OpenPunctuation
                | GeneralCategory::ClosePunctuation
                | GeneralCategory::InitialPunctuation
                | GeneralCategory::FinalPunctuation
                | GeneralCategory::OtherPunctuation
        )
}

fn is_cjk_letter(ch: char) -> bool {
    matches!(ch,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
        | '\u{20000}'..='\u{2fa1f}' | '\u{30000}'..='\u{3347f}'
        | '\u{3041}'..='\u{3096}' | '\u{30a1}'..='\u{30fa}' | '\u{31f0}'..='\u{31ff}'
        | '\u{1100}'..='\u{11ff}' | '\u{3131}'..='\u{318e}' | '\u{a960}'..='\u{a97f}'
        | '\u{ac00}'..='\u{d7af}' | '\u{d7b0}'..='\u{d7ff}')
}

// CJK punctuation remains a CJK boundary when the neighboring prose switches
// to Latin letters or digits, as in **中文。**EML. Keep ASCII punctuation's
// CommonMark rules and the existing restriction on digit-led opening markers.
fn is_cjk_punctuation(ch: char) -> bool {
    is_punctuation(ch)
        && matches!(ch,
            '\u{3001}'..='\u{303f}' | '\u{fe10}'..='\u{fe1f}'
            | '\u{fe30}'..='\u{fe4f}' | '\u{ff01}'..='\u{ff65}')
}

// PageMD accepts padded inline math. Pulldown-cmark's delimiter flanking
// needs temporary non-whitespace neighbors; source mapping removes them.
fn padded_math_offsets(
    source: &str,
    root: &mdast::Node,
    currency: &[usize],
) -> Vec<usize> {
    fn collect(node: &mdast::Node, ranges: &mut Vec<Range<usize>>) {
        if let mdast::Node::Text(text) = node {
            if let Some(position) = &text.position {
                ranges.push(position.start.offset..position.end.offset);
            }
        }
        if let Some(children) = node.children() {
            for child in children {
                collect(child, ranges);
            }
        }
    }
    let mut ranges = Vec::new();
    collect(root, &mut ranges);
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let escaped = |offset: usize| {
        bytes[..offset]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count()
            % 2
            != 0
    };
    for range in ranges {
        let mut i = range.start;
        while i < range.end {
            if bytes[i] != b'$'
                || escaped(i)
                || currency.contains(&i)
                || bytes.get(i + 1) == Some(&b'$')
                || (i > 0 && bytes[i - 1] == b'$')
            {
                i += 1;
                continue;
            }
            let start = i;
            let mut end = i + 1;
            while end < range.end && !matches!(bytes[end], b'\n' | b'\r') {
                if bytes[end] == b'$'
                    && !escaped(end)
                    && !currency.contains(&end)
                {
                    break;
                }
                end += 1;
            }
            if end >= range.end || bytes[end] != b'$' {
                i += 1;
                continue;
            }
            let value = &source[start + 1..end];
            if !value.trim().is_empty()
                && (value.starts_with(char::is_whitespace)
                    || value.ends_with(char::is_whitespace))
            {
                out.push(start + 1);
                out.push(end);
            }
            i = end + 1;
        }
    }
    out
}

// A dollar immediately followed by a digit cannot close inline math: in
// "$5 and $10" it starts the next price. Escape both dollars only in the
// prepared parse, preserving the original bytes and source coordinates.
fn currency_math_offsets(source: &str, root: &mdast::Node) -> Vec<usize> {
    fn visit(source: &str, node: &mdast::Node, offsets: &mut Vec<usize>) {
        if matches!(node, mdast::Node::Text(_) | mdast::Node::InlineMath(_)) {
            if let Some(position) = node.position() {
                let bytes = source.as_bytes();
                let eligible = |i: usize| {
                    bytes[i] == b'$'
                        && bytes.get(i + 1) != Some(&b'$')
                        && (i == 0 || bytes[i - 1] != b'$')
                        && bytes[..i]
                            .iter()
                            .rev()
                            .take_while(|&&b| b == b'\\')
                            .count()
                            % 2
                            == 0
                };
                let mut start = position.start.offset;
                let limit = position.end.offset;
                while start < limit {
                    if !eligible(start) {
                        start += 1;
                        continue;
                    }
                    let mut end = start + 1;
                    while end < limit
                        && !matches!(bytes[end], b'\n' | b'\r')
                        && !eligible(end)
                    {
                        end += 1;
                    }
                    if end < limit && eligible(end) {
                        if bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
                            offsets.extend([start, end]);
                        }
                        start = end + 1;
                    } else {
                        start += 1;
                    }
                }
            }
        }
        if let Some(children) = node.children() {
            for child in children {
                visit(source, child, offsets);
            }
        }
    }
    let mut offsets = Vec::new();
    visit(source, root, &mut offsets);
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}
