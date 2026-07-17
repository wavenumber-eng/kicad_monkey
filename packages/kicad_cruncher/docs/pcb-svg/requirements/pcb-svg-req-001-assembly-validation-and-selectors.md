+++
type = "requirement"
id = "pcb-svg-req-001-assembly-validation-and-selectors"
domain = "pcb-svg"
status = "active"
title = "PCB SVG assembly HLR follow-up validation and selector work remains tracked"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
verification_status = "unverified"
design_refs = [
  "docs/design/cli/pcb-svg.html",
  "docs/contracts/pcb_svg_config.a0.schema.json",
]
+++

# PCB SVG Assembly Follow-Up

The released PCB SVG design docs and contract describe the current assembly HLR,
model-bounds, pad-bounds, pin-1 marker, and assembly-designator behavior. The
following follow-up obligations recovered from the deleted
`docs/plans/pcb-svg-assembly-virtual-layer-fix.md` plan remain active:

- Manually verify the `hlr_test` HLR outline against footprint pads and body in
  generated SVG coordinates.
- Add an L3 `pad_bounds` test for a synthetic no-model footprint.
- Add component selector groups after exact component overrides are stable.
- Keep selector shapes explicit: exact designator, list, wildcard/prefix, and
  designator range.
- Continue to keep `docs/design/cli/pcb-svg.html`,
  `docs/contracts/pcb_svg_config.a0.schema.json`, and L3/L99 tests synchronized
  before release-facing behavior changes.

The completed pin-1, designator, orientation, opacity, and bounds-placement
items from the old plan are represented in the current PCB SVG design doc and
tests; this requirement tracks only the remaining unfinished items.
