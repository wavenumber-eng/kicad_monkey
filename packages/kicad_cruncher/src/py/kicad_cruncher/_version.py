"""Date-based version helpers for KiCad Cruncher."""

from __future__ import annotations

import re
from dataclasses import dataclass
from datetime import date
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as distribution_version

__version__ = "2026.6.7"

_DISTRIBUTION_NAME = "kicad-cruncher"
_CONTROLLED_DEPENDENCIES = ("kicad-monkey", "wn-geometer")
_VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:\.(\d+))?$")


@dataclass(frozen=True, slots=True)
class Version:
    """Parsed package version using the project date-version contract."""

    major: int
    minor: int
    patch: int
    string: str
    build: int | None = None

    @property
    def release_date(self) -> date:
        """Return the calendar release date encoded by the version."""
        return date(self.major, self.minor, self.patch)


def parse_version(raw_version: str) -> Version:
    """Parse a supported date-based KiCad Cruncher version string."""
    match = _VERSION_RE.match(raw_version)
    if match is None:
        raise ValueError(f"Unsupported KiCad Cruncher version: {raw_version!r}")

    major = int(match.group(1))
    minor = int(match.group(2))
    patch = int(match.group(3))
    build = int(match.group(4)) if match.group(4) is not None else None
    version_string = f"{major}.{minor}.{patch}"
    if build is not None:
        version_string = f"{version_string}.{build}"
    date(major, minor, patch)
    return Version(
        major=major,
        minor=minor,
        patch=patch,
        build=build,
        string=version_string,
    )


def version() -> Version:
    """Return the installed distribution version, falling back to source metadata."""
    try:
        raw_version = distribution_version(_DISTRIBUTION_NAME)
    except PackageNotFoundError:
        raw_version = __version__
    else:
        try:
            return parse_version(raw_version)
        except ValueError:
            raw_version = __version__
    return parse_version(raw_version)


def cli_version_text() -> str:
    """Return the one-line CLI version banner."""
    return f"kicad-cruncher {version().string}"


def dependency_version_text(distribution_name: str) -> str:
    """Return a dependency version line for CLI diagnostics."""
    try:
        dependency_version = distribution_version(distribution_name)
    except PackageNotFoundError:
        dependency_version = "not installed"
    return f"{distribution_name} {dependency_version}"


def cli_version_report() -> str:
    """Return the multi-line CLI version report."""
    lines = [cli_version_text()]
    lines.extend(
        dependency_version_text(distribution_name)
        for distribution_name in _CONTROLLED_DEPENDENCIES
    )
    return "\n".join(lines)


__all__ = [
    "Version",
    "__version__",
    "cli_version_report",
    "cli_version_text",
    "dependency_version_text",
    "parse_version",
    "version",
]
