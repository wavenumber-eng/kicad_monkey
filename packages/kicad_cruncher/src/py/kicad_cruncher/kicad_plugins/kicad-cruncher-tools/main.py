from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
import webbrowser
from importlib import import_module

DEFAULT_DAEMON_URL = "http://127.0.0.1:8765"
LAYER_CLEANUP_REQUEST_SCHEMA = "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0"


def main() -> int:
    daemon_url = os.environ.get("KICAD_CRUNCHER_DAEMON_URL", DEFAULT_DAEMON_URL).rstrip("/")
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

    session = _discover_kicad_session()
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


def _discover_kicad_session() -> dict[str, object]:
    try:
        kipy = import_module("kipy")
        board = kipy.KiCad().get_board()
    except Exception as exc:
        return {
            "connected": False,
            "source": "kicad-ipc-plugin",
            "reason": str(exc),
        }

    board_path = _first_text_attribute(
        board,
        ("file_name", "filename", "path", "board_path", "project_path"),
    )
    return {
        "connected": True,
        "source": "kicad-ipc-plugin",
        "board_name": _first_text_attribute(board, ("name", "title")),
        "board_path": board_path,
    }


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


if __name__ == "__main__":
    raise SystemExit(main())
