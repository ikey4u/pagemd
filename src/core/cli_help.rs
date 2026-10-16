/// Top-level `pagemd --help` text.
pub const PAGEMD_LONG_ABOUT: &str = concat!(
    "\
Convert Markdown to a SingleFile-style HTML document (default mode).

Usage:
  pagemd -i INPUT.md -o OUTPUT.html
  pagemd -i a.md -i b.md -o doc.html
  pagemd -i docs/ -o docs.html
  pagemd -d docs/ -o doc.html
  pagemd -d src/ --exclude drafts/** node_modules

Use `pagemd view --help` for live preview with hot reload.
Use `pagemd browser --help` for Chrome workflows (`dev` REPL, `script` runner).

Supported features
─────────────────────────────────

Core Markdown:
  Headings, paragraphs, emphasis, strikethrough, links, images, horizontal rules,
  ordered/unordered/task lists, tables, blockquotes, footnotes ([^id] with [^id]: body),
  and fenced code blocks with syntax highlighting (syntect).

Math:
  Inline: $E=mc^2$
  Display: a paragraph that is only $$...$$ on one line, or a fenced block:

  ```math
  \\int_0^1 x^2\\,dx = \\frac{1}{3}
  ```
  Alias fence language: latex

Embedded Typst diagrams are not supported; Typst fences display source code.
Use Mermaid or diagram html to embed figures.

Diagrams — prefer these for AI-generated visuals:

  ```mermaid
  flowchart LR
    A[Input] --> B[PageMD] --> C[HTML]
  ```
  Fence languages: mermaid, mmd

  ```plantuml
  @startuml
  Alice -> Bob: hello
  @enduml
  ```
  Fence languages: plantuml, puml, uml

  ```diagram html
  <div class=\"rounded-3xl border border-slate-200 bg-sky-50 p-6\">
    <div class=\"text-xs font-bold uppercase tracking-wider text-sky-700\">Architecture</div>
    <div class=\"mt-2 grid grid-cols-2 gap-3\">
      <div class=\"rounded-xl bg-white p-3 font-semibold\">Markdown</div>
      <div class=\"rounded-xl bg-white p-3 font-semibold\">diagram html</div>
    </div>
  </div>
  ```
  Raw HTML/SVG inside a diagram container. Tailwind utilities are scoped to
  `.diagram-html-display` (Preflight is skipped so host chrome is not restyled).
  Best choice for architecture, UI mockups, and precise layouts.

Callouts / admonitions:
  GitHub-style blockquote marker:
    > [!NOTE] Optional title
    > Body supports **Markdown** and inline math.

  Fenced admonition:
  :::tip Optional title
  Body text
  :::

  Foldable section (collapsed by default; `:::details+` starts open):

  :::details Optional title
  Nested Markdown body
  :::

  Indented admonition:
    !!! warning \"Title\"
        Body text

  Kinds (and common aliases): note, info, tip, warning, danger, important, caution,
  question, success, failure, bug, example, quote, abstract; also hint, faq, error, …

Multi-file input:
  --input FILE (repeatable) and/or --dir DIR (recursive scan for .md/.markdown).
  Multiple files merge into one HTML document with a file-tree sidebar and per-file outline.
  --exclude PATTERN skips paths while scanning (name, glob, or drafts/** style).

Resources:
  Local images and assets referenced from Markdown or raw HTML are inlined when possible.
  Remote http(s) URLs in img/src/href/poster and CSS url(...) are fetched and embedded
  as data URIs when conversion succeeds.

Browser output:
  Self-contained HTML with embedded CSS. Footnote references show a hover hint with the
  note body. Use --title, --icon (2-char tab label), and --font-size for math scaling.

Example document
────────────────

",
    include_str!("../../examples/BASIC.md"),
);
