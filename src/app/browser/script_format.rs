//! Shared `.pagemd.js` format text injected into Cursor workspace rules and `/export`.

/// Canonical script format for AI (dev workspace rules + export prompt).
pub const PAGEMD_JS_FORMAT: &str = r#"## `.pagemd.js` format (required)

Plain JavaScript only — **no** `import` / `export`. Top-level `const` / `function` helpers above hooks are OK (bundled with each hook at runtime).

There is **no** `urlPattern` in the script. Restrict pages at run time with CLI `--filter` (full URL or path glob like `/document/*`).

```javascript
const usage = "One-line description shown by --usage / /run --usage";

const defaultParams = {
  contentSelector: "article",
  noiseSelectors: ["nav", "footer"],
  nextSelector: "a.next",
  stopUrl: "",
};

const paramHelp = {
  contentSelector: "Main article CSS selector for extract()",
  noiseSelectors: "Removed by clean() on the live DOM",
  nextSelector: "Next-page control for navigate()",
  stopUrl: "Stop after extracting this exact URL (optional)",
};

function clean() {
  let removed = 0;
  (params.noiseSelectors || []).forEach((sel) => {
    document.querySelectorAll(sel).forEach((el) => { el.remove(); removed++; });
  });
  return { removed };
}

function extract() {
  const el = document.querySelector(params.contentSelector || "article");
  if (!el) return null;
  const clone = el.cloneNode(true);
  return { title: document.title.trim(), html: clone.innerHTML.trim() };
}

function navigate() {
  const next = document.querySelector(params.nextSelector);
  if (!next) return { success: false };
  next.click();
  return { success: true };
}

function stop(context) {
  if (params.stopUrl && context.currentUrl === params.stopUrl) {
    return { shouldStop: true, reason: "Reached stopUrl" };
  }
  // collectedUrls includes the page just extracted — check prior pages for loops
  const prior = (context.collectedUrls || []).slice(0, -1);
  if (prior.includes(context.currentUrl)) {
    return { shouldStop: true, reason: "URL loop detected" };
  }
  return { shouldStop: false };
}
```

### Contract

| Piece | Required | Notes |
| --- | --- | --- |
| `usage` | no | Shown by `pagemd browser script file.pagemd.js --usage` |
| `defaultParams` | no | Defaults for runtime `params` |
| `paramHelp` | no | Descriptions for `--usage` |
| `function clean()` | no | Mutates live DOM → `{ removed: number }` |
| `function extract()` | yes | → `{ title, html }` or `null` (prefer `cloneNode`) |
| `function navigate()` | no | → `{ success: boolean }` |
| `function stop(context)` | no | → `{ shouldStop, reason? }` |

Runtime injects: `params = Object.assign({}, defaultParams || {}, cliParams)`.
Hooks **must** be `function name()` declarations (not `const name = () => …`).
`stop(context)` fields: `currentUrl`, `pageIndex`, `collectedUrls`, `collectedTitles`, `params`.

CLI: `pagemd browser script file.pagemd.js --url … [--filter '/path/*'] [--param KEY=VALUE] [--usage]`
REPL: `/run file.pagemd.js …`, `/export [name]` to save after live verify.
"#;
