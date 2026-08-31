"""
Subtest: OOP Model Equivalency
Stratum: L1_parsing
Purpose: OOP model data matches raw sexp after parsing

Tests verify that:
1. Load file to object (board1)
2. Serialize to string
3. Parse string into new object (board2)
4. board1 and board2 have equivalent OOP state

This is different from sexp-level round-trip tests - these compare
the actual Python object fields after round-trip.
"""

from pathlib import Path
from typing import List

import pytest

from kicad_monkey.kicad_pcb import KiCadPcb, from_kicad_pcb
from kicad_monkey.kicad_pcb_gr_text import GrText
from kicad_monkey.kicad_pcb_gr_line import GrLine
from kicad_monkey.kicad_pcb_gr_rect import GrRect
from kicad_monkey.kicad_pcb_gr_arc import GrArc
from kicad_monkey.kicad_pcb_gr_circle import GrCircle
from kicad_monkey.kicad_pcb_gr_poly import GrPoly
from kicad_monkey.kicad_pcb_footprint import Footprint, Pad, Property, FpText, FpLine, FpPoly, Model, EmbeddedFile
from kicad_monkey.kicad_pcb_zone import Zone, Keepout
from kicad_monkey.kicad_pcb_routing import Segment, Via, Arc
from kicad_monkey.kicad_pcb_other import (
    Barcode,
    BarcodeMargins,
    BoardVariant,
    ComponentClassRef,
    Dimension,
    DimensionFormat,
    DimensionStyle,
    DrillProps,
    FootprintPlacement,
    FootprintVariant,
    FootprintVariantField,
    GeneratedObject,
    GeneratedProperty,
    Group,
    Image,
    Layer,
    Net,
    NetRef,
    PadNameGroup,
    PostMachiningProps,
    Table,
    ZoneLayerConnections,
)
from kicad_monkey.kicad_primitives import Stroke, Font, Effects, RenderCache

from conftest import get_all_pcb_files, get_pcb_test_ids


# ============================================================================
# Helper Functions for Comparison
# ============================================================================

def compare_float(a: float, b: float, tolerance: float = 1e-6) -> bool:
    """Compare two floats with tolerance."""
    return abs(a - b) < tolerance


def compare_lists(list1: List, list2: List, compare_func) -> List[str]:
    """Compare two lists element by element."""
    diffs = []
    if len(list1) != len(list2):
        diffs.append(f"List length mismatch: {len(list1)} vs {len(list2)}")
        return diffs

    for i, (a, b) in enumerate(zip(list1, list2)):
        elem_diffs = compare_func(a, b)
        for diff in elem_diffs:
            diffs.append(f"[{i}].{diff}")
    return diffs


# ============================================================================
# Object Comparison Functions
# ============================================================================

def compare_stroke(s1: Stroke, s2: Stroke) -> List[str]:
    """Compare two Stroke objects."""
    diffs = []
    if not compare_float(s1.width, s2.width):
        diffs.append(f"width: {s1.width} != {s2.width}")
    if s1.type != s2.type:
        diffs.append(f"type: {s1.type} != {s2.type}")
    return diffs


def compare_font(f1: Font, f2: Font) -> List[str]:
    """Compare two Font objects."""
    diffs = []
    if f1.face != f2.face:
        diffs.append(f"face: {f1.face} != {f2.face}")
    if not compare_float(f1.size_x, f2.size_x):
        diffs.append(f"size_x: {f1.size_x} != {f2.size_x}")
    if not compare_float(f1.size_y, f2.size_y):
        diffs.append(f"size_y: {f1.size_y} != {f2.size_y}")
    if not compare_float(f1.thickness, f2.thickness):
        diffs.append(f"thickness: {f1.thickness} != {f2.thickness}")
    if f1.bold != f2.bold:
        diffs.append(f"bold: {f1.bold} != {f2.bold}")
    if f1.italic != f2.italic:
        diffs.append(f"italic: {f1.italic} != {f2.italic}")
    return diffs


def compare_effects(e1: Effects, e2: Effects) -> List[str]:
    """Compare two Effects objects."""
    diffs = []
    diffs.extend([f"font.{d}" for d in compare_font(e1.font, e2.font)])
    if e1.justify != e2.justify:
        diffs.append(f"justify: {e1.justify} != {e2.justify}")
    if e1.hide != e2.hide:
        diffs.append(f"hide: {e1.hide} != {e2.hide}")
    return diffs


def compare_barcode_margins(m1: BarcodeMargins, m2: BarcodeMargins) -> List[str]:
    """Compare two BarcodeMargins objects."""
    diffs = []
    if not compare_float(m1.x, m2.x):
        diffs.append(f"x: {m1.x} != {m2.x}")
    if not compare_float(m1.y, m2.y):
        diffs.append(f"y: {m1.y} != {m2.y}")
    return diffs


def compare_barcode(b1: Barcode, b2: Barcode) -> List[str]:
    """Compare two Barcode objects."""
    diffs = []
    if not compare_float(b1.at_x, b2.at_x):
        diffs.append(f"at_x: {b1.at_x} != {b2.at_x}")
    if not compare_float(b1.at_y, b2.at_y):
        diffs.append(f"at_y: {b1.at_y} != {b2.at_y}")
    if not compare_float(b1.at_angle, b2.at_angle):
        diffs.append(f"at_angle: {b1.at_angle} != {b2.at_angle}")
    if b1.layer != b2.layer:
        diffs.append(f"layer: {b1.layer} != {b2.layer}")
    if not compare_float(b1.width, b2.width):
        diffs.append(f"width: {b1.width} != {b2.width}")
    if not compare_float(b1.height, b2.height):
        diffs.append(f"height: {b1.height} != {b2.height}")
    if b1.text != b2.text:
        diffs.append(f"text: {b1.text!r} != {b2.text!r}")
    if not compare_float(b1.text_height, b2.text_height):
        diffs.append(f"text_height: {b1.text_height} != {b2.text_height}")
    if b1.barcode_type != b2.barcode_type:
        diffs.append(f"barcode_type: {b1.barcode_type} != {b2.barcode_type}")
    if b1.ecc_level != b2.ecc_level:
        diffs.append(f"ecc_level: {b1.ecc_level} != {b2.ecc_level}")
    if b1.locked != b2.locked:
        diffs.append(f"locked: {b1.locked} != {b2.locked}")
    if b1.show_text != b2.show_text:
        diffs.append(f"show_text: {b1.show_text} != {b2.show_text}")
    if b1.knockout != b2.knockout:
        diffs.append(f"knockout: {b1.knockout} != {b2.knockout}")
    diffs.extend([f"margins.{d}" for d in compare_barcode_margins(b1.margins, b2.margins)])
    if b1.uuid != b2.uuid:
        diffs.append(f"uuid: {b1.uuid} != {b2.uuid}")
    return diffs


