"""Restore a staged kicad-cli bundle from the R2 dependency cache.

The restored tree is written to:

    <corpus-root>/tools/kicad-cli/<short_hash>/

Run with:

    uv run --with boto3 python scripts/restore_kicad_cli_cache.py \
        --short-hash b70f2b514aa2-irbbox
"""

from __future__ import annotations

import argparse
import hashlib
import importlib
import json
import os
import shutil
import sys
import tempfile
import tomllib
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import urlparse, urlunparse


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPO_ROOT / "tools" / "kicad-cli" / "MANIFEST.toml"
DEFAULT_R2_REGION = "auto"
EXPECTED_SCHEMA = "wavenumber.dependency_cache_manifest.a1"
ARCHIVE_NAME = "kicad-cli-bundle.zip"


@dataclass(frozen=True)
class RestoreConfig:
    bucket: str
    endpoint_url: str
    access_key_id: str
    secret_access_key: str
    region: str


@dataclass(frozen=True)
class KicadCliCacheEntry:
    short_hash: str
    r2_bucket: str
    r2_prefix: str
    r2_archive: str
    r2_sha256: str


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    load_dotenv(args.env_file)

    entry = _select_cache_entry(args.manifest, args.short_hash)
    config = config_from_env(default_bucket=entry.r2_bucket)
    target_dir = _target_dir(args, entry)

    print(f"short_hash={entry.short_hash}")
    print(f"bucket={config.bucket}")
    print(f"endpoint_host={urlparse(config.endpoint_url).netloc}")
    print(f"r2_prefix={entry.r2_prefix}")
    print(f"target_dir={target_dir}")

    if args.dry_run:
        return 0

    if target_dir.exists() and not args.force:
        raise SystemExit(
            f"target already exists: {target_dir}\n"
            "Use --force to replace it, or --target-dir to restore elsewhere."
        )

    s3 = _make_s3_client(config)
    with tempfile.TemporaryDirectory(prefix="kicad-cli-r2-restore-") as temp_name:
        temp_dir = Path(temp_name)
        remote_manifest_path = temp_dir / "manifest.json"
        remote_sha_path = temp_dir / f"{entry.r2_archive}.sha256"
        archive_path = temp_dir / entry.r2_archive
        extract_dir = temp_dir / "extract"

        _download_file(s3, config.bucket, f"{entry.r2_prefix}manifest.json", remote_manifest_path)
        cache_manifest = _load_cache_manifest(remote_manifest_path, entry)

        _download_file(s3, config.bucket, f"{entry.r2_prefix}{entry.r2_archive}.sha256", remote_sha_path)
        _download_file(s3, config.bucket, f"{entry.r2_prefix}{entry.r2_archive}", archive_path)

        archive_sha256 = sha256_file(archive_path)
        expected_sha256 = str(cache_manifest["archive"]["sha256"])
        sidecar_sha256 = _parse_sha256_sidecar(remote_sha_path)
        expected_values = {entry.r2_sha256.lower(), expected_sha256.lower(), sidecar_sha256.lower()}
        if len(expected_values) != 1 or archive_sha256.lower() not in expected_values:
            raise RuntimeError(
                "archive SHA-256 mismatch: "
                f"computed={archive_sha256}, manifest={expected_sha256}, "
                f"sidecar={sidecar_sha256}, repo_manifest={entry.r2_sha256}"
            )

        _extract_zip_safely(archive_path, extract_dir)
        _validate_extracted_bundle(extract_dir)
        _install_extracted_tree(extract_dir, target_dir, force=args.force)

    print("restored=true")
    return 0


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="KiCad CLI manifest TOML. Defaults to tools/kicad-cli/MANIFEST.toml.",
    )
    parser.add_argument(
        "--short-hash",
        help="Manifest short_hash to restore. Defaults to the last entry with R2 fields.",
    )
    parser.add_argument(
        "--corpus-root",
        type=Path,
        help=(
            "Corpus root that contains tools/kicad-cli. Defaults to "
            "$WN_TEST_CORPUS after .env loading, then tests/corpus."
        ),
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        help="Explicit restore target directory. Overrides --corpus-root.",
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=REPO_ROOT / ".env",
        help="Optional dotenv file to load before reading R2_* variables.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Replace an existing target directory after path safety checks.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the selected cache entry and target without downloading.",
    )
    return parser.parse_args(argv)


