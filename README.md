# pagemd

Convert any web page to Markdown. A Chrome extension powered by Rust/WASM.

## Features

- **Quick Convert**: One-click page-to-Markdown using Readability-based extraction
- **AI-Assisted Hooks**: Copy page DOM context as a prompt, use any AI tool to generate extraction/navigation scripts
- **Batch Crawling**: Automatically navigate and extract multiple pages with configurable delays
- **Three Hook Types**:
  - **Extract Hook**: Define how to pull content from a page
  - **Navigate Hook**: Define how to go to the next page
  - **Stop Hook**: Define when to stop batch crawling
- **Recipes**: Save and reuse Hook configurations per website
- **WASM-Powered**: HTML-to-Markdown conversion runs in Rust/WASM for speed

## Architecture

```
src/
  background/    # Service worker: WASM routing, tab event monitoring
  content/       # Minimal content script (message proxy)
  sidepanel/     # Main UI: hook editor, pipeline controls, results
  options/       # Settings and recipe management
  offscreen/     # WASM execution environment
  lib/
    hook-executor.ts   # MAIN world + debugger fallback execution
    dom-summary.ts     # DOM summarization for AI prompts
    prompt.ts          # Prompt template assembly
    pipeline.ts        # Batch execution engine
    recipe.ts          # Recipe CRUD
    settings.ts        # Settings persistence
    types.ts           # Shared type definitions
  wasm.ts        # WASM loader
wasm/
  src/lib.rs     # Rust: html_to_markdown (html-to-markdown-rs)
```

## Usage

1. Click the extension icon to open the Side Panel
2. **Quick Convert**: Click "⚡ Quick Convert" for one-click extraction
3. **AI-Assisted**:
   - Select hook type, click "📋 Copy Prompt"
   - Paste into ChatGPT/Claude/DeepSeek, get JS code back
   - Paste code into the editor, click "▶ Test"
   - Click "📄 Convert Current Page" or "🔄 Batch Execute"
4. **Save as Recipe** for reuse on the same site

## Development

```bash
npm install
npm run dev          # Watch mode
npm run build        # Production build
npm run build:wasm   # Rebuild WASM module
```

## Tech Stack

- TypeScript + Chrome MV3
- Rust / wasm-bindgen / html-to-markdown-rs
- esbuild (via brosion)
