//! Native command-line boundary for KiCad Cruncher.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

pub mod design;
pub mod design_bundle;
pub mod pcb_review_svg;
pub mod pcb_svg;
mod performance;
pub mod schematic_review_svg;
pub mod toon;
pub mod toon_config;

pub const TOP_LEVEL_HELP: &str = "\
usage: kicad-cruncher <command> ...

High-level CLI for KiCad design workflows

Commands:
  design (design-review, dr)  generate KiCad design review artifacts
  toon                        render native top/bottom assembly illustrations
  version                     print version information

Options:
  -h, --help     show this help message and exit
  -V, --version  print version information and exit
";

pub const TOON_HELP: &str = "\
usage: kicad-cruncher toon [-h] [-o OUTPUT] [--doc BOARD] [--side {top,bottom,both}] [--assembly] [--footprint REF] [--variant NAME | --all-variants] [--theme THEME | --soldermask-color #RRGGBB] [--config PATH] [--write-config PATH] [--workers N] [--cache-dir PATH | --no-cache] [--timings PATH] [file]

Render native KiCad board illustrations with physical copper, substrate,
solder-mask film, silkscreen, and embedded STEP models through Geometer.

positional arguments:
  file                  .kicad_pcb or .kicad_pro; optional when exactly one
                        .kicad_pcb is in CWD

options:
  -h, --help            show this help message and exit
  -o OUTPUT, --output OUTPUT
                        transactional output directory (default: ./output/toon)
  --doc BOARD, --pcbdoc BOARD
                        select the adjacent board within a project
  --side {top,bottom,both}
                        board side(s) to render (default: both)
  --assembly            add Autodoc-fitted assembly designators
  --footprint REF       isolate one placed footprint and its embedded models
  --variant NAME        named KiCad project variant; omit DNP models/labels
  --all-variants        base plus all named variants in separate directories
  --theme {saved,white,black,blue,red,purple,yellow,green}
                        Altium-compatible mask/silk palette (default: saved)
  --soldermask-color #RRGGBB
                        custom solder-mask film color or auto
  --config PATH         editable PCB SVG JSON/JSONC config (default: toon.config beside input)
  --write-config PATH   write the resolved editable config and exit
  --workers N           bounded Geometer workers, 1..64 (default: 4)
  --cache-dir PATH      persistent successful-model cache directory
  --no-cache            disable persistent cache; same-job reuse remains enabled
  --timings PATH        write machine-readable phase and model-work timings

Examples:
  kicad-cruncher toon board.kicad_pcb
  kcr toon project.kicad_pro --side top -o output/toon
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

#[allow(
    clippy::large_enum_variant,
    reason = "the CLI owns exactly one invocation and avoids heap allocation at this short-lived boundary"
)]
#[derive(Debug, Eq, PartialEq)]
pub enum Invocation {
    TopLevelHelp,
    Version,
    DesignHelp,
    Design(DesignOptions),
    ToonHelp,
    Toon(ToonOptions),
}

#[derive(Debug, Eq, PartialEq)]
pub struct DesignOptions {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub include_indexes: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToonSides {
    Top,
    Bottom,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToonTheme {
    Saved,
    White,
    Black,
    Blue,
    Red,
    Purple,
    Yellow,
    Green,
}

impl ToonTheme {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::White => "white",
            Self::Black => "black",
            Self::Blue => "blue",
            Self::Red => "red",
            Self::Purple => "purple",
            Self::Yellow => "yellow",
            Self::Green => "green",
        }
    }

    pub(crate) const fn colors(self) -> (&'static str, &'static str) {
        match self {
            Self::Saved => ("auto", "#F5F5F5"),
            Self::White => ("#EEEEEE", "#000000"),
            Self::Black => ("#000000", "#F5F5F5"),
            Self::Blue => ("#1D4F91", "#F5F5F5"),
            Self::Red => ("#A62A2A", "#F5F5F5"),
            Self::Purple => ("#6F3C8A", "#F5F5F5"),
            Self::Yellow => ("#D6A600", "#F5F5F5"),
            Self::Green => ("#176B3A", "#F5F5F5"),
        }
    }
}

