use std::process::ExitCode;

use kicad_cruncher_cli::{DESIGN_HELP, Invocation, TOP_LEVEL_HELP, parse_args, version_text};

fn main() -> ExitCode {
    let invocation = match parse_args(std::env::args_os().skip(1)) {
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
        Invocation::Design(_) => {
            eprintln!(
                "error: the Rust design workflow is still under migration and is not yet canonical"
            );
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
