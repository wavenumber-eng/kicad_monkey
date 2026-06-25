# Package-Local Test Corpus

This directory carries the public KiCad test corpus so the public repository
can run corpus-backed tests without depending on a machine-local
`WN_TEST_CORPUS`.

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

For local review, the unpacked mirror may exist at `tests/corpus/kicad/`; that
directory is gitignored. Tests prefer the loose mirror when present. Otherwise
`tests/_suite_paths.py` extracts `kicad.zip` to `tests/corpus/.unpacked/` and
uses that path as the default `WN_TEST_CORPUS`.

Source snapshot:

- Mirrored root: `tests/corpus/kicad`
- Excluded generated/local-only directories: `output`, `review`,
  `review_tmp`, `.git`, `.history`
- Preserved oracle/reference directories such as `reference_output`
- Generated visual-review artifacts are written under each case's
  `output/<domain>/` folder, for example `projects/<name>/output/board_svg/`.
  These files are local review outputs and are not part of the tracked corpus
  archive.
- Real-world `projects/*_assembly` entries are assembly-procedure
  documentation projects. Their PCB files are intentionally empty/header-only,
  so they are schematic SVG/IR fixtures rather than `board_svg` review cases.

Archive SOP:

```powershell
uv run --extra test python scripts/package_kicad_corpus.py
uv run --extra test python scripts/kicad_corpus_archive.py verify
uv run --extra test python scripts/package_kicad_corpus.py --check
uv run --extra test python tests/rack.py run L99_signoff
```

These assets are for repository tests and review. They are excluded from sdist
artifacts by `pyproject.toml`.
