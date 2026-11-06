//! A bounded, display-only OpenAPI projection. Parsing never performs I/O.
//! Hosts resolve `src` and external references before calling `parse_with_refs`.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

mod html;
pub use html::{render_html, CSS};
mod sources;
pub use sources::parse_loaded;
#[cfg(feature = "io")]
mod io;
#[cfg(feature = "io")]
pub use io::{load_blocking, read_blocking};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ITEMS: usize = 10_000;
const MAX_DEPTH: usize = 24;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Document {
    pub operations: Vec<Operation>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Operation {
    pub method: String,
    pub path: String,
    pub request: Vec<Item>,
    pub responses: Vec<Item>,
    pub comment: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Item {
    pub name: String,
    pub metadata: String,
    pub description: String,
    pub children: Vec<Item>,
}

/// Parse only the fence attributes, never Markdown using a regular expression.
/// An inline body and a source are mutually exclusive.
pub fn source_attribute(
    meta: &str,
    body: &str,
) -> Result<Option<String>, String> {
    let meta = meta.trim();
    if meta.is_empty() {
        return Ok(None);
    }
    let value = meta
        .strip_prefix("src=")
        .ok_or("Expected src=\"…\" after openapi")?;
    let quote = value
        .chars()
        .next()
        .filter(|c| matches!(c, '\'' | '"'))
        .ok_or("OpenAPI src must be quoted")?;
    let rest = &value[quote.len_utf8()..];
    let end = rest
        .find(quote)
        .ok_or("OpenAPI src is missing its closing quote")?;
    let source = &rest[..end];
    if source.is_empty()
        || !rest[end + quote.len_utf8()..].trim().is_empty()
        || source.chars().any(char::is_control)
    {
        return Err("Expected one nonempty OpenAPI src attribute".into());
    }
    if !body.trim().is_empty() {
        return Err("Use either an inline OpenAPI body or src, not both".into());
    }
    Ok(Some(source.to_owned()))
}

/// Is the fenced block complete? Avoid issuing I/O for a streaming opening fence.
pub fn closed_fence(source: &str) -> bool {
    let mut lines = source.lines();
    let Some(first) = lines.next() else {
        return false;
    };
    let first = first.trim_start();
    let Some(marker @ ('`' | '~')) = first.chars().next() else {
        return false;
    };
    let length = first.chars().take_while(|c| *c == marker).count();
    length >= 3
        && lines.any(|line| {
            let line = line.trim();
            line.chars().count() >= length && line.chars().all(|c| c == marker)
        })
}

pub fn parse(source: &str) -> Result<Document, String> {
    parse_with_refs(source, &BTreeMap::new())
}

pub fn decode(source: &str) -> Result<Value, String> {
    if source.len() > MAX_BYTES {
        return Err("OpenAPI source exceeds the 2 MiB display limit".into());
    }
    serde_yaml::from_str(source)
        .map_err(|e| format!("Invalid OpenAPI YAML/JSON: {e}"))
}

/// External documents are keyed by the literal reference URI (without fragment).
/// Their own references must have been rebased by the host resolver.
pub fn parse_with_refs(
    source: &str,
    documents: &BTreeMap<String, Value>,
) -> Result<Document, String> {
    let root = decode(source)?;
    let version = text(&root, "openapi");
    if !["3.0.", "3.1.", "3.2."]
        .iter()
        .any(|v| version.starts_with(v))
    {
        return Err(format!(
            "Unsupported OpenAPI version: {} (expected 3.0–3.2)",
            if version.is_empty() {
                "missing"
            } else {
                version
            }
        ));
    }
    if !root["info"].is_object()
        || !root["info"]["title"].is_string()
        || !root["info"]["version"].is_string()
    {
        return Err("OpenAPI info.title and info.version are required".into());
    }
    let mut context = Projection {
        root: &root,
        documents,
        warnings: vec![],
        count: 0,
    };
    let mut operations = vec![];
    context.paths(&root["paths"], "", &mut operations, 0)?;
    context.paths(&root["webhooks"], "Webhook ", &mut operations, 0)?;
    Ok(Document {
        operations,
        warnings: context.warnings,
    })
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
fn entries(value: &Value) -> impl Iterator<Item = (&String, &Value)> {
    value.as_object().into_iter().flatten()
}
fn values(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}
fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

struct Projection<'a> {
    root: &'a Value,
    documents: &'a BTreeMap<String, Value>,
    warnings: Vec<String>,
    count: usize,
}
impl Projection<'_> {
    fn resolve(&mut self, input: &Value) -> Value {
        let mut value = input.clone();
        let mut seen = HashSet::new();
        while let Some(reference) = value["$ref"].as_str().map(str::to_owned) {
            if !seen.insert(reference.clone()) {
                self.warn(format!("Recursive reference: {reference}"));
                break;
            }
            let (uri, pointer) =
                reference.split_once('#').unwrap_or((&reference, ""));
            let base = if uri.is_empty() {
                Some(self.root)
            } else {
                self.documents.get(uri)
            };
            let target = base.and_then(|base| {
                if pointer.is_empty() {
                    Some(base)
                } else {
                    base.pointer(pointer)
                }
            });
            let Some(target) = target else {
                self.warn(format!("Unresolved reference: {reference}"));
                break;
            };
            let siblings = value.as_object().cloned().unwrap_or_default();
            value = target.clone();
            if let Some(object) = value.as_object_mut() {
                for (key, v) in siblings {
                    if key != "$ref" {
                        object.insert(key, v);
                    }
                }
            }
        }
        value
    }
    fn warn(&mut self, warning: String) {
        if self.warnings.len() < 100 && !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }
    fn budget(&mut self) -> Result<(), String> {
        self.count += 1;
        if self.count > MAX_ITEMS {
            Err("OpenAPI document exceeds the 10,000 item display limit".into())
        } else {
            Ok(())
        }
    }
    fn schema(
        &mut self,
        name: &str,
        schema: &Value,
        location: &str,
        required: bool,
        depth: usize,
        ancestors: &mut HashSet<String>,
    ) -> Result<Item, String> {
        self.budget()?;
        let reference = schema["$ref"].as_str().map(str::to_owned);
        let cycle = reference.as_ref().is_some_and(|r| ancestors.contains(r));
        let schema = self.resolve(schema);
        let kind = match &schema["type"] {
            Value::String(s) => s.clone(),
            Value::Array(types) => {
                types.iter().map(display).collect::<Vec<_>>().join(" | ")
            }
            _ if schema["properties"].is_object() => "object".into(),
            _ if schema["items"].is_object() => "array".into(),
            _ if schema.is_boolean() => {
                if schema == true { "any" } else { "never" }.into()
            }
            _ => "schema".into(),
        };
        let kind = if text(&schema, "format").is_empty() {
            kind
        } else {
            format!("{kind} ({})", text(&schema, "format"))
        };
        let mut metadata = vec![];
        if !location.is_empty() {
            metadata.push(location.to_owned());
        }
        metadata.push(kind);
        metadata.push(if required { "required" } else { "optional" }.into());
        if schema["nullable"] == true {
            metadata.push("nullable".into());
        }
        if schema["deprecated"] == true {
            metadata.push("deprecated".into());
        }
        for (key, label) in
            [("readOnly", "read only"), ("writeOnly", "write only")]
        {
            if schema[key] == true {
                metadata.push(label.into());
            }
        }
        let mut description = text(&schema, "description").to_owned();
        for (key, label) in [
            ("enum", "Allowed"),
            ("const", "Value"),
            ("default", "Default"),
            ("minimum", "Minimum"),
            ("maximum", "Maximum"),
            ("minLength", "Minimum length"),
            ("maxLength", "Maximum length"),
            ("minItems", "Minimum items"),
            ("maxItems", "Maximum items"),
            ("pattern", "Pattern"),
            ("example", "Example"),
        ] {
            if !schema[key].is_null() {
                description.push_str(&format!(
                    "{}{}: {}.",
                    if description.is_empty() { "" } else { " " },
                    label,
                    display(&schema[key])
                ));
            }
        }
        let mut children = vec![];
        if cycle || depth >= MAX_DEPTH {
            description.push_str(&format!(
                " Reference: {}.",
                reference.as_deref().unwrap_or("depth limit reached")
            ));
        } else {
            if let Some(r) = &reference {
                ancestors.insert(r.clone());
            }
            for (name, child) in entries(&schema["properties"]) {
                let required = values(&schema["required"])
                    .any(|v| v.as_str() == Some(name));
                children.push(self.schema(
                    name,
                    child,
                    "",
                    required,
                    depth + 1,
                    ancestors,
                )?);
            }
            if !schema["items"].is_null() {
                children.push(self.schema(
                    "items",
                    &schema["items"],
                    "",
                    true,
                    depth + 1,
                    ancestors,
                )?);
            }
            for keyword in ["allOf", "oneOf", "anyOf"] {
                for (index, variant) in values(&schema[keyword]).enumerate() {
                    children.push(self.schema(
                        &format!("{keyword} {}", index + 1),
                        variant,
                        "",
                        false,
                        depth + 1,
                        ancestors,
                    )?);
                }
            }
            if schema["additionalProperties"].is_object() {
                children.push(self.schema(
                    "additional properties",
                    &schema["additionalProperties"],
                    "",
                    false,
                    depth + 1,
                    ancestors,
                )?);
            }
            if let Some(r) = &reference {
                ancestors.remove(r);
            }
        }
        if let Some(r) = schema["$ref"].as_str() {
            description.push_str(&format!(" Reference: {r}."));
        }
        Ok(Item {
            name: name.into(),
            metadata: metadata.join(" · "),
            description: description.trim().into(),
            children,
        })
    }
    fn parameter(
        &mut self,
        input: &Value,
        location: Option<&str>,
    ) -> Result<Item, String> {
        let p = self.resolve(input);
        let schema = if p["schema"].is_null() {
            entries(&p["content"])
                .next()
                .map(|(_, v)| &v["schema"])
                .unwrap_or(&Value::Null)
        } else {
            &p["schema"]
        };
        let location = location.unwrap_or_else(|| text(&p, "in"));
        let mut item = self.schema(
            text(&p, "name"),
            schema,
            location,
            p["required"] == true || location == "path",
            0,
            &mut HashSet::new(),
        )?;
        if !text(&p, "description").is_empty() {
            item.description = text(&p, "description").into();
        }
        if !p["example"].is_null() {
            item.children.push(Item {
                name: "Example".into(),
                description: display(&p["example"]),
                ..Default::default()
            });
        }
        Ok(item)
    }
    fn content(
        &mut self,
        content: &Value,
        location: &str,
        required: bool,
    ) -> Result<Vec<Item>, String> {
        let mut items = vec![];
        for (media, value) in entries(content) {
            let mut item = self.schema(
                media,
                &value["schema"],
                location,
                required,
                0,
                &mut HashSet::new(),
            )?;
            if !value["example"].is_null() {
                item.children.push(Item {
                    name: "Example".into(),
                    description: display(&value["example"]),
                    ..Default::default()
                });
            }
            for (name, example) in entries(&value["examples"]) {
                let example = self.resolve(example);
                item.children.push(Item {
                    name: format!("Example: {name}"),
                    description: if !example["value"].is_null() {
                        display(&example["value"])
                    } else {
                        text(&example, "externalValue").into()
                    },
                    ..Default::default()
                });
            }
            items.push(item);
        }
        Ok(items)
    }
    fn security(&mut self, operation: &Value) -> Vec<Item> {
        let mut result = vec![];
        let security = operation
            .get("security")
            .unwrap_or(&self.root["security"])
            .clone();
        let optional = values(&security)
            .any(|r| r.as_object().is_some_and(|r| r.is_empty()));
        let alternatives = values(&security).count() > 1;
        for (index, requirement) in values(&security).enumerate() {
            for (name, scopes) in entries(requirement) {
                let scheme = self.resolve(
                    &self.root["components"]["securitySchemes"][name].clone(),
                );
                let kind = text(&scheme, "type");
                let field = if kind == "apiKey" {
                    text(&scheme, "name")
                } else {
                    "Authorization"
                };
                let location = if kind == "apiKey" {
                    text(&scheme, "in")
                } else if kind == "mutualTLS" {
                    "TLS"
                } else {
                    "header"
                };
                let auth = match kind {
                    "http"
                        if text(&scheme, "scheme")
                            .eq_ignore_ascii_case("bearer") =>
                    {
                        "Bearer token"
                    }
                    "http"
                        if text(&scheme, "scheme")
                            .eq_ignore_ascii_case("basic") =>
                    {
                        "Basic authentication"
                    }
                    "oauth2" | "openIdConnect" => "OAuth access token",
                    "apiKey" => "API key",
                    "mutualTLS" => "Client certificate",
                    _ => name,
                };
                let mut description =
                    text(&scheme, "description").trim().to_owned();
                if description.is_empty() {
                    description = auth.to_owned();
                }
                let scopes: Vec<_> = values(scopes).map(display).collect();
                if !scopes.is_empty() {
                    description
                        .push_str(&format!("; scopes: {}", scopes.join(", ")));
                }
                result.push(Item {
                    name: if kind == "mutualTLS" {
                        name.clone()
                    } else {
                        field.into()
                    },
                    metadata: format!(
                        "{location} · {}{}",
                        if optional { "optional" } else { "required" },
                        if alternatives {
                            format!(" · security alternative {}", index + 1)
                        } else {
                            String::new()
                        }
                    ),
                    description,
                    children: vec![],
                });
            }
        }
        result
    }
    fn paths(
        &mut self,
        paths: &Value,
        prefix: &str,
        output: &mut Vec<Operation>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 8 {
            return Err("OpenAPI callback nesting limit reached".into());
        }
        for (path, input) in entries(paths) {
            if path.starts_with("x-") {
                continue;
            }
            let path_item = self.resolve(input);
            for (method, operation) in entries(&path_item) {
                if ![
                    "get", "post", "put", "patch", "delete", "head", "options",
                    "trace", "query",
                ]
                .contains(&method.as_str())
                {
                    continue;
                }
                self.operation(
                    method, path, prefix, operation, &path_item, output, depth,
                )?;
            }
            for (method, operation) in
                entries(&path_item["additionalOperations"])
            {
                self.operation(
                    method, path, prefix, operation, &path_item, output, depth,
                )?;
            }
        }
        Ok(())
    }
    fn operation(
        &mut self,
        method: &str,
        path: &str,
        prefix: &str,
        op: &Value,
        path_item: &Value,
        output: &mut Vec<Operation>,
        depth: usize,
    ) -> Result<(), String> {
        self.budget()?;
        if !op.is_object() {
            return Err(format!("Invalid operation: {method} {path}"));
        }
        let mut request = self.security(op);
        let mut parameters: Vec<Value> = values(&path_item["parameters"])
            .map(|v| self.resolve(v))
            .collect();
        for value in values(&op["parameters"]) {
            let value = self.resolve(value);
            parameters.retain(|p| {
                !(p["name"] == value["name"] && p["in"] == value["in"])
            });
            parameters.push(value);
        }
        for p in parameters {
            request.push(self.parameter(&p, None)?);
        }
        if !op["requestBody"].is_null() {
            let body = self.resolve(&op["requestBody"]);
            let media = entries(&body["content"])
                .map(|(m, _)| m.as_str())
                .collect::<Vec<_>>()
                .join(" | ");
            request.push(Item {
                name: "Content-Type".into(),
                metadata: format!(
                    "header · {}",
                    if body["required"] == true {
                        "required"
                    } else {
                        "when body is present"
                    }
                ),
                description: media,
                ..Default::default()
            });
            for mut item in self.content(
                &body["content"],
                "body",
                body["required"] == true,
            )? {
                if !text(&body, "description").is_empty() {
                    item.description = format!(
                        "{} {}",
                        text(&body, "description"),
                        item.description
                    )
                    .trim()
                    .into();
                }
                if body["content"].as_object().is_some_and(|c| c.len() == 1)
                    && item.metadata.contains("object")
                    && item.description.is_empty()
                    && !item.children.is_empty()
                {
                    for mut field in item.children {
                        field.metadata = format!("body · {}", field.metadata);
                        request.push(field);
                    }
                } else {
                    request.push(item);
                }
            }
        }
        let mut responses = vec![];
        if !op["responses"].is_object() {
            return Err(format!("Missing responses for {method} {path}"));
        }
        for (status, input) in entries(&op["responses"]) {
            if status.starts_with("x-") {
                continue;
            }
            let response = self.resolve(input);
            let mut children = vec![];
            for (name, header) in entries(&response["headers"]) {
                let mut h = self.resolve(header);
                if let Some(object) = h.as_object_mut() {
                    object.insert("name".into(), name.clone().into());
                }
                children.push(self.parameter(&h, Some("header"))?);
            }
            let content = self.content(&response["content"], "", true)?;
            let mut metadata = if content.is_empty() {
                "No response body".into()
            } else {
                String::new()
            };
            for item in content {
                if response["content"]
                    .as_object()
                    .is_some_and(|c| c.len() == 1)
                    && item.metadata.contains("object")
                    && item.description.is_empty()
                    && !item.children.is_empty()
                {
                    metadata = item.name;
                    children.extend(item.children);
                } else {
                    children.push(item);
                }
            }
            responses.push(Item {
                name: status.clone(),
                metadata,
                description: text(&response, "description").into(),
                children,
            });
        }
        let comment = [text(op, "summary"), text(op, "description")]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        output.push(Operation {
            method: method.to_uppercase(),
            path: format!("{prefix}{path}"),
            request,
            responses,
            comment: if op["deprecated"] == true {
                format!("Deprecated. {comment}")
            } else {
                comment
            },
        });
        for (name, callback) in entries(&op["callbacks"]) {
            let callback = self.resolve(callback);
            self.paths(
                &callback,
                &format!("Callback {name}: "),
                output,
                depth + 1,
            )?;
        }
        Ok(())
    }
}
