const WORKSPACE_SCRIPT: &str = include_str!("../../../../assets/workspace.js");

pub(crate) fn workspace_script_tag() -> String {
    format!("<script data-pagemd-workspace>\n{WORKSPACE_SCRIPT}\n</script>\n")
}