def compare_image(i1: Image, i2: Image) -> List[str]:
    """Compare two Image objects."""
    diffs = []
    if not compare_float(i1.at_x, i2.at_x):
        diffs.append(f"at_x: {i1.at_x} != {i2.at_x}")
    if not compare_float(i1.at_y, i2.at_y):
        diffs.append(f"at_y: {i1.at_y} != {i2.at_y}")
    if not compare_float(i1.scale, i2.scale):
        diffs.append(f"scale: {i1.scale} != {i2.scale}")
    if i1.layer != i2.layer:
        diffs.append(f"layer: {i1.layer} != {i2.layer}")
    if i1.data != i2.data:
        diffs.append(f"data length: {len(i1.data)} != {len(i2.data)}")
    if i1.uuid != i2.uuid:
        diffs.append(f"uuid: {i1.uuid} != {i2.uuid}")
    return diffs


def compare_table(t1: Table, t2: Table) -> List[str]:
    """Compare two Table objects."""
    diffs = []
    if t1.column_count != t2.column_count:
        diffs.append(f"column_count: {t1.column_count} != {t2.column_count}")
    if t1.layer != t2.layer:
        diffs.append(f"layer: {t1.layer} != {t2.layer}")
    if t1.border_external != t2.border_external:
        diffs.append(f"border_external: {t1.border_external} != {t2.border_external}")
    if t1.border_header != t2.border_header:
        diffs.append(f"border_header: {t1.border_header} != {t2.border_header}")
    if t1.separators_rows != t2.separators_rows:
        diffs.append(f"separators_rows: {t1.separators_rows} != {t2.separators_rows}")
    if t1.separators_cols != t2.separators_cols:
        diffs.append(f"separators_cols: {t1.separators_cols} != {t2.separators_cols}")
    if len(t1.cells) != len(t2.cells):
        diffs.append(f"cells count: {len(t1.cells)} != {len(t2.cells)}")
    if t1.uuid != t2.uuid:
        diffs.append(f"uuid: {t1.uuid} != {t2.uuid}")
    return diffs


def compare_board_variant(v1: BoardVariant, v2: BoardVariant) -> List[str]:
    """Compare two board-level variant objects."""
    diffs = []
    if v1.name != v2.name:
        diffs.append(f"name: {v1.name!r} != {v2.name!r}")
    if v1.description != v2.description:
        diffs.append(f"description: {v1.description!r} != {v2.description!r}")
    return diffs


def compare_footprint_variant_field(
    f1: FootprintVariantField,
    f2: FootprintVariantField,
) -> List[str]:
    """Compare two footprint-variant field overrides."""
    diffs = []
    if f1.name != f2.name:
        diffs.append(f"name: {f1.name!r} != {f2.name!r}")
    if f1.value != f2.value:
        diffs.append(f"value: {f1.value!r} != {f2.value!r}")
    return diffs


def compare_footprint_variant(v1: FootprintVariant, v2: FootprintVariant) -> List[str]:
    """Compare two footprint-variant override objects."""
    diffs = []
    if v1.name != v2.name:
        diffs.append(f"name: {v1.name!r} != {v2.name!r}")
    if v1.dnp != v2.dnp:
        diffs.append(f"dnp: {v1.dnp} != {v2.dnp}")
    if v1.exclude_from_bom != v2.exclude_from_bom:
        diffs.append(f"exclude_from_bom: {v1.exclude_from_bom} != {v2.exclude_from_bom}")
    if v1.exclude_from_pos_files != v2.exclude_from_pos_files:
        diffs.append(
            "exclude_from_pos_files: "
            f"{v1.exclude_from_pos_files} != {v2.exclude_from_pos_files}"
        )
    diffs.extend(
        [
            f"fields.{d}"
            for d in compare_lists(v1.fields, v2.fields, compare_footprint_variant_field)
        ]
    )
    return diffs


def compare_footprint_placement(p1: FootprintPlacement, p2: FootprintPlacement) -> List[str]:
    """Compare two footprint-placement metadata objects."""
    diffs = []
    if p1.path != p2.path:
        diffs.append(f"path: '{p1.path}' != '{p2.path}'")
    if p1.sheetname != p2.sheetname:
        diffs.append(f"sheetname: '{p1.sheetname}' != '{p2.sheetname}'")
    if p1.sheetfile != p2.sheetfile:
        diffs.append(f"sheetfile: '{p1.sheetfile}' != '{p2.sheetfile}'")
    return diffs


def compare_component_class_ref(c1: ComponentClassRef, c2: ComponentClassRef) -> List[str]:
    """Compare two footprint component-class references."""
    if c1.name == c2.name:
        return []
    return [f"name: '{c1.name}' != '{c2.name}'"]


def compare_generated_property(p1: GeneratedProperty, p2: GeneratedProperty) -> List[str]:
    """Compare two generated-item preserved properties."""
    diffs = []
    if p1.name != p2.name:
        diffs.append(f"name: '{p1.name}' != '{p2.name}'")
    if p1.raw_sexp != p2.raw_sexp:
        diffs.append(f"raw_sexp: {p1.raw_sexp!r} != {p2.raw_sexp!r}")
    return diffs


