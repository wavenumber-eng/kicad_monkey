//! Bounded KiCad-compatible contour normalization and hole fracture.

use crate::{TextContour, TextContourError, TextContourErrorKind, TextPoint};
use std::cmp::Ordering;

/// Independent resource ceilings for contour topology and bridge construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextTopologyLimits {
    pub max_input_contours: usize,
    pub max_input_points: usize,
    pub max_output_polygons: usize,
    pub max_edge_slots: usize,
    pub max_output_points: usize,
    pub max_containment_edge_checks: usize,
    pub max_bridge_edge_checks: usize,
    pub max_link_steps: usize,
}

impl Default for TextTopologyLimits {
    fn default() -> Self {
        Self {
            max_input_contours: 1024 * 1024,
            max_input_points: 16 * 1024 * 1024,
            max_output_polygons: 1024 * 1024,
            max_edge_slots: 32 * 1024 * 1024,
            max_output_points: 32 * 1024 * 1024,
            max_containment_edge_checks: 64 * 1024 * 1024,
            max_bridge_edge_checks: 64 * 1024 * 1024,
            max_link_steps: 64 * 1024 * 1024,
        }
    }
}

/// Fractured exterior-only contours and observable topology work.
#[derive(Clone, Debug, PartialEq)]
pub struct TextTopologyOutput {
    pub contours: Vec<TextContour>,
    pub input_points: usize,
    pub edge_slots: usize,
    pub output_points: usize,
    pub containment_edge_checks: usize,
    pub bridge_edge_checks: usize,
    pub link_steps: usize,
}

#[derive(Clone, Debug)]
struct Polygon {
    exterior: usize,
    holes: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
struct PathInfo {
    contour: usize,
    start: usize,
    leftmost: usize,
    x_min: f64,
    y_min: f64,
    provoking: usize,
    bridge: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    p1: TextPoint,
    p2: TextPoint,
    next: usize,
}

#[derive(Default)]
struct Work {
    input_points: usize,
    edge_slots: usize,
    output_points: usize,
    containment_edge_checks: usize,
    bridge_edge_checks: usize,
    link_steps: usize,
}

/// Normalize duplicate closure points and fracture holes into exterior paths.
///
/// This mirrors the cache-oriented ordering used by the Python KiCad reference.
/// Input contours must arrive with each exterior before any holes it contains,
/// and one call must cover exactly one glyph's polygon set: KiCad only runs
/// `Fracture()` (and its Clipper2 `Simplify()` seam rewrite plus outer
/// reordering) when the glyph set has holes, so a hole-free input keeps font
/// order and ring starts untouched.
/// Contours with fewer than three points after closure normalization are ignored.
pub fn fracture_text_contours_a0(
    contours: &[TextContour],
    limits: TextTopologyLimits,
) -> Result<TextTopologyOutput, TextContourError> {
    if contours.len() > limits.max_input_contours {
        return Err(limit("$.contours", "input contour limit exceeded"));
    }
    let mut work = Work::default();
    let normalized = normalize_contours(contours, limits, &mut work)?;
    let polygons = classify_contours(&normalized, limits, &mut work)?;
    let has_holes = polygons.iter().any(|polygon| !polygon.holes.is_empty());
    let mut output = Vec::with_capacity(polygons.len());
    for polygon in &polygons {
        let points = fracture_polygon(&normalized, polygon, has_holes, limits, &mut work)?;
        output.push(TextContour { points });
    }
    if has_holes {
        sort_contours_by_simplify_order(&mut output);
    }
    Ok(TextTopologyOutput {
        contours: output,
        input_points: work.input_points,
        edge_slots: work.edge_slots,
        output_points: work.output_points,
        containment_edge_checks: work.containment_edge_checks,
        bridge_edge_checks: work.bridge_edge_checks,
        link_steps: work.link_steps,
    })
}

fn normalize_contours(
    contours: &[TextContour],
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<Vec<TextContour>, TextContourError> {
    let mut normalized = Vec::with_capacity(contours.len());
    for (contour_index, contour) in contours.iter().enumerate() {
        charge(
            &mut work.input_points,
            contour.points.len(),
            limits.max_input_points,
            "$.contours[*].points",
            "input point limit exceeded",
        )?;
        if contour
            .points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid(
                format!("$.contours[{contour_index}].points"),
                "contour coordinates must be finite",
            ));
        }
        let retained =
            if contour.points.len() > 1 && contour.points.first() == contour.points.last() {
                &contour.points[..contour.points.len() - 1]
            } else {
                &contour.points
            };
        if retained.len() >= 3 {
            normalized.push(TextContour {
                points: retained.to_vec(),
            });
        }
    }
    Ok(normalized)
}

