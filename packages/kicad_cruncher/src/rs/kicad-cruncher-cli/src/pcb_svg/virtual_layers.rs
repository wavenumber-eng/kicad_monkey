//! Physical board-domain layers shared by future configured compositions.

use std::collections::BTreeMap;
use std::fmt::Write;

use kicad_monkey_core::{
    PcbFootprint, PcbGraphic, PcbGraphicKind, PcbHoleOwner, PcbHoleShape, PcbPoint,
    PcbPolygonPoint, PcbProfileOwner, PcbSetup, PcbVia, PcbView,
};
use kicad_monkey_svg::SvgViewport;

use super::preview::{ToonPreviewError, ToonStyleSettings};

const POINT_SCALE: f64 = 10_000.0;
const POINT_KEY_EPSILON: f64 = 1.0e-6;
const MIN_REGION_AREA_MM2: f64 = 1.0e-4;

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}

impl From<PcbPoint> for Point {
    fn from(value: PcbPoint) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Debug)]
struct Segment {
    points: Vec<Point>,
}

impl Segment {
    fn start_key(&self) -> (i64, i64) {
        point_key(self.points[0])
    }

    fn end_key(&self) -> (i64, i64) {
        point_key(*self.points.last().expect("segment has points"))
    }
}

#[derive(Debug)]
struct Region {
    points: Vec<Point>,
}

impl Region {
    fn area(&self) -> f64 {
        signed_area(&self.points).abs()
    }

    fn centroid(&self) -> Point {
        let (x, y) = self
            .points
            .iter()
            .fold((0.0, 0.0), |(x, y), point| (x + point.x, y + point.y));
        let count = self.points.len() as f64;
        Point {
            x: x / count,
            y: y / count,
        }
    }
}

pub(super) struct BoardMaterialLayers {
    pub substrate: String,
    pub soldermask_film: String,
    pub cutouts: String,
    pub drills: String,
    pub slots: String,
    pub outline: String,
}

pub(super) struct FootprintHoleLayers {
    pub drills: String,
    pub slots: String,
}

pub(super) fn footprint_hole_layers(
    view: &PcbView<'_>,
    footprints: &[PcbFootprint],
    footprint_index: usize,
    viewport: SvgViewport,
    mask_side: &str,
    style: &ToonStyleSettings,
) -> Result<FootprintHoleLayers, ToonPreviewError> {
    let holes = board_holes(
        view,
        footprints,
        viewport,
        mask_side,
        Some(footprint_index),
        style,
    )?;
    Ok(FootprintHoleLayers {
        drills: format!(
            "<g id=\"layer-DRILLS\" data-layer-token=\"DRILLS\" data-layer-origin=\"synthetic-footprint-bores\">{}</g>",
            holes.drills
        ),
        slots: format!(
            "<g id=\"layer-SLOTS\" data-layer-token=\"SLOTS\" data-layer-origin=\"synthetic-footprint-bores\">{}</g>",
            holes.slots
        ),
    })
}