def compare_generated_object(g1: GeneratedObject, g2: GeneratedObject) -> List[str]:
    """Compare two board-level generated objects."""
    diffs = []
    if g1.uuid != g2.uuid:
        diffs.append(f"uuid: {g1.uuid} != {g2.uuid}")
    if g1.generator_type != g2.generator_type:
        diffs.append(f"generator_type: '{g1.generator_type}' != '{g2.generator_type}'")
    if g1.name != g2.name:
        diffs.append(f"name: '{g1.name}' != '{g2.name}'")
    if g1.layer != g2.layer:
        diffs.append(f"layer: '{g1.layer}' != '{g2.layer}'")
    if g1.locked != g2.locked:
        diffs.append(f"locked: {g1.locked} != {g2.locked}")
    if g1.members != g2.members:
        diffs.append(f"members: {g1.members} != {g2.members}")
    diffs.extend(
        [f"properties.{d}" for d in compare_lists(g1.properties, g2.properties, compare_generated_property)]
    )
    return diffs


def compare_layer(l1: Layer, l2: Layer) -> List[str]:
    """Compare two Layer objects."""
    diffs = []
    if l1.ordinal != l2.ordinal:
        diffs.append(f"ordinal: {l1.ordinal} != {l2.ordinal}")
    if l1.canonical_name != l2.canonical_name:
        diffs.append(f"canonical_name: {l1.canonical_name} != {l2.canonical_name}")
    if l1.layer_type != l2.layer_type:
        diffs.append(f"layer_type: {l1.layer_type} != {l2.layer_type}")
    return diffs


def compare_net(n1: Net, n2: Net) -> List[str]:
    """Compare two Net objects."""
    diffs = []
    if n1.ordinal != n2.ordinal:
        diffs.append(f"ordinal: {n1.ordinal} != {n2.ordinal}")
    if n1.name != n2.name:
        diffs.append(f"name: '{n1.name}' != '{n2.name}'")
    return diffs


def compare_net_ref(n1: NetRef, n2: NetRef) -> List[str]:
    """Compare two NetRef objects."""
    diffs = []
    if n1.ordinal != n2.ordinal:
        diffs.append(f"ordinal: {n1.ordinal} != {n2.ordinal}")
    if n1.name != n2.name:
        diffs.append(f"name: '{n1.name}' != '{n2.name}'")
    return diffs


def compare_drill_props(d1: DrillProps | None, d2: DrillProps | None) -> List[str]:
    """Compare two DrillProps objects."""
    diffs = []
    if (d1 is None) != (d2 is None):
        diffs.append(f"presence: {d1 is not None} != {d2 is not None}")
        return diffs
    if d1 is None or d2 is None:
        return diffs
    if not compare_float(d1.size or 0.0, d2.size or 0.0):
        diffs.append(f"size: {d1.size} != {d2.size}")
    if d1.layers.start != d2.layers.start:
        diffs.append(f"layers.start: {d1.layers.start} != {d2.layers.start}")
    if d1.layers.end != d2.layers.end:
        diffs.append(f"layers.end: {d1.layers.end} != {d2.layers.end}")
    return diffs


def compare_post_machining(p1: PostMachiningProps | None, p2: PostMachiningProps | None) -> List[str]:
    """Compare two PostMachiningProps objects."""
    diffs = []
    if (p1 is None) != (p2 is None):
        diffs.append(f"presence: {p1 is not None} != {p2 is not None}")
        return diffs
    if p1 is None or p2 is None:
        return diffs
    if p1.mode != p2.mode:
        diffs.append(f"mode: {p1.mode} != {p2.mode}")
    if not compare_float(p1.size or 0.0, p2.size or 0.0):
        diffs.append(f"size: {p1.size} != {p2.size}")
    if not compare_float(p1.depth or 0.0, p2.depth or 0.0):
        diffs.append(f"depth: {p1.depth} != {p2.depth}")
    if not compare_float(p1.angle or 0.0, p2.angle or 0.0):
        diffs.append(f"angle: {p1.angle} != {p2.angle}")
    return diffs


def compare_zone_layer_connections(
    z1: ZoneLayerConnections | None, z2: ZoneLayerConnections | None
) -> List[str]:
    """Compare two ZoneLayerConnections objects."""
    diffs = []
    if (z1 is None) != (z2 is None):
        diffs.append(f"presence: {z1 is not None} != {z2 is not None}")
        return diffs
    if z1 is None or z2 is None:
        return diffs
    if tuple(z1.forced_layers) != tuple(z2.forced_layers):
        diffs.append(f"forced_layers: {z1.forced_layers} != {z2.forced_layers}")
    return diffs


def compare_pad_name_groups(g1: List[PadNameGroup], g2: List[PadNameGroup]) -> List[str]:
    """Compare ordered footprint pad-name groups."""
    diffs = []
    if len(g1) != len(g2):
        diffs.append(f"count: {len(g1)} != {len(g2)}")
        return diffs
    for index, (group1, group2) in enumerate(zip(g1, g2)):
        if tuple(group1.pad_names) != tuple(group2.pad_names):
            diffs.append(
                f"[{index}].pad_names: {group1.pad_names} != {group2.pad_names}"
            )
    return diffs


def compare_gr_text(t1: GrText, t2: GrText) -> List[str]:
    """Compare two GrText objects."""
    diffs = []
    if t1.text != t2.text:
        diffs.append(f"text: '{t1.text}' != '{t2.text}'")
    if not compare_float(t1.at_x, t2.at_x):
        diffs.append(f"at_x: {t1.at_x} != {t2.at_x}")
    if not compare_float(t1.at_y, t2.at_y):
        diffs.append(f"at_y: {t1.at_y} != {t2.at_y}")
    if not compare_float(t1.at_angle, t2.at_angle):
        diffs.append(f"at_angle: {t1.at_angle} != {t2.at_angle}")
    if t1.layer != t2.layer:
        diffs.append(f"layer: '{t1.layer}' != '{t2.layer}'")
    if t1.knockout != t2.knockout:
        diffs.append(f"knockout: {t1.knockout} != {t2.knockout}")
    if t1.uuid != t2.uuid:
        diffs.append(f"uuid: {t1.uuid} != {t2.uuid}")
    diffs.extend([f"effects.{d}" for d in compare_effects(t1.effects, t2.effects)])
    return diffs


