# Changelog

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
