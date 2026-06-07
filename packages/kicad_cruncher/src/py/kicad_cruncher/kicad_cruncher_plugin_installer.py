"""KiCad IPC plugin install helpers for KiCad Cruncher."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import cast

DEFAULT_KICAD_VERSIONS = ("10.0", "9.0")
DEFAULT_PLUGIN_NAME = "kicad-cruncher-tools"
KICAD_EXECUTABLE_NAMES = ("kicad.exe", "kicad-cli.exe", "kicad", "kicad-cli")
PYTHON_EXECUTABLE_NAMES = ("python.exe", "pythonw.exe", "python3.exe", "python", "python3")
EXCLUDED_DIRS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "tests",
}
EXCLUDED_SUFFIXES = {".pyc", ".pyo"}


class _PluginInstallTarget:
    """One KiCad user plugin folder target."""

    def __init__(self, version: str | None, plugins_dir: Path) -> None:
        self.version = version
        self.plugins_dir = plugins_dir


class _PluginInstallResult:
    """One plugin installation or dry-run result."""

    def __init__(
        self,
        *,
        source_dir: Path,
        target_dir: Path,
        version: str | None,
        dry_run: bool,
    ) -> None:
        self.source_dir = source_dir
        self.target_dir = target_dir
        self.version = version
        self.dry_run = dry_run


class _KiCadApiConfig:
    """Observed KiCad IPC API preference state."""

    def __init__(
        self,
        *,
        version: str,
        config_path: Path,
        config_exists: bool,
        api_enabled: bool | None,
        interpreter_path: Path | None,
        interpreter_exists: bool | None,
    ) -> None:
        self.version = version
        self.config_path = config_path
        self.config_exists = config_exists
        self.api_enabled = api_enabled
        self.interpreter_path = interpreter_path
        self.interpreter_exists = interpreter_exists


def available_plugin_names() -> tuple[str, ...]:
    """Return installable built-in plugin package names."""
    return (DEFAULT_PLUGIN_NAME,)


def plugin_package_root(plugin_name: str) -> Path:
    """Return the source path for a bundled KiCad IPC plugin package."""
    source = Path(__file__).resolve().parent / "kicad_plugins" / plugin_name
    if plugin_name not in available_plugin_names():
        raise ValueError(f"Unknown KiCad Cruncher plugin package: {plugin_name}")
    if not (source / "plugin.json").is_file():
        raise FileNotFoundError(f"plugin.json not found in plugin package: {source}")
    return source


def plugin_identifier(source_dir: Path) -> str:
    """Return the KiCad plugin identifier declared by a plugin package."""
    metadata = json.loads((source_dir / "plugin.json").read_text(encoding="utf-8"))
    if not isinstance(metadata, dict):
        raise ValueError(f"{source_dir / 'plugin.json'} must contain a JSON object")
    identifier = str(metadata.get("identifier", "")).strip()
    if not identifier:
        raise ValueError(f"Plugin identifier missing from {source_dir / 'plugin.json'}")
    return identifier


def known_documents_dir() -> Path:
    """Return the platform documents folder used by KiCad user plugin paths."""
    if os.name != "nt":
        return Path.home() / "Documents"

    try:
        import ctypes
        from ctypes import wintypes

        class GUID(ctypes.Structure):
            _fields_ = [
                ("Data1", wintypes.DWORD),
                ("Data2", wintypes.WORD),
                ("Data3", wintypes.WORD),
                ("Data4", wintypes.BYTE * 8),
            ]

        folder_id_documents = GUID(
            0xFDD39AD0,
            0x238F,
            0x46AF,
            (wintypes.BYTE * 8)(0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7),
        )
        path_ptr = wintypes.LPWSTR()
        result = ctypes.windll.shell32.SHGetKnownFolderPath(
            ctypes.byref(folder_id_documents), 0, None, ctypes.byref(path_ptr)
        )
        if result == 0 and path_ptr.value:
            path = Path(path_ptr.value)
            ctypes.windll.ole32.CoTaskMemFree(path_ptr)
            return path
    except Exception:
        pass

    return Path.home() / "Documents"


def candidate_documents_roots() -> list[Path]:
    """Return likely KiCad documents roots."""
    roots: list[Path] = []
    env_root = os.environ.get("KICAD_DOCUMENTS_HOME")
    if env_root:
        roots.append(Path(env_root).expanduser())
    roots.append(known_documents_dir() / "KiCad")
    roots.append(Path.home() / "Documents" / "KiCad")
    return _unique_paths(roots)


def candidate_config_roots() -> list[Path]:
    """Return likely KiCad config roots."""
    roots: list[Path] = []
    env_root = os.environ.get("KICAD_CONFIG_HOME")
    if env_root:
        roots.append(Path(env_root).expanduser())
    if os.name == "nt":
        appdata = os.environ.get("APPDATA")
        if appdata:
            roots.append(Path(appdata) / "kicad")
        roots.append(Path.home() / "AppData" / "Roaming" / "kicad")
    elif sys.platform == "darwin":
        roots.append(Path.home() / "Library" / "Preferences" / "kicad")
    else:
        xdg_config = os.environ.get("XDG_CONFIG_HOME")
        config_root = Path(xdg_config).expanduser() if xdg_config else Path.home() / ".config"
        roots.append(config_root / "kicad")
    return _unique_paths(roots)


def _add_kicad_install_candidate(candidates: list[Path], candidate: str | Path | None) -> None:
    if not candidate:
        return

    path = Path(candidate).expanduser()
    if not path.exists():
        return

    if path.is_file():
        if path.name.lower() not in KICAD_EXECUTABLE_NAMES:
            return
        path = path.parent.parent
    elif path.name.lower() == "bin":
        path = path.parent

    bin_dir = path / "bin"
    if any((bin_dir / name).exists() for name in KICAD_EXECUTABLE_NAMES) and not any(
        existing == path for existing in candidates
    ):
        candidates.append(path)


def discover_kicad_installs() -> list[Path]:
    """Return likely KiCad install roots for interpreter and version diagnostics."""
    candidates: list[Path] = []
    _add_env_kicad_install_candidates(candidates)
    if os.name == "nt":
        _add_windows_kicad_install_candidates(candidates)
    return candidates


def _add_env_kicad_install_candidates(candidates: list[Path]) -> None:
    for env_name in ("KICAD_EXE", "KICAD_CLI", "KICAD_INSTALL_ROOT", "KICAD_HOME", "KICAD_PATH"):
        _add_kicad_install_candidate(candidates, os.environ.get(env_name))


def _add_windows_kicad_install_candidates(candidates: list[Path]) -> None:
    local_programs = Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "KiCad"
    _add_kicad_root_and_children(candidates, local_programs)

    for program_root in (
        os.environ.get("PROGRAMFILES"),
        os.environ.get("PROGRAMFILES(X86)"),
        r"C:\Program Files",
        r"C:\Program Files (x86)",
    ):
        if not program_root:
            continue
        _add_kicad_root_and_children(candidates, Path(program_root) / "KiCad")


def _add_kicad_root_and_children(candidates: list[Path], root: Path) -> None:
    _add_kicad_install_candidate(candidates, root)
    if not root.exists():
        return
    for child in root.iterdir():
        if child.is_dir():
            _add_kicad_install_candidate(candidates, child)


def _install_version(install_root: Path) -> str:
    return install_root.name


def installed_kicad_version(version: str) -> str | None:
    """Return the first matching KiCad executable version string, if available."""
    executable_names = ("kicad-cli.exe", "kicad-cli") if os.name == "nt" else ("kicad-cli",)
    for install_root in discover_kicad_installs():
        if _install_version(install_root) != version:
            continue
        for executable_name in executable_names:
            executable = install_root / "bin" / executable_name
            if not executable.exists():
                continue
            try:
                completed = subprocess.run(
                    [str(executable), "version"],
                    capture_output=True,
                    check=False,
                    text=True,
                    timeout=10,
                )
            except Exception:
                continue
            output = (completed.stdout or completed.stderr).strip()
            if completed.returncode == 0 and output:
                return output.splitlines()[0].strip()
    return None


def find_default_python_interpreter(version: str, current: Path | None = None) -> Path | None:
    """Find the Python interpreter bundled with the matching KiCad install."""
    if current is not None and current.exists():
        return current

    matches: list[Path] = []
    for install_root in discover_kicad_installs():
        if _install_version(install_root) != version:
            continue
        bin_dir = install_root / "bin"
        for executable_name in PYTHON_EXECUTABLE_NAMES:
            candidate = bin_dir / executable_name
            if candidate.exists():
                matches.append(candidate)
    return matches[0] if matches else None


def _unique_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = str(path.resolve() if path.exists() else path.absolute()).lower()
        if key in seen:
            continue
        seen.add(key)
        unique.append(path)
    return unique


def discover_plugin_targets(
    *,
    plugins_dir: Path | None = None,
    kicad_version: str | None = None,
    create_default: bool = False,
) -> list[_PluginInstallTarget]:
    """Discover KiCad user plugin folders."""
    if plugins_dir is not None:
        return [_PluginInstallTarget(kicad_version, plugins_dir)]

    targets: list[_PluginInstallTarget] = []
    for root in candidate_documents_roots():
        if kicad_version:
            targets.append(_PluginInstallTarget(kicad_version, root / kicad_version / "plugins"))
            continue
        if root.is_dir():
            targets.extend(_versioned_plugin_targets(root))
        if create_default and not targets:
            version = DEFAULT_KICAD_VERSIONS[0]
            targets.append(_PluginInstallTarget(version, root / version / "plugins"))
    return _unique_targets(targets)


def _versioned_plugin_targets(root: Path) -> list[_PluginInstallTarget]:
    targets: list[_PluginInstallTarget] = []
    for child in sorted(root.iterdir(), key=lambda item: item.name, reverse=True):
        if child.is_dir() and child.name[:1].isdigit():
            targets.append(_PluginInstallTarget(child.name, child / "plugins"))
    return targets


def _unique_targets(targets: list[_PluginInstallTarget]) -> list[_PluginInstallTarget]:
    unique: list[_PluginInstallTarget] = []
    seen: set[str] = set()
    for target in targets:
        key = str(target.plugins_dir.absolute()).lower()
        if key in seen:
            continue
        seen.add(key)
        unique.append(target)
    return unique


def inspect_api_config(version: str) -> _KiCadApiConfig:
    """Inspect KiCad IPC API preferences for one KiCad version."""
    config_path = kicad_common_path(version)
    if not config_path.exists():
        return _KiCadApiConfig(
            version=version,
            config_path=config_path,
            config_exists=False,
            api_enabled=None,
            interpreter_path=None,
            interpreter_exists=None,
        )

    payload = _load_json_object(config_path)
    api = payload.get("api", {})
    if not isinstance(api, dict):
        api = {}
    interpreter_text = str(api.get("interpreter_path", "")).strip()
    interpreter_path = Path(interpreter_text) if interpreter_text else None
    return _KiCadApiConfig(
        version=version,
        config_path=config_path,
        config_exists=True,
        api_enabled=bool(api.get("enable_server", False)),
        interpreter_path=interpreter_path,
        interpreter_exists=interpreter_path.exists() if interpreter_path else False,
    )


def configure_api(
    version: str,
    *,
    enable_api: bool,
    python_interpreter: Path | None,
    dry_run: bool,
) -> list[str]:
    """Configure KiCad IPC API preferences for one KiCad version."""
    if not enable_api and python_interpreter is None:
        return []
    config_path = kicad_common_path(version)
    payload = _load_json_object(config_path) if config_path.exists() else {}
    api = payload.get("api", {})
    if not isinstance(api, dict):
        api = {}
    payload["api"] = api

    changes = _apply_api_changes(api, enable_api=enable_api, python_interpreter=python_interpreter)
    if changes and not dry_run:
        config_path.parent.mkdir(parents=True, exist_ok=True)
        config_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return changes


def _apply_api_changes(
    api: dict[object, object],
    *,
    enable_api: bool,
    python_interpreter: Path | None,
) -> list[str]:
    changes: list[str] = []
    if enable_api and api.get("enable_server") is not True:
        api["enable_server"] = True
        changes.append("enable KiCad IPC API")
    if python_interpreter is not None:
        interpreter = str(python_interpreter)
        if str(api.get("interpreter_path", "")) != interpreter:
            api["interpreter_path"] = interpreter
            changes.append(f"set Python interpreter to {interpreter}")
    return changes


def install_plugin(
    plugin_name: str = DEFAULT_PLUGIN_NAME,
    *,
    plugins_dir: Path | None = None,
    kicad_version: str | None = None,
    dry_run: bool = False,
    create_default: bool = False,
) -> list[_PluginInstallResult]:
    """Install or dry-run install of one built-in KiCad Cruncher plugin package."""
    source_dir = plugin_package_root(plugin_name)
    identifier = plugin_identifier(source_dir)
    targets = discover_plugin_targets(
        plugins_dir=plugins_dir,
        kicad_version=kicad_version,
        create_default=create_default,
    )
    if not targets:
        raise FileNotFoundError(
            "No KiCad plugin directory found. Pass --plugins-dir or --kicad-version "
            "after KiCad has created its user documents folder."
        )

    results: list[_PluginInstallResult] = []
    for target in targets:
        target_dir = target.plugins_dir / identifier
        results.append(
            _PluginInstallResult(
                source_dir=source_dir,
                target_dir=target_dir,
                version=target.version,
                dry_run=dry_run,
            )
        )
        if not dry_run:
            _copy_plugin_tree(source_dir, target.plugins_dir, target_dir)
    return results


def uninstall_plugin(
    plugin_name: str = DEFAULT_PLUGIN_NAME,
    *,
    plugins_dir: Path | None = None,
    kicad_version: str | None = None,
    dry_run: bool = False,
) -> list[_PluginInstallResult]:
    """Uninstall or dry-run uninstall of one built-in KiCad Cruncher plugin package."""
    source_dir = plugin_package_root(plugin_name)
    identifier = plugin_identifier(source_dir)
    targets = discover_plugin_targets(plugins_dir=plugins_dir, kicad_version=kicad_version)
    results: list[_PluginInstallResult] = []
    for target in targets:
        target_dir = target.plugins_dir / identifier
        results.append(
            _PluginInstallResult(
                source_dir=source_dir,
                target_dir=target_dir,
                version=target.version,
                dry_run=dry_run,
            )
        )
        if target_dir.exists() and not dry_run:
            _remove_existing_target(target.plugins_dir, target_dir)
    return results


def kicad_common_path(version: str) -> Path:
    """Return the preferred KiCad common config path for one version."""
    for root in candidate_config_roots():
        candidate = root / version / "kicad_common.json"
        if candidate.exists():
            return candidate
    roots = candidate_config_roots()
    if not roots:
        return Path.home() / ".config" / "kicad" / version / "kicad_common.json"
    return roots[0] / version / "kicad_common.json"


def _copy_plugin_tree(source_dir: Path, target_root: Path, target_dir: Path) -> None:
    target_root.mkdir(parents=True, exist_ok=True)
    if target_dir.exists() or target_dir.is_symlink():
        _remove_existing_target(target_root, target_dir)
    shutil.copytree(source_dir, target_dir, ignore=_copy_filter)


def _remove_existing_target(target_root: Path, target_dir: Path) -> None:
    if not _is_relative_to(target_dir, target_root):
        raise RuntimeError(f"Refusing to remove target outside plugin root: {target_dir}")
    if target_dir.is_symlink() or target_dir.is_file():
        target_dir.unlink()
    else:
        shutil.rmtree(target_dir)


def _is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def _copy_filter(_dir_path: str, names: list[str]) -> set[str]:
    ignored: set[str] = set()
    for name in names:
        path = Path(name)
        if name in EXCLUDED_DIRS or path.suffix.lower() in EXCLUDED_SUFFIXES:
            ignored.add(name)
    return ignored


def _load_json_object(path: Path) -> dict[str, object]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return cast(dict[str, object], payload)


def result_lines(results: list[_PluginInstallResult], *, verb: str) -> list[str]:
    """Format install/uninstall result lines."""
    return [f"{verb} {result.source_dir.name} -> {result.target_dir}" for result in results]


def api_report_lines(report: _KiCadApiConfig) -> list[str]:
    """Format KiCad IPC API status lines."""
    if not report.config_exists:
        return [
            f"warning: KiCad {report.version} config not found at {report.config_path}; "
            "open KiCad once or configure it with --enable-api."
        ]
    lines: list[str] = []
    if not report.api_enabled:
        lines.append(
            f"warning: KiCad {report.version} IPC API is disabled in {report.config_path}. "
            "Enable Preferences > Preferences > Plugins > Enable KiCad API, "
            "or reinstall with --enable-api while KiCad is closed."
        )
    if report.interpreter_path is None:
        lines.append(f"warning: KiCad {report.version} has no Python interpreter configured")
    elif not report.interpreter_exists:
        lines.append(f"warning: KiCad {report.version} Python missing: {report.interpreter_path}")
    if report.api_enabled and report.interpreter_exists:
        lines.append(f"KiCad {report.version} IPC config OK: Python={report.interpreter_path}")
    return lines


def version_note_lines(version: str) -> list[str]:
    """Return known KiCad version-specific install notes."""
    if installed_kicad_version(version) == "10.0.0":
        return [
            "warning: KiCad 10.0.0 has a known IPC plugin toolbar refresh issue. "
            "The action may load and become ready but not appear on an already-open "
            "PCB editor toolbar. Upgrade to KiCad 10.0.1 or newer, or open the PCB "
            "editor after the plugin environment is ready."
        ]
    return []


def ipc_plugin_note_lines(results: list[_PluginInstallResult]) -> list[str]:
    """Return post-install notes for KiCad IPC plugin discovery."""
    installed = [result for result in results if not result.dry_run]
    if not installed:
        return []

    roots: list[Path] = []
    seen: set[str] = set()
    for result in installed:
        root = result.target_dir.parent
        key = str(root.absolute()).lower()
        if key in seen:
            continue
        seen.add(key)
        roots.append(root)

    lines = [f"KiCad IPC plugin directory: {root}" for root in roots]
    lines.extend(
        [
            "Validate IPC actions in KiCad via Preferences > Preferences > PCB Editor > "
            "Action Plugins, or by their PCB editor toolbar buttons after restart.",
            "Note: Tools > External Plugins does not list individual IPC actions; some "
            "KiCad 10 builds open the legacy scripting\\plugins folder from that menu.",
        ]
    )
    return lines
