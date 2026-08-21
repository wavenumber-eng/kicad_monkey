"""Cross-platform paths for repository-owned development toolchains."""

from __future__ import annotations

import os
from pathlib import Path


def typespec_executable(package_root: Path, *, platform: str | None = None) -> Path:
    """Return the npm-installed TypeSpec launcher for the selected platform."""
    selected = os.name if platform is None else platform
    launcher = "tsp.cmd" if selected == "nt" else "tsp"
    return package_root / "node_modules" / ".bin" / launcher