def compare_gr_line(l1: GrLine, l2: GrLine) -> List[str]:
    """Compare two GrLine objects."""
    diffs = []
    if not compare_float(l1.start_x, l2.start_x):
        diffs.append(f"start_x: {l1.start_x} != {l2.start_x}")
    if not compare_float(l1.start_y, l2.start_y):
        diffs.append(f"start_y: {l1.start_y} != {l2.start_y}")
    if not compare_float(l1.end_x, l2.end_x):
        diffs.append(f"end_x: {l1.end_x} != {l2.end_x}")
    if not compare_float(l1.end_y, l2.end_y):
        diffs.append(f"end_y: {l1.end_y} != {l2.end_y}")
    if l1.layer != l2.layer:
        diffs.append(f"layer: '{l1.layer}' != '{l2.layer}'")
    diffs.extend([f"stroke.{d}" for d in compare_stroke(l1.stroke, l2.stroke)])
    return diffs


def compare_segment(s1: Segment, s2: Segment) -> List[str]:
    """Compare two Segment objects."""
    diffs = []
    if not compare_float(s1.start_x, s2.start_x):
        diffs.append(f"start_x: {s1.start_x} != {s2.start_x}")
    if not compare_float(s1.start_y, s2.start_y):
        diffs.append(f"start_y: {s1.start_y} != {s2.start_y}")
    if not compare_float(s1.end_x, s2.end_x):
        diffs.append(f"end_x: {s1.end_x} != {s2.end_x}")
    if not compare_float(s1.end_y, s2.end_y):
        diffs.append(f"end_y: {s1.end_y} != {s2.end_y}")
    if not compare_float(s1.width, s2.width):
        diffs.append(f"width: {s1.width} != {s2.width}")
    if s1.layer != s2.layer:
        diffs.append(f"layer: '{s1.layer}' != '{s2.layer}'")
    if s1.net != s2.net:
        diffs.append(f"net: {s1.net} != {s2.net}")
    return diffs


def compare_via(v1: Via, v2: Via) -> List[str]:
    """Compare two Via objects."""
    diffs = []
    if not compare_float(v1.at_x, v2.at_x):
        diffs.append(f"at_x: {v1.at_x} != {v2.at_x}")
    if not compare_float(v1.at_y, v2.at_y):
        diffs.append(f"at_y: {v1.at_y} != {v2.at_y}")
    if not compare_float(v1.size, v2.size):
        diffs.append(f"size: {v1.size} != {v2.size}")
    if not compare_float(v1.drill, v2.drill):
        diffs.append(f"drill: {v1.drill} != {v2.drill}")
    if v1.layers != v2.layers:
        diffs.append(f"layers: {v1.layers} != {v2.layers}")
    if v1.free != v2.free:
        diffs.append(f"free: {v1.free} != {v2.free}")
    if v1.tenting != v2.tenting:
        diffs.append(f"tenting: {v1.tenting} != {v2.tenting}")
    if v1.remove_unused_layers != v2.remove_unused_layers:
        diffs.append(
            f"remove_unused_layers: {v1.remove_unused_layers} != {v2.remove_unused_layers}"
        )
    if v1.keep_end_layers != v2.keep_end_layers:
        diffs.append(f"keep_end_layers: {v1.keep_end_layers} != {v2.keep_end_layers}")
    if v1.start_end_only != v2.start_end_only:
        diffs.append(f"start_end_only: {v1.start_end_only} != {v2.start_end_only}")
    if v1.net != v2.net:
        diffs.append(f"net: {v1.net} != {v2.net}")
    diffs.extend([f"backdrill.{d}" for d in compare_drill_props(v1.backdrill, v2.backdrill)])
    diffs.extend(
        [f"tertiary_drill.{d}" for d in compare_drill_props(v1.tertiary_drill, v2.tertiary_drill)]
    )
    diffs.extend(
        [
            f"front_post_machining.{d}"
            for d in compare_post_machining(v1.front_post_machining, v2.front_post_machining)
        ]
    )
    diffs.extend(
        [
            f"back_post_machining.{d}"
            for d in compare_post_machining(v1.back_post_machining, v2.back_post_machining)
        ]
    )
    diffs.extend(
        [
            f"zone_layer_connections.{d}"
            for d in compare_zone_layer_connections(v1.zone_layer_connections, v2.zone_layer_connections)
        ]
    )
    return diffs


def compare_arc_track(a1: Arc, a2: Arc) -> List[str]:
    """Compare two Arc (track) objects."""
    diffs = []
    if not compare_float(a1.start_x, a2.start_x):
        diffs.append(f"start_x: {a1.start_x} != {a2.start_x}")
    if not compare_float(a1.start_y, a2.start_y):
        diffs.append(f"start_y: {a1.start_y} != {a2.start_y}")
    if not compare_float(a1.mid_x, a2.mid_x):
        diffs.append(f"mid_x: {a1.mid_x} != {a2.mid_x}")
    if not compare_float(a1.mid_y, a2.mid_y):
        diffs.append(f"mid_y: {a1.mid_y} != {a2.mid_y}")
    if not compare_float(a1.end_x, a2.end_x):
        diffs.append(f"end_x: {a1.end_x} != {a2.end_x}")
    if not compare_float(a1.end_y, a2.end_y):
        diffs.append(f"end_y: {a1.end_y} != {a2.end_y}")
    if not compare_float(a1.width, a2.width):
        diffs.append(f"width: {a1.width} != {a2.width}")
    if a1.layer != a2.layer:
        diffs.append(f"layer: '{a1.layer}' != '{a2.layer}'")
    if a1.net != a2.net:
        diffs.append(f"net: {a1.net} != {a2.net}")
    return diffs


