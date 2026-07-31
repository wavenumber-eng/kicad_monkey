# Changelog

## Unreleased

## 2026.7.31.1

- Fix `project-lib --relink-sources --repair-cache-links` so embedded
  schematic cache `Footprint` defaults match the generated project-local symbol
  libraries, eliminating KiCad `lib_symbol_mismatch` warnings caused by the
  localization step.
- Add cache-body validation to `source_relink.json` and make apply mode block
  on unresolved cache-link, cache-unit, or cache-body issues.
- Add a `--validate-kicad-cli` apply-mode ERC hygiene gate that records
  before/after KiCad schematic ERC JSON, requires zero post-relink
  library-hygiene findings, and verifies ordinary ERC counts are unchanged.
- Harden the ERC hygiene gate so failed `kicad-cli` runs cannot pass by parsing
  stale JSON from a previous run.

## 2026.7.31

- Fix `project-lib --relink-sources --repair-cache-links` for placed schematic
  symbols whose `lib_id` is not directly mapped but whose `lib_name` matches a
  generated local embedded-cache member.
- Add regression coverage for member-only cache aliases so project-local relink
  output does not leave mixed external symbol links behind.

## 2026.7.30

- Update the controlled `kicad-monkey` dependency floor to
  `kicad-monkey>=2026.7.28` for project-local scan hardening and deterministic
  relink maps.
- Add explicit `project-lib` source relink dry-run/apply reporting for local
  symbol and footprint library migration.
- Harden `project-lib --relink-sources` so placed schematic symbols, embedded
  schematic cache symbols, direct cache unit names, footprint properties, PCB
  footprint links, and project library tables stay consistent with KiCad's
  loader invariants.
- Add `--repair-cache-links` for guarded placed `lib_name` cache repairs, with
  apply mode blocking instead of partially writing when unresolved cache-link
  or cache-unit issues remain.
- Preserve schematic and PCB source newline style during relink apply and
  reject `--relink-sources --no-update-library-tables` so generated local
  nicknames are registered before source files point at them.

## 2026.7.17

- Update the controlled `kicad-monkey` dependency floor to
  `kicad-monkey>=2026.7.17`, consuming the public KiCad Monkey parser and
  rendering performance improvements.
- Cache design-review PCB SVG render state within one command invocation so
  `design`, `design-review`, and `dr` avoid rebuilding the board IR for every
  copper layer while preserving enriched SVG output, viewBox framing, and
  drill/slot counts.
- Add direct-vs-cached PCB review SVG parity coverage, including a multi-layer
  public corpus fixture, and keep large-board timing evidence in the profiling
  helper and durable research notes.
- Record the deferred `KiCadDesign.to_json()` materialization cost as upstream
  `kicad-monkey` follow-up work and file the public tracking issue.
- Keep dev-std audit in L99 release signoff so governance checks are
  release-blocking.

## 2026.7.16

- Update the controlled `kicad-monkey` dependency floor to
  `kicad-monkey>=2026.7.16` while keeping `wn-geometer==2026.6.10`.
- Migrate the public command manifest to the `wn_dev_std.command_manifest.a0`
  governance contract and wire `docs.cli`, `docs.plans`, `docs.requirements`,
  and `docs.release` into release signoff.
- Add durable release-governance and requirement records for CLI
  documentation, PyPI release signoff, publish authorization, deferred
  daemon/plugin validation, future footprint HLR daemon work, schematic cleanup,
  PCB SVG selector follow-up, and library extraction hardening.
- Delete closed active plan files from the release artifact path after moving
  durable obligations into requirements, design docs, release governance, and
  validation records.

## 2026.6.25

- Update the controlled `kicad-monkey` dependency to `2026.6.25` for the
  object-model new-project assembly API (`KiCadProject.create`,
  `KICAD_PAGE_SIZES`, `kicad_page_size_label`).
- Add the `project` command with a `create` subcommand that scaffolds a KiCad
  project from flags, a JSONC `--config`, or an interactive `--tui` form, with
  all KiCad construction owned by `kicad-monkey` and the cruncher only gathering
  input.
- Fix `health` double-counting genuinely-missing 3D models: the same missing
  model file is now reported once per distinct file with every reference site
  listed under it, so registering the `project-lib` generated `local-library`
  no longer inflates `missing_or_unresolved_model`.
- Keep `wn-geometer==2026.6.10`.

## 2026.6.19

- Update the controlled `kicad-monkey` dependency to `2026.6.19` for the PCB
  graphical bounds fix, KiCad bbox oracle coverage, and canonical IR-backed
  PCB SVG wrapper behavior.
- Keep `kicad-cruncher` PCB SVG canvas bounds on the `kicad-monkey`
  `compute_pcb_svg_bounding_box()` path and avoid the removed direct PCB SVG
  renderer surface.
- Keep `wn-geometer==2026.6.10`.

## 2026.6.18

- Update the controlled `kicad-monkey` dependency to `2026.6.18` for targeted
  KiCad object extraction, cleaned library metadata normalization, and current
  symbol-library validation behavior.
- Promote the project-local library, cleaned library extraction, project
  health, and Megamaid workflows through the public CLI release gates.
- Add documented `library_extraction.json` contract coverage for raw and
  canonical parameter maps.
- Improve Megamaid and PCB SVG status logging for long-running project
  dissection and rendering workflows.

## 2026.6.15

- Update the controlled `kicad-monkey` dependency to `2026.6.15` for KiCad
  library extraction and project asset scanning helpers.
- Add `project-lib` for metadata-preserving project-local library extraction.
- Add `lib-extract`/`library-extract` aliases for cleaned library-ingestion
  bundles while keeping `megamaid` as the compatibility command name.
- Add `health` for non-destructive project asset and 3D model
  diagnostics, with interactive and `--fail-on-issues` modes.
- Improve health README output with issue-kind counts and representative
  examples.
- Keep `wn-geometer==2026.6.10`.

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
