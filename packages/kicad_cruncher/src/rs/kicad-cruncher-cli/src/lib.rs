//! Native command-line boundary for KiCad Cruncher.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

pub mod design;
pub mod schematic_review_svg;

pub const TOP_LEVEL_HELP: &str = "\
usage: kicad-cruncher <command> ...

High-level CLI for KiCad design workflows

Commands:
  design (design-review, dr)  generate KiCad design review artifacts
  version                     print version information

Options:
  -h, --help     show this help message and exit
  -V, --version  print version information and exit
";

pub const DESIGN_HELP: &str = "\
usage: kicad-cruncher design [-h] [-o OUTPUT] [--no-indexes] [file]

Generate a KiCad design review bundle from .kicad_pro or .kicad_sch files. The output includes KiCad-native design JSON, enriched black-and-white schematic SVGs, an occurrence-scoped compiled schematic graph, enriched PCB copper-layer SVGs, KiCad-native netlist JSON, a KiCad S-expression netlist, a manifest, and a README for review agents. The design JSON includes project metadata, schematic hierarchy, components, nets, variants, and optional lookup indexes.

positional arguments:
  file                  KiCad project or schematic file; optional when one
                        .kicad_pro is in CWD

options:
  -h, --help            show this help message and exit
  -o OUTPUT, --output OUTPUT
                        output directory (default: ./output/design)
  --no-indexes          exclude lookup indexes from JSON

Examples:
  kicad-cruncher design project.kicad_pro
  kicad-cruncher design-review project.kicad_pro
  kicad-cruncher dr project.kicad_pro
  kicad-cruncher design schematic.kicad_sch
  kicad-cruncher design                    # Auto-detect one .kicad_pro in CWD
  kicad-cruncher design project.kicad_pro --no-indexes
  kicad-cruncher design project.kicad_pro -o output_dir/
";

#[derive(Debug, Eq, PartialEq)]
pub enum Invocation {
    TopLevelHelp,
    Version,
    DesignHelp,
    Design(DesignOptions),
}

#[derive(Debug, Eq, PartialEq)]
pub struct DesignOptions {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub include_indexes: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Invocation, CliError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Ok(Invocation::TopLevelHelp);
    };

    match command.to_str() {
        Some("-h" | "--help") => no_trailing_args(args, Invocation::TopLevelHelp),
        Some("-V" | "--version" | "version") => no_trailing_args(args, Invocation::Version),
        Some("design" | "design-review" | "dr") => parse_design_args(args),
        Some(value) if value.starts_with('-') => {
            Err(CliError::new(format!("unrecognized arguments: {value}")))
        }
        Some(value) => Err(CliError::new(format!(
            "argument <command>: invalid choice: '{value}'"
        ))),
        None => Err(CliError::new("command is not valid Unicode")),
    }
}

fn no_trailing_args(
    mut args: impl Iterator<Item = OsString>,
    invocation: Invocation,
) -> Result<Invocation, CliError> {
    match args.next() {
        Some(argument) => Err(CliError::new(format!(
            "unexpected argument: {}",
            argument.to_string_lossy()
        ))),
        None => Ok(invocation),
    }
}

fn parse_design_args(mut args: impl Iterator<Item = OsString>) -> Result<Invocation, CliError> {
    let mut input = None;
    let mut output = None;
    let mut include_indexes = true;

    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("-h" | "--help") => {
                return no_trailing_args(args, Invocation::DesignHelp);
            }
            Some("--no-indexes") => include_indexes = false,
            Some("-o" | "--output") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--output requires a directory"))?;
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err(CliError::new("--output may be supplied only once"));
                }
            }
            Some(value) if value.starts_with('-') => {
                return Err(CliError::new(format!("unrecognized arguments: {value}")));
            }
            _ => {
                if input.is_some() {
                    return Err(CliError::new(format!(
                        "unrecognized arguments: {}",
                        argument.to_string_lossy()
                    )));
                }
                input = Some(PathBuf::from(argument));
            }
        }
    }

    Ok(Invocation::Design(DesignOptions {
        input,
        output,
        include_indexes,
    }))
}

pub fn version_text() -> String {
    format!("kicad-cruncher {}", env!("CARGO_PKG_VERSION"))
}

pub fn os_args(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Vec<OsString> {
    args.into_iter()
        .map(|argument| argument.as_ref().to_os_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_requests_top_level_help() {
        assert_eq!(parse_args(Vec::new()).unwrap(), Invocation::TopLevelHelp);
    }

    #[test]
    fn version_forms_are_equivalent() {
        for argument in ["version", "--version", "-V"] {
            assert_eq!(
                parse_args(os_args([argument])).unwrap(),
                Invocation::Version
            );
        }
        assert!(version_text().starts_with("kicad-cruncher "));
    }

    #[test]
    fn design_aliases_share_options() {
        let expected = Invocation::Design(DesignOptions {
            input: Some(PathBuf::from("project.kicad_pro")),
            output: Some(PathBuf::from("review")),
            include_indexes: false,
        });
        for command in ["design", "design-review", "dr"] {
            assert_eq!(
                parse_args(os_args([
                    command,
                    "project.kicad_pro",
                    "--no-indexes",
                    "--output",
                    "review",
                ]))
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn design_help_is_command_specific() {
        assert_eq!(
            parse_args(os_args(["design", "--help"])).unwrap(),
            Invocation::DesignHelp
        );
    }

    #[test]
    fn duplicate_inputs_and_unknown_options_fail() {
        assert_eq!(
            parse_args(os_args(["design", "one.kicad_sch", "two.kicad_sch"]))
                .unwrap_err()
                .to_string(),
            "unrecognized arguments: two.kicad_sch"
        );
        assert_eq!(
            parse_args(os_args(["design", "--wat"]))
                .unwrap_err()
                .to_string(),
            "unrecognized arguments: --wat"
        );
    }
}
