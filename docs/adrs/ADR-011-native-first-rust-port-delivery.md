# ADR-011: Native-First Rust Port Delivery

## Status

Accepted

## Date

2026-08-13

## Context

The Rust port established native and executable WASM proofs early so parser,
generated-contract, resource-limit, feature-topology, and large-output
ownership decisions could be tested before the source model expanded. That
feasibility work is complete.

Continuing to expand native and WASM surfaces together would put packaging work
on the critical path before the complete parser, writer, object models,
plotters, text pipeline, schematic compiler, and native application workflows
exist. The concrete browser operation set is also not yet known. Downstream
projects may need different operation combinations and can package the shared
Rust core themselves.

The `pcb_a0` contract and KiCad-to-generic adapter are owned by a downstream
model package. Their implementation and Viz/ALX adoption depend on a trusted
KiCad-native surface, but they are not part of the `kicad_monkey` Rust port.

## Decision

Deliver the port native-first in this order:

1. complete the high-speed parser, selective reads, semantic writeback, and
   KiCad-native typed reader/writer object models;
2. close native plotter, text, netlist, and compiled-schematic-graph parity;
3. stand up native Windows `kicad-cruncher` directly over the Rust crates;
4. pass the existing in-scope Cargo, Python-oracle, KiCad-oracle, and Rack
   tests, with `design` / `design-review` / `dr` as the primary application
   acceptance workflow;
5. only then select concrete browser operations and package thin,
   operation-specific WASM artifacts.

For the bounded native application delivery, step 3 means that the installed Windows
Cruncher entry points directly select the packaged Rust operations for every
promoted physical, graph, and version-E netlist result, with no Python retry.
The existing Python facade may continue to own CLI parsing, application
orchestration, presentation, and transactional artifact publication. This does
not assert that the universal Cruncher wheel contains a separately compiled
Rust `kicad-cruncher` executable or that every Cruncher command has been
rewritten in Rust. Requiring that literal executable boundary would be a new
application-ownership decision with its own contracts and packaging evidence.

The existing WASM adapters remain maintained feasibility evidence. They prove
byte-oriented requests, generated diagnostics, independent feature families,
resource limits, and take-once large output ownership. New PCB, schematic,
graph, plotter, or text WASM exports do not gate native parity.

Core crates remain transport-neutral. WASM wrappers must be thin and
operation-shaped, and may be owned by `kicad_monkey` or by the downstream
consumer that needs the composition. Not every native capability requires a
WASM export.

`pcb_a0`, Viz, ALX, and the KiCad-to-`pcb_a0` adapter are excluded from this
plan's phases and Definition of Done. A separate downstream task may begin
after native test parity and consume documented KiCad-native crate features.

## Consequences

- Native Windows tools and the existing parity suite determine the critical
  path and can stand up without browser toolchain or packaging work.
- The browser API is selected from proven operations and real consumer needs
  instead of being frozen speculatively.
- Completed WASM work is retained rather than discarded, but further artifact
  size, transfer, browser, and worker evidence is required only for operations
  later selected for WASM.
- PyO3 acceleration is optional follow-on integration and does not precede the
  native CLI.
- Downstream adapters can compose their own native or WASM products without
  adding external model concepts or dependencies to `kicad_monkey`.
- Public Cargo publication remains a later explicit release decision after
  native parity and artifact disposition.
