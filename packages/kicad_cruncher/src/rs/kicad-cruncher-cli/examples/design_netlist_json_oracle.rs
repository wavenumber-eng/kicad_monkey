use std::path::PathBuf;
use std::process::ExitCode;

use kicad_cruncher_cli::design::{build_structured_design_facts, load_design_sources};

fn main() -> ExitCode {
    let Some(input) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: design_netlist_json_oracle <project.kicad_pro|schematic.kicad_sch>");
        return ExitCode::from(2);
    };
    let result =
        load_design_sources(&input).and_then(|loaded| build_structured_design_facts(&loaded));
    match result {
        Ok(facts) => {
            if let Err(error) = serde_json::to_writer_pretty(std::io::stdout(), &facts.netlist_json)
            {
                eprintln!("could not write netlist JSON: {error}");
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
