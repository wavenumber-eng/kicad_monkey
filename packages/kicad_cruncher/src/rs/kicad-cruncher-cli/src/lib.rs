//! Native command-line boundary for KiCad Cruncher.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

pub mod design;

pub const TOP_LEVEL_HELP: &str = "\
KiCad Cruncher native CLI

Usage: kicad-cruncher <COMMAND>

Commands:
  design, design-review, dr  Generate a KiCad design-review bundle
  version                    Print version information

Options:
  -h, --help     Print help
  -V, --version  Print version
";

pub const DESIGN_HELP: &str = "\
Generate a KiCad design-review bundle

Usage: kicad-cruncher design [OPTIONS] [FILE]

Arguments:
  [FILE]  KiCad project or schematic; auto-detect one .kicad_pro when omitted

Options:
  -o, --output <DIRECTORY>  Output directory [default: ./output/design]
      --no-indexes          Exclude optional lookup indexes from Design JSON
  -h, --help                Print help
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
            Err(CliError::new(format!("unknown top-level option: {value}")))
        }
        Some(value) => Err(CliError::new(format!("unknown command: {value}"))),
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
                return Err(CliError::new(format!("unknown design option: {value}")));
            }
            _ => {
                if input.replace(PathBuf::from(argument)).is_some() {
                    return Err(CliError::new("design accepts at most one input file"));
                }
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
            "design accepts at most one input file"
        );
        assert_eq!(
            parse_args(os_args(["design", "--wat"]))
                .unwrap_err()
                .to_string(),
            "unknown design option: --wat"
        );
    }
}
