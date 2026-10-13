//! Shape rules: parameter ranges, non-empty paths, and bounds that hold the
//! shape.

use super::{NM_MAX, check_length};
use crate::generated::materialization_document::{
    Affine2D, Contour, NmBounds, NmPoint, PathSegment, Shape2d,
};
use crate::{ValidationError, validation_error};

/// Rounding slack between a transformed anchor point and stored bounds.
const BOUNDS_SLACK_NM: f64 = 2.0;

/// Parameter ranges and path rules of `shape`.
pub(super) fn validate_shape(shape: &Shape2d, path: &str) -> Result<(), ValidationError> {
    for length in shape_lengths(shape) {
        check_length(length, path)?;
    }
    match shape {
        Shape2d::RoundedRect2d(rect) => {
            check_corner(*rect.radius_nm, *rect.width_nm, *rect.height_nm, path)
        }
        Shape2d::ChamferedRect2d(rect) => {
            check_corner(*rect.radius_nm, *rect.width_nm, *rect.height_nm, path)?;
            check_corner(*rect.chamfer_nm, *rect.width_nm, *rect.height_nm, path)
        }
        Shape2d::Annulus2d(annulus) => {
            check_annulus(*annulus.inner_radius_nm, *annulus.outer_radius_nm, path)
        }
        Shape2d::SweptPath2d(swept) => check_segments(&swept.segments, path),
        Shape2d::PlanarRegion2d(region) => {
            check_segments(&region.outer.segments, &format!("{path}.outer"))?;
            for (position, hole) in region.holes.iter().enumerate() {
                check_segments(&hole.segments, &format!("{path}.holes[{position}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn shape_lengths(shape: &Shape2d) -> Vec<i64> {
    match shape {
        Shape2d::Disk2d(disk) => vec![*disk.radius_nm],
        Shape2d::Rect2d(rect) => vec![*rect.width_nm, *rect.height_nm],
        Shape2d::RoundedRect2d(rect) => vec![*rect.width_nm, *rect.height_nm, *rect.radius_nm],
        Shape2d::ChamferedRect2d(rect) => vec![
            *rect.width_nm,
            *rect.height_nm,
            *rect.radius_nm,
            *rect.chamfer_nm,
        ],
        Shape2d::Trapezoid2d(trapezoid) => vec![*trapezoid.width_nm, *trapezoid.height_nm],
        Shape2d::Capsule2d(capsule) => vec![*capsule.radius_nm],
        Shape2d::Annulus2d(annulus) => vec![*annulus.outer_radius_nm, *annulus.inner_radius_nm],
        Shape2d::SweptPath2d(swept) => vec![*swept.width_nm],
        Shape2d::PlanarRegion2d(_) => Vec::new(),
    }
}

fn check_segments(segments: &[PathSegment], path: &str) -> Result<(), ValidationError> {
    if !segments.is_empty() {
        return Ok(());
    }
    Err(validation_error(
        "empty_path",
        format!("{path}.segments"),
        "contours and swept paths have at least one segment",
    ))
}

fn check_annulus(inner: i64, outer: i64, path: &str) -> Result<(), ValidationError> {
    if inner < outer {
        return Ok(());
    }
    Err(validation_error(
        "invalid_annulus",
        path.to_owned(),
        "an annulus inner radius is below its outer radius",
    ))
}

fn check_corner(size: i64, width: i64, height: i64, path: &str) -> Result<(), ValidationError> {
    if size.saturating_mul(2) <= width.min(height) {
        return Ok(());
    }
    Err(validation_error(
        "invalid_corner",
        path.to_owned(),
        "corner radius or chamfer is at most half the shorter side",
    ))
}

/// Bounds hold every anchor point of the shape: points that lie on any
/// realization of it (centres' extremes, corners, edge midpoints, path
/// vertices, arc midpoints). This is a necessary condition, not an exact
/// bounds check. An eroding offset may shrink the shape past its anchors, so
/// it is skipped there.
pub(super) fn check_bounds_hold_shape(
    shape: &Shape2d,
    affine: &Affine2D,
    offset_nm: i64,
    bounds: &NmBounds,
    path: &str,
) -> Result<(), ValidationError> {
    if offset_nm < 0 {
        return Ok(());
    }
    let inside = |(x, y): (f64, f64)| {
        let board_x = affine.m11 * x + affine.m12 * y + f64::from(affine.tx_nm.0);
        let board_y = affine.m21 * x + affine.m22 * y + f64::from(affine.ty_nm.0);
        board_x >= f64::from(bounds.min_x_nm.0) - BOUNDS_SLACK_NM
            && board_x <= f64::from(bounds.max_x_nm.0) + BOUNDS_SLACK_NM
            && board_y >= f64::from(bounds.min_y_nm.0) - BOUNDS_SLACK_NM
            && board_y <= f64::from(bounds.max_y_nm.0) + BOUNDS_SLACK_NM
    };
    if anchor_points(shape).into_iter().all(inside) {
        return Ok(());
    }
    Err(validation_error(
        "bounds_exclude_shape",
        format!("{path}.bounds"),
        "operation bounds hold the whole shape after its affine",
    ))
}

fn point(value: &NmPoint) -> (f64, f64) {
    let [x, y] = [value.0.first(), value.0.get(1)].map(|v| v.map_or(0.0, |n| f64::from(n.0)));
    (x, y)
}

/// The four axis extremes of a disk of `radius` around `centre`.
fn disk_extremes((x, y): (f64, f64), radius: f64) -> [(f64, f64); 4] {
    [
        (x - radius, y),
        (x + radius, y),
        (x, y - radius),
        (x, y + radius),
    ]
}

/// Edge midpoints of a `width` by `height` box around `centre`; they lie on
/// every rectangle variant, rounded, chamfered, or trapezoidal.
fn edge_midpoints((x, y): (f64, f64), width: i64, height: i64) -> [(f64, f64); 4] {
    let (half_w, half_h) = (width as f64 / 2.0, height as f64 / 2.0);
    [
        (x - half_w, y),
        (x + half_w, y),
        (x, y - half_h),
        (x, y + half_h),
    ]
}

fn contour_points(start: &NmPoint, segments: &[PathSegment]) -> Vec<(f64, f64)> {
    let mut points = vec![point(start)];
    for segment in segments {
        match segment {
            PathSegment::LineSegment(line) => points.push(point(&line.end_nm)),
            PathSegment::ArcSegment(arc) => {
                points.push(point(&arc.mid_nm));
                points.push(point(&arc.end_nm));
            }
            PathSegment::CubicSegment(cubic) => points.push(point(&cubic.end_nm)),
        }
    }
    points
}

fn anchor_points(shape: &Shape2d) -> Vec<(f64, f64)> {
    match shape {
        Shape2d::Disk2d(disk) => {
            disk_extremes(point(&disk.center_nm), *disk.radius_nm as f64).to_vec()
        }
        Shape2d::Rect2d(rect) => {
            let (x, y) = point(&rect.center_nm);
            let (half_w, half_h) = (*rect.width_nm as f64 / 2.0, *rect.height_nm as f64 / 2.0);
            vec![
                (x - half_w, y - half_h),
                (x + half_w, y - half_h),
                (x - half_w, y + half_h),
                (x + half_w, y + half_h),
            ]
        }
        Shape2d::RoundedRect2d(rect) => {
            edge_midpoints(point(&rect.center_nm), *rect.width_nm, *rect.height_nm).to_vec()
        }
        Shape2d::ChamferedRect2d(rect) => {
            edge_midpoints(point(&rect.center_nm), *rect.width_nm, *rect.height_nm).to_vec()
        }
        Shape2d::Trapezoid2d(trapezoid) => edge_midpoints(
            point(&trapezoid.center_nm),
            *trapezoid.width_nm,
            *trapezoid.height_nm,
        )
        .to_vec(),
        Shape2d::Capsule2d(capsule) => {
            let radius = *capsule.radius_nm as f64;
            let mut points = disk_extremes(point(&capsule.start_nm), radius).to_vec();
            points.extend(disk_extremes(point(&capsule.end_nm), radius));
            points
        }
        Shape2d::Annulus2d(annulus) => {
            disk_extremes(point(&annulus.center_nm), *annulus.outer_radius_nm as f64).to_vec()
        }
        Shape2d::SweptPath2d(swept) => contour_points(&swept.start_nm, &swept.segments),
        Shape2d::PlanarRegion2d(region) => outer_points(&region.outer),
    }
}

fn outer_points(contour: &Contour) -> Vec<(f64, f64)> {
    contour_points(&contour.start_nm, &contour.segments)
}

/// Whether `value` is a valid non-negative 32-bit nanometre length.
pub(super) fn in_length_range(value: i64) -> bool {
    (0..=NM_MAX).contains(&value)
}
