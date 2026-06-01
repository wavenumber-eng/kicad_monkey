# Test Corpus

This directory holds redistributable KiCad fixtures used by public
`kicad-cruncher` workflow tests.

The current copied fixtures are:

- `kicad/board_svg/input/led_component`
- `kicad/projects/taillight/input`
- `kicad/projects/charge_indicator/input`

Each fixture is copied from the cleaned public `kicad-monkey` corpus. The
directory shape mirrors the source corpus so future fixtures can be compared or
refreshed without changing test lookup constants.
