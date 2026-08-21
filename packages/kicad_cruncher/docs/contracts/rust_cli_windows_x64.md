# KiCad Cruncher native design CLI for Windows x64

This archive contains the pure-Rust `kicad-cruncher.exe` and `kcr.exe`
entry points. Put the extracted directory before Python tool-script directories
on `PATH` to make the native implementation canonical for these supported
commands:

- `design`
- `design-review`
- `dr`
- `--version`

The executable does not require Python at runtime. Other KiCad Cruncher
commands remain in the Python distribution and can be invoked explicitly with
`python -m kicad_cruncher` until their Rust vertical slices are complete.

The companion `kicad-cruncher` Python wheel retains its normal public
`kicad-monkey` dependency. The native executable instead composes the reviewed
Monkey Rust crates at build time and contains no workspace-path dependency.
