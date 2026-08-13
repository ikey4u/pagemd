# PageMD

## Introduction

PageMD converts Markdown into a SingleFile-style HTML document. It is designed for readable, portable documents with embedded styling, resources, syntax highlighting, diagrams, math rendering, and callout blocks.

Use it as a **Rust library** or via the `pagemd` CLI — both share the same rendering engine.

```rust
use pagemd::{render_to_html, RenderOptions};

let html = render_to_html("# Hello", &RenderOptions::default())?;
```

Browser extension documentation is available at `extension/README.md`.

## Browser

`pagemd browser` drives Chrome over the Chrome DevTools Protocol. It has two subcommands:

### `dev` — interactive REPL

Author and tune extraction scripts with slash commands and optional Cursor agent tools.

```bash
pagemd browser dev --url https://example.com/article
pagemd browser dev --connect --port 9222    # attach to an existing Chrome
pagemd browser dev --clean --url https://example.com   # ephemeral profile
```

At a high level:

- **Page control** — navigate, reload, run JavaScript (`/goto`, `/reload`, `/eval`), with undo for DOM mutations.
- **Inspect & extract** — snapshot page structure (`/snap`), dump HTML or Markdown (`/html`, `/md`).
- **AI-assisted cleanup** — `/pretty` runs Cursor against a hidden sandbox copy of the page so the visible tab stays unchanged for comparison; cleaned output is saved per URL under `.pagemd/sessions/`.
- **Preview** — `/pmd` opens a live PageMD preview of the cleaned session Markdown; `/pmd --original` shows the unmodified baseline for side-by-side comparison.
- **Run scripts** — `/run file.pagemd.js` executes a validated script against the live tab.
- **Export scripts** — `/export` asks Cursor to save a validated `.pagemd.js` file (with `urlPattern`, `clean()`, `extract()`, and optional helpers) that you can load in the Chrome extension.

When Cursor is enabled, the REPL registers a local MCP bridge (`browser_snap`, `browser_clean`, `browser_eval`, `browser_save_markdown`, …). Use `/manual` and `/ai` to toggle whether free-form input is forwarded to the agent. See `pagemd browser dev --help` for flags such as `--no-ai`, `--port`, and profile options.

### `script` — one-shot runner

Run a `.pagemd.js` file from the CLI (no REPL): open Chrome, navigate to `--url`, loop clean/extract/navigate/stop, write Markdown, exit.

Scripts may declare `const defaultParams = { … }` and read `params` in hooks. Override from the command line with `--param KEY=VALUE` or `--params '{…}'`. Use `--usage` to dump the script's params without launching Chrome.

```bash
pagemd browser script meeting-docs.pagemd.js --usage

pagemd browser script meeting-docs.pagemd.js \
  --url https://cloud.tencent.com/document/product/1095/83658 \
  -o meeting-docs \
  --param stopUrl=https://cloud.tencent.com/document/product/1095/94313

pagemd browser script site.pagemd.js --url https://example.com/a --headless
```

Pages are written as `001-….md`, `002-….md`, … under the output directory (default `<script-stem>-run/`). Pass `-o out.md` only if you want one combined Markdown file.
See `extension/README.md` for the `.pagemd.js` contract and a full worked example.

For day-to-day page extraction in the browser UI (Clean / Extract / Navigate / Stop tabs), use the Chrome extension documented in `extension/README.md`. The `dev` REPL is the authoring and tuning environment; `script` and the extension are the portable runtimes for saved `.pagemd.js` scripts.

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

Convert a whole directory of Markdown files into one SingleFile HTML document:

```bash
cargo run -- --input examples --output examples.html
# or
cargo run -- --dir docs -o docs.html
```

Convert the basic example into a demo HTML file:

```bash
cargo run -- --input examples/BASIC.md --output pagemd-basic.html
```

Preview the basic example in the default browser:

```bash
cargo run -- view --input examples/BASIC.md
```

Cross-compile release binaries (Linux glibc 2.17+, Windows, macOS) with Zig:

```bash
mise run dist
```

Artifacts are written under `dist/` as `pagemd-{os}-{arch}-{version}.zip`
(for example `pagemd-linux-x64-0.7.0.zip`).

Run validation checks:

```bash
cargo test
cargo check
cargo fmt --check
```
