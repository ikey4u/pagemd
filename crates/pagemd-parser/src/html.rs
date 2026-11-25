//! The HTML a PageMD document may hold, for every renderer: one limited set
//! of elements, attributes and style properties ([`INLINE_ELEMENTS`],
//! [`BLOCK_ELEMENTS`], [`STYLE_PROPERTIES`]). The HTML renderer keeps exactly
//! this set when it renders safely ([`HtmlCleaner`]); the native (GPUI)
//! renderer draws the same elements. Also the attributes written after a
//! Markdown image (`![a](x.png){width=320 .wide}`), read the same way by both.
//!
//! Anything else is left out the same way everywhere: scripts, styles,
//! frames, forms' fields and embedded objects go with what they contain;
//! other elements (`section`, `font`, `svg`…) are unwrapped, their text kept.

use std::fmt::Write as _;

/// Inline elements: text styles, links and images.
pub const INLINE_ELEMENTS: &[&str] = &[
    "a", "abbr", "b", "strong", "i", "em", "u", "ins", "s", "del", "strike",
    "mark", "sub", "sup", "kbd", "code", "small", "span", "br", "img",
];

/// Block elements: what Markdown lacks (centering, figures, foldable
/// details, media) and the blocks it has, written as HTML.
pub const BLOCK_ELEMENTS: &[&str] = &[
    "p",
    "div",
    "center",
    "figure",
    "figcaption",
    "details",
    "summary",
    "blockquote",
    "pre",
    "hr",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "caption",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "video",
    "audio",
    "source",
    "picture",
];

/// Elements dropped with all they contain.
const DROP_WITH_CONTENT: &[&str] = &[
    "script",
    "style",
    "noscript",
    "template",
    "iframe",
    "frame",
    "frameset",
    "object",
    "embed",
    "applet",
    "xmp",
    "plaintext",
    "title",
    "head",
    "textarea",
    "select",
    "svg",
    "math",
    "canvas",
    "button",
];

/// CSS properties a `style` may set: size, alignment, spacing and a simple
/// look; nothing that loads anything.
pub const STYLE_PROPERTIES: &[&str] = &[
    "width",
    "height",
    "max-width",
    "max-height",
    "min-width",
    "min-height",
    "text-align",
    "vertical-align",
    "border-radius",
    "float",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "color",
    "background-color",
    "font-weight",
    "font-style",
    "font-size",
    "text-decoration",
    "display",
    "gap",
    "flex-wrap",
    "justify-content",
    "align-items",
    "object-fit",
    "border",
    "opacity",
];

/// Attributes any element of the set may carry.
const GLOBAL_ATTRIBUTES: &[&str] =
    &["id", "class", "title", "lang", "dir", "style", "align"];

/// Attributes holding an address.
const URL_ATTRIBUTES: &[&str] = &["href", "src", "srcset", "poster"];

/// Whether an element is in the set.
pub fn is_allowed_element(name: &str) -> bool {
    INLINE_ELEMENTS.contains(&name) || BLOCK_ELEMENTS.contains(&name)
}

/// Whether an element is dropped with everything in it. Elements neither
/// allowed nor dropped are unwrapped: their content stays.
pub fn is_dropped_with_content(name: &str) -> bool {
    DROP_WITH_CONTENT.contains(&name)
}

/// The attributes an element may carry, besides the global ones.
fn element_attributes(element: &str) -> &'static [&'static str] {
    match element {
        "a" => &["href", "name"],
        "img" => &["src", "alt", "width", "height", "loading"],
        "video" => &[
            "src",
            "poster",
            "controls",
            "loop",
            "muted",
            "playsinline",
            "preload",
            "width",
            "height",
        ],
        "audio" => &["src", "controls", "loop", "muted", "preload"],
        "source" => &["src", "type", "srcset", "media"],
        "td" | "th" => &["colspan", "rowspan", "valign", "width"],
        "ol" => &["start", "reversed", "type"],
        "li" => &["value"],
        "details" => &["open"],
        "table" => &["width"],
        _ => &[],
    }
}

/// Keeps the set of HTML written in Markdown. It works tag by tag, not on a
/// tree: Markdown gives block HTML in pieces (`<details>`, the Markdown
/// inside, `</details>`), and each piece is cleaned as it comes, with what
/// is still open (a dropped `<script>`) carried to the next.
#[derive(Debug, Default)]
pub struct HtmlCleaner {
    /// Inside an element dropped with its content: its name.
    skipping: Option<String>,
}

