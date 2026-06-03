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

This slice now uses `tests/corpus/kicad/projects/hlr_test/` as the dedicated
HLR pose fixture. The old `tests/corpus/kicad/board_svg/input/led_component/`
fixture has been removed from the filesystem and from the L3 workflow contract.
Generated/editor state in `hlr_test` was removed before adding the fixture.

Current unrelated fixture edits remain local user work in progress:

- `tests/corpus/kicad/projects/cutout_test/cutout_test.kicad_pro`
- `tests/corpus/kicad/projects/yoshi_mainboard/input/11-10080__yoshi-mainboard__A.kicad_pro`
- `tests/corpus/kicad/projects/yoshi_mainboard/input/11-10080__yoshi-mainboard__A.kicad_prl`

These should not be staged with the HLR implementation unless explicitly
requested.

## Findings

### Previous Kicad Cruncher HLR Defect

Before this slice, HLR rendering was split across:

- `src/py/kicad_cruncher/kicad_cruncher_cmd_pcb_svg.py`
- `src/py/kicad_cruncher/kicad_cruncher_pcb_svg_projection.py`

The old `_model_transform_matrix()` sent only model scale to Geometer.
`_model_point_to_svg()` then applies model XY offset plus a 2D footprint
translation/rotation after Geometer projection.

That is not KiCad's transform chain. It ignores:

- model rotation
- model Z offset
- footprint side and side flip
- board/copper Z placement
- KiCad's inverted board Y axis in 3D/STEP space
- footprint pose in the Geometer cache key

The result could only work accidentally for simple, unrotated models.

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

The current Python HLR result exposes flattened SVG-ready primitives only:

- `modes.simple.segments`
- `modes.simple.arcs`
- `modes.detail.segments`
- `modes.detail.arcs`

It does not expose projected loop IDs, face IDs, edge classifications, polygon
regions, or an explicit silhouette/outer-contour result. On `hlr_test`,
`edge_v_sharp=True` produces the current simple/detail geometry, including a
small visible model feature inside the outer body outline. `edge_v_outline=True`
without visible sharp edges returns no geometry for this fixture. That means the
current Geometer API cannot directly ask for "outer projected component
perimeter only".

Preferred fix: add a Geometer outer-contour or classified-loop mode that returns
the projected outside silhouette separately from visible internal edges. A
Kicad Cruncher post-process is possible as a stopgap, but it would infer loops
from flattened segments/arcs and drop contained loops heuristically. That can
work for closed interior features, but it is weaker when the model perimeter is
fragmented or internal visible edges are not clean closed loops.

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

- [x] Remove `hlr_test-backups/` and generated/editor state before committing
  the fixture.
- [x] Remove the old LED fixture from the corpus and tests.
- [x] Add `hlr_test` to the L3 PCB SVG corpus after cleanup.
- [x] Add a focused test that locates `U1`, reads the model pose fields, and
  asserts the computed matrix values for this exact fixture.
- If a KiCad CLI path is available for this fixture, add an optional comparison
  artifact that exports KiCad's 3D/STEP placement for manual audit. The required
  test should not depend on a private KiCad build.

### 2. Build A Single KiCad Model Pose Helper

Created private helper module `kicad_cruncher_pcb_model_pose.py`, with:

- [x] `KiCadModelPose`: normalized footprint/model pose fields and derived side.
- [x] `kicad_model_pose(...)`: row-major 4x4 matrix in millimeters.
- [x] `board_world_to_svg(...)`: convert Geometer projected board-world XY back
  to SVG board coordinates.
- [x] `model_bounds_to_svg_rect(...)`: convert Geometer transformed model
  bounds into an SVG rectangle.
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

The first automated fixture test covers the top-side offset/rotation case. Add
bottom-side, scale, and model-rotation synthetic cases before declaring this API
release-final. Record the sign convention in the design doc once stable.

### 3. Refactor HLR Rendering To Use Full Pose

- [x] Send the full model-to-board-world matrix to Geometer.
- [x] Stop adding model offset and footprint XY/rotation after Geometer
  projection.
- [x] Convert projected Geometer points from KiCad board-world XY to SVG board
  XY in one place.
- [x] Include the full pose signature in the HLR cache key, including footprint
  position, footprint rotation, side, model offset, model rotation, model scale,
  and board thickness terms.
- [x] Keep Geometer options and simple/detail extraction unchanged until pose is
  validated.

### 4. Validate `hlr_test` First

