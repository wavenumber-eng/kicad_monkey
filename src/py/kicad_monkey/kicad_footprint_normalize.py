"""Round-trip-safe normalization for KiCad footprint source forms."""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any

from .kicad_base import find_all_elements


KICAD_MINIMUM_PAD_SIZE_MM = 0.001


@dataclass(frozen=True, slots=True)
class PadSizeNormalizationChange:
    """One direct pad-size repair made by KiCad-compatible normalization."""

    pad_name: str
    original_size: tuple[float, float]
    replacement_size: tuple[float, float] = (
        KICAD_MINIMUM_PAD_SIZE_MM,
        KICAD_MINIMUM_PAD_SIZE_MM,
    )


@dataclass(frozen=True, slots=True)
class FootprintPadSizeNormalization:
    """The mutated expression and structured direct-pad repair diagnostics."""

    expression: Any
    changes: tuple[PadSizeNormalizationChange, ...]

    @property
    def count(self) -> int:
        """Return the number of direct pad sizes that were repaired."""

        return len(self.changes)


def normalize_unsafe_footprint_pad_sizes(
    expression: Any,
) -> FootprintPadSizeNormalization:
    """Pin unsafe direct pad sizes to 1 um, matching KiCad's native parser.

    KiCad treats a pad as algorithmically unsafe when either axis in its
    direct/default ``(size x y)`` form is nonpositive.  It repairs that pad by
    setting both axes to 0.001 mm.  Nested per-layer padstack size forms are
    intentionally left untouched.

    The supplied expression is mutated in place, consistent with the existing
    KiCad Monkey filter operations, and is also returned in the result.
    """

    changes: list[PadSizeNormalizationChange] = []

    for pad in find_all_elements(expression, "pad"):
        pad_name = str(pad[1]) if len(pad) > 1 else "unknown"

        for index, item in enumerate(pad):
            if not isinstance(item, list) or not item or item[0] != "size":
                continue

            if len(item) < 3:
                raise ValueError(f"Pad {pad_name!r} has a malformed direct size form")

            try:
                size_x = float(item[1])
                size_y = float(item[2])
            except (TypeError, ValueError) as exc:
                raise ValueError(
                    f"Pad {pad_name!r} has a nonnumeric direct size form"
                ) from exc

            if not math.isfinite(size_x) or not math.isfinite(size_y):
                raise ValueError(f"Pad {pad_name!r} has a nonfinite direct size form")

            if size_x <= 0.0 or size_y <= 0.0:
                changes.append(
                    PadSizeNormalizationChange(
                        pad_name=pad_name,
                        original_size=(size_x, size_y),
                    )
                )
                pad[index] = [
                    item[0],
                    KICAD_MINIMUM_PAD_SIZE_MM,
                    KICAD_MINIMUM_PAD_SIZE_MM,
                    *item[3:],
                ]

            # Only the direct/default size child belongs to this operation.
            break

    return FootprintPadSizeNormalization(expression, tuple(changes))


__all__ = [
    "FootprintPadSizeNormalization",
    "KICAD_MINIMUM_PAD_SIZE_MM",
    "PadSizeNormalizationChange",
    "normalize_unsafe_footprint_pad_sizes",
]
