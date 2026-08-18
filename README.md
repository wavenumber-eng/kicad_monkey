# KiCad Monkey

```text
            ▓▓▓▓▓▓▓▓▓▓
          ▓▓▓▓▓▓▓▓▓▓▓▓▓▓
        ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓
      ▓▓▓▓░░░░░░▓▓░░░░░░▓▓▓▓
  ░░░░▓▓░░░░░░░░░░░░░░░░░░▓▓░░░░
  ░░░░▓▓░░    ░░░░░░    ░░▓▓░░░░
    ░░▓▓░░  ██░░░░░░  ██░░▓▓░░
      ▓▓░░░░░░░░░░░░░░░░░░▓▓
        ▓▓░░░░░░░░░░░░░░▓▓
          ▓▓▓▓░░░░░░▓▓▓▓
  ░░          ▓▓▓▓▓▓
    ▓▓      ▓▓▓▓▓▓▓▓▓▓
    ▓▓▓▓    ▓▓▓▓▓▓▓▓▓▓
      ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓
          ▓▓▓▓░░▓▓░░▓▓▓▓
```

`kicad_monkey` is a focused Python package for KiCad source-file parsing,
round-trip modeling, close-to-format utilities, and IR-backed 2D rendering.

Use it when you need Python code to inspect or modify KiCad files directly:

- read `.kicad_pro`, `.kicad_sch`, `.kicad_pcb`, `.kicad_sym`, and
  `.kicad_mod` files;
- query schematic and PCB objects through typed model facades;
- compile KiCad-native design netlists and design JSON;
- render schematic, PCB, symbol, and footprint views through plotter IR and SVG;
- make focused model edits, then write KiCad files back out.

This package is the low-level parser/model/rendering library. The same
repository also contains the separately published `kicad-cruncher` workflow
CLI under `packages/kicad_cruncher/`. The CLI depends on Monkey; Monkey never
depends on the CLI.

The Rust port has closed its Plotter-IR boundary. The accepted
Windows native-operation transport/package foundation and native base SVG
serializer now start native application delivery; the higher-level, cross-platform Cruncher hard
switch remains staged work. See
the [native Cruncher delivery audit](docs/design/rust-phase6-native-cruncher-audit.html).
The accepted [native base SVG slice](docs/design/rust-native-svg-phase6-slice.html)
is followed by the accepted
[Windows no-fallback PCB physical provider](docs/design/rust-native-physical-provider-phase6-slice.html),
which retains Cruncher-owned enrichment and composition while replacing its
physical-base serialization seam. The accepted
[source-bound native design-facts slice](docs/design/rust-native-design-facts-phase6-slice.html)
next switches the Windows compiled graph and version-E netlist while retaining
Python Design JSON, netlist JSON, presentation, and orchestration.

## Install

For library use inside an existing Python environment:

```powershell
pip install kicad-monkey
```

For development:

```powershell
git clone https://github.com/wavenumber-eng/kicad_monkey.git
cd kicad_monkey
uv sync --extra test
```

To develop and validate both public distributions from one checkout:

```powershell
uv sync --all-packages --all-extras
uv run --package kicad-cruncher kicad-cruncher --help
uv run --all-packages --all-extras python -m pytest tests/cross_package -q
```

## Quick Examples

### Load A Design And Inspect Nets

```python
from kicad_monkey import KiCadDesign

design = KiCadDesign.from_project_file("hardware/demo.kicad_pro")
netlist = design.to_netlist()

for net in netlist.nets:
    terminals = ", ".join(
        f"{terminal.designator}.{terminal.pin}"
        for terminal in net.terminals
    )
    print(f"{net.name}: {terminals}")
```

Save the KiCad-native design JSON used by higher-level review tools:

```python
from pathlib import Path

Path("build").mkdir(parents=True, exist_ok=True)
design.save_json("build/design.json")
```

### Render PCB SVG

```python
from pathlib import Path

from kicad_monkey import KiCadDesign

design = KiCadDesign.from_project_file("hardware/demo.kicad_pro")

out_dir = Path("build/svg")
out_dir.mkdir(parents=True, exist_ok=True)

svg = design.to_pcb_svg(
    layers=["Edge.Cuts", "F.Cu", "F.SilkS"],
    profile="enriched",
)
(out_dir / "front-copper.svg").write_text(svg, encoding="utf-8")
```