pub(super) fn board_material_layers(
    view: &PcbView<'_>,
    footprints: &[PcbFootprint],
    viewport: SvgViewport,
    mask_side: &str,
    mask_openings: &str,
    soldermask_color: &str,
    style: &ToonStyleSettings,
) -> Result<BoardMaterialLayers, ToonPreviewError> {
    let regions = board_regions(view, footprints)?;
    let (outer_path, domain, cutout_paths) = board_domain_paths(&regions, viewport)?;
    let holes = board_holes(view, footprints, viewport, mask_side, None, style)?;
    let width = viewport.width_nm as f64 / 1_000_000.0;
    let height = viewport.height_nm as f64 / 1_000_000.0;
    let substrate = format!(
        concat!(
            "<g id=\"layer-BOARD_SUBSTRATE\" data-layer-token=\"BOARD_SUBSTRATE\" data-layer-origin=\"synthetic-board-substrate\">",
            "<defs><mask id=\"board-substrate-openings\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" style=\"mask-type:luminance\" color-interpolation=\"sRGB\">",
            "<path d=\"{}\" fill=\"white\" fill-rule=\"evenodd\"/>{}",
            "</mask></defs><path d=\"{}\" fill=\"{}\" fill-rule=\"evenodd\" opacity=\"{}\" stroke=\"none\" mask=\"url(#board-substrate-openings)\"/></g>"
        ),
        number(width),
        number(height),
        domain,
        holes.substrate,
        domain,
        style.substrate_color,
        number(style.substrate_opacity),
    );
    let (film_token, film_id, geometry_id) = soldermask_ids(mask_side)?;
    let soldermask_film = format!(
        concat!(
            "<g id=\"layer-{}\" data-layer-token=\"{}\" data-layer-origin=\"synthetic-soldermask-film\">",
            "<defs><g id=\"{}\">{}</g>",
            "<mask id=\"{}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" style=\"mask-type:luminance\" color-interpolation=\"sRGB\">",
            "<path d=\"{}\" fill=\"white\" fill-rule=\"evenodd\"/><use href=\"#{}\"/>",
            "</mask></defs>",
            "<path d=\"{}\" fill=\"{}\" fill-rule=\"evenodd\" opacity=\"{}\" stroke=\"none\" mask=\"url(#{})\"/></g>"
        ),
        film_token,
        film_token,
        geometry_id,
        mask_openings,
        film_id,
        number(width),
        number(height),
        domain,
        geometry_id,
        domain,
        soldermask_color,
        number(style.soldermask_opacity),
        film_id,
    );
    let cutouts = format!(
        concat!(
            "<g id=\"layer-BOARD_CUTOUTS\" data-layer-token=\"BOARD_CUTOUTS\" data-layer-origin=\"synthetic-board-cutouts\">",
            "<defs><pattern id=\"board-cutout-hatch\" patternUnits=\"userSpaceOnUse\" width=\"{}\" height=\"{}\" patternTransform=\"rotate({})\">",
            "<path d=\"M0 0L0 {}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-opacity=\"{}\"/></pattern></defs>",
            "<path d=\"{}\" fill=\"url(#board-cutout-hatch)\" fill-rule=\"evenodd\" stroke=\"{}\" stroke-width=\"{}\" stroke-opacity=\"{}\"/></g>"
        ),
        number(style.cutout_hatch_spacing_mm),
        number(style.cutout_hatch_spacing_mm),
        number(style.cutout_hatch_angle_deg),
        number(style.cutout_hatch_spacing_mm),
        style.cutout_hatch_color,
        number(style.cutout_hatch_line_width_mm),
        number(style.cutout_hatch_opacity),
        cutout_paths,
        style.cutout_outline_color,
        number(style.cutout_outline_width_mm),
        number(style.cutout_outline_opacity),
    );
    let drills = format!(
        concat!(
            "<g id=\"layer-DRILLS\" data-layer-token=\"DRILLS\" data-layer-origin=\"synthetic-physical-bores\" clip-path=\"url(#drills-board-domain)\">",
            "<defs><clipPath id=\"drills-board-domain\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\" clip-rule=\"evenodd\"/></clipPath></defs>{}</g>"
        ),
        domain, holes.drills,
    );
    let slots = format!(
        concat!(
            "<g id=\"layer-SLOTS\" data-layer-token=\"SLOTS\" data-layer-origin=\"synthetic-physical-bores\" clip-path=\"url(#slots-board-domain)\">",
            "<defs><clipPath id=\"slots-board-domain\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\" clip-rule=\"evenodd\"/></clipPath></defs>{}</g>"
        ),
        domain, holes.slots,
    );
    let outline = format!(
        "<g id=\"layer-BOARD_OUTLINE\" data-layer-token=\"BOARD_OUTLINE\" data-layer-origin=\"synthetic-board-outline\"><path d=\"{outer_path}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/></g>",
        style.board_outline_color,
        number(style.board_outline_width_mm),
    );
    Ok(BoardMaterialLayers {
        substrate,
        soldermask_film,
        cutouts,
        drills,
        slots,
        outline,
    })
}

fn board_domain_paths(
    regions: &[Region],
    viewport: SvgViewport,
) -> Result<(String, String, String), ToonPreviewError> {
    let Some((outer_index, _)) = regions
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.area().total_cmp(&right.area()))
    else {
        return Err(ToonPreviewError::new(
            "BOARD_SUBSTRATE requires a closed Edge.Cuts profile",
        ));
    };
    let outer = &regions[outer_index];
    let outer_path = region_path(outer, viewport);
    let mut domain = outer_path.clone();
    let mut cutout_paths = String::new();
    for (index, region) in regions.iter().enumerate() {
        if index != outer_index
            && region.area() < outer.area() * 0.95
            && point_in_polygon(region.centroid(), &outer.points)
        {
            let path = region_path(region, viewport);
            domain.push_str(&path);
            cutout_paths.push_str(&path);
        }
    }
    Ok((outer_path, domain, cutout_paths))
}

