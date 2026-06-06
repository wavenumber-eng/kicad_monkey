"""daemon command for KiCad Cruncher."""

from __future__ import annotations

import argparse
import json

from kicad_cruncher.kicad_cruncher_daemon import (
    DEFAULT_DAEMON_HOST,
    DEFAULT_DAEMON_PORT,
    daemon_health_payload,
)
from kicad_cruncher.kicad_cruncher_daemon_state import write_daemon_state


def cmd_daemon(args: argparse.Namespace) -> int:
    """Run or inspect the local KiCad Cruncher daemon."""
    if bool(getattr(args, "health", False)):
        print(json.dumps(daemon_health_payload(), indent=2))
        return 0

    import uvicorn

    host = str(getattr(args, "host", DEFAULT_DAEMON_HOST))
    port = int(getattr(args, "port", DEFAULT_DAEMON_PORT))
    write_daemon_state(host=host, port=port)
    uvicorn.run(
        "kicad_cruncher.kicad_cruncher_daemon:create_app",
        factory=True,
        host=host,
        port=port,
        reload=bool(getattr(args, "reload", False)),
    )
    return 0


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the daemon command parser."""
    parser = subparsers.add_parser("daemon", help="Run the local plugin daemon")
    parser.add_argument("--host", default=DEFAULT_DAEMON_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_DAEMON_PORT)
    parser.add_argument("--reload", action="store_true")
    parser.add_argument(
        "--health",
        action="store_true",
        help="Print the daemon health payload and exit without starting a server",
    )
    parser.set_defaults(handler=cmd_daemon)
    return parser
