# KiCad Monkey Tests

This suite follows the package-local corpus model:
- persistent file-backed fixtures resolve from the archive selected by `KM_CORPUS`
- the default public-repo carrier is `tests/corpus/kicad.zip`
- synthetic tests may stay local
- `input/`, `reference_output/`, and `output/` are the standard case buckets
- `output/` is transient

`tests/rack.py` is a thin delegating wrapper to the installed `wn-rack` CLI. It
is not a local fork of the rack framework.

## Quick Start

```powershell
cd kicad_monkey
uv sync --group dev
uv run --extra test python scripts/kicad_corpus_archive.py restore --check-zip
uv run python tests/rack.py list
```

The harness safely extracts the selected archive and publishes its directory
internally as `KM_CORPUS_ROOT`. Set `KM_CORPUS` to another reviewed
`kicad.zip` when a bot or developer needs to override the package archive. A
directory containing `kicad/` is accepted only for fixture authoring.

Run a stratum:

```powershell
uv run python tests/rack.py run L1_parsing
```

Regenerate the manifest-driven SVG review page:

```powershell
uv run python tests/generate_manifest_svg_review.py
```

Generate the downstream KiCad CLI vs IR SVG comparison report:

```powershell
uv run python tests/generate_cli_svg_comparison.py
```

## Strata

- `L0_foundation`: S-expression parsing and low-level syntax behavior
- `L1_parsing`: core parsing, round-trip, and shared-corpus readiness
- `L2_tools`: extraction, splitting, merging, indexing
- `L3_rendering`: SVG and 2D rendering
- `L4_applications`: higher-level workflows and external-tool validation

## Lanes

KiCad follows the shared lane model:
- `fast`: default smoke/structural lane
- `full`: broader routine-validation lane
- `strict`: heavier or stricter validation lane

The early KiCad strata are still converging on how much data each lane should cover, but the lane names are fixed now.

## Corpus Layout

Examples:
- `${KM_CORPUS_ROOT}/kicad/common/board/input`
- `${KM_CORPUS_ROOT}/kicad/common/footprints/input`
- `${KM_CORPUS_ROOT}/kicad/common/reference_symbols/input`
- `${KM_CORPUS_ROOT}/kicad/common/reference_schematics/input`
- `${KM_CORPUS_ROOT}/kicad/common/reference_worksheets/input`
- `${KM_CORPUS_ROOT}/kicad/pcb_roundtrip_features/input`

Keep new persistent assets under `tests/corpus/kicad/...` with the mirrored
corpus layout unless they are synthetic/local-only by design.

Generated visual-review outputs belong under the owning stratum's ignored
`output/<domain>/` tree. The extracted `KM_CORPUS_ROOT` is immutable runtime
input. Persistent reference updates must target an explicit writable fixture-
authoring tree and then rebuild the reviewed ZIP.

Real-world `projects/*_assembly` entries are assembly-procedure documentation
projects. They are valid schematic SVG/IR cases, but their PCB files are
intentionally empty/header-only and are not useful `board_svg` review cases.