fn soldermask_ids(
    mask_side: &str,
) -> Result<(&'static str, &'static str, &'static str), ToonPreviewError> {
    match mask_side {
        "F" => Ok((
            "SOLDERMASK_FILM_TOP",
            "soldermask-film-openings-top",
            "soldermask-opening-geometry-top",
        )),
        "B" => Ok((
            "SOLDERMASK_FILM_BOTTOM",
            "soldermask-film-openings-bottom",
            "soldermask-opening-geometry-bottom",
        )),
        _ => Err(ToonPreviewError::new(format!(
            "unsupported solder-mask film side {mask_side:?}"
        ))),
    }
}

fn board_regions(
    view: &PcbView<'_>,
    footprints: &[PcbFootprint],
) -> Result<Vec<Region>, ToonPreviewError> {
    let mut segments = Vec::new();
    let mut closed = Vec::new();
    for profile in view.profile_primitives() {
        let profile = profile.map_err(ToonPreviewError::from_display)?;
        let transform = |point: PcbPoint| match profile.owner {
            PcbProfileOwner::Board => Point::from(point),
            PcbProfileOwner::Footprint { footprint_index } => footprints
                .get(footprint_index)
                .map_or(Point::from(point), |footprint| {
                    footprint_point(Point::from(point), footprint)
                }),
        };
        append_graphic(&profile.graphic, &transform, &mut segments, &mut closed);
    }
    closed.extend(assemble_regions(&segments));
    closed.retain(|region| region.area() > MIN_REGION_AREA_MM2);
    Ok(closed)
}

fn append_graphic(
    graphic: &PcbGraphic,
    transform: &impl Fn(PcbPoint) -> Point,
    segments: &mut Vec<Segment>,
    closed: &mut Vec<Region>,
) {
    match graphic.kind {
        PcbGraphicKind::Line => {
            if let (Some(start), Some(end)) = (graphic.start, graphic.end) {
                segments.push(Segment {
                    points: vec![transform(start), transform(end)],
                });
            }
        }
        PcbGraphicKind::Arc => {
            if let (Some(start), Some(mid), Some(end)) = (graphic.start, graphic.mid, graphic.end) {
                segments.push(Segment {
                    points: sample_arc(transform(start), transform(mid), transform(end)),
                });
            }
        }
        PcbGraphicKind::Curve => {
            if graphic.points.len() == 4 {
                segments.push(Segment {
                    points: sample_curve(graphic.points.iter().copied().map(transform).collect()),
                });
            }
        }
        PcbGraphicKind::Rect => {
            if let (Some(start), Some(end)) = (graphic.start, graphic.end) {
                let corners = [
                    start,
                    PcbPoint {
                        x: end.x,
                        y: start.y,
                    },
                    end,
                    PcbPoint {
                        x: start.x,
                        y: end.y,
                    },
                ];
                closed.push(Region {
                    points: corners.into_iter().map(transform).collect(),
                });
            }
        }
        PcbGraphicKind::Circle => {
            if let (Some(center), Some(end)) = (graphic.center.or(graphic.start), graphic.end) {
                let center = transform(center);
                let end = transform(end);
                let radius = distance(center, end);
                if radius > 0.0 {
                    let count = sample_count(2.0 * std::f64::consts::PI * radius, 1.0, 64);
                    closed.push(Region {
                        points: (0..count)
                            .map(|index| {
                                let angle =
                                    2.0 * std::f64::consts::PI * index as f64 / count as f64;
                                Point {
                                    x: center.x + angle.cos() * radius,
                                    y: center.y + angle.sin() * radius,
                                }
                            })
                            .collect(),
                    });
                }
            }
        }
        PcbGraphicKind::Poly => {
            let points = polygon_points(&graphic.polygon_points, transform);
            if points.len() >= 3 {
                closed.push(Region { points });
            }
        }
        _ => {}
    }
}

fn polygon_points(
    authored: &[PcbPolygonPoint],
    transform: &impl Fn(PcbPoint) -> Point,
) -> Vec<Point> {
    let mut result = Vec::new();
    for point in authored {
        match point {
            PcbPolygonPoint::Xy(point) => result.push(transform(*point)),
            PcbPolygonPoint::Arc { start, mid, end } => {
                let arc = sample_arc(transform(*start), transform(*mid), transform(*end));
                if result
                    .last()
                    .is_some_and(|last| point_key(*last) == point_key(arc[0]))
                {
                    result.extend_from_slice(&arc[1..]);
                } else {
                    result.extend(arc);
                }
            }
        }
    }
    result
}

