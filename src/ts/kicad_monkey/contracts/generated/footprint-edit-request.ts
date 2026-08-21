/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Focused source-preserving property edit; source bytes are supplied out of band.
 */
export interface FootprintEditRequestA0 {
  type: "kicad_monkey.footprint_edit.request";
  version: "a0";
  property_name: string;
  value: string;
  max_source_bytes: string;
  max_output_bytes: string;
  max_depth: number;
  max_properties: number;
  max_pads: number;
}