def load_dotenv(path: Path) -> None:
    if not path.exists():
        return

    for raw_line in path.read_text(encoding="utf-8-sig").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue

        name, value = line.split("=", 1)
        name = name.strip()
        value = value.strip()
        if not name or name in os.environ:
            continue
        if (value.startswith('"') and value.endswith('"')) or (
            value.startswith("'") and value.endswith("'")
        ):
            value = value[1:-1]
        os.environ[name] = value


def config_from_env(*, default_bucket: str) -> RestoreConfig:
    bucket = os.environ.get("R2_BUCKET") or default_bucket
    endpoint_url = os.environ.get("R2_ENDPOINT_URL")
    access_key_id = os.environ.get("R2_ACCESS_KEY_ID")
    secret_access_key = os.environ.get("R2_SECRET_ACCESS_KEY")
    region = os.environ.get("AWS_DEFAULT_REGION") or DEFAULT_R2_REGION

    missing = [
        name
        for name, value in (
            ("R2_BUCKET", bucket),
            ("R2_ENDPOINT_URL", endpoint_url),
            ("R2_ACCESS_KEY_ID", access_key_id),
            ("R2_SECRET_ACCESS_KEY", secret_access_key),
        )
        if not value
    ]
    if missing:
        raise SystemExit(f"missing R2 environment variables: {', '.join(missing)}")

    parsed = urlparse(str(endpoint_url).rstrip("/"))
    if not parsed.scheme or not parsed.netloc:
        raise SystemExit(f"invalid R2_ENDPOINT_URL: {endpoint_url!r}")

    if parsed.path and parsed.path != "/":
        print(
            "warning: R2_ENDPOINT_URL includes a path; stripping it and using "
            "R2_BUCKET for the bucket name.",
            file=sys.stderr,
        )

    normalized_endpoint = urlunparse((parsed.scheme, parsed.netloc, "", "", "", ""))
    return RestoreConfig(
        bucket=str(bucket),
        endpoint_url=normalized_endpoint,
        access_key_id=str(access_key_id),
        secret_access_key=str(secret_access_key),
        region=region,
    )


def _make_s3_client(config: RestoreConfig) -> Any:
    try:
        boto3 = importlib.import_module("boto3")
        botocore_client = importlib.import_module("botocore.client")
    except ModuleNotFoundError as exc:
        raise SystemExit(
            "boto3 is required for R2 restore. Run:\n"
            "  uv run --with boto3 python scripts/restore_kicad_cli_cache.py"
        ) from exc

    return boto3.client(
        "s3",
        endpoint_url=config.endpoint_url,
        aws_access_key_id=config.access_key_id,
        aws_secret_access_key=config.secret_access_key,
        region_name=config.region,
        config=botocore_client.Config(signature_version="s3v4", s3={"addressing_style": "path"}),
    )


def _select_cache_entry(manifest_path: Path, short_hash: str | None) -> KicadCliCacheEntry:
    data = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    versions = data.get("versions")
    if not isinstance(versions, list):
        raise RuntimeError(f"manifest has no [[versions]] entries: {manifest_path}")

    candidates: list[dict[str, Any]] = []
    for raw_entry in versions:
        if not isinstance(raw_entry, dict):
            continue
        if short_hash is not None and raw_entry.get("short_hash") != short_hash:
            continue
        if all(raw_entry.get(key) for key in ("r2_bucket", "r2_prefix", "r2_archive", "r2_sha256")):
            candidates.append(raw_entry)

    if not candidates:
        if short_hash is None:
            raise RuntimeError("manifest has no R2-backed kicad-cli versions")
        raise RuntimeError(f"manifest has no R2-backed version with short_hash={short_hash!r}")

    raw = candidates[-1]
    prefix = str(raw["r2_prefix"]).strip("/")
    return KicadCliCacheEntry(
        short_hash=str(raw["short_hash"]),
        r2_bucket=str(raw["r2_bucket"]),
        r2_prefix=f"{prefix}/",
        r2_archive=str(raw["r2_archive"]),
        r2_sha256=str(raw["r2_sha256"]).lower(),
    )