fn assemble_regions(segments: &[Segment]) -> Vec<Region> {
    let mut adjacency = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for (index, segment) in segments.iter().enumerate() {
        adjacency
            .entry(segment.start_key())
            .or_default()
            .push(index);
        adjacency.entry(segment.end_key()).or_default().push(index);
    }
    let mut visited = vec![false; segments.len()];
    let mut result = Vec::new();
    for start_index in 0..segments.len() {
        if visited[start_index] {
            continue;
        }
        let start = &segments[start_index];
        let start_key = start.start_key();
        let mut points = start.points.clone();
        let mut current = start.end_key();
        visited[start_index] = true;
        while current != start_key {
            let Some(next_index) = adjacency
                .get(&current)
                .and_then(|values| values.iter().copied().find(|index| !visited[*index]))
            else {
                break;
            };
            let next = &segments[next_index];
            let (oriented, end) = if next.start_key() == current {
                (next.points.clone(), next.end_key())
            } else {
                let mut reversed = next.points.clone();
                reversed.reverse();
                (reversed, next.start_key())
            };
            points.extend_from_slice(&oriented[1..]);
            current = end;
            visited[next_index] = true;
        }
        if current == start_key && points.len() >= 3 {
            if point_key(*points.last().expect("loop has points")) == start_key {
                points.pop();
            }
            result.push(Region { points });
        }
    }
    result
}

fn sample_arc(start: Point, mid: Point, end: Point) -> Vec<Point> {
    let Some((center, radius)) = circle_from_three_points(start, mid, end) else {
        return vec![start, mid, end];
    };
    let start_angle = (start.y - center.y).atan2(start.x - center.x);
    let mid_angle = (mid.y - center.y).atan2(mid.x - center.x);
    let end_angle = (end.y - center.y).atan2(end.x - center.x);
    let ccw = positive_angle_delta(start_angle, end_angle);
    let mid_delta = positive_angle_delta(start_angle, mid_angle);
    let sweep = if mid_delta <= ccw {
        ccw
    } else {
        -(2.0 * std::f64::consts::PI - ccw)
    };
    let count = sample_count(sweep.abs() * radius, 1.0, 7);
    (0..count)
        .map(|index| {
            let angle = start_angle + sweep * index as f64 / (count - 1) as f64;
            Point {
                x: center.x + angle.cos() * radius,
                y: center.y + angle.sin() * radius,
            }
        })
        .collect()
}

fn sample_curve(points: Vec<Point>) -> Vec<Point> {
    let [p0, p1, p2, p3] = points.as_slice() else {
        return points;
    };
    let count = sample_count(
        distance(*p0, *p1) + distance(*p1, *p2) + distance(*p2, *p3),
        0.5,
        9,
    );
    (0..count)
        .map(|index| {
            let t = index as f64 / (count - 1) as f64;
            let opposite = 1.0 - t;
            Point {
                x: opposite.powi(3) * p0.x
                    + 3.0 * opposite.powi(2) * t * p1.x
                    + 3.0 * opposite * t.powi(2) * p2.x
                    + t.powi(3) * p3.x,
                y: opposite.powi(3) * p0.y
                    + 3.0 * opposite.powi(2) * t * p1.y
                    + 3.0 * opposite * t.powi(2) * p2.y
                    + t.powi(3) * p3.y,
            }
        })
        .collect()
}

fn sample_count(length: f64, max_segment: f64, minimum: usize) -> usize {
    minimum
        .max((length / max_segment.max(1.0e-6)).ceil() as usize + 1)
        .min(2049)
}

fn circle_from_three_points(first: Point, second: Point, third: Point) -> Option<(Point, f64)> {
    let determinant = 2.0
        * (first.x * (second.y - third.y)
            + second.x * (third.y - first.y)
            + third.x * (first.y - second.y));
    if determinant.abs() < 1.0e-9 {
        return None;
    }
    let first_norm = first.x * first.x + first.y * first.y;
    let second_norm = second.x * second.x + second.y * second.y;
    let third_norm = third.x * third.x + third.y * third.y;
    let center = Point {
        x: (first_norm * (second.y - third.y)
            + second_norm * (third.y - first.y)
            + third_norm * (first.y - second.y))
            / determinant,
        y: (first_norm * (third.x - second.x)
            + second_norm * (first.x - third.x)
            + third_norm * (second.x - first.x))
            / determinant,
    };
    Some((center, distance(first, center)))
}

struct BoardHoleLayers {
    substrate: String,
    drills: String,
    slots: String,
}

