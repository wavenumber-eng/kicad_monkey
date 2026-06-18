"""CLI logging helpers for kicad_cruncher."""

from __future__ import annotations

import logging
import os
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


def _stage_color_enabled(stream: object | None = None) -> bool:
    """Return whether stage/progress messages should include terminal color."""
    if os.environ.get("NO_COLOR") is not None or os.environ.get("TERM") == "dumb":
        return False
    output_stream = sys.stdout if stream is None else stream
    isatty = getattr(output_stream, "isatty", None)
    return bool(callable(isatty) and isatty())


def _color_stage_text(text: str, color: str) -> str:
    if not _stage_color_enabled():
        return text
    return f"{color}{text}{Style.RESET_ALL}"


def stage_start_text(text: str) -> str:
    """Return a high-visibility stage-start message for CLI logs."""
    return _color_stage_text(text, f"{Style.BRIGHT}{Fore.CYAN}")


def stage_progress_text(text: str) -> str:
    """Return a lower-emphasis progress message for CLI logs."""
    return _color_stage_text(text, Fore.CYAN)


def stage_done_text(text: str) -> str:
    """Return a completion message for CLI logs."""
    return _color_stage_text(text, Fore.GREEN)