impl HtmlCleaner {
    pub fn new() -> Self {
        Self::default()
    }

    /// The part of `raw` in the set.
    pub fn clean(&mut self, raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut rest = raw;
        while !rest.is_empty() {
            let Some(lt) = rest.find('<') else {
                if self.skipping.is_none() {
                    out.push_str(rest);
                }
                break;
            };
            if self.skipping.is_none() {
                out.push_str(&rest[..lt]);
            }
            rest = &rest[lt..];
            // A comment, a doctype or a processing instruction: dropped.
            if let Some(after) = rest.strip_prefix("<!--") {
                rest = after.find("-->").map_or("", |end| &after[end + 3..]);
                continue;
            }
            if rest.starts_with("<!") || rest.starts_with("<?") {
                rest = rest.find('>').map_or("", |end| &rest[end + 1..]);
                continue;
            }
            match parse_tag(rest) {
                Some((tag, len)) => {
                    rest = &rest[len..];
                    self.tag(&tag, &mut out);
                }
                None => {
                    // A lone `<` (as in `a < b`): text.
                    if self.skipping.is_none() {
                        out.push_str("&lt;");
                    }
                    rest = &rest[1..];
                }
            }
        }
        out
    }

    fn tag(&mut self, tag: &Tag, out: &mut String) {
        if let Some(name) = &self.skipping {
            if tag.closing && &tag.name == name {
                self.skipping = None;
            }
            return;
        }
        if is_dropped_with_content(&tag.name) {
            if !tag.closing && !tag.self_closing {
                self.skipping = Some(tag.name.clone());
            }
            return;
        }
        if !is_allowed_element(&tag.name) {
            return;
        }
        if tag.closing {
            let _ = write!(out, "</{}>", tag.name);
            return;
        }
        let _ = write!(out, "<{}", tag.name);
        for (name, value) in safe_attributes(tag) {
            match value {
                Some(value) => {
                    let _ =
                        write!(out, " {name}=\"{}\"", attribute_escape(&value));
                }
                None => {
                    let _ = write!(out, " {name}");
                }
            }
        }
        out.push_str(if tag.self_closing { " />" } else { ">" });
    }
}

/// The attributes of an allowed tag that the set keeps (values decoded and
/// filtered; `None` for a bare attribute).
pub fn safe_attributes(tag: &Tag) -> Vec<(String, Option<String>)> {
    tag.attributes
        .iter()
        .filter_map(|(name, value)| {
            let allowed = GLOBAL_ATTRIBUTES.contains(&name.as_str())
                || element_attributes(&tag.name).contains(&name.as_str())
                || name.starts_with("aria-");
            if !allowed || name.starts_with("on") {
                return None;
            }
            let Some(value) = value else {
                return Some((name.clone(), None));
            };
            let value = decode_entities(value);
            if URL_ATTRIBUTES.contains(&name.as_str()) {
                let addresses: Vec<&str> = if name == "srcset" {
                    value
                        .split(',')
                        .map(|part| {
                            part.split_whitespace().next().unwrap_or("")
                        })
                        .collect()
                } else {
                    vec![value.as_str()]
                };
                let images =
                    matches!(name.as_str(), "src" | "srcset" | "poster")
                        && matches!(
                            tag.name.as_str(),
                            "img" | "source" | "video"
                        );
                if addresses.iter().any(|address| !safe_url(address, images)) {
                    return None;
                }
            }
            if name == "style" {
                return filter_style(&value)
                    .map(|style| (name.clone(), Some(style)));
            }
            Some((name.clone(), Some(value)))
        })
        .collect()
}

/// Whether an address may be followed or shown: no script, no data but an
/// image's.
pub fn safe_url(address: &str, image: bool) -> bool {
    let compact: String = address
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    let Some(colon) = compact.find(':') else {
        return true;
    };
    // A colon after a slash, `?` or `#` is part of a path, not a scheme.
    if compact[..colon].contains(['/', '?', '#']) {
        return true;
    }
    match &compact[..colon] {
        "http" | "https" | "mailto" | "tel" | "asset" | "lirra-media" => true,
        "data" => image && compact.starts_with("data:image/"),
        _ => false,
    }
}

