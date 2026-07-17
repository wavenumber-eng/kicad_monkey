"""Date-based version helpers for KiCad Monkey."""

from __future__ import annotations

import re
from dataclasses import dataclass
from datetime import date
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as distribution_version

__version__ = "2026.7.16a0"

_DISTRIBUTION_NAME = "kicad-monkey"
_VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:\.(\d+)|\.?a(\d+))?$")


@dataclass(frozen=True, slots=True)
class Version:
    """Parsed package version using the project date-version contract."""

    major: int
    minor: int
    patch: int
    string: str
    build: int | None = None
    alpha: int | None = None

    @property
    def release_date(self) -> date:
        """Return the calendar release date encoded by the version."""
        return date(self.major, self.minor, self.patch)

    @property
    def is_prerelease(self) -> bool:
        """Return True for alpha release-candidate builds."""
        return self.alpha is not None


def parse_version(raw_version: str) -> Version:
    """Parse a supported date-based KiCad Monkey version string."""
    match = _VERSION_RE.match(raw_version)
    if match is None:
        raise ValueError(f"Unsupported KiCad Monkey version: {raw_version!r}")

    major = int(match.group(1))
    minor = int(match.group(2))
    patch = int(match.group(3))
    build = int(match.group(4)) if match.group(4) is not None else None
    alpha = int(match.group(5)) if match.group(5) is not None else None
    version_string = f"{major}.{minor}.{patch}"
    if build is not None:
        version_string = f"{version_string}.{build}"
    if alpha is not None:
        version_string = f"{version_string}a{alpha}"
    date(major, minor, patch)
    return Version(
        major=major,
        minor=minor,
        patch=patch,
        build=build,
        alpha=alpha,
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


__all__ = ["Version", "__version__", "parse_version", "version"]
