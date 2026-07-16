pub const FOOTNOTE_MARKER: &str = "class=\"footnote\"";

const FOOTNOTE_SCRIPT: &str = include_str!("../../../../assets/footnotes.js");

pub fn footnote_script_tag() -> String {
    format!("<script data-pagemd-footnotes>\n{FOOTNOTE_SCRIPT}\n</script>\n")
}
