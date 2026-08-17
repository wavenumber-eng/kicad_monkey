//! Test-only corpus runner used by the package-local Rack gate.
//!
//! Absolute corpus paths are supplied one per line on stdin. Keeping corpus
//! discovery in Rack/Python preserves the package's canonical archive and
//! `KM_CORPUS` resolution rules without adding archive or CLI dependencies
//! to the production parser crate.

use kicad_monkey_core::{ErrorPhase, build, parse_bytes};
use serde::Serialize;
use std::io::{self, BufRead};
use std::path::Path;
use std::time::Instant;

#[derive(Serialize)]
struct CorpusRecord {
    schema: &'static str,
    path: String,
    phase: &'static str,
    error: Option<String>,
    input_bytes: usize,
    output_bytes: usize,
    elapsed_ns: u128,
}

impl CorpusRecord {
    fn failure(
        path: &Path,
        phase: &'static str,
        error: impl Into<String>,
        input_bytes: usize,
        started: Instant,
    ) -> Self {
        Self {
            schema: "kicad_monkey.sexpr_corpus_record.a0",
            path: path.display().to_string(),
            phase,
            error: Some(error.into()),
            input_bytes,
            output_bytes: 0,
            elapsed_ns: started.elapsed().as_nanos(),
        }
    }
}

fn initial_parse_phase(phase: ErrorPhase) -> &'static str {
    match phase {
        ErrorPhase::Lex => "lex",
        ErrorPhase::Tree => "tree",
        ErrorPhase::Build => "build",
    }
}

fn process(path: &Path) -> CorpusRecord {
    let started = Instant::now();
    let source = match std::fs::read(path) {
        Ok(source) => source,
        Err(error) => return CorpusRecord::failure(path, "read", error.to_string(), 0, started),
    };
    let input_bytes = source.len();

    let parsed = match parse_bytes(&source) {
        Ok(parsed) => parsed,
        Err(error) => {
            return CorpusRecord::failure(
                path,
                initial_parse_phase(error.phase),
                error.to_string(),
                input_bytes,
                started,
            );
        }
    };
    let built = match build(&parsed) {
        Ok(built) => built,
        Err(error) => {
            return CorpusRecord::failure(path, "build", error.to_string(), input_bytes, started);
        }
    };
    let reparsed = match parse_bytes(built.as_bytes()) {
        Ok(reparsed) => reparsed,
        Err(error) => {
            return CorpusRecord::failure(path, "reparse", error.to_string(), input_bytes, started);
        }
    };
    if reparsed != parsed {
        return CorpusRecord::failure(
            path,
            "compare",
            "reparsed tree differs from the first parse",
            input_bytes,
            started,
        );
    }
    let rebuilt = match build(&reparsed) {
        Ok(rebuilt) => rebuilt,
        Err(error) => {
            return CorpusRecord::failure(
                path,
                "compare",
                format!("second build failed: {error}"),
                input_bytes,
                started,
            );
        }
    };
    if rebuilt != built {
        return CorpusRecord::failure(
            path,
            "compare",
            "second deterministic output differs from the first",
            input_bytes,
            started,
        );
    }

    CorpusRecord {
        schema: "kicad_monkey.sexpr_corpus_record.a0",
        path: path.display().to_string(),
        phase: "ok",
        error: None,
        input_bytes,
        output_bytes: built.len(),
        elapsed_ns: started.elapsed().as_nanos(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for line in io::stdin().lock().lines() {
        let path_text = line?;
        if path_text.is_empty() {
            continue;
        }
        let record = process(Path::new(&path_text));
        println!("{}", serde_json::to_string(&record)?);
    }
    Ok(())
}
