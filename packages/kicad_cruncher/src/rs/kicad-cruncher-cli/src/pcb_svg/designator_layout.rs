//! Source-neutral assembly-designator fitting ported from PCB Autodoc.

use std::collections::HashMap;

use geo::{
    Area, BooleanOps, BoundingRect, Centroid, ConvexHull, Coord, Covers, InteriorPoint, LineString,
    MultiPolygon, Point, Polygon, Rect, Translate, algorithm::bool_ops::unary_union,
};

const TEXT_ADVANCE_PER_CHARACTER: f64 = 0.62;
const CENTER_GRID_DIVISIONS: usize = 4;
const BINARY_SEARCH_STEPS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DesignatorFit {
    pub center_mm: [f64; 2],
    pub font_size_mm: f64,
    pub rotation_degrees: f64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FitKey {
    geometry: Vec<i64>,
    text_aspect: i64,
    rotations: Vec<i64>,
    fill_ratio: i64,
    maximum_font_size_mm: i64,
}

#[derive(Default)]
pub(crate) struct DesignatorFitSession {
    fits: HashMap<FitKey, Option<DesignatorFit>>,
    pub requests: usize,
    pub hits: usize,
    pub misses: usize,
}

impl DesignatorFitSession {
    pub fn fit(
        &mut self,
        designator: &str,
        region: &MultiPolygon<f64>,
        fill_ratio: f64,
        maximum_font_size_mm: f64,
        auto_base_rotation_degrees: f64,
        manual_rotation_degrees: Option<f64>,
    ) -> Option<DesignatorFit> {
        self.requests += 1;
        let bounds = region.bounding_rect()?;
        let anchor = [bounds.min().x, bounds.min().y];
        let relative = region.translate(-anchor[0], -anchor[1]);
        let rotations = match manual_rotation_degrees {
            Some(rotation) => vec![normalize_designator_axis(rotation)],
            None => {
                let primary = normalize_designator_axis(auto_base_rotation_degrees);
                let secondary = normalize_designator_axis(primary + 90.0);
                if (primary - secondary).abs() <= 1.0e-9 {
                    vec![primary]
                } else {
                    vec![primary, secondary]
                }
            }
        };
        let text_aspect = (designator.chars().count() as f64 * TEXT_ADVANCE_PER_CHARACTER).max(1.0);
        let key = FitKey {
            geometry: geometry_key(&relative),
            text_aspect: quantize(text_aspect),
            rotations: rotations.iter().copied().map(quantize).collect(),
            fill_ratio: quantize(fill_ratio.clamp(0.05, 1.0)),
            maximum_font_size_mm: quantize(maximum_font_size_mm.max(0.0)),
        };
        let relative_fit = if let Some(cached) = self.fits.get(&key) {
            self.hits += 1;
            *cached
        } else {
            self.misses += 1;
            let fitted = fit_uncached(
                &relative,
                text_aspect,
                fill_ratio.clamp(0.05, 1.0),
                maximum_font_size_mm.max(0.0),
                &rotations,
            );
            self.fits.insert(key, fitted);
            fitted
        }?;
        Some(DesignatorFit {
            center_mm: [
                relative_fit.center_mm[0] + anchor[0],
                relative_fit.center_mm[1] + anchor[1],
            ],
            ..relative_fit
        })
    }
}

pub(crate) fn normalize_designator_axis(angle_degrees: f64) -> f64 {
    let normalized = (angle_degrees + 90.0).rem_euclid(180.0) - 90.0;
    if normalized.abs() <= 1.0e-12 {
        0.0
    } else {
        normalized
    }
}

/// Combine already-closed projected rings with SVG's even-odd rule per layer,
/// then union the visible layers into one annotation region.
pub(crate) fn region_from_evenodd_layers(
    layers: impl IntoIterator<Item = Vec<Vec<[f64; 2]>>>,
) -> Option<MultiPolygon<f64>> {
    let mut visible_layers = Vec::new();
    for rings in layers {
        let mut layer = MultiPolygon::new(Vec::new());
        for ring in rings {
            let polygon = ring_polygon(&ring)?;
            layer = if layer.0.is_empty() {
                MultiPolygon::new(vec![polygon])
            } else {
                layer.xor(&polygon)
            };
        }
        if !layer.0.is_empty() {
            visible_layers.push(layer);
        }
    }
    if visible_layers.is_empty() {
        None
    } else {
        Some(unary_union(&visible_layers))
    }
}

pub(crate) fn pad_envelope(polygons: &[Polygon<f64>]) -> Option<MultiPolygon<f64>> {
    if polygons.is_empty() {
        return None;
    }
    Some(MultiPolygon::new(polygons.to_vec()).convex_hull().into())
}

pub(crate) fn ring_polygon(points: &[[f64; 2]]) -> Option<Polygon<f64>> {
    if points.len() < 3 {
        return None;
    }
    let mut coordinates = points
        .iter()
        .map(|point| Coord {
            x: point[0],
            y: point[1],
        })
        .collect::<Vec<_>>();
    if coordinates.first() != coordinates.last() {
        coordinates.push(coordinates[0]);
    }
    let polygon = Polygon::new(LineString::new(coordinates), Vec::new());
    (polygon.unsigned_area() > 1.0e-12).then_some(polygon)
}

fn fit_uncached(
    region: &MultiPolygon<f64>,
    text_aspect: f64,
    fill_ratio: f64,
    maximum_font_size_mm: f64,
    rotations: &[f64],
) -> Option<DesignatorFit> {
    let centers = candidate_centers(region);
    let mut best = None;
    for &rotation in rotations {
        let (raw_font, center) =
            largest_font_at_candidates(region, &centers, text_aspect, rotation)?;
        let candidate = DesignatorFit {
            center_mm: center,
            font_size_mm: maximum_font_size_mm.min(raw_font * fill_ratio),
            rotation_degrees: rotation,
        };
        if candidate.font_size_mm <= 0.0 {
            continue;
        }
        if best.is_none_or(|current: DesignatorFit| {
            candidate.font_size_mm > current.font_size_mm + 1.0e-6
                || ((candidate.font_size_mm - current.font_size_mm).abs() <= 1.0e-6
                    && candidate.rotation_degrees.abs() < current.rotation_degrees.abs())
        }) {
            best = Some(candidate);
        }
    }
    best
}

fn candidate_centers(region: &MultiPolygon<f64>) -> Vec<[f64; 2]> {
    let Some(bounds) = region.bounding_rect() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if let Some(point) = region.centroid() {
        add_candidate(region, &mut candidates, point);
    }
    if let Some(point) = region.interior_point() {
        add_candidate(region, &mut candidates, point);
    }
    add_candidate(region, &mut candidates, rect_center(bounds));
    let mut parts = region.0.iter().collect::<Vec<_>>();
    parts.sort_by(|left, right| {
        polygon_bounds_key(left)
            .partial_cmp(&polygon_bounds_key(right))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for part in parts {
        if let Some(point) = part.centroid() {
            add_candidate(region, &mut candidates, point);
        }
        if let Some(point) = part.interior_point() {
            add_candidate(region, &mut candidates, point);
        }
        if let Some(bounds) = part.bounding_rect() {
            add_candidate(region, &mut candidates, rect_center(bounds));
        }
    }
    for x_index in 1..CENTER_GRID_DIVISIONS {
        let x = bounds.min().x + bounds.width() * x_index as f64 / CENTER_GRID_DIVISIONS as f64;
        for y_index in 1..CENTER_GRID_DIVISIONS {
            let y =
                bounds.min().y + bounds.height() * y_index as f64 / CENTER_GRID_DIVISIONS as f64;
            add_candidate(region, &mut candidates, Point::new(x, y));
        }
    }
    candidates
}

fn add_candidate(region: &MultiPolygon<f64>, candidates: &mut Vec<[f64; 2]>, point: Point<f64>) {
    let candidate = [point.x(), point.y()];
    if region.covers(&point)
        && !candidates.iter().any(|existing| {
            (existing[0] - candidate[0]).abs() <= 1.0e-12
                && (existing[1] - candidate[1]).abs() <= 1.0e-12
        })
    {
        candidates.push(candidate);
    }
}

fn largest_font_at_candidates(
    region: &MultiPolygon<f64>,
    centers: &[[f64; 2]],
    text_aspect: f64,
    rotation_degrees: f64,
) -> Option<(f64, [f64; 2])> {
    let bounds = region.bounding_rect()?;
    let upper_bound = bounds.width().max(bounds.height());
    let mut best_font = 0.0;
    let mut best_center = *centers.first()?;
    for &center in centers {
        let mut low = 0.0;
        let mut high = upper_bound;
        for _ in 0..BINARY_SEARCH_STEPS {
            let font_size = (low + high) / 2.0;
            if text_box_fits(region, center, font_size, text_aspect, rotation_degrees) {
                low = font_size;
            } else {
                high = font_size;
            }
        }
        if low > best_font + 1.0e-9 {
            best_font = low;
            best_center = center;
        }
    }
    Some((best_font, best_center))
}

fn text_box_fits(
    region: &MultiPolygon<f64>,
    center: [f64; 2],
    font_size: f64,
    text_aspect: f64,
    rotation_degrees: f64,
) -> bool {
    let half_width = text_aspect * font_size / 2.0;
    let half_height = font_size / 2.0;
    let angle = rotation_degrees.to_radians();
    let (sin, cos) = angle.sin_cos();
    let corners = [
        [-half_width, -half_height],
        [half_width, -half_height],
        [half_width, half_height],
        [-half_width, half_height],
    ]
    .map(|point| {
        [
            center[0] + point[0] * cos - point[1] * sin,
            center[1] + point[0] * sin + point[1] * cos,
        ]
    });
    let rectangle = ring_polygon(&corners).expect("text rectangle has positive area");
    region.covers(&rectangle)
}

fn rect_center(bounds: Rect<f64>) -> Point<f64> {
    Point::new(
        (bounds.min().x + bounds.max().x) / 2.0,
        (bounds.min().y + bounds.max().y) / 2.0,
    )
}

fn polygon_bounds_key(polygon: &Polygon<f64>) -> [f64; 5] {
    let bounds = polygon
        .bounding_rect()
        .expect("non-empty fitted polygon has bounds");
    [
        bounds.min().x,
        bounds.min().y,
        bounds.max().x,
        bounds.max().y,
        polygon.unsigned_area(),
    ]
}

fn geometry_key(region: &MultiPolygon<f64>) -> Vec<i64> {
    let mut result = Vec::new();
    for polygon in &region.0 {
        append_ring_key(&mut result, polygon.exterior());
        result.push(i64::MIN);
        for interior in polygon.interiors() {
            append_ring_key(&mut result, interior);
            result.push(i64::MIN + 1);
        }
        result.push(i64::MAX);
    }
    result
}

fn append_ring_key(result: &mut Vec<i64>, ring: &LineString<f64>) {
    for coordinate in ring.coords() {
        result.push(quantize(coordinate.x));
        result.push(quantize(coordinate.y));
    }
}

fn quantize(value: f64) -> i64 {
    let scaled = (value * 1_000_000_000.0).round();
    if scaled == 0.0 { 0 } else { scaled as i64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Vec<[f64; 2]> {
        vec![
            [x, y],
            [x + width, y],
            [x + width, y + height],
            [x, y + height],
        ]
    }

    #[test]
    fn disconnected_body_fit_matches_autodoc_long_axis_and_cap_tie() {
        let region = region_from_evenodd_layers([vec![
            rectangle(0.0, 0.0, 1.0, 1.0),
            rectangle(10.0, 0.0, 1.0, 4.0),
        ]])
        .unwrap();
        let mut session = DesignatorFitSession::default();
        let fit = session.fit("R100", &region, 0.8, 2.5, 0.0, None).unwrap();
        assert_eq!(fit.rotation_degrees, -90.0);
        assert!(fit.center_mm[0] > 10.0 && fit.center_mm[0] < 11.0);
        assert!((fit.font_size_mm - 0.8).abs() < 0.002);
        let capped = session.fit("R100", &region, 0.8, 0.1, 0.0, None).unwrap();
        assert_eq!(capped.rotation_degrees, 0.0);
    }

    #[test]
    fn pad_envelope_and_relative_cache_match_autodoc() {
        let polygons = [
            ring_polygon(&rectangle(0.0, 0.0, 1.0, 1.0)).unwrap(),
            ring_polygon(&rectangle(3.0, 0.0, 1.0, 1.0)).unwrap(),
        ];
        let region = pad_envelope(&polygons).unwrap();
        let translated = region.translate(17.0, -23.0);
        let mut session = DesignatorFitSession::default();
        let first = session.fit("J1", &region, 0.8, 2.5, 0.0, None).unwrap();
        let second = session.fit("J1", &translated, 0.8, 2.5, 0.0, None).unwrap();
        assert!((first.center_mm[0] - 2.0).abs() < 1.0e-9);
        assert!((first.center_mm[1] - 0.5).abs() < 1.0e-9);
        assert!((second.center_mm[0] - 19.0).abs() < 1.0e-9);
        assert!((second.center_mm[1] + 22.5).abs() < 1.0e-9);
        assert_eq!(first.font_size_mm, second.font_size_mm);
        assert_eq!(session.hits, 1);
    }
}