/// The declarations of a `style` the set allows (see [`STYLE_PROPERTIES`]);
/// none when nothing is left. Values that load or run anything are dropped.
pub fn filter_style(style: &str) -> Option<String> {
    let kept: Vec<String> = style
        .split(';')
        .filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let property = property.trim().to_ascii_lowercase();
            let value = value.trim();
            let lowered: String = value
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase();
            let loads = ["url(", "expression(", "javascript:", "@import", "\\"]
                .iter()
                .any(|bad| lowered.contains(bad));
            (STYLE_PROPERTIES.contains(&property.as_str())
                && !value.is_empty()
                && !loads)
                .then(|| format!("{property}:{value}"))
        })
        .collect();
    (!kept.is_empty()).then(|| kept.join(";"))
}

/// A CSS length as pixels, when it is one (`320`, `320px`); none for
/// relative lengths (`50%`, `20em`), which only a browser lays out.
pub fn length_px(value: &str) -> Option<f32> {
    let value = value.trim();
    let number = value.strip_suffix("px").unwrap_or(value).trim();
    number
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

/// The value of one property in a `style`.
pub fn style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(property)
            .then(|| value.trim())
    })
}

/// A tag as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    /// Lower-cased.
    pub name: String,
    pub closing: bool,
    pub self_closing: bool,
    /// Lower-cased names, values as written.
    pub attributes: Vec<(String, Option<String>)>,
}

impl Tag {
    /// An attribute's value, decoded.
    pub fn attribute(&self, name: &str) -> Option<String> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
            .map(decode_entities)
    }
}

/// A tag at the start of `text` and its length; none when it is not one.
pub fn parse_tag(text: &str) -> Option<(Tag, usize)> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let mut i = 1;
    let closing = bytes.get(i) == Some(&b'/');
    if closing {
        i += 1;
    }
    let start = i;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric()
            || bytes[i] == b'-'
            || bytes[i] == b':')
    {
        i += 1;
    }
    if i == start || !bytes[start].is_ascii_alphabetic() {
        return None;
    }
    let name = text[start..i].to_ascii_lowercase();
    let mut attributes = Vec::new();
    let mut self_closing = false;
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => return None,
            Some(b'>') => {
                i += 1;
                break;
            }
            Some(b'/') => {
                self_closing = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        let attr_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        let attr = text[attr_start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = None;
        if bytes.get(i) == Some(&b'=') {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            match bytes.get(i) {
                Some(&quote @ (b'"' | b'\'')) => {
                    let end = text[i + 1..].find(quote as char)? + i + 1;
                    value = Some(text[i + 1..end].to_string());
                    i = end + 1;
                }
                Some(_) => {
                    let v_start = i;
                    while i < bytes.len()
                        && !bytes[i].is_ascii_whitespace()
                        && bytes[i] != b'>'
                    {
                        i += 1;
                    }
                    value = Some(text[v_start..i].to_string());
                }
                None => return None,
            }
        }
        if !attr.is_empty() {
            attributes.push((attr, value));
        }
    }
    Some((
        Tag {
            name,
            closing,
            self_closing,
            attributes,
        },
        i,
    ))
}

/// The entities an attribute is likely to hold, decoded, so that an
/// address's scheme is seen as a browser sees it.
pub fn decode_entities(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let entity = &rest[1..e];
            match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "colon" => Some(':'),
                "tab" => Some('\t'),
                "newline" => Some('\n'),
                _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                    u32::from_str_radix(&entity[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                }
                _ if entity.starts_with('#') => {
                    entity[1..].parse().ok().and_then(char::from_u32)
                }
                _ => None,
            }
        });
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A value escaped for a double-quoted attribute.
pub fn attribute_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// What `{width=320 height=200 .class #id style="…" align=center}` after an
/// image gives it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageAttributes {
    /// As written: `320`, `320px`, `50%`, `20em`.
    pub width: Option<String>,
    pub height: Option<String>,
    pub align: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Filtered by [`filter_style`].
    pub style: Option<String>,
    pub title: Option<String>,
    pub alt: Option<String>,
}