def compare_pad(p1: Pad, p2: Pad) -> List[str]:
    """Compare two Pad objects."""
    diffs = []
    if p1.number != p2.number:
        diffs.append(f"number: '{p1.number}' != '{p2.number}'")
    if p1.pad_type != p2.pad_type:
        diffs.append(f"pad_type: {p1.pad_type} != {p2.pad_type}")
    if p1.shape != p2.shape:
        diffs.append(f"shape: {p1.shape} != {p2.shape}")
    if not compare_float(p1.at_x, p2.at_x):
        diffs.append(f"at_x: {p1.at_x} != {p2.at_x}")
    if not compare_float(p1.at_y, p2.at_y):
        diffs.append(f"at_y: {p1.at_y} != {p2.at_y}")
    if not compare_float(p1.size_x, p2.size_x):
        diffs.append(f"size_x: {p1.size_x} != {p2.size_x}")
    if not compare_float(p1.size_y, p2.size_y):
        diffs.append(f"size_y: {p1.size_y} != {p2.size_y}")
    if p1.layers != p2.layers:
        diffs.append(f"layers: {p1.layers} != {p2.layers}")
    if p1.drill_oval != p2.drill_oval:
        diffs.append(f"drill_oval: {p1.drill_oval} != {p2.drill_oval}")
    if not compare_float(p1.drill or 0.0, p2.drill or 0.0):
        diffs.append(f"drill: {p1.drill} != {p2.drill}")
    if not compare_float(p1.drill_width or 0.0, p2.drill_width or 0.0):
        diffs.append(f"drill_width: {p1.drill_width} != {p2.drill_width}")
    if not compare_float(p1.drill_height or 0.0, p2.drill_height or 0.0):
        diffs.append(f"drill_height: {p1.drill_height} != {p2.drill_height}")
    if not compare_float(p1.drill_offset_x or 0.0, p2.drill_offset_x or 0.0):
        diffs.append(f"drill_offset_x: {p1.drill_offset_x} != {p2.drill_offset_x}")
    if not compare_float(p1.drill_offset_y or 0.0, p2.drill_offset_y or 0.0):
        diffs.append(f"drill_offset_y: {p1.drill_offset_y} != {p2.drill_offset_y}")
    diffs.extend([f"net.{d}" for d in compare_net_ref(p1.net, p2.net)])
    if p1.pinfunction != p2.pinfunction:
        diffs.append(f"pinfunction: {p1.pinfunction} != {p2.pinfunction}")
    if p1.pintype != p2.pintype:
        diffs.append(f"pintype: {p1.pintype} != {p2.pintype}")
    if not compare_float(p1.die_length or 0.0, p2.die_length or 0.0):
        diffs.append(f"die_length: {p1.die_length} != {p2.die_length}")
    if not compare_float(p1.solder_mask_margin or 0.0, p2.solder_mask_margin or 0.0):
        diffs.append(f"solder_mask_margin: {p1.solder_mask_margin} != {p2.solder_mask_margin}")
    if not compare_float(p1.solder_paste_margin or 0.0, p2.solder_paste_margin or 0.0):
        diffs.append(f"solder_paste_margin: {p1.solder_paste_margin} != {p2.solder_paste_margin}")
    if not compare_float(
        p1.solder_paste_margin_ratio or 0.0,
        p2.solder_paste_margin_ratio or 0.0,
    ):
        diffs.append(
            f"solder_paste_margin_ratio: {p1.solder_paste_margin_ratio} != {p2.solder_paste_margin_ratio}"
        )
    if not compare_float(p1.thermal_bridge_angle or 0.0, p2.thermal_bridge_angle or 0.0):
        diffs.append(f"thermal_bridge_angle: {p1.thermal_bridge_angle} != {p2.thermal_bridge_angle}")
    if p1.zone_connect != p2.zone_connect:
        diffs.append(f"zone_connect: {p1.zone_connect} != {p2.zone_connect}")
    if p1.remove_unused_layers != p2.remove_unused_layers:
        diffs.append(f"remove_unused_layers: {p1.remove_unused_layers} != {p2.remove_unused_layers}")
    if p1.keep_end_layers != p2.keep_end_layers:
        diffs.append(f"keep_end_layers: {p1.keep_end_layers} != {p2.keep_end_layers}")
    diffs.extend([f"backdrill.{d}" for d in compare_drill_props(p1.backdrill, p2.backdrill)])
    diffs.extend(
        [f"tertiary_drill.{d}" for d in compare_drill_props(p1.tertiary_drill, p2.tertiary_drill)]
    )
    diffs.extend(
        [
            f"front_post_machining.{d}"
            for d in compare_post_machining(p1.front_post_machining, p2.front_post_machining)
        ]
    )
    diffs.extend(
        [
            f"back_post_machining.{d}"
            for d in compare_post_machining(p1.back_post_machining, p2.back_post_machining)
        ]
    )
    diffs.extend(
        [
            f"zone_layer_connections.{d}"
            for d in compare_zone_layer_connections(p1.zone_layer_connections, p2.zone_layer_connections)
        ]
    )
    if not compare_float(p1.roundrect_rratio or 0.0, p2.roundrect_rratio or 0.0):
        diffs.append(f"roundrect_rratio: {p1.roundrect_rratio} != {p2.roundrect_rratio}")
    if not compare_float(p1.chamfer_ratio or 0.0, p2.chamfer_ratio or 0.0):
        diffs.append(f"chamfer_ratio: {p1.chamfer_ratio} != {p2.chamfer_ratio}")
    if set(p1.chamfer_corners) != set(p2.chamfer_corners):
        diffs.append(f"chamfer_corners: {p1.chamfer_corners} != {p2.chamfer_corners}")
    o1 = p1.custom_options
    o2 = p2.custom_options
    if (o1 is None) != (o2 is None):
        diffs.append(f"custom_options presence: {o1 is not None} != {o2 is not None}")
    elif o1 is not None and o2 is not None:
        if o1.clearance != o2.clearance:
            diffs.append(f"custom_options.clearance: {o1.clearance} != {o2.clearance}")
        if o1.anchor != o2.anchor:
            diffs.append(f"custom_options.anchor: {o1.anchor} != {o2.anchor}")

    if len(p1.custom_primitives) != len(p2.custom_primitives):
        diffs.append(
            f"custom_primitives count: {len(p1.custom_primitives)} != {len(p2.custom_primitives)}"
        )
    else:
        for i, (prim1, prim2) in enumerate(zip(p1.custom_primitives, p2.custom_primitives)):
            if prim1.primitive_type != prim2.primitive_type:
                diffs.append(
                    f"custom_primitives[{i}].primitive_type: {prim1.primitive_type} != {prim2.primitive_type}"
                )
            if prim1.fill != prim2.fill:
                diffs.append(f"custom_primitives[{i}].fill: {prim1.fill} != {prim2.fill}")
            if not compare_float(prim1.width or 0.0, prim2.width or 0.0):
                diffs.append(
                    f"custom_primitives[{i}].width: {prim1.width} != {prim2.width}"
                )
            if len(prim1.points) != len(prim2.points):
                diffs.append(
                    f"custom_primitives[{i}].points count: {len(prim1.points)} != {len(prim2.points)}"
                )
            else:
                for j, (pt1, pt2) in enumerate(zip(prim1.points, prim2.points)):
                    if not compare_float(pt1[0], pt2[0]) or not compare_float(pt1[1], pt2[1]):
                        diffs.append(
                            f"custom_primitives[{i}].points[{j}]: {pt1} != {pt2}"
                        )
    return diffs