fn classify_contours(
    contours: &[TextContour],
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<Vec<Polygon>, TextContourError> {
    let mut polygons: Vec<Polygon> = Vec::new();
    for (contour_index, contour) in contours.iter().enumerate() {
        let mut parent = None;
        for (polygon_index, candidate) in polygons.iter().enumerate() {
            if point_in_polygon(
                contour.points[0],
                &contours[candidate.exterior].points,
                limits,
                work,
            )? {
                parent = Some(polygon_index);
                break;
            }
        }
        if let Some(parent) = parent {
            polygons[parent].holes.push(contour_index);
        } else {
            if polygons.len() >= limits.max_output_polygons {
                return Err(limit("$.polygons", "output polygon limit exceeded"));
            }
            polygons.push(Polygon {
                exterior: contour_index,
                holes: Vec::new(),
            });
        }
    }
    Ok(polygons)
}

fn point_in_polygon(
    point: TextPoint,
    polygon: &[TextPoint],
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<bool, TextContourError> {
    let mut inside = false;
    for index in 0..polygon.len() {
        charge(
            &mut work.containment_edge_checks,
            1,
            limits.max_containment_edge_checks,
            "$.work.containment_edge_checks",
            "containment edge-check limit exceeded",
        )?;
        let p1 = polygon[index];
        let p2 = polygon[(index + 1) % polygon.len()];
        if (p1.y > point.y) != (p2.y > point.y) {
            let delta_x = checked_derived(p2.x - p1.x)?;
            let delta_y = checked_derived(p2.y - p1.y)?;
            let query_delta_y = checked_derived(point.y - p1.y)?;
            let intersect =
                checked_derived(checked_derived(delta_x * query_delta_y)? / delta_y + p1.x)?;
            if point.x < intersect {
                inside = !inside;
            }
        }
    }
    Ok(inside)
}

fn fracture_polygon(
    contours: &[TextContour],
    polygon: &Polygon,
    group_has_holes: bool,
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<Vec<TextPoint>, TextContourError> {
    if polygon.holes.is_empty() {
        let exterior = &contours[polygon.exterior].points;
        charge_output(exterior.len(), limits, work)?;
        if !group_has_holes {
            return Ok(exterior.clone());
        }
        // A holed glyph's `Simplify()` rewrites every ring seam, including
        // hole-free outers sharing the glyph's polygon set.
        let start = seam_index(exterior);
        let mut rotated = exterior[start..].to_vec();
        rotated.extend_from_slice(&exterior[..start]);
        return Ok(rotated);
    }

    let exterior = &contours[polygon.exterior].points;
    let mut infos = Vec::with_capacity(polygon.holes.len() + 1);
    infos.push(path_info(contours, polygon.exterior, seam_index(exterior)));
    for &hole in &polygon.holes {
        infos.push(path_info(contours, hole, 0));
    }
    infos[1..].sort_by(|left, right| {
        finite_cmp(left.x_min, right.x_min).then_with(|| finite_cmp(left.y_min, right.y_min))
    });

    let required_slots = infos.iter().try_fold(0usize, |total, info| {
        total
            .checked_add(contours[info.contour].points.len())
            .and_then(|value| value.checked_add(usize::from(info.contour != polygon.exterior) * 3))
    });
    let required_slots =
        required_slots.ok_or_else(|| limit("$.edges", "edge slot count overflow"))?;
    charge(
        &mut work.edge_slots,
        required_slots,
        limits.max_edge_slots,
        "$.edges",
        "edge slot limit exceeded",
    )?;
    let mut edges: Vec<Option<Edge>> = Vec::with_capacity(required_slots);
    for info in &mut infos {
        let points = &contours[info.contour].points;
        let provoking = edges.len();
        info.provoking = provoking;
        for index in 0..points.len() {
            edges.push(Some(Edge {
                p1: path_point(points, info.start, index),
                p2: path_point(points, info.start, (index + 1) % points.len()),
                next: if index + 1 < points.len() {
                    provoking + index + 1
                } else {
                    provoking
                },
            }));
        }
        if info.contour != polygon.exterior {
            info.bridge = Some(edges.len());
            edges.extend([None, None, None]);
        }
    }

    for info in infos.iter().skip(1) {
        bridge_hole(&mut edges, *info, limits, work)?;
    }
    collect_fractured(&edges, limits, work)
}

/// Ring seam stored by `Fracture()`'s Clipper2 `Simplify()` rewrite: one past
/// the exit of the min-y run holding the smallest-x min-y point.
///
/// A "run" is a maximal stretch of consecutive stored points at the ring's
/// minimum y. When several min-y runs exist, the run containing the
/// smallest-x min-y point wins, matching Clipper2's local-minima sweep order.
fn seam_index(points: &[TextPoint]) -> usize {
    let min_y = points
        .iter()
        .map(|point| point.y)
        .min_by(|left, right| finite_cmp(*left, *right))
        .expect("normalized contours are nonempty");
    let anchor = points
        .iter()
        .enumerate()
        .filter(|(_, point)| point.y == min_y)
        .min_by(|(_, left), (_, right)| finite_cmp(left.x, right.x))
        .map(|(index, _)| index)
        .expect("normalized contours are nonempty");
    let mut end = anchor;
    for _ in 1..points.len() {
        let following = (end + 1) % points.len();
        if points[following].y != min_y {
            break;
        }
        end = following;
    }
    (end + 1) % points.len()
}

/// Approximate Clipper2's `LocMinSorter` outer output order within one glyph.
///
/// KiCad's `Fracture()` runs `Simplify()` per glyph `SHAPE_POLY_SET`, and
/// Clipper2's sweep emits each disjoint outer at its bottom-most local
/// minimum: bottom-most first (largest y in the y-down cache frame), ties by
/// smallest x, otherwise stable in input order.
fn sort_contours_by_simplify_order(contours: &mut [TextContour]) {
    contours.sort_by(|left, right| {
        let (left_y, left_x) = bottom_vertex_key(&left.points);
        let (right_y, right_x) = bottom_vertex_key(&right.points);
        finite_cmp(right_y, left_y).then_with(|| finite_cmp(left_x, right_x))
    });
}

/// Bottom-most vertex key: the largest y and the smallest x at that y.
fn bottom_vertex_key(points: &[TextPoint]) -> (f64, f64) {
    let mut max_y = f64::NEG_INFINITY;
    let mut min_x = f64::INFINITY;
    for point in points {
        if point.y > max_y {
            max_y = point.y;
            min_x = point.x;
        } else if point.y == max_y && point.x < min_x {
            min_x = point.x;
        }
    }
    (max_y, min_x)
}

fn path_info(contours: &[TextContour], contour: usize, start: usize) -> PathInfo {
    let points = &contours[contour].points;
    let count = points.len();
    let x_min = points
        .iter()
        .map(|point| point.x)
        .min_by(|left, right| finite_cmp(*left, *right))
        .expect("normalized contours are nonempty");
    // KiCad's `fractureSingleCacheFriendly()` bridges each hole at the first
    // stored point whose x equals the hole's minimum (strict `<` scan, no
    // tie-break).  The stored order comes from `Fracture()`'s Clipper2
    // `Simplify()`, so the scan starts at the ring's Simplify seam and takes
    // the first min-x point reached in stored traversal order.
    let seam = seam_index(points);
    let leftmost_raw = (0..count)
        .map(|offset| (seam + offset) % count)
        .find(|&index| points[index].x == x_min)
        .expect("normalized contours are nonempty");
    // Edge slots view the ring rotated by `start`; express the bridge vertex
    // in that view.
    let leftmost = (leftmost_raw + count - start) % count;
    let y_min = points
        .iter()
        .map(|point| point.y)
        .min_by(|left, right| finite_cmp(*left, *right))
        .expect("normalized contours are nonempty");
    PathInfo {
        contour,
        start,
        leftmost,
        x_min,
        y_min,
        provoking: 0,
        bridge: None,
    }
}

fn bridge_hole(
    edges: &mut [Option<Edge>],
    info: PathInfo,
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<(), TextContourError> {
    let hole_edge_index = info.provoking + info.leftmost;
    let hole_edge = edges[hole_edge_index].expect("hole edge is populated");
    let mut nearest: Option<(usize, f64)> = None;
    let mut min_distance = f64::INFINITY;
    for (candidate_index, candidate) in edges.iter().copied().enumerate().take(info.provoking) {
        charge(
            &mut work.bridge_edge_checks,
            1,
            limits.max_bridge_edge_checks,
            "$.work.bridge_edge_checks",
            "bridge edge-check limit exceeded",
        )?;
        let Some(candidate) = candidate else {
            continue;
        };
        if !edge_matches_y(candidate.p1, candidate.p2, hole_edge.p1.y) {
            continue;
        }
        let intersect = edge_x_intersect(candidate.p1, candidate.p2, hole_edge.p1.y)?;
        let distance = checked_derived(hole_edge.p1.x - intersect)?;
        if distance >= 0.0 && distance < min_distance {
            min_distance = distance;
            nearest = Some((candidate_index, intersect));
        }
    }
    let Some((nearest_index, nearest_x)) = nearest else {
        return Ok(());
    };
    let bridge = info.bridge.expect("holes reserve bridge slots");
    let nearest_edge = edges[nearest_index].expect("nearest edge is populated");
    let split = TextPoint {
        x: nearest_x,
        y: hole_edge.p1.y,
    };
    edges[bridge] = Some(Edge {
        p1: split,
        p2: hole_edge.p1,
        next: hole_edge_index,
    });
    edges[bridge + 1] = Some(Edge {
        p1: hole_edge.p1,
        p2: split,
        next: bridge + 2,
    });
    edges[bridge + 2] = Some(Edge {
        p1: split,
        p2: nearest_edge.p2,
        next: nearest_edge.next,
    });
    edges[nearest_index] = Some(Edge {
        p2: split,
        next: bridge,
        ..nearest_edge
    });

    let mut last = hole_edge_index;
    loop {
        charge_link(limits, work)?;
        let next = edges[last].expect("hole ring edge is populated").next;
        if next == hole_edge_index {
            break;
        }
        last = next;
    }
    edges[last]
        .as_mut()
        .expect("hole ring edge is populated")
        .next = bridge + 1;
    Ok(())
}

fn collect_fractured(
    edges: &[Option<Edge>],
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<Vec<TextPoint>, TextContourError> {
    let mut points = Vec::new();
    let mut current = 0;
    loop {
        charge_link(limits, work)?;
        charge_output(1, limits, work)?;
        let edge = edges[current].expect("fractured exterior edge is populated");
        points.push(edge.p1);
        if edge.next == 0 {
            break;
        }
        current = edge.next;
    }
    Ok(points)
}

fn path_point(points: &[TextPoint], start: usize, offset: usize) -> TextPoint {
    points[(start + offset) % points.len()]
}

fn edge_matches_y(p1: TextPoint, p2: TextPoint, y: f64) -> bool {
    (y >= p1.y || y >= p2.y) && (y <= p1.y || y <= p2.y)
}

fn edge_x_intersect(p1: TextPoint, p2: TextPoint, y: f64) -> Result<f64, TextContourError> {
    if p1.y == p2.y {
        Ok(if p1.x < p2.x { p2.x } else { p1.x })
    } else {
        let delta_x = checked_derived(p2.x - p1.x)?;
        let delta_y = checked_derived(p2.y - p1.y)?;
        let query_delta_y = checked_derived(y - p1.y)?;
        checked_derived(p1.x + checked_derived(delta_x * query_delta_y)? / delta_y)
    }
}

fn finite_cmp(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right)
        .expect("validated coordinates are finite")
}

fn checked_derived(value: f64) -> Result<f64, TextContourError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid(
            "$.work.derived_coordinate".to_owned(),
            "contour topology produced a nonfinite derived coordinate",
        ))
    }
}

fn charge_output(
    amount: usize,
    limits: TextTopologyLimits,
    work: &mut Work,
) -> Result<(), TextContourError> {
    charge(
        &mut work.output_points,
        amount,
        limits.max_output_points,
        "$.output.points",
        "output point limit exceeded",
    )
}

fn charge_link(limits: TextTopologyLimits, work: &mut Work) -> Result<(), TextContourError> {
    charge(
        &mut work.link_steps,
        1,
        limits.max_link_steps,
        "$.work.link_steps",
        "edge-link traversal limit exceeded",
    )
}

fn charge(
    value: &mut usize,
    amount: usize,
    maximum: usize,
    path: &'static str,
    message: &'static str,
) -> Result<(), TextContourError> {
    let next = value
        .checked_add(amount)
        .ok_or_else(|| limit(path, message))?;
    if next > maximum {
        return Err(limit(path, message));
    }
    *value = next;
    Ok(())
}

fn limit(path: &'static str, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::ResourceLimit,
        path: path.to_owned(),
        message,
    }
}

fn invalid(path: String, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::InvalidInput,
        path,
        message,
    }
}
