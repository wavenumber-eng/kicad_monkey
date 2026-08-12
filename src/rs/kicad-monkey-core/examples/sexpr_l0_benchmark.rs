use kicad_monkey_core::{Selector, build, parse, scan_form_spans};
use std::collections::BTreeSet;
use std::hint::black_box;
use std::time::{Duration, Instant};

const ITEMS: usize = 20_000;
const ROUNDS: u32 = 5;

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

fn best_of(mut operation: impl FnMut(), rounds: u32) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..rounds {
        let started = Instant::now();
        operation();
        best = best.min(started.elapsed());
    }
    best
}

fn mib_per_second(bytes: usize, elapsed: Duration) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
}

fn main() {
    let source = fixture();
    let selector = Selector {
        heads: Some(BTreeSet::from(["footprint".to_owned()])),
        ..Selector::default()
    };
    let spans = scan_form_spans(&source, &selector).expect("benchmark fixture should scan");
    let tree = parse(&source).expect("benchmark fixture should parse");
    let built = build(&tree).expect("benchmark tree should build");

    let scan = best_of(
        || {
            black_box(
                scan_form_spans(black_box(&source), black_box(&selector)).expect("benchmark scan"),
            );
        },
        ROUNDS,
    );
    let full_parse = best_of(
        || {
            black_box(parse(black_box(&source)).expect("benchmark parse"));
        },
        ROUNDS,
    );
    let serialization = best_of(
        || {
            black_box(build(black_box(&tree)).expect("benchmark build"));
        },
        ROUNDS,
    );

    println!(
        concat!(
            "{{\"schema\":\"kicad_monkey.sexpr_benchmark.a0\",",
            "\"fixture\":\"synthetic_pcb_20000\",",
            "\"input_bytes\":{},\"selected_forms\":{},\"output_bytes\":{},",
            "\"rounds\":{},\"scan_seconds\":{:.9},\"parse_seconds\":{:.9},",
            "\"build_seconds\":{:.9},\"scan_mib_s\":{:.3},",
            "\"parse_mib_s\":{:.3},\"build_mib_s\":{:.3}}}"
        ),
        source.len(),
        spans.len(),
        built.len(),
        ROUNDS,
        scan.as_secs_f64(),
        full_parse.as_secs_f64(),
        serialization.as_secs_f64(),
        mib_per_second(source.len(), scan),
        mib_per_second(source.len(), full_parse),
        mib_per_second(built.len(), serialization),
    );
}
