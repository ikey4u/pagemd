//! Writing in Markdown end to end, as applications render it: the document's
//! own HTML limited to the shared set, resources linked rather than read,
//! and the written conventions ([TOC], heading ids, image sizes, table
//! captions, definition lists).
use pagemd_core::{
    render_to_html, HtmlExportOptions, RawHtml, RenderOptions, Resources,
};

fn render(source: &str, raw_html: RawHtml, resources: Resources) -> String {
    let options = RenderOptions {
        html: HtmlExportOptions::embedded(),
        raw_html,
        resources,
        ..Default::default()
    };
    let html = render_to_html(source, &options).expect("renders");
    // The document's content, without the page's own styles and scripts.
    let start = html.find("<body").unwrap_or(0);
    html[start..].to_string()
}

#[test]
fn own_html_is_limited_to_the_shared_set_when_safe() {
    let source = r#"Before

<img src="https://example.com/cat.png" width="240" style="border-radius:8px" onerror="alert(1)">

<details><summary>More</summary>

Hidden **markdown**.

</details>

<script>alert(2)</script>

Press <kbd>Ctrl</kbd> and <a href="javascript:alert(3)">this</a>.
"#;
    let html = render(source, RawHtml::Safe, Resources::Link);
    assert!(html.contains(r#"<img src="https://example.com/cat.png" width="240" style="border-radius:8px">"#), "{html}");
    assert!(html.contains("<details><summary>More</summary>"), "{html}");
    assert!(html.contains("<strong>markdown</strong>"), "{html}");
    assert!(html.contains("</details>"), "{html}");
    assert!(html.contains("<kbd>Ctrl</kbd>"), "{html}");
    assert!(
        !html.contains("alert(2)")
            && !html.contains("onerror")
            && !html.contains("javascript:"),
        "{html}"
    );
    // Kept as written when trusted.
    let kept = render(source, RawHtml::Keep, Resources::Link);
    assert!(kept.contains("<script>alert(2)</script>"));
}

#[test]
fn linked_resources_are_neither_read_nor_fetched() {
    let html = render(
        "![a](/etc/hostname)\n\n![b](https://example.com/b.png)",
        RawHtml::Safe,
        Resources::Link,
    );
    assert!(html.contains(r#"src="/etc/hostname""#), "{html}");
    assert!(
        html.contains(r#"src="https://example.com/b.png""#),
        "{html}"
    );
    assert!(!html.contains("data:"), "nothing embedded: {html}");
    let html =
        render("![x](javascript:alert(1))", RawHtml::Safe, Resources::Link);
    assert!(!html.contains("javascript:"), "{html}");
}

#[test]
fn written_conventions() {
    let source = "# Guide\n\n[TOC]\n\n## Start {#begin .lead}\n\n### Install\n\n## Use\n\n\
        ![Chart](chart.png){width=320 .wide}\n\n\
        | A | B |\n|---|---|\n| 1 | 2 |\n\nTable: Results\n\n\
        Term\n: Its definition\n";
    let html = render(source, RawHtml::Safe, Resources::Link);
    assert!(
        html.contains(r#"<h2 id="begin" class="lead">Start</h2>"#),
        "{html}"
    );
    assert!(html.contains(r##"<nav class="toc""##), "{html}");
    assert!(html.contains(r##"<a href="#begin">Start</a>"##), "{html}");
    assert!(
        html.contains(r##"<a href="#install">Install</a>"##),
        "{html}"
    );
    assert!(
        !html.contains(r##"<a href="#guide">"##),
        "the title is not in the contents: {html}"
    );
    assert!(
        html.contains(
            r#"<img src="chart.png" alt="Chart" width="320" class="wide">"#
        ),
        "{html}"
    );
    assert!(html.contains("<caption>Results</caption>"), "{html}");
    assert!(
        html.contains("<dl>")
            && html.contains("<dt>Term</dt>")
            && html.contains("<dd>Its definition</dd>"),
        "{html}"
    );
}