def compare_footprint(fp1: Footprint, fp2: Footprint) -> List[str]:
    """Compare two Footprint objects."""
    diffs = []
    if fp1.library_link != fp2.library_link:
        diffs.append(f"library_link: '{fp1.library_link}' != '{fp2.library_link}'")
    if fp1.layer != fp2.layer:
        diffs.append(f"layer: '{fp1.layer}' != '{fp2.layer}'")
    if not compare_float(fp1.at_x, fp2.at_x):
        diffs.append(f"at_x: {fp1.at_x} != {fp2.at_x}")
    if not compare_float(fp1.at_y, fp2.at_y):
        diffs.append(f"at_y: {fp1.at_y} != {fp2.at_y}")
    if not compare_float(fp1.at_angle, fp2.at_angle):
        diffs.append(f"at_angle: {fp1.at_angle} != {fp2.at_angle}")
    diffs.extend([f"placement.{d}" for d in compare_footprint_placement(fp1.placement, fp2.placement)])
    if not compare_float(fp1.solder_mask_margin or 0.0, fp2.solder_mask_margin or 0.0):
        diffs.append(f"solder_mask_margin: {fp1.solder_mask_margin} != {fp2.solder_mask_margin}")
    if not compare_float(fp1.solder_paste_margin or 0.0, fp2.solder_paste_margin or 0.0):
        diffs.append(f"solder_paste_margin: {fp1.solder_paste_margin} != {fp2.solder_paste_margin}")
    if not compare_float(
        fp1.solder_paste_margin_ratio or 0.0,
        fp2.solder_paste_margin_ratio or 0.0,
    ):
        diffs.append(
            f"solder_paste_margin_ratio: {fp1.solder_paste_margin_ratio} != {fp2.solder_paste_margin_ratio}"
        )
    if not compare_float(fp1.clearance or 0.0, fp2.clearance or 0.0):
        diffs.append(f"clearance: {fp1.clearance} != {fp2.clearance}")
    if fp1.zone_connect != fp2.zone_connect:
        diffs.append(f"zone_connect: {fp1.zone_connect} != {fp2.zone_connect}")
    if fp1.locked != fp2.locked:
        diffs.append(f"locked: {fp1.locked} != {fp2.locked}")
    if fp1.duplicate_pad_numbers_are_jumpers != fp2.duplicate_pad_numbers_are_jumpers:
        diffs.append(
            "duplicate_pad_numbers_are_jumpers: "
            f"{fp1.duplicate_pad_numbers_are_jumpers} != {fp2.duplicate_pad_numbers_are_jumpers}"
        )
    diffs.extend(
        [f"net_tie_pad_groups.{d}" for d in compare_pad_name_groups(fp1.net_tie_pad_groups, fp2.net_tie_pad_groups)]
    )
    diffs.extend(
        [f"jumper_pad_groups.{d}" for d in compare_pad_name_groups(fp1.jumper_pad_groups, fp2.jumper_pad_groups)]
    )
    diffs.extend([f"images.{d}" for d in compare_lists(fp1.images, fp2.images, compare_image)])
    diffs.extend([f"tables.{d}" for d in compare_lists(fp1.tables, fp2.tables, compare_table)])
    diffs.extend([f"barcodes.{d}" for d in compare_lists(fp1.barcodes, fp2.barcodes, compare_barcode)])
    diffs.extend([f"dimensions.{d}" for d in compare_lists(fp1.dimensions, fp2.dimensions, compare_dimension)])
    diffs.extend([f"zones.{d}" for d in compare_lists(fp1.zones, fp2.zones, compare_zone)])
    diffs.extend([f"groups.{d}" for d in compare_lists(fp1.groups, fp2.groups, compare_group)])
    diffs.extend(
        [f"variants.{d}" for d in compare_lists(fp1.variants, fp2.variants, compare_footprint_variant)]
    )
    diffs.extend(
        [
            f"component_classes.{d}"
            for d in compare_lists(fp1.component_classes, fp2.component_classes, compare_component_class_ref)
        ]
    )

    # Compare pads
    if len(fp1.pads) != len(fp2.pads):
        diffs.append(f"pads count: {len(fp1.pads)} != {len(fp2.pads)}")
    else:
        for i, (p1, p2) in enumerate(zip(fp1.pads, fp2.pads)):
            pad_diffs = compare_pad(p1, p2)
            diffs.extend([f"pads[{i}].{d}" for d in pad_diffs])

    # Compare properties count
    if len(fp1.properties) != len(fp2.properties):
        diffs.append(f"properties count: {len(fp1.properties)} != {len(fp2.properties)}")

    return diffs


