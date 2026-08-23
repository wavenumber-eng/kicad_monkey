# KiCad Cruncher

`kicad-cruncher` is a cross-platform command-line application for KiCad design
workflows. It consumes the public `kicad-monkey` package and keeps higher-level
CLI behavior outside the core parser package.

The public commands generate KiCad-native design review bundles, project-local
library bundles, cleaned library-ingestion bundles, project health reports, PCB
SVG/STEP review artifacts, plugin tooling, and BOM/PnP/JLC manufacturing
outputs from public `kicad-monkey` parsers/renderers.

## Install

Install `uv` first if it is not already available:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
```

On macOS or Linux:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

Install as a uv tool:

```powershell
uv tool install kicad-cruncher
uv tool update-shell
kicad-cruncher --help
```

During local development:

```powershell
git clone https://github.com/wavenumber-eng/kicad_monkey.git
cd kicad_monkey
uv sync --all-packages --all-extras
uv run --package kicad-cruncher kicad-cruncher --help
uv run --package kicad-cruncher python -m kicad_cruncher version
```

Within the universal wheel's Python module path, Windows x64 selects the
installed Monkey wheel's `kicad-monkey-native` sidecar for PCB physical-base
SVG, compiled schematic graphs, and version-E netlists; a native failure is
terminal and is never retried through the corresponding Python implementation.
Linux and macOS retain the Python providers for that module path. The
`KICAD_CRUNCHER_NATIVE_DESIGN_FACTS=1` and
`KICAD_CRUNCHER_NATIVE_PHYSICAL=1` environment switches are development/test
opt-ins on other platforms, not production-support declarations.

### Pure-Rust design CLI on Windows x64

Each Cruncher GitHub release now includes a hash-manifested
`kicad-cruncher-<version>-windows-x64.zip`. Extract it and put that directory
before Python tool-script directories on `PATH` to select the pure-Rust
`kicad-cruncher.exe` and `kcr.exe`. These executables currently own
`design`, `design-review`, `dr`, and `--version`; they generate the complete
review bundle without a Python interpreter.

For a source checkout, the equivalent tested install is:

```powershell
cargo install --locked `
  --path packages/kicad_cruncher/src/rs/kicad-cruncher-cli `
  --root .native-cruncher --force --bins
