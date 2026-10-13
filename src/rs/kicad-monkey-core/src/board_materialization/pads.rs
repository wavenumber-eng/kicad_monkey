//! Pad lands in the pad's local frame under one composed affine.
//!
//! The pad anchor is the footprint-local pad position rotated by the
//! footprint angle and translated to the footprint origin. The pad angle is
//! already a board-frame angle, so the land affine rotates by the pad angle
//! alone.

use super::emit::source_uid;
use super::emit::{Emitter, Origin};
use super::geometry::{length, nm, origin, pcb_point, rotate_mm, rotation_translation};
use super::graphics::rect_shapes;
use super::nets::net_key;
use super::routing::polyline_contour;
use super::source_graphics::outline;
use crate::pcb::{
    PcbFootprint, PcbHoleShape, PcbPad, PcbPadCustomPrimitive, PcbPadPrimitiveGeometry, PcbPoint,
    PcbPolygonPoint, PcbResolvedPadCopperLayer,
};
use kicad_monkey_contracts::generated::materialization_document::{
    Affine2D, Annulus2d, ArcSegment, BindingSourceKind, Capsule2d, ChamferedRect2d, Contour,
    CubicSegment, Disk2d, LineSegment, Nm, NmPoint, NonNegativeNm, PathCap, PathJoin, PathSegment,
    PlanarRegion2d, Rect2d, RectCorner, RoundedRect2d, Shape2d, SweptPath2d, Trapezoid2d,
};

/// A corner radius or chamfer, clamped in whole nanometres to half the
/// shorter side as the sides themselves round, so it is always valid.
fn corner_size(value_mm: f64, width_mm: f64, height_mm: f64) -> NonNegativeNm {
    let half = (*length(width_mm)).min(*length(height_mm)) / 2;
    NonNegativeNm((*length(value_mm)).clamp(0, half))
}

fn at(x_mm: f64, y_mm: f64) -> NmPoint {
    NmPoint([Nm(nm(x_mm)), Nm(nm(y_mm))])
}

/// Board-frame placement of a pad-local point.
pub(crate) struct PadFrame {
    pub anchor_x: f64,
    pub anchor_y: f64,
    pub angle: f64,
}

impl PadFrame {
    pub(crate) fn new(
        pad: &PcbPad,
        footprint_x: f64,
        footprint_y: f64,
        footprint_angle: f64,
    ) -> Self {
        let (dx, dy) = rotate_mm(pad.at_x, pad.at_y, -footprint_angle);
        Self {
            anchor_x: footprint_x + dx,
            anchor_y: footprint_y + dy,
            angle: -pad.angle,
        }
    }

    /// Affine placing pad-local geometry offset by (`offset_x`, `offset_y`).
    pub(crate) fn affine(&self, offset_x: f64, offset_y: f64) -> Affine2D {
        let (dx, dy) = rotate_mm(offset_x, offset_y, self.angle);
        rotation_translation(self.angle, self.anchor_x + dx, self.anchor_y + dy)
    }
}

/// Whether an unplated pad has no copper on this layer (`PAD::FlashLayer`):
/// a circle on a round drill, or an oval on an oblong drill, centred on the
/// hole and no larger than it. Every other NPTH pad keeps its copper.
/// The pad shape as KiCad loads it: a pad of any standard shape with
/// chamfered corners becomes a chamfered rectangle (`parsePAD`), which is
/// a chamfered round rectangle here.
pub(crate) fn as_loaded<'a>(
    resolved: &PcbResolvedPadCopperLayer<'a>,
) -> PcbResolvedPadCopperLayer<'a> {
    let mut loaded = *resolved;
    if loaded.shape != "custom" && !loaded.chamfer_corners.is_empty() {
        loaded.shape = "roundrect";
    }
    loaded
}

