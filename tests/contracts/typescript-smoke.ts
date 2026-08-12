import type {
  FootprintEditRequestA0,
  FootprintReadRequestA0,
  SExpressionBuildRequestA0,
  SExpressionScanRequestA0,
} from "../../src/ts/kicad_monkey/contracts/generated/index.js";

const buildRequest = {
  type: "kicad_monkey.sexpr_build.request",
  version: "a0",
  root: {
    kind: "list",
    children: [{ kind: "atom", text: "footprint" }],
  },
  max_output_bytes: "4096",
  max_depth: 16,
  max_nodes: 64,
} satisfies SExpressionBuildRequestA0;

const scanRequest = {
  type: "kicad_monkey.sexpr_scan.request",
  version: "a0",
  selector: { heads: ["footprint"] },
  max_source_bytes: "4096",
  max_depth: 16,
  max_selected_forms: 8,
} satisfies SExpressionScanRequestA0;

void buildRequest;
void scanRequest;

const footprintRead = {
  type: "kicad_monkey.footprint_read.request",
  version: "a0",
  max_source_bytes: "1048576",
  max_depth: 64,
  max_properties: 128,
  max_pads: 4096,
} satisfies FootprintReadRequestA0;

const footprintEdit = {
  ...footprintRead,
  type: "kicad_monkey.footprint_edit.request",
  property_name: "Value",
  value: "R_0603",
  max_output_bytes: "1048576",
} satisfies FootprintEditRequestA0;

void footprintRead;
void footprintEdit;
