"""Standard KiCad schematic page sizes and their nominal dimensions.

The single source of truth for the page-size *choice list* and the dimension
readout that front-ends show (e.g. ``kcr project create``).  Dimensions here are
nominal landscape millimetres for display; the precise, plotter-rounded values
used when rendering live in :mod:`kicad_schematic_to_ir`.
"""

from __future__ import annotations

from ._api_markers import public_api

# Ordered as KiCad's eeschema "Page Settings" dialog presents them.
KICAD_PAGE_SIZES: tuple[str, ...] = (
    "A4", "A3", "A2", "A1", "A0",
    "A", "B", "C", "D", "E",
    "USLetter", "USLegal", "USLedger",
)

# size -> (width_mm, height_mm), nominal/rounded for display.
KICAD_PAGE_DIMENSIONS_MM: dict[str, tuple[int, int]] = {
    "A4": (210, 297),
    "A3": (297, 420),
    "A2": (420, 594),
    "A1": (594, 841),
    "A0": (841, 1189),
    "A": (216, 279),
    "B": (279, 432),
    "C": (432, 559),
    "D": (559, 864),
    "E": (864, 1118),
    "USLetter": (216, 279),
    "USLegal": (216, 356),
    "USLedger": (279, 432),
}


@public_api
def kicad_page_size_label(size: str) -> str:
    """Return a display label such as ``'D  —  559 x 864 mm'``.

    Falls back to the bare size string for non-standard / ``User`` sizes.
    """
    dims = KICAD_PAGE_DIMENSIONS_MM.get(size)
    if dims is None:
        return size
    return f"{size}  —  {dims[0]} x {dims[1]} mm"
