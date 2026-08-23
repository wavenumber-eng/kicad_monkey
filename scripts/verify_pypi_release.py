#!/usr/bin/env python3
"""Verify that a PyPI release contains exactly the selected candidate bytes."""

from __future__ import annotations

import argparse
import hashlib
import json
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def compare_release_files(
    expected_directory: Path, payload: dict[str, Any], *, expected_version: str
) -> None:
    """Reject missing, extra, renamed, or byte-different PyPI distributions."""
    actual_version = str(payload.get("info", {}).get("version", ""))
    if actual_version != expected_version:
        raise SystemExit(
            f"PyPI returned version {actual_version!r}, expected {expected_version!r}"
        )

    expected = {
        path.name: _sha256(path)
        for path in expected_directory.iterdir()
        if path.is_file()
    }
    if not expected:
        raise SystemExit(f"No candidate files found in {expected_directory}")

    published: dict[str, str] = {}
    for item in payload.get("urls", []):
        filename = str(item.get("filename", ""))
        sha256 = str(item.get("digests", {}).get("sha256", ""))
        if not filename or not sha256:
            raise SystemExit("PyPI release metadata has a file without a SHA256 digest")
        published[filename] = sha256

    if published != expected:
        missing = sorted(expected.keys() - published.keys())
        extra = sorted(published.keys() - expected.keys())
        changed = sorted(
            name
            for name in expected.keys() & published.keys()
            if expected[name] != published[name]
        )
        raise SystemExit(
            "PyPI files do not match the exact CI candidates: "
            f"missing={missing}, extra={extra}, changed={changed}"
        )


def verify(
    package: str,
    version: str,
    expected_directory: Path,
    *,
    attempts: int = 20,
    delay_seconds: float = 15,
) -> None:
    encoded_package = urllib.parse.quote(package, safe="")
    encoded_version = urllib.parse.quote(version, safe="")
    url = f"https://pypi.org/pypi/{encoded_package}/{encoded_version}/json"
    last_error = "release metadata was not available"
    for attempt in range(1, attempts + 1):
        try:
            with urllib.request.urlopen(url, timeout=30) as response:
                payload = json.load(response)
            compare_release_files(
                expected_directory, payload, expected_version=version
            )
            print(
                f"Verified {package} {version}: all files and SHA256 digests match CI"
            )
            return
        except (urllib.error.URLError, json.JSONDecodeError) as error:
            last_error = str(error)
        except SystemExit as error:
            last_error = str(error)
        if attempt < attempts:
            time.sleep(delay_seconds)
    raise SystemExit(
        f"Could not verify {package} {version} after {attempts} attempts: {last_error}"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package")
    parser.add_argument("version")
    parser.add_argument("expected_directory", type=Path)
    parser.add_argument("--attempts", type=int, default=20)
    parser.add_argument("--delay-seconds", type=float, default=15)
    args = parser.parse_args()
    verify(
        args.package,
        args.version,
        args.expected_directory,
        attempts=args.attempts,
        delay_seconds=args.delay_seconds,
    )


if __name__ == "__main__":
    main()
