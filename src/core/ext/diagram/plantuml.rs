use anyhow::{Context, Result};

use plantuml_encoding::encode_plantuml_deflate;

use crate::core::export::html::bundler::{data_uri_from_bytes, fetch_remote_resource};
use crate::core::util::html_escape;

fn plantuml_skinparams() -> &'static str {
    "skinparam backgroundColor transparent\nskinparam sequenceParticipantBackgroundColor white\nskinparam sequenceParticipantBorderColor #94a3b8\nskinparam actorBackgroundColor white\nskinparam actorBorderColor #94a3b8\nskinparam shadowing false"
}

fn normalize_plantuml_source(code: &str) -> String {
    let trimmed = code.trim();
    if trimmed.contains("@start") && trimmed.contains("@end") {
        if trimmed.contains("skinparam") {
            trimmed.to_string()
        } else if let Some(index) = trimmed.find('\n') {
            format!(
                "{}\n{}{}",
                &trimmed[..index],
                plantuml_skinparams(),
                &trimmed[index..]
            )
        } else {
            trimmed.to_string()
        }
    } else {
        format!("@startuml\n{}\n{trimmed}\n@enduml", plantuml_skinparams())
    }
}

pub(crate) fn render_plantuml(code: &str) -> Result<String> {
    let source = normalize_plantuml_source(code);
    let encoded = encode_plantuml_deflate(&source)
        .map_err(|err| anyhow::anyhow!("Failed to encode PlantUML diagram: {:?}", err))?;
    let url = format!("https://www.plantuml.com/plantuml/svg/{encoded}");
    let (bytes, mime) = fetch_remote_resource(&url)?;
    if mime.eq_ignore_ascii_case("image/svg+xml") || bytes.starts_with(b"<svg") {
        let svg = String::from_utf8(bytes).context("PlantUML server returned non-UTF-8 SVG")?;
        Ok(format!(
            "<div class=\"plantuml-display\"><div class=\"plantuml-canvas\">{svg}</div></div>\n"
        ))
    } else {
        let data_uri = data_uri_from_bytes(&mime, &bytes);
        Ok(format!(
            "<div class=\"plantuml-display\"><img class=\"plantuml-image\" src=\"{}\" alt=\"PlantUML diagram\" loading=\"lazy\"></div>\n",
            html_escape(&data_uri)
        ))
    }
}

pub(crate) fn plantuml_error_html(code: &str) -> String {
    format!(
        "<div class=\"plantuml-display plantuml-error\"><strong>PlantUML render failed</strong><pre><code>{}</code></pre></div>\n",
        html_escape(code)
    )
}
