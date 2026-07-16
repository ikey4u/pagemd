use std::path::PathBuf;

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

/// Default HTML path when `--output` is omitted for a single file or directory.
pub(crate) fn default_output_path(args: &CliArgs) -> Option<PathBuf> {
    if let ([path], []) = (args.inputs.as_slice(), args.directories.as_slice()) {
        if path.is_dir() {
            let name = path.file_name()?.to_string_lossy();
            return Some(PathBuf::from(format!("{name}.html")));
        }
        return Some(path.with_extension("html"));
    }
    if let ([], [path]) = (args.inputs.as_slice(), args.directories.as_slice()) {
        let name = path.file_name()?.to_string_lossy();
        return Some(PathBuf::from(format!("{name}.html")));
    }
    None
}

pub(crate) fn run_convert(args: &CliArgs) -> anyhow::Result<()> {
    use crate::core::{export_to_file, HtmlExportOptions};
    use anyhow::Context;

    let owned_output = args.output.clone().or_else(|| default_output_path(args));
    let output = owned_output
        .as_deref()
        .context("Missing required output. Pass --output <FILE>, or convert a single file/directory to use the default <name>.html.")?;

    let opts = ConvertOptions::from(args);
    let html_opts = HtmlExportOptions {
        embed_workspace_script: true,
        client_mermaid_runtime: false,
        ..Default::default()
    };
    let result = export_to_file(&opts, &html_opts, output)?;

    eprintln!(
        "Written {} section(s) -> {}",
        result.section_count,
        output.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_args() -> CliArgs {
        CliArgs {
            inputs: Vec::new(),
            directories: Vec::new(),
            output: None,
            excludes: Vec::new(),
            title: None,
            icon: None,
            math_font_size: 16.0,
            katex_fonts: None,
        }
    }

    #[test]
    fn default_output_from_single_directory_flag() {
        let mut args = empty_args();
        args.directories = vec![PathBuf::from("docs")];
        assert_eq!(default_output_path(&args), Some(PathBuf::from("docs.html")));
    }

    #[test]
    fn default_output_from_single_markdown_file() {
        let mut args = empty_args();
        args.inputs = vec![PathBuf::from("guide/intro.md")];
        assert_eq!(
            default_output_path(&args),
            Some(PathBuf::from("guide/intro.html"))
        );
    }
}