Use `profile="oracle"` when comparing against KiCad CLI output. Use
`profile="enriched"` when an app needs metadata on SVG elements. PCB SVG
rendering builds the board render IR before applying `layers=`, so layer
filters reduce output size but do not avoid full PCB parse or IR-build cost.

### Render Every Schematic Sheet Instance

Hierarchical designs can instantiate one `.kicad_sch` file more than once.
`KiCadSchematicInstance` represents each concrete sheet view.

```python
from pathlib import Path

from kicad_monkey import KiCadDesign, render_ir_to_svg

design = KiCadDesign.from_project_file("hardware/demo.kicad_pro")

out_dir = Path("build/schematic-svg")
out_dir.mkdir(parents=True, exist_ok=True)

for sheet in design.schematic_instances():
    doc = design.to_schematic_instance_ir(sheet)
    svg = render_ir_to_svg(doc)
    safe_name = sheet.sheet_name.replace("/", "_").replace("\\", "_")
    (out_dir / f"{sheet.instance_index:02d}_{safe_name}.svg").write_text(
        svg,
        encoding="utf-8",
    )
```

To find where a reused child schematic appears:

```python
for instance in design.schematic_instances_for("hardware/LED_Controller.kicad_sch"):
    print(instance.sheet_name, instance.sheet_path, instance.sheet_instance_path)
```

### Query And Mutate Schematic Objects

The `.objects` property is a live read-only query view over model-owned
objects. Mutate the returned objects, then call `save()`.

```python
from kicad_monkey import KiCadSchematic

schematic = KiCadSchematic.from_file("hardware/demo.kicad_sch")

for symbol in schematic.objects.where("SchSymbol"):
    if symbol.reference.startswith("R"):
        symbol.set_property_value("Value", "10 kOhm")

for label in schematic.objects.where("SchLabel"):
    if label.effects is not None and label.effects.font is not None:
        label.effects.font.size_x = 1.5
        label.effects.font.size_y = 1.5

schematic.save("hardware/demo.edited.kicad_sch")
```

### Query And Mutate PCB Objects

```python
from kicad_monkey import KiCadPcb

board = KiCadPcb.from_file("hardware/demo.kicad_pcb")

for footprint in board.objects.where("Footprint"):
    reference = footprint.get_property_value("Reference")
    if reference.startswith("U"):
        footprint.set_property_value("Reviewed", "yes", create=True)

for text in board.objects.where("GrText", layer="F.SilkS"):
    text.effects.font.size_x = 1.0
    text.effects.font.size_y = 1.0
    text.text = text.text.strip()

board.save("hardware/demo.edited.kicad_pcb")
```

Object queries also work with class objects when you prefer typed imports:

```python
from kicad_monkey import Footprint, KiCadPcb

board = KiCadPcb.from_file("hardware/demo.kicad_pcb")
connectors = [
    footprint
    for footprint in board.objects.where(Footprint)
    if footprint.get_property_value("Reference").startswith("J")
]
```

### Scan Large Files Without Full Model Materialization

Use projection or targeted readers when you only need narrow inventories,
diagnostics, source spans, or selected object families from a large file.

```python
from kicad_monkey import KiCadPcbProjection

projection = KiCadPcbProjection.from_file("hardware/demo.kicad_pcb")

for model_ref in projection.model_references():
    print(model_ref.reference, model_ref.path)

route_count = len(projection.segments()) + len(projection.vias())
print(f"{route_count} route objects")
```

For schematic or custom narrow reads, use the generic targeted reader:

```python
from kicad_monkey import SchSymbol, iter_kicad_objects_from_file

for symbol in iter_kicad_objects_from_file("hardware/demo.kicad_sch", SchSymbol):
    print(symbol.reference, symbol.value)
```

Projection still scans the source file, but it hydrates only the requested
object families. Use `KiCadPcb`, `KiCadSchematic`, or `KiCadDesign` when you
need mutation, rendering, netlisting, full geometry, or cross-document context.
For measured tradeoffs, net-table caveats, and render cost details, see
[Project Workflows And Read-Path Selection](docs/guides/project-workflows.html).

## Testing

Rack is the primary public gate:

