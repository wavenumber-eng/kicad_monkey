"""CLI logging helpers for kicad_cruncher."""

from __future__ import annotations

import logging
import sys

from colorama import Fore, Style
from colorama import init as colorama_init

colorama_init()


class ColoredFormatter(logging.Formatter):
    """Logging formatter that colors warnings and errors on terminals."""

    COLORS = {
        logging.DEBUG: Fore.CYAN,
        logging.INFO: "",
        logging.WARNING: Fore.YELLOW,
        logging.ERROR: Fore.RED,
        logging.CRITICAL: Fore.RED + Style.BRIGHT,
    }
    RESET = Style.RESET_ALL

    def format(self, record: logging.LogRecord) -> str:
        """Format one log record."""
        color = self.COLORS.get(record.levelno, "")
        message = super().format(record)
        return f"{color}{message}{self.RESET}" if color else message


def _configure_stream_encoding_errors(stream: object) -> None:
    reconfigure = getattr(stream, "reconfigure", None)
    if not callable(reconfigure):
        return
    try:
        reconfigure(errors="backslashreplace")
    except Exception:
        return


def setup_cli_logging(level: int = logging.INFO, *, force_flush: bool = False) -> None:
    """Configure root logging for CLI commands."""
    _configure_stream_encoding_errors(sys.stdout)
    _configure_stream_encoding_errors(sys.__stdout__)

    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(ColoredFormatter("%(message)s"))
    handler.setLevel(level)

    if force_flush:
        original_emit = handler.emit

        def flushing_emit(record: logging.LogRecord) -> None:
            original_emit(record)
            sys.stdout.flush()

        handler.emit = flushing_emit

    root = logging.getLogger()
    root.setLevel(level)
    root.handlers.clear()
    root.addHandler(handler)

