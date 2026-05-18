# PageMD

## Introduction

PageMD is a Rust command-line tool that converts Markdown into a SingleFile-style HTML document. It is designed for readable, portable documents with embedded styling, resources, syntax highlighting, diagrams, math rendering, and callout blocks.

Browser extension documentation is available at `extension/README.md`.

## Features

PageMD converts one or more Markdown files into a single HTML document, embeds the default stylesheet, inlines local and remote resources when possible, and rewrites common raw HTML resources such as `src`, `poster`, `<link href>`, and CSS `url(...)`.

- Generates SingleFile-style HTML by embedding styling and supported resources into one portable document.
- Supports common Markdown syntax, including headings, tables, task lists, footnotes, blockquotes, links, images, and fenced code blocks.
- Highlights code blocks with `syntect`.
- Renders inline and display math as embedded SVG.
- Renders `mermaid` / `mmd` diagrams as inline SVG.
- Fetches `plantuml` / `puml` / `uml` diagrams during conversion and embeds the returned SVG.
- Supports GitHub-style callouts, fenced admonitions, and indented admonitions.
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
