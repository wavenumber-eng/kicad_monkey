from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
import webbrowser
from importlib import import_module
from pathlib import Path
from typing import Protocol, cast
from urllib.parse import urlparse

DAEMON_STATE_ENV = "KICAD_CRUNCHER_DAEMON_STATE"
DAEMON_URL_ENV = "KICAD_CRUNCHER_DAEMON_URL"
DEFAULT_DAEMON_URL = "http://127.0.0.1:8765"
LAYER_CLEANUP_REQUEST_SCHEMA = "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0"
PCB_CLEAN_APPLY_ENV = "KICAD_CRUNCHER_PCB_CLEAN_APPLY"


class _IpcApplyModule(Protocol):
    def apply_pcb_clean_mutation_request(
        self,
        board: object,
        mutation_request: dict[str, object],
    ) -> dict[str, object]: ...


def main() -> int:
    daemon_url = _resolve_daemon_url()
    health_url = f"{daemon_url}/health"
    try:
        with urllib.request.urlopen(health_url, timeout=2) as response:
            if response.status != 200:
                print(f"KiCad Cruncher daemon health failed: HTTP {response.status}")
                return 1
    except (OSError, urllib.error.URLError) as exc:
        print(
            "KiCad Cruncher daemon is not reachable. "
            "Start it with `kicad-cruncher daemon`.",
            file=sys.stderr,
        )
        print(f"Health URL: {health_url}", file=sys.stderr)
        print(f"Error: {exc}", file=sys.stderr)
        return 1

    board, session = _discover_kicad_session()
    cleanup_payload: dict[str, object] = {
        "schema": LAYER_CLEANUP_REQUEST_SCHEMA,
        "mode": "kicad-ipc",
        "apply": False,
        "session": session,
    }
    board_path = session.get("board_path")
    if board_path:
        cleanup_payload["board_path"] = board_path
    cleanup_url = f"{daemon_url}/api/v1/pcb/layer-cleanup"
    try:
        cleanup_result = _post_json(cleanup_url, cleanup_payload)
    except (OSError, urllib.error.URLError, json.JSONDecodeError) as exc:
        print(
            "KiCad Cruncher daemon PCB clean planning failed.",
            file=sys.stderr,
        )
        print(f"Cleanup URL: {cleanup_url}", file=sys.stderr)
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    if cleanup_result.get("ok") is not True:
        print(
            "KiCad Cruncher daemon did not accept the PCB clean planning request.",
            file=sys.stderr,
        )
        print(json.dumps(cleanup_result, indent=2), file=sys.stderr)
        return 1

    if _env_bool(PCB_CLEAN_APPLY_ENV):
        if board is None:
            print("KiCad Cruncher cannot apply PCB clean: no KiCad board session.", file=sys.stderr)
            return 1
        try:
            result_payload = cleanup_result.get("result")
            if not isinstance(result_payload, dict):
                print("KiCad Cruncher daemon did not return a mutation request.", file=sys.stderr)
                return 1
            apply_result = _apply_pcb_clean_mutation_request(board, result_payload)
        except Exception as exc:
            print(f"KiCad Cruncher PCB clean IPC apply failed: {exc}", file=sys.stderr)
            return 1
        print(json.dumps(apply_result, indent=2))

    webbrowser.open(daemon_url)
    print(f"Opened KiCad Cruncher tools: {daemon_url}")
    print("PCB clean operation plan routed through daemon.")
    return 0


def _post_json(url: str, payload: dict[str, object]) -> dict[str, object]:
    data = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        if response.status != 200:
            return {"ok": False, "status": response.status}
        body = response.read().decode("utf-8")
    decoded = json.loads(body)
    return decoded if isinstance(decoded, dict) else {"ok": False, "body": decoded}


def _resolve_daemon_url() -> str:
    env_url = os.environ.get(DAEMON_URL_ENV, "").strip()
    if env_url:
        return env_url.rstrip("/")
    state_url = _read_daemon_state_url()
    return (state_url or DEFAULT_DAEMON_URL).rstrip("/")


def _read_daemon_state_url() -> str | None:
    path = _daemon_state_path()
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    if not isinstance(payload, dict):
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


def _apply_pcb_clean_mutation_request(
    board: object,
    mutation_request: dict[str, object],
) -> dict[str, object]:
    plugin_dir = os.path.dirname(__file__)
    if plugin_dir not in sys.path:
        sys.path.insert(0, plugin_dir)
    ipc_apply = cast(_IpcApplyModule, import_module("ipc_apply"))
    result = ipc_apply.apply_pcb_clean_mutation_request(board, mutation_request)
    if not isinstance(result, dict):
        raise RuntimeError("KiCad Cruncher PCB clean IPC apply returned a non-object result.")
    return result


def _discover_kicad_session() -> tuple[object | None, dict[str, object]]:
    try:
        kipy = import_module("kipy")
        board = kipy.KiCad().get_board()
    except Exception as exc:
        return (
            None,
            {
                "connected": False,
                "source": "kicad-ipc-plugin",
                "reason": str(exc),
            },
        )

    board_path = _first_text_attribute(
        board,
        ("file_name", "filename", "path", "board_path", "project_path"),
    )
    return (
        board,
        {
            "connected": True,
            "source": "kicad-ipc-plugin",
            "board_name": _first_text_attribute(board, ("name", "title")),
            "board_path": board_path,
        },
    )


def _first_text_attribute(target: object, names: tuple[str, ...]) -> str | None:
    for name in names:
        value = getattr(target, name, None)
        if callable(value):
            try:
                value = value()
            except Exception:
                continue
        text = str(value or "").strip()
        if text:
            return text
    return None


def _env_bool(name: str) -> bool:
    value = os.environ.get(name, "")
    return value.strip().lower() in {"1", "true", "yes", "on"}


if __name__ == "__main__":
    raise SystemExit(main())
