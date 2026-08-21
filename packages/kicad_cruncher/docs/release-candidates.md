# Release candidate construction

The repository-root GitHub Actions release workflow constructs one reviewed
Cruncher release set from an exact source commit:

- a universal Python wheel and source distribution that retain the public
  `kicad-monkey` dependency; and
- a Windows x64 native archive containing `kicad-cruncher.exe` and `kcr.exe`
  for the promoted design aliases and version surface.

The Phase 6 candidate workflow hash-binds the Python distributions. The Phase 7
candidate workflow restores the reviewed `KM_CORPUS` ZIP, installs the Rust
crate through `cargo install --locked`, verifies Windows x64 PE architecture
and workspace-path hygiene, runs a design bundle without Python in the runtime
environment, and emits an archive plus `kicad_cruncher.rust_cli_release.a0`
manifest. Before candidate construction it runs the complete L3_011
Rust/Python migration oracle and L3_012 installed-candidate/tamper suite
sequentially under package-local Rack.

Before normal-lane PyPI publication, the release workflow verifies the native
archive's exact topology, sizes, SHA-256 values, source commit, and version
against the Python package, then attaches both native assets to the GitHub
Release. The coordinated two-package lane performs the same verification before
Cruncher publication and attaches the native assets when it creates the
package-qualified release.
