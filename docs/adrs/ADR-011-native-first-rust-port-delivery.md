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

The native application release target is specifically Windows x64. Its exit gate binds a
single tested Monkey platform wheel and universal Cruncher distribution set to
the source commit and requires release publication to consume those exact
artifacts. Linux and macOS retain the Python provider path until separately
measured and promoted; this Windows hard switch is not an all-platform or
all-Rust Cruncher claim.

The existing WASM adapters remain maintained feasibility evidence. They prove
byte-oriented requests, generated diagnostics, independent feature families,
resource limits, and take-once large output ownership. New PCB, schematic,
graph, plotter, or text WASM exports do not gate native parity.

Core crates remain transport-neutral. WASM wrappers must be thin and
operation-shaped, and may be owned by `kicad_monkey` or by the downstream
consumer that needs the composition. Not every native capability requires a
WASM export.

`pcb_a0`, Viz, ALX, and the KiCad-to-`pcb_a0` adapter are excluded from this
decision's Definition of Done. A separate downstream task may begin
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

## Phase 7 follow-on decision

Accepted on 2026-08-20, this follow-on supersedes the earlier statement that a
literal Rust executable was outside the bounded application delivery. The
first real application consumer is now proven: the complete design-review
bundle. Windows x64 releases therefore include a hash-bound platform archive
containing pure-Rust `kicad-cruncher` and `kcr` executables for `design`,
`design-review`, `dr`, and version reporting.

The installed smoke builds through `cargo install --locked`, rejects binaries
that embed the source workspace path, removes Python from the runtime
environment, and publishes a real review bundle from copied public KiCad
sources. Both executable names share one Rust dispatch implementation. The
tested archive and its SHA-256 manifest are attached to the Cruncher GitHub
release; release publication consumes the candidate produced by the same
Windows gate.

This is not an all-command or all-platform Rust claim. The universal Cruncher
wheel remains the cross-platform distribution for commands that have not yet
received Rust vertical slices and retains its normal public `kicad-monkey`
dependency. When the native archive is first on `PATH`, legacy commands remain
available explicitly through `python -m kicad_cruncher`. Public publication of
the internal Monkey Rust crates remains deferred.
