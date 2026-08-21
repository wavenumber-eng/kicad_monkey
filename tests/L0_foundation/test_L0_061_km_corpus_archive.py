"""ZIP-first corpus selection for bots and package-local Rack runs."""

from __future__ import annotations

import os
import zipfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest

from _suite_paths import TEST_CORPUS_ROOT, resolve_test_corpus_output_path
from kicad_monkey.testing import corpus


def _write_archive(path: Path, *, marker: str = "first") -> None:
    with zipfile.ZipFile(path, "w") as zipped:
        for name, value in (
            ("kicad/manifest.json", '{"schema": "test"}\n'),
            ("kicad/common/marker.txt", marker),
        ):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            zipped.writestr(info, value)


def _isolate_cache(monkeypatch: pytest.MonkeyPatch, cache_root: Path) -> None:
    monkeypatch.setattr(corpus, "_archive_cache_root", lambda _archive: cache_root)
    monkeypatch.delenv(corpus.CORPUS_ARCHIVE_ENV, raising=False)
    monkeypatch.delenv(corpus.CORPUS_ROOT_ENV, raising=False)
    monkeypatch.delenv(corpus.CORPUS_RESOLVED_FROM_ENV, raising=False)


def _write_review_manifest(path: Path, archive: Path) -> None:
    path.write_text(
        "\n".join(
            (
                'schema = "kicad_monkey.corpus_archive.v1"',
                f'archive = "{archive.name}"',
                f"size = {archive.stat().st_size}",
                f'sha256 = "{corpus._archive_digest(archive)}"',
                "",
            )
        ),
        encoding="utf-8",
    )


def test_km_corpus_cache_is_content_addressed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "kicad.zip"
    cache_root = tmp_path / "cache"
    _isolate_cache(monkeypatch, cache_root)
    _write_archive(archive, marker="first-version")

    first = corpus.resolve_test_corpus_root(archive)
    assert (first / "kicad" / "common" / "marker.txt").read_text() == "first-version"

    original_stat = archive.stat()
    _write_archive(archive, marker="other-version")
    assert archive.stat().st_size == original_stat.st_size
    os.utime(archive, ns=(original_stat.st_atime_ns, original_stat.st_mtime_ns))
    second = corpus.resolve_test_corpus_root(archive)
    assert second != first
    assert (second / "kicad" / "common" / "marker.txt").read_text() == "other-version"
    assert (first / "kicad" / "common" / "marker.txt").read_text() == "first-version"


def test_orphaned_extraction_artifact_does_not_block_resolution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "kicad.zip"
    cache_root = tmp_path / "cache"
    _write_archive(archive)
    _isolate_cache(monkeypatch, cache_root)
    cache_root.mkdir()
    (cache_root / ".orphan.extracting").mkdir()

    resolved = corpus.resolve_test_corpus_root(archive)
    assert (resolved / "kicad" / "common" / "marker.txt").is_file()


