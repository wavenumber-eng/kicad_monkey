"""KiCad installation and configuration discovery."""

from __future__ import annotations

import json
import os
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

__all__ = ["KiCadEnvironment", "KiCadInstallation"]


_KICAD_EXECUTABLE_NAMES = ("kicad.exe", "kicad")
_KICAD_CLI_NAMES = ("kicad-cli.exe", "kicad-cli")


def _parse_version_pair(name: str) -> tuple[int, int] | None:
    parts = name.split(".")
    if len(parts) != 2 or not all(part.isdigit() for part in parts):
        return None
    return int(parts[0]), int(parts[1])


@dataclass(frozen=True, slots=True)
class KiCadInstallation:
    """One detected KiCad installation root."""

    root: Path

    def __post_init__(self) -> None:
        object.__setattr__(self, "root", Path(self.root))

    @property
    def bin_dir(self) -> Path:
        return self.root / "bin"

    @property
    def version(self) -> tuple[int, int] | None:
        return _parse_version_pair(self.root.name)

    @property
    def version_text(self) -> str:
        return self.root.name if self.version is not None else ""

    @property
    def is_beta(self) -> bool:
        version = self.version
        return version is not None and version[1] == 99

    @property
    def kicad_cli(self) -> Path:
        for name in _KICAD_CLI_NAMES:
            candidate = self.bin_dir / name
            if candidate.exists():
                return candidate
        return self.bin_dir / "kicad-cli"

    @property
    def kicad_exe(self) -> Path:
        for name in _KICAD_EXECUTABLE_NAMES:
            candidate = self.bin_dir / name
            if candidate.exists():
                return candidate
        return self.bin_dir / "kicad"


