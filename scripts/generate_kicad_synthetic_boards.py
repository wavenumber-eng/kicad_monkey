r"""Generate synthetic KiCad PCB corpus fixtures.

The default target layout matches the KiCad corpus manifest builder:

    kicad/board_svg/input/<case_id>/<case_id>.kicad_pcb
    kicad/board_svg/input/<case_id>/<case_id>.kicad_pro
    kicad/board_svg/input/<case_id>/<case_id>.kicad_sch
    kicad/board_svg/input/<case_id>/case_metadata.json

Use a staging corpus root while developing, then run against the package-local
or external corpus root once the corpus maintenance window is clear.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, replace
from pathlib import Path
import sys
import uuid

from kicad_monkey.kicad_base import FillType, LayerType, PadShape, PadType, Stroke, StrokeType
from kicad_monkey.kicad_pad import Pad, PadCustomOptions, PadCustomPrimitive
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_pcb_footprint import Footprint
from kicad_monkey.kicad_pcb_gr_arc import GrArc
from kicad_monkey.kicad_pcb_gr_circle import GrCircle
from kicad_monkey.kicad_pcb_gr_line import GrLine
from kicad_monkey.kicad_pcb_gr_poly import GrPoly
from kicad_monkey.kicad_pcb_gr_text import Effects as GrTextEffects
from kicad_monkey.kicad_pcb_gr_text import Font as GrTextFont
from kicad_monkey.kicad_pcb_gr_text import GrText
from kicad_monkey.kicad_pcb_other import Layer, Net, NetRef, Stackup, StackupLayer
from kicad_monkey.kicad_pcb_routing import FrontBackOptBool, Via
from kicad_monkey.kicad_primitives import Effects as PropertyEffects
from kicad_monkey.kicad_primitives import Font as PropertyFont
from kicad_monkey.kicad_property import Property

REPO_ROOT = Path(__file__).resolve().parents[1]


BOARD_VERSION = 20241229
GENERATOR_VERSION = "9.2"
PAD_SHAPES_CASE_ID = "synthetic_pad_shapes"
BOARD_CUTOUTS_CASE_ID = "synthetic_board_cutouts"
CASE_ID = PAD_SHAPES_CASE_ID
NAMESPACE = uuid.uuid5(uuid.NAMESPACE_URL, "wavenumber/kicad/synthetic-fixtures")


def _uid(*parts: object) -> str:
    return str(uuid.uuid5(NAMESPACE, "/".join(str(part) for part in parts)))


def _q(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


@dataclass(frozen=True)
class SyntheticItem:
    key: str
    label: str
    kind: str
    side: str = "F"
    pad_type: str = "smd"
    shape: str = "rect"
    size: tuple[float, float] = (2.4, 1.4)
    rotation: float = 0.0
    layers: tuple[str, ...] | None = None
    drill: float | None = None
    drill_oval: tuple[float, float] | None = None
    drill_offset: tuple[float, float] | None = None
    roundrect_rratio: float | None = None
    chamfer_ratio: float | None = None
    chamfer_corners: tuple[str, ...] = ()
    custom_polygon: tuple[tuple[float, float], ...] = ()
    custom_anchor: str = "rect"
    custom_primitive_width: float = 0.01
    solder_mask_margin: float | None = None
    solder_paste_margin: float | None = None
    solder_paste_margin_ratio: float | None = None
    clearance: float | None = None
    zone_connect: int | None = None
    via_type: str | None = None
    via_layers: tuple[str, str] = ("F.Cu", "B.Cu")
    via_size: float = 0.8
    via_drill: float = 0.3
    via_tenting: tuple[bool | None, bool | None] | None = None
    via_covering: tuple[bool | None, bool | None] | None = None
    via_plugging: tuple[bool | None, bool | None] | None = None
    via_capping: bool | None = None
    via_filling: bool | None = None
    via_in_pad: bool = False


ROTATION_CASES: tuple[float, ...] = (0.0, 45.0, 90.0, 17.0, 123.0)

SMD_PAD_BASE_ITEMS: tuple[SyntheticItem, ...] = (
    SyntheticItem("rect", "RECT", "pad", shape="rect"),
    SyntheticItem("circle", "CIRC", "pad", shape="circle", size=(1.9, 1.9)),
    SyntheticItem("oval", "OVAL", "pad", shape="oval", size=(3.0, 1.4)),
    SyntheticItem(
        "roundrect",
        "RR25",
        "pad",
        shape="roundrect",
        size=(2.8, 1.5),
        roundrect_rratio=0.25,
    ),
    SyntheticItem(
        "cham_diag",
        "CHAM-D",
        "pad",
        shape="roundrect",
        size=(2.8, 1.6),
        roundrect_rratio=0,
        chamfer_ratio=0.25,
        chamfer_corners=("top_left", "bottom_right"),
    ),
    SyntheticItem(
        "cham_all",
        "CHAM-A",
        "pad",
        shape="roundrect",
        size=(2.8, 1.6),
        roundrect_rratio=0,
        chamfer_ratio=0.2,
        chamfer_corners=("top_left", "top_right", "bottom_left", "bottom_right"),
    ),
    SyntheticItem(
        "speedy_0402",
        "SPD0402",
        "pad",
        shape="roundrect",
        size=(0.54, 0.46),
        roundrect_rratio=0,
        chamfer_ratio=0.2,
        chamfer_corners=("top_left", "top_right", "bottom_left", "bottom_right"),
    ),
    SyntheticItem(
        "custom_poly",
        "CUSTOM",
        "pad",
        shape="custom",
        size=(0.5, 0.5),
        custom_polygon=((-1.1, -0.8), (1.1, -0.8), (1.1, -0.25), (0.35, -0.25), (0.35, 0.8), (-1.1, 0.8)),
    ),
    SyntheticItem(
        "custom_mask_triangle",
        "CUST-MASK-TRI",
        "pad",
        shape="custom",
        size=(0.000001, 0.000001),
        custom_polygon=((0.0, 0.0), (-0.245, 0.25), (-0.425, 0.25), (-0.425, -0.03), (0.0, -0.03)),
        custom_anchor="circle",
        custom_primitive_width=0.0,
        solder_mask_margin=0.05,
        solder_paste_margin=0.000001,
    ),
    SyntheticItem(
        "mask_expand",
        "MASK+25",
        "pad",
        shape="rect",
        size=(2.4, 1.4),
        solder_mask_margin=0.25,
    ),
    SyntheticItem(
        "mask_negative",
        "MASK-10",
        "pad",
        shape="rect",
        size=(2.4, 1.4),
        solder_mask_margin=-0.10,
    ),
    SyntheticItem(
        "paste_shrink",
        "PASTE-",
        "pad",
        shape="rect",
        size=(2.4, 1.4),
        solder_paste_margin=-0.05,
        solder_paste_margin_ratio=-0.10,
    ),
    SyntheticItem(
        "pad_no_mask",
        "NO-MASK",
        "pad",
        shape="rect",
        size=(2.4, 1.4),
        layers=("F.Cu", "F.Paste"),
    ),
    SyntheticItem(
        "pad_no_paste",
        "NO-PASTE",
        "pad",
        shape="rect",
        size=(2.4, 1.4),
        layers=("F.Cu", "F.Mask"),
    ),
    SyntheticItem(
        "local_clearance",
        "CLR-ZONE",
        "pad",
        shape="roundrect",
        size=(2.6, 1.5),
        roundrect_rratio=0.2,
        clearance=0.35,
        zone_connect=2,
    ),
    SyntheticItem(
        "vip_filled",
        "VIP-FILL",
        "via",
        shape="circle",
        size=(2.2, 2.2),
        via_in_pad=True,
        via_size=0.75,
        via_drill=0.28,
        via_tenting=(True, True),
        via_covering=(True, True),
        via_plugging=(True, True),
        via_capping=True,
        via_filling=True,
    ),
)

THROUGH_HOLE_PAD_BASE_ITEMS: tuple[SyntheticItem, ...] = (
    SyntheticItem(
        "th_round",
        "TH-RND",
        "pad",
        pad_type="thru_hole",
        shape="circle",
        size=(2.4, 2.4),
        drill=0.8,
    ),
    SyntheticItem(
        "th_rect",
        "TH-RECT",
        "pad",
        pad_type="thru_hole",
        shape="rect",
        size=(2.6, 1.8),
        drill=0.75,
    ),
    SyntheticItem(
        "th_oval_slot",
        "TH-SLOT",
        "pad",
        pad_type="thru_hole",
        shape="oval",
        size=(3.2, 1.8),
        drill_oval=(0.75, 1.8),
    ),
    SyntheticItem(
        "th_oval_slot_major_first",
        "TH-SLOT-MAJ1",
        "pad",
        pad_type="thru_hole",
        shape="oval",
        size=(1.6, 1.1),
        drill_oval=(1.1, 0.6),
    ),
    SyntheticItem(
        "npth_round",
        "NPTH-RND",
        "pad",
        pad_type="np_thru_hole",
        shape="circle",
        size=(2.0, 2.0),
        drill=1.2,
    ),
    SyntheticItem(
        "npth_slot",
        "NPTH-SLOT",
        "pad",
        pad_type="np_thru_hole",
        shape="oval",
        size=(3.0, 1.6),
        drill_oval=(0.7, 1.8),
    ),
    SyntheticItem(
        "drill_offset",
        "OFF-DRILL",
        "pad",
        pad_type="thru_hole",
        shape="oval",
        size=(3.0, 2.0),
        drill=0.7,
        drill_offset=(0.45, 0),
    ),
)

VIA_ONLY_ITEMS: tuple[SyntheticItem, ...] = (
    SyntheticItem(
        "via_untent",
        "VIA UNTENT",
        "via",
        via_size=1.0,
        via_drill=0.45,
        via_tenting=(False, False),
        via_covering=(False, False),
        via_plugging=(False, False),
        via_capping=False,
        via_filling=False,
    ),
    SyntheticItem(
        "via_front_tent",
        "VIA F TENT",
        "via",
        via_size=1.0,
        via_drill=0.45,
        via_tenting=(True, False),
    ),
    SyntheticItem(
        "blind_f_in1",
        "BLIND F-IN1",
        "via",
        via_type="blind",
        via_layers=("F.Cu", "In1.Cu"),
        via_size=0.75,
        via_drill=0.28,
    ),
    SyntheticItem(
        "buried_in1_in2",
        "BURIED 1-2",
        "via",
        via_type="buried",
        via_layers=("In1.Cu", "In2.Cu"),
        via_size=0.75,
        via_drill=0.28,
    ),
    SyntheticItem(
        "blind_in2_b",
        "BLIND IN2-B",
        "via",
        via_type="blind",
        via_layers=("In2.Cu", "B.Cu"),
        via_size=0.75,
        via_drill=0.28,
    ),
    SyntheticItem(
        "micro_f_in1",
        "MICRO F-IN1",
        "via",
        via_type="micro",
        via_layers=("F.Cu", "In1.Cu"),
        via_size=0.4,
        via_drill=0.15,
    ),
)


def _rotation_label(rotation: float) -> str:
    return str(int(rotation)) if rotation.is_integer() else f"{rotation:g}"


def _rotation_key(rotation: float) -> str:
    return f"r{int(rotation):03d}" if rotation.is_integer() else f"r{rotation:g}".replace(".", "p")


def _layers_for_side(layers: tuple[str, ...] | None, side: str) -> tuple[str, ...] | None:
    if layers is None or side == "F":
        return layers
    return tuple(layer.replace("F.", "B.", 1) if layer.startswith("F.") else layer for layer in layers)


def _rotated_item(item: SyntheticItem, *, rotation: float, side: str | None = None) -> SyntheticItem:
    label_parts = []
    key_parts = []
    if side is not None:
        label_parts.append(side)
        key_parts.append(side.lower())
    label_parts.extend((item.label, f"R{_rotation_label(rotation)}"))
    key_parts.extend((item.key, _rotation_key(rotation)))
    resolved_side = side if side is not None else item.side
    return replace(
        item,
        key="_".join(key_parts),
        label=" ".join(label_parts),
        side=resolved_side,
        rotation=rotation,
        layers=_layers_for_side(item.layers, resolved_side),
    )


def _build_pad_items() -> tuple[SyntheticItem, ...]:
    items: list[SyntheticItem] = []
    for item in SMD_PAD_BASE_ITEMS:
        for side in ("F", "B"):
            for rotation in ROTATION_CASES:
                items.append(_rotated_item(item, rotation=rotation, side=side))
    for item in THROUGH_HOLE_PAD_BASE_ITEMS:
        for rotation in ROTATION_CASES:
            items.append(_rotated_item(item, rotation=rotation))
    items.extend(VIA_ONLY_ITEMS)
    return tuple(items)


PAD_ITEMS: tuple[SyntheticItem, ...] = _build_pad_items()


def _layers_for_item(item: SyntheticItem) -> tuple[str, ...]:
    if item.layers is not None:
        return item.layers
    if item.pad_type in {"thru_hole", "np_thru_hole"}:
        return ("*.Cu", "*.Mask")
    if item.side == "B":
        return ("B.Cu", "B.Mask", "B.Paste")
    return ("F.Cu", "F.Mask", "F.Paste")


def _kicad_layers() -> list[Layer]:
    return [
        Layer(0, "F.Cu", LayerType.SIGNAL),
        Layer(1, "In1.Cu", LayerType.SIGNAL),
        Layer(2, "In2.Cu", LayerType.SIGNAL),
        Layer(31, "B.Cu", LayerType.SIGNAL),
        Layer(32, "B.Adhes", LayerType.USER, "B.Adhesive"),
        Layer(33, "F.Adhes", LayerType.USER, "F.Adhesive"),
        Layer(34, "B.Paste", LayerType.USER),
        Layer(35, "F.Paste", LayerType.USER),
        Layer(36, "B.SilkS", LayerType.USER, "B.Silkscreen"),
        Layer(37, "F.SilkS", LayerType.USER, "F.Silkscreen"),
        Layer(38, "B.Mask", LayerType.USER),
        Layer(39, "F.Mask", LayerType.USER),
        Layer(44, "Edge.Cuts", LayerType.USER),
        Layer(45, "Margin", LayerType.USER),
        Layer(46, "B.CrtYd", LayerType.USER, "B.Courtyard"),
        Layer(47, "F.CrtYd", LayerType.USER, "F.Courtyard"),
        Layer(48, "B.Fab", LayerType.USER),
        Layer(49, "F.Fab", LayerType.USER),
    ]


def _stackup() -> Stackup:
    return Stackup(
        layers=[
            StackupLayer("F.SilkS", "Top Silk Screen", color="White"),
            StackupLayer("F.Paste", "Top Solder Paste"),
            StackupLayer("F.Mask", "Top Solder Mask", thickness=0.015, color="Green"),
            StackupLayer("F.Cu", "copper", thickness=0.035),
            StackupLayer(
                "dielectric 1",
                "prepreg",
                thickness=0.18,
                material="FR4",
                epsilon_r=4.2,
                loss_tangent=0.02,
            ),
            StackupLayer("In1.Cu", "copper", thickness=0.018),
            StackupLayer(
                "dielectric 2",
                "core",
                thickness=1.1,
                material="FR4",
                epsilon_r=4.2,
                loss_tangent=0.02,
            ),
            StackupLayer("In2.Cu", "copper", thickness=0.018),
            StackupLayer(
                "dielectric 3",
                "prepreg",
                thickness=0.18,
                material="FR4",
                epsilon_r=4.2,
                loss_tangent=0.02,
            ),
            StackupLayer("B.Cu", "copper", thickness=0.035),
            StackupLayer("B.Mask", "Bottom Solder Mask", thickness=0.015, color="Green"),
            StackupLayer("B.Paste", "Bottom Solder Paste"),
            StackupLayer("B.SilkS", "Bottom Silk Screen", color="White"),
        ],
        copper_finish="None",
        dielectric_constraints=True,
    )


def _setup_sexp() -> list[object]:
    return [
        "setup",
        _stackup().to_sexp(),
        ["pad_to_mask_clearance", 0.05],
        ["allow_soldermask_bridges_in_footprints", "no"],
        ["blind_buried_vias_allowed", "yes"],
        ["uvias_allowed", "yes"],
        ["tenting", ["front", "yes"], ["back", "yes"]],
        ["covering", ["front", "no"], ["back", "no"]],
        ["plugging", ["front", "no"], ["back", "no"]],
        ["capping", "no"],
        ["filling", "no"],
    ]


def _pad_for_item(item: SyntheticItem, *, ref: str, net_id: int, net_name: str) -> Pad:
    pad = Pad(
        number="" if item.pad_type == "np_thru_hole" else "1",
        pad_type=PadType(item.pad_type),
        shape=PadShape(item.shape),
        at_x=0.0,
        at_y=0.0,
        at_angle=item.rotation,
        size_x=item.size[0],
        size_y=item.size[1],
        layers=list(_layers_for_item(item)),
        net=NetRef(net_id, net_name) if item.pad_type != "np_thru_hole" else NetRef(),
        uuid=_uid(ref, item.key, "pad"),
        pinfunction="1" if item.pad_type != "np_thru_hole" else None,
        pintype="passive" if item.pad_type != "np_thru_hole" else None,
        roundrect_rratio=item.roundrect_rratio,
        chamfer_ratio=item.chamfer_ratio,
        chamfer_corners=list(item.chamfer_corners),
        solder_mask_margin=item.solder_mask_margin,
        solder_paste_margin=item.solder_paste_margin,
        solder_paste_margin_ratio=item.solder_paste_margin_ratio,
        clearance=item.clearance,
        zone_connect=item.zone_connect,
    )

    if item.drill_oval is not None:
        pad.drill_oval = True
        pad.drill_width = item.drill_oval[0]
        pad.drill_height = item.drill_oval[1]
    elif item.drill is not None:
        pad.drill = item.drill

    if item.drill_offset is not None:
        pad.drill_offset_x = item.drill_offset[0]
        pad.drill_offset_y = item.drill_offset[1]

    if item.shape == "custom":
        pad.custom_options = PadCustomOptions(clearance="outline", anchor=item.custom_anchor)
        pad.custom_primitives = [
            PadCustomPrimitive(
                primitive_type="gr_poly",
                points=list(item.custom_polygon),
                width=item.custom_primitive_width,
                fill=FillType.YES,
            )
        ]

    return pad


def _property_effects(size: float, thickness: float) -> PropertyEffects:
    return PropertyEffects(font=PropertyFont(size_x=size, size_y=size, thickness=thickness))


def _footprint_for_item(item: SyntheticItem, *, index: int, x: float, y: float, net_id: int, net_name: str) -> Footprint:
    ref = f"P{index:02d}"
    prop_layer = "B.Fab" if item.side == "B" else "F.Fab"
    return Footprint(
        library_link=f"Synthetic:{item.key}",
        layer="B.Cu" if item.side == "B" else "F.Cu",
        at_x=x,
        at_y=y,
        at_angle=0.0,
        uuid=_uid(ref, item.key, "footprint"),
        properties=[
            Property(
                "Reference",
                ref,
                at_x=0.0,
                at_y=-2.6,
                at_angle=0.0,
                layer=prop_layer,
                hide=True,
                uuid=_uid(ref, item.key, "reference"),
                effects=_property_effects(0.8, 0.1),
            ),
            Property(
                "Value",
                item.label,
                at_x=0.0,
                at_y=2.6,
                at_angle=0.0,
                layer=prop_layer,
                hide=True,
                uuid=_uid(ref, item.key, "value"),
                effects=_property_effects(0.8, 0.1),
            ),
        ],
        pads=[_pad_for_item(item, ref=ref, net_id=net_id, net_name=net_name)],
    )


def _front_back_values(values: tuple[bool | None, bool | None] | None) -> FrontBackOptBool | None:
    if values is None:
        return None
    front, back = values
    return FrontBackOptBool(front=front, back=back)


def _via_for_item(item: SyntheticItem, *, index: int, x: float, y: float, net_id: int) -> Via:
    return Via(
        at_x=x,
        at_y=y,
        size=item.via_size,
        drill=item.via_drill,
        layers=list(item.via_layers),
        tenting=_front_back_values(item.via_tenting),
        covering=_front_back_values(item.via_covering),
        plugging=_front_back_values(item.via_plugging),
        capping=item.via_capping,
        filling=item.via_filling,
        net=NetRef(ordinal=net_id),
        uuid=_uid(index, item.key, "via"),
        via_type=item.via_type,
    )


def _label_for_item(item: SyntheticItem, *, index: int, x: float, y: float) -> GrText:
    is_bottom = item.side == "B"
    return GrText(
        text=item.label,
        at_x=x,
        at_y=y - 3.6,
        at_angle=0.0,
        layer="B.SilkS" if is_bottom else "F.SilkS",
        uuid=_uid(index, item.key, "label"),
        effects=GrTextEffects(
            font=GrTextFont(size_x=0.8, size_y=0.8, thickness=0.1),
            justify=["bottom", "mirror"] if is_bottom else ["bottom"],
        ),
    )


def _outline(width: float, height: float) -> list[GrLine]:
    corners = ((0.0, 0.0), (width, 0.0), (width, height), (0.0, height))
    result: list[GrLine] = []
    for index, (start, end) in enumerate(zip(corners, corners[1:] + corners[:1]), start=1):
        result.append(
            GrLine(
                start_x=start[0],
                start_y=start[1],
                end_x=end[0],
                end_y=end[1],
                layer="Edge.Cuts",
                stroke=Stroke(width=0.1, type=StrokeType.DEFAULT),
                uuid=_uid("outline", index),
            )
        )
    return result


def build_pad_shapes_board(*, spacing_mm: float, margin_mm: float, columns: int) -> KiCadPcb:
    if columns < 1:
        raise ValueError("columns must be >= 1")
    rows = (len(PAD_ITEMS) + columns - 1) // columns
    top_label_clearance = 5.0
    bottom_clearance = 6.0
    width = margin_mm * 2 + spacing_mm * (columns - 1)
    height = margin_mm * 2 + top_label_clearance + bottom_clearance + spacing_mm * (rows - 1)
    x0 = margin_mm
    y0 = margin_mm + top_label_clearance

    pcb = KiCadPcb()
    pcb.version = BOARD_VERSION
    pcb.generator = "generate_kicad_synthetic_boards.py"
    pcb.generator_version = GENERATOR_VERSION
    pcb.thickness = 1.6
    pcb.paper = "A4"
    pcb.layers = _kicad_layers()
    pcb.setup_sexp = _setup_sexp()
    pcb.nets = [Net(0, "")]
    pcb.gr_lines = _outline(width, height)

    for index, item in enumerate(PAD_ITEMS, start=1):
        col = (index - 1) % columns
        row = (index - 1) // columns
        x = x0 + col * spacing_mm
        y = y0 + row * spacing_mm
        net_name = f"SYN_{item.key.upper()}"
        pcb.nets.append(Net(index, net_name))
        pcb.gr_texts.append(_label_for_item(item, index=index, x=x, y=y))
        if item.kind == "pad" or item.via_in_pad:
            pcb.footprints.append(_footprint_for_item(item, index=index, x=x, y=y, net_id=index, net_name=net_name))
        if item.kind == "via":
            pcb.vias.append(_via_for_item(item, index=index, x=x, y=y, net_id=index))

    return pcb


def render_pad_shapes_board(*, spacing_mm: float, margin_mm: float, columns: int) -> str:
    return build_pad_shapes_board(spacing_mm=spacing_mm, margin_mm=margin_mm, columns=columns).to_string()


def _edge_stroke() -> Stroke:
    return Stroke(width=0.1, type=StrokeType.DEFAULT)


def _edge_line(key: str, start: tuple[float, float], end: tuple[float, float]) -> GrLine:
    return GrLine(
        start_x=start[0],
        start_y=start[1],
        end_x=end[0],
        end_y=end[1],
        layer="Edge.Cuts",
        stroke=_edge_stroke(),
        uuid=_uid("edge", key),
    )


def _edge_arc(
    key: str,
    start: tuple[float, float],
    mid: tuple[float, float],
    end: tuple[float, float],
) -> GrArc:
    return GrArc(
        start_x=start[0],
        start_y=start[1],
        mid_x=mid[0],
        mid_y=mid[1],
        end_x=end[0],
        end_y=end[1],
        layer="Edge.Cuts",
        stroke=_edge_stroke(),
        uuid=_uid("edge", key),
    )


def _edge_circle(key: str, center: tuple[float, float], radius: float) -> GrCircle:
    return GrCircle(
        center_x=center[0],
        center_y=center[1],
        end_x=center[0] + radius,
        end_y=center[1],
        layer="Edge.Cuts",
        stroke=_edge_stroke(),
        fill=FillType.NO,
        uuid=_uid("edge", key),
    )


def _edge_poly(key: str, points: tuple[tuple[float, float], ...]) -> GrPoly:
    return GrPoly(
        points=list(points),
        layer="Edge.Cuts",
        stroke=_edge_stroke(),
        fill=FillType.NO,
        uuid=_uid("edge", key),
    )


def _board_cutout_label(text: str, *, x: float, y: float) -> GrText:
    return GrText(
        text=text,
        at_x=x,
        at_y=y,
        at_angle=0.0,
        layer="F.SilkS",
        uuid=_uid("board-cutouts", text, x, y),
        effects=GrTextEffects(
            font=GrTextFont(size_x=1.0, size_y=1.0, thickness=0.12),
            justify=["bottom"],
        ),
    )


def build_board_cutouts_board(*, margin_mm: float = 8.0) -> KiCadPcb:
    pcb = KiCadPcb()
    pcb.version = BOARD_VERSION
    pcb.generator = "generate_kicad_synthetic_boards.py"
    pcb.generator_version = GENERATOR_VERSION
    pcb.thickness = 1.6
    pcb.paper = "A4"
    pcb.layers = _kicad_layers()
    pcb.setup_sexp = _setup_sexp()
    pcb.nets = [Net(0, "")]

    ox = margin_mm
    oy = margin_mm

    def pt(x: float, y: float) -> tuple[float, float]:
        return (ox + x, oy + y)

    pcb.gr_lines = [
        _edge_line("outline-top", pt(8.0, 0.0), pt(92.0, 0.0)),
        _edge_line("outline-right", pt(100.0, 8.0), pt(100.0, 48.0)),
        _edge_line("outline-bottom-right", pt(92.0, 56.0), pt(68.0, 56.0)),
        _edge_line("outline-tab-right", pt(68.0, 56.0), pt(62.0, 68.0)),
        _edge_line("outline-bottom", pt(62.0, 68.0), pt(18.0, 68.0)),
        _edge_line("outline-tab-left", pt(18.0, 68.0), pt(12.0, 56.0)),
        _edge_line("outline-left-bottom", pt(12.0, 56.0), pt(0.0, 56.0)),
        _edge_line("outline-left", pt(0.0, 56.0), pt(0.0, 14.0)),
        _edge_line("outline-upper-left", pt(0.0, 14.0), pt(8.0, 0.0)),
        _edge_line("rect-cutout-top", pt(18.0, 16.0), pt(34.0, 16.0)),
        _edge_line("rect-cutout-right", pt(34.0, 16.0), pt(34.0, 28.0)),
        _edge_line("rect-cutout-bottom", pt(34.0, 28.0), pt(18.0, 28.0)),
        _edge_line("rect-cutout-left", pt(18.0, 28.0), pt(18.0, 16.0)),
        _edge_line("slot-cutout-top", pt(62.0, 42.0), pt(80.0, 42.0)),
        _edge_line("slot-cutout-bottom", pt(80.0, 52.0), pt(62.0, 52.0)),
    ]
    pcb.gr_arcs = [
        _edge_arc("outline-top-right-radius", pt(92.0, 0.0), pt(97.657, 2.343), pt(100.0, 8.0)),
        _edge_arc("outline-bottom-right-radius", pt(100.0, 48.0), pt(97.657, 53.657), pt(92.0, 56.0)),
        _edge_arc("slot-cutout-right-radius", pt(80.0, 42.0), pt(85.0, 47.0), pt(80.0, 52.0)),
        _edge_arc("slot-cutout-left-radius", pt(62.0, 52.0), pt(57.0, 47.0), pt(62.0, 42.0)),
    ]
    pcb.gr_circles = [
        _edge_circle("circle-cutout", pt(55.0, 22.0), 5.0),
    ]
    pcb.gr_polys = [
        _edge_poly("poly-cutout", (pt(43.0, 44.0), pt(51.0, 36.0), pt(55.0, 51.0))),
    ]
    pcb.gr_texts = [
        _board_cutout_label("complex outline", x=pt(8.0, -2.0)[0], y=pt(8.0, -2.0)[1]),
        _board_cutout_label("line cutout", x=pt(18.0, 14.0)[0], y=pt(18.0, 14.0)[1]),
        _board_cutout_label("circle cutout", x=pt(49.0, 14.0)[0], y=pt(49.0, 14.0)[1]),
        _board_cutout_label("arc slot", x=pt(62.0, 40.0)[0], y=pt(62.0, 40.0)[1]),
        _board_cutout_label("poly cutout", x=pt(40.0, 34.0)[0], y=pt(40.0, 34.0)[1]),
    ]

    return pcb


def render_board_cutouts_board(*, margin_mm: float) -> str:
    return build_board_cutouts_board(margin_mm=margin_mm).to_string()


def _render_board_for_case(
    case_id: str,
    *,
    spacing_mm: float,
    margin_mm: float,
    columns: int,
) -> str:
    if case_id == BOARD_CUTOUTS_CASE_ID:
        return render_board_cutouts_board(margin_mm=margin_mm)
    return render_pad_shapes_board(spacing_mm=spacing_mm, margin_mm=margin_mm, columns=columns)


def _minimal_project_json(project_name: str) -> str:
    data = {
        "board": {
            "design_settings": {
                "defaults": {
                    "board_outline_line_width": 0.1,
                    "copper_line_width": 0.2,
                    "copper_text_size_h": 1.5,
                    "copper_text_size_v": 1.5,
                    "silk_line_width": 0.12,
                    "silk_text_size_h": 1.0,
                    "silk_text_size_v": 1.0,
                }
            }
        },
        "meta": {
            "filename": f"{project_name}.kicad_pro",
            "version": 1,
        },
        "net_settings": {
            "classes": [
                {
                    "name": "Default",
                    "clearance": 0.2,
                    "track_width": 0.25,
                    "via_diameter": 0.8,
                    "via_drill": 0.3,
                    "microvia_diameter": 0.4,
                    "microvia_drill": 0.15,
                }
            ],
            "meta": {"version": 3},
            "net_colors": {},
            "netclass_assignments": {},
            "netclass_patterns": [],
        },
        "project": {"files": []},
        "text_variables": {
            "SYNTHETIC_FIXTURE": project_name,
        },
    }
    return json.dumps(data, indent=2, sort_keys=True) + "\n"


def _minimal_schematic_text(project_name: str) -> str:
    return (
        f"(kicad_sch\n"
        f"  (version 20250114)\n"
        f"  (generator {_q('generate_kicad_synthetic_boards.py')})\n"
        f"  (generator_version {_q(GENERATOR_VERSION)})\n"
        f"  (uuid {_q(_uid(project_name, 'schematic'))})\n"
        f"  (paper {_q('A4')})\n"
        f"  (title_block\n"
        f"    (title {_q(project_name)})\n"
        f"    (comment 1 {_q('Synthetic KiCad PCB fixture; geometry lives on the PCB.')})\n"
        f"  )\n"
        f"  (lib_symbols)\n"
        f"  (text {_q('Synthetic PCB fixture. Open the PCB editor for geometry.')}\n"
        f"    (exclude_from_sim yes)\n"
        f"    (at 20 20 0)\n"
        f"    (effects (font (size 1.27 1.27)))\n"
        f"    (uuid {_q(_uid(project_name, 'schematic-note'))})\n"
        f"  )\n"
        f"  (sheet_instances\n"
        f"    (path {_q('/')}\n"
        f"      (page {_q('1')})\n"
        f"    )\n"
        f"  )\n"
        f"  (embedded_fonts no)\n"
        f")\n"
    )


def _case_metadata(case_id: str, *, layout: str) -> dict[str, object]:
    base_domains = ["pcb_ir", "pcb_data_models", "pcb_viz_3d"]
    if layout == "board_svg":
        base_domains.insert(0, "board_svg")
    if case_id == BOARD_CUTOUTS_CASE_ID:
        return {
            "origin": "synthetic",
            "status": "active",
            "domains": base_domains,
            "tags": [
                "synthetic",
                "data_models",
                "viz_3d",
                "board_svg",
                "focused_feature_case",
                "board_profile",
                "cutouts",
                "edge_cuts",
            ],
            "test_intent": (
                "Exercise KiCad Edge.Cuts board-profile import through pcb_a0 "
                "and 3D/viz rendering, including a complex outer shape and "
                "multiple interior cutout carrier types."
            ),
            "feature_coverage": {
                "pcb": [
                    "edge_cuts_lines",
                    "edge_cuts_arcs",
                    "edge_cuts_circles",
                    "edge_cuts_polygons",
                    "interior_board_cutouts",
                ],
                "pcb_a0": [
                    "profile_outline",
                    "profile_cutouts",
                    "contour_line_segments",
                    "contour_arc_segments",
                    "profile_centroid_with_cutouts",
                ],
                "board_svg": [
                    "board_outline_path",
                    "board_cutout_paths",
                    "silkscreen_labels",
                ],
                "renderer_3d": [
                    "dielectric_profile_cutouts",
                    "solder_mask_profile_cutouts",
                    "complex_board_outline",
                ],
            },
            "oracle_policy": {
                "board_svg": "smoke",
                "pcb_ir": "smoke",
                "pcb_data_models": "smoke",
                "pcb_viz_3d": "smoke",
            },
            "provenance": {
                "source_kind": "synthetic",
                "source_path": None,
                "license_usage": "test_fixture",
                "generator": "kicad_monkey/scripts/generate_kicad_synthetic_boards.py",
            },
            "notes": [
                f"Generated fixture case id: {case_id}.",
                "KiCad encodes profile cutouts as additional closed Edge.Cuts contours.",
                "The pcb_a0 importer normalizes the largest closed contour as profile.outline and the remaining contours as profile.cutouts.",
                "Regenerate instead of hand-editing the .kicad_pcb or metadata.",
            ],
        }
    return {
        "origin": "synthetic",
        "status": "active",
        "domains": base_domains,
        "tags": [
            "synthetic",
            "data_models",
            "viz_3d",
            "board_svg",
            "focused_feature_case",
            "pads",
            "vias",
        ],
        "test_intent": (
            "Exercise KiCad pad, hole, via, mask, paste, and via-treatment "
            "surfaces through data_models conversion, board SVG rendering, "
            "and 3D/viz rendering."
        ),
        "feature_coverage": {
            "pcb": [
                "smd_pads",
                "top_bottom_smd_pads",
                "rotated_pad_shapes",
                "through_hole_pads",
                "npth_holes",
                "slots",
                "custom_pad_primitives",
                "vias",
                "blind_buried_vias",
                "microvias",
                "mask_paste_openings",
            ],
            "pcb_a0": [
                "pad_shapes",
                "pad_rotation_matrix",
                "chamfered_pads",
                "custom_shape_regions",
                "padstack_layer_geometry",
                "holes",
                "via_spans",
                "via_treatment",
                "layer_stack",
            ],
            "board_svg": [
                "pad_shape_rendering",
                "drill_rendering",
                "silkscreen_labels",
                "mask_paste_expansion",
            ],
            "renderer_3d": [
                "pad_shape_rendering",
                "rotated_top_bottom_pads",
                "hole_openings",
                "bottom_side_pad_orientation",
                "mask_paste_openings",
                "layer_stack",
            ],
        },
        "oracle_policy": {
            "board_svg": "smoke",
            "pcb_ir": "smoke",
            "pcb_data_models": "smoke",
            "pcb_viz_3d": "smoke",
        },
        "provenance": {
            "source_kind": "synthetic",
            "source_path": None,
            "license_usage": "test_fixture",
            "generator": "kicad_monkey/scripts/generate_kicad_synthetic_boards.py",
        },
        "notes": [
            f"Generated fixture case id: {case_id}.",
            "SMD pad cases are generated on front and back copper at 0, 45, 90, 17, and 123 degrees.",
            "Through-hole and NPTH pad cases are generated at the same rotation matrix.",
            "Regenerate instead of hand-editing the .kicad_pcb or metadata.",
            "Designed for visual review in KiCad and automated converter/render smoke tests.",
        ],
    }


def _target_paths(
    corpus_root: Path,
    *,
    layout: str,
    case_id: str,
    spacing_mm: float,
    margin_mm: float,
    columns: int,
) -> tuple[Path, list[tuple[Path, str]]]:
    board_text = _render_board_for_case(case_id, spacing_mm=spacing_mm, margin_mm=margin_mm, columns=columns)
    project_text = _minimal_project_json(case_id)
    schematic_text = _minimal_schematic_text(case_id)
    metadata_text = json.dumps(_case_metadata(case_id, layout=layout), indent=2, sort_keys=True) + "\n"
    if layout == "board_svg":
        case_dir = corpus_root / "kicad" / "board_svg" / "input" / case_id
        board_path = case_dir / f"{case_id}.kicad_pcb"
        project_path = case_dir / f"{case_id}.kicad_pro"
        schematic_path = case_dir / f"{case_id}.kicad_sch"
        metadata_path = case_dir / "case_metadata.json"
        return board_path, [
            (board_path, board_text),
            (project_path, project_text),
            (schematic_path, schematic_text),
            (metadata_path, metadata_text),
        ]
    if layout == "project":
        input_dir = corpus_root / "kicad" / "projects" / case_id / "input"
        board_path = input_dir / f"{case_id}.kicad_pcb"
        project_path = input_dir / f"{case_id}.kicad_pro"
        schematic_path = input_dir / f"{case_id}.kicad_sch"
        metadata_path = corpus_root / "kicad" / "projects" / case_id / "case_metadata.json"
        return board_path, [
            (board_path, board_text),
            (project_path, project_text),
            (schematic_path, schematic_text),
            (metadata_path, metadata_text),
        ]
    raise ValueError(f"unsupported layout: {layout}")


def write_fixture(
    *,
    corpus_root: Path,
    layout: str,
    case_id: str,
    spacing_mm: float,
    margin_mm: float,
    columns: int,
    force: bool,
    dry_run: bool,
) -> list[Path]:
    primary_path, files = _target_paths(
        corpus_root,
        layout=layout,
        case_id=case_id,
        spacing_mm=spacing_mm,
        margin_mm=margin_mm,
        columns=columns,
    )
    if dry_run:
        return [path for path, _ in files]
    existing = [path for path, _ in files if path.exists()]
    if existing and not force:
        joined = "\n".join(str(path) for path in existing)
        raise FileExistsError(f"refusing to overwrite without --force:\n{joined}")
    for path, text in files:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")
    return [path for path, _ in files]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--corpus-root",
        type=Path,
        default=REPO_ROOT / "tests" / "corpus",
        help="Writable authoring root containing the kicad corpus folder.",
    )
    parser.add_argument(
        "--layout",
        choices=("board_svg", "project"),
        default="board_svg",
        help="Corpus layout to generate. board_svg is the manifest-friendly synthetic PCB bucket.",
    )
    parser.add_argument("--case-id", default=CASE_ID)
    parser.add_argument("--spacing-mm", type=float, default=14.0, help="Grid pitch for visual inspection.")
    parser.add_argument("--margin-mm", type=float, default=8.0, help="Board outline margin around the grid.")
    parser.add_argument("--columns", type=int, default=10)
    parser.add_argument("--force", action="store_true", help="Overwrite existing generated files.")
    parser.add_argument("--dry-run", action="store_true", help="Print target paths without writing files.")
    args = parser.parse_args(argv)

    paths = write_fixture(
        corpus_root=args.corpus_root,
        layout=args.layout,
        case_id=args.case_id,
        spacing_mm=args.spacing_mm,
        margin_mm=args.margin_mm,
        columns=args.columns,
        force=args.force,
        dry_run=args.dry_run,
    )

    action = "Would write" if args.dry_run else "Wrote"
    for path in paths:
        print(f"{action} {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
