use std::process::ExitCode;

fn main() -> ExitCode {
    kicad_cruncher_cli::run_cli(std::env::args_os().skip(1))
}
