//! Board profile closure from Edge.Cuts carriers.
//!
//! Every board and footprint `Edge.Cuts` carrier becomes exact edges in the
//! board frame: lines, three-point arcs, and cubic Béziers, plus closed
//! loops for rectangles, circles, and polygons. Open edges are chained end
//! to end within [`CHAIN_TOLERANCE_NM`] (KiCad's outline chaining epsilon),
//! through a grid of endpoints so chaining stays near linear. Loops are
//! nested by even-odd
//! containment: even-depth loops are board regions and the loops directly
//! inside them are cutouts. Loops below [`MIN_LOOP_AREA_NM2`] are dropped.

use super::geometry::{circumcircle, nm, rotate_mm};
use crate::pcb::{
    PcbFootprint, PcbGraphic, PcbGraphicKind, PcbPoint, PcbPolygonPoint, PcbProfileOwner,
    PcbProfilePrimitive,
};
use kicad_monkey_contracts::generated::materialization_document::{
    ArcSegment, Contour, CubicSegment, LineSegment, Nm, NmPoint, PathSegment, PlanarRegion2d,
};

/// Endpoints within this Euclidean distance are the same point when
/// chaining: KiCad's `DEFAULT_CHAINING_EPSILON_MM` of 0.01 mm.
pub(crate) const CHAIN_TOLERANCE_NM: i64 = 10_000;
/// Loops enclosing less area than 1e-4 mm² are discarded.
pub(crate) const MIN_LOOP_AREA_NM2: f64 = 1.0e8;
const SAMPLES_PER_CURVE: usize = 16;

type Xy = (i64, i64);

#[derive(Clone)]
struct Edge {
    start: Xy,
    segment: PathSegment,
}

fn point(xy: Xy) -> NmPoint {
    NmPoint([Nm(xy.0 as i32), Nm(xy.1 as i32)])
}

fn xy(point: &NmPoint) -> Xy {
    (i64::from(*point.0[0]), i64::from(*point.0[1]))
}

fn end_of(segment: &PathSegment) -> Xy {
    match segment {
        PathSegment::LineSegment(line) => xy(&line.end_nm),
        PathSegment::ArcSegment(arc) => xy(&arc.end_nm),
        PathSegment::CubicSegment(cubic) => xy(&cubic.end_nm),
    }
}

fn line(end: Xy) -> PathSegment {
    PathSegment::LineSegment(LineSegment {
        kind: "line".to_owned(),
        end_nm: point(end),
    })
}

fn arc(mid: Xy, end: Xy) -> PathSegment {
    PathSegment::ArcSegment(ArcSegment {
        kind: "arc".to_owned(),
        mid_nm: point(mid),
        end_nm: point(end),
    })
}

fn cubic(control1: Xy, control2: Xy, end: Xy) -> PathSegment {
    PathSegment::CubicSegment(CubicSegment {
        kind: "cubic".to_owned(),
        control1_nm: point(control1),
        control2_nm: point(control2),
        end_nm: point(end),
    })
}

/// Same edge traversed the other way.
fn reversed(edge: &Edge) -> Edge {
    let end = end_of(&edge.segment);
    let segment = match &edge.segment {
        PathSegment::LineSegment(_) => line(edge.start),
        PathSegment::ArcSegment(value) => arc(xy(&value.mid_nm), edge.start),
        PathSegment::CubicSegment(value) => {
            cubic(xy(&value.control2_nm), xy(&value.control1_nm), edge.start)
        }
    };
    Edge {
        start: end,
        segment,
    }
}

/// Places carrier-local millimetre points in the board frame, in nm.
struct Placement {
    x: f64,
    y: f64,
    angle: f64,
}

impl Placement {
    fn of(owner: &PcbProfileOwner, footprints: &[PcbFootprint]) -> Self {
        match owner {
            PcbProfileOwner::Footprint { footprint_index } => footprints
                .get(*footprint_index)
                .map_or(Self::board(), |footprint| Self {
                    x: footprint.at_x.unwrap_or_default(),
                    y: footprint.at_y.unwrap_or_default(),
                    angle: footprint.angle.unwrap_or_default(),
                }),
            PcbProfileOwner::Board => Self::board(),
        }
    }

