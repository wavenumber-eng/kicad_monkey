# PCB SVG Assembly Virtual Layer Fix Plan

Status: active
Last updated: 2026-06-03
Owner: kicad_cruncher

## Goal

Fix the PCB SVG assembly virtual layers so KiCad component STEP models are posed
the same way KiCad poses them, then use that same pose data for transformed model
bounding boxes and component selection modes.

This plan covers the `pcb-svg` command only. Durable behavior must move into
`docs/design/cli/pcb-svg.html` and `docs/contracts/pcb_svg_config.a0.schema.json`
as the implementation lands.

## Current State

Recent PCB SVG cleanup already landed on the local branch:

- `38260e5` includes board outline in the cutout view.
- `13457b1` fixes compositor layer alignment.
- `fead6c7` separates raw physical layer outputs from virtual layer outputs.
- `8e53e6b` adds configurable layer context overlays.
- `d6cf6c4` adds the standalone virtual-layer output toggle.

The latest generated review/signoff outputs are under
`output/pcb-svg-signoff/`. They are ignored outputs and are not release
artifacts.

The current dirty/untracked fixture state is intentional user work in progress:

- `tests/corpus/kicad/projects/hlr_test/` is untracked and contains the new
  one-component pose fixture.
- `hlr_test` currently contains `hlr_test-backups/*.zip`; remove these before
  committing the fixture.
- `cutout_test` and `yoshi_mainboard` have local fixture edits outside this
  planning change.

## Findings

### Current Kicad Cruncher HLR Defect

Current HLR rendering is split across:

- `src/py/kicad_cruncher/kicad_cruncher_cmd_pcb_svg.py`
- `src/py/kicad_cruncher/kicad_cruncher_pcb_svg_projection.py`

The current `_model_transform_matrix()` sends only model scale to Geometer.
`_model_point_to_svg()` then applies model XY offset plus a 2D footprint
translation/rotation after Geometer projection.

That is not KiCad's transform chain. It ignores:

- model rotation
- model Z offset
- footprint side and side flip
- board/copper Z placement
- KiCad's inverted board Y axis in 3D/STEP space
- footprint pose in the Geometer cache key

The result can only work accidentally for simple, unrotated models.

### KiCad Source Findings

KiCad stores footprint model fields in `pcbnew/footprint.h` as:

- `m_Scale`: dimensionless scale
- `m_Rotation`: degrees
- `m_Offset`: millimeters

The STEP exporter path is the best semantic oracle for Geometer because
Geometer consumes STEP geometry in millimeters:

- `C:\eli\kicad_build\kicad\pcbnew\exporters\step\exporter_step.cpp`
- `C:\eli\kicad_build\kicad\pcbnew\exporters\step\step_pcb_model.cpp`

`STEP_PCB_MODEL::getModelLocation()` composes:

1. footprint XY translation as `(x, -y, 0)` because KiCad 3D/STEP space inverts
   board Y
2. side-dependent Z placement using board body and copper layer Z
3. footprint Z rotation
4. bottom-side extra X rotation by pi
5. model offset
6. model orientation rotations in `-Z`, `-Y`, `-X` order

The OpenGL 3D viewer agrees on the high-level chain:

- `C:\eli\kicad_build\kicad\3d-viewer\3d_rendering\opengl\render_3d_opengl.cpp`

It composes footprint translation, footprint Z rotation, side flip, model-unit
conversion, then model offset, negative `Z/Y/X` model rotations, and model
scale. For this work, use the STEP exporter path as the numeric model because
it is in millimeters and feeds OCCT shapes like Geometer.

### Geometer Findings

The pinned package is `wn-geometer==2026.5.25`.

Geometer accepts a row-major 4x4 affine `model_transform` with translation in
the final column and final row `[0, 0, 0, 1]`. It applies that transform before
HLR projection and before model bounds:

- `geometer.project_step_hlr(..., model_transform=...)`
- `geometer.model_bounds(..., model_transform=...)`

The source tests for the pinned version verify transformed model bounds. This
means Kicad Cruncher should compute one KiCad model-to-board-world matrix and
send the same matrix to both Geometer HLR and Geometer bounds.

### New HLR Fixture

`tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pcb` contains one top-side
`SOT23-5` footprint:

- designator `U1`
- footprint at `(136.55, 98.05)`
- embedded model `kicad-embed://SOT23-5.STEP`
- model offset `(-0.0292, 1.9, 0.75)`
- model scale `(1, 1, 1)`
- model rotation `(-90, 0, 90)`

This is the first gate. Do not tune against yoshi, speedy, taillight, or charge
indicator until this one-component fixture is correct.

## Ownership Decision

Phase 1 ownership stays in `kicad_cruncher`:

- it composes `kicad_monkey` parsed PCB/model data with Geometer-specific matrix
  input and PCB SVG view semantics
- it avoids adding a Geometer dependency or app-level rendering policy to
  `kicad_monkey`

If the pure KiCad model-pose calculation proves reusable by `kicad_cruncher`,
the 3D viz work, and future tools, promote only the pure transform helper to
`kicad_monkey` later. That helper must stay independent of Geometer and output
plain matrices/pose metadata.

## Implementation Plan

### 1. Fixture Hygiene And Oracle Setup

- Remove `hlr_test-backups/` and generated/editor state before committing the
  fixture.
