#![forbid(unsafe_code)]

use kicad_monkey_native::{
    execute_request_a1_reader, execute_request_reader, execute_svg_request_reader, handshake,
    handshake_a1, handshake_a2, serialize_error,
};
use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [command] if command == "handshake" => write_success(&handshake()),
        [command] if command == "handshake-a1" => write_success(&handshake_a1()),
        [command] if command == "handshake-a2" => write_success(&handshake_a2()),
        [command] if command == "design-facts" => match execute_request_reader(std::io::stdin()) {
            Ok(output) => write_output(&output),
            Err(error) => write_error(&error),
        },
        [command] if command == "design-facts-a1" => {
            match execute_request_a1_reader(std::io::stdin()) {
                Ok(output) => write_output(&output),
                Err(error) => write_error(&error),
            }
        }
        [command] if command == "render-svg" => {
            match execute_svg_request_reader(std::io::stdin()) {
                Ok(output) => write_output(&output),
                Err(error) => write_error(&error),
            }
        }
        _ => write_error(&kicad_monkey_native::NativeError::new_for_cli(
            "expected exactly one command: handshake, handshake-a1, handshake-a2, design-facts, design-facts-a1, or render-svg",
        )),
    }
}

fn write_success(value: &impl serde::Serialize) -> ExitCode {
    match serde_json::to_vec(value) {
        Ok(output) => write_output(&output),
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "could not serialize handshake: {error}");
            ExitCode::FAILURE
        }
    }
}

fn write_output(output: &[u8]) -> ExitCode {
    if std::io::stdout().write_all(output).is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn write_error(error: &kicad_monkey_native::NativeError) -> ExitCode {
    let mut stderr = std::io::stderr();
    let _ = stderr.write_all(&serialize_error(error));
    let _ = writeln!(stderr);
    ExitCode::FAILURE
}