    fn board() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            angle: 0.0,
        }
    }

    fn place(&self, point: PcbPoint) -> Xy {
        let (dx, dy) = rotate_mm(point.x, point.y, -self.angle);
        (i64::from(nm(self.x + dx)), i64::from(nm(self.y + dy)))
    }
}

/// Open edges and closed loops contributed by the profile carriers.
#[derive(Default)]
struct Pieces {
    edges: Vec<Edge>,
    loops: Vec<Contour>,
}

impl Pieces {
    fn add(&mut self, graphic: &PcbGraphic, placement: &Placement) {
        let at = |value: Option<PcbPoint>| value.map(|value| placement.place(value));
        match graphic.kind {
            PcbGraphicKind::Line => self.open(at(graphic.start), at(graphic.end), line),
            PcbGraphicKind::Arc => {
                if let Some(mid) = at(graphic.mid) {
                    self.open(at(graphic.start), at(graphic.end), |end| arc(mid, end));
                }
            }
            PcbGraphicKind::Curve => self.curve(&graphic.points, placement),
            PcbGraphicKind::Rect => self.rect(graphic, placement),
            PcbGraphicKind::Circle => self.circle(at(graphic.center), at(graphic.end)),
            PcbGraphicKind::Poly => self.poly(graphic, placement),
            PcbGraphicKind::Text | PcbGraphicKind::TextBox => {}
        }
    }

    fn open(&mut self, start: Option<Xy>, end: Option<Xy>, make: impl Fn(Xy) -> PathSegment) {
        if let (Some(start), Some(end)) = (start, end)
            && start != end
        {
            self.edges.push(Edge {
                start,
                segment: make(end),
            });
        }
    }

    fn curve(&mut self, points: &[PcbPoint], placement: &Placement) {
        if let [p0, p1, p2, p3] = points {
            let [a, b, c, d] = [*p0, *p1, *p2, *p3].map(|value| placement.place(value));
            self.edges.push(Edge {
                start: a,
                segment: cubic(b, c, d),
            });
        }
    }

    /// A rectangle built in its carrier's frame, so a rotated footprint
    /// rotates it, with KiCad's corner radius (board format 20250829) as
    /// quarter arcs.
    fn rect(&mut self, graphic: &PcbGraphic, placement: &Placement) {
        let (Some(start), Some(end)) = (graphic.start, graphic.end) else {
            return;
        };
        let (left, right) = (start.x.min(end.x), start.x.max(end.x));
        let (top, bottom) = (start.y.min(end.y), start.y.max(end.y));
        let radius = graphic
            .radius
            .unwrap_or(0.0)
            .clamp(0.0, (right - left).min(bottom - top) / 2.0);
        let at = |x: f64, y: f64| placement.place(PcbPoint { x, y });
        if nm(radius) == 0 {
            self.loops.push(Contour {
                start_nm: point(at(left, top)),
                segments: vec![
                    line(at(right, top)),
                    line(at(right, bottom)),
                    line(at(left, bottom)),
                ],
            });
            return;
        }
        let bulge = radius * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
        self.loops.push(Contour {
            start_nm: point(at(left + radius, top)),
            segments: vec![
                line(at(right - radius, top)),
                arc(at(right - bulge, top + bulge), at(right, top + radius)),
                line(at(right, bottom - radius)),
                arc(
                    at(right - bulge, bottom - bulge),
                    at(right - radius, bottom),
                ),
                line(at(left + radius, bottom)),
                arc(at(left + bulge, bottom - bulge), at(left, bottom - radius)),
                line(at(left, top + radius)),
                arc(at(left + bulge, top + bulge), at(left + radius, top)),
            ],
        });
    }

