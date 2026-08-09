# Test Corpus

This directory holds redistributable KiCad fixtures used by public
`kicad-cruncher` workflow tests.

The current copied fixtures are:

- `kicad/projects/hlr_test`
- `kicad/projects/taillight/input`
- `kicad/projects/charge_indicator/input`
- `kicad/projects/yoshi_mainboard/input`
- `kicad/projects/speedy_processing_module/input`
- `kicad/projects/4-ch-backplane/input`
- `kicad/projects/cutout_test`

Each fixture is copied from the cleaned public `kicad-monkey` corpus. The
directory shape mirrors the source corpus so future fixtures can be compared or
refreshed without changing test lookup constants.

`speedy_processing_module/input` intentionally carries only the project root,
board, and schematic files referenced by the active sheet hierarchy. Stale or
unreferenced scratch schematics from the expanded source corpus are omitted.

`hlr_test` is a focused single-component assembly projection fixture with an
embedded STEP model and non-default model offset/rotation.

`4-ch-backplane/input` is a sanitized real-world hierarchical project with
replicated sheet instances and an off-board USB sheet. It is kept as a netlist
and design-review regression fixture.
