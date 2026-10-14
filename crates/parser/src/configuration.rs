//! Native-host options mapped to PageMD's pulldown-cmark analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constructs {
    pub attention: bool,
    pub frontmatter: bool,
    pub math_flow: bool,
    pub math_text: bool,
    pub html_flow: bool,
    pub html_text: bool,
    pub gfm_table: bool,
    pub gfm_strikethrough: bool,
    pub gfm_task_list_item: bool,
    pub gfm_footnote_definition: bool,
    pub gfm_autolink_literal: bool,
    pub mdx_expression_flow: bool,
    pub mdx_expression_text: bool,
    pub mdx_jsx_flow: bool,
    pub mdx_jsx_text: bool,
}
impl Default for Constructs {
    fn default() -> Self {
        Self {
            attention: true,
            frontmatter: false,
            math_flow: false,
            math_text: false,
            html_flow: true,
            html_text: true,
            gfm_table: false,
            gfm_strikethrough: false,
            gfm_task_list_item: false,
            gfm_footnote_definition: false,
            gfm_autolink_literal: false,
            mdx_expression_flow: false,
            mdx_expression_text: false,
            mdx_jsx_flow: false,
            mdx_jsx_text: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ParseOptions {
    pub constructs: Constructs,
}
impl ParseOptions {
    pub fn gfm() -> Self {
        let mut options = Self::default();
        options.constructs.gfm_table = true;
        options.constructs.gfm_strikethrough = true;
        options.constructs.gfm_task_list_item = true;
        options.constructs.gfm_footnote_definition = true;
        options.constructs.gfm_autolink_literal = true;
        options
    }
    pub fn mdx() -> Self {
        let mut options = Self::default();
        options.constructs.mdx_expression_flow = true;
        options.constructs.mdx_expression_text = true;
        options.constructs.mdx_jsx_flow = true;
        options.constructs.mdx_jsx_text = true;
        options
    }
    pub(crate) fn pulldown(&self) -> pulldown_cmark::Options {
        use pulldown_cmark::Options as P;
        let mut flags = P::empty();
        let c = &self.constructs;
        flags.set(P::ENABLE_TABLES, c.gfm_table);
        flags.set(P::ENABLE_STRIKETHROUGH, c.gfm_strikethrough);
        flags.set(P::ENABLE_TASKLISTS, c.gfm_task_list_item);
        flags.set(P::ENABLE_FOOTNOTES, c.gfm_footnote_definition);
        flags.set(P::ENABLE_MATH, c.math_text || c.math_flow);
        flags.set(P::ENABLE_YAML_STYLE_METADATA_BLOCKS, c.frontmatter);
        flags.set(P::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS, c.frontmatter);
        flags
    }
    pub(crate) fn validate(&self) -> Result<(), crate::message::Message> {
        let c = &self.constructs;
        if c.mdx_expression_flow
            || c.mdx_expression_text
            || c.mdx_jsx_flow
            || c.mdx_jsx_text
        {
            return Err(crate::message::Message::unsupported(
                "MDX is not part of PageMD's Markdown syntax",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CompileOptions {
    pub allow_dangerous_html: bool,
    pub allow_dangerous_protocol: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Options {
    pub parse: ParseOptions,
    pub compile: CompileOptions,
}
impl Options {
    pub fn gfm() -> Self {
        Self {
            parse: ParseOptions::gfm(),
            compile: CompileOptions::default(),
        }
    }
}
