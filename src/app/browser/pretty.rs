/// Built-in prompt for `/pretty`: Cursor mutates a **hidden sandbox DOM copy**; the visible tab stays unchanged.
pub const PRETTY_PROMPT: &str = r#"Clean the page DOM for PageMD Markdown export inside the **PageMD sandbox** (hidden iframe copy). The user's visible Chrome tab must stay unchanged.

Steps (in order):
1. `browser_snap` — inspect sandbox structure once
2. `browser_clean` — remove nav, ads, sidebars, chrome in one call (or one targeted `browser_eval` if needed)
3. `browser_save_markdown` — extract Markdown from the sandbox DOM into the session file
4. Optionally `browser_get_session_markdown` once to verify quality

Hard rules:
- Change DOM ONLY via `browser_clean` / `browser_eval` (these operate on the sandbox when active)
- **`browser_eval` defaults to `record_undo: false`** — keep it false for read-only probes (`document.querySelector`, checks). Set `true` only when mutating DOM.
- Do **NOT** read `.pagemd/runtime.json`, curl the bridge, or **kill the pagemd process** — if a tool times out, tell the user to Ctrl+C and retry with `record_undo: false`
- Do **NOT** write or paste Markdown in chat
- Do **NOT** loop unless extraction is clearly broken (missing main body)
- Do **NOT** call `browser_get_html` unless eval failed and you need structure
- Compare with original baseline: user can run **`/pmd --original`** vs **`/pmd`** for before/after

When done, tell the user to run **`/pmd`** to preview cleaned session Markdown, and **`/pmd --original`** to compare with the unmodified page."#;
