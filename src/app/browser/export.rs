/// Built-in prompt for `/export`: turn validated live-page work into a `.pagemd.js` script.
const EXPORT_PROMPT_BODY: &str = r#"Export the **current PageMD Browser session** as a reusable `.pagemd.js` script.

Context: the user already tuned DOM cleanup / extraction (e.g. via `/pretty`, `/eval`, `/pmd`). Your job is to **capture what works** into a standalone script file — not to re-invent from scratch.

## Script contract (plain JS for Chrome extension)

The extension parses your file, shows each hook in the **Clean / Extract / Navigate / Stop** tabs, and runs them in the page MAIN world. **Top-level helpers are supported** — put shared `const` / `function` declarations **above** the hook functions (not inside ESM modules).

```javascript
const urlPattern = "https://example.com/*";  // whole-site: https://host/* ; path: https://host/docs/*

const DEFAULT_SELECTORS = ["nav", "footer"];  // optional shared helpers

function clean() {
  let removed = 0;
  DEFAULT_SELECTORS.forEach((sel) => {
    document.querySelectorAll(sel).forEach((el) => { el.remove(); removed++; });
  });
  return { removed };  // required shape when clean() is defined
}

function extract() {
  const el = document.querySelector("article") || document.body;
  return { title: document.title.trim(), html: el.innerHTML.trim() };
}

// optional:
function navigate() { /* return { success: boolean } */ }
function stop(context) { /* return { shouldStop: boolean, reason?: string } */ }
```

Hard rules:
- **Do NOT** use `import` / `export`.
- **`browser_eval`**: default **`record_undo: false`** for all read-only probes; set `true` only when replaying DOM mutations.
- **Never** read `.pagemd/runtime.json`, curl the bridge, or kill/restart the pagemd REPL process. If MCP is slow, ask the user to Ctrl+C the agent turn and retry.
- Hook names must be **`function clean()` / `function extract()`** declarations (not arrow assignments).
- **`urlPattern`**: call `browser_get_url`, use `https://<host>/*` for site-wide scripts.
- **`clean()`**: return **`{ removed: number }`**; mutates live DOM before extract.
- **`extract()`**: return **`{ title, html }`** (or `null` on failure). `html` = main content markup only.
- Helpers used by hooks must live at **top level in the same file** (the extension bundles them automatically).
- **Do NOT** write Markdown in chat. Markdown is produced later by PageMD from `html`.
- **Save location**: use **`browser_save_script` only** — it writes to the user's **current working directory** (where they started `pagemd browser`). Do **not** write under `~/Library/.../scripts` or any other path.

## Required workflow

1. `browser_get_url` + `browser_get_title` — anchor urlPattern and naming.
2. `browser_snap` — confirm page structure if needed.
3. Draft the full script text in memory following the contract above.
4. **Verify on the live tab before saving** (mandatory):
   - `browser_undo` with `{ "all": true }` if you need a clean baseline, then replay your logic; OR
   - use `browser_eval` with **`"record_undo": false`** to run read-only tests (e.g. `document.querySelector("h1")?.innerText`)
   - for mutating replay use `record_undo: true` sparingly; prefer drafting from session work already in `/pmd`
   - Optionally `browser_save_markdown` + `browser_get_session_markdown` to confirm extraction quality matches what the user approved in `/pmd`.
5. Fix any failures — **do not save** until the live test passes.
6. **`browser_save_script`** with `{ "filename": "<short-site-name>.pagemd.js", "content": "<full script source>" }`.
7. Reply briefly: saved path, urlPattern, what clean/extract do, and remind the user they can re-test with `/eval` or re-run on similar URLs.

If verification fails after 2 attempts, explain what is blocked and what the user should `/eval` manually — do not save a broken script."#;

pub fn build_export_prompt(export_dir: &std::path::Path, filename_hint: Option<&str>) -> String {
    let mut prompt = format!(
        "{EXPORT_PROMPT_BODY}\n\n**Export directory (mandatory):** `{}`",
        export_dir.display()
    );
    if let Some(hint) = filename_hint.filter(|s| !s.trim().is_empty()) {
        prompt.push_str(&format!(
            "\n\nUser preferred filename stem: `{hint}` (normalize to `<stem>.pagemd.js`)."
        ));
    }
    prompt
}
