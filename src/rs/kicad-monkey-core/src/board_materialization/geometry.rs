//! Unit conversion, affines, and analytic bounds for materialized shapes.

use crate::board_plotter_ir::BoardFootprintPlacement;
use crate::pcb::{PcbFootprint, PcbPoint};
use kicad_monkey_contracts::generated::materialization_document::{
    Affine2D, Contour, Nm, NmBounds, NmPoint, NonNegativeNm, PathSegment, Shape2d,
};

const NM_PER_MM: f64 = 1_000_000.0;

/// Millimetres to nanometres, rounded and saturated to KiCad's 32-bit range.
pub(crate) fn nm(value_mm: f64) -> i32 {
    let scaled = (value_mm * NM_PER_MM).round();
    if scaled.is_nan() {
        return 0;
    }
    scaled.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// [`nm`] widened for arithmetic, with the same saturation.
pub(crate) fn nm64(value_mm: f64) -> i64 {
    i64::from(nm(value_mm))
}

/// Millimetres to a non-negative length, saturated like [`nm`].
pub(crate) fn length(value_mm: f64) -> NonNegativeNm {
    NonNegativeNm(nm64(value_mm).max(0))
}

/// Nanometres to millimetres.
pub(crate) fn mm(value_nm: i64) -> f64 {
    value_nm as f64 / NM_PER_MM
}

pub(crate) fn nm_point(x_mm: f64, y_mm: f64) -> NmPoint {
    NmPoint([Nm(nm(x_mm)), Nm(nm(y_mm))])
}

pub(crate) fn pcb_point(point: PcbPoint) -> NmPoint {
    nm_point(point.x, point.y)
}

pub(crate) fn origin() -> NmPoint {
    NmPoint([Nm(0), Nm(0)])
}

pub(crate) fn identity() -> Affine2D {
    rotation_translation(0.0, 0.0, 0.0)
}

/// Rotation by `angle_deg` about the local origin, then translation to
/// (`x_mm`, `y_mm`).
pub(crate) fn rotation_translation(angle_deg: f64, x_mm: f64, y_mm: f64) -> Affine2D {
    let angle = angle_deg.to_radians();
    let (sin, cos) = snap_trig(angle.sin(), angle.cos());
    Affine2D {
        m11: cos,
        m12: -sin,
        m21: sin,
        m22: cos,
        tx_nm: Nm(nm(x_mm)),
        ty_nm: Nm(nm(y_mm)),
    }
}

/// Board placement of a footprint: its rotation, then its position.
/// KiCad angles turn counter-clockwise on screen, which is clockwise in the
/// Y-down board frame, hence the negated angle.
pub(crate) fn footprint_affine(footprint: &PcbFootprint) -> Affine2D {
    rotation_translation(
        -footprint.angle.unwrap_or_default(),
        footprint.at_x.unwrap_or_default(),
        footprint.at_y.unwrap_or_default(),
    )
}

/// [`footprint_affine`] for a plot document's footprint record.
pub(crate) fn placement_affine(placement: &BoardFootprintPlacement) -> Affine2D {
    rotation_translation(-placement.angle_deg, mm(placement.x_nm), mm(placement.y_nm))
}

/// `affine` applied to a local point, rounded to whole nanometres.
pub(crate) fn place(affine: &Affine2D, x: i64, y: i64) -> (i64, i64) {
    let (board_x, board_y) = apply(affine, x, y);
    (board_x.round() as i64, board_y.round() as i64)
}

/// Snap trig values that are within rounding noise of -1, 0, or 1 so right
/// angles produce exact matrices and byte-stable output.
fn snap_trig(sin: f64, cos: f64) -> (f64, f64) {
    let snap = |value: f64| {
        for exact in [-1.0, 0.0, 1.0] {
            if (value - exact).abs() < 1e-12 {
                return exact;
            }
        }
        value
    };
    (snap(sin), snap(cos))
}

/// Rotate a millimetre point by `angle_deg`.
pub(crate) fn rotate_mm(x: f64, y: f64, angle_deg: f64) -> (f64, f64) {
    let angle = angle_deg.to_radians();
    (
        x * angle.cos() - y * angle.sin(),
        x * angle.sin() + y * angle.cos(),
    )
}

/// `affine` applied to a local point, unrounded, so bounds can round
/// outward.
fn apply(affine: &Affine2D, x: i64, y: i64) -> (f64, f64) {
    let (xf, yf) = (x as f64, y as f64);
    (
        affine.m11 * xf + affine.m12 * yf + f64::from(*affine.tx_nm),
        affine.m21 * xf + affine.m22 * yf + f64::from(*affine.ty_nm),
    )
}

/// Integer box around `[min_x, min_y, max_x, max_y]`, rounded outward.
fn outward(bounds: [f64; 4]) -> [i64; 4] {
    [
        bounds[0].floor() as i64,
        bounds[1].floor() as i64,
        bounds[2].ceil() as i64,
        bounds[3].ceil() as i64,
    ]
}

/// Local bounds (min_x, min_y, max_x, max_y) of an analytic shape.
fn local_bounds(shape: &Shape2d) -> [i64; 4] {
    match shape {
        Shape2d::Disk2d(disk) => around(&disk.center_nm, *disk.radius_nm),
        Shape2d::Annulus2d(annulus) => around(&annulus.center_nm, *annulus.outer_radius_nm),
        Shape2d::Rect2d(rect) => centred(&rect.center_nm, *rect.width_nm, *rect.height_nm),
        Shape2d::RoundedRect2d(rect) => centred(&rect.center_nm, *rect.width_nm, *rect.height_nm),
        Shape2d::ChamferedRect2d(rect) => centred(&rect.center_nm, *rect.width_nm, *rect.height_nm),
        Shape2d::Trapezoid2d(trapezoid) => centred(
            &trapezoid.center_nm,
            *trapezoid.width_nm + i64::from(*trapezoid.delta_y_nm).abs(),
            *trapezoid.height_nm + i64::from(*trapezoid.delta_x_nm).abs(),
        ),
        Shape2d::Capsule2d(capsule) => grow(
            span(&[&capsule.start_nm, &capsule.end_nm]),
            *capsule.radius_nm,
        ),
        Shape2d::SweptPath2d(path) => grow(
            path_bounds(&path.start_nm, &path.segments),
            *path.width_nm / 2 + 1,
        ),
        Shape2d::PlanarRegion2d(region) => contour_bounds(&region.outer),
    }
}

fn xy(point: &NmPoint) -> (i64, i64) {
    (i64::from(*point.0[0]), i64::from(*point.0[1]))
}

fn around(center: &NmPoint, radius: i64) -> [i64; 4] {
    let (x, y) = xy(center);
    [x - radius, y - radius, x + radius, y + radius]
}

fn centred(center: &NmPoint, width: i64, height: i64) -> [i64; 4] {
    let (x, y) = xy(center);
    let (half_w, half_h) = ((width.abs() + 1) / 2, (height.abs() + 1) / 2);
    [x - half_w, y - half_h, x + half_w, y + half_h]
}

fn grow(bounds: [i64; 4], amount: i64) -> [i64; 4] {
    [
        bounds[0] - amount,
        bounds[1] - amount,
        bounds[2] + amount,
        bounds[3] + amount,
    ]
}

fn span(points: &[&NmPoint]) -> [i64; 4] {
    points
        .iter()
        .fold([i64::MAX, i64::MAX, i64::MIN, i64::MIN], |bounds, point| {
            let (x, y) = xy(point);
            [
                bounds[0].min(x),
                bounds[1].min(y),
                bounds[2].max(x),
                bounds[3].max(y),
            ]
        })
}

fn merge(a: [i64; 4], b: [i64; 4]) -> [i64; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// Exact bounds of a cubic Bézier: its end points plus the curve at every
/// interior parameter where one coordinate's derivative vanishes.
fn cubic_bounds(points: [&NmPoint; 4]) -> [i64; 4] {
    let [p0, p1, p2, p3] = points.map(xy);
    let mut bounds = span(&[points[0], points[3]]);
    for t in cubic_extrema(p0.0, p1.0, p2.0, p3.0)
        .into_iter()
        .chain(cubic_extrema(p0.1, p1.1, p2.1, p3.1))
    {
        let x = cubic_at(p0.0, p1.0, p2.0, p3.0, t);
        let y = cubic_at(p0.1, p1.1, p2.1, p3.1, t);
        bounds = merge(
            bounds,
            [
                x.floor() as i64,
                y.floor() as i64,
                x.ceil() as i64,
                y.ceil() as i64,
            ],
        );
    }
    bounds
}

fn cubic_at(a: i64, b: i64, c: i64, d: i64, t: f64) -> f64 {
    let mt = 1.0 - t;
    mt * mt * mt * a as f64
        + 3.0 * mt * mt * t * b as f64
        + 3.0 * mt * t * t * c as f64
        + t * t * t * d as f64
}

/// Parameters in (0, 1) where a cubic coordinate is stationary: the roots of
/// its derivative, the quadratic `qa*t^2 + qb*t + qc`.
fn cubic_extrema(a: i64, b: i64, c: i64, d: i64) -> Vec<f64> {
    let (a, b, c, d) = (a as f64, b as f64, c as f64, d as f64);
    let qa = -a + 3.0 * b - 3.0 * c + d;
    let qb = 2.0 * (a - 2.0 * b + c);
    let qc = b - a;
    let roots = if qa.abs() < 1e-12 {
        if qb.abs() < 1e-12 {
            Vec::new()
        } else {
            vec![-qc / qb]
        }
    } else {
        let discriminant = qb * qb - 4.0 * qa * qc;
        if discriminant < 0.0 {
            Vec::new()
        } else {
            let root = discriminant.sqrt();
            vec![(-qb + root) / (2.0 * qa), (-qb - root) / (2.0 * qa)]
        }
    };
    roots.into_iter().filter(|t| *t > 0.0 && *t < 1.0).collect()
}

/// Bounds of a line/arc/cubic path: arcs contribute their swept extremes and
/// cubics their exact extrema.
fn path_bounds(start: &NmPoint, segments: &[PathSegment]) -> [i64; 4] {
    let mut bounds = span(&[start]);
    let mut previous = start.clone();
    for segment in segments {
        match segment {
            PathSegment::LineSegment(line) => {
                bounds = merge(bounds, span(&[&line.end_nm]));
                previous = line.end_nm.clone();
            }
            PathSegment::ArcSegment(arc) => {
                bounds = merge(bounds, arc_bounds(&previous, &arc.mid_nm, &arc.end_nm));
                previous = arc.end_nm.clone();
            }
            PathSegment::CubicSegment(cubic) => {
                let points = [
                    &previous,
                    &cubic.control1_nm,
                    &cubic.control2_nm,
                    &cubic.end_nm,
                ];
                bounds = merge(bounds, cubic_bounds(points));
                previous = cubic.end_nm.clone();
            }
        }
    }
    bounds
}

fn contour_bounds(contour: &Contour) -> [i64; 4] {
    path_bounds(&contour.start_nm, &contour.segments)
}

/// Exact box of a three-point arc: its end points plus each axis extreme
/// of the circle that the arc sweeps, or the point span when collinear.
///
/// A circle point lies on the arc exactly when it is on the same side of
/// the start-end chord as `mid`. An extreme within one nanometre of the
/// chord line is an end point, already included. Extremes are rounded
/// outward.
pub(crate) fn arc_bounds(start: &NmPoint, mid: &NmPoint, end: &NmPoint) -> [i64; 4] {
    let Some((cx, cy, radius)) = circumcircle(start, mid, end) else {
        return span(&[start, mid, end]);
    };
    let ((sx, sy), (mx, my), (ex, ey)) = (xy(start), xy(mid), xy(end));
    let (sx, sy, ex, ey) = (sx as f64, sy as f64, ex as f64, ey as f64);
    let side = |px: f64, py: f64| (ex - sx) * (py - sy) - (ey - sy) * (px - sx);
    let mid_side = side(mx as f64, my as f64).signum();
    let on_chord = (ex - sx).hypot(ey - sy);
    let mut bounds = span(&[start, end]);
    for (px, py) in [
        (cx - radius, cy),
        (cx + radius, cy),
        (cx, cy - radius),
        (cx, cy + radius),
    ] {
        let offset = side(px, py);
        if offset.abs() > on_chord && offset.signum() == mid_side {
            bounds = merge(
                bounds,
                [
                    px.floor() as i64,
                    py.floor() as i64,
                    px.ceil() as i64,
                    py.ceil() as i64,
                ],
            );
        }
    }
    bounds
}

/// Centre and radius of the circle through three points, if not collinear.
pub(crate) fn circumcircle(a: &NmPoint, b: &NmPoint, c: &NmPoint) -> Option<(f64, f64, f64)> {
    let (ax, ay) = xy(a);
    let (bx, by) = xy(b);
    let (cx, cy) = xy(c);
    let (ax, ay, bx, by, cx, cy) = (
        ax as f64, ay as f64, bx as f64, by as f64, cx as f64, cy as f64,
    );
    let determinant = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    let chord = (cx - ax).hypot(cy - ay).max((bx - ax).hypot(by - ay));
    if determinant.abs() <= chord * 1e-6 {
        return None;
    }
    let (sa, sb, sc) = (ax * ax + ay * ay, bx * bx + by * by, cx * cx + cy * cy);
    let ux = (sa * (by - cy) + sb * (cy - ay) + sc * (ay - by)) / determinant;
    let uy = (sa * (cx - bx) + sb * (ax - cx) + sc * (bx - ax)) / determinant;
    Some((ux, uy, (ax - ux).hypot(ay - uy)))
}

/// Uniform scale of a similarity affine (rotation, reflection, uniform
/// scale), or `None` when the affine shears or scales non-uniformly.
fn similarity_scale(affine: &Affine2D) -> Option<f64> {
    let column_x = affine.m11.hypot(affine.m21);
    let column_y = affine.m12.hypot(affine.m22);
    let orthogonal = (affine.m11 * affine.m12 + affine.m21 * affine.m22).abs();
    ((column_x - column_y).abs() < 1e-9 && orthogonal < 1e-9).then_some(column_x)
}

/// Exact bounds of round shapes under a similarity: transform the centre
/// (or capsule ends) and grow by the scaled radius.
fn round_bounds(shape: &Shape2d, affine: &Affine2D) -> Option<[i64; 4]> {
    let scale = similarity_scale(affine)?;
    let radius = |value: i64| value as f64 * scale;
    let place = |point: &NmPoint| {
        let (x, y) = xy(point);
        apply(affine, x, y)
    };
    let disc = |center: &NmPoint, r: f64| {
        let (x, y) = place(center);
        outward([x - r, y - r, x + r, y + r])
    };
    match shape {
        Shape2d::Disk2d(disk) => Some(disc(&disk.center_nm, radius(*disk.radius_nm))),
        Shape2d::Annulus2d(annulus) => {
            Some(disc(&annulus.center_nm, radius(*annulus.outer_radius_nm)))
        }
        Shape2d::Capsule2d(capsule) => {
            let r = radius(*capsule.radius_nm);
            Some(merge(disc(&capsule.start_nm, r), disc(&capsule.end_nm, r)))
        }
        _ => None,
    }
}

/// Board-frame bounds of `shape` after `affine`.
pub(crate) fn board_bounds(shape: &Shape2d, affine: &Affine2D) -> NmBounds {
    let [min_x, min_y, max_x, max_y] =
        round_bounds(shape, affine).unwrap_or_else(|| box_bounds(shape, affine));
    let clamp = |value: i64| Nm(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
    NmBounds {
        min_x_nm: clamp(min_x),
        min_y_nm: clamp(min_y),
        max_x_nm: clamp(max_x),
        max_y_nm: clamp(max_y),
    }
}

/// Bounds of the transformed local box; exact for rectangles, conservative
/// for curved shapes.
fn box_bounds(shape: &Shape2d, affine: &Affine2D) -> [i64; 4] {
    let local = local_bounds(shape);
    let corners = [
        apply(affine, local[0], local[1]),
        apply(affine, local[2], local[1]),
        apply(affine, local[2], local[3]),
        apply(affine, local[0], local[3]),
    ];
    let fold = |pick: fn(&(f64, f64)) -> f64, min: bool| {
        corners
            .iter()
            .map(pick)
            .fold(if min { f64::MAX } else { f64::MIN }, |a, b| {
                if min { a.min(b) } else { a.max(b) }
            })
    };
    outward([
        fold(|corner| corner.0, true),
        fold(|corner| corner.1, true),
        fold(|corner| corner.0, false),
        fold(|corner| corner.1, false),
    ])
}

#[cfg(test)]
mod tests {
    use super::{arc_bounds, nm_point};

    /// Mid points snap to the nanometre grid, which moves the circle by a
    /// fraction of a nanometre, so boxes are compared within 2 nm.
    fn assert_near(actual: [i64; 4], expected: [i64; 4]) {
        assert!(
            actual.iter().zip(expected).all(|(a, e)| (a - e).abs() <= 2),
            "{actual:?} is not within 2 nm of {expected:?}"
        );
    }

    #[test]
    fn arc_bounds_cover_only_the_swept_extremes() {
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let (east, north) = (nm_point(1.0, 0.0), nm_point(0.0, 1.0));
        // Quarter arc: no axis extreme beyond its end points is swept.
        let quarter = arc_bounds(&east, &nm_point(half, half), &north);
        assert_near(quarter, [0, 0, 1_000_000, 1_000_000]);
        // The complementary three-quarter arc sweeps the other extremes.
        let major = arc_bounds(&east, &nm_point(-half, -half), &north);
        assert_near(major, [-1_000_000, -1_000_000, 1_000_000, 1_000_000]);
        // A half circle through (0, 1) reaches y = 1 but not y = -1.
        let semi = arc_bounds(&nm_point(-1.0, 0.0), &north, &east);
        assert_near(semi, [-1_000_000, 0, 1_000_000, 1_000_000]);
    }
}
