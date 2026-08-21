//! Release measurement of select-all scan versus final source-order sorting.

use kicad_monkey_core::{
    ProjectionLimits, Selector, measure_form_span_sort, measure_reader_form_span_sort,
};
use serde::Serialize;
use std::fs::File;
use std::path::PathBuf;

#[derive(Serialize)]
struct Measurement {
    schema: &'static str,
    scanner: &'static str,
    input_bytes: u64,
    selected_forms: usize,
    scan_ns: u128,
    sort_ns: u128,
    sort_fraction: f64,
}

fn arguments() -> Result<(String, PathBuf), String> {
    let mut arguments = std::env::args_os();
    let executable = arguments.next().unwrap_or_default();
    let Some(scanner) = arguments.next() else {
        return Err(format!(
            "usage: {} <memory|stream> <KiCad S-expression path>",
            PathBuf::from(executable).display()
        ));
    };
    let Some(path) = arguments.next() else {
        return Err("missing KiCad S-expression path".to_owned());
    };
    if arguments.next().is_some() {
        return Err("expected exactly one input path".to_owned());
    }
    let scanner = scanner
        .into_string()
        .map_err(|_| "scanner must be valid UTF-8")?;
    if scanner != "memory" && scanner != "stream" {
        return Err(format!("unsupported scanner: {scanner}"));
    }
    Ok((scanner, PathBuf::from(path)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (scanner, path) = arguments().map_err(std::io::Error::other)?;
    let input_bytes = std::fs::metadata(&path)?.len();
    let selector = Selector::default();
    let limits = ProjectionLimits::default();
    let (spans, scan_ns, sort_ns) = if scanner == "memory" {
        let source = std::fs::read_to_string(&path)?;
        measure_form_span_sort(&source, &selector, limits)?
    } else {
        measure_reader_form_span_sort(File::open(&path)?, &selector, limits)?
    };
    let total_ns = scan_ns + sort_ns;
    let measurement = Measurement {
        schema: "kicad_monkey.sexpr_selection_sort_benchmark.a0",
        scanner: if scanner == "memory" {
            "memory"
        } else {
            "stream"
        },
        input_bytes,
        selected_forms: spans.len(),
        scan_ns,
        sort_ns,
        sort_fraction: sort_ns as f64 / total_ns as f64,
    };
    println!("{}", serde_json::to_string(&measurement)?);
    Ok(())
}