pub(crate) fn npth_without_copper(pad: &PcbPad, resolved: &PcbResolvedPadCopperLayer<'_>) -> bool {
    let resolved = &as_loaded(resolved);
    let Some(drill) = pad.drill.as_ref().filter(|_| pad.kind == "np_thru_hole") else {
        return false;
    };
    let centred = nm(resolved.offset.x) == 0 && nm(resolved.offset.y) == 0;
    let covers = |hole: f64, size: f64| nm(hole) >= nm(size);
    let (drill_x, drill_y) = (drill.width, drill.height.unwrap_or(drill.width));
    match (resolved.shape, drill.shape) {
        ("circle", PcbHoleShape::Round) => centred && covers(drill_x, resolved.size.x),
        ("oval", PcbHoleShape::Oval) => {
            centred && covers(drill_x, resolved.size.x) && covers(drill_y, resolved.size.y)
        }
        _ => false,
    }
}

/// Reference designator of `footprint`, or its library link when unset.
pub(crate) fn reference(footprint: &PcbFootprint) -> String {
    footprint
        .reference
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| footprint.library_link.clone())
}

/// A pad with its owning footprint, binding origin, and board placement.
pub(crate) struct PlacedPad<'a> {
    pub footprint: &'a PcbFootprint,
    pub origin: Origin,
    pub frame: PadFrame,
}

/// Place `pad`, or report it when its footprint is missing.
pub(crate) fn place_pad<'a>(
    pad: &PcbPad,
    footprints: &'a [PcbFootprint],
    emitter: &mut Emitter,
) -> Option<PlacedPad<'a>> {
    let uid = source_uid("pad", pad.uuid.as_deref(), pad.source_range.start);
    let Some(footprint) = footprints.get(pad.footprint_index) else {
        emitter.warn(
            "missing_footprint",
            format!("pad references missing footprint {}", pad.footprint_index),
            Some(uid),
        );
        return None;
    };
    let frame = PadFrame::new(
        pad,
        footprint.at_x.unwrap_or_default(),
        footprint.at_y.unwrap_or_default(),
        footprint.angle.unwrap_or_default(),
    );
    let origin = Origin {
        kind: BindingSourceKind::Pad,
        uid,
        refdes: Some(reference(footprint)),
        pad_number: Some(pad.number.clone()),
        net_key: net_key(&pad.net),
        die_length_nm: pad
            .die_length
            .filter(|length| *length > 0.0)
            .map(|length| i64::from(nm(length))),
        die_delay_ps: pad.die_delay.filter(|delay| *delay > 0.0),
        tied_net_keys: Vec::new(),
    };
    Some(PlacedPad {
        footprint,
        origin,
        frame,
    })
}

/// Shapes of one resolved pad layer in the pad frame: the pad shape (or a
/// custom pad's anchor) and any custom primitives.
#[derive(Default)]
pub(crate) struct PadShapes {
    pub base: Option<Shape2d>,
    pub primitives: Vec<Shape2d>,
}

/// Build the shapes of `resolved`, reporting anything not materialized.
pub(crate) fn pad_shapes(
    resolved: &PcbResolvedPadCopperLayer<'_>,
    origin: &Origin,
    slice: &str,
    emitter: &mut Emitter,
) -> PadShapes {
    let resolved = &as_loaded(resolved);
    if resolved.shape == "custom" {
        return custom_shapes(resolved, origin, emitter);
    }
    let base = standard_shape(resolved);
    if base.is_none() {
        emitter.warn(
            "unsupported_pad_shape",
            format!(
                "pad shape {} on {slice} is not materialized",
                resolved.shape
            ),
            Some(origin.uid.clone()),
        );
    }
    PadShapes {
        base,
        primitives: Vec::new(),
    }
}

