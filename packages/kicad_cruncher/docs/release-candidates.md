# Release candidate construction

One successful full `CI` run constructs the reviewed release set from an exact
source commit. The Linux Python-provider job produces Monkey's universal wheel.
The consolidated Windows job produces:

- Monkey and Python Cruncher source distributions;
- a Windows x64 Monkey wheel containing the package-owned native helper;
- a universal Python Cruncher wheel that retains its public Monkey dependency;
  and
- a Windows x64 Rust archive containing `kicad-cruncher.exe`, `kcr.exe`, and
  the qualified `geometer.exe` sidecar with its attestation and licenses.

The Windows job restores the reviewed `KM_CORPUS` ZIP and builds the native
helper once. It then runs the native SVG, physical-provider, design-facts,
installed Python CLI, and Rust CLI migration gates sequentially against that
shared setup. The Python distributions and Rust archive have separate
manifests binding workflow run, commit, versions, filenames, sizes, and
SHA-256 values. The Linux universal wheel has the same binding.

Separate native smoke jobs compile the Rust CLI and run its real Geometer
process contract on Linux x64, Linux ARM64, and macOS ARM64 using the exact
hash-pinned release assets. These checks establish source-build and runtime
compatibility; they do not yet promote non-Windows native archives.

The publish workflow locates the successful main CI run for the tagged commit,
downloads all three artifact sets, verifies their manifests and hashes, and
publishes those exact files. It never rebuilds a candidate. A retry uses
`skip-existing` and the same resolved run. Before either upload, every existing
public filename and digest must be an exact subset of that package's candidate
set; after upload, the complete public set must match. Recovery therefore
cannot select, build, or silently combine different bytes.

Candidate construction also rejects untracked source and scans archive members,
metadata, and compiled binaries for local Windows, WSL, home, workspace, and
temporary paths. Rust source roots are remapped before the native builds.
