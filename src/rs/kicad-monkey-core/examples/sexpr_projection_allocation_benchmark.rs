//! Single-invocation allocation evidence for sparse projection scanners.

use kicad_monkey_core::{
    ProjectionLimits, Selector, scan_form_spans_with_limits, scan_reader_form_spans,
};
use serde::Serialize;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};
use std::alloc::System;
use std::collections::BTreeSet;
use std::hint::black_box;
use std::io::Cursor;

#[path = "support/sexpr_benchmark_fixture.rs"]
mod benchmark_fixture;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[derive(Clone, Copy, Serialize)]
struct AllocationCounts {
    allocation_calls: usize,
    deallocation_calls: usize,
    reallocation_calls: usize,
    allocated_bytes: usize,
    deallocated_bytes: usize,
    reallocated_bytes: isize,
}

#[derive(Serialize)]
struct Measurement {
    schema: &'static str,
    scanner: &'static str,
    fixture: &'static str,
    input_bytes: usize,
    visited_forms: usize,
    selected_forms: usize,
    allocation: AllocationCounts,
    control: AllocationCounts,
}

fn counts(stats: Stats) -> AllocationCounts {
    AllocationCounts {
        allocation_calls: stats.allocations,
        deallocation_calls: stats.deallocations,
        reallocation_calls: stats.reallocations,
        allocated_bytes: stats.bytes_allocated,
        deallocated_bytes: stats.bytes_deallocated,
        reallocated_bytes: stats.bytes_reallocated,
    }
}

fn measure<T>(operation: impl FnOnce() -> T) -> (T, AllocationCounts) {
    let region = Region::new(GLOBAL);
    let result = operation();
    (result, counts(region.change()))
}

fn validate_control() -> AllocationCounts {
    let (control, counts) = measure(|| Box::new([0_u8; 4096]));
    black_box(&control);
    assert_eq!(counts.allocation_calls, 1, "known allocation control");
    assert_eq!(counts.reallocation_calls, 0, "known allocation control");
    assert!(counts.allocated_bytes >= 4096, "known allocation control");
    counts
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scanner = std::env::args()
        .nth(1)
        .ok_or("usage: sexpr_projection_allocation_benchmark <memory|stream>")?;
    if scanner != "memory" && scanner != "stream" {
        return Err(format!("unsupported scanner: {scanner}").into());
    }
    let (source, visited_forms, expected_selected) =
        benchmark_fixture::speedy_shaped_sparse_fixture();
    let selector = Selector {
        paths: Some(BTreeSet::from([vec![
            "kicad_sch".to_owned(),
            "symbol".to_owned(),
            "target".to_owned(),
        ]])),
        ..Selector::default()
    };
    let limits = ProjectionLimits::default();

    if scanner == "memory" {
        black_box(scan_form_spans_with_limits(&source, &selector, limits)?);
    } else {
        black_box(scan_reader_form_spans(
            Cursor::new(source.as_bytes()),
            &selector,
            limits,
        )?);
    }
    let control = validate_control();
    let (spans, allocation) = if scanner == "memory" {
        measure(|| scan_form_spans_with_limits(&source, &selector, limits))
    } else {
        measure(|| scan_reader_form_spans(Cursor::new(source.as_bytes()), &selector, limits))
    };
    let spans = spans?;
    assert_eq!(spans.len(), expected_selected);
    black_box(&spans);

    let measurement = Measurement {
        schema: "kicad_monkey.sexpr_projection_allocation_benchmark.a0",
        scanner: if scanner == "memory" {
            "memory"
        } else {
            "stream"
        },
        fixture: "speedy_shaped_sparse_50000",
        input_bytes: source.len(),
        visited_forms,
        selected_forms: spans.len(),
        allocation,
        control,
    };
    println!("{}", serde_json::to_string(&measurement)?);
    Ok(())
}