def test_concurrent_resolvers_publish_one_immutable_tree(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "kicad.zip"
    cache_root = tmp_path / "cache"
    _write_archive(archive)
    _isolate_cache(monkeypatch, cache_root)

    with ThreadPoolExecutor(max_workers=2) as pool:
        roots = list(pool.map(lambda _index: corpus.resolve_test_corpus_root(archive), range(2)))

    assert roots[0] == roots[1]
    assert (roots[0] / "kicad" / "common" / "marker.txt").is_file()


def test_incomplete_digest_directory_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "kicad.zip"
    cache_root = tmp_path / "cache"
    _write_archive(archive)
    _isolate_cache(monkeypatch, cache_root)
    incomplete = cache_root / corpus._archive_digest(archive)
    incomplete.mkdir(parents=True)
    (incomplete / "partial.txt").write_text("partial")

    with pytest.raises(RuntimeError, match="Incomplete content-addressed corpus cache"):
        corpus.resolve_test_corpus_root(archive)
    assert (incomplete / "partial.txt").is_file()


def test_generated_output_mapping_stays_outside_extracted_corpus() -> None:
    output = resolve_test_corpus_output_path(
        {"output_root": "projects/example/output"}
    )
    assert output is not None
    for unsafe in ("../escape", "..\\escape", "/escape", "\\escape", "C:escape", "C:\\escape"):
        with pytest.raises(ValueError, match="Unsafe corpus output path"):
            resolve_test_corpus_output_path({"output_root": unsafe})
    with pytest.raises(ValueError):
        output.resolve().relative_to(TEST_CORPUS_ROOT.resolve())


def test_km_corpus_directory_is_an_explicit_authoring_escape_hatch(
    tmp_path: Path,
) -> None:
    (tmp_path / "kicad").mkdir()
    assert corpus.resolve_test_corpus_root(tmp_path) == tmp_path


def test_explicit_invalid_km_corpus_fails_without_fallback(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError, match="KM_CORPUS not found"):
        corpus.resolve_test_corpus_root(tmp_path / "missing.zip")

    invalid_directory = tmp_path / "not-a-corpus"
    invalid_directory.mkdir()
    with pytest.raises(FileNotFoundError, match="KM_CORPUS kicad root"):
        corpus.resolve_test_corpus_root(invalid_directory)


def test_unsafe_zip_member_is_rejected(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "unsafe.zip"
    with zipfile.ZipFile(archive, "w") as zipped:
        zipped.writestr("../escape.txt", "unsafe")
    _isolate_cache(monkeypatch, tmp_path / "cache")

    with pytest.raises(RuntimeError, match="Unsafe corpus archive member"):
        corpus.resolve_test_corpus_root(archive)
    assert not (tmp_path / "escape.txt").exists()


def test_km_corpus_hard_switch_ignores_legacy_wn_directory(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "kicad.zip"
    _write_archive(archive, marker="km")
    legacy = tmp_path / "legacy"
    (legacy / "kicad").mkdir(parents=True)
    (legacy / "kicad" / "legacy.txt").write_text("legacy")
    _isolate_cache(monkeypatch, tmp_path / "cache")
    monkeypatch.setenv("KM_CORPUS", str(archive))
    monkeypatch.setenv("KM_CORPUS_ROOT", str(legacy))
    monkeypatch.setenv("WN_TEST_CORPUS", str(legacy))

    resolved = corpus.get_test_corpus_root()
    assert (resolved / "kicad" / "common" / "marker.txt").read_text() == "km"
    assert not (resolved / "kicad" / "legacy.txt").exists()


def test_legacy_wn_corpus_alone_cannot_select_a_corpus(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "default-kicad.zip"
    _write_archive(archive, marker="default")
    legacy = tmp_path / "legacy"
    (legacy / "kicad").mkdir(parents=True)
    (legacy / "kicad" / "legacy.txt").write_text("legacy")
    _isolate_cache(monkeypatch, tmp_path / "cache")
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE", archive)
    manifest = tmp_path / "kicad.archive.toml"
    _write_review_manifest(manifest, archive)
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE_MANIFEST", manifest)
    monkeypatch.delenv("KM_CORPUS", raising=False)
    monkeypatch.delenv("KM_CORPUS_ROOT", raising=False)
    monkeypatch.setenv("WN_TEST_CORPUS", str(legacy))

    resolved = corpus.get_test_corpus_root()
    assert (resolved / "kicad" / "common" / "marker.txt").read_text() == "default"
    assert not (resolved / "kicad" / "legacy.txt").exists()


def test_internal_root_alone_cannot_select_a_corpus(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "default-kicad.zip"
    _write_archive(archive, marker="default")
    untrusted = tmp_path / "untrusted"
    (untrusted / "kicad").mkdir(parents=True)
    (untrusted / "kicad" / "untrusted.txt").write_text("untrusted")
    _isolate_cache(monkeypatch, tmp_path / "cache")
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE", archive)
    manifest = tmp_path / "kicad.archive.toml"
    _write_review_manifest(manifest, archive)
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE_MANIFEST", manifest)
    monkeypatch.setenv("KM_CORPUS_ROOT", str(untrusted))

    resolved = corpus.get_test_corpus_root()
    assert (resolved / "kicad" / "common" / "marker.txt").read_text() == "default"
    assert not (resolved / "kicad" / "untrusted.txt").exists()


def test_published_root_avoids_rehashing_the_same_carrier(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive = tmp_path / "kicad.zip"
    _write_archive(archive)
    _isolate_cache(monkeypatch, tmp_path / "cache")
    resolved = corpus.resolve_test_corpus_root(archive)
    monkeypatch.setenv("KM_CORPUS", str(archive))
    monkeypatch.setenv("KM_CORPUS_ROOT", str(resolved))
    monkeypatch.setenv("KM_CORPUS_RESOLVED_FROM", str(archive.resolve()))
    monkeypatch.setattr(
        corpus,
        "resolve_test_corpus_root",
        lambda: pytest.fail("published corpus root should be reused"),
    )

    assert corpus.get_test_corpus_root() == resolved


@pytest.mark.parametrize("mode", ["missing", "mismatch"])
def test_default_archive_requires_matching_review_manifest(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mode: str,
) -> None:
    archive = tmp_path / "kicad.zip"
    manifest = tmp_path / "kicad.archive.toml"
    _write_archive(archive)
    _isolate_cache(monkeypatch, tmp_path / "cache")
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE", archive)
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE_MANIFEST", manifest)
    if mode == "mismatch":
        _write_review_manifest(manifest, archive)
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                corpus._archive_digest(archive), "0" * 64
            ),
            encoding="utf-8",
        )

    error = FileNotFoundError if mode == "missing" else RuntimeError
    with pytest.raises(error):
        corpus.resolve_test_corpus_root()


def test_missing_default_archive_does_not_fall_back_to_loose_tree(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    authoring_root = tmp_path / "authoring"
    (authoring_root / "kicad").mkdir(parents=True)
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_ARCHIVE", tmp_path / "missing.zip")
    monkeypatch.setattr(
        corpus, "DEFAULT_CORPUS_ARCHIVE_MANIFEST", tmp_path / "missing.toml"
    )
    monkeypatch.setattr(corpus, "DEFAULT_CORPUS_AUTHORING_ROOT", authoring_root)
    monkeypatch.delenv("KM_CORPUS", raising=False)
    monkeypatch.delenv("KM_CORPUS_ROOT", raising=False)

    with pytest.raises(FileNotFoundError, match="KiCad corpus archive not found"):
        corpus.resolve_test_corpus_root()
