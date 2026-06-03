# Changelog

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
