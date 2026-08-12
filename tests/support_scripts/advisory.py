"""Shared opt-in policy for advisory performance tests."""

from __future__ import annotations

import os


def advisory_benchmarks_enabled() -> bool:
    """Return true for the strict Rack lane or an explicit environment opt-in."""
    lane = os.environ.get("RACK_LANE", os.environ.get("WN_RACK_LANE", "fast"))
    explicit = os.environ.get("KICAD_MONKEY_RUN_ADVISORY_BENCHMARKS", "")
    return lane.lower() == "strict" or explicit.lower() in {"1", "true", "yes", "on"}
