# KiCad Cruncher native CLI for Windows x64

This archive contains the pure-Rust `kicad-cruncher.exe` and `kcr.exe`
entry points plus the qualified Geometer 2026.9.13 `geometer.exe` sidecar used
for native model illustration. Put the extracted directory before Python
tool-script directories on `PATH` to make the native implementation canonical
for these supported commands:

- `design`
- `design-review`
- `dr`
- `toon`
- `--version`

The executables do not require Python at runtime. Geometer is discovered beside
the Cruncher entry points; `GEOMETER_EXECUTABLE` may select an explicit matching
runtime. `toon` emits transactional top/bottom SVGs and a digest-indexed JSON
manifest; it renders embedded STEP models without Python. Its clean-source
build attestation and third-party license notices are
included in the archive. Other KiCad Cruncher
commands remain in the Python distribution and can be invoked explicitly with
`python -m kicad_cruncher` until their Rust vertical slices are complete.

The companion `kicad-cruncher` Python wheel retains its normal public
`kicad-monkey` dependency. The native executable instead composes the reviewed
Monkey Rust crates at build time and contains no workspace-path dependency.