#[allow(
    clippy::too_many_lines,
    reason = "hole classification keeps pad/via tenting, plating, slots, and clipping in one source-order pass"
)]
fn board_holes(
    view: &PcbView<'_>,
    footprints: &[PcbFootprint],
    viewport: SvgViewport,
    mask_side: &str,
    footprint_filter: Option<usize>,
    style: &ToonStyleSettings,
) -> Result<BoardHoleLayers, ToonPreviewError> {
    let setup = view.setup().map_err(ToonPreviewError::from_display)?;
    let vias = view
        .vias()
        .collect::<Result<Vec<_>, _>>()
        .map_err(ToonPreviewError::from_display)?;
    let mut substrate = String::new();
    let mut drills = String::new();
    let mut slots = String::new();
    for hole in view.holes() {
        let hole = hole.map_err(ToonPreviewError::from_display)?;
        if let Some(footprint_index) = footprint_filter
            && (hole.owner != PcbHoleOwner::Pad || hole.footprint_index != Some(footprint_index))
        {
            continue;
        }
        let (center, angle) = if hole.owner == PcbHoleOwner::Pad {
            let Some(footprint) = hole.footprint_index.and_then(|index| footprints.get(index))
            else {
                continue;
            };
            let local_offset = rotate(Point::from(hole.offset), -hole.angle.to_radians());
            let local = Point {
                x: hole.center.x + local_offset.x,
                y: hole.center.y + local_offset.y,
            };
            (
                footprint_point(local, footprint),
                -(hole.angle + footprint.angle.unwrap_or(0.0)),
            )
        } else {
            (Point::from(hole.center), -hole.angle)
        };
        let center = canvas_point(center, viewport);
        let round = hole.shape == PcbHoleShape::Round || (hole.width - hole.height).abs() < 1.0e-9;
        let respect_tenting = if round {
            style.drill_respect_tenting
        } else {
            style.slot_respect_tenting
        };
        let show_visible = hole.owner != PcbHoleOwner::Via
            || !respect_tenting
            || vias
                .get(hole.owner_index)
                .is_some_and(|via| via_hole_is_visible_on_side(via, setup.as_ref(), mask_side));
        let visible = if round { &mut drills } else { &mut slots };
        let owner = if hole.owner == PcbHoleOwner::Pad {
            "pad"
        } else {
            "via"
        };
        let (color, opacity, outline, outline_width_mm) = if round {
            (
                if hole.plated {
                    style.plated_drill_color.as_str()
                } else {
                    style.non_plated_drill_color.as_str()
                },
                style.drill_opacity,
                style.drill_outline,
                style.drill_outline_width_mm,
            )
        } else {
            (
                if hole.plated {
                    style.plated_slot_color.as_str()
                } else {
                    style.non_plated_slot_color.as_str()
                },
                style.slot_opacity,
                style.slot_outline,
                style.slot_outline_width_mm,
            )
        };
        let (fill, stroke, stroke_width) = if outline {
            ("none", color, number(outline_width_mm))
        } else {
            (color, "none", "0".to_owned())
        };
        if round {
            write!(
                substrate,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"black\"/>",
                number(center.x),
                number(center.y),
                number(hole.width / 2.0)
            )
            .expect("writing to String cannot fail");
            if show_visible {
                write!(
                    visible,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" opacity=\"{}\" data-hole-owner=\"{}\" data-plated=\"{}\"/>",
                    number(center.x),
                    number(center.y),
                    number(hole.width / 2.0),
                    fill,
                    stroke,
                    stroke_width,
                    number(opacity),
                    owner,
                    hole.plated,
                )
                .expect("writing to String cannot fail");
            }
        } else {
            write!(
                substrate,
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({} {} {})\" fill=\"black\"/>",
                number(center.x),
                number(center.y),
                number(hole.width / 2.0),
                number(hole.height / 2.0),
                number(angle),
                number(center.x),
                number(center.y),
            )
            .expect("writing to String cannot fail");
            if show_visible {
                write!(
                    visible,
                    "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({} {} {})\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" opacity=\"{}\" data-hole-owner=\"{}\" data-plated=\"{}\"/>",
                    number(center.x),
                    number(center.y),
                    number(hole.width / 2.0),
                    number(hole.height / 2.0),
                    number(angle),
                    number(center.x),
                    number(center.y),
                    fill,
                    stroke,
                    stroke_width,
                    number(opacity),
                    owner,
                    hole.plated,
                )
                .expect("writing to String cannot fail");
            }
        }
    }
    Ok(BoardHoleLayers {
        substrate,
        drills,
        slots,
    })
}

fn via_hole_is_visible_on_side(via: &PcbVia, setup: Option<&PcbSetup>, mask_side: &str) -> bool {
    let (outer_copper, explicit_tenting, setup_tenting) = match mask_side {
        "F" => (
            "F.Cu",
            via.tenting.as_ref().and_then(|value| value.front),
            setup.is_some_and(|value| value.tenting_front),
        ),
        "B" => (
            "B.Cu",
            via.tenting.as_ref().and_then(|value| value.back),
            setup.is_some_and(|value| value.tenting_back),
        ),
        _ => return false,
    };
    let reaches_surface = via
        .layers
        .iter()
        .any(|layer| layer == outer_copper || layer == "*.Cu");
    let tented = explicit_tenting.unwrap_or(setup_tenting);
    let filled = via
        .filling
        .unwrap_or_else(|| setup.is_some_and(|value| value.filling));
    let capped = via
        .capping
        .unwrap_or_else(|| setup.is_some_and(|value| value.capping));
    reaches_surface && !tented && !filled && !capped
}

