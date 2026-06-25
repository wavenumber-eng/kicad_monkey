"""Restore and verify the public KiCad corpus archive."""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
import tempfile
import tomllib
import urllib.error
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPO_ROOT / "tests" / "corpus" / "kicad.archive.toml"
DEFAULT_ARCHIVE = REPO_ROOT / "tests" / "corpus" / "kicad.zip"
EXPECTED_SCHEMA = "kicad_monkey.corpus_archive.v1"
URL_ENV_NAMES = ("KICAD_MONKEY_CORPUS_URL", "KICAD_CORPUS_URL")
DOWNLOAD_USER_AGENT = "kicad-monkey-ci/1.0"
DOWNLOAD_TIMEOUT_SECONDS = 300


@dataclass(frozen=True)
class CorpusArchiveManifest:
    archive: str
    size: int
    sha256: str
    url: str | None = None
    r2_bucket: str | None = None
    r2_key: str | None = None


def load_manifest(path: Path = DEFAULT_MANIFEST) -> CorpusArchiveManifest:
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != EXPECTED_SCHEMA:
        raise RuntimeError(f"unexpected corpus archive schema: {data.get('schema')!r}")

    archive = str(data.get("archive") or "")
    sha256 = str(data.get("sha256") or "").lower()
    size = int(data.get("size") or 0)
    if not archive or "/" in archive or "\\" in archive:
        raise RuntimeError("corpus archive manifest must name one local archive file")
    if len(sha256) != 64:
        raise RuntimeError("corpus archive manifest SHA-256 is invalid")
    if size <= 0:
        raise RuntimeError("corpus archive manifest size is invalid")

    return CorpusArchiveManifest(
        archive=archive,
        size=size,
        sha256=sha256,
        url=str(data["url"]) if data.get("url") else None,
        r2_bucket=str(data["r2_bucket"]) if data.get("r2_bucket") else None,
        r2_key=str(data["r2_key"]) if data.get("r2_key") else None,
    )


def archive_path(manifest: CorpusArchiveManifest) -> Path:
    return (DEFAULT_MANIFEST.parent / manifest.archive).resolve()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_archive(path: Path, manifest: CorpusArchiveManifest, *, check_zip: bool) -> list[str]:
    errors: list[str] = []
    if not path.is_file():
        return [f"archive not found: {path}"]

    size = path.stat().st_size
    if size != manifest.size:
        errors.append(f"archive size mismatch: expected {manifest.size}, got {size}")

    actual_sha256 = sha256_file(path)
    if actual_sha256 != manifest.sha256:
        errors.append(f"archive SHA-256 mismatch: expected {manifest.sha256}, got {actual_sha256}")

    if check_zip and not errors:
        try:
            with zipfile.ZipFile(path) as archive:
                bad_member = archive.testzip()
        except zipfile.BadZipFile:
            errors.append(f"archive is not a valid zip file: {path}")
        else:
            if bad_member is not None:
                errors.append(f"archive zip member failed CRC check: {bad_member}")

    return errors


def corpus_url(manifest: CorpusArchiveManifest, explicit_url: str | None) -> str | None:
    if explicit_url:
        return explicit_url
    for name in URL_ENV_NAMES:
        value = os.environ.get(name)
        if value:
            return value
    return manifest.url


def restore_archive(
    path: Path,
    manifest: CorpusArchiveManifest,
    *,
    explicit_url: str | None,
    check_zip: bool,
) -> bool:
    current_errors = verify_archive(path, manifest, check_zip=check_zip)
    if not current_errors:
        print(f"archive ok: {path}")
        return False

    url = corpus_url(manifest, explicit_url)
    if not url:
        raise RuntimeError(
            "corpus archive is missing or invalid and no public download URL is configured. "
            f"Set {URL_ENV_NAMES[0]} to the public R2 object URL."
        )

    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="kicad-corpus-archive-", dir=path.parent) as temp_name:
        temp_path = Path(temp_name) / path.name
        print(f"download={url}")
        try:
            request = urllib.request.Request(url, headers={"User-Agent": DOWNLOAD_USER_AGENT})
            with urllib.request.urlopen(request, timeout=DOWNLOAD_TIMEOUT_SECONDS) as response:
                with temp_path.open("wb") as handle:
                    shutil.copyfileobj(response, handle)
        except urllib.error.URLError as exc:
            raise RuntimeError(f"failed to download corpus archive from {url}: {exc}") from exc

        errors = verify_archive(temp_path, manifest, check_zip=check_zip)
        if errors:
            raise RuntimeError("downloaded corpus archive failed verification:\n" + "\n".join(errors))
        temp_path.replace(path)

    print(f"restored={path}")
    return True


def write_github_output(manifest: CorpusArchiveManifest) -> None:
    output_path = os.environ.get("GITHUB_OUTPUT")
    lines = [
        f"archive={manifest.archive}",
        f"size={manifest.size}",
        f"sha256={manifest.sha256}",
    ]
    text = "\n".join(lines) + "\n"
    if output_path:
        with Path(output_path).open("a", encoding="utf-8") as handle:
            handle.write(text)
    else:
        print(text, end="")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=("metadata", "verify", "restore"),
        help="Print metadata, verify the local archive, or restore it from public object storage.",
    )
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--url", help=f"Override {'/'.join(URL_ENV_NAMES)}.")
    parser.add_argument("--check-zip", action="store_true", help="Also validate zip structure and CRCs.")
    parser.add_argument(
        "--github-output",
        action="store_true",
        help="Write metadata to $GITHUB_OUTPUT instead of stdout.",
    )
    args = parser.parse_args(argv)

    manifest = load_manifest(args.manifest)
    path = archive_path(manifest)

    if args.command == "metadata":
        if args.github_output:
            write_github_output(manifest)
        else:
            print(f"archive={manifest.archive}")
            print(f"size={manifest.size}")
            print(f"sha256={manifest.sha256}")
        return 0

    if args.command == "verify":
        errors = verify_archive(path, manifest, check_zip=args.check_zip)
        if errors:
            print("\n".join(errors), file=sys.stderr)
            return 1
        print(f"archive ok: {path}")
        return 0

    restore_archive(path, manifest, explicit_url=args.url, check_zip=args.check_zip)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
