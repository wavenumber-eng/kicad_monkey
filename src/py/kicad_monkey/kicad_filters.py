"""
KiCad Filters - Re-export filter pipeline and transforms from kicad_filter_core.

This module provides convenient access to:
  - KiCadFilterPipeline: File-level footprint, symbol, schematic, and PCB filters
  - Individual s-expression filter functions

Individual filter implementations are in:
  - kicad_filter_footprint.py
  - kicad_filter_symbol.py
  - kicad_filter_schematic.py
  - kicad_filter_pcb.py
"""

from .kicad_filter_core import (
    KiCadFilterPipeline,
    format_kicad_sexp,
    fp_filter__clean_fab,
    fp_filter__fix_fp_text_font_to_arial,
    fp_filter__fix_zero_sized_pads,
    fp_filter__normalized_embedded_model_naming,
    fp_filter__orthographic_projection_outline,
    fp_filter__remove_schematic_inherited_metadata,
    pcb_filter__process_embedded_footprints,
    pcb_filter__reset_layer_user_names,
    sch_filter__remove_altium_value_property,
    sym_filter__clear_property_values,
    sym_filter__remove_nonstandard_properties,
    sym_filter__standardize_reference_value_fonts,
)

__all__ = [
    "KiCadFilterPipeline",
    "format_kicad_sexp",
    "fp_filter__clean_fab",
    "fp_filter__fix_fp_text_font_to_arial",
    "fp_filter__fix_zero_sized_pads",
    "fp_filter__normalized_embedded_model_naming",
    "fp_filter__orthographic_projection_outline",
    "fp_filter__remove_schematic_inherited_metadata",
    "pcb_filter__process_embedded_footprints",
    "pcb_filter__reset_layer_user_names",
    "sch_filter__remove_altium_value_property",
    "sym_filter__clear_property_values",
    "sym_filter__remove_nonstandard_properties",
    "sym_filter__standardize_reference_value_fonts",
]
