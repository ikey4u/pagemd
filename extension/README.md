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
pagemd browser script meeting-docs.pagemd.js --usage

pagemd browser script meeting-docs.pagemd.js \
  --url https://cloud.tencent.com/document/product/1095/83658 \
  --filter '/document/product/1095/*' \
  -o meeting-docs \
  --param stopUrl=https://cloud.tencent.com/document/product/1095/94313 \
  --param 'noiseSelectors=["#document-feedback-container"]'

# or a JSON object
pagemd browser script meeting-docs.pagemd.js \
  --url https://cloud.tencent.com/document/product/1095/83658 \
  --filter '/document/*' \
  --params '{"stopUrl":"https://cloud.tencent.com/document/product/1095/94313"}'
```

Inside the REPL (`pagemd browser dev`):

```
/run meeting-docs.pagemd.js --usage
/run meeting-docs.pagemd.js --param stopUrl=https://…/94313
```

`--param KEY=VALUE` parses VALUE as JSON when possible (`3`, `true`, `"…"`, `[…]`, `{…}`), otherwise as a string. Repeat `--param` as needed; `--params '{…}'` merges a whole object.

## Full example

Crawl Tencent Meeting [Open Platform documentation](https://cloud.tencent.com/document/product/1095/83658) into Markdown. Same logic as the side-panel hooks, packaged as a standard `.pagemd.js`:

```js
const usage =
  "Crawl Tencent Meeting Open Platform documentation into Markdown (clean → extract → next page).";

const defaultParams = {
  stopUrl: "https://cloud.tencent.com/document/product/1095/94313",
  contentSelector: "div.J-mainContent.responsible.documents-container",
  noiseSelectors: [
    "#document-feedback-container",
    ".J-relatedArticleLayout",
  ],
  stripSelectors: ["nav", ".ads"],
  nextSelector: "a.next.J-docDetailPaginationPage",
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
pagemd browser script meeting-docs.pagemd.js --usage

pagemd browser script meeting-docs.pagemd.js \
  --url https://cloud.tencent.com/document/product/1095/83658 \
  --filter '/document/product/1095/*' \
  -o meeting-docs \
  --param stopUrl=https://cloud.tencent.com/document/product/1095/94313
```

### Equivalent side-panel IIFEs

The extension editor also accepts one-off IIFEs (what you paste into Clean / Extract / Navigate / Stop tabs). The same crawl looks like:

```js
// Clean
(function() {
  const sels = ['#document-feedback-container', '.J-relatedArticleLayout'];
  let removed = 0;
  sels.forEach(s =>
    document.querySelectorAll(s).forEach(el => { el.remove(); removed++; })
  );
  return { removed };
})()

// Extract
(function() {
  const el = document.querySelector("div.J-mainContent.responsible.documents-container");
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
  const next = document.querySelector("a.next.J-docDetailPaginationPage");
  if (!next || next.classList.contains('disabled'))
    return { success: false };
  next.click();
  return { success: true };
})()

// Stop
(function(context) {
  if (context.currentUrl === 'https://cloud.tencent.com/document/product/1095/94313')
    return { shouldStop: true, reason: 'Reached target' };
  return { shouldStop: false };
})()
```

Prefer the `.pagemd.js` file form for anything you want to reuse from the CLI or reload in the extension.
