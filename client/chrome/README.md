# PageMD Browser Extension

Chrome extension for running `.pagemd.js` extraction scripts (Clean → Extract → Navigate → Stop) on live pages. Pair it with `pagemd browser dev` for authoring and `pagemd browser script` for one-shot CLI runs.

## `.pagemd.js` standard

A script is plain JavaScript (no `import` / `export`) with this shape:

| Piece | Required | Role |
| --- | --- | --- |
| `const usage = "…"` | no | Blurb printed by `--usage` / `/run --usage` |
| `const defaultParams = { … }` | no | Defaults for `params` (overridable from CLI / host) |
| `const paramHelp = { key: "…", … }` | no | Per-param descriptions for `--usage` |
| `function clean()` | no | Remove noise **before** extract → `{ removed: number }` |
| `function extract()` | yes | Return `{ title, html }` or `null` (prefer `cloneNode`) |
| `function navigate()` | no | Go to next page → `{ success: boolean }` |
| `function stop(context)` | no | End batch → `{ shouldStop: boolean, reason?: string }` |

Page allow-lists are **not** in the script. Use CLI `--filter` (full URL or path like `/document/*`) when running.

At runtime the host injects:

```js
params = Object.assign({}, defaultParams || {}, cliParams)
```

Hooks read `params.*`. `stop(context)` also gets:

- `context.currentUrl`
- `context.pageIndex`
- `context.collectedUrls` / `context.collectedTitles`
- `context.params` (same merged object)

### CLI parameters

```bash
# Inspect params without launching Chrome
pagemd browser script example-docs.pagemd.js --usage

pagemd browser script example-docs.pagemd.js \
  --url https://docs.example.com/document/product/100/1 \
  --filter '/document/product/100/*' \
  -o example-docs \
  --param stopUrl=https://docs.example.com/document/product/100/2 \
  --param 'noiseSelectors=["#document-feedback"]'

# or a JSON object
pagemd browser script example-docs.pagemd.js \
  --url https://docs.example.com/document/product/100/1 \
  --filter '/document/*' \
  --params '{"stopUrl":"https://docs.example.com/document/product/100/2"}'
```

Inside the REPL (`pagemd browser dev`):

```
/run example-docs.pagemd.js --usage
/run example-docs.pagemd.js --param stopUrl=https://docs.example.com/document/product/100/2
```

`--param KEY=VALUE` parses VALUE as JSON when possible (`3`, `true`, `"…"`, `[…]`, `{…}`), otherwise as a string. Repeat `--param` as needed; `--params '{…}'` merges a whole object.

## Full example

The domain, identifiers and CSS selectors below are synthetic; replace them with your target site's values.

Crawl a fictional [documentation site](https://docs.example.com/document/product/100/1) into Markdown. Same logic as the side-panel hooks, packaged as a standard `.pagemd.js`:

```js
const usage =
  "Crawl example documentation into Markdown (clean → extract → next page).";

const defaultParams = {
  stopUrl: "https://docs.example.com/document/product/100/2",
  contentSelector: "article.document-content",
  noiseSelectors: [
    "#document-feedback",
    ".related-articles",
  ],
  stripSelectors: ["nav", ".ads"],
  nextSelector: "a.next-page",
};

const paramHelp = {
  stopUrl: "Stop after extracting this exact page URL",
  contentSelector: "CSS selector for the main article container",
  noiseSelectors: "Selectors removed from the live DOM in clean()",
  stripSelectors: "Selectors stripped from the extract() clone only",
  nextSelector: "CSS selector for the next-page control in navigate()",
};

function clean() {
  let removed = 0;
  (params.noiseSelectors || []).forEach((sel) => {
    document.querySelectorAll(sel).forEach((el) => {
      el.remove();
      removed++;
    });
  });
  return { removed };
}

function extract() {
  const el = document.querySelector(params.contentSelector);
  if (!el) return null;
  const clone = el.cloneNode(true);
  (params.stripSelectors || []).forEach((sel) => {
    clone.querySelectorAll(sel).forEach((node) => node.remove());
  });
  return {
    title: document.title,
    html: clone.innerHTML,
  };
}

function navigate() {
  const next = document.querySelector(params.nextSelector);
  if (!next || next.classList.contains("disabled")) {
    return { success: false };
  }
  next.click();
  return { success: true };
}

function stop(context) {
  if (params.stopUrl && context.currentUrl === params.stopUrl) {
    return { shouldStop: true, reason: "Reached stopUrl" };
  }
  // collectedUrls includes the page just extracted; a real loop means it appeared earlier.
  const prior = (context.collectedUrls || []).slice(0, -1);
  if (prior.includes(context.currentUrl)) {
    return { shouldStop: true, reason: "URL loop detected" };
  }
  return { shouldStop: false };
}
```

Run it:

```bash
pagemd browser script example-docs.pagemd.js --usage

pagemd browser script example-docs.pagemd.js \
  --url https://docs.example.com/document/product/100/1 \
  --filter '/document/product/100/*' \
  -o example-docs \
  --param stopUrl=https://docs.example.com/document/product/100/2
```

### Equivalent side-panel IIFEs

The extension editor also accepts one-off IIFEs (what you paste into Clean / Extract / Navigate / Stop tabs). The same crawl looks like:

```js
// Clean
(function() {
  const sels = ['#document-feedback', '.related-articles'];
  let removed = 0;
  sels.forEach(s =>
    document.querySelectorAll(s).forEach(el => { el.remove(); removed++; })
  );
  return { removed };
})()

// Extract
(function() {
  const el = document.querySelector("article.document-content");
  if (!el) return null;
  const clone = el.cloneNode(true);
  clone.querySelectorAll('nav, .ads').forEach(e => e.remove());
  return {
    title: document.title,
    html: clone.innerHTML
  };
})()

// Navigate
(function() {
  const next = document.querySelector("a.next-page");
  if (!next || next.classList.contains('disabled'))
    return { success: false };
  next.click();
  return { success: true };
})()

// Stop
(function(context) {
  if (context.currentUrl === 'https://docs.example.com/document/product/100/2')
    return { shouldStop: true, reason: 'Reached target' };
  return { shouldStop: false };
})()
```

Prefer the `.pagemd.js` file form for anything you want to reuse from the CLI or reload in the extension.
