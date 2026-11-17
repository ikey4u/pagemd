//! Synthetic CJK punctuation fixtures for the shared PageMD parser.
use pagemd_parser::{mdast::Node, ParseOptions};

#[test]
fn cjk_punctuation_emphasis_allows_adjacent_latin_text_and_digits() {
    for (source, expected) in [
        ("**中文句号。**EML", "<p><strong>中文句号。</strong>EML</p>"),
        ("**中文逗号，**EML", "<p><strong>中文逗号，</strong>EML</p>"),
        ("**中文冒号：**ABC", "<p><strong>中文冒号：</strong>ABC</p>"),
        (
            "**中文括号（说明）**v2",
            "<p><strong>中文括号（说明）</strong>v2</p>",
        ),
        ("**中文句号。**123", "<p><strong>中文句号。</strong>123</p>"),
        (
            "ASCII**「中文」**after",
            "<p>ASCII<strong>「中文」</strong>after</p>",
        ),
        ("ASCII*（重点）*after", "<p>ASCII<em>（重点）</em>after</p>"),
        (
            "前***中文。***EML",
            "<p>前<em><strong>中文。</strong></em>EML</p>",
        ),
        (
            "**中文。**EML**重点。**ABC",
            "<p><strong>中文。</strong>EML<strong>重点。</strong>ABC</p>",
        ),
    ] {
        assert_eq!(pagemd_parser::to_html(source), expected, "{source}");
    }
}

#[test]
fn cjk_latin_boundary_normalization_preserves_non_prose_and_ascii_rules() {
    for source in [
        "`**中文。**EML`",
        "```md\n**中文。**EML\n```",
        "    **中文。**EML",
        "\\*\\*中文。\\*\\*EML",
        "[链接](https://example.com/**中文。**EML)",
        "<div title=\"**中文。**EML\">text</div>",
        "**English.**after",
        "ABC**(English)**after",
        "**中文。 **EML",
    ] {
        assert_eq!(
            pagemd_parser::fix_emphasis_cjk_punctuation(source),
            source,
            "{source}"
        );
    }
    assert!(!pagemd_parser::to_html("100**（税）**元").contains("<strong>"));
}

#[test]
fn cjk_emphasis_handles_punctuation_next_to_chinese() {
    for (source, expected) in [
        (
            "内容经由**示例字段（编号 ①）**进入 Example Handler。",
            "<p>内容经由<strong>示例字段（编号 ①）</strong>进入 Example Handler。</p>",
        ),
        (
            "示例也从 **Sample Component 的示例字段（编号 ①）**进来的。",
            "<p>示例也从 <strong>Sample Component 的示例字段（编号 ①）</strong>进来的。</p>",
        ),
        (
            "内容经由**示例字段 (编号 ①)**进入。",
            "<p>内容经由<strong>示例字段 (编号 ①)</strong>进入。</p>",
        ),
        (
            "中文**「重要内容」**紧邻中文",
            "<p>中文<strong>「重要内容」</strong>紧邻中文</p>",
        ),
        (
            "这是**“重要内容”**和**（另一个重点）**。",
            "<p>这是<strong>“重要内容”</strong>和<strong>（另一个重点）</strong>。</p>",
        ),
        (
            "之前**重点。**之后",
            "<p>之前<strong>重点。</strong>之后</p>",
        ),
        ("之前*「斜体」*之后", "<p>之前<em>「斜体」</em>之后</p>"),
        ("前**〔重点〕**后", "<p>前<strong>〔重点〕</strong>后</p>"),
        ("前**【重点】**后", "<p>前<strong>【重点】</strong>后</p>"),
        ("前**《重点》**后", "<p>前<strong>《重点》</strong>后</p>"),
    ] {
        assert_eq!(pagemd_parser::to_html(source), expected, "{source}");
    }
}

#[test]
fn cjk_emphasis_keeps_nested_inline_markup() {
    for (source, expected) in [
        (
            "前**「*重点*」**后",
            "<p>前<strong>「<em>重点</em>」</strong>后</p>",
        ),
        (
            "前***「重点」***后",
            "<p>前<em><strong>「重点」</strong></em>后</p>",
        ),
        (
            "前**「[链接](https://example.com) 与 `值`」**后",
            "<p>前<strong>「<a href=\"https://example.com\">链接</a> 与 <code>值</code>」</strong>后</p>",
        ),
        (
            "前**「A &amp; B」**后",
            "<p>前<strong>「A &amp; B」</strong>后</p>",
        ),
    ] {
        assert_eq!(pagemd_parser::to_html(source), expected, "{source}");
    }
}

#[test]
fn cjk_emphasis_keeps_original_utf8_positions() {
    fn strong_ranges(node: &Node, found: &mut Vec<std::ops::Range<usize>>) {
        if let Node::Strong(_) = node {
            let position = node.position().unwrap();
            found.push(position.start.offset..position.end.offset);
        }
        if let Some(children) = node.children() {
            for child in children {
                strong_ranges(child, found);
            }
        }
    }
    let source = "1. 内容经由**示例字段（编号 ①）**进入。\n\n2. 前**「[链接](https://example.com)」**后。";
    let tree = pagemd_parser::to_mdast(source, &ParseOptions::gfm()).unwrap();
    let mut ranges = vec![];
    strong_ranges(&tree, &mut ranges);
    assert_eq!(ranges.len(), 2);
    assert_eq!(&source[ranges[0].clone()], "**示例字段（编号 ①）**");
    assert_eq!(
        &source[ranges[1].clone()],
        "**「[链接](https://example.com)」**"
    );
}

#[test]
fn cjk_emphasis_preserves_literal_markers_and_standard_boundaries() {
    for (source, expected) in [
        ("foo**(bar)**baz", "<p>foo**(bar)**baz</p>"),
        ("foo**bar.**baz", "<p>foo**bar.**baz</p>"),
        ("a**b**c", "<p>a<strong>b</strong>c</p>"),
        ("中文__（重要）__中文", "<p>中文__（重要）__中文</p>"),
        ("前** 内容**后", "<p>前** 内容**后</p>"),
        ("前**内容 **后", "<p>前**内容 **后</p>"),
        ("前**「内容」后", "<p>前**「内容」后</p>"),
        ("前\\*\\*「内容」\\*\\*后", "<p>前**「内容」**后</p>"),
        ("前`**「代码」**`后", "<p>前<code>**「代码」**</code>后</p>"),
        (
            "```text\n前**「代码」**后\n```",
            "<pre><code class=\"language-text\">前**「代码」**后\n</code></pre>",
        ),
        (
            "    前**「代码」**后",
            "<pre><code>前**「代码」**后\n</code></pre>",
        ),
        (
            "[链接](https://example.com/**中文**)",
            "<p><a href=\"https://example.com/**%E4%B8%AD%E6%96%87**\">链接</a></p>",
        ),
    ] {
        assert_eq!(pagemd_parser::to_html(source), expected, "{source}");
    }
}

#[test]
fn cjk_emphasis_covers_tables_and_other_cjk_scripts() {
    let source = "| 标题 |\n| --- |\n| 前**（重点）**后 |";
    let html = pagemd_parser::to_html_with_options(
        source,
        &pagemd_parser::Options::gfm(),
    )
    .unwrap();
    assert!(html.contains("<td>前<strong>（重点）</strong>后</td>"));
    for source in [
        "かな**「重点」**かな",
        "한글**（重点）**한글",
        "𠀀**（重点）**𠀀",
    ] {
        assert!(
            pagemd_parser::to_html(source).contains("<strong>"),
            "{source}"
        );
    }
}
