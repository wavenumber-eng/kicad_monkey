use std::process::ExitCode;

use kicad_cruncher_cli::design_bundle::run_design;
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
        Invocation::Design(options) => match run_design(&options) {
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
    }
    ExitCode::SUCCESS
}