fn region_path(region: &Region, viewport: SvgViewport) -> String {
    let mut result = String::new();
    for (index, point) in region.points.iter().enumerate() {
        let point = canvas_point(*point, viewport);
        write!(
            result,
            "{}{} {}",
            if index == 0 { 'M' } else { 'L' },
            number(point.x),
            number(point.y)
        )
        .expect("writing to String cannot fail");
    }
    result.push('Z');
    result
}

fn footprint_point(point: Point, footprint: &PcbFootprint) -> Point {
    let rotated = rotate(point, -footprint.angle.unwrap_or(0.0).to_radians());
    Point {
        x: footprint.at_x.unwrap_or(0.0) + rotated.x,
        y: footprint.at_y.unwrap_or(0.0) + rotated.y,
    }
}

fn canvas_point(point: Point, viewport: SvgViewport) -> Point {
    Point {
        x: point.x - viewport.min_x_nm as f64 / 1_000_000.0,
        y: point.y - viewport.min_y_nm as f64 / 1_000_000.0,
    }
}

fn rotate(point: Point, angle: f64) -> Point {
    let (sin, cos) = angle.sin_cos();
    Point {
        x: point.x * cos - point.y * sin,
        y: point.x * sin + point.y * cos,
    }
}

fn point_key(point: Point) -> (i64, i64) {
    (quantized_coord(point.x), quantized_coord(point.y))
}

fn quantized_coord(value: f64) -> i64 {
    let scaled = value * POINT_SCALE;
    if scaled >= 0.0 {
        (scaled + 0.5 + POINT_KEY_EPSILON).floor() as i64
    } else {
        (scaled - 0.5 - POINT_KEY_EPSILON).ceil() as i64
    }
}

fn signed_area(points: &[Point]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(left, right)| left.x * right.y - right.x * left.y)
        .sum::<f64>()
        / 2.0
}

fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for (index, current) in polygon.iter().enumerate() {
        let prior = polygon[previous];
        if (current.y > point.y) != (prior.y > point.y)
            && point.x
                < (prior.x - current.x) * (point.y - current.y) / (prior.y - current.y) + current.x
        {
            inside = !inside;
        }
        previous = index;
    }
    inside
}

fn positive_angle_delta(start: f64, end: f64) -> f64 {
    (end - start).rem_euclid(2.0 * std::f64::consts::PI)
}

fn distance(first: Point, second: Point) -> f64 {
    (first.x - second.x).hypot(first.y - second.y)
}