```powershell
uv run --extra test python tests/rack.py run L0_foundation
uv run --extra test python tests/rack.py run L99_signoff
```

The parser-first Rust port uses the same Rack orchestrator. Its L0 gate is
split into locked Cargo tests, shared Python/Rust vectors, generated-contract
and executable WASM signoff, and comparative performance evidence:

```powershell
uv run python tests/rack.py run L0_044
uv run python tests/rack.py run L0_045
uv run python tests/rack.py run L0_046
```

Performance cases `L0_047`, `L1_023`, and `L1_024` are advisory and skip in
ordinary fast/full development runs. Run them explicitly in the strict lane:

```powershell
uv run python tests/rack.py run L0_047 --lane strict
uv run python tests/rack.py run L1_023 --lane strict
uv run python tests/rack.py run L1_024 --lane strict
```

Run `npm ci` once to install the pinned TypeSpec toolchain. The executable WASM
test also requires the lock-compatible runner documented in
`docs/design/rust-standard.html`.

`L99_signoff` checks release metadata, changelog coverage, public API contract
resolution, API design-doc ownership, Rack test ownership, corpus archive
hygiene, and the current ruff/pyright ratchet state.

## KiCad Newstroke Webfonts

The authoritative `assets/fonts/` bundle contains the KiCad Stroke family as
Light, Regular, and Bold faces, each upright and italic, in TTF, OTF, WOFF,
and WOFF2 formats. The companion CSS and offline demo exercise electronics
notation, Greek, mathematical symbols, BOM text, and fabrication notes.

Regenerate and verify the complete bundle from the vendored CC0 Newstroke
table with:

```powershell
uv run python tools/package_kicad_stroke_webfont_assets.py
uv run python tools/package_kicad_stroke_webfont_assets.py --check
```

The checked manifest pins the generator, source table, mark, theme, every font
file, CSS, and demo. See [KiCad Newstroke Webfont Bundle](docs/design/kicad-stroke-webfont.html)
for the format and ownership decisions.

The redistributable KiCad corpus is restored locally as
`tests/corpus/kicad.zip`; the archive itself is ignored and is not tracked with
Git LFS. CI restores that archive from the public object URL recorded in
`tests/corpus/kicad.archive.toml` and verifies size and SHA-256 before tests run.
`KICAD_MONKEY_CORPUS_URL` may override the manifest URL for local testing or an
emergency reroute. The loose mirror is ignored locally; test helpers extract the
archive on demand when no external corpus is configured.

Restore and verify the archive before running mandatory corpus-backed Rust
parity gates:

```powershell
uv run --extra test python scripts/kicad_corpus_archive.py restore --check-zip
uv run --extra test python tests/rack.py run L1_029
```

## API Shape

Stable package-root exports are recorded in
`kicad_monkey.kicad_api_contract`. Those names are the public API that
downstream code should rely on. The broader package `__all__` remains a
discovery surface while downstream integrations prove which additional symbols
should become stable public exports.

The public OOP facade groups and supporting public classes are documented under
[docs/design/api](docs/design/api). Use
[Project Workflows And Read-Path Selection](docs/guides/project-workflows.html)
for practical guidance on which API to choose for project, render, inventory,
and large-file workflows. L99 fails when a stable public class or major
interface is missing design documentation or Rack test ownership.

Typical entrypoints:

```python
from kicad_monkey import KiCadDesign, KiCadFootprint, KiCadPcb, KiCadSchematic
from kicad_monkey import KiCadSymbolLib

schematic = KiCadSchematic.from_file("design.kicad_sch")
board = KiCadPcb.from_file("board.kicad_pcb")
design = KiCadDesign.from_project_file("project.kicad_pro")
symbols = KiCadSymbolLib.from_file("library.kicad_sym")
footprint = KiCadFootprint.from_file("package.kicad_mod")
```

For workflow-level API choice, use
[Project Workflows And Read-Path Selection](docs/guides/project-workflows.html)
as the canonical guide.

## Fixture Model

Public fixtures should be redistributable and package-local when possible.
Broader fixture families should use this shape:

- `input/`
- `reference_output/`
- `output/`

`output/` is transient and should stay local or temporary.

## Documentation

- [User Guides](docs/guides)
- [Architecture Decision Records](docs/adrs)
- [Design Notes](docs/design)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)

## License

MIT.
