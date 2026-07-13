use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::app::cli::{parse_icon_arg, CliArgs};
use crate::core::export::html::build_html;
use crate::core::export::html::favicon::{
    contrast_ratio, default_icon_label_from_path, icon_background_rgb, icon_colors,
    relative_luminance,
};
use crate::core::export::html::page::build_html_with_nav;
use crate::core::md::render_markdown;
use crate::core::model::{HeadingOutline, RenderedSection};
use crate::core::resolve_inputs;

fn render_html_at(source: &str, base_dir: &Path) -> String {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    render_markdown(source, base_dir, 16.0, "", &ss, &ts)
        .unwrap()
        .html
}

fn render_html(source: &str) -> String {
    render_html_at(source, Path::new("."))
}

fn temp_test_dir(name: &str) -> PathBuf {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("pagemd-{name}-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn math_inline_count(html: &str) -> usize {
    html.matches("class=\"math-inline\"").count()
}

fn mermaid_count(html: &str) -> usize {
    html.matches("class=\"mermaid-display\"").count()
}

fn plantuml_count(html: &str) -> usize {
    html.matches("plantuml-display").count()
}

fn typst_count(html: &str) -> usize {
    html.matches("class=\"typst-display\"").count()
}

fn diagram_html_count(html: &str) -> usize {
    html.matches("class=\"diagram-html-display\"").count()
}

fn callout_count(html: &str) -> usize {
    html.matches("class=\"callout callout-").count()
}

fn test_args(inputs: Vec<PathBuf>, directories: Vec<PathBuf>) -> CliArgs {
    CliArgs {
        inputs,
        directories,
        excludes: Vec::new(),
        output: None,
        title: None,
        icon: None,
        math_font_size: 16.0,
        katex_fonts: None,
    }
}
#[test]
fn parse_icon_arg_validates_and_uppercases() {
    assert_eq!(parse_icon_arg("ab").unwrap(), "AB");
    assert_eq!(parse_icon_arg("x9").unwrap(), "X9");
    assert!(parse_icon_arg("a").is_err());
    assert!(parse_icon_arg("abc").is_err());
    assert!(parse_icon_arg("a!").is_err());
    assert!(parse_icon_arg("中文").is_err());
}

#[test]
fn default_icon_label_from_path_rules() {
    assert_eq!(default_icon_label_from_path(Path::new("readme.md")), "RE");
    assert_eq!(default_icon_label_from_path(Path::new("a.md")), "AA");
    assert_eq!(default_icon_label_from_path(Path::new("my-doc.md")), "MY");
    assert_eq!(default_icon_label_from_path(Path::new("笔记.md")), "PG");
}

#[test]
fn build_html_embeds_two_char_favicon() {
    let html = build_html(
        "Title",
        &[RenderedSection {
            title: String::new(),
            html: "<p>x</p>".to_string(),
            outline: Vec::new(),
        }],
        "ab",
    );
    assert!(html.contains("rel=\"icon\""));
    assert!(html.contains("data:image/svg+xml,"));
    assert!(html.contains("AB</text>") || html.contains("AB%3C/text"));
    assert!(html.contains("rx='7'") || html.contains("rx=%277%27"));
}

#[test]
fn directory_inputs_collect_markdown_and_dedup_files() {
    let dir = temp_test_dir("dir-inputs");
    let nested = dir.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let first = dir.join("a.md");
    let second = nested.join("b.markdown");
    let ignored = nested.join("c.txt");
    std::fs::write(&first, "# A").unwrap();
    std::fs::write(&second, "# B").unwrap();
    std::fs::write(&ignored, "# C").unwrap();

    let args = test_args(vec![first.clone()], vec![dir.clone(), dir.clone()]);
    let resolved = resolve_inputs(&(&args).into()).unwrap();

    assert_eq!(resolved.files.len(), 2);
    assert!(resolved.files.iter().any(|path| path.ends_with("a.md")));
    assert!(resolved
        .files
        .iter()
        .any(|path| path.ends_with("b.markdown")));
    assert!(!resolved.files.iter().any(|path| path.ends_with("c.txt")));
    assert_eq!(resolved.directories.len(), 1);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn exclude_patterns_skip_directories_and_files() {
    let dir = temp_test_dir("exclude-inputs");
    let skipped_dir = dir.join("drafts");
    let nested = dir.join("guide");
    std::fs::create_dir_all(skipped_dir.join("nested")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(dir.join("keep.md"), "# Keep").unwrap();
    std::fs::write(skipped_dir.join("skip.md"), "# Skip").unwrap();
    std::fs::write(nested.join("topic.md"), "# Topic").unwrap();
    std::fs::write(dir.join("notes.tmp.md"), "# Tmp").unwrap();

    let mut args = test_args(Vec::new(), vec![dir.clone()]);
    args.excludes = vec!["drafts".to_string(), "*.tmp.md".to_string()];
    let resolved = resolve_inputs(&(&args).into()).unwrap();

    assert_eq!(resolved.files.len(), 2);
    assert!(resolved.files.iter().any(|path| path.ends_with("keep.md")));
    assert!(resolved
        .files
        .iter()
        .any(|path| path.ends_with("guide/topic.md") || path.ends_with("guide\\topic.md")));
    assert!(!resolved.files.iter().any(|path| path.ends_with("skip.md")));
    assert!(!resolved
        .files
        .iter()
        .any(|path| path.ends_with("notes.tmp.md")));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn exclude_skips_explicit_input_files_in_excluded_directories() {
    let dir = temp_test_dir("exclude-input-file");
    let guide = dir.join("guide");
    std::fs::create_dir_all(&guide).unwrap();
    let guide_file = guide.join("topic.md");
    std::fs::write(&guide_file, "# Topic").unwrap();

    let mut args = test_args(vec![guide_file.clone()], Vec::new());
    args.excludes = vec!["guide".to_string()];
    let resolved = resolve_inputs(&(&args).into());

    assert!(resolved.is_err() || resolved.unwrap().files.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tree_nav_uses_directory_structure_with_tmp_paths() {
    let dir = temp_test_dir("tree-nav");
    let guide = dir.join("guide");
    std::fs::create_dir_all(&guide).unwrap();
    std::fs::write(dir.join("keep.md"), "# Keep").unwrap();
    std::fs::write(guide.join("topic.md"), "# Topic").unwrap();

    let args = test_args(Vec::new(), vec![dir.clone()]);
    let resolved = resolve_inputs(&(&args).into()).unwrap();
    let html_opts = crate::core::HtmlExportOptions {
        embed_workspace_script: true,
    };
    let convert_opts = (&args).into();
    let resources = crate::core::prepare_resources(&convert_opts).unwrap();
    let output = crate::core::export_with_resources(
        &convert_opts,
        &html_opts,
        &resources,
        resolved.files.first().map(|path| path.as_path()),
    )
    .unwrap();

    assert!(output.html.contains("data-nav-folder=\"guide\""));
    assert!(output.html.contains("class=\"doc-nav-tree\""));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn preview_html_omits_embedded_workspace_script() {
    let html = build_html_with_nav(
        "Title",
        &[RenderedSection {
            title: "Doc".to_string(),
            html: "<h1 id=\"intro\">Intro</h1>".to_string(),
            outline: vec![HeadingOutline {
                level: 1,
                id: "intro".to_string(),
                text: "Intro".to_string(),
            }],
        }],
        "PG",
        None,
        None,
        false,
    );
    assert!(html.contains("data-doc-workspace"));
    assert!(!html.contains("data-pagemd-workspace"));
    let preview = crate::app::preview::wrap_for_preview(html);
    assert!(preview.contains("data-pagemd-workspace"));
    assert!(preview.contains("data-pagemd-live-preview"));
}

#[test]
fn export_html_restores_workspace_script_for_preview_render() {
    let html = build_html_with_nav(
        "Title",
        &[RenderedSection {
            title: "Doc".to_string(),
            html: "<h1 id=\"intro\">Intro</h1>".to_string(),
            outline: vec![HeadingOutline {
                level: 1,
                id: "intro".to_string(),
                text: "Intro".to_string(),
            }],
        }],
        "PG",
        None,
        None,
        false,
    );
    let exported = crate::app::preview::ensure_export_html(html);
    assert!(exported.contains("data-pagemd-workspace"));
    assert!(!exported.contains("data-pagemd-live-preview"));
}

#[tokio::test(flavor = "multi_thread")]
async fn hosted_preview_starts_inside_tokio_runtime() {
    use crate::app::preview::error::{build_preview_error_html, preview_html_opts};
    use crate::app::preview::{HostedPreview, HostedPreviewOptions, RenderRequest, RenderResult};
    use crate::core::{export_with_resources, prepare_resources, ConvertOptions, OutputFormat};

    let dir = temp_test_dir("hosted-preview");
    let session_path = dir.join("session.md");
    std::fs::write(&session_path, "# Hello\n\nPreview inside runtime.\n").unwrap();

    let convert_opts = ConvertOptions {
        inputs: vec![session_path.clone()],
        directories: Vec::new(),
        excludes: Vec::new(),
        title: Some("Hosted preview".to_string()),
        icon: None,
        math_font_size: 1.0,
        katex_fonts: None,
        output_format: OutputFormat::Html,
    };
    let resources = prepare_resources(&convert_opts).unwrap();
    let html_opts = preview_html_opts();

    let hosted = HostedPreview::start(
        HostedPreviewOptions {
            host: "127.0.0.1".to_string(),
            port: 0,
            inputs: vec![session_path.clone()],
            watch_paths: vec![session_path.clone()],
            export_path: None,
        },
        move |_request: RenderRequest| match export_with_resources(
            &convert_opts,
            &html_opts,
            &resources,
            Some(session_path.as_path()),
        ) {
            Ok(document) => RenderResult::Ok {
                html: document.html,
                extra_watch_paths: Vec::new(),
            },
            Err(err) => RenderResult::Err {
                html: build_preview_error_html(&err),
            },
        },
    )
    .await
    .unwrap();

    assert!(hosted.url().starts_with("http://127.0.0.1:"));
    hosted.shutdown().await;
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn single_file_html_includes_outline_workspace() {
    let html = build_html(
        "Title",
        &[RenderedSection {
            title: "Doc".to_string(),
            html: "<h1 id=\"intro\">Intro</h1><h2 id=\"details\">Details</h2>".to_string(),
            outline: vec![
                HeadingOutline {
                    level: 1,
                    id: "intro".to_string(),
                    text: "Intro".to_string(),
                },
                HeadingOutline {
                    level: 2,
                    id: "details".to_string(),
                    text: "Details".to_string(),
                },
            ],
        }],
        "PG",
    );

    assert!(html.contains("doc-workspace-single"));
    assert!(html.contains("data-doc-workspace"));
    assert!(html.contains("doc-topbar"));
    assert!(html.contains("data-doc-title"));
    assert!(html.contains("data-theme-toggle"));
    assert!(html.contains("doc-theme-icon-moon"));
    assert!(html.contains("data-pagemd-workspace"));
    assert!(html.contains("id=\"doc-1\" data-doc-panel"));
    assert!(html.contains("data-outline-toggle"));
    assert!(html.contains("aria-label=\"Outline\""));
    assert!(html.contains("data-heading-target=\"intro\""));
    assert!(html.contains("data-heading-target=\"details\""));
    assert!(!html.contains("class=\"doc-sidebar doc-pane\""));
    assert!(!html.contains("aria-label=\"Files\""));
}

#[test]
fn single_file_without_headings_uses_plain_container() {
    let html = build_html(
        "Title",
        &[RenderedSection {
            title: String::new(),
            html: "<p>No headings here.</p>".to_string(),
            outline: Vec::new(),
        }],
        "PG",
    );

    assert!(!html.contains("data-doc-workspace"));
    assert!(!html.contains("data-pagemd-workspace"));
    assert!(html.contains("<p>No headings here.</p>"));
}

#[test]
fn multi_file_html_includes_standalone_sidebar() {
    let html = build_html_with_nav(
        "Title",
        &[
            RenderedSection {
                title: "A".to_string(),
                html: "<h1>A</h1>".to_string(),
                outline: vec![HeadingOutline {
                    level: 1,
                    id: "a".to_string(),
                    text: "A".to_string(),
                }],
            },
            RenderedSection {
                title: "B".to_string(),
                html: "<h1>B</h1>".to_string(),
                outline: vec![HeadingOutline {
                    level: 1,
                    id: "b".to_string(),
                    text: "B".to_string(),
                }],
            },
        ],
        "PG",
        Some(&["a.md".to_string(), "b.md".to_string()]),
        None,
        true,
    );

    assert!(html.contains("data-doc-workspace"));
    assert!(html.contains("data-pagemd-workspace"));
    assert!(html.contains("class=\"doc-sidebar doc-pane\""));
    assert!(html.contains("data-nav-toggle"));
    assert!(html.contains("doc-topbar"));
    assert!(html.contains("data-doc-title"));
    assert!(html.contains("data-theme-toggle"));
    assert!(html.contains("doc-theme-icon-moon"));
    assert!(html.contains("data-doc-target=\"doc-1\""));
    assert!(html.contains("class=\"doc-nav-label\""));
    assert!(html.contains("class=\"doc-nav-copy\""));
    assert!(html.contains("data-copy-label=\"a.md\""));
    assert!(html.contains("navigator.clipboard"));
    assert!(html.contains("fallbackCopyText"));
    assert!(html.contains("activeDoc"));
    assert!(html.contains("data-doc-panel"));
    assert!(html.contains("class=\"doc-outline"));
    assert!(html.contains("data-heading-target=\"a\""));
    assert!(html.contains("PageMDActivateDocumentFromHash"));
}

#[test]
fn multi_file_tree_sidebar_renders_folders() {
    let html = build_html_with_nav(
        "Title",
        &[
            RenderedSection {
                title: "Root".to_string(),
                html: "<h1>Root</h1>".to_string(),
                outline: Vec::new(),
            },
            RenderedSection {
                title: "Guide".to_string(),
                html: "<h1>Guide</h1>".to_string(),
                outline: Vec::new(),
            },
        ],
        "PG",
        Some(&["readme.md".to_string(), "guide/start.md".to_string()]),
        Some(&[
            PathBuf::from("/project/docs/readme.md"),
            PathBuf::from("/project/docs/guide/start.md"),
        ]),
        true,
    );

    assert!(html.contains("class=\"doc-nav-tree\""));
    assert!(html.contains("data-nav-folder=\"guide\""));
    assert!(html.contains("class=\"doc-nav-folder-toggle\""));
    assert!(html.contains("data-nav-toggle"));
    assert!(html.contains("doc-topbar"));
    assert!(html.contains("folder:"));
    assert!(html.contains("restoreFolderStates"));
    assert!(html.contains("setNavVisible"));
    assert!(html.contains("navVisible"));
    assert!(html.contains("updateDocTitle"));
    assert!(html.contains("setTheme"));
    assert!(html.contains("data-theme-toggle"));
}

#[test]
fn outline_uses_markdown_plain_text_not_html_roundtrip() {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let section = render_markdown(
        "## 3.4 send -> RunHandle & `Async`\n",
        Path::new("."),
        16.0,
        "",
        &ss,
        &ts,
    )
    .unwrap();

    assert_eq!(section.outline.len(), 1);
    assert_eq!(section.outline[0].text, "3.4 send -> RunHandle & Async");
    assert!(!section.outline[0].id.contains("gt"));

    let html = build_html("doc", &[section], "PG");
    // Escaped exactly once when embedded into outline HTML.
    assert!(html.contains(">3.4 send -&gt; RunHandle &amp; Async</a>"));
    assert!(!html.contains("-&amp;gt;"));
    assert!(!html.contains("&amp;amp;"));
}

#[test]
fn duplicate_heading_ids_are_unique_for_outline_links() {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let section = render_markdown(
        "# Repeat\n\n## Repeat\n\n# Repeat\n",
        Path::new("."),
        16.0,
        "",
        &ss,
        &ts,
    )
    .unwrap();

    assert!(section.html.contains("id=\"repeat\""));
    assert!(section.html.contains("id=\"repeat-2\""));
    assert!(section.html.contains("id=\"repeat-3\""));
    assert_eq!(
        section
            .outline
            .iter()
            .map(|heading| heading.id.as_str())
            .collect::<Vec<_>>(),
        vec!["repeat", "repeat-2", "repeat-3"]
    );
}

#[test]
fn icon_colors_are_deterministic_and_readable() {
    let bg1 = icon_background_rgb("BX");
    let bg2 = icon_background_rgb("BX");
    assert_eq!(bg1, bg2);
    assert_ne!(icon_background_rgb("AB"), icon_background_rgb("BX"));

    for label in ["AB", "BX", "PG", "Z9", "00", "XY"] {
        let (bg, fg) = icon_colors(label);
        let bg_l = relative_luminance(bg);
        let fg_l = relative_luminance(fg);
        let (hi, lo) = if bg_l > fg_l {
            (bg_l, fg_l)
        } else {
            (fg_l, bg_l)
        };
        let ratio = contrast_ratio(hi, lo);
        assert!(
            ratio >= 4.5,
            "label {label} contrast {ratio:.2} bg={bg:?} fg={fg:?}"
        );
    }
}

#[test]
fn currency_sentence_with_cjk_text_stays_plain() {
    let html = render_html("（$21 发行价，融资 $2.08 亿）");
    assert!(html.contains("<p>（$21 发行价，融资 $2.08 亿）</p>"));
    assert_eq!(math_inline_count(&html), 0);
}

#[test]
fn blockquote_soft_breaks_become_line_breaks() {
    let html = render_html("> **alpha**: one\n> **beta**: two\n");
    assert!(html.contains("<blockquote>"));
    assert!(
        html.contains("<strong>alpha</strong>: one<br>\n<strong>beta</strong>: two"),
        "blockquote consecutive lines should keep a visible break: {html}"
    );

    let paragraph = render_html("hello\nworld");
    assert!(
        !paragraph.contains("<br>"),
        "normal paragraph soft breaks should not become <br>: {paragraph}"
    );
}

#[test]
fn currency_and_bold_currency_do_not_merge_into_math() {
    let html = render_html("但合并营业利润因 $738M 减值几乎归零；OCF **$710M**");
    assert!(html.contains("$738M"));
    assert!(html.contains("<strong>$710M</strong>"));
    assert_eq!(math_inline_count(&html), 0);
}

#[test]
fn currency_range_stays_plain() {
    let html = render_html("品牌溢价（售价 $40–$60）");
    assert!(html.contains("<p>品牌溢价（售价 $40–$60）</p>"));
    assert_eq!(math_inline_count(&html), 0);
}

#[test]
fn eps_sequence_with_arrows_stays_plain() {
    let html = render_html("（EPS：$8.71→$12.79→$15.88）");
    assert!(html.contains("<p>（EPS：$8.71→$12.79→$15.88）</p>"));
    assert_eq!(math_inline_count(&html), 0);
}

#[test]
fn actual_inline_and_display_math_still_render() {
    let html = render_html("真公式 $x+y$\n\n**$x+y$**\n\n$$x+y$$");
    assert_eq!(math_inline_count(&html), 2);
    assert_eq!(html.matches("class=\"math-display\"").count(), 1);
    assert!(html.contains("<strong><span class=\"math-inline\">"));
}

#[test]
fn display_math_with_text_commands_renders() {
    let html = render_html("$$\n\\text{score}(w_1 \\cdots w_n) = \\sum_i \\bigl[\\log P(w_i) + \\lambda \\cdot \\log P(w_i \\mid w_{i-1}) + \\text{WORD\\_PENALTY}\\bigr]\n$$");
    assert_eq!(html.matches("class=\"math-display\"").count(), 1);
}

#[test]
fn display_math_renders_inside_chinese_section() {
    let html = render_html(
        r#"#### 示例小节

这是一段包含中文上下文的说明：

$$
\text{label}(x_1 \cdots x_n) = \sum_i \bigl[x_i + \text{TOKEN\_VALUE}\bigr]
$$

**说明**：这里包含后续加粗文本和 `inline-code`。

**下一步**：继续普通 Markdown 内容。
"#,
    );
    assert_eq!(html.matches("class=\"math-display\"").count(), 1);
    assert!(html.contains("<h4"));
    assert!(html.contains("<strong>说明</strong>"));
    assert!(html.contains("<code>inline-code</code>"));
}

#[test]
fn mermaid_code_block_renders_svg() {
    let html = render_html("```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n");
    assert_eq!(mermaid_count(&html), 1);
    assert!(html.contains("<svg"));
    assert!(!html.contains("language-mermaid"));
}

#[test]
fn plantuml_code_block_renders_self_contained_output() {
    let html = render_html("```plantuml\n@startuml\nAlice -> Bob: Hi\n@enduml\n```\n");
    assert_eq!(plantuml_count(&html), 1);
    assert!(!html.contains("https://www.plantuml.com/plantuml/svg/"));
    assert!(html.contains("<svg") || html.contains("PlantUML render failed"));
    assert!(!html.contains("language-plantuml"));
}

#[test]
fn typst_code_block_renders_svg() {
    let html = render_html(
        "```typst\n#circle(radius: 30pt, fill: blue.lighten(30%))\n#text(size: 14pt)[Hello Typst]\n```\n",
    );
    assert_eq!(typst_count(&html), 1);
    assert!(html.contains("<svg") || html.contains("Typst render failed"));
    assert!(!html.contains("language-typst"));
}

#[test]
fn diagram_html_code_block_renders_raw_html() {
    let html = render_html(
        "```diagram html\n<div class=\"rounded-xl bg-sky-50 p-4\">Graph node</div>\n```\n",
    );
    assert_eq!(diagram_html_count(&html), 1);
    assert!(html.contains("rounded-xl bg-sky-50 p-4"));
    assert!(html.contains("Graph node"));
    assert!(!html.contains("language-diagram"));
}

#[test]
fn diagram_html_svg_marker_end_fragment_urls_are_preserved() {
    let html = render_html(
        "```diagram html\n<svg viewBox=\"0 0 200 50\"><defs><marker id=\"arr\"><path d=\"M0,0 L10,5 L0,10 Z\"/></marker></defs><path d=\"M 10 25 L 180 25\" fill=\"none\" stroke=\"#d97706\" marker-end=\"url(#arr)\"/></svg>\n```\n",
    );
    assert_eq!(diagram_html_count(&html), 1);
    assert!(html.contains("marker-end=\"url(#arr)\""));
    assert!(!html.contains("marker-end=\"url(\"#arr\")\""));
}

#[test]
fn diagram_html_tailwind_browser_runtime_is_embedded_when_needed() {
    let section = RenderedSection {
        title: String::new(),
        html: render_html("```diagram html\n<div class=\"rounded-xl\">Node</div>\n```\n"),
        outline: Vec::new(),
    };
    let html = build_html("Title", &[section], "PG");
    assert!(html.contains("<script>"));
    assert!(html.contains("tailwind"));
    assert!(html.contains("diagram-html-display"));
}

#[test]
fn bundled_typst_packages_are_embedded() {
    assert_eq!(crate::core::ext::typst::bundled_specs().len(), 3);
}

#[test]
fn typst_cetz_package_renders_svg() {
    let html = render_html(
        "```typst\n#import \"@preview/cetz:0.3.2\"\n#cetz.canvas({\n  import cetz.draw: *\n  circle((0, 0), radius: 1)\n})\n```\n",
    );
    assert_eq!(typst_count(&html), 1);
    assert!(
        html.contains("<svg"),
        "expected cetz diagram SVG, got: {}",
        &html[..html.len().min(500)]
    );
    assert!(!html.contains("Typst render failed"));
}

#[test]
fn github_callout_renders_admonition() {
    let html = render_html("> [!NOTE] Custom title\n> This is **important**.\n");
    assert_eq!(callout_count(&html), 1);
    assert!(html.contains("class=\"callout callout-note\""));
    assert!(html.contains("Custom title"));
    assert!(html.contains("<strong>important</strong>"));
    assert!(!html.contains("<blockquote>"));
}

#[test]
fn fenced_admonition_renders_nested_markdown() {
    let html = render_html(":::warning Pay attention\nUse `pagemd` safely.\n:::\n");
    assert_eq!(callout_count(&html), 1);
    assert!(html.contains("class=\"callout callout-warning\""));
    assert!(html.contains("Pay attention"));
    assert!(html.contains("<code>pagemd</code>"));
}

#[test]
fn local_markdown_images_are_embedded_as_data_uris() {
    let dir = temp_test_dir("local-image");
    std::fs::write(
        dir.join("tiny.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>",
    )
    .unwrap();
    let html = render_html_at("![tiny](tiny.svg)\n", &dir);
    assert!(html.contains("data:image/svg+xml;base64,"));
    assert!(!html.contains("src=\"tiny.svg\""));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn raw_html_resources_are_embedded() {
    let dir = temp_test_dir("raw-html");
    std::fs::write(
        dir.join("tiny.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>",
    )
    .unwrap();
    std::fs::write(dir.join("style.css"), "body { color: #111; }").unwrap();
    let html = render_html_at(
        "<img src=\"tiny.svg\"><link rel=\"stylesheet\" href=\"style.css\"><style>.x{background:url('tiny.svg')}</style>",
        &dir,
    );
    assert!(html.contains("data:image/svg+xml;base64,"));
    assert!(html.contains("data:text/css;base64,"));
    assert!(!html.contains("src=\"tiny.svg\""));
    assert!(!html.contains("href=\"style.css\""));
    assert!(!html.contains("url('tiny.svg')"));
    std::fs::remove_dir_all(dir).unwrap();
}
