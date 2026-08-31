# Project-level bus alias hierarchy fixture

This original KiCad 10 fixture mirrors the useful topology of KiCad's
`issue24220` QA project without copying its files. `CTRL` is declared only in
the `.kicad_pro` file and expands to `CTRL_A` and `CTRL_B`. Both members are
terminal-bearing nets connected across the `MEMBER_SHEET` hierarchy boundary.

The XML netlist in `reference_output/` was generated with KiCad CLI 10.0.5.
Its timestamp and absolute source path are canonicalized. `output/` is
transient, and local editor-state files such as `.kicad_prl` must not be
committed.