    fn circle(&mut self, center: Option<Xy>, end: Option<Xy>) {
        let (Some((cx, cy)), Some((ex, ey))) = (center, end) else {
            return;
        };
        let radius = ((ex - cx) as f64).hypot((ey - cy) as f64).round() as i64;
        if radius <= 0 {
            return;
        }
        self.loops.push(Contour {
            start_nm: point((cx + radius, cy)),
            segments: vec![
                arc((cx, cy + radius), (cx - radius, cy)),
                arc((cx, cy - radius), (cx + radius, cy)),
            ],
        });
    }

    fn poly(&mut self, graphic: &PcbGraphic, placement: &Placement) {
        let mut start = None;
        let mut segments = Vec::new();
        for item in &graphic.polygon_points {
            match item {
                PcbPolygonPoint::Xy(value) if start.is_none() => {
                    start = Some(placement.place(*value))
                }
                PcbPolygonPoint::Xy(value) => segments.push(line(placement.place(*value))),
                PcbPolygonPoint::Arc {
                    start: from,
                    mid,
                    end,
                } => {
                    match start {
                        None => start = Some(placement.place(*from)),
                        Some(_) => segments.push(line(placement.place(*from))),
                    }
                    segments.push(arc(placement.place(*mid), placement.place(*end)));
                }
            }
        }
        if graphic.polygon_points.is_empty() {
            let mut points = graphic.points.iter().map(|value| placement.place(*value));
            start = points.next();
            segments.extend(points.map(line));
        }
        if let Some(start) = start
            && segments.len() >= 2
        {
            self.loops.push(Contour {
                start_nm: point(start),
                segments,
            });
        }
    }
}

fn near(a: Xy, b: Xy) -> bool {
    let (dx, dy) = (i128::from(a.0 - b.0), i128::from(a.1 - b.1));
    dx * dx + dy * dy <= i128::from(CHAIN_TOLERANCE_NM) * i128::from(CHAIN_TOLERANCE_NM)
}

/// Open edges indexed by the grid cells of their endpoints; a cell is one
/// tolerance wide, so every near endpoint is in the 3 x 3 cells around.
struct Endpoints {
    edges: Vec<Edge>,
    used: Vec<bool>,
    cells: std::collections::HashMap<Xy, Vec<usize>>,
}

fn cell(point: Xy) -> Xy {
    (
        point.0.div_euclid(CHAIN_TOLERANCE_NM),
        point.1.div_euclid(CHAIN_TOLERANCE_NM),
    )
}

impl Endpoints {
    fn new(edges: Vec<Edge>) -> Self {
        let mut cells: std::collections::HashMap<Xy, Vec<usize>> = Default::default();
        for (index, edge) in edges.iter().enumerate() {
            for end in [edge.start, end_of(&edge.segment)] {
                cells.entry(cell(end)).or_default().push(index);
            }
        }
        Self {
            used: vec![false; edges.len()],
            edges,
            cells,
        }
    }

    /// The lowest-index unused edge with an endpoint near `point`, oriented
    /// to start there.
    fn take_near(&mut self, point: Xy) -> Option<Edge> {
        let (cx, cy) = cell(point);
        let mut best: Option<usize> = None;
        for key in (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (cx + dx, cy + dy))) {
            for &index in self.cells.get(&key).into_iter().flatten() {
                let edge = &self.edges[index];
                let touches = near(edge.start, point) || near(end_of(&edge.segment), point);
                if !self.used[index] && touches && best.is_none_or(|b| index < b) {
                    best = Some(index);
                }
            }
        }
        let index = best?;
        self.used[index] = true;
        let edge = &self.edges[index];
        Some(if near(edge.start, point) {
            edge.clone()
        } else {
            reversed(edge)
        })
    }
}

/// Grow `path` from its tail with matching edges until it closes or no
/// edge continues it.
fn extend(path: &mut Vec<Edge>, endpoints: &mut Endpoints) {
    loop {
        let tail = end_of(&path[path.len() - 1].segment);
        if path.len() > 1 && near(tail, path[0].start) {
            return;
        }
        let Some(edge) = endpoints.take_near(tail) else {
            return;
        };
        path.push(edge);
    }
}

