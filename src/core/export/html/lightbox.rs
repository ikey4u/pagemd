const LIGHTBOX_SCRIPT: &str = include_str!("../../../../assets/diagram-lightbox.js");

pub(crate) fn diagram_lightbox_script_tag() -> String {
    format!("<script data-pagemd-diagram-lightbox>\n{LIGHTBOX_SCRIPT}\n</script>\n")
}