- [x] Generate top assembly HLR and top pin-1 view for `hlr_test`.
- [ ] Manually verify the HLR outline lands around the footprint pads/body in
  SVG coordinates.
- [x] Add assertions on SVG extents for `U1` so the test fails on obvious
  translation, rotation, side, or Y-axis mistakes.
- Only after this passes, regenerate and inspect taillight, yoshi, speedy, and
  charge indicator.

### 5. Fix Bounding Box Virtual Layer

Implemented two explicit bounding-box modes after the first HLR pose fix:

- [x] `model_bounds`: use `geometer.model_bounds()` with the same full KiCad pose
  matrix, then project the transformed model bounds into the current view.
- [x] `pad_bounds`: for missing models or selected overrides, compute a 2D
  component box from copper-bearing pads only, including SMT pads and
  through-hole pad copper.
- [x] default views now include separate top/bottom model bounding box and
  top/bottom pad bounding box views, so manual review can compare the two
  bounding-box sources directly.

Do not use `footprint.get_bounds()` for the primary assembly bounding-box mode.
It includes non-copper graphics/text and is not the requested fallback.

### 6. Component And Group Selection

Keep Altium Cruncher terminology where practical:

- [x] default view mode: `detail`, `simple`, `bounding_box`, or `none`
- [x] component overrides by exact designator continue to work
- [x] exact component overrides now apply to assembly HLR projection and
  `assembly_hlr` style, including Geometer option knobs
- [ ] add component selector groups after exact overrides are stable

Planned selector shapes:

- exact designator: `U1`
- list: `["U1", "U2", "J1"]`
- designator prefix: `U*`, `J*`
- designator range: `U1-U8`

The selected projection should be able to choose:

- HLR detail
- HLR simple
- transformed model bounding box
- pad bounding box
- none

Final naming should avoid ambiguity between HLR projection modes and bounding
box kinds. A likely contract is `projection` for high-level selection plus
`bounding_box_kind` for `model_bounds` or `pad_bounds`.

### 7. Tests And Signoff

Add tests before broad tuning:

- [x] L3 unit-style test for the `hlr_test` pose matrix.
- [x] L3 workflow test that generates `hlr_test` PCB SVG and asserts visible HLR
  geometry lands near the footprint.
- [x] L3 test for `model_bounds` using Geometer transformed bounds.
- [x] L3 test for `pad_bounds` output on the HLR fixture.
- [ ] L3 test for `pad_bounds` on a no-model synthetic footprint.
- [x] L3 test for exact component override precedence.
- L3 test for selector groups once implemented.
- [x] Regenerate ignored signoff outputs for `hlr_test`, taillight, yoshi,
  speedy, cutout test, and charge indicator.

Run before committing implementation:

```powershell
uv run ruff check src\py\kicad_cruncher tests\L3_public_workflows\test_L3_001_design_workflow.py
uv run pyright
uv run pytest tests\L3_public_workflows\test_L3_001_design_workflow.py -q
```

Current validation on 2026-06-03:

- `uv run pytest tests\L3_public_workflows\test_L3_001_design_workflow.py -q`
  passed: `22 passed`
- `uv run pyright` passed with zero errors.
- `uv run ruff check src\py\kicad_cruncher tests\L3_public_workflows\test_L3_001_design_workflow.py`
  passed.
- `uv run pytest tests\L99_signoff -q` passed: `17 passed`.
- Ignored signoff outputs were regenerated under `output\pcb-svg-signoff\` for
  `hlr_test`, `cutout_test`, `taillight`, `charge_indicator`,
  `speedy_processing_module`, and `yoshi_mainboard`.

Recovered wrap-up request from the interrupted 2026-06-03 session:

- Pin-1 views need global and per-view exclusion overrides. Defaults should
  exclude single-pin parts plus all `R` and `C` designators. Selectors should
  support exact designators, numeric ranges such as `U5-U15`, and full-prefix
  selectors such as `U` or `U*`.
- Pin-1 marker dot size should be configurable as a fraction of the selected
  pad size.
- Add assembly designator virtual layers for top and bottom assembly views.
  Default assembly views should use pad bounding boxes, draw designators on top,
  and fit designator text inside the projected component bounds.
- Use step-model bounds for designator text placement when HLR/model projection
  is selected, and pad bounds when pad-bounds projection is selected.
- Designator orientation should be 0 degrees for 0/180-degree parts and 90
  degrees for 90/270-degree parts. Color and font should be configurable.
- HLR and bounding-box overlays should have configurable opacity, defaulting to
  75%.

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