/// Chain open edges into closed contours, starting from the lowest unused
/// edge each time. Returns the contours and the number of edges that could
/// not be closed into a loop.
fn chain(edges: Vec<Edge>) -> (Vec<Contour>, usize) {
    let mut endpoints = Endpoints::new(edges);
    let mut contours = Vec::new();
    let mut unclosed = 0usize;
    for first in 0..endpoints.edges.len() {
        if endpoints.used[first] {
            continue;
        }
        endpoints.used[first] = true;
        let mut path = vec![endpoints.edges[first].clone()];
        extend(&mut path, &mut endpoints);
        if near(end_of(&path[path.len() - 1].segment), path[0].start) {
            contours.push(Contour {
                start_nm: point(path[0].start),
                segments: path.into_iter().map(|edge| edge.segment).collect(),
            });
        } else {
            unclosed += path.len();
        }
    }
    (contours, unclosed)
}

/// Sampled polyline of a contour in nm, for area and containment only.
fn sample(contour: &Contour) -> Vec<(f64, f64)> {
    let mut previous = xy(&contour.start_nm);
    let mut points = vec![(previous.0 as f64, previous.1 as f64)];
    for segment in &contour.segments {
        let end = end_of(segment);
        match segment {
            PathSegment::LineSegment(_) => {}
            PathSegment::ArcSegment(value) => {
                points.extend(sample_arc(previous, xy(&value.mid_nm), end));
            }
            PathSegment::CubicSegment(value) => {
                points.extend(sample_cubic(
                    previous,
                    xy(&value.control1_nm),
                    xy(&value.control2_nm),
                    end,
                ));
            }
        }
        points.push((end.0 as f64, end.1 as f64));
        previous = end;
    }
    points
}

fn sample_arc(start: Xy, mid: Xy, end: Xy) -> Vec<(f64, f64)> {
    let Some((cx, cy, radius)) = circumcircle(&point(start), &point(mid), &point(end)) else {
        return Vec::new();
    };
    let angle = |p: Xy| (p.1 as f64 - cy).atan2(p.0 as f64 - cx);
    let (a0, am, a1) = (angle(start), angle(mid), angle(end));
    let tau = std::f64::consts::TAU;
    let ccw = |from: f64, to: f64| (to - from).rem_euclid(tau);
    let sweep = if ccw(a0, am) <= ccw(a0, a1) {
        ccw(a0, a1)
    } else {
        ccw(a0, a1) - tau
    };
    (1..SAMPLES_PER_CURVE)
        .map(|step| {
            let theta = a0 + sweep * step as f64 / SAMPLES_PER_CURVE as f64;
            (cx + radius * theta.cos(), cy + radius * theta.sin())
        })
        .collect()
}

fn sample_cubic(p0: Xy, p1: Xy, p2: Xy, p3: Xy) -> Vec<(f64, f64)> {
    (1..SAMPLES_PER_CURVE)
        .map(|step| {
            let t = step as f64 / SAMPLES_PER_CURVE as f64;
            let mt = 1.0 - t;
            let blend = |a: i64, b: i64, c: i64, d: i64| {
                mt * mt * mt * a as f64
                    + 3.0 * mt * mt * t * b as f64
                    + 3.0 * mt * t * t * c as f64
                    + t * t * t * d as f64
            };
            (blend(p0.0, p1.0, p2.0, p3.0), blend(p0.1, p1.1, p2.1, p3.1))
        })
        .collect()
}

fn area(points: &[(f64, f64)]) -> f64 {
    let twice: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum();
    twice.abs() / 2.0
}

fn contains(polygon: &[(f64, f64)], (x, y): (f64, f64)) -> bool {
    let mut inside = false;
    for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1) {
            inside = !inside;
        }
    }
    inside
}

/// Result of profile closure.
pub(crate) struct Profile {
    pub regions: Vec<PlanarRegion2d>,
    pub unclosed_edges: usize,
}

