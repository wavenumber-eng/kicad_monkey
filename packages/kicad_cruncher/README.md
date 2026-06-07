# KiCad Cruncher

`kicad-cruncher` is a cross-platform command-line application for KiCad design
workflows. It consumes the public `kicad-monkey` package and keeps higher-level
CLI behavior outside the core parser package.

The public commands generate KiCad-native design review bundles, PCB SVG/STEP
review artifacts, and initial BOM/PnP/JLC manufacturing outputs from public
`kicad-monkey` parsers/renderers.

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
uv sync --extra test
uv run kicad-cruncher --help
uv run python -m kicad_cruncher version
```

## Commands

Run `kicad-cruncher <command> --help` for command-specific options.

| Command | Purpose | Status |
| --- | --- | --- |
| `bom` | Generate KiCad BOM outputs with shared field alias coalescing, variant-aware DNP handling, grouped JSON/CSV/XLSX review tables, and JLC BOM rows. | Public |
| `design` / `design-review` / `dr` | Generate a design review bundle with KiCad-native design JSON, schematic SVGs, PCB copper-layer SVGs, a manifest, and a README for agents. | Public |
| `jlc` | Generate paired JLCPCB BOM XLSX and CPL XLSX upload workbooks from the shared BOM/PnP normalization layer. | Public |
| `pcb-layer-step` | Generate compact colored STEP models for fixture-alignment checks on one KiCad PCB layer. | Public |
| `pcb-svg` | Generate PCB layer SVG artifacts and configured design views, including geometer-backed assembly HLR overlays. | Public |
| `pnp` | Generate KiCad pick-and-place JSON, CSV, XLSX, and JLC CPL outputs using component-center coordinates relative to the aux axis/drill-place file origin. | Public |
| `version` | Print `kicad-cruncher` and controlled dependency versions. | Public |

The `design` command writes to `./output/design/` by default. Its aliases
`design-review` and `dr` produce the same output:

```powershell
kicad-cruncher design board.kicad_pro
kicad-cruncher design-review board.kicad_pro
kicad-cruncher dr board.kicad_sch --no-indexes
kicad-cruncher design -o output/design
```

The design review output includes `<input-stem>_design.json`,
`design_review_manifest.json`, `README.md`, enriched black-and-white schematic
SVGs under `schematics/`, and one PCB review SVG per copper layer under
`pcb/copper_layers/` when a board is present. Schematic review SVGs preserve
the `kicad-monkey` enrichment metadata while applying the
`kicad_cruncher.design_review.schematic_svg.a0` black-and-white role theme.
PCB review SVGs include the copper layer, `Edge.Cuts`, and `kicad-monkey`
enriched drill/slot records.
Plated pads and vias, and KiCad `np_thru_hole` mechanical pads, are
distinguished with `data-hole-plating` and `data-hole-kind` attributes.
Design-review styling colors those existing records in place: plated drills are
blue, plated slots are cyan, non-plated drills are red, and non-plated slots are
orange. KiCad Cruncher does not add a second drill/slot overlay or duplicate
the `kicad-monkey` plating metadata.

The `pcb-svg` command writes to `./output/pcb-svg/` by default and uses
`pcb.svg.config` JSON/JSONC configs compatible with the A0 PCB SVG view
contract. This remains a preview feature in the `2026.6.6` release: SVG structure,
virtual-layer metadata, default views, and config controls may change as more
real-world boards are tested.

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
overlays or footprint-bound fallbacks. Default assembly views use pad bounding
boxes with aspect-preserving fitted
`ASSEMBLY_DESIGNATORS_TOP`/`ASSEMBLY_DESIGNATORS_BOTTOM` labels drawn above the
75% opacity HLR/bounds overlay. Assembly labels are blue by default and rotate
90 degrees in the configurable `ccw`/`cw` direction when their fitted bounds
exceed the configurable height/width aspect threshold. Assembly designator
style overrides can target exact refs, prefixes, wildcards, or ranges.

The `bom`, `pnp`, and `jlc` commands provide initial KiCad manufacturing output
support. They share a documented `bom.config` JSONC file with a top block
comment, field aliases for manufacturer/part/JLC/value/description/footprint
parameters, variant selection, DNP policy, grouping fields, PnP table fields,
and output path templates.

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
kicad-cruncher pcb-layer-step --init-config --config pcb-layer-step.json
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
`docs/adrs/ADR-0001-versioning-tagging-release-policy.md`. The intended release
workflow is GitHub Actions plus PyPI Trusted Publishing/OIDC. Local Twine upload
is fallback only.
