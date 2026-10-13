/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Selectable products. Every operation belongs to exactly one product. The
 * board profile has no product of its own: it is the add operation of every
 * dielectric and solder-mask slice, with cutouts subtracted there.
 */
export type MaterialProduct = "copper" | "holes" | "dielectric" | "solder_mask" | "paste" | "silkscreen" | "components";
/**
 * Board-frame length in integer nanometres. The range matches KiCad's own
 * 32-bit internal-unit limit, so the packed encoding stores it losslessly.
 */
export type Nm = number;
/**
 * Non-negative length in integer nanometres.
 */
export type NonNegativeNm = number;

/**
 * Resource-bounded materialization request. Source bytes are out of band.
 */
export interface MaterializationRequestA0 {
  type: "kicad_monkey.materialization.request";
  version: "a0";
  products: MaterialProduct[];
  slice_keys?: string[];
  net_keys?: string[];
  region?: NmBounds;
  tile_size_nm: NonNegativeNm;
  /**
   * Bounds the board and project files together.
   */
  max_source_bytes: string;
  max_operations: number;
  max_points: number;
}
/**
 * Axis-aligned bounds in the board frame, inclusive.
 */
export interface NmBounds {
  min_x_nm: Nm;
  min_y_nm: Nm;
  max_x_nm: Nm;
  max_y_nm: Nm;
}