def compare_zone(z1: Zone, z2: Zone) -> List[str]:
    """Compare two Zone objects."""
    diffs = []
    diffs.extend([f"net.{d}" for d in compare_net_ref(z1.net, z2.net)])
    if z1.layer != z2.layer:
        diffs.append(f"layer: '{z1.layer}' != '{z2.layer}'")
    if z1.has_explicit_net_name != z2.has_explicit_net_name:
        diffs.append(
            f"has_explicit_net_name: {z1.has_explicit_net_name} != {z2.has_explicit_net_name}"
        )
    if len(z1.polygons) != len(z2.polygons):
        diffs.append(f"polygons count: {len(z1.polygons)} != {len(z2.polygons)}")
    if len(z1.filled_polygons) != len(z2.filled_polygons):
        diffs.append(f"filled_polygons count: {len(z1.filled_polygons)} != {len(z2.filled_polygons)}")
    # Compare keepout
    if (z1.keepout is None) != (z2.keepout is None):
        diffs.append(f"keepout presence: {z1.keepout is not None} != {z2.keepout is not None}")
    elif z1.keepout and z2.keepout:
        if z1.keepout.tracks != z2.keepout.tracks:
            diffs.append(f"keepout.tracks: {z1.keepout.tracks} != {z2.keepout.tracks}")
        if z1.keepout.vias != z2.keepout.vias:
            diffs.append(f"keepout.vias: {z1.keepout.vias} != {z2.keepout.vias}")
        if z1.keepout.pads != z2.keepout.pads:
            diffs.append(f"keepout.pads: {z1.keepout.pads} != {z2.keepout.pads}")
    return diffs


def compare_group(g1: Group, g2: Group) -> List[str]:
    """Compare two Group objects."""
    diffs = []
    if g1.name != g2.name:
        diffs.append(f"name: '{g1.name}' != '{g2.name}'")
    if g1.uuid != g2.uuid:
        diffs.append(f"uuid: {g1.uuid} != {g2.uuid}")
    if g1.locked != g2.locked:
        diffs.append(f"locked: {g1.locked} != {g2.locked}")
    if set(g1.members) != set(g2.members):
        diffs.append(f"members count: {len(g1.members)} != {len(g2.members)}")
    return diffs


def compare_dimension(d1: Dimension, d2: Dimension) -> List[str]:
    """Compare two Dimension objects."""
    diffs = []
    if d1.dimension_type != d2.dimension_type:
        diffs.append(f"dimension_type: '{d1.dimension_type}' != '{d2.dimension_type}'")
    if d1.layer != d2.layer:
        diffs.append(f"layer: '{d1.layer}' != '{d2.layer}'")
    if d1.uuid != d2.uuid:
        diffs.append(f"uuid: {d1.uuid} != {d2.uuid}")
    if d1.locked != d2.locked:
        diffs.append(f"locked: {d1.locked} != {d2.locked}")
    if not compare_float(d1.height, d2.height):
        diffs.append(f"height: {d1.height} != {d2.height}")
    if not compare_float(d1.leader_length or 0.0, d2.leader_length or 0.0):
        diffs.append(f"leader_length: {d1.leader_length} != {d2.leader_length}")
    if d1.orientation != d2.orientation:
        diffs.append(f"orientation: {d1.orientation} != {d2.orientation}")
    if d1.format.override_value != d2.format.override_value:
        diffs.append(f"format.override_value: {d1.format.override_value} != {d2.format.override_value}")
    if d1.format.suppress_zeroes != d2.format.suppress_zeroes:
        diffs.append(f"format.suppress_zeroes: {d1.format.suppress_zeroes} != {d2.format.suppress_zeroes}")
    if d1.style.text_frame != d2.style.text_frame:
        diffs.append(f"style.text_frame: {d1.style.text_frame} != {d2.style.text_frame}")
    if len(d1.points) != len(d2.points):
        diffs.append(f"points count: {len(d1.points)} != {len(d2.points)}")
    return diffs


