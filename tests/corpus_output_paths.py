"""Paths for transient products derived from immutable corpus inputs."""

from __future__ import annotations

from pathlib import Path, PurePosixPath, PureWindowsPath


TESTS_DIR = Path(__file__).resolve().parent
TEST_GENERATED_CORPUS_ROOT = TESTS_DIR / "L3_rendering" / "output" / "corpus"


def resolve_test_corpus_output_path(case: dict, key: str = "output_root") -> Path | None:
    value = case.get(key)
    if value in (None, ""):
        return None
    raw = str(value).replace("\\", "/")
    portable = PurePosixPath(raw)
    windows = PureWindowsPath(str(value))
    if portable.is_absolute() or ".." in portable.parts or windows.drive or windows.root:
        raise ValueError(f"Unsafe corpus output path: {value!r}")
    destination = (TEST_GENERATED_CORPUS_ROOT / Path(*portable.parts)).resolve()
    try:
        destination.relative_to(TEST_GENERATED_CORPUS_ROOT.resolve())
    except ValueError as error:
        raise ValueError(f"Unsafe corpus output path: {value!r}") from error
    return destination
