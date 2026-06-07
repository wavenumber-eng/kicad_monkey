"""KiCad workstation helper command for installs, processes, and preferences."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from kicad_cruncher.kicad_cruncher_plugin_installer import (
    candidate_config_roots,
    candidate_documents_roots,
    discover_kicad_installs,
    find_default_python_interpreter,
    inspect_api_config,
    kicad_common_path,
)

APP_EXECUTABLES = {
    "kicad": ("kicad.exe", "kicad"),
    "pcbnew": ("pcbnew.exe", "pcbnew"),
    "eeschema": ("eeschema.exe", "eeschema"),
    "kicad-cli": ("kicad-cli.exe", "kicad-cli"),
}
PROCESS_NAMES = frozenset(
    name.lower()
    for names in APP_EXECUTABLES.values()
    for name in names
)


@dataclass(frozen=True, slots=True)
class KiCadProcess:
    """One running KiCad-family process."""

    pid: int
    name: str
    executable: str | None
    command_line: str | None
    version: str | None

    def to_json(self) -> dict[str, object]:
        """Return a JSON-serializable process record."""
        return {
            "pid": self.pid,
            "name": self.name,
            "executable": self.executable,
            "command_line": self.command_line,
            "version": self.version,
        }


def cmd_kicad(args: argparse.Namespace) -> int:
    """Run one KiCad workstation helper subcommand."""
    action = str(getattr(args, "kicad_action", ""))
    if action == "installs":
        return _cmd_installs(args)
    if action == "running":
        return _cmd_running(args)
    if action == "launch":
        return _cmd_launch(args)
    if action == "stop":
        return _cmd_stop(args)
    if action == "prefs":
        return _cmd_prefs(args)
    print("kicad subcommand required")
    return 2


def _cmd_installs(args: argparse.Namespace) -> int:
    records = install_records(version=getattr(args, "version", None))
    payload = {"schema": "kicad_cruncher.kicad.installs.v0", "installs": records}
    if bool(getattr(args, "json", False)):
        print(json.dumps(payload, indent=2))
        return 0
    if not records:
        print("No KiCad installs discovered.")
        return 0
    for record in records:
        print(_install_summary(record))
    return 0


def _cmd_running(args: argparse.Namespace) -> int:
    processes = filter_processes(
        running_kicad_processes(),
        version=getattr(args, "version", None),
        app=getattr(args, "app", None),
    )
    payload = {
        "schema": "kicad_cruncher.kicad.running.v0",
        "processes": [process.to_json() for process in processes],
    }
    if bool(getattr(args, "json", False)):
        print(json.dumps(payload, indent=2))
        return 0
    if not processes:
        print("No running KiCad processes found.")
        return 0
    for process in processes:
        version = process.version or "unknown-version"
        executable = process.executable or "unknown-executable"
        print(f"{process.pid} {process.name} {version} {executable}")
    return 0


def _cmd_launch(args: argparse.Namespace) -> int:
    app = str(getattr(args, "app", "kicad"))
    try:
        executable = _resolve_launch_executable(
            app=app,
            version=getattr(args, "version", None),
            explicit=getattr(args, "exe", None),
        )
    except RuntimeError as exc:
        print(f"error: {exc}")
        return 1

    command = [str(executable)]
    if bool(getattr(args, "new", False)):
        if app != "kicad":
            print("error: --new is only supported when launching --app kicad")
            return 1
        command.append("--new")
    project = getattr(args, "project", None)
    if project is not None:
        command.append(str(project))
    command.extend(_passthrough_args(getattr(args, "args", [])))

    dry_run = bool(getattr(args, "dry_run", False))
    pid: int | None = None
    if not dry_run:
        process = subprocess.Popen(command)  # noqa: S603
        pid = process.pid

    payload = {
        "schema": "kicad_cruncher.kicad.launch.v0",
        "dry_run": dry_run,
        "command": command,
        "pid": pid,
    }
    if bool(getattr(args, "json", False)):
        print(json.dumps(payload, indent=2))
    elif dry_run:
        print("Would launch: " + _quote_command(command))
    else:
        print(f"Launched {app}: pid={pid} command={_quote_command(command)}")
    return 0


def _cmd_stop(args: argparse.Namespace) -> int:
    processes = filter_processes(
        running_kicad_processes(),
        version=getattr(args, "version", None),
        app=getattr(args, "app", None),
    )
    confirmed = bool(getattr(args, "all", False))
    stopped: list[dict[str, object]] = []
    for process in processes:
        if confirmed:
            stopped.append(_terminate_process(process))
        else:
            stopped.append({"pid": process.pid, "name": process.name, "status": "dry-run"})

    payload = {
        "schema": "kicad_cruncher.kicad.stop.v0",
        "dry_run": not confirmed,
        "processes": [process.to_json() for process in processes],
        "results": stopped,
    }
    if bool(getattr(args, "json", False)):
        print(json.dumps(payload, indent=2))
        return 0
    if not processes:
        print("No matching KiCad processes found.")
        return 0
    verb = "Stopped" if confirmed else "Would stop"
    for process in processes:
        print(f"{verb}: {process.pid} {process.name} {process.executable or ''}".rstrip())
    if not confirmed:
        print("Pass --all to terminate these processes.")
    return 0


def _cmd_prefs(args: argparse.Namespace) -> int:
    version = getattr(args, "version", None)
    records = preference_records(version=version)
    open_target = getattr(args, "open_target", None)
    if open_target:
        try:
            opened = _open_preference_target(records, open_target)
        except RuntimeError as exc:
            print(f"error: {exc}")
            return 1
    else:
        opened = None

    payload: dict[str, object] = {
        "schema": "kicad_cruncher.kicad.prefs.v0",
        "preferences": records,
    }
    if opened is not None:
        payload["opened"] = opened
    if bool(getattr(args, "json", False)):
        print(json.dumps(payload, indent=2))
        return 0
    if not records:
        print("No KiCad preference directories discovered.")
        return 0
    for record in records:
        print(_preference_summary(record))
    if opened is not None:
        print(f"Opened {open_target}: {opened}")
    return 0


def install_records(*, version: str | None = None) -> list[dict[str, object]]:
    """Return detected KiCad install records."""
    records: list[dict[str, object]] = []
    for root in sorted(discover_kicad_installs(), key=lambda path: path.name, reverse=True):
        install_version = root.name
        if version is not None and install_version != version:
            continue
        executables = {
            app: str(path)
            for app, path in _install_executables(root).items()
            if path is not None
        }
        records.append(
            {
                "version": install_version,
                "nightly": is_nightly_version(install_version),
                "install_root": str(root),
                "executables": executables,
                "reported_version": _reported_version_for_install(root),
            }
        )
    return records


def preference_records(*, version: str | None = None) -> list[dict[str, object]]:
    """Return KiCad config/documents/plugin directory records."""
    versions = [version] if version else discovered_kicad_versions()
    records: list[dict[str, object]] = []
    for item in versions:
        api = inspect_api_config(item)
        documents_root = _documents_root_for_version(item)
        plugins_dir = documents_root / "plugins"
        python_path = api.interpreter_path or find_default_python_interpreter(item)
        records.append(
            {
                "version": item,
                "nightly": is_nightly_version(item),
                "config_path": str(kicad_common_path(item)),
                "config_exists": api.config_exists,
                "documents_root": str(documents_root),
                "documents_exists": documents_root.exists(),
                "plugins_dir": str(plugins_dir),
                "plugins_exists": plugins_dir.exists(),
                "api_enabled": api.api_enabled,
                "python_interpreter": str(python_path) if python_path else None,
                "python_exists": python_path.exists() if python_path else None,
            }
        )
    return records


def discovered_kicad_versions() -> list[str]:
    """Return discovered KiCad versions from installs, configs, and documents."""
    versions: set[str] = set()
    for root in discover_kicad_installs():
        if _looks_like_version(root.name):
            versions.add(root.name)
    for root in [*candidate_config_roots(), *candidate_documents_roots()]:
        if not root.is_dir():
            continue
        for child in root.iterdir():
            if child.is_dir() and _looks_like_version(child.name):
                versions.add(child.name)
    return sorted(versions, key=_version_sort_key, reverse=True)


def running_kicad_processes() -> list[KiCadProcess]:
    """Return running KiCad-family processes."""
    if os.name == "nt":
        return _running_processes_windows()
    return _running_processes_posix()


def filter_processes(
    processes: list[KiCadProcess],
    *,
    version: str | None = None,
    app: str | None = None,
) -> list[KiCadProcess]:
    """Filter process records by version and app."""
    app_names = set(APP_EXECUTABLES.get(app, ())) if app else PROCESS_NAMES
    filtered: list[KiCadProcess] = []
    for process in processes:
        if process.name.lower() not in app_names:
            continue
        if version is not None and process.version != version:
            continue
        filtered.append(process)
    return filtered


def is_nightly_version(version: str) -> bool:
    """Return whether a KiCad version folder looks like a nightly/dev build."""
    return any(part == "99" for part in version.split("."))


def _running_processes_windows() -> list[KiCadProcess]:
    command = [
        "powershell",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "$names=@('kicad.exe','pcbnew.exe','eeschema.exe','kicad-cli.exe');"
        "Get-CimInstance Win32_Process | "
        "Where-Object { $names -contains $_.Name } | "
        "Select-Object ProcessId,Name,ExecutablePath,CommandLine | "
        "ConvertTo-Json -Compress",
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            check=False,
            text=True,
            timeout=10,
        )
    except Exception:
        return []
    if completed.returncode != 0 or not completed.stdout.strip():
        return []
    return _processes_from_windows_json(completed.stdout)


def _processes_from_windows_json(text: str) -> list[KiCadProcess]:
    payload = json.loads(text)
    records = payload if isinstance(payload, list) else [payload]
    processes: list[KiCadProcess] = []
    for record in records:
        if not isinstance(record, dict):
            continue
        name = str(record.get("Name", "")).strip()
        if name.lower() not in PROCESS_NAMES:
            continue
        executable = _optional_string(record.get("ExecutablePath"))
        processes.append(
            KiCadProcess(
                pid=int(record.get("ProcessId", 0)),
                name=name,
                executable=executable,
                command_line=_optional_string(record.get("CommandLine")),
                version=_version_from_executable(executable),
            )
        )
    return processes


def _running_processes_posix() -> list[KiCadProcess]:
    try:
        completed = subprocess.run(
            ["ps", "-eo", "pid=,comm=,args="],
            capture_output=True,
            check=False,
            text=True,
            timeout=10,
        )
    except Exception:
        return []
    if completed.returncode != 0:
        return []
    return _processes_from_posix_ps(completed.stdout)


def _processes_from_posix_ps(text: str) -> list[KiCadProcess]:
    processes: list[KiCadProcess] = []
    for line in text.splitlines():
        parts = line.strip().split(None, 2)
        if len(parts) < 2:
            continue
        pid_text, name = parts[:2]
        command_line = parts[2] if len(parts) > 2 else None
        executable = _first_command_token(command_line)
        if name.lower() not in PROCESS_NAMES:
            continue
        processes.append(
            KiCadProcess(
                pid=int(pid_text),
                name=name,
                executable=executable,
                command_line=command_line,
                version=_version_from_executable(executable),
            )
        )
    return processes


def _resolve_launch_executable(
    *,
    app: str,
    version: str | None,
    explicit: Path | None,
) -> Path:
    resolved = _resolve_explicit_executable(explicit)
    if resolved is not None:
        return resolved
    return _single_launch_candidate(
        app=app,
        version=version,
        candidates=_launch_candidates(app=app, version=version),
    )


def _resolve_explicit_executable(explicit: Path | None) -> Path | None:
    if explicit is not None:
        if not explicit.exists():
            raise RuntimeError(f"explicit executable does not exist: {explicit}")
        return explicit
    return None


def _launch_candidates(*, app: str, version: str | None) -> list[Path]:
    candidates: list[Path] = []
    for root in discover_kicad_installs():
        if version is not None and root.name != version:
            continue
        executable = _install_executables(root).get(app)
        if executable is not None:
            candidates.append(executable)
    return candidates


def _single_launch_candidate(
    *,
    app: str,
    version: str | None,
    candidates: list[Path],
) -> Path:
    if not candidates:
        hint = f" for KiCad {version}" if version else ""
        raise RuntimeError(f"no {app} executable discovered{hint}")
    if version is None and len(candidates) > 1:
        versions = ", ".join(sorted({path.parent.parent.name for path in candidates}))
        raise RuntimeError(
            f"multiple {app} executables discovered ({versions}); pass --version"
        )
    return candidates[0]


def _install_executables(root: Path) -> dict[str, Path | None]:
    bin_dir = root / "bin"
    return {app: _first_existing(bin_dir, names) for app, names in APP_EXECUTABLES.items()}


def _first_existing(directory: Path, names: tuple[str, ...]) -> Path | None:
    for name in names:
        candidate = directory / name
        if candidate.exists():
            return candidate
    return None


def _reported_version_for_install(root: Path) -> str | None:
    executable = _install_executables(root).get("kicad-cli")
    if executable is None:
        return None
    try:
        completed = subprocess.run(
            [str(executable), "version"],
            capture_output=True,
            check=False,
            text=True,
            timeout=10,
        )
    except Exception:
        return None
    output = (completed.stdout or completed.stderr).strip()
    if completed.returncode != 0 or not output:
        return None
    return output.splitlines()[0].strip()


def _documents_root_for_version(version: str) -> Path:
    for root in candidate_documents_roots():
        candidate = root / version
        if candidate.exists():
            return candidate
    roots = candidate_documents_roots()
    if roots:
        return roots[0] / version
    return Path.home() / "Documents" / "KiCad" / version


def _open_preference_target(records: list[dict[str, object]], target: str) -> str:
    if not records:
        raise RuntimeError("no KiCad preference target discovered")
    if len(records) > 1:
        versions = ", ".join(str(record["version"]) for record in records)
        raise RuntimeError(f"multiple preference versions discovered ({versions}); pass --version")
    record = records[0]
    path = _path_for_preference_target(record, target)
    _open_path(path)
    return str(path)


def _path_for_preference_target(record: dict[str, object], target: str) -> Path:
    if target == "config":
        config_path = Path(str(record["config_path"]))
        return config_path if config_path.exists() else config_path.parent
    if target == "documents":
        return Path(str(record["documents_root"]))
    if target == "plugins":
        return Path(str(record["plugins_dir"]))
    raise RuntimeError(f"unknown open target: {target}")


def _open_path(path: Path) -> None:
    if os.name == "nt":
        subprocess.Popen(["explorer", str(path)])  # noqa: S603, S607
    elif sys.platform == "darwin":
        subprocess.Popen(["open", str(path)])  # noqa: S603, S607
    else:
        subprocess.Popen(["xdg-open", str(path)])  # noqa: S603, S607


def _terminate_process(process: KiCadProcess) -> dict[str, object]:
    if os.name == "nt":
        command = ["taskkill", "/PID", str(process.pid), "/T", "/F"]
    else:
        command = ["kill", "-TERM", str(process.pid)]
    completed = subprocess.run(command, capture_output=True, check=False, text=True)
    return {
        "pid": process.pid,
        "name": process.name,
        "status": "stopped" if completed.returncode == 0 else "failed",
        "returncode": completed.returncode,
    }


def _version_from_executable(executable: str | None) -> str | None:
    if not executable:
        return None
    path = Path(executable)
    parts = path.parts
    for index, part in enumerate(parts):
        if part.lower() == "kicad" and index + 1 < len(parts):
            version = parts[index + 1]
            if _looks_like_version(version):
                return version
    if path.parent.name.lower() == "bin" and _looks_like_version(path.parent.parent.name):
        return path.parent.parent.name
    return None


def _looks_like_version(value: str) -> bool:
    return bool(value) and value[0].isdigit()


def _version_sort_key(version: str) -> tuple[int, ...]:
    parts: list[int] = []
    for part in version.split("."):
        try:
            parts.append(int(part))
        except ValueError:
            parts.append(-1)
    return tuple(parts)


def _optional_string(value: object) -> str | None:
    text = str(value).strip() if value is not None else ""
    return text or None


def _first_command_token(command_line: str | None) -> str | None:
    if not command_line:
        return None
    if command_line.startswith('"'):
        end_quote = command_line.find('"', 1)
        if end_quote > 1:
            return command_line[1:end_quote]
    return command_line.split()[0] if command_line.split() else None


def _passthrough_args(args: list[str]) -> list[str]:
    if args and args[0] == "--":
        return args[1:]
    return args


def _quote_command(command: list[str]) -> str:
    return " ".join(f'"{part}"' if " " in part else part for part in command)


def _install_summary(record: dict[str, object]) -> str:
    lines = [
        f"{record['version']} {'nightly/dev' if record['nightly'] else 'stable'}",
        f"  root: {record['install_root']}",
    ]
    reported = record.get("reported_version")
    if reported:
        lines.append(f"  reported: {reported}")
    executables = record.get("executables", {})
    if isinstance(executables, dict):
        for app in sorted(executables):
            lines.append(f"  {app}: {executables[app]}")
    return "\n".join(lines)


def _preference_summary(record: dict[str, object]) -> str:
    return "\n".join(
        [
            f"{record['version']} {'nightly/dev' if record['nightly'] else 'stable'}",
            f"  config: {record['config_path']} exists={record['config_exists']}",
            f"  documents: {record['documents_root']} exists={record['documents_exists']}",
            f"  plugins: {record['plugins_dir']} exists={record['plugins_exists']}",
            f"  api_enabled: {record['api_enabled']}",
            f"  python: {record['python_interpreter']} exists={record['python_exists']}",
        ]
    )


def _path_arg(value: str) -> Path:
    return Path(value).expanduser()


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the kicad helper command parser."""
    parser = subparsers.add_parser(
        "kicad",
        help="Inspect and manage local KiCad installs and processes",
    )
    kicad_subparsers = parser.add_subparsers(dest="kicad_action", metavar="<kicad-action>")

    installs_parser = kicad_subparsers.add_parser("installs", help="List KiCad installs")
    installs_parser.add_argument("--version")
    installs_parser.add_argument("--json", action="store_true")
    installs_parser.set_defaults(handler=cmd_kicad)

    running_parser = kicad_subparsers.add_parser("running", help="List running KiCad processes")
    running_parser.add_argument("--version")
    running_parser.add_argument("--app", choices=tuple(APP_EXECUTABLES))
    running_parser.add_argument("--json", action="store_true")
    running_parser.set_defaults(handler=cmd_kicad)

    launch_parser = kicad_subparsers.add_parser("launch", help="Launch a KiCad executable")
    launch_parser.add_argument("--version")
    launch_parser.add_argument("--app", choices=tuple(APP_EXECUTABLES), default="kicad")
    launch_parser.add_argument("--exe", type=_path_arg)
    launch_parser.add_argument("--project", type=_path_arg)
    launch_parser.add_argument(
        "--new",
        action="store_true",
        help="Pass KiCad --new so the project manager does not reload the previous project",
    )
    launch_parser.add_argument("--dry-run", action="store_true")
    launch_parser.add_argument("--json", action="store_true")
    launch_parser.add_argument("args", nargs=argparse.REMAINDER)
    launch_parser.set_defaults(handler=cmd_kicad)

    stop_parser = kicad_subparsers.add_parser("stop", help="Stop running KiCad processes")
    stop_parser.add_argument("--version")
    stop_parser.add_argument("--app", choices=tuple(APP_EXECUTABLES))
    stop_parser.add_argument(
        "--all",
        action="store_true",
        help="Terminate all matching KiCad processes instead of printing a dry run",
    )
    stop_parser.add_argument("--json", action="store_true")
    stop_parser.set_defaults(handler=cmd_kicad)

    prefs_parser = kicad_subparsers.add_parser("prefs", help="List KiCad preference paths")
    prefs_parser.add_argument("--version")
    prefs_parser.add_argument(
        "--open",
        dest="open_target",
        choices=("config", "documents", "plugins"),
    )
    prefs_parser.add_argument("--json", action="store_true")
    prefs_parser.set_defaults(handler=cmd_kicad)

    parser.set_defaults(handler=cmd_kicad)
    return parser
