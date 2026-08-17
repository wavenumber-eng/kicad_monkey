# KiCad Monkey Working Rules

- Distribution name: `kicad-monkey`.
- Python import name: `kicad_monkey`.
- Keep package-local tests under `tests/` and run Rack through `tests/rack.py`.
- Use `KM_CORPUS` to select a reviewed `kicad.zip`; the package-local archive
  is the default. A directory containing `kicad/` is only for fixture authoring.
- Persistent cases should follow `input/`, `reference_output/`, `output/`.
- `output/` is transient and belongs in local temp/output paths, not authoritative fixture data.
- Keep `kicad_monkey` focused on parser/source-model, round-trip, basic 2D rendering, and close-to-format utilities.
- Keep higher-level report, migration, and application workflows in downstream packages.
- The repository also contains the `kicad-cruncher` distribution under
  `packages/kicad_cruncher/`. Keep its CLI and artifact-orchestration behavior
  in that package; `kicad_monkey` must never depend on `kicad_cruncher`.
- A Monkey public-surface change used by Cruncher needs a same-change
  cross-package test. Built Cruncher artifacts must retain a normal public
  `kicad-monkey` dependency and must not embed workspace paths.
- Keep plans and research local. When the work lands, move durable decisions and status into ADRs, design docs, release notes, or contracts.
