# Changelog

## 2026.6.13

- Update the controlled `kicad-monkey` dependency to `2026.6.13` so
  `design`/`design-review` use KiCad-parity hierarchical netlist generation.
- Add the sanitized 4-channel backplane fixture to the regular public workflow
  corpus for design JSON, schematic SVG, and PCB SVG coverage.
- Refresh copied-corpus design JSON count assertions where KiCad-parity
  component materialization now omits duplicate/non-materialized rows.
- Keep `wn-geometer==2026.6.10`.

## 2026.6.11

- Move all generated command configs to documented JSONC emitted from
  structured payload/comment metadata.
- Add per-field generated comments and explicit enum option comments for
  BOM/PnP/JLC, `pcb-svg`, `pcb clean`, and `pcb-layer-step` configs.
- Remove legacy config fallback behavior and old schema aliases. BOM/PnP/JLC
  now require `kicad_cruncher.bom.config.v1`, and `pcb-layer-step` now uses
  `pcb-layer-step.jsonc` with the v2 schema.
- Move fixture STEP color/body policy under `features.*`, using
  `step_body_name`, per-feature `thickness_bias_mm`, and
  `features.component_pads.highlight_rules`.
- Add scoped drill modes for selected component pads, other component pads,
  free pads, and vias.
- Keep controlled dependency pins at `kicad-monkey==2026.6.10` and
  `wn-geometer==2026.6.10`.

## 2026.6.10

- Update the controlled dependencies to `kicad-monkey==2026.6.10` and
  `wn-geometer==2026.6.10`, including the OCCT V8-backed projection stack.
- Rename PCB SVG assembly projection output from `simple` to `outline` and use
  Geometer's mesh-shadow outline algorithm by default for assembly silhouettes.
- Generate `ASSEMBLY_HLR_TOP_OUTLINE` and `ASSEMBLY_HLR_BOTTOM_OUTLINE`
  virtual layer tokens while accepting legacy `*_SIMPLE` tokens and `simple`
  projection values as aliases.
- Keep `detail`, `bounding_box`, `model_bounds`, and `pad_bounds` projection
  behavior unchanged.

## 2026.6.7

- Add the public `kicad` workstation helper command for KiCad install
  discovery, running-process inspection, launch, stop, and preference path
  reporting.
- Add the short `kcr` console alias for the existing `kicad-cruncher` entry
  point.
- Add `kicad launch --new` so automation can start the KiCad project manager
  without reloading the previous project.
- Gate destructive process termination behind `kicad stop --all`; no-argument
  `kicad stop` remains a dry-run process report.
- Adopt accepted design-doc status markers and update CI/release workflows to
  current checkout/setup-uv action versions.

## 2026.6.6

- Add the first public KiCad IPC plugin and daemon framework, including plugin
  install/status/uninstall commands, daemon state discovery, loopback host
  policy, and a browser tool-center shell.
- Route PCB clean through the daemon/plugin path and add KiCad IPC mutation
  request/apply coverage for documentation-layer cleanup under KiCad undo.
- Move KiCad plugin ownership out of appz into `kicad-cruncher`; appz now keeps
  only a workspace setup wrapper.
- Codify plugin metadata namespace policy and installer diagnostics for KiCad
  IPC API and Python interpreter setup.

## 2026.6.4

- Add initial public BOM, PnP, and JLC manufacturing output support with shared
  JSONC config, field alias coalescing, variant-aware DNP handling, grouped
  BOM review outputs, and JLC BOM/CPL XLSX generation.
- Add `pcb-layer-step` fixture-alignment STEP output for KiCad PCB layers,
  including configurable copper bodies, board outline/cutout bodies, drill
  overlays, fused copper review output, and pad/via trace clipping.
- Keep `pcb-svg` as a preview feature while continuing real-board coverage for
  configured views, assembly overlays, virtual layers, and designator rendering.
- Release `kicad-cruncher` version `2026.6.4` against
  `kicad-monkey==2026.6.3` and `wn-geometer==2026.6.4`.

## 2026.6.3

- Release `kicad-cruncher` against `kicad-monkey==2026.6.3`.
- Add `pcb-svg` preview outputs for A0 PCB layer/view SVG generation, including
  pin-1 markers, assembly HLR/bounds overlays, assembly designators, muted
  assembly copper colors, and smoother configurable derived board-outline arc
  sampling.
- Mark `pcb-svg` as a preview feature: SVG structure, virtual-layer metadata,
  and `pcb.svg.config` controls may change in future releases based on real
  board testing.

## 2026.5.31

- Initial public repository setup for `kicad-cruncher`.
- Add the `design` command for generating KiCad-native design JSON through the
  public `kicad-monkey` API.
- Add the `pcb-svg` command for A0 PCB SVG layer outputs and configured design
  views, including `wn-geometer` assembly HLR overlays.
- Add release-facing `pcb-svg` controls for pin-1 selector exclusions, relative
  pin-1 marker sizing, aspect-preserving assembly designator virtual layers,
  aspect-threshold designator rotation with configurable direction and selector
  overrides, pad-bounds default assembly views, muted assembly copper colors,
  smoother configurable derived board-outline arc sampling, and 75% default
  HLR/bounds overlay opacity.
- Add public CI, release, Rack, documentation, and source-hygiene signoff gates.