impl ImageAttributes {
    /// As HTML attributes (each with a leading space); sizes that are not
    /// plain pixels go into the style.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        let mut style = Vec::new();
        for (key, value) in [("width", &self.width), ("height", &self.height)] {
            if let Some(value) = value {
                if value.chars().all(|c| c.is_ascii_digit()) {
                    let _ = write!(out, " {key}=\"{value}\"");
                } else {
                    style.push(format!("{key}:{value}"));
                }
            }
        }
        for (key, value) in [
            ("align", &self.align),
            ("id", &self.id),
            ("title", &self.title),
            ("alt", &self.alt),
        ] {
            if let Some(value) = value {
                let _ = write!(out, " {key}=\"{}\"", attribute_escape(value));
            }
        }
        if let Some(own) = &self.style {
            style.push(own.clone());
        }
        if !style.is_empty() {
            let _ = write!(
                out,
                " style=\"{}\"",
                attribute_escape(&style.join(";"))
            );
        }
        if !self.classes.is_empty() {
            let _ = write!(out, " class=\"{}\"", self.classes.join(" "));
        }
        out
    }

    /// The width in pixels, from the attribute or the style.
    pub fn width_px(&self) -> Option<f32> {
        self.width.as_deref().and_then(length_px).or_else(|| {
            self.style
                .as_deref()
                .and_then(|s| style_property(s, "width"))
                .and_then(length_px)
        })
    }

    /// The height in pixels, from the attribute or the style.
    pub fn height_px(&self) -> Option<f32> {
        self.height.as_deref().and_then(length_px).or_else(|| {
            self.style
                .as_deref()
                .and_then(|s| style_property(s, "height"))
                .and_then(length_px)
        })
    }
}

/// The attributes in `{…}` after an image (the braces' content); none when
/// it is not such a list (`{see below}` stays text).
pub fn parse_image_attributes(spec: &str) -> Option<ImageAttributes> {
    let spec = decode_entities(spec);
    let mut attributes = ImageAttributes::default();
    let mut rest = spec.trim();
    if rest.is_empty() {
        return None;
    }
    let plain = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    };
    while !rest.is_empty() {
        let token_end = if let Some(eq) = rest
            .find('=')
            .filter(|&eq| !rest[..eq].contains(char::is_whitespace))
        {
            let after = &rest[eq + 1..];
            if let Some(quoted) = after.strip_prefix('"') {
                eq + 2 + quoted.find('"')? + 1
            } else {
                eq + 1 + after.find(char::is_whitespace).unwrap_or(after.len())
            }
        } else {
            rest.find(char::is_whitespace).unwrap_or(rest.len())
        };
        let token = &rest[..token_end];
        rest = rest[token_end..].trim_start();
        if let Some(class) = token.strip_prefix('.') {
            if !plain(class) {
                return None;
            }
            attributes.classes.push(class.to_string());
            continue;
        }
        if let Some(id) = token.strip_prefix('#') {
            if !plain(id) {
                return None;
            }
            attributes.id = Some(id.to_string());
            continue;
        }
        let (key, value) = token.split_once('=')?;
        let value = value.trim_matches('"').to_string();
        match key {
            "width" | "height" => {
                let number = value.trim_end_matches(|c: char| {
                    c.is_ascii_alphabetic() || c == '%'
                });
                if number.parse::<f32>().is_err() {
                    return None;
                }
                if key == "width" {
                    attributes.width = Some(value);
                } else {
                    attributes.height = Some(value);
                }
            }
            "align" => attributes.align = Some(value),
            "title" => attributes.title = Some(value),
            "alt" => attributes.alt = Some(value),
            "style" => attributes.style = filter_style(&value),
            _ => return None,
        }
    }
    Some(attributes)
}

