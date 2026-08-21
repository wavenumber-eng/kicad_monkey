# Package-Local Test Corpus

This directory carries the public KiCad test corpus so the public repository
can run corpus-backed tests without depending on a machine-local
fixture checkout.

The public archive form is `kicad.zip`. It is restored locally and ignored by
Git; only `kicad.archive.toml` is tracked. The archive contains a top-level
`kicad/` directory matching the external corpus layout:

```text
tests/corpus/kicad.zip
  kicad/...
```

`kicad.archive.toml` records the expected archive size, SHA-256, and R2 object
key. CI restores the real archive from the public URL recorded in that manifest,
then verifies it before any tests extract or use the corpus.

Tests use the ZIP even when a loose authoring mirror exists. The shared corpus
resolver verifies the package archive manifest, hashes the selected ZIP, and
extracts it to an immutable content-addressed directory under
`tests/corpus/.unpacked/`. That directory is published internally as
`KM_CORPUS_ROOT`. `KM_CORPUS` may select another reviewed ZIP. For fixture
maintenance only, it may instead explicitly name a writable directory
containing `kicad/`; there is no implicit loose-tree fallback.

Source snapshot:

- Mirrored root: `tests/corpus/kicad`
- Excluded generated/local-only directories: `output`, `review`,
  `review_tmp`, `.git`, `.history`
- Preserved oracle/reference directories such as `reference_output`
- Generated visual-review artifacts are written under the owning test
  stratum's ignored `output/` tree, never into the extracted archive cache.
- Real-world `projects/*_assembly` entries are assembly-procedure
  documentation projects. Their PCB files are intentionally empty/header-only,
  so they are schematic SVG/IR fixtures rather than `board_svg` review cases.

Archive SOP:

```powershell
uv run --extra test python scripts/kicad_corpus_archive.py restore --check-zip
uv run --extra test python scripts/package_kicad_corpus.py
uv run --extra test python scripts/kicad_corpus_archive.py verify --check-zip
uv run --extra test python scripts/package_kicad_corpus.py --check
uv run --extra test python tests/rack.py run L1_029
uv run --extra test python tests/rack.py run L99_signoff
```

These assets are for repository tests and review. They are excluded from sdist
artifacts by `pyproject.toml`.
