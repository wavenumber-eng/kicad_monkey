"""Contained directory-tree publication for Cruncher artifact commands."""

from __future__ import annotations

from pathlib import Path


def publish_staged_tree(staging: Path, destination: Path) -> None:
    """Replace a destination with one complete sibling staging tree."""

    if not staging.is_dir():
        return
    destination = destination.resolve()
    backup = staging.parent / "previous"
    had_destination = destination.exists()
    if had_destination:
        destination.replace(backup)
    try:
        staging.replace(destination)
    except Exception:
        if had_destination and backup.exists() and not destination.exists():
            backup.replace(destination)
        raise