fn number(value: f64) -> String {
    if value.abs() < 0.000_000_5 {
        "0".to_owned()
    } else {
        format!("{value:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kicad_monkey_core::PcbLimits;
    use std::{fs, path::PathBuf};

    #[test]
    fn material_layers_use_profile_cutouts_and_physical_bores() {
        let source = r#"(kicad_pcb
          (gr_rect (start 1 1) (end 21 11) (stroke (width 0.1) (type solid))
            (fill none) (layer "Edge.Cuts"))
          (gr_circle (center 5 5) (end 6 5) (stroke (width 0.1) (type solid))
            (fill none) (layer "Edge.Cuts"))
          (footprint "Hole" (layer "F.Cu") (at 10 5)
            (pad "" np_thru_hole oval (at 0 0 90) (size 2 3)
              (drill oval 1 2) (layers "*.Cu" "*.Mask"))
            (pad "1" thru_hole circle (at 3 0) (size 2 2)
              (drill 1) (layers "*.Cu" "*.Mask"))))"#;
        let view = PcbView::parse(source, PcbLimits::default()).unwrap();
        let footprints = view.footprints().collect::<Result<Vec<_>, _>>().unwrap();
        let layers = board_material_layers(
            &view,
            &footprints,
            SvgViewport {
                min_x_nm: 0,
                min_y_nm: 0,
                width_nm: 25_000_000,
                height_nm: 15_000_000,
            },
            "F",
            "<g transform=\"translate(0 0)\"><rect width=\"2\" height=\"1\" fill=\"black\"/></g>",
            "#EEEEEE",
            &ToonStyleSettings::default(),
        )
        .unwrap();
        assert_top_material_layers(&layers);

        let bottom = board_material_layers(
            &view,
            &footprints,
            SvgViewport {
                min_x_nm: 0,
                min_y_nm: 0,
                width_nm: 25_000_000,
                height_nm: 15_000_000,
            },
            "B",
            "",
            "#EEEEEE",
            &ToonStyleSettings::default(),
        )
        .unwrap();
        assert!(
            bottom
                .soldermask_film
                .contains("data-layer-token=\"SOLDERMASK_FILM_BOTTOM\"")
        );
    }

    #[test]
    fn taillight_j1_holes_are_colored_objects_clipped_only_to_the_board_domain() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let board_path = package_root
            .join("tests/corpus/kicad/projects/taillight/input/11-10045__taillight__C.kicad_pcb");
        let source = fs::read_to_string(board_path).expect("read governed Taillight board");
        let view = PcbView::parse(&source, PcbLimits::default()).expect("parse Taillight board");
        let footprints = view
            .footprints()
            .collect::<Result<Vec<_>, _>>()
            .expect("parse Taillight footprints");
        let layers = board_material_layers(
            &view,
            &footprints,
            SvgViewport {
                min_x_nm: 100_000_000,
                min_y_nm: 84_000_000,
                width_nm: 110_000_000,
                height_nm: 30_000_000,
            },
            "F",
            "",
            "#EEEEEE",
            &ToonStyleSettings::default(),
        )
        .expect("compose Taillight material layers");

        assert_eq!(layers.drills.matches("data-hole-owner=\"pad\"").count(), 8);
        assert_eq!(layers.drills.matches("data-hole-owner=\"via\"").count(), 0);
        assert_eq!(layers.drills.matches("r=\"0.4445\"").count(), 4);
        assert_eq!(layers.drills.matches("r=\"1.25\"").count(), 4);
        assert!(
            layers
                .drills
                .contains("clip-path=\"url(#drills-board-domain)\"")
        );
        assert!(!layers.drills.contains("soldermask-exposed-openings"));
    }

    #[test]
    fn via_drill_objects_respect_resolved_tenting_fill_and_surface_span() {
        let source = r#"(kicad_pcb
          (setup (tenting (front yes) (back yes)))
          (gr_rect (start 0 0) (end 20 10) (stroke (width 0.1) (type solid))
            (fill none) (layer "Edge.Cuts"))
          (via (at 2 2) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu"))
          (via (at 4 2) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu")
            (tenting (front no) (back yes)))
          (via buried (at 6 2) (size 1) (drill 0.5) (layers "In1.Cu" "In2.Cu")
            (tenting (front no) (back no)))
          (via (at 8 2) (size 1) (drill 0.5) (layers "F.Cu" "B.Cu")
            (tenting (front no) (back no)) (filling yes)))"#;
        let view = PcbView::parse(source, PcbLimits::default()).unwrap();
        let footprints = view.footprints().collect::<Result<Vec<_>, _>>().unwrap();
        let render = |side| {
            board_material_layers(
                &view,
                &footprints,
                SvgViewport {
                    min_x_nm: 0,
                    min_y_nm: 0,
                    width_nm: 20_000_000,
                    height_nm: 10_000_000,
                },
                side,
                "",
                "#EEEEEE",
                &ToonStyleSettings::default(),
            )
            .unwrap()
        };
        let top = render("F");
        let bottom = render("B");

        assert_eq!(top.substrate.matches("<circle ").count(), 4);
        assert_eq!(top.drills.matches("data-hole-owner=\"via\"").count(), 1);
        assert_eq!(bottom.drills.matches("data-hole-owner=\"via\"").count(), 0);
    }

    fn assert_top_material_layers(layers: &BoardMaterialLayers) {
        assert_substrate_and_mask(layers);
        assert_cutouts_and_holes(layers);
        assert_outline(layers);
    }

    fn assert_substrate_and_mask(layers: &BoardMaterialLayers) {
        assert!(
            layers
                .substrate
                .contains("data-layer-token=\"BOARD_SUBSTRATE\"")
        );
        assert!(layers.substrate.contains("<ellipse "));
        assert!(layers.substrate.contains("M1 1L21 1L21 11L1 11Z"));
        assert!(
            layers
                .soldermask_film
                .contains("data-layer-token=\"SOLDERMASK_FILM_TOP\"")
        );
        assert!(
            layers
                .soldermask_film
                .contains("<rect width=\"2\" height=\"1\" fill=\"black\"/>")
        );
        assert!(layers.soldermask_film.contains("opacity=\"0.75\""));
        assert!(layers.soldermask_film.contains("fill=\"#EEEEEE\""));
    }

    fn assert_cutouts_and_holes(layers: &BoardMaterialLayers) {
        assert!(
            layers
                .cutouts
                .contains("data-layer-token=\"BOARD_CUTOUTS\"")
        );
        assert!(layers.cutouts.contains("board-cutout-hatch"));
        assert!(layers.drills.contains("data-layer-token=\"DRILLS\""));
        assert!(layers.drills.contains("<circle "));
        assert!(layers.slots.contains("data-layer-token=\"SLOTS\""));
        assert!(layers.slots.contains("<ellipse "));
        assert!(layers.drills.contains("fill=\"#D3D3D3\""));
        assert!(
            layers
                .drills
                .contains("clip-path=\"url(#drills-board-domain)\"")
        );
        assert!(layers.drills.contains("clip-rule=\"evenodd\""));
        assert!(
            layers
                .slots
                .contains("clip-path=\"url(#slots-board-domain)\"")
        );
        assert!(!layers.drills.contains("soldermask-exposed-openings"));
        assert!(!layers.slots.contains("soldermask-exposed-openings"));
    }

    fn assert_outline(layers: &BoardMaterialLayers) {
        assert!(
            layers
                .outline
                .contains("data-layer-token=\"BOARD_OUTLINE\"")
        );
        assert!(layers.outline.contains("stroke-width=\"0.25\""));
    }

    #[test]
    fn profile_stitching_tolerates_arc_sampling_float_noise() {
        let segments = vec![
            Segment {
                points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 }],
            },
            Segment {
                points: vec![
                    Point {
                        x: 9.999_999_999_99,
                        y: 0.0,
                    },
                    Point { x: 10.0, y: 10.0 },
                ],
            },
            Segment {
                points: vec![Point { x: 10.0, y: 10.0 }, Point { x: 0.0, y: 10.0 }],
            },
            Segment {
                points: vec![Point { x: 0.0, y: 10.0 }, Point { x: 0.0, y: 0.0 }],
            },
        ];

        let regions = assemble_regions(&segments);
        assert_eq!(regions.len(), 1);
        assert!((regions[0].area() - 100.0).abs() < 0.01);
    }

    #[test]
    fn yoshi_style_arc_line_profile_is_the_outer_board() {
        let source = r#"(kicad_pcb
          (gr_line (start 154.7965 134.499999) (end 154.7965 141.643799) (layer "Edge.Cuts"))
          (gr_arc (start 152.12955 131.832999) (mid 154.015386 132.614128) (end 154.7965 134.499999) (layer "Edge.Cuts"))
          (gr_arc (start 143.39815 123.101749) (mid 145.289873 123.888669) (end 146.06525 125.78513) (layer "Edge.Cuts"))
          (gr_arc (start 132 127.356199) (mid 133.2461 124.347849) (end 136.25445 123.101749) (layer "Edge.Cuts"))
          (gr_line (start 136.25445 123.101749) (end 143.39815 123.101749) (layer "Edge.Cuts"))
          (gr_line (start 136.25445 137.166999) (end 134.66695 137.166999) (layer "Edge.Cuts"))
          (gr_line (start 150.54205 131.832999) (end 152.12955 131.832999) (layer "Edge.Cuts"))
          (gr_line (start 140.73125 143.215033) (end 140.73125 141.643799) (layer "Edge.Cuts"))
          (gr_line (start 146.06525 125.78513) (end 146.06525 127.356199) (layer "Edge.Cuts"))
          (gr_arc (start 134.66695 137.166999) (mid 132.7811 136.385864) (end 132 134.499999) (layer "Edge.Cuts"))
          (gr_line (start 150.54205 145.898249) (end 143.39825 145.898249) (layer "Edge.Cuts"))
          (gr_arc (start 154.7965 141.643799) (mid 153.550398 144.652132) (end 150.54205 145.898249) (layer "Edge.Cuts"))
          (gr_arc (start 136.25445 137.166999) (mid 139.42004 138.478209) (end 140.73125 141.643799) (layer "Edge.Cuts"))
          (gr_arc (start 150.54205 131.832999) (mid 147.37646 130.521789) (end 146.06525 127.356199) (layer "Edge.Cuts"))
          (gr_arc (start 143.39825 145.898249) (mid 141.506651 145.111352) (end 140.73125 143.215033) (layer "Edge.Cuts"))
          (gr_line (start 132 134.499999) (end 132 127.356199) (layer "Edge.Cuts")))"#;
        let view = PcbView::parse(source, PcbLimits::default()).unwrap();
        let regions = board_regions(&view, &[]).unwrap();

        assert_eq!(regions.len(), 1);
        assert!((regions[0].area() - 361.647).abs() < 0.01, "{regions:?}");
    }
}
