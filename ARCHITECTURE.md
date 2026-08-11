# KiCad Monkey Architecture

Version: 2026.5.31
Last updated: 2026-05-31

This document describes the public architecture for the standalone
`kicad-monkey` package.

## Decision Sources

This root document is the package-level architecture overview. Normative
architecture decisions live in `docs/adrs/`:

- `ADR-001`: source layout and API conventions;
- `ADR-002`: test corpus layout and Rack lane model;
- `ADR-003`: design documentation and test-ownership signoff;
- `ADR-004`: KiCad project library extraction;
- `ADR-005`: S-expression projection parsing;
- `ADR-006`: PCB projection domain objects;
- `ADR-007`: PCB graphical bounds and bbox oracle policy.

Detailed interface intent lives in `docs/design/api/`. Stable machine-readable
contracts live in `docs/contracts/`.

## Package Boundary

`kicad-monkey` owns KiCad source parsing, round-trip modeling, close-to-format
mutation helpers, and IR-backed 2D rendering.

Higher-level workflows belong in downstream applications. The parser layer does
not perform BOM policy, project migration orchestration, preference management
workflows, release packaging for generated artifacts, or neutral-model business
logic.

In particular, invoking `kicad-cli` to convert foreign CAD libraries, selecting
assets, staging and publishing output, and applying application overwrite policy
belong outside `kicad-monkey`. The sibling `kicad-cruncher` distribution owns
generic KiCad CLI and artifact orchestration; applications own their business
models and persistence.

## Core Principles

1. Core KiCad S-expression parsing stays dependency-light and round-trip safe.
2. Parser and OOP model classes extract KiCad-native source data without
   applying application policy.
3. File-format mutation helpers preserve KiCad source semantics and should be
   tested with parse -> emit -> parse coverage.
4. Rendering uses package-owned KiCad source models and public rendering entry
   points; generated SVG metadata that leaves the package must have a documented
   contract and conformance tests.
5. Public API promotion requires matching design documentation and Rack test
   ownership.

## Source Layout

```text
src/py/kicad_monkey/
  kicad_sexpr.py              S-expression parser and serializer
  kicad_schematic.py          schematic OOP facade
  kicad_symbol.py             symbol-library OOP facade
  kicad_pcb.py                PCB OOP facade
  kicad_footprint.py          footprint OOP facade
  kicad_project.py            project file facade
  kicad_design.py             project-level design assembly helpers
  kicad_*_svg.py              SVG rendering entry points
  kicad_*_ir.py               renderer-neutral IR helpers
  kicad_*_filter*.py          low-level KiCad source cleanup helpers
  testing/                    public test helpers

tests/
  L0_foundation/              public fast foundation tests
  L1_parsing/                 parser and corpus coverage
  L2_tools/                   tool-level KiCad source behavior
  L3_rendering/               rendering and design JSON checks
  L99_signoff/                release signoff gates
```

## Public API Surface

The package root intentionally exposes a broad discovery surface while early
downstream consumers prove the API shape. The promoted public contract is the
narrower set recorded in `kicad_monkey.kicad_api_contract`.

Adding a class or major interface to the promoted public contract requires:

- design documentation under `docs/design/api/`;
- Rack test ownership metadata;
- public import resolution through the package root;
- a release-signoff update when the contract changes.

## Data Flow

### PCB Source Flow

```text
.kicad_pcb
  -> parse_sexp()
  -> KiCadPcb
  -> KiCad-native analysis, mutation, rendering, or downstream adapters
```

`kicad_pcb_parser.py` and `KiCadPcbDoc` are legacy parser-equivalency surfaces.
New code should use the typed `KiCadPcb` object graph.

### Schematic And Design Flow

```text
.kicad_pro + .kicad_sch + libraries
  -> KiCadProject / KiCadSchematic / KiCadSymbolLib
  -> KiCadDesign
  -> design JSON, netlist JSON, rendering, or downstream adapters
```

The package may expose optional bridge helpers for neutral data models, but the
KiCad source model remains the package-owned boundary. Conversion policy belongs
outside the parser.

## Corpus And Test Model

The public corpus is transported as `tests/corpus/kicad.zip`. The loose corpus
mirror is ignored locally and extracted by test helpers when needed.

Persistent cases should use:

```text
input/
reference_output/
output/
```

`output/` is transient and should not be authoritative fixture data.

Rack lanes:

- `fast`: default routine gate;
- `full`: broader validation;
- `strict`: heavier or stricter checks.

## Quality Gates

`L99_signoff` enforces the current public release bar:

- date-version release metadata;
- changelog coverage;
- public repository support files;
- promoted public API import resolution;
- design-doc and Rack test ownership;
- source ruff gate;
- package pyright gate;
- corpus archive packaging policy.

## References

- KiCad file formats: https://dev-docs.kicad.org/en/file-formats/
- KiCad source format code: https://gitlab.com/kicad/code/kicad/-/tree/master/common/
- Architecture decisions: `docs/adrs/`
- Public API design docs: `docs/design/api/`
- Release signoff status: `docs/design/quality-signoff-status.md`
