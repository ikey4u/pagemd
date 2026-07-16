use crate::app::cli::CliArgs;
use crate::core::{ConvertOptions, OutputFormat};

impl From<&CliArgs> for ConvertOptions {
    fn from(args: &CliArgs) -> Self {
        ConvertOptions {
            inputs: args.inputs.clone(),
            directories: args.directories.clone(),
            excludes: args.excludes.clone(),
            title: args.title.clone(),
            icon: args.icon.clone(),
            math_font_size: args.math_font_size,
            katex_fonts: args.katex_fonts.clone(),
            output_format: OutputFormat::Html,
            client_mermaid: false,
        }
    }
}

impl From<CliArgs> for ConvertOptions {
    fn from(args: CliArgs) -> Self {
        ConvertOptions::from(&args)
    }
}

pub(crate) fn run_convert(args: &CliArgs) -> anyhow::Result<()> {
    use crate::core::{export_to_file, HtmlExportOptions};
    use anyhow::Context;

    let output = args
        .output
        .as_deref()
        .context("Missing required output. Pass --output <FILE>.")?;

    let opts = ConvertOptions::from(args);
    let html_opts = HtmlExportOptions {
        embed_workspace_script: true,
        client_mermaid_runtime: false,
    };
    let result = export_to_file(&opts, &html_opts, output)?;

    eprintln!(
        "Written {} section(s) -> {}",
        result.section_count,
        output.display()
    );

    Ok(())
}
