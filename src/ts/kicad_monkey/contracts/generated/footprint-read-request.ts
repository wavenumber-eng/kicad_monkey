/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Controls a typed footprint summary read; source bytes are supplied out of band.
 */
export interface FootprintReadRequestA0 {
  type: "kicad_monkey.footprint_read.request";
  version: "a0";
  max_source_bytes: string;
  max_depth: number;
  max_properties: number;
  max_pads: number;
}