impl ToonSides {
    pub(crate) fn board_sides(self) -> Vec<pcb_svg::preview::BoardSide> {
        match self {
            Self::Top => vec![pcb_svg::preview::BoardSide::Top],
            Self::Bottom => vec![pcb_svg::preview::BoardSide::Bottom],
            Self::Both => vec![
                pcb_svg::preview::BoardSide::Top,
                pcb_svg::preview::BoardSide::Bottom,
            ],
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ToonOptions {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub pcbdoc: Option<String>,
    pub sides: ToonSides,
    pub assembly: bool,
    pub footprint: Option<String>,
    pub variant: Option<String>,
    pub all_variants: bool,
    pub theme: Option<ToonTheme>,
    pub soldermask_color: Option<String>,
    pub config: Option<PathBuf>,
    pub write_config: Option<PathBuf>,
    pub workers: usize,
    pub cache_dir: Option<PathBuf>,
    pub no_cache: bool,
    pub timings: Option<PathBuf>,
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
        Some("toon") => parse_toon_args(args),
        Some(value) if value.starts_with('-') => {
            Err(CliError::new(format!("unrecognized arguments: {value}")))
        }
        Some(value) => Err(CliError::new(format!(
            "argument <command>: invalid choice: '{value}'"
        ))),
        None => Err(CliError::new("command is not valid Unicode")),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the dependency-free CLI parser keeps Toon option validation in one auditable match"
)]
fn parse_toon_args(mut args: impl Iterator<Item = OsString>) -> Result<Invocation, CliError> {
    let mut input = None;
    let mut output = None;
    let mut pcbdoc = None;
    let mut sides = ToonSides::Both;
    let mut side_seen = false;
    let mut assembly = false;
    let mut footprint = None;
    let mut variant = None;
    let mut all_variants = false;
    let mut theme = None;
    let mut soldermask_color = None;
    let mut config = None;
    let mut write_config = None;
    let mut workers = None;
    let mut cache_dir = None;
    let mut no_cache = false;
    let mut timings = None;
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("-h" | "--help") => return no_trailing_args(args, Invocation::ToonHelp),
            Some("-o" | "--output") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--output requires a directory"))?;
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err(CliError::new("--output may be supplied only once"));
                }
            }
            Some("--side") => {
                if side_seen {
                    return Err(CliError::new("--side may be supplied only once"));
                }
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--side requires top, bottom, or both"))?;
                sides = match value.to_str() {
                    Some("top") => ToonSides::Top,
                    Some("bottom") => ToonSides::Bottom,
                    Some("both") => ToonSides::Both,
                    Some(value) => {
                        return Err(CliError::new(format!(
                            "invalid --side value {value:?}; expected top, bottom, or both"
                        )));
                    }
                    None => return Err(CliError::new("--side value is not valid Unicode")),
                };
                side_seen = true;
            }
            Some("--assembly") => {
                if assembly {
                    return Err(CliError::new("--assembly may be supplied only once"));
                }
                assembly = true;
            }
            Some("--doc" | "--pcbdoc") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--doc requires a board name"))?
                    .into_string()
                    .map_err(|_| CliError::new("--doc value is not valid Unicode"))?;
                if value.is_empty() {
                    return Err(CliError::new("--doc board name must not be empty"));
                }
                if pcbdoc.replace(value).is_some() {
                    return Err(CliError::new("--doc/--pcbdoc may be supplied only once"));
                }
            }
            Some("--soldermask-color") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--soldermask-color requires #RRGGBB"))?;
                let value = value
                    .into_string()
                    .map_err(|_| CliError::new("--soldermask-color value is not valid Unicode"))?;
                if soldermask_color.replace(value).is_some() {
                    return Err(CliError::new(
                        "--soldermask-color may be supplied only once",
                    ));
                }
            }
            Some("--theme") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--theme requires a named Toon palette"))?;
                let value = match value.to_str() {
                    Some("saved") => ToonTheme::Saved,
                    Some("white") => ToonTheme::White,
                    Some("black") => ToonTheme::Black,
                    Some("blue") => ToonTheme::Blue,
                    Some("red") => ToonTheme::Red,
                    Some("purple") => ToonTheme::Purple,
                    Some("yellow") => ToonTheme::Yellow,
                    Some("green") => ToonTheme::Green,
                    Some(value) => {
                        return Err(CliError::new(format!(
                            "invalid --theme value {value:?}; expected saved, white, black, blue, red, purple, yellow, or green"
                        )));
                    }
                    None => return Err(CliError::new("--theme value is not valid Unicode")),
                };
                if theme.replace(value).is_some() {
                    return Err(CliError::new("--theme may be supplied only once"));
                }
            }
            Some("--footprint") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--footprint requires a reference"))?
                    .into_string()
                    .map_err(|_| CliError::new("--footprint value is not valid Unicode"))?;
                if value.is_empty() {
                    return Err(CliError::new("--footprint reference must not be empty"));
                }
                if footprint.replace(value).is_some() {
                    return Err(CliError::new("--footprint may be supplied only once"));
                }
            }
            Some("--variant") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--variant requires a name"))?
                    .into_string()
                    .map_err(|_| CliError::new("--variant value is not valid Unicode"))?;
                if value.is_empty() {
                    return Err(CliError::new("--variant name must not be empty"));
                }
                if variant.replace(value).is_some() {
                    return Err(CliError::new("--variant may be supplied only once"));
                }
            }
            Some("--all-variants") => {
                if all_variants {
                    return Err(CliError::new("--all-variants may be supplied only once"));
                }
                all_variants = true;
            }
            Some("--config") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--config requires a path"))?;
                if config.replace(PathBuf::from(value)).is_some() {
                    return Err(CliError::new("--config may be supplied only once"));
                }
            }
            Some("--write-config") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--write-config requires a path"))?;
                if write_config.replace(PathBuf::from(value)).is_some() {
                    return Err(CliError::new("--write-config may be supplied only once"));
                }
            }
            Some("--timings") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--timings requires a JSON output path"))?;
                if timings.replace(PathBuf::from(value)).is_some() {
                    return Err(CliError::new("--timings may be supplied only once"));
                }
            }
            Some("--workers") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--workers requires an integer from 1 to 64"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| CliError::new("--workers value is not valid Unicode"))?
                    .parse::<usize>()
                    .map_err(|_| CliError::new("--workers requires an integer from 1 to 64"))?;
                if !(1..=64).contains(&value) {
                    return Err(CliError::new("--workers requires an integer from 1 to 64"));
                }
                if workers.replace(value).is_some() {
                    return Err(CliError::new("--workers may be supplied only once"));
                }
            }
            Some("--cache-dir") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--cache-dir requires a directory"))?;
                if cache_dir.replace(PathBuf::from(value)).is_some() {
                    return Err(CliError::new("--cache-dir may be supplied only once"));
                }
            }
            Some("--no-cache") => {
                if no_cache {
                    return Err(CliError::new("--no-cache may be supplied only once"));
                }
                no_cache = true;
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
    if no_cache && cache_dir.is_some() {
        return Err(CliError::new(
            "--cache-dir and --no-cache are mutually exclusive",
        ));
    }
    if theme.is_some() && soldermask_color.is_some() {
        return Err(CliError::new(
            "--theme and --soldermask-color are mutually exclusive",
        ));
    }
    if variant.is_some() && all_variants {
        return Err(CliError::new(
            "--variant and --all-variants are mutually exclusive",
        ));
    }
    Ok(Invocation::Toon(ToonOptions {
        input,
        output,
        pcbdoc,
        sides,
        assembly,
        footprint,
        variant,
        all_variants,
        theme,
        soldermask_color,
        config,
        write_config,
        workers: workers.unwrap_or(4),
        cache_dir,
        no_cache,
        timings,
    }))
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

/// Execute the native CLI over an explicit argument sequence.
pub fn run_cli(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let args = args.into_iter().collect::<Vec<_>>();
    #[cfg(feature = "embedded-geometer")]
    if args == [OsStr::new("serve"), OsStr::new("--stdio")] {
        return u8::try_from(geometer_client::serve_stdio())
            .map_or(ExitCode::FAILURE, ExitCode::from);
    }
    let invocation = match parse_args(args) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("{TOP_LEVEL_HELP}kicad-cruncher: error: {error}");
            return ExitCode::from(2);
        }
    };

    match invocation {
        Invocation::TopLevelHelp => print!("{}\n\n{TOP_LEVEL_HELP}", version_text()),
        Invocation::Version => println!("{}", version_text()),
        Invocation::DesignHelp => print!("{}\n\n{DESIGN_HELP}", version_text()),
        Invocation::ToonHelp => print!("{}\n\n{TOON_HELP}", version_text()),
        Invocation::Design(options) => match design_bundle::run_design(&options) {
            Ok(bundle) => println!(
                "Design review: {} components, {} nets, {} schematic SVGs, {} PCB SVGs -> {}",
                bundle.component_count,
                bundle.net_count,
                bundle.schematic_svg_count,
                bundle.pcb_svg_count,
                bundle.output_dir.display()
            ),
            Err(error) => {
                eprintln!("kicad-cruncher: error: {error}");
                return ExitCode::FAILURE;
            }
        },
        Invocation::Toon(options) => match toon::run_toon(&options) {
            Ok(run) if run.wrote_config => println!(
                "Success: wrote Toon SVG config to {}",
                run.output_dir.display()
            ),
            Ok(run) => println!(
                "Toon: {} SVG(s), {} model instance(s), {} Geometer request(s), {} cache hit(s) ({} persistent), {} warning(s) -> {}",
                run.artifact_count,
                run.rendered_model_count,
                run.geometer_request_count,
                run.model_cache_hit_count,
                run.persistent_model_cache_hit_count,
                run.warning_count,
                run.output_dir.display()
            ),
            Err(error) => {
                eprintln!("kicad-cruncher: error: {error}");
                return ExitCode::FAILURE;
            }
        },
    }
    ExitCode::SUCCESS
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
    #[allow(
        clippy::cognitive_complexity,
        clippy::too_many_lines,
        reason = "the parser contract test covers successful options and all adjacent validation failures"
    )]
    fn toon_parses_side_and_transactional_output_directory() {
        assert_eq!(
            parse_args(os_args([
                "toon",
                "board.kicad_pcb",
                "--side",
                "bottom",
                "-o",
                "review",
            ]))
            .unwrap(),
            Invocation::Toon(ToonOptions {
                input: Some(PathBuf::from("board.kicad_pcb")),
                output: Some(PathBuf::from("review")),
                pcbdoc: None,
                sides: ToonSides::Bottom,
                assembly: false,
                footprint: None,
                variant: None,
                all_variants: false,
                theme: None,
                soldermask_color: None,
                config: None,
                write_config: None,
                workers: 4,
                cache_dir: None,
                no_cache: false,
                timings: None,
            })
        );
        assert_eq!(
            parse_args(os_args(["toon", "board.kicad_pcb"])).unwrap(),
            Invocation::Toon(ToonOptions {
                input: Some(PathBuf::from("board.kicad_pcb")),
                output: None,
                pcbdoc: None,
                sides: ToonSides::Both,
                assembly: false,
                footprint: None,
                variant: None,
                all_variants: false,
                theme: None,
                soldermask_color: None,
                config: None,
                write_config: None,
                workers: 4,
                cache_dir: None,
                no_cache: false,
                timings: None,
            })
        );
        assert_eq!(
            parse_args(os_args([
                "toon",
                "board.kicad_pcb",
                "--soldermask-color",
                "#000000",
            ]))
            .unwrap(),
            Invocation::Toon(ToonOptions {
                input: Some(PathBuf::from("board.kicad_pcb")),
                output: None,
                pcbdoc: None,
                sides: ToonSides::Both,
                assembly: false,
                footprint: None,
                variant: None,
                all_variants: false,
                theme: None,
                soldermask_color: Some("#000000".to_owned()),
                config: None,
                write_config: None,
                workers: 4,
                cache_dir: None,
                no_cache: false,
                timings: None,
            })
        );
        assert_eq!(
            parse_args(os_args([
                "toon",
                "board.kicad_pcb",
                "--timings",
                "profile.json",
            ]))
            .unwrap(),
            Invocation::Toon(ToonOptions {
                input: Some(PathBuf::from("board.kicad_pcb")),
                output: None,
                pcbdoc: None,
                sides: ToonSides::Both,
                assembly: false,
                footprint: None,
                variant: None,
                all_variants: false,
                theme: None,
                soldermask_color: None,
                config: None,
                write_config: None,
                workers: 4,
                cache_dir: None,
                no_cache: false,
                timings: Some(PathBuf::from("profile.json")),
            })
        );
        assert!(
            parse_args(os_args([
                "toon",
                "board.kicad_pcb",
                "--cache-dir",
                "cache",
                "--no-cache",
            ]))
            .is_err()
        );
        assert_eq!(
            parse_args(os_args(["toon", "--help"])).unwrap(),
            Invocation::ToonHelp
        );
        assert!(parse_args(os_args(["toon", "--side", "left"])).is_err());
        assert!(parse_args(os_args(["toon", "--workers", "0"])).is_err());
        let Invocation::Toon(config) = parse_args(os_args([
            "toon",
            "--config",
            "authored.jsonc",
            "--write-config",
            "resolved.config",
        ]))
        .unwrap() else {
            panic!("toon invocation");
        };
        assert_eq!(
            config.config.as_deref(),
            Some(std::path::Path::new("authored.jsonc"))
        );
        assert_eq!(
            config.write_config.as_deref(),
            Some(std::path::Path::new("resolved.config"))
        );
        let Invocation::Toon(footprint) =
            parse_args(os_args(["toon", "board.kicad_pcb", "--footprint", "U1"])).unwrap()
        else {
            panic!("toon invocation");
        };
        assert_eq!(footprint.footprint.as_deref(), Some("U1"));
        let Invocation::Toon(assembly) =
            parse_args(os_args(["toon", "board.kicad_pcb", "--assembly"])).unwrap()
        else {
            panic!("toon invocation");
        };
        assert!(assembly.assembly);
        assert!(parse_args(os_args(["toon", "--assembly", "--assembly"])).is_err());
        let Invocation::Toon(variant) = parse_args(os_args([
            "toon",
            "project.kicad_pro",
            "--doc",
            "mainboard",
            "--variant",
            "production",
        ]))
        .unwrap() else {
            panic!("toon invocation");
        };
        assert_eq!(variant.pcbdoc.as_deref(), Some("mainboard"));
        assert_eq!(variant.variant.as_deref(), Some("production"));
        assert!(!variant.all_variants);
        assert!(
            parse_args(os_args([
                "toon",
                "project.kicad_pro",
                "--variant",
                "production",
                "--all-variants",
            ]))
            .is_err()
        );
    }

    #[test]
    fn toon_themes_match_the_altium_palette_and_exclude_custom_mask_color() {
        let expected = [
            (ToonTheme::Saved, "saved", "auto", "#F5F5F5"),
            (ToonTheme::White, "white", "#EEEEEE", "#000000"),
            (ToonTheme::Black, "black", "#000000", "#F5F5F5"),
            (ToonTheme::Blue, "blue", "#1D4F91", "#F5F5F5"),
            (ToonTheme::Red, "red", "#A62A2A", "#F5F5F5"),
            (ToonTheme::Purple, "purple", "#6F3C8A", "#F5F5F5"),
            (ToonTheme::Yellow, "yellow", "#D6A600", "#F5F5F5"),
            (ToonTheme::Green, "green", "#176B3A", "#F5F5F5"),
        ];
        for (theme, name, mask, silk) in expected {
            assert_eq!(theme.name(), name);
            assert_eq!(theme.colors(), (mask, silk));
            let Invocation::Toon(options) =
                parse_args(os_args(["toon", "board.kicad_pcb", "--theme", name])).unwrap()
            else {
                panic!("toon invocation");
            };
            assert_eq!(options.theme, Some(theme));
            assert_eq!(options.soldermask_color, None);
        }

        let error = parse_args(os_args([
            "toon",
            "board.kicad_pcb",
            "--theme",
            "black",
            "--soldermask-color",
            "#000000",
        ]))
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "--theme and --soldermask-color are mutually exclusive"
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
