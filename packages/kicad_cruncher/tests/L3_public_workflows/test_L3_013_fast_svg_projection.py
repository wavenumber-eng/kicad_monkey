"""Guard the fast/legacy Geometer boundary, including cache identity."""

from dataclasses import replace

import pytest
from kicad_cruncher.kicad_cruncher_pcb_svg_projection import (
    _AssemblyProjectionCache,
    _AssemblyProjectionOptions,
)


def test_fast_defaults_and_legacy_controls_are_explicit() -> None:
    cache = _AssemblyProjectionCache()
    options = _AssemblyProjectionOptions(side="top")
    forwarded = cache._hlr_options_for_geometer(options, curve_mode="native_arcs", round_digits=3)
    assert forwarded["projection_algorithm"] == "fast"
    assert forwarded["outline_algorithm"] == "fast-mesh-shadow"
    assert forwarded["curve_mode"] == "polyline"
    with pytest.raises(ValueError, match="require projection_algorithm"):
        replace(options, include_visible=False)
    legacy = replace(options, projection_algorithm="exact", include_visible=False)
    forwarded = cache._hlr_options_for_geometer(legacy, curve_mode="native_arcs", round_digits=3)
    assert forwarded["projection_algorithm"] == "exact"
    assert "outline_algorithm" not in forwarded
    with pytest.raises(ValueError, match="requires projection_algorithm fast"):
        replace(legacy, fast={"include_hidden": False})


def test_fast_controls_affect_projection_cache_identity() -> None:
    cache = _AssemblyProjectionCache()
    options = _AssemblyProjectionOptions(side="top", fast={"crease_angle_rad": 0.4})
    def key(value: _AssemblyProjectionOptions) -> tuple[object, ...]:
        return cache.build_cache_key(model_hash="model", pose_signature=(), options=value)
    assert key(options) != key(replace(options, fast={"crease_angle_rad": 0.8}))
