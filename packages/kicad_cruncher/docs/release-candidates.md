# Release candidate construction

One successful full `CI` run constructs the reviewed release set from an exact
source commit. The Linux Python-provider job produces Monkey's universal wheel.
The consolidated Windows job produces:

- Monkey and Python Cruncher source distributions;
- a Windows x64 Monkey wheel containing the package-owned native helper;
- a universal Python Cruncher wheel that retains its public Monkey dependency;
  and
- a Windows x64 Rust archive containing `kicad-cruncher.exe` and `kcr.exe`.

The Windows job restores the reviewed `KM_CORPUS` ZIP and builds the native
helper once. It then runs the native SVG, physical-provider, design-facts,
installed Python CLI, and Rust CLI migration gates sequentially against that
shared setup. The Python distributions and Rust archive have separate
manifests binding workflow run, commit, versions, filenames, sizes, and
SHA-256 values. The Linux universal wheel has the same binding.

The publish workflow locates the successful main CI run for the tagged commit,
downloads all three artifact sets, verifies their manifests and hashes, and
publishes those exact files. It never rebuilds a candidate. A retry uses
`skip-existing` and the same resolved run, so recovery cannot select or build
different bytes.