/// Emit the copper land of `pad` on one resolved layer.
pub(crate) fn land(
    resolved: &PcbResolvedPadCopperLayer<'_>,
    frame: &PadFrame,
    origin: &Origin,
    slice: &str,
    emitter: &mut Emitter,
) {
    let affine = frame.affine(resolved.offset.x, resolved.offset.y);
    let shapes = pad_shapes(resolved, origin, slice, emitter);
    for shape in shapes.base.into_iter().chain(shapes.primitives) {
        emitter.copper(origin, slice, shape, affine.clone());
    }
}

fn standard_shape(resolved: &PcbResolvedPadCopperLayer<'_>) -> Option<Shape2d> {
    let (width, height) = (resolved.size.x, resolved.size.y);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let short = width.min(height);
    let chamfered = resolved.chamfer_ratio > 0.0 && !resolved.chamfer_corners.is_empty();
    match resolved.shape {
        "roundrect" if chamfered => Some(chamfered_rect(resolved, width, height)),
        "circle" => Some(disk(width / 2.0)),
        "oval" => Some(oval(width, height)),
        "rect" => Some(Shape2d::Rect2d(Rect2d {
            kind: "rect_2d".to_owned(),
            center_nm: origin(),
            width_nm: length(width),
            height_nm: length(height),
        })),
        "roundrect" => Some(Shape2d::RoundedRect2d(RoundedRect2d {
            kind: "rounded_rect_2d".to_owned(),
            center_nm: origin(),
            width_nm: length(width),
            height_nm: length(height),
            radius_nm: corner_size(short * resolved.roundrect_rratio, width, height),
        })),
        "trapezoid" => Some(Shape2d::Trapezoid2d(Trapezoid2d {
            kind: "trapezoid_2d".to_owned(),
            center_nm: origin(),
            width_nm: length(width),
            height_nm: length(height),
            delta_x_nm: Nm(nm(resolved.rect_delta.x)),
            delta_y_nm: Nm(nm(resolved.rect_delta.y)),
        })),
        _ => None,
    }
}

fn disk(radius_mm: f64) -> Shape2d {
    Shape2d::Disk2d(Disk2d {
        kind: "disk_2d".to_owned(),
        center_nm: origin(),
        radius_nm: length(radius_mm),
    })
}

/// Stadium along the longer axis.
pub(crate) fn oval(width: f64, height: f64) -> Shape2d {
    let (half_span, radius) = ((width - height).abs() / 2.0, width.min(height) / 2.0);
    let (start, end) = if width >= height {
        (at(-half_span, 0.0), at(half_span, 0.0))
    } else {
        (at(0.0, -half_span), at(0.0, half_span))
    };
    Shape2d::Capsule2d(Capsule2d {
        kind: "capsule_2d".to_owned(),
        start_nm: start,
        end_nm: end,
        radius_nm: length(radius),
    })
}

fn corner(name: &str) -> Option<RectCorner> {
    match name {
        "top_left" => Some(RectCorner::TopLeft),
        "top_right" => Some(RectCorner::TopRight),
        "bottom_left" => Some(RectCorner::BottomLeft),
        "bottom_right" => Some(RectCorner::BottomRight),
        _ => None,
    }
}

fn chamfered_rect(resolved: &PcbResolvedPadCopperLayer<'_>, width: f64, height: f64) -> Shape2d {
    let short = width.min(height);
    Shape2d::ChamferedRect2d(ChamferedRect2d {
        kind: "chamfered_rect_2d".to_owned(),
        center_nm: origin(),
        width_nm: length(width),
        height_nm: length(height),
        chamfer_nm: corner_size(short * resolved.chamfer_ratio, width, height),
        chamfered_corners: resolved
            .chamfer_corners
            .iter()
            .filter_map(|name| corner(name))
            .collect(),
        radius_nm: corner_size(short * resolved.roundrect_rratio, width, height),
    })
}

