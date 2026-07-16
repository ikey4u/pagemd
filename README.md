# PageMD

## Introduction

PageMD converts Markdown into a SingleFile-style HTML document. It is designed for readable, portable documents with embedded styling, resources, syntax highlighting, diagrams, math rendering, and callout blocks.

Use it as a **Rust library** or via the `pagemd` CLI — both share the same rendering engine.

```rust
use pagemd::{render_to_html, RenderOptions};

let html = render_to_html("# Hello", &RenderOptions::default())?;
```

Browser extension documentation is available at `extension/README.md`.

## Browser REPL

`pagemd browser` starts an interactive workflow for turning live web pages into Markdown and reusable extraction scripts. It launches (or connects to) Chrome over the Chrome DevTools Protocol, opens a slash-command REPL, and optionally wires in Cursor so the agent can drive the page through MCP tools.

Typical usage:

```bash
pagemd browser --url https://example.com/article
pagemd browser --connect --port 9222    # attach to an existing Chrome
pagemd browser --clean --url https://example.com   # ephemeral profile
```

At a high level:

- **Page control** — navigate, reload, run JavaScript (`/goto`, `/reload`, `/eval`), with undo for DOM mutations.
- **Inspect & extract** — snapshot page structure (`/snap`), dump HTML or Markdown (`/html`, `/md`).
- **AI-assisted cleanup** — `/pretty` runs Cursor against a hidden sandbox copy of the page so the visible tab stays unchanged for comparison; cleaned output is saved per URL under `.pagemd/sessions/`.
- **Preview** — `/pmd` opens a live PageMD preview of the cleaned session Markdown; `/pmd --original` shows the unmodified baseline for side-by-side comparison.
- **Export scripts** — `/export` asks Cursor to save a validated `.pagemd.js` file (with `urlPattern`, `clean()`, `extract()`, and optional helpers) that you can load in the Chrome extension.

When Cursor is enabled, the REPL registers a local MCP bridge (`browser_snap`, `browser_clean`, `browser_eval`, `browser_save_markdown`, …). Use `/manual` and `/ai` to toggle whether free-form input is forwarded to the agent. See `pagemd browser --help` for flags such as `--no-ai`, `--port`, and profile options.

For day-to-day page extraction in the browser UI (Clean / Extract / Navigate / Stop tabs), use the Chrome extension documented in `extension/README.md`. The browser REPL is the authoring and tuning environment; the extension is the portable runtime for saved `.pagemd.js` scripts.

## Features

PageMD converts one or more Markdown files into a single HTML document, embeds the default stylesheet, inlines local and remote resources when possible, and rewrites common raw HTML resources such as `src`, `poster`, `<link href>`, and CSS `url(...)`.

- Generates SingleFile-style HTML by embedding styling and supported resources into one portable document.
- Supports common Markdown syntax, including headings, tables, task lists, footnotes, blockquotes, links, images, and fenced code blocks.
- Highlights code blocks with `syntect`.
- Renders inline and display math as embedded SVG.
- Renders `mermaid` / `mmd` diagrams as inline SVG.
- Fetches `plantuml` / `puml` / `uml` diagrams during conversion and embeds the returned SVG.
- Renders `diagram html` fenced blocks as styled HTML/SVG (Tailwind utilities supported; recommended for AI-generated diagrams).
- Supports GitHub-style callouts, fenced admonitions, and indented admonitions.
- Live-preview Markdown in the browser with hot reload (`pagemd view`).
- Provides a full conversion fixture at `examples/BASIC.md`.

## Development

Build and run the CLI:

```bash
cargo run -- --input input.md --output output.html
```

Convert the basic example into a demo HTML file:

```bash
cargo run -- --input examples/BASIC.md --output pagemd-basic.html
```

Preview the basic example in the default browser:

```bash
cargo run -- view --input examples/BASIC.md
```

Run validation checks:

```bash
cargo test
cargo check
cargo fmt --check
```
