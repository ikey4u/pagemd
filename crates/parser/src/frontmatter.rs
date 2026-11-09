//! Validated document metadata with a renderer-independent field tree.
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

/// A presentation tree independent of serialization syntax.
#[derive(Clone, Debug)]
pub enum Value {
    Scalar(String),
    Object(Vec<(String, Value)>),
    List(Vec<Value>),
}

pub fn presentation(language: &str, source: &str) -> Option<Value> {
    fn yaml(value: serde_yaml::Value) -> Value {
        use serde_yaml::Value as Y;
        match value {
            Y::Mapping(items) => Value::Object(
                items
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            match key {
                                Y::String(s) => s,
                                other => serde_yaml::to_string(&other)
                                    .unwrap_or_default()
                                    .trim()
                                    .to_owned(),
                            },
                            yaml(value),
                        )
                    })
                    .collect(),
            ),
            Y::Sequence(items) => {
                Value::List(items.into_iter().map(yaml).collect())
            }
            Y::String(s) => Value::Scalar(s),
            Y::Tagged(tag) => yaml(tag.value),
            other => Value::Scalar(
                serde_yaml::to_string(&other)
                    .ok()
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            ),
        }
    }
    if source.trim().is_empty() { return Some(Value::Object(vec![])); }
    let value = match language {
        "yaml" => serde_yaml::from_str(source).ok()?,
        "json" => serde_yaml::to_value(
            serde_json::from_str::<serde_json::Value>(source).ok()?,
        )
        .ok()?,
        "toml" => {
            serde_yaml::to_value(toml::from_str::<toml::Table>(source).ok()?)
                .ok()?
        }
        _ => return None,
    };
    Some(yaml(value))
}

pub fn render_html(language: &str, source: &str) -> Option<String> {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    fn render(value: &Value) -> String {
        match value {
            Value::Scalar(s) => format!("<span style=\"white-space:pre-wrap;overflow-wrap:anywhere\">{}</span>", escape(s)),
            Value::Object(fields) if fields.is_empty() => "<span>{}</span>".to_owned(),
            Value::List(items) if items.is_empty() => "<span>[]</span>".to_owned(),
            Value::Object(fields) => format!("<dl style=\"margin:0\">{}</dl>", fields.iter().map(|(key,value)| format!("<dt style=\"font-weight:600;margin-top:.65em\">{}</dt><dd style=\"margin:0 0 0 1em\">{}</dd>",escape(key),render(value))).collect::<String>()),
            Value::List(items) => format!("<ul>{}</ul>",items.iter().map(|item|format!("<li>{}</li>",render(item))).collect::<String>()),
        }
    }
    Some(format!(
        "<section class=\"pagemd-metadata\">{}</section>",
        render(&presentation(language, source)?)
    ))
}
