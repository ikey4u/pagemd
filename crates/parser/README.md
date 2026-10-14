# PageMD parsing library

`pagemd-parser` owns Markdown analysis, PageMD dialect preprocessing, CJK
compatibility and original-source coordinates. It produces immutable
`ParsedDocument` snapshots with an AST and source-aware events. PageMD's full
HTML renderer and its native GPUI renderer consume these snapshots through
`Renderer`.

```rust
use pagemd_parser::{parse, HtmlFragmentRenderer, ParseOptions, Renderer};

let document = parse("前**「重点」**后", &ParseOptions::gfm())?;
let html = HtmlFragmentRenderer::default().render(&document, ())?;
assert!(html.contains("<strong>「重点」</strong>"));
```

A renderer chooses its output, error and per-render context:

```rust
pub trait Renderer {
    type Output;
    type Error;
    type Context<'a>;
    fn render<'a>(
        &mut self,
        document: &ParsedDocument,
        context: Self::Context<'a>,
    ) -> Result<Self::Output, Self::Error>;
}
```

`HtmlFragmentRenderer` produces simple HTML. PageMD's `HtmlRenderer` provides
its complete HTML pipeline with themes, highlighting, diagrams, math SVG,
resources and footnotes. `pagemd-gpui::GpuiRenderer` creates native GPUI text
states with source selection and streaming. Third-party renderers implement the
same trait.

Dialect preprocessing helpers live in `extensions`. Native raw-source parsing
retains exact source positions for editing. The bounded document cache shares immutable parsing snapshots across renderers;
it holds no view state. MDX is outside PageMD's supported Markdown syntax and yields an
explicit error.

```sh
cargo test --manifest-path crates/parser/Cargo.toml --all-features
```