/// A custom pad is its anchor shape plus each primitive, all in the pad frame.
fn custom_shapes(
    resolved: &PcbResolvedPadCopperLayer<'_>,
    origin: &Origin,
    emitter: &mut Emitter,
) -> PadShapes {
    let (width, height) = (resolved.size.x, resolved.size.y);
    let base = (width > 0.0 && height > 0.0).then(|| {
        if resolved.anchor == "rect" {
            Shape2d::Rect2d(Rect2d {
                kind: "rect_2d".to_owned(),
                center_nm: self::origin(),
                width_nm: length(width),
                height_nm: length(height),
            })
        } else {
            // KiCad sizes a circular anchor by its X size.
            disk(width / 2.0)
        }
    });
    let mut primitives = Vec::new();
    for primitive in resolved.custom_primitives {
        match primitive_shapes(primitive) {
            shapes if !shapes.is_empty() => primitives.extend(shapes),
            _ => emitter.warn(
                "unsupported_custom_pad_primitive",
                format!(
                    "custom pad primitive {} is not materialized",
                    primitive.kind
                ),
                Some(origin.uid.clone()),
            ),
        }
    }
    PadShapes { base, primitives }
}

fn stroke(start: PcbPoint, segment: PathSegment, width: f64) -> Shape2d {
    Shape2d::SweptPath2d(SweptPath2d {
        kind: "swept_path_2d".to_owned(),
        start_nm: pcb_point(start),
        segments: vec![segment],
        width_nm: length(width),
        cap: PathCap::Round,
        join: PathJoin::Round,
    })
}

/// Fill and stroke of a custom-pad primitive as KiCad's `parsePCB_SHAPE`
/// resolves them: an explicit `fill` wins; otherwise rectangles and
/// circles are filled only without a stroke, and polygons always are. An
/// unfilled shape without a stroke gets KiCad's 0.10 mm default.
fn primitive_fill(primitive: &PcbPadCustomPrimitive) -> (bool, f64) {
    let width = primitive.width.unwrap_or(0.0).max(0.0);
    let zero = nm(width) == 0;
    let solid = match (primitive.fill.as_deref(), primitive.geometry.as_ref()) {
        (Some("none" | "no"), _) => false,
        (Some(_), _) => true,
        (None, Some(PcbPadPrimitiveGeometry::Polygon { .. })) => true,
        (
            None,
            Some(PcbPadPrimitiveGeometry::Rect { .. } | PcbPadPrimitiveGeometry::Circle { .. }),
        ) => zero,
        (None, _) => false,
    };
    let stroke = if zero && !solid { 0.1 } else { width };
    (solid, stroke)
}

/// Shapes of one custom-pad primitive in the pad frame. A filled shape is
/// grown by half its stroke; an unfilled one is its outline.
fn primitive_shapes(primitive: &PcbPadCustomPrimitive) -> Vec<Shape2d> {
    let (solid, width) = primitive_fill(primitive);
    let Some(geometry) = primitive.geometry.as_ref() else {
        return Vec::new();
    };
    match geometry {
        PcbPadPrimitiveGeometry::Line { start, end } => vec![stroke(*start, line_to(*end), width)],
        PcbPadPrimitiveGeometry::Arc { start, mid, end } => {
            vec![stroke(*start, arc_to(*mid, *end), width)]
        }
        PcbPadPrimitiveGeometry::Circle { center, end } => {
            circle(*center, *end, width, solid).into_iter().collect()
        }
        PcbPadPrimitiveGeometry::Rect { start, end, radius } => {
            let corners = [start.x, start.y, end.x, end.y].map(|value| i64::from(nm(value)));
            let radius = i64::from(nm(radius.unwrap_or(0.0)));
            rect_shapes(corners, radius, i64::from(nm(width)), solid)
                .into_iter()
                .collect()
        }
        PcbPadPrimitiveGeometry::Polygon { points } => {
            let Some(region) = polygon(points) else {
                return Vec::new();
            };
            let mut shapes: Vec<Shape2d> =
                outline(&region, i64::from(nm(width))).into_iter().collect();
            if solid {
                shapes.push(region);
            }
            shapes
        }
        PcbPadPrimitiveGeometry::Curve { points } => vec![bezier(*points, width)],
        // Bounding-box and vector proxies have no copper of their own; they
        // are reported as unsupported primitives by the caller.
        _ => Vec::new(),
    }
}

