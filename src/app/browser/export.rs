/// Built-in prompt for `/export`: turn validated live-page work into a `.pagemd.js` script.
use super::script_format::PAGEMD_JS_FORMAT;

const EXPORT_INTRO: &str = r#"Export the **current PageMD Browser session** as a reusable `.pagemd.js` script.

Context: the user already tuned DOM cleanup / extraction (e.g. via `/pretty`, `/eval`, `/pmd`). Your job is to **capture what works** into a standalone script file — not to re-invent from scratch.

"#;

const EXPORT_OUTRO: &str = r#"

### Hard rules (export)
- **`browser_eval`**: default **`record_undo: false`** for read-only probes; set `true` only when mutating DOM.
- **Never** read `.pagemd/runtime.json`, curl the bridge, or kill/restart the pagemd REPL. If MCP is slow, ask the user to Ctrl+C and retry.
- **Do NOT** write Markdown in chat. Markdown is produced later by PageMD from `html`.
- **Save location**: use **`browser_save_script` only** — writes to the user's **current working directory** (REPL cwd). Do **not** write under `~/Library/.../scripts`.

## Required workflow

1. `browser_get_url` + `browser_get_title` — anchor naming and suggested `--filter`.
2. `browser_snap` — confirm page structure if needed.
3. Draft the full script text following the format above (`usage`, `defaultParams`, `paramHelp`, hooks). **Do not** include `urlPattern`.
4. **Verify on the live tab before saving** (mandatory):
   - `browser_undo` with `{ "all": true }` if needed; OR
   - `browser_eval` with **`"record_undo": false`** for read-only tests
   - Optionally `browser_save_markdown` + `browser_get_session_markdown` to match `/pmd` quality
5. Fix any failures — **do not save** until the live test passes.
6. **`browser_save_script`** with `{ "filename": "<short-site-name>.pagemd.js", "content": "<full script source>" }`.
7. Reply briefly: saved path, suggested `--filter`, params, what clean/extract do; remind the user to smoke-test with `/run` or `pagemd browser script`.

If verification fails after 2 attempts, explain what is blocked and what the user should `/eval` manually — do not save a broken script."#;

pub fn build_export_prompt(export_dir: &std::path::Path, filename_hint: Option<&str>) -> String {
    let mut prompt = format!(
        "{EXPORT_INTRO}{PAGEMD_JS_FORMAT}{EXPORT_OUTRO}\n\n**Export directory (mandatory):** `{}`",
        export_dir.display()
    );
    if let Some(hint) = filename_hint.filter(|s| !s.trim().is_empty()) {
        prompt.push_str(&format!(
            "\n\nUser preferred filename stem: `{hint}` (normalize to `<stem>.pagemd.js`)."
        ));
    }
    prompt
}