/// `{…}` at the very start of `text` (what follows an image), parsed, and
/// the text after it.
pub fn leading_image_attributes(text: &str) -> Option<(ImageAttributes, &str)> {
    let body = text.strip_prefix('{')?;
    let end = body
        .find('}')
        .filter(|&e| e <= 400 && !body[..e].contains(['\n', '<']))?;
    Some((parse_image_attributes(&body[..end])?, &body[end + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(raw: &str) -> String {
        HtmlCleaner::new().clean(raw)
    }

    #[test]
    fn keeps_the_set() {
        let raw = r#"<img src="asset://localhost/a/cat.png" width="320" style="border-radius:8px; max-width: 100%" alt="Cat">"#;
        assert_eq!(
            clean(raw),
            r#"<img src="asset://localhost/a/cat.png" width="320" style="border-radius:8px;max-width:100%" alt="Cat">"#
        );
        assert_eq!(
            clean("<details open><summary>More</summary>"),
            "<details open><summary>More</summary>"
        );
        assert_eq!(
            clean("<kbd>Ctrl</kbd> H<sub>2</sub>O <mark>m</mark> <u>u</u>"),
            "<kbd>Ctrl</kbd> H<sub>2</sub>O <mark>m</mark> <u>u</u>"
        );
        assert_eq!(
            clean(r#"<p align="center"><img src="x.png"></p>"#),
            r#"<p align="center"><img src="x.png"></p>"#
        );
        assert_eq!(
            clean(
                r#"<video src="/p/a/media/v.mp4" controls playsinline></video>"#
            ),
            r#"<video src="/p/a/media/v.mp4" controls playsinline></video>"#
        );
        assert_eq!(
            clean(
                r#"<div style="display:flex;gap:8px;position:fixed">x</div>"#
            ),
            r#"<div style="display:flex;gap:8px">x</div>"#
        );
    }

    #[test]
    fn outside_the_set_is_unwrapped_or_dropped() {
        assert_eq!(clean("a<script>alert(1)</script>b"), "ab");
        // A script given in pieces, as Markdown hands block HTML over.
        let mut cleaner = HtmlCleaner::new();
        assert_eq!(cleaner.clean("<script>"), "");
        assert_eq!(cleaner.clean("alert(1)"), "");
        assert_eq!(cleaner.clean("</script><b>ok</b>"), "<b>ok</b>");
        assert_eq!(
            clean(r#"<img src="x.png" onerror="alert(2)" onclick=go()>"#),
            r#"<img src="x.png">"#
        );
        assert_eq!(clean(r#"<a href="javascript:alert(1)">x</a>"#), "<a>x</a>");
        assert_eq!(
            clean(r#"<a href="jav&#x61;script:alert(1)">x</a>"#),
            "<a>x</a>"
        );
        assert_eq!(
            clean(r#"<a href=" JaVaScRiPt:alert(1)">x</a>"#),
            "<a>x</a>"
        );
        assert_eq!(clean(r#"<img src="data:text/html;base64,AAA">"#), "<img>");
        assert_eq!(
            clean(r#"<img src="data:image/png;base64,AAA">"#),
            r#"<img src="data:image/png;base64,AAA">"#
        );
        assert_eq!(
            clean(r#"<div style="background:url(javascript:x)">t</div>"#),
            "<div>t</div>"
        );
        assert_eq!(clean(r#"<iframe src="https://x"></iframe>after"#), "after");
        assert_eq!(
            clean(r#"<form action="/x"><input name="a">text</form>"#),
            "text"
        );
        assert_eq!(
            clean(r#"<section class="x"><svg><circle/></svg>kept</section>"#),
            "kept"
        );
        assert_eq!(
            clean(r#"<span data-x="1" onmouseover="x()">s</span>"#),
            "<span>s</span>"
        );
        assert_eq!(clean("<!-- note -->kept"), "kept");
        assert_eq!(clean("1 < 2"), "1 &lt; 2");
    }

    #[test]
    fn addresses() {
        assert!(safe_url("images/a:b.png", true));
        assert!(safe_url("#notes", false));
        assert!(safe_url("lirra-media://abc.png", true));
        assert!(!safe_url("vbscript:x", false));
    }

    #[test]
    fn image_attributes() {
        let a = parse_image_attributes(
            r#"width=320 .wide #fig-1 style="border-radius:8px""#,
        )
        .unwrap();
        assert_eq!(
            a.to_html(),
            r#" width="320" id="fig-1" style="border-radius:8px" class="wide""#
        );
        assert_eq!(a.width_px(), Some(320.0));
        let b = parse_image_attributes("width=50% height=10em").unwrap();
        assert_eq!(b.to_html(), r#" style="width:50%;height:10em""#);
        assert_eq!(b.width_px(), None);
        assert_eq!(parse_image_attributes("see below"), None);
        assert_eq!(
            parse_image_attributes(r#"style="background:url(x)""#)
                .unwrap()
                .to_html(),
            ""
        );
        assert_eq!(
            parse_image_attributes(r#"style="width:240px""#)
                .unwrap()
                .width_px(),
            Some(240.0)
        );
        let (attrs, rest) =
            leading_image_attributes("{width=80} and more").unwrap();
        assert_eq!((attrs.width_px(), rest), (Some(80.0), " and more"));
    }
}