def compare_pcb_objects(pcb1: KiCadPcb, pcb2: KiCadPcb) -> List[str]:
    """Compare two KiCadPcb objects at the OOP level."""
    diffs = []

    # Header
    if pcb1.version != pcb2.version:
        diffs.append(f"version: {pcb1.version} != {pcb2.version}")
    if pcb1.generator != pcb2.generator:
        diffs.append(f"generator: '{pcb1.generator}' != '{pcb2.generator}'")
    if pcb1.generator_version != pcb2.generator_version:
        diffs.append(f"generator_version: '{pcb1.generator_version}' != '{pcb2.generator_version}'")

    # General
    if not compare_float(pcb1.thickness, pcb2.thickness):
        diffs.append(f"thickness: {pcb1.thickness} != {pcb2.thickness}")
    if not compare_float(pcb1.pad_to_mask_clearance, pcb2.pad_to_mask_clearance):
        diffs.append(f"pad_to_mask_clearance: {pcb1.pad_to_mask_clearance} != {pcb2.pad_to_mask_clearance}")
    if not compare_float(pcb1.pad_to_paste_clearance, pcb2.pad_to_paste_clearance):
        diffs.append(f"pad_to_paste_clearance: {pcb1.pad_to_paste_clearance} != {pcb2.pad_to_paste_clearance}")
    if not compare_float(
        pcb1.pad_to_paste_clearance_ratio,
        pcb2.pad_to_paste_clearance_ratio,
    ):
        diffs.append(
            "pad_to_paste_clearance_ratio: "
            f"{pcb1.pad_to_paste_clearance_ratio} != {pcb2.pad_to_paste_clearance_ratio}"
        )

    # Layers
    diffs.extend([f"layers.{d}" for d in compare_lists(pcb1.layers, pcb2.layers, compare_layer)])

    # Nets
    diffs.extend([f"nets.{d}" for d in compare_lists(pcb1.nets, pcb2.nets, compare_net)])
    diffs.extend(
        [f"variants.{d}" for d in compare_lists(pcb1.variants, pcb2.variants, compare_board_variant)]
    )

    # Graphics
    diffs.extend([f"gr_texts.{d}" for d in compare_lists(pcb1.gr_texts, pcb2.gr_texts, compare_gr_text)])
    diffs.extend([f"gr_lines.{d}" for d in compare_lists(pcb1.gr_lines, pcb2.gr_lines, compare_gr_line)])
    diffs.extend([f"images.{d}" for d in compare_lists(pcb1.images, pcb2.images, compare_image)])
    diffs.extend([f"tables.{d}" for d in compare_lists(pcb1.tables, pcb2.tables, compare_table)])
    diffs.extend([f"barcodes.{d}" for d in compare_lists(pcb1.barcodes, pcb2.barcodes, compare_barcode)])

    # Footprints
    diffs.extend([f"footprints.{d}" for d in compare_lists(pcb1.footprints, pcb2.footprints, compare_footprint)])

    # Zones
    diffs.extend([f"zones.{d}" for d in compare_lists(pcb1.zones, pcb2.zones, compare_zone)])

    # Dimensions
    diffs.extend([f"dimensions.{d}" for d in compare_lists(pcb1.dimensions, pcb2.dimensions, compare_dimension)])

    # Tracks
    diffs.extend([f"segments.{d}" for d in compare_lists(pcb1.segments, pcb2.segments, compare_segment)])
    diffs.extend([f"vias.{d}" for d in compare_lists(pcb1.vias, pcb2.vias, compare_via)])
    diffs.extend([f"arcs.{d}" for d in compare_lists(pcb1.arcs, pcb2.arcs, compare_arc_track)])

    # Groups
    diffs.extend([f"groups.{d}" for d in compare_lists(pcb1.groups, pcb2.groups, compare_group)])
    diffs.extend(
        [f"generated_items.{d}" for d in compare_lists(pcb1.generated_items, pcb2.generated_items, compare_generated_object)]
    )

    # Embedded
    if pcb1.embedded_fonts != pcb2.embedded_fonts:
        diffs.append(f"embedded_fonts: {pcb1.embedded_fonts} != {pcb2.embedded_fonts}")

    return diffs


# ============================================================================
# OOP Equivalency Tests
# ============================================================================

class TestOOPEquivalency:
    """Test OOP-level equivalency after round-trip."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_oop_roundtrip_equivalency(self, pcb_path: Path):
        """Test that load -> serialize -> load produces equivalent objects."""
        # Load original file
        pcb1 = from_kicad_pcb(pcb_path)

        # Serialize to string
        serialized = pcb1.to_string()

        # Parse back into new object
        pcb2 = KiCadPcb.from_string(serialized)

        # Compare objects
        diffs = compare_pcb_objects(pcb1, pcb2)

        if diffs:
            # Show first 20 differences
            diff_msg = "\n".join(diffs[:20])
            if len(diffs) > 20:
                diff_msg += f"\n... and {len(diffs) - 20} more differences"
            pytest.fail(f"OOP equivalency failed:\n{diff_msg}")

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_layer_count_match(self, pcb_path: Path):
        """Test that layer count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.layers) == len(pcb2.layers)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_net_count_match(self, pcb_path: Path):
        """Test that net count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.nets) == len(pcb2.nets)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_footprint_count_match(self, pcb_path: Path):
        """Test that footprint count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.footprints) == len(pcb2.footprints)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_segment_count_match(self, pcb_path: Path):
        """Test that segment count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.segments) == len(pcb2.segments)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_via_count_match(self, pcb_path: Path):
        """Test that via count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.vias) == len(pcb2.vias)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_arc_count_match(self, pcb_path: Path):
        """Test that arc count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.arcs) == len(pcb2.arcs)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_zone_count_match(self, pcb_path: Path):
        """Test that zone count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.zones) == len(pcb2.zones)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_dimension_count_match(self, pcb_path: Path):
        """Test that dimension count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.dimensions) == len(pcb2.dimensions)

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_group_count_match(self, pcb_path: Path):
        """Test that group count matches after round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)
        pcb2 = KiCadPcb.from_string(pcb1.to_string())
        assert len(pcb1.groups) == len(pcb2.groups)


class TestKeeoutParsing:
    """Test keepout zone parsing."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_keepout_preserved(self, pcb_path: Path):
        """Test that keepout settings survive round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)

        # Find zones with keepout
        zones_with_keepout1 = [z for z in pcb1.zones if z.keepout is not None]

        if zones_with_keepout1:
            # Round-trip
            pcb2 = KiCadPcb.from_string(pcb1.to_string())
            zones_with_keepout2 = [z for z in pcb2.zones if z.keepout is not None]

            assert len(zones_with_keepout1) == len(zones_with_keepout2), \
                f"Keepout zone count mismatch: {len(zones_with_keepout1)} vs {len(zones_with_keepout2)}"


class TestDimensionParsing:
    """Test dimension parsing."""

    @pytest.mark.parametrize("pcb_path", get_all_pcb_files(), ids=get_pcb_test_ids())
    def test_dimension_preserved(self, pcb_path: Path):
        """Test that dimensions survive round-trip."""
        pcb1 = from_kicad_pcb(pcb_path)

        if pcb1.dimensions:
            # Round-trip
            pcb2 = KiCadPcb.from_string(pcb1.to_string())

            assert len(pcb1.dimensions) == len(pcb2.dimensions), \
                f"Dimension count mismatch: {len(pcb1.dimensions)} vs {len(pcb2.dimensions)}"

            # Check dimension details
            for d1, d2 in zip(pcb1.dimensions, pcb2.dimensions):
                assert d1.dimension_type == d2.dimension_type
                assert d1.layer == d2.layer
                assert compare_float(d1.height, d2.height)


# ============================================================================
# Run tests standalone
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v"])
