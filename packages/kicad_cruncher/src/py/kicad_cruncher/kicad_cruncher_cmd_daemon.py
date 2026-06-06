"""daemon command for KiCad Cruncher."""

from __future__ import annotations

import argparse
import ipaddress
import json
import sys

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

    host = str(getattr(args, "host", DEFAULT_DAEMON_HOST))
    port = int(getattr(args, "port", DEFAULT_DAEMON_PORT))
    if not daemon_host_allowed(host, allow_remote=bool(getattr(args, "allow_remote_host", False))):
        print(
            "Refusing remote daemon host. Use --allow-remote-host for an explicit remote bind.",
            file=sys.stderr,
        )
        return 2

    import uvicorn

    write_daemon_state(host=host, port=port)
    uvicorn.run(
        "kicad_cruncher.kicad_cruncher_daemon:create_app",
        factory=True,
        host=host,
        port=port,
        reload=bool(getattr(args, "reload", False)),
    )
    return 0


def daemon_host_allowed(host: str, *, allow_remote: bool) -> bool:
    """Return whether a requested daemon host is allowed."""
    return allow_remote or daemon_host_is_loopback(host)


def daemon_host_is_loopback(host: str) -> bool:
    """Return whether a daemon host is a local loopback address."""
    normalized = host.strip().lower()
    if normalized == "localhost":
        return True
    try:
        return ipaddress.ip_address(normalized).is_loopback
    except ValueError:
        return False


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the daemon command parser."""
    parser = subparsers.add_parser("daemon", help="Run the local plugin daemon")
    parser.add_argument("--host", default=DEFAULT_DAEMON_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_DAEMON_PORT)
    parser.add_argument("--reload", action="store_true")
    parser.add_argument(
        "--allow-remote-host",
        action="store_true",
        help="Allow binding the daemon to a non-loopback host",
    )
    parser.add_argument(
        "--health",
        action="store_true",
        help="Print the daemon health payload and exit without starting a server",
    )
    parser.set_defaults(handler=cmd_daemon)
    return parser
