use std::path::PathBuf;
use std::process::ExitCode;

use kicad_cruncher_cli::design::{build_structured_design_facts_with_options, load_design_sources};

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(input) = arguments.next().map(PathBuf::from) else {
        eprintln!(
            "usage: design_json_oracle <project.kicad_pro|schematic.kicad_sch> [--no-indexes]"
        );
        return ExitCode::from(2);
    };
    let include_indexes = match arguments.next().as_deref().and_then(|value| value.to_str()) {
        None => true,
        Some("--no-indexes") => false,
        Some(argument) => {
            eprintln!("unrecognized argument: {argument}");
            return ExitCode::from(2);
        }
    };
    if arguments.next().is_some() {
        eprintln!("too many arguments");
        return ExitCode::from(2);
    }
    let result = load_design_sources(&input)
        .and_then(|loaded| build_structured_design_facts_with_options(&loaded, include_indexes));
    match result {
        Ok(facts) => {
            if let Err(error) = serde_json::to_writer_pretty(std::io::stdout(), &facts.design_json)
            {
                eprintln!("could not write design JSON: {error}");
                return ExitCode::FAILURE;
            }
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not build structured design facts: {error}");
            ExitCode::FAILURE
        }
    }
}
