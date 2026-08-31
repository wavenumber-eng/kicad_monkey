use kicad_monkey_core::{Lexer, Selector, build, lex, parse, scan_form_spans};
use serde::Serialize;
use std::collections::BTreeSet;
use std::hint::black_box;
use std::time::Instant;

const ITEMS: usize = 20_000;
const ROUNDS: usize = 5;

fn fixture() -> String {
    let mut source = String::with_capacity(4 * 1024 * 1024);
    source.push_str("(kicad_pcb\n");
    for index in 0..ITEMS {
        source.push_str("  (footprint \"Bench:R_0805\" (property \"Reference\" \"R");
        source.push_str(&index.to_string());
        source.push_str("\") (at 1.25 2.5 90) (pad \"1\" smd rect (at 0 0)))\n");
    }
    source.push_str(")\n");
    source
}

fn measure(mut operation: impl FnMut()) -> Vec<f64> {
    operation();
    (0..ROUNDS)
        .map(|_| {
            let started = Instant::now();
            operation();
            started.elapsed().as_secs_f64()
        })
        .collect()
}

fn median(values: &[f64]) -> f64 {
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    ordered[ordered.len() / 2]
}

fn best(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::INFINITY, f64::min)
}

fn mib_per_second(bytes: usize, elapsed_seconds: f64) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0)) / elapsed_seconds
}

fn drain_lexer(source: &str) -> Result<(usize, usize), kicad_monkey_core::Error> {
    let mut count = 0_usize;
    let mut checksum = 0_usize;
    for token in Lexer::new(source) {
        let token = token?;
        count += 1;
        checksum = checksum
            .wrapping_mul(16_777_619)
            .wrapping_add(token.position.offset)
            .wrapping_add(token.lexeme.len());
    }
    Ok((count, checksum))
}

#[derive(Serialize)]
struct Measurement {
    schema: &'static str,
    fixture: &'static str,
    input_bytes: usize,
    token_count: usize,
    token_checksum: usize,
    selected_forms: usize,
    output_bytes: usize,
    rounds: usize,
    lex_drain_raw_seconds: Vec<f64>,
    lex_collect_raw_seconds: Vec<f64>,
    scan_raw_seconds: Vec<f64>,
    parse_raw_seconds: Vec<f64>,
    build_raw_seconds: Vec<f64>,
    lex_drain_seconds: f64,
    lex_collect_seconds: f64,
    scan_seconds: f64,
    parse_seconds: f64,
    build_seconds: f64,
    lex_drain_best_seconds: f64,
    lex_collect_best_seconds: f64,
    scan_best_seconds: f64,
    parse_best_seconds: f64,
    build_best_seconds: f64,
    lex_drain_mib_s: f64,
    lex_collect_mib_s: f64,
    scan_mib_s: f64,
    parse_mib_s: f64,
    build_mib_s: f64,
}

fn main() {
    let source = fixture();
    let selector = Selector {
        heads: Some(BTreeSet::from(["footprint".to_owned()])),
        ..Selector::default()
    };
    let (token_count, token_checksum) = drain_lexer(&source).expect("fixture should lex");
    let spans = scan_form_spans(&source, &selector).expect("fixture should scan");
    let tree = parse(&source).expect("fixture should parse");
    let built = build(&tree).expect("fixture tree should build");

    let lex_drain_raw_seconds = measure(|| {
        black_box(drain_lexer(black_box(&source)).expect("benchmark lexer drain"));
    });
    let lex_collect_raw_seconds = measure(|| {
        black_box(lex(black_box(&source)).expect("benchmark public lex"));
    });
    let scan_raw_seconds = measure(|| {
        black_box(
            scan_form_spans(black_box(&source), black_box(&selector)).expect("benchmark scan"),
        );
    });
    let parse_raw_seconds = measure(|| {
        black_box(parse(black_box(&source)).expect("benchmark parse"));
    });
    let build_raw_seconds = measure(|| {
        black_box(build(black_box(&tree)).expect("benchmark build"));
    });

    let lex_drain_seconds = median(&lex_drain_raw_seconds);
    let lex_collect_seconds = median(&lex_collect_raw_seconds);
    let scan_seconds = median(&scan_raw_seconds);
    let parse_seconds = median(&parse_raw_seconds);
    let build_seconds = median(&build_raw_seconds);
    let measurement = Measurement {
        schema: "kicad_monkey.sexpr_benchmark.a1",
        fixture: "synthetic_pcb_20000",
        input_bytes: source.len(),
        token_count,
        token_checksum,
        selected_forms: spans.len(),
        output_bytes: built.len(),
        rounds: ROUNDS,
        lex_drain_best_seconds: best(&lex_drain_raw_seconds),
        lex_collect_best_seconds: best(&lex_collect_raw_seconds),
        scan_best_seconds: best(&scan_raw_seconds),
        parse_best_seconds: best(&parse_raw_seconds),
        build_best_seconds: best(&build_raw_seconds),
        lex_drain_mib_s: mib_per_second(source.len(), lex_drain_seconds),
        lex_collect_mib_s: mib_per_second(source.len(), lex_collect_seconds),
        scan_mib_s: mib_per_second(source.len(), scan_seconds),
        parse_mib_s: mib_per_second(source.len(), parse_seconds),
        build_mib_s: mib_per_second(built.len(), build_seconds),
        lex_drain_raw_seconds,
        lex_collect_raw_seconds,
        scan_raw_seconds,
        parse_raw_seconds,
        build_raw_seconds,
        lex_drain_seconds,
        lex_collect_seconds,
        scan_seconds,
        parse_seconds,
        build_seconds,
    };
    println!(
        "{}",
        serde_json::to_string(&measurement).expect("measurement should serialize")
    );
}