class KiCadEnvironment:
    """Discover local KiCad installations and user configuration directories."""

    _MODEL_DIR_PARTS = ("share", "kicad", "3dmodels")

    def __init__(
        self,
        *,
        env: Mapping[str, str] | None = None,
        platform: str | None = None,
    ) -> None:
        self._env = dict(os.environ if env is None else env)
        self._env_casefold = {key.casefold(): value for key, value in self._env.items()}
        self._platform = platform or sys.platform

    def find_installations(self) -> list[KiCadInstallation]:
        """Return detected KiCad installations sorted by filesystem path."""
        roots: set[Path] = set()

        for env_name in (
            "KICAD_EXE",
            "KICAD_CLI",
            "KICAD_INSTALL_ROOT",
            "KICAD_HOME",
            "KICAD_PATH",
        ):
            self._add_installation_candidate(self._env_value(env_name), roots)

        for root in self._standard_install_roots():
            if not root.exists():
                continue
            self._add_installation_candidate(root, roots)
            for child in root.iterdir():
                if child.is_dir():
                    self._add_installation_candidate(child, roots)

        return [KiCadInstallation(root) for root in sorted(roots, key=lambda path: str(path).casefold())]

    def highest_installation(
        self,
        installations: Sequence[KiCadInstallation | str | Path] | None = None,
        *,
        ignore_beta: bool = True,
    ) -> KiCadInstallation | None:
        """Return the highest versioned KiCad installation, optionally skipping .99 beta builds."""
        candidates = self.find_installations() if installations is None else [
            self._coerce_installation(installation) for installation in installations
        ]
        versioned = [
            installation
            for installation in candidates
            if installation.version is not None
            and not (ignore_beta and installation.is_beta)
        ]
        if not versioned:
            return None
        return max(versioned, key=lambda installation: installation.version or (0, 0))

    def find_config_paths(self, *, min_major: int | None = None) -> list[Path]:
        """Return KiCad user configuration version directories sorted by version."""
        base_config_path = self._base_config_path()
        if base_config_path is None or not base_config_path.exists():
            return []

        version_paths: list[Path] = []
        for item in base_config_path.iterdir():
            version = _parse_version_pair(item.name)
            if not item.is_dir() or version is None:
                continue
            if min_major is not None and version[0] < min_major:
                continue
            version_paths.append(item)

        return sorted(version_paths, key=lambda path: _parse_version_pair(path.name) or (0, 0))

    def model_directory_variables(self) -> dict[str, str]:
        """Return KiCad's implicit ``KICAD<major>_3DMODEL_DIR`` path variables.

        KiCad defines ``KICAD<major>_3DMODEL_DIR`` for its own version, pointing at
        ``<install>/share/kicad/3dmodels``.  KiCad only defines the variable for the
        running major, so a project authored under a different KiCad version names a
        variable that is never set when tooling runs outside KiCad.  This returns the
        mapping for every detected installation that ships a 3D model directory, so
        callers can resolve references that name another KiCad major version.
        """
        variables: dict[str, tuple[tuple[int, int], str]] = {}
        for installation in self.find_installations():
            version = installation.version
            if version is None:
                continue
            model_dir = installation.root.joinpath(*self._MODEL_DIR_PARTS)
            if not model_dir.exists():
                continue
            name = f"KICAD{version[0]}_3DMODEL_DIR"
            existing = variables.get(name)
            if existing is None or version > existing[0]:
                variables[name] = (version, str(model_dir))
        return {name: value for name, (_version, value) in variables.items()}

    def config_path_variables(self, *, version: tuple[int, int] | None = None) -> dict[str, str]:
        """Return user ``environment.vars`` overrides from ``kicad_common.json``.

        These are the custom path variables a user defines under
        Preferences > Configure Paths (for example ``${ANT3DMDL}``).  Returns an
        empty mapping when the file, section, or value is absent or null.
        """
        data = self._read_kicad_common(version)
        environment = data.get("environment") if isinstance(data, dict) else None
        raw = environment.get("vars") if isinstance(environment, dict) else None
        if not isinstance(raw, dict):
            return {}
        return {str(key): str(value) for key, value in raw.items() if value is not None}

    def extra_3d_model_search_dirs(self, *, version: tuple[int, int] | None = None) -> tuple[Path, ...]:
        """Return ``system.extra_3d_search_dirs`` from ``kicad_common.json``."""
        data = self._read_kicad_common(version)
        system = data.get("system") if isinstance(data, dict) else None
        raw = system.get("extra_3d_search_dirs") if isinstance(system, dict) else None
        if not isinstance(raw, list):
            return ()
        return tuple(Path(str(item)) for item in raw if item)

    def path_variable_map(self, *, version: tuple[int, int] | None = None) -> dict[str, str]:
        """Return install model-dir variables overlaid with user config overrides.

        Installation defaults provide every ``KICAD<major>_3DMODEL_DIR``; user
        ``environment.vars`` overrides win on conflict, mirroring KiCad's own
        precedence where a configured path variable overrides the built-in default.
        """
        variables = self.model_directory_variables()
        variables.update(self.config_path_variables(version=version))
        return variables

    def _read_kicad_common(self, version: tuple[int, int] | None) -> dict[str, Any] | None:
        config_path = self._config_path_for(version)
        if config_path is None:
            return None
        common = config_path / "kicad_common.json"
        if not common.is_file():
            return None
        try:
            data = json.loads(common.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None
        return data if isinstance(data, dict) else None

    def _config_path_for(self, version: tuple[int, int] | None) -> Path | None:
        paths = self.find_config_paths()
        if not paths:
            return None
        if version is not None:
            for path in paths:
                if _parse_version_pair(path.name) == version:
                    return path
            return None
        non_beta = [path for path in paths if (_parse_version_pair(path.name) or (0, 99))[1] != 99]
        candidates = non_beta or paths
        return max(candidates, key=lambda path: _parse_version_pair(path.name) or (0, 0))

    def _env_value(self, name: str) -> str | None:
        return self._env.get(name) or self._env_casefold.get(name.casefold())

    def _standard_install_roots(self) -> tuple[Path, ...]:
        roots: list[Path] = []
        local_app_data = self._env_value("LOCALAPPDATA")
        if local_app_data:
            roots.append(Path(local_app_data) / "Programs" / "KiCad")

        for env_name in ("PROGRAMFILES", "PROGRAMFILES(X86)"):
            value = self._env_value(env_name)
            if value:
                roots.append(Path(value) / "KiCad")

        if self._platform.startswith("win"):
            roots.extend(
                [
                    Path(r"C:\Program Files\KiCad"),
                    Path(r"C:\Program Files (x86)\KiCad"),
                ]
            )
        return tuple(dict.fromkeys(roots))

    def _base_config_path(self) -> Path | None:
        if self._platform.startswith("win"):
            app_data = self._env_value("APPDATA")
            return Path(app_data) / "kicad" if app_data else None
        if self._platform.startswith("linux"):
            home = Path(self._env_value("HOME") or "~").expanduser()
            return home / ".config" / "kicad"
        if self._platform.startswith("darwin"):
            home = Path(self._env_value("HOME") or "~").expanduser()
            return home / "Library" / "Preferences" / "kicad"
        return None

    def _add_installation_candidate(self, candidate: str | Path | None, roots: set[Path]) -> None:
        root = self._installation_root(candidate)
        if root is not None:
            roots.add(root)

    def _installation_root(self, candidate: str | Path | None) -> Path | None:
        if candidate is None or str(candidate).strip() == "":
            return None

        candidate_path = Path(candidate).expanduser()
        if not candidate_path.exists():
            return None

        normalized = candidate_path
        if normalized.is_file():
            if normalized.name.casefold() in {*_KICAD_EXECUTABLE_NAMES, *_KICAD_CLI_NAMES}:
                normalized = normalized.parent.parent
            else:
                return None
        elif normalized.name.casefold() == "bin":
            normalized = normalized.parent

        return normalized if self._looks_like_installation_root(normalized) else None

    def _coerce_installation(self, installation: KiCadInstallation | str | Path) -> KiCadInstallation:
        if isinstance(installation, KiCadInstallation):
            return installation
        root = self._installation_root(installation) or Path(installation)
        return KiCadInstallation(root)

    @staticmethod
    def _looks_like_installation_root(root: Path) -> bool:
        bin_dir = root / "bin"
        return any((bin_dir / name).exists() for name in (*_KICAD_EXECUTABLE_NAMES, *_KICAD_CLI_NAMES))
