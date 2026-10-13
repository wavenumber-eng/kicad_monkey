//! Board and footprint graphic shapes straight from the parsed source.
//!
//! The board plot document applies preview pen rules to footprint graphics
//! (a 0.1524 mm default and a 0.0847 mm minimum pen). Physical geometry
//! instead follows KiCad's `parsePCB_SHAPE`: a filled shape may have no
//! stroke, an unfilled shape with no stroke gets 0.10 mm, and authored
//! widths are kept as written. Text is not handled here.
//!
//! Graphics on copper layers are copper; see [`copper_graphics`].

use super::emit::{Emitter, Origin, source_uid};
use super::geometry::{board_bounds, footprint_affine, identity, nm64};
use super::graphics::{Drawn, circle, line_to, point, rect_shapes, swept};
use super::net_ties::NetTies;
use super::nets::NetNames;
use super::pads::{polygon, reference};
use super::stack::CopperStack;
use crate::pcb::{PcbFootprint, PcbFootprintGraphic, PcbGraphic, PcbGraphicKind, PcbPoint};
use kicad_monkey_contracts::generated::materialization_document::{
    ArcSegment, BindingSourceKind, CubicSegment, Nm, NmBounds, PathSegment, Shape2d,
};

/// KiCad's `DEFAULT_LINE_WIDTH` for unfilled shapes authored without one.
const DEFAULT_LINE_WIDTH_NM: i64 = 100_000;

fn xy(value: PcbPoint) -> (i64, i64) {
    (nm64(value.x), nm64(value.y))
}

/// Concrete layers a graphic is drawn on.
pub(crate) fn graphic_layers(graphic: &PcbGraphic) -> Vec<&str> {
    match (&graphic.layers, &graphic.layer) {
        (Some(layers), _) if !layers.is_empty() => layers.iter().map(String::as_str).collect(),
        (_, Some(layer)) => vec![layer.as_str()],
        _ => Vec::new(),
    }
}

/// Fill and stroke of `graphic` on `layer`, as KiCad resolves them.
fn fill_and_stroke(graphic: &PcbGraphic, layer: &str, drawn: &mut Drawn) -> (bool, i64) {
    let solid = match graphic.effective_fill_for_layer(layer) {
        "none" | "no" => false,
        "hatch" | "reverse_hatch" | "cross_hatch" => {
            drawn.unsupported.push((
                "hatch_fill_not_materialized",
                "hatched fill is drawn as its outline only".to_owned(),
            ));
            false
        }
        _ => true,
    };
    let authored = nm64(graphic.stroke_width.unwrap_or(0.0)).max(0);
    let stroke = if authored == 0 && !solid {
        DEFAULT_LINE_WIDTH_NM
    } else {
        authored
    };
    if !matches!(
        graphic.stroke_kind.as_deref(),
        None | Some("default" | "solid")
    ) {
        drawn.unsupported.push((
            "stroke_pattern_drawn_solid",
            format!(
                "{} stroke is drawn solid",
                graphic.stroke_kind.as_deref().unwrap_or_default()
            ),
        ));
    }
    (solid, stroke)
}

/// A closed stroke along the outer contour of a polygon region.
pub(crate) fn outline(region: &Shape2d, stroke: i64) -> Option<Shape2d> {
    let Shape2d::PlanarRegion2d(region) = region else {
        return None;
    };
    let start = region.outer.start_nm.clone();
    let (x, y) = (i64::from(*start.0[0]), i64::from(*start.0[1]));
    let mut segments = region.outer.segments.clone();
    segments.push(line_to(x, y));
    (stroke > 0).then(|| swept(start, segments, stroke))
}

/// Open strokes: lines, arcs, and cubic curves.
fn stroke_shape(graphic: &PcbGraphic, stroke: i64) -> Option<Shape2d> {
    let segment = match graphic.kind {
        PcbGraphicKind::Line => {
            let (ex, ey) = xy(graphic.end?);
            line_to(ex, ey)
        }
        PcbGraphicKind::Arc => {
            let ((mx, my), (ex, ey)) = (xy(graphic.mid?), xy(graphic.end?));
            PathSegment::ArcSegment(ArcSegment {
                kind: "arc".to_owned(),
                mid_nm: point(mx, my),
                end_nm: point(ex, ey),
            })
        }
        PcbGraphicKind::Curve => {
            let [_, p1, p2, p3] = graphic.points[..] else {
                return None;
            };
            let ((x1, y1), (x2, y2), (x3, y3)) = (xy(p1), xy(p2), xy(p3));
            PathSegment::CubicSegment(CubicSegment {
                kind: "cubic".to_owned(),
                control1_nm: point(x1, y1),
                control2_nm: point(x2, y2),
                end_nm: point(x3, y3),
            })
        }
        _ => return None,
    };
    let start = match graphic.kind {
        PcbGraphicKind::Curve => graphic.points.first().copied()?,
        _ => graphic.start?,
    };
    let (sx, sy) = xy(start);
    Some(swept(point(sx, sy), vec![segment], stroke))
}

