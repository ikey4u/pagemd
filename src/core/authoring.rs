//! Concise Markdown dialect help for host apps / LLM system prompts.
//!
//! Keep this short: Pack and similar hosts inject it into prompts. Prefer
//! [`markdown_help`] / [`diagram_help`] over [`crate::PAGEMD_LONG_ABOUT`]
//! (CLI text + full BASIC.md).

/// Compact PageMD authoring cheat-sheet for AI / host prompts.
///
/// Intentionally small so it guides formatting without crowding the task.
pub fn markdown_help() -> &'static str {
    MARKDOWN_HELP
}

/// Focused cheat-sheet for PageMD figure fences only.
pub fn diagram_help() -> &'static str {
    DIAGRAM_HELP
}

/// Same text as [`markdown_help`], as a `const` for `concat!` / static embedding.
pub const MARKDOWN_HELP: &str = "\
PageMD dialect (answers render with this engine — use rich Markdown when helpful):

Core: headings, **bold**, *italic*, ~~strike~~, `code`, links, images, lists, \
task lists, tables, blockquotes, footnotes (`[^id]` + `[^id]: note`).

Math: inline `$E=mc^2$`; display `$$...$$` or fenced ```math / ```latex.

Diagrams (fenced):
- ```mermaid / ```mmd — flowcharts, sequence, class, state, ER
- ```plantuml / ```puml — UML (needs network for render)
- ```typst — figures via Typst (optional `@preview/cetz` etc.)
- ```diagram html — raw HTML/SVG + Tailwind utility classes (runtime embedded)

Callouts:
- `> [!NOTE] Title` then body lines (also TIP/WARNING/DANGER/IMPORTANT/…)
- `:::tip Title` … `:::`
- `!!! warning \"Title\"` indented body

Prefer one clear figure over a wall of prose; callouts for caveats; tables for comparisons. \
Do not invent file contents; cite pack paths when answering about materials.\
";

/// Same text as [`diagram_help`].
pub const DIAGRAM_HELP: &str = "\
PageMD diagram fences:
- ```mermaid / ```mmd — flowcharts, sequence, class, state, ER
- ```plantuml / ```puml — UML (needs network for render)
- ```typst — figures via Typst (optional `@preview/cetz` etc.)
- ```diagram html — raw HTML/SVG + Tailwind utility classes (runtime embedded)
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_help_is_compact_and_covers_extensions() {
        let help = markdown_help();
        assert!(
            help.len() < 1600,
            "help too long for prompts: {}",
            help.len()
        );
        assert!(help.contains("mermaid"));
        assert!(help.contains("diagram html"));
        assert!(help.contains("plantuml") || help.contains("typst"));
        assert!(help.contains("[!NOTE]"));
        assert!(help.contains("$E=mc^2$") || help.contains("Math:"));
        assert!(help.contains("footnote") || help.contains("[^id]"));
    }

    #[test]
    fn diagram_help_lists_all_figure_fences() {
        let help = diagram_help();
        assert!(help.contains("mermaid"));
        assert!(help.contains("diagram html"));
        assert!(help.contains("typst"));
        assert!(help.contains("plantuml"));
        assert!(help.len() < 800, "diagram help too long: {}", help.len());
    }
}
