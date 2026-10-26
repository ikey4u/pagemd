# PageMD native GPUI renderer

`pagemd-gpui` implements the shared `Renderer` trait directly with GPUI elements.
It owns `MarkdownViewState`, block layout, inline shaping, document updates,
selection/source mapping, search highlights and reveal geometry. It never calls
GPUI Kit's Markdown or HTML `TextView` and needs no Cargo patch or parser bridge.
Hovering link text shows its destination in a native tooltip, including wrapped
links and local file references. Tooltip targets follow the visible glyph rows;
paragraph gutters retain normal text selection and do not activate links.
The format-independent GPUI selection service supplies window-wide gestures,
copy integration and touch handles. The renderer targets GPUI Kit 0.8.1.
`SelectionFormat::SourceAcrossStyles` copies plain text inside one style and
preserves source across formatting, block boundaries and inline objects;
select-all always retains the complete source.

Consecutive lines in Markdown blockquotes keep their visible line breaks,
matching PageMD's HTML renderer. Soft breaks in ordinary paragraphs still
collapse to spaces. Source selection and copying retain the original Markdown.

```rust,ignore
use pagemd_gpui::{GpuiRenderer, Renderer, MarkdownView};
let document = pagemd_parser::parse(source, &GpuiRenderer::parse_options())?;
let state = GpuiRenderer.render(&document, cx).unwrap();
let view = MarkdownView::new(&state).selectable(true);
```

`render(source, cx)`, `markdown_state(source, cx)` and `markdown(id, source)`
are convenience entry points. Retaining the state preserves document geometry
across virtual-list frames. Long updates parse in the background, retain the
previous displayed document, and install only the newest requested revision.

`MarkdownExtensions` and `MarkdownPlugin` accept PageMD AST nodes and return
native block or inline elements. Hosts can render images, formulas and diagrams
with their own interaction handlers. `MarkdownStyle` configures native layout;
GPUI Kit component text styles are also accepted as styling inputs.

```toml
[dependencies]
pagemd-parser = { path = "../pagemd/crates/parser" }
pagemd-gpui = { path = "../pagemd/crates/gpui" }
```

Initialize GPUI Kit and use its Root/selection layer as usual. GPUI Kit retains
its own Markdown dependency for its other widgets; PageMD does not use it.
