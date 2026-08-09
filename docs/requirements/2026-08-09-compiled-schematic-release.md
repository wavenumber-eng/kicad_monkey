# 2026-08-09 Compiled Schematic Release Acceptance

Status: accepted

This document is the durable closeout record for KiCad Monkey issues #36, #37,
#38, #41, and #42. The implementation ships in `kicad-monkey` 2026.8.9.

## Issue acceptance

| Issue | Accepted behavior | Owning evidence |
| --- | --- | --- |
| #36 | DNP is inherited through every ancestor sheet and agrees across netlist, Design JSON, and variant assembly. | `test_L0_025_netlist_multi_sheet.py`, `test_L0_026_netlist_components_libparts.py`, `test_L1_016_bom_filter_parity.py` |
| #37 | BOM, board, and simulation exclusion fold through the full occurrence path; excluded descendants remain represented with effective policy while the KiCad S-expression export omits off-board components and nodes like `kicad-cli`. | `test_L0_025_netlist_multi_sheet.py`, `test_L0_026_netlist_components_libparts.py`, `test_L0_027_netlist_kicad_sexpr_emit.py`, `test_L1_016_bom_filter_parity.py` |
| #38 | DNP hierarchical sheets are dimmed, crossed with KiCad-compatible marker geometry, and expose `extras.dnp`. | `test_L0_008_schematic_to_ir.py` |
| #41 | Speedy directive marker geometry and both schematic rule areas reach Plotter IR with source identity and generic operations. | `test_L0_008_schematic_to_ir.py`, `test_L3_019_schematic_new_graphics.py` |
| #42 | Real single-page, repeated-hierarchy, multipart, hierarchy-binding, global-label, bus, bus-entry, and occurrence-scoped drawing cases compile into the accepted a0 graph without netlist or Plotter IR regressions. | `test_L0_029_design_netlist_api.py`, `test_L3_018_compiled_schematic_graph.py` |

## Identity and validator gates

The acceptance suite additionally proves:

- reordering collections, renaming hierarchy endpoints, and changing an
  attached wire UUID do not replace semantic graph identities;
- nested reuse is unambiguous, and adding a reused sibling does not change the
  surviving occurrence ID;
- global labels become page-port terminals and every Jumperless bus/bus entry
  has scoped drawing evidence;
- missing hierarchy matches carry diagnostics and fail closed if those
  diagnostics are removed;
- wrong-type refs, wrong-owner refs, inverse-membership gaps, and hierarchy
  cycles are rejected at the producer boundary;
- KiCad and the generic allocator match shared identity vectors while the
  released Altium projector snapshot remains unchanged.

## Release boundary

The KiCad package release closes the five KiCad-owned issues above. Consumer
packages remain responsible for pinning the release, deleting transitional
graph synthesis, and passing their own importer, viewer, and portability gates.
Other source-format graph producers remain separate work.