def _target_dir(args: argparse.Namespace, entry: KicadCliCacheEntry) -> Path:
    if args.target_dir is not None:
        return args.target_dir.resolve()
    corpus_root = args.corpus_root or Path(os.environ.get("WN_TEST_CORPUS", REPO_ROOT / "tests" / "corpus"))
    return (corpus_root / "tools" / "kicad-cli" / entry.short_hash).resolve()


def _download_file(s3: Any, bucket: str, key: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    print(f"download=s3://{bucket}/{key}")
    s3.download_file(bucket, key, str(path))


def _load_cache_manifest(path: Path, entry: KicadCliCacheEntry) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != EXPECTED_SCHEMA:
        raise RuntimeError(f"unexpected cache manifest schema: {data.get('schema')!r}")
    if data.get("cache_key") not in entry.r2_prefix:
        raise RuntimeError("cache manifest cache_key does not match repo R2 prefix")
    archive = data.get("archive")
    if not isinstance(archive, dict):
        raise RuntimeError("cache manifest archive block is missing")
    if archive.get("name") != entry.r2_archive:
        raise RuntimeError(
            f"cache manifest archive name mismatch: {archive.get('name')!r} != {entry.r2_archive!r}"
        )
    if not archive.get("sha256"):
        raise RuntimeError("cache manifest archive SHA-256 is missing")
    return data


def _parse_sha256_sidecar(path: Path) -> str:
    text = path.read_text(encoding="ascii").strip()
    value = text.split()[0] if text else ""
    if len(value) != 64:
        raise RuntimeError(f"invalid SHA-256 sidecar: {path}")
    return value.lower()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _extract_zip_safely(archive_path: Path, extract_dir: Path) -> None:
    extract_dir.mkdir(parents=True, exist_ok=False)
    root = extract_dir.resolve()
    with zipfile.ZipFile(archive_path) as archive:
        for member in archive.infolist():
            destination = (extract_dir / member.filename).resolve()
            if destination != root and root not in destination.parents:
                raise RuntimeError(f"unsafe archive member path: {member.filename!r}")
        archive.extractall(extract_dir)


def _validate_extracted_bundle(extract_dir: Path) -> None:
    required = [
        extract_dir / "bin" / "kicad-cli.exe",
        extract_dir / "bin" / "_pcbnew.dll",
        extract_dir / "share",
        extract_dir / "provenance.json",
    ]
    missing = [path for path in required if not path.exists()]
    if missing:
        joined = "\n".join(str(path) for path in missing)
        raise RuntimeError(f"restored bundle is missing required paths:\n{joined}")


def _install_extracted_tree(extract_dir: Path, target_dir: Path, *, force: bool) -> None:
    target_parent = target_dir.parent.resolve()
    if not target_parent.exists():
        target_parent.mkdir(parents=True, exist_ok=True)
    target_parent = target_parent.resolve()
    resolved_target = target_dir.resolve()

    if target_parent not in resolved_target.parents:
        raise RuntimeError(f"target directory is outside its parent: {target_dir}")
    if resolved_target == target_parent or resolved_target.anchor == str(resolved_target):
        raise RuntimeError(f"refusing unsafe target directory: {target_dir}")

    if target_dir.exists():
        if not force:
            raise RuntimeError(f"target already exists: {target_dir}")
        shutil.rmtree(target_dir)

    shutil.move(str(extract_dir), str(target_dir))
    print(f"installed={target_dir}")


if __name__ == "__main__":
    raise SystemExit(main())
