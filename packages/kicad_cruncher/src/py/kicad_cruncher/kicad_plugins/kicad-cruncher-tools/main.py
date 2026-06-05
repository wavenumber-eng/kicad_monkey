from __future__ import annotations

import os
import sys
import urllib.error
import urllib.request
import webbrowser

DEFAULT_DAEMON_URL = "http://127.0.0.1:8765"


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

    webbrowser.open(daemon_url)
    print(f"Opened KiCad Cruncher tools: {daemon_url}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
