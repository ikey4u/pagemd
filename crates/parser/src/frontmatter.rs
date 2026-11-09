//! Validated document metadata, displayed through the ordinary code renderer.
//! Only a complete mapping/object at byte zero (optionally after a BOM) qualifies.
//! Invalid or incomplete input remains ordinary Markdown, including thematic breaks.
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatter {
    pub language: &'static str,
    /// Includes delimiters and their final newline when present.
    pub range: Range<usize>,
    /// Original bytes, excluding delimiters. JSON includes its object braces.
    pub value_range: Range<usize>,
}

pub fn detect(source: &str) -> Option<FrontMatter> {
    let start = source.len() - source.trim_start_matches('\u{feff}').len();
    let body = &source[start..];
    let first = body.split_inclusive('\n').next()?;
    let delimiter = first.trim_end_matches(['\r', '\n', ' ', '\t']);
    if matches!(delimiter, "---" | "+++") {
        if !first.ends_with('\n') {
            return None;
        }
        let value_start = start + first.len();
        let mut offset = value_start;
        for line in source[value_start..].split_inclusive('\n') {
            let close = line.trim_end_matches(['\r', '\n', ' ', '\t']);
            if close == delimiter || (delimiter == "---" && close == "...") {
                let value = &source[value_start..offset];
                let language = if delimiter == "---" {
                    if !value.trim().is_empty()
                        && !matches!(
                            serde_yaml::from_str::<serde_yaml::Value>(value)
                                .ok()?,
                            serde_yaml::Value::Mapping(_)
                        )
                    {
                        return None;
                    }
                    "yaml"
                } else {
                    toml::from_str::<toml::Table>(value).ok()?;
                    "toml"
                };
                return Some(FrontMatter {
                    language,
                    range: 0..offset + line.len(),
                    value_range: value_start..offset,
                });
            }
            offset += line.len();
        }
        return None;
    }
    if !body.starts_with('{') {
        return None;
    }
    // The JSON parser owns string escaping, nesting and braces inside strings.
    let mut values = serde_json::Deserializer::from_str(body)
        .into_iter::<serde_json::Value>();
    if !values.next()?.ok()?.is_object() {
        return None;
    }
    let end = start + values.byte_offset();
    let rest = &source[end..];
    let line_end = rest.find('\n').map_or(rest.len(), |n| n + 1);
    if !rest[..line_end].trim().is_empty() {
        return None;
    }
    Some(FrontMatter {
        language: "json",
        range: 0..end + line_end,
        value_range: start..end,
    })
}
