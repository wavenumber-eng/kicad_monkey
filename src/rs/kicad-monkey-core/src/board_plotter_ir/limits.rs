//! Translate plot-request budgets into source-reader ceilings.

use super::{BoardPlotLimits, PcbLimits};

pub(super) fn board_pcb_limits(limits: BoardPlotLimits) -> PcbLimits {
    PcbLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_top_level_forms: limits.max_parse_nodes,
        max_object_children: limits.max_parse_nodes,
        max_nets: limits.max_graphics,
        max_footprints: limits.max_graphics,
        max_footprint_children: limits.max_parse_nodes,
        max_footprint_header_scalars: limits.max_parse_nodes,
        max_footprint_attributes: limits.max_parse_nodes.min(256),
        max_footprint_properties: limits.max_graphics,
        max_footprint_graphics: limits.max_graphics,
        max_footprint_texts: limits.max_graphics,
        max_footprint_text_boxes: limits.max_graphics,
        max_text_effect_children: limits.max_parse_nodes,
        max_text_font_children: limits.max_parse_nodes,
        max_text_justify_tokens: limits.max_parse_nodes,
        max_text_box_points: limits.max_input_points,
        max_pad_header_scalars: limits.max_parse_nodes.min(256),
        max_pad_children: limits.max_parse_nodes,
        max_pad_chamfer_corners: limits.max_parse_nodes,
        max_pad_custom_primitives: limits.max_input_polygons,
        max_pad_custom_point_forms: limits.max_input_points,
        max_pad_custom_points: limits.max_input_points,
        max_pads: limits.max_graphics,
        max_graphics: limits.max_graphics,
        max_graphic_points: limits.max_input_points,
        // The request-level record budget bounds every promoted family.
        max_segments: limits.max_graphics,
        max_vias: limits.max_graphics,
        max_arcs: limits.max_graphics,
        max_zones: limits.max_graphics,
        max_dimensions: limits.max_graphics,
        max_tables: limits.max_graphics,
        max_images: limits.max_graphics,
        max_image_data_parts: limits.max_parse_nodes,
        max_table_cells: limits.max_parse_nodes,
        max_table_values: limits.max_parse_nodes,
        max_zone_polygons: limits.max_input_polygons,
        max_zone_points: limits.max_input_points,
        ..PcbLimits::default()
    }
}
