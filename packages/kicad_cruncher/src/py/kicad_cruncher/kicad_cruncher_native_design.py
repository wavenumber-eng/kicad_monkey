"""No-fallback native provider for Cruncher's schematic design facts."""

from __future__ import annotations

import os
import platform
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from kicad_monkey import KiCadDesign, KiCadNativeDesignFacts


class DesignFactsProvider(Protocol):
    """Cruncher-owned boundary for one current-state design-facts snapshot."""

    def design_facts(
        self,
        design: KiCadDesign,
        *,
        source_path: str,
        date: str,
        tool: str,
    ) -> KiCadNativeDesignFacts: ...


@dataclass(frozen=True, slots=True)
class NativeDesignFactsProvider:
    """Request validated facts from Monkey's package-owned native sidecar.

    Native failures propagate unchanged.  This provider never retries through
    Python graph compilation or Python's version-E netlist writer.
    """

    executable: Path | str | None = None
    timeout: float = 120.0

    def design_facts(
        self,
        design: KiCadDesign,
        *,
        source_path: str,
        date: str,
        tool: str,
    ) -> KiCadNativeDesignFacts:
        from kicad_monkey import native_design_facts_for_design

        return native_design_facts_for_design(
            design,
            source_path=source_path,
            date=date,
            tool=tool,
            executable=self.executable,
            timeout=self.timeout,
        )


def use_native_design_facts_provider() -> bool:
    """Return whether this platform is promoted to native design facts."""

    explicitly_enabled = os.environ.get("KICAD_CRUNCHER_NATIVE_DESIGN_FACTS") == "1"
    windows_x64 = sys.platform == "win32" and platform.machine().casefold() in {
        "amd64",
        "x86_64",
    }
    return windows_x64 or explicitly_enabled


def selected_design_facts_provider() -> DesignFactsProvider | None:
    """Resolve the provider once, before design-review artifact production."""

    if use_native_design_facts_provider():
        return NativeDesignFactsProvider()
    return None


__all__ = [
    "DesignFactsProvider",
    "NativeDesignFactsProvider",
    "selected_design_facts_provider",
    "use_native_design_facts_provider",
]