$env:PATH = "$PWD\.native-cruncher\bin;$env:PATH"
kcr design-review board.kicad_pro
```

The universal Python wheel remains the cross-platform distribution for the
full command set below and retains its public `kicad-monkey` dependency. Until
another command receives its own Rust vertical slice, invoke it explicitly as
`python -m kicad_cruncher <command>` when the native directory is first on
`PATH`.

## Commands

Run `python -m kicad_cruncher <command> --help` for the full Python command
set. The promoted Rust executable accepts the design aliases documented above.

| Command | Purpose | Status |
| --- | --- | --- |
| `bom` | Generate KiCad BOM outputs with shared field alias coalescing, variant-aware DNP handling, grouped JSON/CSV/XLSX review tables, and JLC BOM rows. | Public |
| `daemon` | Run the local plugin daemon and browser tool center used by KiCad IPC plugin workflows. | Public |
| `design` | Generate a design review bundle with KiCad-native design JSON, schematic SVGs, PCB copper-layer SVGs, a manifest, and a README for agents. Aliases: `design-review`, `dr`. | Public |
| `health` | Scan active project assets, model references, and project-listed local libraries without recursively importing unrelated library folders. | Public |
| `jlc` | Generate paired JLCPCB BOM XLSX and CPL XLSX upload workbooks from the shared BOM/PnP normalization layer. | Public |
| `kicad` | Inspect local KiCad installs, running KiCad-family processes, preferences, launch commands, and opt-in stop plans. | Public |
| `lib-extract` | Generate a cleaned lib_cruncher/Alexandria ingestion bundle with stripped symbols, footprints, optional models, and separated metadata JSON. Alias: `library-extract`. | Public |
| `megamaid` | Aggressively dissect a KiCad project into library artifacts, embedded assets, models, design review artifacts, netlists, SVGs, manifests, and README output. | Public |
| `pcb` | Run PCB utility commands, including config-driven PCB cleanup planning and explicit direct-file apply. | Public |
| `pcb-layer-step` | Generate compact colored STEP models for fixture-alignment checks on one KiCad PCB layer. | Public |
| `pcb-svg` | Generate PCB layer SVG artifacts and configured design views, including geometer-backed assembly HLR overlays. | Public |
| `plugin` | Install, inspect, and remove bundled KiCad IPC plugin packages. | Public |
| `pnp` | Generate KiCad pick-and-place JSON, CSV, XLSX, and JLC CPL outputs using component-center coordinates relative to the aux axis/drill-place file origin. | Public |
| `project` | Scaffold a new KiCad project (`.kicad_pro` + top-level `.kicad_sch`, optional embedded worksheet, library tables, PCB, title-block metadata, and text variables) from flags, a JSONC config, or the interactive `--tui` form. | Public |
| `project-lib` | Extract metadata-preserving project-local symbol and footprint libraries, defaulting to `./local-library/` and updating project library tables by default. Aliases: `project-library`, `project-local-lib`, `local-library`. | Public |
| `schematic` | Run schematic utility commands, currently exposing the deferred schematic cleanup planning stub. | Public |
| `version` | Print `kicad-cruncher` and controlled dependency versions. | Public |

The `design` command writes to `./output/design/` by default. Its aliases
`design-review` and `dr` produce the same output:

```powershell
kicad-cruncher design board.kicad_pro
kicad-cruncher design-review board.kicad_pro
kicad-cruncher dr board.kicad_sch --no-indexes
kicad-cruncher design -o output/design
```

The design review output includes `<input-stem>_design.json`, the exact
`<input-stem>_compiled_schematic_graph.json` embedded in that Design JSON,
`design_review_manifest.json`, `README.md`, enriched black-and-white schematic
SVGs under `schematics/`, and one PCB review SVG per copper layer under
`pcb/copper_layers/` when a board is present. Schematic review SVGs preserve
the `kicad-monkey` enrichment metadata while applying the
`kicad_cruncher.design_review.schematic_svg.a0` black-and-white role theme.
Each schematic SVG identifies its canonical graph page and carries forward and
reverse indexes between source-owned SVG ids and compiled-graph targets. Agents
should join with `page_occurrence_ref + artifact_key + element_id`, then follow
terminal/local-net and hierarchy-binding refs; names, text, geometry, and DOM
order are not connectivity keys. `--no-indexes` removes only the optional
legacy Design JSON indexes and retains this graph linkage.
PCB review SVGs include the copper layer, `Edge.Cuts`, and `kicad-monkey`
enriched drill/slot records.
Plated pads and vias, and KiCad `np_thru_hole` mechanical pads, are
distinguished with `data-hole-plating` and `data-hole-kind` attributes.
Design-review styling colors those existing records in place: plated drills are
blue, plated slots are cyan, non-plated drills are red, and non-plated slots are
orange. KiCad Cruncher does not add a second drill/slot overlay or duplicate
the `kicad-monkey` plating metadata.

For large boards, `design`/`design-review`/`dr` still produce full design JSON
and netlist artifacts, so they intentionally materialize broad project state.
The PCB review SVG pass reuses a command-scoped cached board render state while
writing one SVG per copper layer. On public corpus measurements with
`kicad-monkey 2026.7.17`, this mainly helps boards with several copper layers;
the full design JSON artifact can still dominate total runtime on smaller
layer counts.

The `pcb-svg` command writes to `./output/pcb-svg/` by default and uses
`pcb.svg.config` JSON/JSONC configs compatible with the A0 PCB SVG view
contract. This remains a preview feature: SVG structure,
virtual-layer metadata, default views, and config controls may change as more
real-world boards are tested.

Artifact publication is transactional: failed rendering leaves a new output
directory absent and preserves an existing output tree byte-for-byte. When the
project-adjacent `pcb.svg.config` is missing, the command intentionally authors
that config template before rendering; it is configuration input and is not
part of the transient artifact transaction.

```powershell
kicad-cruncher pcb-svg board.kicad_pcb
kicad-cruncher pcb-svg project.kicad_pro --views assembly-top
kicad-cruncher pcb-svg board.kicad_pcb --config pcb.svg.config -o output/pcb-svg
```

`pcb-svg` composes KiCad Monkey enriched physical layer SVG with explicit
virtual layers. `BOARD_OUTLINE` and `BOARD_CUTOUTS` are synthesized from closed
`Edge.Cuts` regions, with derived arc/curve/circle smoothness controlled by
`styles.board_outline.max_*_segment_mm`, `DRILLS` and `SLOTS` preserve KiCad
Monkey hole metadata, `PIN1_TOP`/`PIN1_BOTTOM` add configurable pad-linked
marker groups, and
`ASSEMBLY_HLR_TOP`/`ASSEMBLY_HLR_BOTTOM` append geometer-backed STEP hidden-line
overlays or footprint-bound fallbacks. The assembly silhouette mode is named
`outline` and uses Geometer's mesh-shadow outline algorithm by default. Use
`outline`; legacy `simple` config values are no longer accepted. Default assembly views include
only top and bottom assembly outputs. They include board cutouts, drills, slots,
pin-1 markers, Geometer `outline` HLR with hole-first bounds fallback, and
aspect-preserving fitted `ASSEMBLY_DESIGNATORS_TOP`/
`ASSEMBLY_DESIGNATORS_BOTTOM` labels drawn above the 75% opacity HLR/bounds
overlay. Assembly labels are blue, bold monospace by default and rotate 90
degrees in the configurable `ccw`/`cw` direction when their fitted bounds exceed
the configurable height/width aspect threshold. Assembly designator style
overrides can target exact refs, prefixes, wildcards, or ranges.

The `bom`, `pnp`, and `jlc` commands provide initial KiCad manufacturing output
support. They share a documented `bom.config` JSONC file with a top block
summary, generated per-field comments, field aliases for
manufacturer/part/JLC/value/description/footprint parameters, variant
selection, DNP policy, grouping fields, PnP table fields, and output path
templates.

```powershell
kicad-cruncher bom project.kicad_pro
kicad-cruncher pnp project.kicad_pro --format xlsx
kicad-cruncher jlc project.kicad_pro --variant ADXL355
kicad-cruncher bom --write-config bom.config
```

PnP and JLC CPL output use the documented `component-center` mode, which maps to
KiCad's footprint placement point relative to the aux axis, also called the
drill/place file origin. Alternate geometric centroid modes are not exposed in
this release.

The `pcb-layer-step` command writes fixture-alignment STEP artifacts under
`./output/pcb-layer-step/` by default. The generated config is intentionally
comment-heavy and can enable tracks, arcs, poured copper, vias, component pads,
board outline/cutout bodies, drill overlays, and fused copper review bodies.

```powershell
kicad-cruncher pcb-layer-step board.kicad_pcb
kicad-cruncher pcb-layer-step project.kicad_pro --doc board.kicad_pcb --layer bottom
kicad-cruncher pcb-layer-step --init-config --config pcb-layer-step.jsonc
```

## Output Layout

Output-producing commands follow the same directory policy:

- when `-o/--output` is omitted, write artifacts under `./output/<command>/`;
- when `-o/--output` is supplied, use that directory directly;
- command modules own filenames inside their command output directory.

## Tests

Run the Rack suite:

```powershell
uv run --extra test rack run --all
```

Run build and installed-console smoke tests:

```powershell
uv run --extra test python -m build
uv run --extra test twine check dist/*
uv run --extra test python tests\support_scripts\install_test.py
```

Rack is the primary local gate. `L0_public_cli` covers startup and command
manifest alignment, `L3_public_workflows` covers fixture-backed command
behavior, and `L99_signoff` covers versioning, docs, contracts, source hygiene,
ruff, and pyright.

## Architecture Docs

- `docs/adrs/` records accepted architecture decisions.
- `docs/design/` records durable interface, command, data-flow, and format
  design notes.
- `docs/contracts/` stores stable manifests and future schemas for public JSON
  or config formats.

## Release Policy

Versioning, tagging, release, and traceability are defined in
`docs/adrs/ADR-0001-versioning-tagging-release-policy.md`. The operator
checklist lives in `docs/release-process.md`. Immutable matching tags for both
packages authorize one manual GitHub Actions dispatch, which publishes through
PyPI Trusted Publishing/OIDC. Local Twine upload is fallback only.
