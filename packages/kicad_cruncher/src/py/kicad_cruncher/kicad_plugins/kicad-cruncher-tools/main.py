from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
import webbrowser
from pathlib import Path
from urllib.parse import urlparse

DAEMON_STATE_ENV = "KICAD_CRUNCHER_DAEMON_STATE"
DAEMON_COMMAND_ENV = "KICAD_CRUNCHER_DAEMON_COMMAND"
DAEMON_URL_ENV = "KICAD_CRUNCHER_DAEMON_URL"
DEFAULT_DAEMON_URL = "http://127.0.0.1:8765"
DAEMON_START_TIMEOUT_SECONDS = 10.0


def main() -> int:
    daemon_url = _resolve_daemon_url()
    if not _ensure_daemon_running(daemon_url):
        return 1

    webbrowser.open(daemon_url)
    print(f"Opened KiCad Cruncher tools: {daemon_url}")
    return 0


def _ensure_daemon_running(daemon_url: str) -> bool:
    if _daemon_is_healthy(daemon_url):
        return True

    started, message = _start_daemon_for_url(daemon_url)
    if not started:
        print("KiCad Cruncher daemon is not reachable and could not be started.", file=sys.stderr)
        print(f"Daemon URL: {daemon_url}", file=sys.stderr)
        print(message, file=sys.stderr)
        return False

    deadline = time.monotonic() + DAEMON_START_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if _daemon_is_healthy(daemon_url, timeout=1):
            return True
        time.sleep(0.25)

    print("KiCad Cruncher daemon was started but did not become healthy.", file=sys.stderr)
    print(f"Daemon URL: {daemon_url}", file=sys.stderr)
    return False


def _daemon_is_healthy(daemon_url: str, *, timeout: float = 2.0) -> bool:
    health_url = f"{daemon_url.rstrip('/')}/health"
    try:
        with urllib.request.urlopen(health_url, timeout=timeout) as response:
            return response.status == 200
    except (OSError, urllib.error.URLError):
        return False


def _start_daemon_for_url(daemon_url: str) -> tuple[bool, str]:
    parsed = urlparse(daemon_url)
    host = parsed.hostname or ""
    if not _is_loopback_http_url(daemon_url):
        return False, "Refusing to auto-start a daemon for a non-loopback URL."

    port = parsed.port or (80 if parsed.scheme == "http" else 8765)
    errors: list[str] = []
    for command in _daemon_command_candidates():
        full_command = [*command, "--host", host, "--port", str(port)]
        try:
            _start_detached_process(full_command)
            return True, f"Started daemon with: {' '.join(full_command)}"
        except OSError as exc:
            errors.append(f"{' '.join(full_command)}: {exc}")
    return False, "No daemon command could be started. " + " | ".join(errors)


def _daemon_command_candidates() -> list[list[str]]:
    explicit = os.environ.get(DAEMON_COMMAND_ENV, "").strip()
    if explicit:
        return [_split_command(explicit)]

    candidates: list[list[str]] = []
    for name in ("kicad-cruncher", "kcr"):
        resolved = shutil.which(name)
        candidates.append([resolved or name, "daemon"])
    for path in _common_user_tool_paths():
        if path.exists():
            candidates.append([str(path), "daemon"])
    return _unique_commands(candidates)


def _split_command(command_text: str) -> list[str]:
    return shlex.split(command_text, posix=os.name != "nt")


def _common_user_tool_paths() -> tuple[Path, ...]:
    suffix = ".exe" if os.name == "nt" else ""
    if os.name == "nt":
        roots = [Path.home() / ".local" / "bin"]
    else:
        roots = [Path.home() / ".local" / "bin"]
    names = (f"kicad-cruncher{suffix}", f"kcr{suffix}")
    return tuple(root / name for root in roots for name in names)


def _unique_commands(commands: list[list[str]]) -> list[list[str]]:
    unique: list[list[str]] = []
    seen: set[tuple[str, ...]] = set()
    for command in commands:
        key = tuple(command)
        if key in seen:
            continue
        seen.add(key)
        unique.append(command)
    return unique


def _start_detached_process(command: list[str]) -> None:
    creationflags = 0
    startupinfo = None
    if os.name == "nt":
        creationflags = (
            getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
            | getattr(subprocess, "DETACHED_PROCESS", 0)
            | getattr(subprocess, "CREATE_NO_WINDOW", 0)
        )
        startupinfo = subprocess.STARTUPINFO()
        startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        close_fds=True,
        creationflags=creationflags,
        startupinfo=startupinfo,
    )


def _load_json_object(path: Path) -> dict[str, object] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def _resolve_daemon_url() -> str:
    env_url = os.environ.get(DAEMON_URL_ENV, "").strip()
    if env_url:
        return env_url.rstrip("/")
    state_url = _read_daemon_state_url()
    return (state_url or DEFAULT_DAEMON_URL).rstrip("/")


def _read_daemon_state_url() -> str | None:
    payload = _load_json_object(_daemon_state_path())
    if payload is None:
        return None
    url = str(payload.get("url") or "").strip()
    return url if _is_loopback_http_url(url) else None


def _daemon_state_path() -> Path:
    override = os.environ.get(DAEMON_STATE_ENV)
    if override:
        return Path(override).expanduser()

    if os.name == "nt":
        root = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local")
    elif sys.platform == "darwin":
        root = Path.home() / "Library" / "Application Support"
    else:
        root = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local" / "state")
    return root / "kicad-cruncher" / "daemon.json"


def _is_loopback_http_url(url: str) -> bool:
    parsed = urlparse(url)
    return parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost", "::1"}


if __name__ == "__main__":
    raise SystemExit(main())
