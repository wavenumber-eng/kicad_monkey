+++
type = "plan"
id = "rust-port-strategy"
status = "active"
created = "2026-08-12"

[[steps]]
id = "rust-baseline-bootstrap"
title = "Establish the Rust workspace, safety, dependency, and Rack baseline"
status = "done"

[[steps]]
id = "typespec-contract-bootstrap"
title = "Establish TypeSpec authority and deterministic Rust contract generation"
status = "done"
depends_on = ["rust-baseline-bootstrap"]

[[steps]]
id = "parity-registry-bootstrap"
title = "Establish the queryable Rust/Python/WASM parity registry"
status = "done"
depends_on = ["rust-baseline-bootstrap"]

[[steps]]
id = "native-wasm-rack-bootstrap"
title = "Prove native and executable WASM byte operations under Rack"
status = "done"
depends_on = ["rust-baseline-bootstrap", "typespec-contract-bootstrap"]

[[steps]]
id = "bootstrap-architecture-review"
title = "Close the bootstrap architecture review"
status = "done"
depends_on = ["typespec-contract-bootstrap", "parity-registry-bootstrap", "native-wasm-rack-bootstrap"]

[[steps]]
id = "phase0-contract-registry-baseline"
title = "Close the Phase 0 contract, registry, and baseline milestone"
status = "done"
depends_on = ["bootstrap-architecture-review"]

[[steps]]
id = "l0-sexpr-foundation"
title = "Complete and review the Rust S-expression foundation"
status = "done"
depends_on = ["phase0-contract-registry-baseline"]

[[steps]]
id = "l1-corpus-parity"
title = "Close parser-only corpus parity under Rack"
status = "done"
depends_on = ["l0-sexpr-foundation"]

[[steps]]
id = "phase1-corpus-performance-memory"
title = "Measure named corpus performance and peak memory"
status = "done"
depends_on = ["l1-corpus-parity"]

[[steps]]
id = "phase1-selected-span-sort"
title = "Measure selected-span sorting on select-everything workloads"
status = "done"
depends_on = ["l1-corpus-parity"]

[[steps]]
id = "phase1-deep-path-ownership"
title = "Measure deep-path ownership and retained path memory"
status = "pending"
depends_on = ["l1-corpus-parity"]

[[steps]]
id = "phase1-many-path-selector"
title = "Measure many-path selector scaling"
status = "pending"
depends_on = ["l1-corpus-parity"]

[[steps]]
id = "phase1-streaming-select"
title = "Measure bounded-memory streaming selection"
status = "pending"
depends_on = ["l1-corpus-parity"]

[[steps]]
id = "phase1-source-patch"
title = "Measure source-preserving patch generation and application"
status = "pending"
depends_on = ["l1-corpus-parity"]

[[steps]]
id = "phase1-wasm-transfer"
title = "Defer browser/WASM transfer measurement until concrete operations are selected"
status = "pending"
depends_on = ["phase7-wasm-operation-selection"]

[[steps]]
id = "phase1-build-node-validator"
title = "Implement and test the build-node semantic validator"
status = "done"
depends_on = ["l1-corpus-parity", "typespec-contract-bootstrap"]

[[steps]]
id = "phase1-python-projection"
title = "Generate and verify the Python contract projection"
status = "done"
depends_on = ["l1-corpus-parity", "typespec-contract-bootstrap"]

[[steps]]
id = "phase1-typescript-projection"
title = "Generate and verify the TypeScript contract projection"
status = "done"
depends_on = ["l1-corpus-parity", "typespec-contract-bootstrap"]

[[steps]]
id = "parser-architecture-review"
title = "Close the parser architecture and Phase 1 promotion review"
status = "done"
depends_on = ["l1-corpus-parity", "phase1-build-node-validator", "phase1-python-projection", "phase1-typescript-projection"]

[[steps]]
id = "phase1-promotion"
title = "Close the Phase 1 parser promotion milestone"
status = "done"
depends_on = ["l1-corpus-parity", "phase1-build-node-validator", "phase1-python-projection", "phase1-typescript-projection", "parser-architecture-review"]

[[steps]]
id = "phase2-typespec-reader-ir-contracts"
title = "Generate the first typed footprint reader/writer contracts"
status = "done"
depends_on = ["phase1-promotion"]

[[steps]]
id = "phase2-typed-reader-writer-ir"
title = "Deliver the first typed footprint reader/writer proof"
status = "done"
depends_on = ["phase2-typespec-reader-ir-contracts"]

[[steps]]
id = "boundary-architecture-review"
title = "Close the typed native-core boundary review and disposition the completed WASM feasibility proof"
status = "done"
depends_on = ["phase2-typed-reader-writer-ir", "phase2-footprint-plotter-ir"]

[[steps]]
id = "phase3-pcb-reader-writer"
title = "Expand the KiCad-native PCB reader/writer object model"
status = "done"
depends_on = ["boundary-architecture-review"]

[[steps]]
id = "phase3-pcb-roundtrip-parity"
title = "Close PCB semantic round-trip, mutation, and iterable-view parity"
status = "done"
depends_on = ["phase3-pcb-reader-writer"]

[[steps]]
id = "phase4-compiled-graph-typespec"
title = "Generate compiled schematic graph contracts and identity vectors"
status = "done"
depends_on = ["phase3-pcb-roundtrip-parity"]

[[steps]]
id = "phase4-schematic-compiler"
title = "Deliver schematic compilation and the compiled graph"
status = "done"
depends_on = ["phase4-compiled-graph-typespec"]

[[steps]]
id = "font-bundle-contracts"
title = "Generate FontBundle, shaping-record, and outline-vector contracts"
status = "done"
depends_on = ["phase4-schematic-compiler"]

[[steps]]
id = "rustybuzz-shaping-parity"
title = "Close Rustybuzz shaping-record parity"
status = "done"
depends_on = ["font-bundle-contracts"]

[[steps]]
id = "outline-extraction-parity"
title = "Close glyph outline-extraction parity"
status = "done"
depends_on = ["rustybuzz-shaping-parity"]

[[steps]]
id = "render-cache-parity"
title = "Close final text render-cache parity"
status = "active"
depends_on = ["outline-extraction-parity"]

[[steps]]
id = "geometer-bridge-entry"
title = "Resolve the conditional Geometer bridge-entry gate by promotion or explicit deferral"
status = "pending"
depends_on = ["phase3-pcb-roundtrip-parity"]

[[steps]]
id = "phase5-plotter-ir"
title = "Close schematic and PCB plotter IR parity"
status = "pending"
depends_on = ["phase4-schematic-compiler", "render-cache-parity", "geometer-bridge-entry"]

[[steps]]
id = "dependency-license-review"
title = "Close dependency, license, maintenance, and feature-topology review"
status = "pending"
depends_on = ["phase5-plotter-ir"]

[[steps]]
id = "unsafe-code-audit"
title = "Prove package-owned unsafe code remains forbidden or approve bounded exceptions"
status = "pending"
depends_on = ["phase5-plotter-ir"]

[[steps]]
id = "phase6-native-cruncher-cli"
title = "Stand up the native Windows kicad-cruncher command surface"
status = "pending"
depends_on = ["phase5-plotter-ir"]

[[steps]]
id = "phase6-cruncher-parity"
title = "Close selected kicad-cruncher and design-review workflow parity"
status = "pending"
depends_on = ["phase6-native-cruncher-cli"]

[[steps]]
id = "platform-matrix"
title = "Pass native Windows core and kicad-cruncher platform gates"
status = "pending"
depends_on = ["repo-local-corpus-audit"]

[[steps]]
id = "native-parity-architecture-review"
title = "Close the native parser, writer, plotter, and CLI parity review"
status = "pending"
depends_on = ["dependency-license-review", "unsafe-code-audit", "platform-matrix", "native-parity-performance-review"]

[[steps]]
id = "parity-zero-gap"
title = "Close every required native, CLI, contract, and existing-test parity cell"
status = "pending"
depends_on = ["native-parity-architecture-review"]

[[steps]]
id = "phase7-wasm-operation-selection"
title = "Select concrete browser operations and their WASM package owners after native parity"
status = "pending"
depends_on = ["parity-zero-gap"]

[[steps]]
id = "phase7-wasm-packaging"
title = "Package and verify only selected WASM operations, or explicitly delegate or defer them"
status = "pending"
depends_on = ["phase7-wasm-operation-selection", "phase7-wasm-shared-plotter-dto", "phase7-wasm-symbol-read-limit", "phase7-wasm-take-output-coverage", "phase7-wasm-artifact-builds"]

[[steps]]
id = "phase7-python-integration-disposition"
title = "Disposition optional Python acceleration without blocking native parity"
status = "pending"
depends_on = ["parity-zero-gap"]

