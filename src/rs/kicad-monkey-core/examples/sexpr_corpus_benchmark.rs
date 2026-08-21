//! Release-mode single-file corpus benchmark used by Rack.

use kicad_monkey_core::{build, parse_bytes};
use serde::Serialize;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const START_DELAY: Duration = Duration::from_millis(100);
const PEAK_OBSERVATION_DELAY: Duration = Duration::from_millis(25);

#[derive(Serialize)]
struct Measurement {
    schema: &'static str,
    operation: &'static str,
    input_bytes: usize,
    output_bytes: Option<usize>,
    read_ns: u128,
    parse_ns: u128,
    build_ns: Option<u128>,
    reparse_ns: Option<u128>,
    compare_ns: Option<u128>,
    second_build_ns: Option<u128>,
    total_operation_ns: u128,
}

fn measure<T>(operation: impl FnOnce() -> T) -> (T, u128) {
    let started = Instant::now();
    let result = operation();
    (result, started.elapsed().as_nanos())
}

fn arguments() -> Result<(String, PathBuf), String> {
    let mut arguments = std::env::args_os();
    let executable = arguments.next().unwrap_or_default();
    let Some(operation) = arguments.next() else {
        return Err(format!(
            "usage: {} <parse|roundtrip> <KiCad S-expression path>",
            PathBuf::from(executable).display()
        ));
    };
    let Some(path) = arguments.next() else {
        return Err(format!(
            "usage: {} <parse|roundtrip> <KiCad S-expression path>",
            PathBuf::from(executable).display()
        ));
    };
    if arguments.next().is_some() {
        return Err("expected exactly one input path".to_owned());
    }
    let operation = operation
        .into_string()
        .map_err(|_| "operation must be valid UTF-8")?;
    if operation != "parse" && operation != "roundtrip" {
        return Err(format!("unsupported operation: {operation}"));
    }
    Ok((operation, PathBuf::from(path)))
}

fn run() -> Result<Measurement, Box<dyn std::error::Error>> {
    let (operation, path) = arguments().map_err(std::io::Error::other)?;

    // Give the Rack parent enough time to attach its peak-memory sampler. The
    // delay is deliberately outside every reported operation duration.
    std::thread::sleep(START_DELAY);

    let total_started = Instant::now();
    let (source, read_ns) = measure(|| std::fs::read(&path));
    let source = source?;
    let input_bytes = source.len();

    let (parsed, parse_ns) = measure(|| parse_bytes(black_box(&source)));
    let parsed = parsed?;
    if operation == "parse" {
        black_box(&parsed);
        let total_operation_ns = total_started.elapsed().as_nanos();
        std::thread::sleep(PEAK_OBSERVATION_DELAY);
        return Ok(Measurement {
            schema: "kicad_monkey.sexpr_corpus_benchmark.a0",
            operation: "parse",
            input_bytes,
            output_bytes: None,
            read_ns,
            parse_ns,
            build_ns: None,
            reparse_ns: None,
            compare_ns: None,
            second_build_ns: None,
            total_operation_ns,
        });
    }

    let (built, build_ns) = measure(|| build(black_box(&parsed)));
    let built = built?;
    let output_bytes = built.len();

    let (reparsed, reparse_ns) = measure(|| parse_bytes(black_box(built.as_bytes())));
    let reparsed = reparsed?;
    let (trees_equal, compare_ns) = measure(|| black_box(&parsed) == black_box(&reparsed));
    if !trees_equal {
        return Err("reparsed tree differs from the original".into());
    }
    let (rebuilt, second_build_ns) = measure(|| build(black_box(&reparsed)));
    let rebuilt = rebuilt?;
    if rebuilt != built {
        return Err("second deterministic build differs from the first".into());
    }
    black_box((&source, &parsed, &built, &reparsed, &rebuilt));
    let total_operation_ns = total_started.elapsed().as_nanos();
    std::thread::sleep(PEAK_OBSERVATION_DELAY);

    Ok(Measurement {
        schema: "kicad_monkey.sexpr_corpus_benchmark.a0",
        operation: "roundtrip",
        input_bytes,
        output_bytes: Some(output_bytes),
        read_ns,
        parse_ns,
        build_ns: Some(build_ns),
        reparse_ns: Some(reparse_ns),
        compare_ns: Some(compare_ns),
        second_build_ns: Some(second_build_ns),
        total_operation_ns,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string(&run()?)?);
    Ok(())
}