/// Closed shapes: circles, rectangles, and polygons, filled or outlined.
fn closed_shapes(graphic: &PcbGraphic, solid: bool, stroke: i64) -> Vec<Shape2d> {
    match graphic.kind {
        PcbGraphicKind::Circle => {
            let (Some(center), Some(end)) = (graphic.center, graphic.end) else {
                return Vec::new();
            };
            let ((cx, cy), (ex, ey)) = (xy(center), xy(end));
            let radius = ((ex - cx) as f64).hypot((ey - cy) as f64).round() as i64;
            circle(cx, cy, radius, stroke, solid).into_iter().collect()
        }
        PcbGraphicKind::Rect => {
            let (Some(start), Some(end)) = (graphic.start, graphic.end) else {
                return Vec::new();
            };
            let ((x1, y1), (x2, y2)) = (xy(start), xy(end));
            let radius = nm64(graphic.radius.unwrap_or(0.0));
            rect_shapes([x1, y1, x2, y2], radius, stroke, solid)
                .into_iter()
                .collect()
        }
        PcbGraphicKind::Poly => {
            let Some(region) = polygon(&graphic.polygon_points) else {
                return Vec::new();
            };
            let mut shapes: Vec<Shape2d> = outline(&region, stroke).into_iter().collect();
            if solid {
                shapes.push(region);
            }
            shapes
        }
        _ => Vec::new(),
    }
}

/// Shapes of `graphic` on `layer`, in the graphic's own frame.
pub(crate) fn graphic_shapes(graphic: &PcbGraphic, layer: &str, drawn: &mut Drawn) {
    let (solid, stroke) = fill_and_stroke(graphic, layer, drawn);
    drawn.shapes.extend(stroke_shape(graphic, stroke));
    drawn.shapes.extend(closed_shapes(graphic, solid, stroke));
}

/// Centreline bounds of the graphics drawn on `layer`, in their own frame:
/// the drawn shapes' bounds less half the stroke, as KiCad outlines a
/// courtyard. `None` when nothing is drawn there.
pub(crate) fn outline_bounds<'a>(
    graphics: impl Iterator<Item = &'a PcbGraphic>,
    layer: &str,
) -> Option<NmBounds> {
    let mut union: Option<[i64; 4]> = None;
    for graphic in graphics.filter(|graphic| graphic_layers(graphic).contains(&layer)) {
        let mut drawn = Drawn::default();
        let (_, stroke) = fill_and_stroke(graphic, layer, &mut drawn);
        graphic_shapes(graphic, layer, &mut drawn);
        let half = stroke / 2;
        for shape in &drawn.shapes {
            let b = board_bounds(shape, &identity());
            let shrunk = [
                i64::from(*b.min_x_nm) + half,
                i64::from(*b.min_y_nm) + half,
                i64::from(*b.max_x_nm) - half,
                i64::from(*b.max_y_nm) - half,
            ];
            union = Some(union.map_or(shrunk, |u| {
                [
                    u[0].min(shrunk[0]),
                    u[1].min(shrunk[1]),
                    u[2].max(shrunk[2]),
                    u[3].max(shrunk[3]),
                ]
            }));
        }
    }
    let [min_x, min_y, max_x, max_y] = union?;
    let clamp = |value: i64| Nm(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
    (min_x <= max_x && min_y <= max_y).then(|| NmBounds {
        min_x_nm: clamp(min_x),
        min_y_nm: clamp(min_y),
        max_x_nm: clamp(max_x),
        max_y_nm: clamp(max_y),
    })
}

/// Board and footprint graphics on copper layers, emitted as copper on the
/// graphic's authored net, as KiCad stores it (KiCad gives footprint
/// copper no net of its own; an unconnected shape has the empty net). A
/// footprint shape bridging net-tie pads lists the nets it may join.
pub(crate) fn copper_graphics(
    board: &[PcbGraphic],
    footprint: &[PcbFootprintGraphic],
    footprints: &[PcbFootprint],
    copper: &CopperStack,
    nets: &NetNames,
    ties: &NetTies,
    emitter: &mut Emitter,
) {
    let placed_board = board.iter().enumerate().map(|(position, graphic)| {
        let uid = source_uid("graphic", graphic.uuid.as_deref(), position);
        (
            graphic,
            BindingSourceKind::BoardGraphic,
            uid,
            None,
            identity(),
            None,
        )
    });
    let placed_footprint = footprint
        .iter()
        .enumerate()
        .filter_map(|(position, owned)| {
            let footprint = footprints.get(owned.footprint_index)?;
            let graphic = &owned.graphic;
            let uid = source_uid("footprint_graphic", graphic.uuid.as_deref(), position);
            let affine = footprint_affine(footprint);
            let kind = BindingSourceKind::FootprintGraphic;
            let index = Some(owned.footprint_index);
            Some((
                graphic,
                kind,
                uid,
                Some(reference(footprint)),
                affine,
                index,
            ))
        });
    for (graphic, kind, uid, refdes, affine, owner) in placed_board.chain(placed_footprint) {
        for layer in graphic_layers(graphic) {
            let Some(slice) = copper.key(layer) else {
                continue;
            };
            let net = nets.resolve(graphic.net.as_ref()).unwrap_or_else(|| {
                emitter.warn(
                    "graphic_net_unknown",
                    "copper graphic names a net the board does not declare; it has no net"
                        .to_owned(),
                    Some(uid.clone()),
                );
                String::new()
            });
            let mut origin = Origin::routed(kind, uid.clone(), net);
            origin.refdes.clone_from(&refdes);
            let mut drawn = Drawn::default();
            graphic_shapes(graphic, layer, &mut drawn);
            for (code, message) in drawn.unsupported {
                emitter.warn(code, message, Some(uid.clone()));
            }
            if let Some(index) = owner {
                let bounds: Vec<_> = drawn
                    .shapes
                    .iter()
                    .map(|shape| board_bounds(shape, &affine))
                    .collect();
                origin.tied_net_keys = ties.tied_nets(index, slice, &bounds);
            }
            for shape in drawn.shapes {
                emitter.copper(&origin, slice, shape, affine.clone());
            }
        }
    }
}
