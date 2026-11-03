use pagemd_openapi::{closed_fence, parse, render_html, source_attribute};

const SPEC: &str = r##"
openapi: 3.1.0
info: {title: Example, version: '1'}
components:
  securitySchemes:
    token: {type: http, scheme: bearer}
  schemas:
    Document:
      type: object
      required: [id]
      properties:
        id: {type: string, format: uuid, description: Unique ID}
        next: {$ref: '#/components/schemas/Document'}
security: [{token: []}]
paths:
  /documents/{id}:
    parameters:
      - {name: id, in: path, required: true, schema: {type: string}}
      - {name: limit, in: query, schema: {type: integer, default: 10}}
    patch:
      summary: Update a document
      description: '<script>bad()</script>'
      parameters:
        - {name: limit, in: query, schema: {type: integer, default: 20}}
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                title: {type: string, maxLength: 120}
      responses:
        '200':
          description: Document updated
          headers:
            X-Revision: {schema: {type: integer}, description: Revision number}
          content:
            application/json:
              schema: {$ref: '#/components/schemas/Document'}
        '404': {description: Not found}
    get:
      security: []
      responses: {'204': {description: No content}}
"##;

#[test]
fn projects_inherited_parameters_security_body_and_response_fields() {
    let document = parse(SPEC).unwrap();
    let patch = &document.operations[0];
    assert_eq!(patch.method, "PATCH");
    assert_eq!(
        patch.request.iter().filter(|p| p.name == "limit").count(),
        1
    );
    assert!(patch
        .request
        .iter()
        .find(|p| p.name == "limit")
        .unwrap()
        .description
        .contains("20"));
    assert!(
        patch
            .request
            .iter()
            .any(|p| p.name == "Authorization"
                && p.description.contains("Bearer"))
    );
    assert!(patch.request.iter().any(|p| p.name == "Content-Type"));
    let response = &patch.responses[0];
    assert!(response.children.iter().any(|p| p.name == "X-Revision"));
    assert_eq!(response.metadata, "application/json");
    assert!(response
        .children
        .iter()
        .any(|p| p.name == "id" && p.metadata.contains("required")));
    assert!(response
        .children
        .iter()
        .find(|p| p.name == "next")
        .unwrap()
        .description
        .contains("Reference:"));
    assert!(!document.operations[1]
        .request
        .iter()
        .any(|p| p.name == "Authorization"));
}

#[test]
fn json_and_yaml_share_the_same_model_and_html_is_inert() {
    let value: serde_json::Value = serde_yaml::from_str(SPEC).unwrap();
    let document = parse(SPEC).unwrap();
    assert_eq!(document, parse(&value.to_string()).unwrap());
    let html = render_html(&document);
    assert!(html.contains("&lt;script&gt;bad()&lt;/script&gt;"));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<button"));
    assert!(!html.contains("<input"));
    assert!(
        html.find("Request</h4>").unwrap()
            < html.find("Response</h4>").unwrap()
    );
    assert!(
        html.find("Response</h4>").unwrap()
            < html.find("Comment</h4>").unwrap()
    );
}

#[test]
fn fence_attributes_and_incomplete_streams_are_unambiguous() {
    assert_eq!(
        source_attribute("src=\"./api.yaml\"", "")
            .unwrap()
            .as_deref(),
        Some("./api.yaml")
    );
    assert!(source_attribute("src=\"./api.yaml\"", "inline").is_err());
    assert!(source_attribute("src=\"x\" src=\"y\"", "").is_err());
    assert!(source_attribute("src=x", "").is_err());
    assert!(closed_fence("``` openapi\n{}\n```"));
    assert!(closed_fence("~~~~openapi\n```\n~~~~"));
    assert!(!closed_fence("```openapi\n{}"));
    assert!(!closed_fence("````openapi\n{}\n```"));
}

#[test]
fn validates_versions_and_handles_empty_documents() {
    for version in ["3.0.4", "3.1.2", "3.2.1"] {
        assert!(parse(&format!("openapi: {version}\ninfo: {{title: Empty, version: '1'}}\npaths: {{}}")).unwrap().operations.is_empty());
    }
    assert!(parse("swagger: '2.0'").unwrap_err().contains("Unsupported"));
    assert!(parse("openapi: 3.1.0").is_err());
    assert!(parse("openapi: [").is_err());
}

#[cfg(feature = "io")]
#[test]
fn source_files_resolve_relative_external_refs_and_keep_recursive_refs_bounded()
{
    let root = std::env::temp_dir()
        .join(format!("pagemd-openapi-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = "openapi: 3.1.0\ninfo: {title: External, version: '1'}\npaths:\n  /example:\n    get:\n      responses:\n        '200':\n          description: Loaded\n          content:\n            application/json:\n              schema: {$ref: './schema.yaml#/$defs/Record'}\n";
    std::fs::write(root.join("api.yaml"), source).unwrap();
    std::fs::write(root.join("schema.yaml"), "$defs:\n  Record:\n    type: object\n    properties:\n      external_id: {type: string}\n      next: {$ref: '#/$defs/Record'}\n").unwrap();
    let document =
        pagemd_openapi::load_blocking("", "src=\"./api.yaml\"", &root).unwrap();
    let model = serde_json::to_string(&document).unwrap();
    assert!(model.contains("external_id"), "{model}");
    assert!(model.contains("Reference:"), "{model}");
    assert!(document.warnings.is_empty(), "{:?}", document.warnings);
    assert!(pagemd_openapi::load_blocking(
        "",
        "src=\"../outside.yaml\"",
        &root
    )
    .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