fn line_to(end: PcbPoint) -> PathSegment {
    PathSegment::LineSegment(LineSegment {
        kind: "line".to_owned(),
        end_nm: pcb_point(end),
    })
}

fn arc_to(mid: PcbPoint, end: PcbPoint) -> PathSegment {
    PathSegment::ArcSegment(ArcSegment {
        kind: "arc".to_owned(),
        mid_nm: pcb_point(mid),
        end_nm: pcb_point(end),
    })
}

fn circle(center: PcbPoint, end: PcbPoint, width: f64, filled: bool) -> Option<Shape2d> {
    let radius = (end.x - center.x).hypot(end.y - center.y);
    let outer = radius + width.max(0.0) / 2.0;
    if outer <= 0.0 {
        return None;
    }
    if filled || width <= 0.0 || radius <= width / 2.0 {
        return Some(Shape2d::Disk2d(Disk2d {
            kind: "disk_2d".to_owned(),
            center_nm: pcb_point(center),
            radius_nm: length(outer),
        }));
    }
    Some(Shape2d::Annulus2d(Annulus2d {
        kind: "annulus_2d".to_owned(),
        center_nm: pcb_point(center),
        outer_radius_nm: length(outer),
        inner_radius_nm: length(radius - width / 2.0),
    }))
}

/// Polygon with any authored arcs kept as arc segments.
pub(crate) fn polygon(points: &[PcbPolygonPoint]) -> Option<Shape2d> {
    let contour = if points
        .iter()
        .any(|point| matches!(point, PcbPolygonPoint::Arc { .. }))
    {
        arc_contour(points)?
    } else {
        let xy = points
            .iter()
            .filter_map(|point| match point {
                PcbPolygonPoint::Xy(value) => Some(pcb_point(*value)),
                PcbPolygonPoint::Arc { .. } => None,
            })
            .collect();
        polyline_contour(xy)?
    };
    Some(Shape2d::PlanarRegion2d(PlanarRegion2d {
        kind: "planar_region_2d".to_owned(),
        outer: contour,
        holes: Vec::new(),
    }))
}

fn arc_contour(points: &[PcbPolygonPoint]) -> Option<Contour> {
    let mut start: Option<NmPoint> = None;
    let mut segments = Vec::new();
    for point in points {
        match point {
            PcbPolygonPoint::Xy(value) if start.is_none() => start = Some(pcb_point(*value)),
            PcbPolygonPoint::Xy(value) => segments.push(line_to(*value)),
            PcbPolygonPoint::Arc {
                start: from,
                mid,
                end,
            } => {
                if start.is_none() {
                    start = Some(pcb_point(*from));
                } else {
                    segments.push(line_to(*from));
                }
                segments.push(arc_to(*mid, *end));
            }
        }
    }
    (!segments.is_empty()).then_some(Contour {
        start_nm: start?,
        segments,
    })
}

/// A cubic Bézier stroke as one swept path with a single cubic segment.
fn bezier(points: [PcbPoint; 4], width: f64) -> Shape2d {
    Shape2d::SweptPath2d(SweptPath2d {
        kind: "swept_path_2d".to_owned(),
        start_nm: pcb_point(points[0]),
        segments: vec![PathSegment::CubicSegment(CubicSegment {
            kind: "cubic".to_owned(),
            control1_nm: pcb_point(points[1]),
            control2_nm: pcb_point(points[2]),
            end_nm: pcb_point(points[3]),
        })],
        width_nm: length(width),
        cap: PathCap::Round,
        join: PathJoin::Round,
    })
}