/// Close the board profile from its Edge.Cuts carriers.
pub(crate) fn close_profile(
    primitives: &[PcbProfilePrimitive],
    footprints: &[PcbFootprint],
) -> Profile {
    let mut pieces = Pieces::default();
    for primitive in primitives {
        pieces.add(
            &primitive.graphic,
            &Placement::of(&primitive.owner, footprints),
        );
    }
    let (mut contours, unclosed_edges) = chain(pieces.edges);
    contours.extend(pieces.loops);
    let sampled: Vec<Vec<(f64, f64)>> = contours.iter().map(sample).collect();
    let kept: Vec<usize> = (0..contours.len())
        .filter(|index| area(&sampled[*index]) >= MIN_LOOP_AREA_NM2)
        .collect();
    let depth = |index: usize| {
        kept.iter()
            .filter(|other| **other != index && contains(&sampled[**other], sampled[index][0]))
            .count()
    };
    let depths: Vec<(usize, usize)> = kept.iter().map(|index| (*index, depth(*index))).collect();
    let mut regions: Vec<PlanarRegion2d> = depths
        .iter()
        .filter(|(_, d)| d % 2 == 0)
        .map(|(outer, d)| PlanarRegion2d {
            kind: "planar_region_2d".to_owned(),
            outer: contours[*outer].clone(),
            holes: depths
                .iter()
                .filter(|(hole, hd)| *hd == d + 1 && contains(&sampled[*outer], sampled[*hole][0]))
                .map(|(hole, _)| contours[*hole].clone())
                .collect(),
        })
        .collect();
    regions.sort_by_key(|region| {
        let start = xy(&region.outer.start_nm);
        (start.1, start.0)
    });
    Profile {
        regions,
        unclosed_edges,
    }
}

#[cfg(test)]
mod tests {
    use super::{CHAIN_TOLERANCE_NM, Edge, chain, line};

    /// A closed polygon of `sides` line edges on a circle of `radius_nm`,
    /// listed in a scrambled order, with every other edge reversed, and
    /// each start nudged by `gap_nm` from its predecessor's end.
    fn polygon(sides: usize, radius_nm: f64, gap_nm: i64) -> Vec<Edge> {
        let vertex = |index: usize| {
            let angle = std::f64::consts::TAU * index as f64 / sides as f64;
            (
                (radius_nm * angle.cos()).round() as i64,
                (radius_nm * angle.sin()).round() as i64,
            )
        };
        let mut edges: Vec<Edge> = (0..sides)
            .map(|index| {
                let (start, end) = (vertex(index), vertex((index + 1) % sides));
                let start = (start.0 + gap_nm, start.1);
                if index % 2 == 0 {
                    Edge {
                        start,
                        segment: line(end),
                    }
                } else {
                    Edge {
                        start: end,
                        segment: line(start),
                    }
                }
            })
            .collect();
        // A fixed stride permutation, so the chain must search.
        let stride = 7919 % sides.max(1);
        let order: Vec<usize> = (0..sides).map(|i| (i * stride.max(1)) % sides).collect();
        if order
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == sides
        {
            edges = order.into_iter().map(|i| edges[i].clone()).collect();
        }
        edges
    }

    #[test]
    fn endpoints_within_kicads_epsilon_chain_and_farther_ones_do_not() {
        let (closed, unclosed) = chain(polygon(12, 5_000_000.0, CHAIN_TOLERANCE_NM));
        assert_eq!((closed.len(), unclosed), (1, 0));
        assert_eq!(closed[0].segments.len(), 12);
        let (closed, unclosed) = chain(polygon(12, 5_000_000.0, CHAIN_TOLERANCE_NM + 1));
        assert!(closed.is_empty());
        assert_eq!(unclosed, 12);
    }

    #[test]
    fn large_outlines_chain_into_one_loop() {
        let (closed, unclosed) = chain(polygon(60_000, 1_000_000_000.0, 3_000));
        assert_eq!((closed.len(), unclosed), (1, 0));
        assert_eq!(closed[0].segments.len(), 60_000);
    }
}