[[steps]]
id = "phase8-publication-closeout"
title = "Disposition the long tail and prepare intentional Cargo publication"
status = "pending"
depends_on = ["parity-zero-gap", "phase7-wasm-packaging", "phase7-python-integration-disposition"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit design docs, ADRs, requirements, and contracts against implementation"
status = "pending"
depends_on = ["phase8-publication-closeout"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit promoted tests and runtime impact"
status = "pending"
depends_on = ["phase8-publication-closeout"]

[[steps]]
id = "external-review"
title = "Complete the final independent external review"
status = "pending"
depends_on = ["design-doc-intent-audit", "test-runtime-impact-audit"]

[[exit_criteria]]
id = "signoff"
title = "All required Rack, Cargo, TypeSpec generation-clean, native CLI, and parity commands pass"
status = "pending"

[[exit_criteria]]
id = "surface-disposition"
title = "Every promoted surface has an explicit native Rust, CLI, test, and support disposition; WASM is classified only for selected browser operations"
status = "pending"

[[exit_criteria]]
id = "standards-and-reviews"
title = "The pinned wn-dev-std profile passes and bootstrap, parser, boundary, and native-parity findings are closed or dispositioned"
status = "pending"

[[exit_criteria]]
id = "typespec-authority"
title = "Every promoted interchange surface is TypeSpec-owned with clean Rust, Python, TypeScript, and schema projections"
status = "pending"

[[exit_criteria]]
id = "parity-zero-gap"
title = "The generated registry reports zero unclassified or uncovered required parity gaps"
status = "pending"

[[exit_criteria]]
id = "semantic-writeback"
title = "Every promoted source family passes semantic write/reparse, stable second write, and applicable KiCad write oracles"
status = "pending"

[[exit_criteria]]
id = "platform-matrix"
title = "The native Windows core and kicad-cruncher lanes pass; additional claimed native platforms have explicit evidence"
status = "pending"

[[exit_criteria]]
id = "performance-budgets"
title = "Representative native cases show no material regression and meet any ratified release budgets"
status = "pending"

[[exit_criteria]]
id = "native-cruncher-parity"
title = "The native Windows kicad-cruncher passes the selected command matrix, including design-review bundle parity"
status = "pending"

[[exit_criteria]]
id = "wasm-disposition"
title = "Selected browser operations have thin tested WASM packages, or are explicitly delegated or deferred without blocking native parity"
status = "pending"

[[exit_criteria]]
id = "text-pipeline"
title = "Promoted outline text passes FontBundle validation, shaping, outline, and render-cache parity"
status = "pending"

[[exit_criteria]]
id = "geometer-disposition"
title = "Each promoted Geometer operation has an owner, generated upstream contract, and native operational evidence; browser evidence is required only when that operation is selected for WASM"
status = "pending"

[[exit_criteria]]
id = "dependency-and-safety"
title = "Dependency/license review passes and package-owned unsafe remains forbidden or has reviewed bounded exceptions"
status = "pending"

[[exit_criteria]]
id = "packaging-publication"
title = "Intentional native crates and any selected WASM artifacts document version/license/support policy and public Cargo crates pass packaged builds plus publish dry-run"
status = "pending"

[[exit_criteria]]
id = "python-continuity"
title = "Python packaging remains usable for intentionally Python-only capabilities"
status = "pending"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Accepted implementation matches ADRs, design docs, requirements, contracts, and release notes"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "Every promoted test and its runtime impact is recorded and reviewed"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "Independent final review is complete with all blockers closed or explicitly dispositioned"
status = "pending"

[[steps]]
id = "dev-std-plan-conversion"
title = "Convert the complete legacy strategy to the compliant wn-dev-std plan format"
status = "done"
depends_on = ["phase0-contract-registry-baseline"]

[[steps]]
id = "phase1-performance-threshold-ratification"
title = "Reproduce corpus evidence on Linux and ratify parser performance/memory budgets"
status = "pending"
depends_on = ["phase1-corpus-performance-memory"]

[[steps]]
id = "advisory-benchmark-policy"
title = "Keep heavy Rust performance evidence opt-in under strict Rack"
status = "done"
depends_on = ["phase1-selected-span-sort"]

[[steps]]
id = "native-parity-performance-review"
title = "Review representative native performance evidence without blocking stand-up on deferred micro-optimization"
status = "pending"
depends_on = ["phase1-corpus-performance-memory", "phase1-selected-span-sort", "advisory-benchmark-policy"]

[[steps]]
id = "phase2-plotter-ir-contracts"
title = "Generate the first footprint and symbol plotter IR contracts"
status = "done"
depends_on = ["phase2-footprint-corrections"]

[[steps]]
id = "phase2-footprint-plotter-ir"
title = "Deliver the first footprint and symbol plotter IR proof"
status = "done"
depends_on = ["phase2-plotter-ir-contracts"]

[[steps]]
id = "phase2-footprint-corrections"
title = "Correct typed footprint root, diagnostic, and edit-result boundaries"
status = "done"
depends_on = ["phase2-typed-reader-writer-ir"]

[[steps]]
id = "phase2-footprint-plotter-initial-proof"
title = "Deliver the solid-line footprint bytes-to-plotter-IR native and WASM proof"
status = "done"
depends_on = ["phase2-footprint-corrections"]

[[steps]]
id = "phase2-footprint-plotter-contract-corrections"
title = "Enforce safe-integer output and independent metadata resource limits"
status = "done"
depends_on = ["phase2-footprint-plotter-initial-proof"]

[[steps]]
id = "phase2-footprint-graphics-plotter-proof"
title = "Promote non-text footprint graphics and patterned strokes through native and WASM plotter IR"
status = "done"
depends_on = ["phase2-footprint-plotter-contract-corrections"]

[[steps]]
id = "phase2-shared-plotter-standard-pads"
title = "Promote shared plotter operations plus standard pad flashes and drills"
status = "done"
depends_on = ["phase2-footprint-graphics-plotter-proof"]

[[steps]]
id = "phase2-shared-plotter-corrections"
title = "Fail closed on decomposition limits and validate shared plotter semantic states"
status = "done"
depends_on = ["phase2-shared-plotter-standard-pads"]

[[steps]]
id = "phase2-custom-chamfered-pad-plotter"
title = "Promote shared custom and chamfered pad flashes through native and WASM plotter IR"
status = "done"
depends_on = ["phase2-shared-plotter-corrections"]

[[steps]]
id = "wn-dev-std-2026-8-12-rust-hygiene"
title = "Adopt the wn-dev-std 2026.8.12 Rust structural hygiene profile"
status = "done"
depends_on = ["rust-baseline-bootstrap"]

[[steps]]
id = "custom-geometry-point-budget"
title = "Add a bounded custom-pad polygon and vertex resource budget before converter cutover"
status = "done"
depends_on = ["phase2-custom-chamfered-pad-plotter"]

[[steps]]
id = "phase2-symbol-body-plotter-proof"
title = "Promote non-text library-symbol body geometry through shared native and WASM plotter IR"
status = "done"
depends_on = ["phase2-custom-chamfered-pad-plotter"]

[[steps]]
id = "phase2-symbol-pin-geometry"
title = "Promote all non-text symbol pin graphic styles through native and WASM plotter IR"
status = "done"
depends_on = ["phase2-symbol-body-plotter-proof"]

[[steps]]
id = "phase2-symbol-library-reader-writer"
title = "Deliver the source-backed symbol-library iterator and semantic writer"
status = "done"
depends_on = ["phase2-typed-reader-writer-ir", "phase2-symbol-pin-geometry"]

[[steps]]
id = "phase2-symbol-inheritance-plotter"
title = "Resolve library-symbol inheritance for non-text plotter geometry"
status = "done"
depends_on = ["phase2-symbol-pin-geometry", "phase2-symbol-library-reader-writer"]

[[steps]]
id = "phase2-boundary-review-packet"
title = "Prepare the typed native/WASM boundary review packet and findings"
status = "done"
depends_on = ["phase2-footprint-plotter-ir", "phase2-symbol-library-reader-writer"]

[[steps]]
id = "phase2-wasm-take-output"
title = "Add take-once ownership for large WASM edit and plot outputs"
status = "done"
depends_on = ["phase2-boundary-review-packet"]

[[steps]]
id = "phase2-wasm-feature-topology"
title = "Split WASM exports into independently buildable operation-family features"
status = "done"
depends_on = ["phase2-boundary-review-packet"]

[[steps]]
id = "native-first-plan-reorder"
title = "Reorder the port around native parity and move pcb_a0 and future WASM packaging off the critical path"
status = "done"
depends_on = ["phase2-boundary-review-packet"]

[[steps]]
id = "phase7-wasm-shared-plotter-dto"
title = "Remove per-operation JSON DTO conversion before expanding WASM plotter producers"
status = "pending"
depends_on = ["phase7-wasm-operation-selection"]

[[steps]]
id = "phase7-wasm-symbol-read-limit"
title = "Bound symbol-library WASM read serialization and fail closed without partial summaries"
status = "pending"
depends_on = ["phase7-wasm-operation-selection"]

[[steps]]
id = "phase7-wasm-take-output-coverage"
title = "Complete non-empty takeOutputBytes boundary coverage for every paired WASM result"
status = "pending"
depends_on = ["phase7-wasm-operation-selection"]

[[steps]]
id = "phase7-wasm-artifact-builds"
title = "Build and link isolated WASM artifacts when packaging resumes"
status = "pending"
depends_on = ["phase7-wasm-operation-selection"]

[[steps]]
id = "repo-local-corpus-audit"
title = "Audit repo-local corpus defaults, naming, and mandatory-gate availability"
status = "pending"
depends_on = ["phase6-cruncher-parity"]
+++

# KiCad Monkey Rust Port Strategy

Status: active execution plan, 2026-08-12. This is a local planning artifact.
It does not by itself establish a public Rust API or wire contract; promoted
decisions move into tracked ADRs, design docs, contracts, and release notes.

## 1. Recommended Direction

Build a Rust implementation that mirrors observable Python behavior at
contract boundaries, but do not translate the Python module and class layout
line by line.

The target architecture has three ownership layers:

```text
KiCad bytes / named project bundle
              |
              v
  kicad-monkey Rust core
  - byte scanner and structural index
  - selective and full typed source views
  - source-safe mutation/round trip
  - plotter IR producers
  - KiCad netlist and compiled schematic graph
  - iterable KiCad layout/source geometry
              |
       +------+--------------------------+
       |                                 |
       v                                 v
 native Windows CLI / Python     optional operation-specific packaging
 compatibility                  - selected WASM wrappers after parity
                                - downstream adapters owned elsewhere
```

`kicad_monkey` continues to own the KiCad reader/writer object model: syntax,
source semantics, source-safe mutation, KiCad compilation, and close-to-format
plotter/rendering facts. `data_models` continues to own `pcb_a0`, DwgScene,
Design, ALX contracts, and the KiCad-to-generic importer. Those adapters are a
separate downstream task after this port reaches test parity; they are not
milestones or completion criteria in this plan. A downstream crate may
eventually compose the native core into its own native or WASM operation
without introducing a `data_models` dependency into the parser package.

The first release should be outcome-oriented rather than a promise to port all
96 currently promoted Python exports. The initial product outcomes are:

1. high-speed full and selective S-expression reads plus build/write-back;
2. iterable typed views and writable object models over board, footprint,
   symbol, schematic, worksheet, and project content;
3. exact `kicad.plotter_ir.a0` production for selected render workflows;
4. exact KiCad netlist and `kicad_monkey.compiled_schematic_graph.a0` output;
5. a native Windows `kicad-cruncher` whose selected commands reproduce their
   Python-backed artifacts; and
6. parity with the existing in-scope Rack and Python oracle tests.

The `design`/`design-review`/`dr` workflow is the primary system-level
functional acceptance test. WASM remains important, but after the completed
feasibility proof it is a packaging phase over stable native operations. The
browser operation list is intentionally selected after native parity rather
than guessed up front, and downstream consumers may own purpose-built WASM
packages over documented core features.

## 2. Evidence From The Current Repositories

The current Python package contains about 65,000 lines across its top-level
modules, 96 promoted root exports, and 103 Rack test modules. The largest and
highest-risk behavior families are already separated enough to port in
vertical slices:

| Capability | Principal Python sources | Existing evidence |
| --- | --- | --- |
| S-expression parsing, spans, projection | `kicad_sexpr.py`, `kicad_targeted_reader.py`, `kicad_pcb_projection.py` | L0 parser/projection tests, L1 round-trip corpus, exact span tests |
| Typed source model | `kicad_pcb.py`, `kicad_schematic.py`, symbol/footprint and primitive modules | L0 model tests, L1 OOP equivalency and round trip |
| Plotter IR | `kicad_plotter_ir.py` plus the schematic, PCB, symbol, and footprint IR producers | JSON schema, L0 operation tests, L3 SVG/oracle tests |
| Netlist and compiled graph | netlist compiler/design modules and `kicad_compiled_schematic_graph.py` | accepted a0 graph contract, golden identity vectors, L0/L3 corpus oracles |
| Text | `kicad_text.py`, `kicad_stroke_font.py`, recorder and SVG paths | focused text/markup/stroke tests and visual oracles |
| PCB projection to generic models | downstream `appz/data_models` KiCad PCB importer | out-of-scope follow-on consumer after this plan reaches parity |
| End-to-end public workflows | `kicad_cruncher` command modules, especially `kicad_cruncher_cmd_design.py` | copied public corpus projects, command manifests, design-review bundle assertions, SVG and hole-metadata parity |

The current Python performance record identifies allocation as the remaining
floor. On the public Jumperless decomposition it records about 8.35 million
regex matches, 5.24 million token objects, 1.119 seconds for raw scanning,
9.541 seconds for token production, and 11.654 seconds for full parsing. This
supports replacing token-object-plus-generic-tree materialization in hot paths,
not merely translating the existing regex tokenizer into Rust.

The Altium native parity system contributes the right governance ideas:

- Python is the initial behavior oracle;
- language-neutral surfaces map to language-specific symbols and tests;
- parity-required, intentionally excluded, and deferred surfaces are explicit;
- exact output and direct external oracle lanes are stronger than shallow API
  comparison;
- a generated queryable registry reports missing symbols, missing tests, and
  stale evidence;
- native, CLI, and WASM applicability are classified independently;
- release closes only at zero unclassified required gaps.

The KiCad implementation should reuse that model, not copy the Altium C++
source structure or its external/private corpus database assumptions.

## 3. Non-Negotiable Architecture Principles

### 3.1 Mirror behavior, not Python internals

The Python implementation defines results, diagnostics, ordering, identity,
and round-trip semantics during the port. Rust may use different ownership,
iteration, indexing, and allocation strategies. A one-class-per-Python-class
translation would preserve the current materialization costs and make the WASM
surface unnecessarily large.

### 3.2 Bytes are the primary input

Core entry points accept `&[u8]` or a named in-memory source bundle. They do
not require filesystem paths, environment discovery, Python objects, or JavaScript
object graphs. Native path APIs and project discovery are adapters over this
core.

Source offsets are byte offsets. Line and column values are derived from a
shared newline index. String unescaping, UTF-8 decoding, number conversion, and
typed object allocation are lazy where the consumer does not require them.

### 3.3 Partial read has three explicit meanings

Avoid using one vague "partial parse" claim for different behaviors:

1. **Selective materialization:** scan the complete byte input but allocate
   only matching forms or typed object families.
2. **Bounded-memory streaming:** scan a native file in chunks while retaining
   only selector state and selected source ranges.
3. **Indexed repeated access:** build a fingerprinted structural span index
   once, then hydrate requested families from source ranges without rescanning
   unrelated bodies.

Native callers may use a buffered `ReadAt` source or optional memory mapping
behind the same read-only source abstraction. A later WASM wrapper already
receives file bytes in memory, so it can reuse selective allocation and indexed
access without changing the core parser design.

### 3.4 Separate syntax, typed views, and writable documents

Use distinct representations:

- a loss-conscious syntax/span document for source locations, round trip, and
  patches;
- borrowed typed views and iterators for high-volume read/convert paths;
- owned typed documents with supported mutation and write-back;
- serialized contract DTOs for native process and Python boundaries, reusable
  later by selected WASM wrappers.

An iterable board API should yield lightweight views backed by immutable source
bytes and indexes. It should not allocate a generic nested list for the whole
file before yielding the first footprint, pad, track, or graphic.

Read-only views are the fast path, not the whole product. Every promoted typed
source-model family must also have a writer path and prove
`parse -> object model -> serialize -> parse` semantic equivalence. Unknown
forms must either survive a source-preserving edit or produce an explicit
unsupported-write diagnostic; a successful write may not silently discard
them.

### 3.5 Keep WASM packaging late, coarse, and operation-shaped

The completed Phase 0-2 WASM work proves that the core can support thin
byte-in/byte-out wrappers, generated diagnostics, independent features, and
take-once output ownership. Preserve that proof, but do not expand WASM on the
native parity critical path. After native `kicad-cruncher` and Rack parity,
select concrete browser operations and package only those. Do not expose Rust
lifetimes, thousands of source objects, or a general mutable object graph to
JavaScript. Downstream consumers may own purpose-built wrappers over documented
core crate features.

### 3.6 Preserve contract ownership

`kicad_monkey` may emit its own plotter IR, compiled graph, source inventory,
and other close-to-KiCad projections from its reader/writer model. The
`pcb_a0` implementation, generic geometry/materialization policy, and validator
belong with `data_models`. That downstream work is outside this plan and begins
only after the native parity surface is trustworthy. A future downstream
native or WASM artifact may link both, but the dependency direction remains:

```text
data_models KiCad adapter -> kicad-monkey core
```

and never the reverse.

## 4. Proposed Rust Workspace Shape

Keep Rust source under `src/rs/` and all package integration/corpus tests under
the repository `tests/` hierarchy. Start with a small number of crates:

```text
Cargo.toml
src/rs/
  kicad-monkey-core/       syntax, indexes, reader/writer models, IR, compiler
  kicad-monkey-io/         native paths, project resolution, buffered sources
  kicad-monkey-cli/        native Windows-facing operations used by cruncher
  kicad-monkey-python/     optional PyO3 compatibility/acceleration wrapper
  kicad-monkey-wasm/       later, selected wasm-bindgen operation wrappers
tests/
  rs/                      Cargo integration tests registered explicitly
  parity/                  scope, case, mirror, and output manifests
```

Do not split every Python module into a crate. Within the core, use modules and
Cargo features to isolate `sexpr`, `pcb`, `schematic`, `plotter_ir`, `text`,
and `compiled_graph`. Split another crate only for a genuine dependency,
licensing, platform, compilation-time, or release boundary.

Any later downstream composition lives in the contract-owning repository and
is not tracked as work in this plan. For example:

```text
appz/data_models/src/rs/
  pcb-contracts/           TypeSpec-generated pcb_a0 DTO plus semantic validation
  kicad-pcb-a0/            kicad-monkey source -> pcb_a0 projection
  kicad-pcb-a0-wasm/       composed byte-in/byte-out browser module
```

Exact names are provisional. Crate and npm package names should be decided in
an ADR before publication.

The Rust MSRV, edition, lint policy, canonical JSON policy, and dependency
approval should align with the current `data_models`/Alexandria Rust work where
practical. Prefer safe Rust and forbid package-owned unsafe code initially.

### 4.1 `wn-dev-std` Rust hygiene baseline

Adopt the reviewed `wn-dev-std` host Rust/polyglot profile as soon as its first
project plan is ready. Do not invent a permanent KiCad-specific substitute
while that work is being stood up. The initial repository baseline should
include:

- an edition 2024 workspace with resolver 3 unless the reviewed standard or
  MSRV forces a documented exception;
- centralized workspace package metadata, dependencies, lints, and release
  profiles;
- committed `Cargo.lock` and `rust-toolchain.toml` with `rustfmt` and Clippy;
- the polyglot `src/rs` source-root declaration and Rack-owned signoff;
- `unsafe_code = "forbid"` for package-owned crates;
- locked `cargo fmt`, `check`, Clippy with warnings denied, unit/integration and
  doc tests, and rustdoc-with-warnings-denied lanes;
- explicit MSRV, dependency, license, generated-code, WASM artifact, and
  exception policies.

The reviewed profile must not become a scheduling dependency for the parser
milestone. If it is not available when Phase 0 starts, adopt a minimal
provisional baseline containing all controls listed above and record it as a
dated exception owned by the Rust port technical lead. Its review trigger is
the earlier of publication of the reviewed `wn-dev-std` profile or the Phase 2
exit review. Phase 2 cannot close until the workspace either conforms to the
reviewed profile or has an explicitly approved, time-bounded exception for
each remaining difference. This contingency permits Phase 1 work but cannot
quietly become a local permanent standard.

The standard audit is a floor, not an architecture review. Rust code also
needs project-specific rules: no panic on untrusted KiCad input, bounded input
and output resources, deterministic ordering, typed errors with byte spans,
no platform APIs in the core, no hidden global caches, and no FFI types in the
borrowed/owned domain model.

### 4.2 Planned Rust architecture review cycles

Run several explicit review cycles rather than waiting for a final code audit:

1. **Bootstrap review:** workspace/crate boundaries, feature topology, MSRV,
   dependency policy, TypeSpec generation, error model, unsafe/FFI posture,
   native packaging, and the bounded WASM feasibility proof before Phase 1
   implementation grows.
2. **Parser review:** after the first S-expression vertical slice and
   benchmarks, review allocation behavior, source ownership/lifetimes, index
   representation, streaming, resource limits, fuzzing, edit/write design, and
   public API leakage.
3. **Boundary review:** close as part of Phase 2 after the first typed
   reader/writer plus the completed Python/WASM feasibility proof. Review
   generated DTO isolation,
   PyO3/wasm-bindgen surfaces, serialization copies, diagnostics, feature
   sizes, and compatibility policy before native PCB/schematic expansion.
4. **Native parity review:** before closing the native CLI milestone, review
   semantic parity, dependency/supply-chain state, representative performance,
   Windows support, release/rollback mechanics, and durable documentation.
   Selected WASM packages receive their own later artifact reviews.

Each cycle should include reviewers familiar with idiomatic Rust library/API
design, performance-sensitive parsing, and WASM. Findings remain in the local
working plan while design is fluid; accepted decisions move to ADRs, design
documents, contracts, or standards before release.

### 4.3 Dependency and implementation selection

Do not decide that every Python dependency must have a one-for-one Rust crate,
or that every helper must be rewritten locally. Decide each behavior family
against the same evidence:

- observable parity and direct KiCad oracle results;
- safe and deterministic API behavior;
- native Windows support first, plus other native or browser targets only where
  the promoted artifact claims them;
- maintenance health, license compatibility, supply-chain review, binary size,
  compile time, and performance;
- ability to accept bytes or explicit data rather than relying on ambient host
  discovery or global state.

The preference order is:

1. use a focused, suitable Rust implementation when it meets the contract;
2. write a small package-owned implementation for KiCad-specific or genuinely
   simple behavior when that is easier to prove and maintain;
3. call an existing Wavenumber process/WASM service such as Geometer for a
   substantial generic kernel;
4. introduce native FFI only after the architecture review records why the
   other choices fail.

Crate choice remains provisional until the relevant parity corpus passes.
Pin accepted dependencies through `Cargo.lock`, isolate optional capabilities
behind narrow modules/features, and record replacements without exposing the
third-party API as the public `kicad_monkey` API.

### 4.4 Eventual Cargo ecosystem distribution

Incubate the Rust workspace in this repository without making early parser or
FFI shapes a public compatibility promise. Once the implementation, review
cycles, parity evidence, and downstream use are stable, distribute the crates
through the normal Rust community channels. The presumptive public path is
crates.io for intended public crates and docs.rs for generated API
documentation, using ordinary Cargo dependency and feature conventions. Revisit
that choice at publication time if Wavenumber has a reviewed registry policy or
a concrete reason to use another registry.

Internal consumption before public publication is intentionally TBD. Use a
normal Cargo-native mechanism appropriate to the eventual repository/build
topology, such as workspace/path dependencies for colocated development or a
pinned Git revision for cross-repository integration. A future reviewed
Wavenumber registry, vendoring, or source-mirroring policy may replace those
choices. Record the selected source and pinning/update policy before the first
downstream cutover; do not make Phase 0 depend on that decision.

Internal dependency coordinates are integration configuration, not public
contract identity. Published manifests must not contain machine-local paths or
private-only source assumptions, and internal Git dependencies must be pinned
to immutable revisions for signoff rather than floating branches.

Publish only crates that are designed as supported public libraries. Internal
workspace helpers, generated-code plumbing, test oracles, and application-
specific composition crates remain unpublished or use `publish = false`.
Before the first publication, approve crate names and ownership, semantic
versioning and MSRV policy, public feature stability, README/examples,
repository/homepage/documentation metadata, license files and dependency
licenses, security/reporting policy, and minimum supported platforms. Require
package-content inspection, a clean build from the packaged crate, and
`cargo publish --dry-run` in signoff. Python wheels and browser npm/WASM
artifacts retain their own release channels; they may depend on the same Rust
core but are not substitutes for the Cargo packages.

## 5. TypeSpec Contract Authority

Use the Alexandria contract system in `appz/data_models` as the pattern for
every cross-language or cross-package contract introduced by this port.
Authored TypeSpec is the source of truth. JSON Schema and language DTOs are
generated projections, not parallel handwritten definitions.

This is a bootstrap decision, not a cleanup after the Rust implementation.
Phase 0 establishes the compiler, normalized catalog, generation-clean gate,
and the first contract definitions. A Rust producer or WASM/Python operation
must not land against a provisional handwritten boundary DTO and migrate to
generated types later.

Contract ownership follows the domain boundary:

- KiCad-native contracts such as `kicad.plotter_ir.a0`,
  `kicad_monkey.compiled_schematic_graph.a0`, source inventory, operation
  request/response envelopes, and structured diagnostics are authored in a
  package-local TypeSpec tree, provisionally `src/tsp/kicad_monkey/`;
- generic contracts such as `pcb_a0`, DwgScene, Design, and ALX operations are
  owned by `data_models`; their promoted TypeSpec authority and projections
  live under `appz/data_models/src/tsp/data_models/`;
- generic planar geometry contracts are owned by Geometer. If its planned
  TypeSpec promotion proceeds in parallel, this port consumes the promoted
  generated projections and does not create a competing KiCad-local copy;
- a downstream composed operation imports both generated contract families;
  it does not redefine either one.

The required projection flow is:

```text
authored TypeSpec
      |
      v
normalized contract catalog
      |
      +--> JSON Schema
      +--> generated Rust DTOs
      +--> generated Python DTOs
      +--> generated TypeScript DTOs
      +--> generated documentation and conformance vectors
```

Generate only the language cells that have a real consumer, but Rust, Python,
JSON Schema, and TypeScript are expected for the initial native/Python/WASM
surfaces. The build must fail on stale generated output, missing required
projection cells, or a runtime that reads a legacy schema instead of the
promoted projection.

TypeSpec governs serialized structure, field names, optionality, enums,
version identity, and operation envelopes. It does not replace the
handwritten KiCad reader/writer object model, borrowed Rust source views, graph
compiler, or semantic rules that cannot be expressed structurally. Graph
acyclicity, hierarchy ownership, identity allocation, cross-record references,
and similar invariants remain explicit semantic validators with shared
cross-language vectors. The normalized catalog should link each contract to
its semantic operation or validator in the same spirit as ALX `model_ops.alx`.

The existing handwritten plotter and compiled-graph DTO/schema artifacts are
the compatibility oracle for the initial TypeSpec definitions. Generate all
required projections and prove exact schema/output compatibility before any
Rust producer for those contracts is accepted. Do not silently change the
current `a0` schema identities. A real incompatible change requires a new
contract version and an explicit migration decision.

No conversion hot path should build a generic JSON tree merely to cross a
contract boundary. Rust producers populate generated Rust DTOs and serialize
them directly; Python and TypeScript consumers use their generated
projections. Canonical JSON and shared request/result vectors are part of the
contract gate.

## 6. S-Expression Engine Design

The parser is the foundation and must be designed for the actual performance
goal rather than as a compatibility afterthought.

### 6.1 Scanner

- Scan bytes with a small explicit state machine for parentheses, whitespace,
  atoms, quoted strings, escapes, comments, and formatted data blocks.
- Emit events or compact token descriptors, not one heap object per token.
- Store ranges as compact offsets into immutable input.
- Decode quoted values and parse numbers only when requested.
- Preserve the existing KiCad escape and error-location behavior.
- Build the newline index once and use binary search for offset-to-line/column.
- Put resource limits on nesting, selected-form count, decoded string length,
  and output size for browser and untrusted-input use.

### 6.2 Structural index and selectors

The one-pass scanner may build a compact form table containing parent, depth,
head range, form range, and first-child linkage. Selector execution supports
head, exact path, depth, and prune rules compatible with the current Python
projection contract.

Two modes are required:

- event-only selection for minimum memory and first-result latency;
- reusable indexed selection for consumers that request several object
  families from the same large board.

The index is invalid when the source content fingerprint changes. It is an
immutable cache, never a second source of truth.

### 6.3 Typed direct parse

Typed parsers consume form cursors/events directly. They should not require an
intermediate `Vec<Value>`/generic S-expression tree. Unknown forms remain
available as source spans or loss-conscious syntax nodes so a supported edit
does not silently erase them.

The generic tree API still exists for Python compatibility and small tools,
but it is built on demand and is not on the board, schematic compiler, IR, or
`pcb_a0` hot paths.

### 6.4 Patch and round-trip model

Writing ships with the parsing foundation. Support two explicit output modes:

1. a builder/formatter that serializes an owned syntax or typed document into
   valid, deterministic KiCad S-expression text;
2. source-preserving write-back using ordered, non-overlapping replacement
   patches, copying unchanged byte ranges verbatim.

The correctness contract is semantic round-trip, not byte-for-byte identity
with the input: parse, write, parse again, and compare the syntax tree or typed
object model. Where deterministic formatting is promised, a second write must
be byte-stable. Focused edit tests must additionally prove that the intended
field changed, unrelated semantics did not change, and KiCad accepts the
result through the existing CLI oracle when one is available.

Views retain source bytes and typed ranges. A caller that edits promotes the
required family into an owned editable document or records typed patches
against the source document. This keeps read-only conversion cheap without
making downstream write-back an afterthought.

## 7. Product Pipelines

### 7.1 Single-file PCB and footprint reads

Provide borrowed iterators for top-level and nested families, including layers,
nets, footprints, pads, graphics, routed copper, vias, zones, dimensions,
models, and embedded files. A selection plan computes dependencies up front;
for example, a route request may also require the net table but not board text
or 3D payload decoding.

Expose KiCad-native physical facts such as copper, authored filled zones,
mask, paste, silkscreen, profile, holes, and explicit placement transforms as
iterable source views. Preserve source references and KiCad semantics. Do not
introduce a generic analytic scene or `pcb_a0`-like intermediate into this
package; downstream visualization and data-model code decides how those facts
are materialized.

### 7.2 Out-of-scope downstream `pcb_a0` conversion

`pcb_a0` conversion is deliberately not a delivery phase, parity gate, or
Definition-of-Done item for this plan. Another task and owner may begin that
adapter after the native parser, writer, object model, plotters, and existing
tests reach parity. This plan provides documented KiCad-native APIs and feature
boundaries that such an adapter can consume; it does not implement or validate
the adapter, its TypeSpec contracts, Viz integration, or its WASM packaging.

### 7.3 Schematic project compilation

A schematic compile is not truly one-file-in: `.kicad_pro`, root schematic,
child schematics, and libraries form a named source set. Define a platform-
neutral `SourceBundle`/`SourceProvider` operation contract with a normalized
entry path and normalized relative names. Author its serializable request and
diagnostic types in TypeSpec. Native I/O fills it from disk. A later browser
wrapper can provide named byte arrays or an application-owned virtual
filesystem without changing this core boundary.

The compiler should emit the existing KiCad-native netlist/design JSON and
`kicad_monkey.compiled_schematic_graph.a0`. Identity allocation inputs,
ordering, hierarchy occurrence realization, local-net topology identity,
scalar hierarchy bindings, policy inheritance, and diagnostics require exact
parity. Golden UUID vectors should be shared across Python and Rust. If graph
generation is later selected as a browser operation, its wrapper must reuse
the same vectors and transport-neutral bundle contract.

### 7.4 Plotter IR and basic 2D output

Port the IR DTOs and exact serializer before porting every producer. Then bring
up producers in increasing complexity:

1. footprint;
2. library symbol;
3. schematic instance;
4. PCB.

The first text implementation should cover the deterministic KiCad stroke-font
path and existing markup behavior. The promoted PCB outline-text path also
requires a shaped-font implementation described below. Heavyweight mesh/HLR
overlays remain outside the initial core.

Port the SVG backend where exact SVG, preview compatibility, or KiCad CLI
oracle tests need it. A later browser package may prefer
`plotter IR -> retained renderer`, but that packaging choice does not shape or
gate the native producer.

### 7.5 `kicad-cruncher` functional acceptance

Treat `kicad-cruncher` as the native Windows workflow consumer and system-test
harness. Its application commands remain owned by `kicad-cruncher`, while
parser, writer, object-model, compiler, and plotter behavior remains in the
`kicad_monkey` Rust crates. Stand up the native Rust command path directly over
those crates; PyO3 acceleration is optional later work and is not a prerequisite
for CLI parity.

The primary acceptance workflow is `design` / `design-review` / `dr`. For each
promoted project case, run the same command once with the Python engine and
once with the Rust engine and compare the complete review bundle:

| `dr` artifact | Required comparison |
| --- | --- |
| design JSON | generated-schema validation plus exact or governed-canonical parity |
| netlist JSON | generated-schema validation plus exact or governed-canonical parity |
| KiCad S-expression netlist | exact bytes where currently stable; otherwise parsed semantic equality |
| per-instance schematic review SVG | existing exact/canonical SVG and structural oracle lane |
| per-copper-layer PCB review SVG plus Edge.Cuts | existing SVG, layer, hole, and metadata assertions |
| manifest and README | exact stable fields with explicitly normalized timestamps/paths |

This one workflow exercises project resolution, schematic compilation,
compiled graph/netlist behavior, instance plotter IR, PCB plotter IR, text,
serialization, and caching. Record stage timings as well as total command time
so a faster parser cannot hide a slower serializer or renderer.

Classify the rest of the CLI in the parity registry rather than promising a
blanket rewrite. Initial high-value candidates are `pcb-svg`, `schematic`,
`bom`, `pnp`, `lib-extract`/`project-lib`, and the read/inspection portions of
`health`. HLR/geometer overlays, daemon/plugin hosting, KiCad application
launch, preference installation, and other local integration remain
downstream native/Python concerns unless separately promoted. The target is
artifact parity for the selected useful subset, including `dr`, not ownership
of application/report workflows by the parser package.

### 7.6 Text shaping and planar geometry dependencies

Use `rustybuzz` as the preferred first candidate for HarfBuzz-compatible text
shaping because it fits native and WASM builds without adding a platform
HarfBuzz runtime. Keep three responsibilities separate:

1. a native font resolver locates system fonts where that behavior is required;
2. the platform-neutral core receives explicit font bytes, face index, shaping
   properties, and text, then uses the selected shaper;
3. a separately selected Rust implementation extracts and transforms glyph
   outlines. Rustybuzz shapes text but does not replace the outline backend.

Choose an appropriate maintained Rust outline/font implementation or write the
small focused behavior needed by the KiCad pipeline after review; do not expose
that dependency's API as the text contract. Embedded fonts and browser callers
must supply bytes explicitly so browser results do not depend on host font
discovery.

Define a TypeSpec-owned `FontBundle` metadata contract for every operation
that may shape outline text. Each entry contains a stable font ID, byte-buffer
slot, SHA-256, face index, ordered variation coordinates, and the names/style
facts needed for resolution. The request carries deterministic resolution
rules: an explicit font ID wins; otherwise aliases are resolved in declared
bundle order with ambiguity reported as an error. WASM never falls back to an
ambient system font. Its supported parity claim is limited to embedded fonts
and caller-supplied font bundles.

Validation is fail-closed: font IDs are unique, buffer slots are unique,
every referenced slot is in range, each supplied buffer is referenced exactly
once, and its bytes match the declared hash. Missing, duplicate, aliased, or
out-of-range slots are errors. Extra unreferenced buffers are rejected rather
than ignored, so all wrappers handle the same inputs deterministically.

Font bytes remain out-of-band binary inputs. The TypeSpec operation envelope
describes slots and hashes; it does not base64-encode font data in JSON. The
logical browser call is:

```text
plotterIrFromKiCadBytes(
    sourceBytes: Uint8Array,
    requestJsonBytes: Uint8Array,
    fontBuffers: Uint8Array[],
) -> Uint8Array(JSON)
```

Native and Python wrappers implement the same logical contract with named byte
buffers and may add a native resolver that constructs the bundle. Required
diagnostics include missing font, ambiguous resolution, hash mismatch, invalid
face index, unsupported variation, malformed font, and resource-limit
failures. Bound font count, individual and aggregate font bytes, glyph count,
contour points, and shaped output size for untrusted/browser inputs.

Rustybuzz compatibility is a hypothesis to prove, not parity by declaration.
Compare glyph IDs, clusters, advances, offsets, direction/script/language and
feature handling, then compare final glyph contours and render-cache records.
Add a three-stage evidence ladder so final geometry failures are diagnosable:

1. a shaping-record corpus with fixed font bytes/hashes, face index, variation
   coordinates, scale, text, direction, script, language, buffer properties,
   and feature inputs; expected records contain glyph IDs, clusters, advances,
   and offsets in explicit units;
2. an outline-extraction corpus keyed by fixed font hash, face/variation, and
   glyph ID, comparing contours before KiCad placement/scaling transforms;
3. the existing final render-cache and SVG comparisons after shaping,
   outlining, transformations, markup, and caching are composed.

Give each vector a stable ID and record whether its field is exact or governed
by a stated tolerance. Generate the shaping/outline vector schema from
TypeSpec, while storing font binaries separately in the normal corpus layout.
The current L2 render-cache suite remains the final acceptance oracle: it
already detects micron-scale drift between shaping implementations and
KiCad's HarfBuzz build.
Cover system fonts, embedded fonts, transformations, styles, markup/runs, text
boxes, footprint properties, and dimension text. If Rustybuzz cannot meet the
governed tolerance for a promoted case, record the gap and review another Rust
implementation, a focused local implementation, or a narrowly isolated native
HarfBuzz adapter; do not hide the difference.

For generalized planar boolean, offset/inflate, cleanup, and triangulation
work, prefer Geometer rather than vendoring Clipper2 or rebuilding a large
polygon kernel in Rust. Geometer already provides generic batch operations,
a native process-level bytes/JSON interface, and a planar-only browser WASM
artifact. Keep this dependency outside the parser/source-model core:

```text
kicad-monkey source/plotter facts
              |
              v
optional KiCad-to-planar adapter
              |
              v
Geometer versioned request -> process or planar WASM -> versioned result
```

Simple KiCad-specific curve tessellation and shape conversion may remain a
small tested Rust module when no boolean kernel is needed. Promote Geometer
only for operations that need its generic kernel, batch calls to amortize the
native process boundary, and keep board/net/render policy in the caller.

The detailed bridge is deliberately deferred. Before it is implemented,
inventory the exact Python polygon operations that require a kernel and map
each to an existing Geometer capability. Coordinate with the parallel
Geometer TypeSpec-contract effort: consume its generated, versioned
request/result projections once promoted, pin the contract/version and test
vectors, and do not publish a private duplicate contract in this repository.
Native composition uses the released process interface. If a later selected
browser operation needs the same kernel, its downstream owner may use
Geometer's planar WASM artifact, normally orchestrated by the downstream worker
rather than linked into the Rust parser core.

Ownership is decided per promoted operation before bridge work begins:

- a transformation required to produce a close-to-KiCad plotter/source result
  may use a sibling optional `kicad_monkey` adapter crate, never
  `kicad-monkey-core`;
- generic materialization for `pcb_a0`, DwgScene, Viz, or application reports
  is owned by `data_models`, Viz, or the relevant downstream application;
- Geometer owns only its generic operation and wire contract, never KiCad or
  board policy.

A bridge-entry record in the parity registry must name the operation, owner,
upstream TypeSpec contract/version, execution modes, vectors, and resource
budget. Entry is blocked until the generated upstream contract exists and the
adapter design covers process version/ABI negotiation, executable discovery,
timeouts and cancellation, crash/exit/stderr mapping, request/result limits,
and deterministic error translation. When the operation is later selected for
browser packaging, browser acceptance additionally defines equivalent Web
Worker initialization, timeout/cancellation, trap, memory-limit, and error
semantics and passes the same semantic vectors.

## 8. Later Operation-Specific WASM Packaging

Phase 0-2 established a first-class feasibility proof for byte-oriented WASM
wrappers. Further browser packaging begins only after native parser, writer,
plotter, compiler, `kicad-cruncher`, and existing-test parity. At that gate,
record concrete browser consumers and select only the operations they need.
The list may include S-expression selection, plotter IR, or compiled graph
generation, but none is presumed required before that selection review.

Each selected operation uses a thin wrapper over the same transport-neutral
Rust core, structured versioned options and diagnostics, separate byte buffers,
take-once large output ownership, and an independent Cargo feature or artifact.
Return UTF-8 bytes for large JSON contracts, avoid base64, and avoid repeated
Rust-to-JavaScript object conversion for large primitive sets. Record WASM
initialization, transfer, linear-memory, execution, and serialization costs for
the selected artifact rather than benchmarking an unknown universal bundle.

Downstream consumers may package their own WASM operations from documented
core features. Such packages, including any future `pcb_a0` composition, are
owned and tested by those downstream projects unless explicitly promoted back
into this plan.

WASI/native filesystem behavior must not leak into the core. Environment
discovery, KiCad preference mutation, application launching, and external
`kicad-cli` execution stay native/local-integration concerns.

## 9. Parity Control System

Lift the Altium traceability ideas into a smaller package-local system.

### 9.1 Authoritative inputs

Keep these declarative and version controlled:

- `tests/parity/scope.toml`: language-neutral capability/API/contract surfaces;
- per-language dispositions: `required`, `wasm_required`, `native_only`,
  `python_only`, `deferred`, `replaced_by_contract`, or `retired`;
- stable case IDs and fixture selectors;
- Python, Rust, and WASM test-to-surface mappings;
- exact-output and direct-oracle classifications;
- TypeSpec contract ownership, required generated projection cells, semantic
  validator/vector mappings, and promotion state;
- out-of-band binary bundle manifests and hashes for fonts or named project
  sources referenced by generated operation envelopes;
- external bridge-entry records naming operation owner, upstream contract and
  version, execution modes, failure semantics, resource budgets, and vectors;
- performance workload manifests.

Generate a SQLite registry and human-readable JSON/HTML report under `temp/`
or Rack results. The database is a query/cache product, not hidden durable
truth. Fingerprint all declarative inputs and fail signoff on a stale report
once the workflow is established.

### 9.2 Comparison strengths

Use the strongest applicable comparison for each surface:

1. exact bytes for stable JSON, S-expression, or SVG contracts;
2. canonicalized structural equality when formatting is intentionally free;
3. typed semantic equality for object models;
4. direct KiCad CLI or other independent oracle comparison where available;
5. Python-vs-Rust differential comparison for behavior without an external
   oracle;
6. property and fuzz tests for syntax, escape, numeric, malformed-input, and
   round-trip invariants.

Python agreement alone is insufficient where the existing tests already have
a KiCad CLI oracle. Rust must pass the same validation class.

### 9.3 Per-slice workflow

Every capability slice follows one repeatable loop:

1. classify the surface for native Rust, CLI, and existing-test parity;
2. capture Python behavior and current direct-oracle evidence;
3. add language-neutral golden or normalized vectors;
4. generate and verify every required TypeSpec projection for boundary DTOs;
5. implement the smallest Rust vertical path using the generated types;
6. run Rust unit/integration tests under `tests/rack.py`;
7. run Python/Rust differential and direct-oracle comparisons;
8. measure representative release-native behavior without blocking stand-up
   on advisory microbenchmarks;
9. update the generated parity registry and close only when no required gap
   remains for that slice.

After native parity, a selected WASM packaging slice repeats the applicable
contract, resource-limit, real-browser, and artifact-size checks for that
operation only.

Stable case IDs matter more than identical test filenames, although mirrored
same-basename tests are useful for human review.

### 9.4 Corpus policy

Reuse the package-local corpus archive and current `input/`,
`reference_output/`, `output/` convention. `WN_TEST_CORPUS` remains only an
override. Rust tests must resolve cases through the same manifest/corpus helper
contract; they must not embed machine-local paths. Generated output remains
transient.

Before the native platform matrix, run the named `repo-local-corpus-audit`.
It must prove that the default archive is exactly `tests/corpus/kicad.zip`,
`WN_TEST_CORPUS` is override-only, stale shared/private-corpus names have been
removed from active tests and contributor instructions, and every required
corpus gate fails with an actionable restore diagnostic instead of silently
skipping when its package-local cases are unavailable.

### 9.5 Rack strata are the delivery order

Do not create a separate native test hierarchy with weaker meanings. Register
Rust and Python/Rust parity cases through `tests/rack.py` and preserve the
existing concern strata:

| Rack stratum | Rust-port proof |
| --- | --- |
| L0 foundation | byte lex/parse, generic build/format, diagnostics, spans, selectors, mutation primitives, property/fuzz invariants |
| L1 parsing | corpus parser pass-through plus typed source-model read/write semantic equivalence for each promoted file family |
| L2 tools | extraction and focused object-model mutation/write behavior, including existing KiCad CLI write oracles |
| L3 rendering | TypeSpec-generated IR/graph/font contracts, staged shaping/outline/cache evidence, plotter/SVG parity, netlist, and direct rendering oracles |
| L4 applications | project-level validation and selected end-to-end consumer workflows |
| L99 signoff | generated-contract cleanliness, parity-registry closure, packaging, platform, and performance release gates |

The first Rust milestone therefore closes L0 parser/build behavior and the
parser-only portion of L1 before any plotter or converter work is allowed to
claim readiness. Reuse current cases and stable IDs; add Rust-specific test
symbols and evidence mappings rather than cloning fixture data.

## 10. Port Scope Classification

The initial classification should be reviewed and then encoded in the parity
scope file.

| Capability | Initial disposition | Reason |
| --- | --- | --- |
| Byte lexer, parser, selectors, spans, diagnostics | required native; WASM proven then deferred | foundation for speed, writers, and native CLI parity |
| Generic build/format APIs | required native; later WASM selectable | parser foundation includes valid write-back, not read-only acceleration |
| Typed board/footprint/symbol/schematic/worksheet/project read-write | required native | reader/writer and existing-test parity boundary |
| Iterable object-family views | required native | avoids full model and generic tree allocation |
| Plotter IR DTO and selected producers | required native | native rendering and `kicad-cruncher` acceptance |
| Netlist and compiled schematic graph | required native | design-review and design-model parity |
| Deterministic stroke text | required native | plotter parity without platform font dependency |
| PCB outline-text shaping | required native for promoted render paths | Rustybuzz is the preferred candidate; shaping-record and final render-cache/KiCad evidence both decide parity |
| Glyph outline extraction | required native for promoted outline text | select a suitable Rust implementation or write the focused required behavior |
| System font discovery and OS preference integration | native-only adapter | platform policy stays outside deterministic font-bytes core |
| Source-safe targeted mutation | required as each model family lands | remote placement, extraction, semantic round-trip, and downstream write-back |
| Broad public Python class compatibility | provided through wrapper as needed | avoid freezing Rust internals to Python layout |
| `pcb_a0` conversion | out of scope | separate downstream task after native test parity |
| Iterable KiCad physical/source geometry | required native | reusable KiCad-native facts without owning generic models |
| Planar boolean/offset/cleanup/triangulation | optional promoted Geometer service | reuse generic Clipper2-backed process/WASM kernel; do not vendor it into Rust core |
| Mesh loading, STEP triangulation/HLR, debug viewers | Python-only/deferred | explicitly outside initial port; heavyweight dependencies |
| Convex-hull footprint filters and Shapely/Trimesh helpers | Python-only/deferred | not needed by target read/convert outcomes |
| Zone refill/recalculation | deferred | consume authored filled polygons first; large geometry kernel scope |
| Environment discovery, preference installation, app launch | Python/native local integration | filesystem/OS workflow, no browser value |
| Unwired symbol-library watcher utilities | retired/deferred pending consumer | no supported product path justifies a port |

Exclusion does not mean deletion from Python. It means the native release does
not claim parity for that surface.

## 11. Phased Execution Plan

### Phase 0: Contract, registry, and baseline

- Approve crate ownership and record `pcb_a0` as an out-of-scope downstream
  consumer rather than a port milestone.
- Apply the first reviewed `wn-dev-std` Rust/polyglot profile, or the explicit
  provisional contingency in Section 4.1 if that profile is not yet ready,
  and complete the bootstrap architecture review. Record temporary exceptions
  with owners and review triggers rather than weakening the baseline.
- Establish the package TypeSpec root, normalized catalog, JSON Schema and
  Rust/Python/TypeScript generators, conformance-vector layout, and
  generation-clean check by following the ALX implementation pattern.
- Author generated request/result/diagnostic contracts for the first
  scan/select/build/write operations. Record plotter IR and compiled graph as
  TypeSpec-owned migrations before their Rust implementations begin.
- Generate the initial Python public-surface and test inventory.
- Classify every promoted surface and the additional outcome contracts.
- Record Rustybuzz as the first shaping candidate and Geometer as the preferred
  external planar-kernel boundary, without adding either to the S-expression
  dependency graph. Track the upstream Geometer TypeSpec-contract effort as an
  integration prerequisite, not a local schema task.
- Stand up Rust build, formatting, lint, native test, Rack integration, and a
  bounded WASM byte-in/byte-out feasibility smoke.
- Capture reproducible Python baselines for small, medium, and largest public
  board/schematic cases, including time, peak memory, allocations where
  available, and serialized output cost.
- Record proposed performance thresholds only after the baseline harness is
  independently reproducible.

Exit gate: the reviewed `wn-dev-std` baseline is clean or the dated
provisional contingency is active with an owner and Phase 2 review trigger;
the bootstrap review is clean; TypeSpec generation is authoritative and clean;
the registry can answer which Python surfaces are required in native Rust and
the CLI, which tests cover them, which generated contract cells are required,
and which gaps remain. WASM applicability may remain unselected after the
feasibility proof.

### Phase 1: High-speed S-expression core

- Implement byte scanner, errors, spans, selectors, pruning, generic tree on
  demand, newline index, selected-form parsing, build/format, and the mutation
  primitives required for write-back.
- Implement both deterministic full serialization and source-preserving patch
  emission over immutable input ranges.
- Add streaming native scanning and reusable structural indexes.
- Mirror the L0 parser/build/format/mutation cases first, then run the L1
  parser-only corpus gate with the same `lex`, `tree`, `build`, `reparse`, and
  `compare` failure phases as `test_L1_018_corpus_sexpr_passthrough.py`.
- Require `parse -> build -> parse` semantic equality across every promoted
  KiCad S-expression suffix and byte-stable second output for the deterministic
  formatter.
- Differential-test all targeted-reader/projection cases and the complete
  corpus, including selective output versus filtered complete output.
- Add fuzz/property tests for malformed nesting, escapes, UTF-8, numbers,
  comments, formatted blocks, chunk boundaries, and resource limits.
- Expose diagnostic native scan/select/build operations using the generated
  Phase 0 envelopes; preserve the completed WASM feasibility adapter without
  making it a continuing phase gate.

Exit gate: the Rust path closes the relevant L0 foundation behavior and the
L1 parser-only corpus pass-through gate, including semantic write/reparse;
exact span/error semantics, bounded-memory selection, and ratified release-mode
throughput/memory targets all pass. The parser architecture review has closed
or explicitly dispositioned its findings before typed model expansion.

### Phase 2: First typed reader/writer and IR proof

- Add typed footprint and symbol-library views plus owned editable documents
  and writers.
- Prove typed `load -> serialize -> load` semantic equivalence, preservation
  of unknown source forms, focused edits, and deterministic second writes
  through the corresponding L1 cases.
- Port plotter IR DTOs, deterministic serialization, footprint IR, symbol IR,
  and stroke text needed by those paths, using TypeSpec-generated DTOs from the
  first implementation commit.
- Prove exact IR and canonical/exact SVG oracle output on existing cases.
- Publish/test operation-shaped native prototypes and retain the already
  implemented WASM prototypes as feasibility evidence.

Exit gate: `.kicad_mod` and selected `.kicad_sym` bytes can be read, edited,
written, and semantically reparsed, and produce parity IR in native tests
without constructing a generic full-file tree. The existing WASM proof remains
green but does not require expansion. The boundary architecture review is
closed or every finding is explicitly dispositioned before Phase 3;
the workspace conforms to the then-reviewed `wn-dev-std` profile or has an
approved, time-bounded exception for each remaining difference.

### Phase 3: KiCad-native PCB reader/writer

- Implement the board typed-view families and dependency-aware iterators.
- Add the owned board document, serializers, and source-preserving focused edit
  path. Close the PCB L1 semantic/OOP round-trip cases and the promoted L2
  mutation/write oracles before downstream cutover.
- Expose iterable KiCad-native copper, authored filled zones, holes, profile,
  placement transforms, and resolved-net facts. Prove selective reads equal
  filtering the complete typed source model.
- Inventory each promoted PCB polygon operation and classify it as simple
  KiCad-specific Rust math, Geometer-backed generic kernel work, or deferred.
  Do not build the Geometer bridge until a required operation and its promoted
  upstream contract are both identified.
- Differential-test source inventory and each model family over the existing
  package corpus and direct KiCad write oracles.
- Document stable native feature boundaries and iterators for later downstream
  adapters without implementing `pcb_a0`, Viz, ALX, or browser packaging.

Exit gate: supported boards pass semantic read/write/reparse, stable second
write, mutation, selective-read, iterable-view, and applicable KiCad CLI
oracles. Unsupported constructs produce explicit diagnostics rather than
silent loss.

### Phase 4: Schematic compiler and compiled graph

- Implement project bundles, project/schematic/library reader/writer models,
  hierarchy
  discovery, occurrence realization, connectivity, bus expansion required by
  a0, netlist models, and serialization.
- Close schematic, symbol-library, worksheet, and project semantic round-trip
  cases plus applicable KiCad CLI mutation/write oracles.
- Generate the compiled schematic graph DTO/schema projections from its
  TypeSpec authority, then port the validator, deterministic UUIDv7 allocator,
  and compiler.
- Share identity vectors across Python and Rust; later packages reuse them.
- Prove single-page, hierarchy, repeated-page, multipart, scalar binding,
  global-label, bus-entry, DNP/off-board, and malformed graph cases.

Exit gate: project bundle bytes produce exact or governed-canonical netlist and
compiled graph output for the full accepted project corpus in native Rust.

### Phase 5: Schematic/PCB plotter IR closure

- Port schematic-instance and PCB IR producers using the typed source views.
- Implement the promoted PCB outline-text path with Rustybuzz first, an
  independently selected outline backend, and explicit font-byte inputs.
- Generate the `FontBundle`, shaping-record, and outline-vector projections,
  then close shaping-record parity, outline-extraction parity, and finally
  render-cache parity against the L2 KiCad save oracle before claiming
  TrueType/outline-text support.
- For promoted generalized polygon operations, consume the generated Geometer
  contract through a batched native-process adapter only after the bridge-entry
  ownership and operational gate in Section 7.6 passes. Keep basic non-kernel
  KiCad shape conversion local. Browser composition is evaluated later only
  for selected operations.
- Close plotter operation, record, source attribution, drawing link, text,
  image, and style parity gaps.
- Reuse existing canonical SVG, KiCad CLI, and visual structural oracles.
- Keep system-font discovery in a native adapter and keep mesh/HLR exclusions
  explicit unless a measured product need promotes them.

Exit gate: every IR surface classified as required has exact schema-valid
output, registered test coverage, and direct oracle evidence where available.
Promoted outline text passes shaping-record, outline-extraction, and final
render-cache tolerances; any promoted Geometer operation passes the native
vectors. Browser-vector parity becomes a gate only if that operation is later
selected for WASM.

### Phase 6: Native Windows `kicad-cruncher` and parity closure

- Stand up the native Windows `kicad-cruncher` command surface directly over
  stable Rust crates and operation APIs.
- Select and document the internal Cargo source/pinning/update mechanism;
  workspace/path or immutable-revision Git dependencies are acceptable
  incubation choices.
- Run native/Python shadow comparisons in Rack and selected command workflows.
- Promote native Rust by operation, with metrics and diagnostics visible to
  callers.
- Do not use silent Python fallback to claim native support; excluded or
  unsupported operations must be explicit.
- Run the selected `kicad-cruncher` command matrix through both engines; make
  `dr` bundle parity and stage-level performance a required application gate.
- Recheck any boundary-review finding affected since Phase 2 and complete the
  narrower native-parity review before removing the shadow implementation.
- Run the complete in-scope existing Rack suite and close every required
  native, CLI, contract, writeback, and direct-oracle parity cell.

Exit gate: the native Windows CLI, selected `kicad-cruncher` workflows, and all
in-scope existing tests point to one Rust implementation per promoted
capability. No `pcb_a0`, Viz, ALX, real-browser, or PyO3 cutover is required.

### Phase 7: Selected WASM packaging and optional Python integration

- Review concrete browser consumers only after Phase 6 closes.
- Before expanding plotter producers, replace the footprint/symbol duplicate
  generated Rust DTOs with one canonical `PlotterOperation` type and remove the
  per-operation JSON conversion in the symbol adapter.
- Add `max_output_bytes` to symbol-library WASM reads, use bounded
  serialization, and return a structured resource-limit result with no partial
  summaries.
- Prove non-empty consuming `takeOutputBytes` behavior for every paired result,
  including direct generated-JavaScript execution evidence.
- Replace isolated `cargo check` evidence with actual artifact build/link
  evidence when WASM packaging resumes.
- Select operation-specific wrappers and artifact owners; explicitly delegate
  or defer operations with no current browser consumer.
- Reuse the transport-neutral core and TypeSpec contracts. Keep wrappers thin,
  byte-oriented, separately featured, resource-bounded, and independently
  testable in a real browser.
- Allow downstream projects to own purpose-built WASM composition rather than
  forcing every browser operation into `kicad_monkey`.
- Bind stable operations with PyO3 only where Python acceleration or
  compatibility has a demonstrated consumer need.

Exit gate: each selected browser operation has explicit ownership and passing
contract/resource/browser evidence, or is explicitly delegated or deferred.
Native parity remains closed regardless of the selected list.

### Phase 8: Long-tail disposition and publication

- Extend mutation/write coverage only for additional object families or edit
  operations promoted by real consumers; core write-back is already required
  in Phases 1 through 4.
- Classify remaining utilities from evidence rather than completeness pressure.
- Port additional text, zone, library, or filter behavior only when a supported
  workflow requires it.
- Select the intentionally public crates, freeze their first supported API and
  feature/MSRV policy, verify packaged-crate builds and publication dry runs,
  and publish through the approved Cargo registry only after the publication
  review passes. Incubation milestones do not require registry publication.
- Move accepted architecture into ADRs/design docs/contracts and retire this
  plan at closeout.

## 12. Performance Program

Measure operations, not just parser microbenchmarks:

- raw scan and structural indexing throughput;
- time to first selected object;
- one-family and repeated-family selective reads;
- complete typed PCB/schematic read;
- board bytes to selected and complete iterable KiCad physical/source facts;
- project bundle to netlist/compiled graph;
- project bundle to per-sheet plotter IR;
- Rustybuzz shaping and glyph-outline/render-cache generation, separated into
  font load, shape, outline, and serialization stages;
- any promoted Geometer path, separated into request construction, process or
  WASM boundary overhead, kernel execution, and result decoding;
- output serialization separately from compilation;
- native peak RSS/allocation count.

For operations selected in Phase 7, add artifact-specific WASM initialization,
input-copy, execution, output-copy, decode, size, and linear-memory evidence.

Use release builds and record CPU, OS/browser, compiler, crate feature set,
input hash/size, output hash/size, and warm/cold status. Keep broad benchmarks
advisory at first; promote stable regression ratios into signoff after enough
machines reproduce them.

Proposed initial objectives, to ratify in Phase 0 rather than treat as already
accepted promises:

- at least 5x faster full typed parsing on the largest public PCB case;
- at least 10x faster than the Python full model for narrow object-family
  reads, with allocation proportional to selected output;
- at least 3x faster project-to-compiled-graph and selected native
  `kicad-cruncher` production;
- no generic S-expression tree allocation in promoted conversion paths;
- no single serialization stage silently dominating the improved parser;
- explicit per-operation WASM size and memory budgets only for Phase 7
  selected artifacts before their publication.

## 13. Main Risks And Mitigations

| Risk | Mitigation |
| --- | --- |
| Port reproduces Python allocation architecture | Require direct typed event/view paths before declaring parser completion |
| Python parity preserves an existing bug | Run the same direct KiCad CLI/oracle lanes and disposition differences explicitly |
| Downstream `pcb_a0` work expands this port | Keep it in a separate task owned by `data_models` after native parity |
| WASM becomes one oversized module | Separate wrappers/features by operation and measure compressed size per artifact |
| WASM packaging delays usable native tools | Freeze the completed feasibility proof; select and package browser operations only after native parity |
| Multi-file schematics are modeled as one byte buffer | Establish named `SourceBundle`/`SourceProvider` before compiler work |
| Borrowed Rust views become an unstable FFI API | Keep them Rust-internal/public-crate APIs; export coarse DTO/byte operations to Python/JS |
| Identity or ordering drifts | Share golden identity vectors and exact serializer tests across all languages |
| Selective reads omit hidden dependencies | Build explicit selection plans and prove selective output equals filtered complete output |
| Fast borrowed views become effectively read-only | Require an owned/patch writer and semantic round-trip gate with each typed model family |
| A successful edit drops unknown KiCad forms | Preserve untouched ranges or fail explicitly; exercise focused edits and KiCad CLI write oracles |
| Automatic fallback hides unsupported constructs | Return structured diagnostics and track support disposition; no silent parity claims |
| Geometry work expands into mesh/zone-kernel scope | Consume authored fills; keep mesh, convex hull, HLR, and refill excluded initially |
| A convenient Rust crate becomes an accidental public architecture | Wrap dependencies behind package-owned traits/types and promote only after parity, claimed-platform, license, and maintenance review |
| Rustybuzz output drifts from KiCad's HarfBuzz build | Compare shaping records and final render caches against L2/KiCad oracles; disposition failures before promotion |
| Glyph shaping is mistaken for complete text rendering | Review and test font resolution, shaping, outline extraction, transformations, and caching as separate stages |
| WASM outline text cannot resolve a native system font | Require an out-of-band `FontBundle`; limit browser parity to embedded/caller-supplied fonts and emit deterministic missing-font diagnostics |
| Geometer is copied or tightly linked into the parser core | Keep it behind its versioned process/WASM contract, batch calls, and consume the upstream TypeSpec projections |
| Geometer process and worker failures have different semantics | Require native failure mapping at bridge entry and equivalent worker semantics only for later browser-selected operations |
| Parallel Geometer contract work drifts from the consumer | Pin schema/version and shared vectors; integrate only a promoted upstream contract and never fork it locally |
| `wn-dev-std` availability blocks parser execution | Permit the owned, dated provisional baseline only through the Phase 2 conformance review trigger |
| Two implementations diverge after port | Adopt standing dual-impact review and generated zero-gap release report, as in Altium |

## 14. Release And Definition Of Done

The native parity milestone is complete only when:

- every in-scope surface has an explicit native Rust, CLI, test, and support
  disposition;
- the pinned `wn-dev-std` Rust profile passes and bootstrap, parser, boundary,
  and cutover architecture-review findings are closed or explicitly
  dispositioned;
- the generated registry reports zero unclassified or uncovered required gaps;
- TypeSpec is the authority for every promoted interchange surface, generated
  projections are clean, and required cross-language vectors pass;
- parser, IR, graph, and native CLI outputs pass their strongest existing
  parity/oracle class;
- every promoted source-model family passes semantic read/write/reparse and
  deterministic second-write gates, plus direct KiCad write oracles where
  available;
- native Windows core and `kicad-cruncher` gates pass, and any additional
  claimed native platform has explicit evidence;
- performance objectives are measured on frozen representative cases and no
  stage-level regression is hidden by aggregate timing;
- any promoted Rustybuzz/outline path uses the generated `FontBundle` contract,
  passes separate shaping and outline vectors, and passes the KiCad
  render-cache oracle;
- any promoted Geometer path has an explicit adapter owner, uses its upstream
  generated contract, and passes the native operational bridge and conformance
  gates;
- Python packaging remains usable for intentionally Python-only capabilities;
- the existing in-scope Rack suite and selected native `kicad-cruncher`
  command matrix, including `dr`, pass their parity gates;
- accepted decisions move into ADRs, design documents, release notes, and
  contracts.

Later WASM/publication closeout additionally requires:

- concrete browser operations and artifact owners are selected after native
  parity, with unknown operations explicitly delegated or deferred;
- each selected WASM artifact passes its generated-contract, resource-limit,
  feature-isolation, real-browser, size, and transfer evidence;
- downstream-owned WASM packages remain downstream rather than becoming
  implied `kicad_monkey` deliverables;
- public crates/npm/Python artifacts have version, license, schema, and support
  policy documented;
- each published Cargo crate is intentionally public, passes package-content
  and clean packaged-build checks plus `cargo publish --dry-run`, and has
  docs.rs-ready documentation and repository metadata;
- this local plan is retired.

## 15. Remaining Decisions At Phase Gates

The package boundary, TypeSpec-first contract policy, behavior-parity model,
S-expression-first milestone, and early semantic write-back requirement are
already planning assumptions. Remaining choices should be made with evidence:

1. Select the first typed reader/writer slice after the parser milestone;
   footprint/symbol remains the recommended small plotter proof.
2. Confirm the iterable KiCad-native PCB source families required for existing
   tests and native `kicad-cruncher`, without adding a generic materialization
   model to `kicad_monkey`.
3. Ratify benchmark cases and performance budgets after Phase 0 measurements.
4. After a focused text spike, select the glyph-outline implementation and
   confirm whether Rustybuzz meets the shaping-record contract before the
   outline and final KiCad render-cache gates.
5. When a real polygon-kernel need is promoted, select the exact Geometer
   operations and minimum upstream TypeSpec contract version; defer this bridge
   until then.
6. Before native `kicad-cruncher` integration, choose the internal Cargo
   reference/distribution mechanism and immutable pin/update policy.
7. At the publication gate, confirm crate names/ownership and the approved
   Cargo registry; crates.io/docs.rs are the default assumption, not a Phase 0
   dependency.
8. After native parity, select the concrete browser operations and decide which
   WASM packages belong here versus in downstream consumer repositories.
9. Decide whether later Python integration ships as an optional accelerator or
   changes the existing wheel build to a mixed Python/Rust package.