- Add `hlr_test` to the L3 PCB SVG corpus only after cleanup.
- Add a focused test that locates `U1`, reads the model pose fields, and asserts
  the computed matrix values for this exact fixture.
- If a KiCad CLI path is available for this fixture, add an optional comparison
  artifact that exports KiCad's 3D/STEP placement for manual audit. The required
  test should not depend on a private KiCad build.

### 2. Build A Single KiCad Model Pose Helper

Create a private helper module first, likely
`kicad_cruncher_pcb_model_pose.py`, with:

- `KiCadModelPose`: normalized footprint/model pose fields and derived side
- `kicad_model_to_board_world_matrix(...)`: row-major 4x4 matrix in millimeters
- `board_world_to_svg_xy(...)`: convert Geometer projected board-world XY back
  to SVG board coordinates
- tests for top side, bottom side, model rotations, model offsets, and scale

The matrix should follow KiCad STEP exporter semantics:

- source model STEP coordinates
- model scale
- model rotations in negative Z/Y/X order
- model offset
- footprint side transform
- footprint Z rotation
- footprint XY translation with KiCad 3D Y inversion
- board/copper Z offset

Record any sign convention discovered during fixture validation in the helper
docstring and in the design doc once stable.

### 3. Refactor HLR Rendering To Use Full Pose

- Send the full model-to-board-world matrix to Geometer.
- Stop adding model offset and footprint XY/rotation in `_model_point_to_svg()`.
- Convert projected Geometer points from KiCad board-world XY to SVG board XY in
  one place.
- Include the full pose signature in the HLR cache key, including footprint
  position, footprint rotation, side, model offset, model rotation, model scale,
  and relevant board Z terms.
- Keep Geometer options and simple/detail extraction unchanged until pose is
  validated.

### 4. Validate `hlr_test` First

- Generate top assembly HLR and top pin-1 view for `hlr_test`.
- Verify the HLR outline lands around the footprint pads/body in SVG coordinates.
- Add assertions on SVG bounding extents for `U1` so the test fails on obvious
  translation, rotation, side, or Y-axis mistakes.
- Only after this passes, regenerate and inspect taillight, yoshi, speedy, and
  charge indicator.

### 5. Fix Bounding Box Virtual Layer

Implement two explicit bounding-box modes after HLR pose is correct:

- `model_bounds`: use `geometer.model_bounds()` with the same full KiCad pose
  matrix, then project the transformed model bounds into the current view.
- `copper_bounds`: for missing models or selected overrides, compute a 2D
  component box from copper-bearing pads only, including SMT pads and
  through-hole pad copper.

Do not use `footprint.get_bounds()` for the primary assembly bounding-box mode.
It includes non-copper graphics/text and is not the requested fallback.

### 6. Component And Group Selection

Keep Altium Cruncher terminology where practical:

- default view mode: `detail`, `simple`, `bounding_box`, or `none`
- component overrides by exact designator continue to work
- add component selector groups after exact overrides are stable

Planned selector shapes:

- exact designator: `U1`
- list: `["U1", "U2", "J1"]`
- designator prefix: `U*`, `J*`
- designator range: `U1-U8`

The selected projection should be able to choose:

- HLR detail
- HLR simple
- transformed model bounding box
- copper bounding box
- none

Final naming should avoid ambiguity between HLR projection modes and bounding
box kinds. A likely contract is `projection` for high-level selection plus
`bounding_box_kind` for `model_bounds` or `copper_bounds`.

### 7. Tests And Signoff

Add tests before broad tuning:

- L3 unit-style test for the `hlr_test` pose matrix.
- L3 workflow test that generates `hlr_test` PCB SVG and asserts visible HLR
  geometry lands near the footprint.
- L3 test for `model_bounds` using Geometer transformed bounds.
- L3 test for `copper_bounds` on a no-model synthetic footprint.
- L3 test for component override precedence.
- L3 test for selector groups once implemented.
- Regenerate ignored signoff outputs for `hlr_test`, taillight, yoshi, speedy,
  and charge indicator.

Run before committing implementation:

```powershell
uv run --extra test ruff check src\py\kicad_cruncher tests\L3_public_workflows\test_L3_001_design_workflow.py
uv run --extra test pyright src\py\kicad_cruncher tests\L3_public_workflows\test_L3_001_design_workflow.py
uv run --extra test pytest tests\L3_public_workflows\test_L3_001_design_workflow.py -q
```

Run L99 before release-facing docs/contracts are declared complete:

```powershell
uv run --extra test pytest tests\L99_signoff -q
```

### 8. Documentation Updates

When behavior lands:

- Update `docs/design/cli/pcb-svg.html` HLR section with pose ownership,
  Geometer transform behavior, and bounding-box semantics.
- Update `docs/contracts/pcb_svg_config.a0.schema.json` for any new selector or
  bounding-box fields.
- Update README examples only after the command shape is stable.
- Keep this plan local under `docs/plans/`; move completed decisions into the
  design doc before public release.

## Completion Criteria

- `hlr_test` pose is correct by automated test and manual SVG inspection.
- Taillight, yoshi, speedy, and charge indicator regenerate with aligned HLR and
  bounding-box overlays.
- Component-level projection and bounding-box selection are documented and
  tested.
- `docs/design/cli/pcb-svg.html`, the A0 config contract, and L3/L99 tests agree
  on the released behavior.
