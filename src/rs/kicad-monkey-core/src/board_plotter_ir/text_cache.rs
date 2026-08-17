//! Authored board-text render-cache decoding and knockout restructuring.

use super::point_limit_error;
use super::text::{BoardTextOperation, BoardTextRenderCache};
use crate::plotter_ir::{child, ensure_javascript_safe_integer, mm_to_nm, model_error, numeric_at};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position, Sexp};

fn resource_limit_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}

/// Authored `(render_cache "text" angle (polygon (pts ...) ...) ...)` facts
/// in raw mm coordinates.
pub(super) struct AuthoredRenderCache {
    /// `None` marks a non-string cache text token, which can never match a
    /// resolved request text.
    text: Option<String>,
    angle: f64,
    polygons: Vec<Vec<Vec<[f64; 2]>>>,
}

impl AuthoredRenderCache {
    pub(super) fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub(super) fn ensure_knockout_limits(
        &self,
        max_points: usize,
        max_contours: usize,
    ) -> Result<(), Error> {
        let (contours, points) = self
            .polygons
            .iter()
            .flat_map(|polygon| polygon.iter())
            .filter(|contour| contour.len() >= 3)
            .try_fold((0usize, 0usize), |(contours, points), contour| {
                Some((contours.checked_add(1)?, points.checked_add(contour.len())?))
            })
            .ok_or_else(point_limit_error)?;
        if contours.saturating_add(1) > max_contours {
            return Err(resource_limit_error(
                "Board text knockout exceeds max_cache_contours",
            ));
        }
        if points.saturating_add(8) > max_points {
            return Err(point_limit_error());
        }
        Ok(())
    }
}

fn list_values(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text_value(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn children<'a>(form: &'a Sexp, head: &'a str) -> impl Iterator<Item = &'a Sexp> + 'a {
    list_values(form)
        .into_iter()
        .flatten()
        .filter(move |value| {
            list_values(value)
                .and_then(|values| values.first())
                .and_then(text_value)
                == Some(head)
        })
}

/// Python `RenderCache.from_sexp`: `None` below three header values.
pub(super) fn parse_render_cache(
    form: &Sexp,
    max_points: usize,
    max_polygons: usize,
    max_contours: usize,
) -> Result<Option<AuthoredRenderCache>, Error> {
    let Some(cache) = child(form, "render_cache") else {
        return Ok(None);
    };
    let Some(values) = list_values(cache).filter(|values| values.len() >= 3) else {
        return Ok(None);
    };
    let text = match &values[1] {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value.clone()),
        // Python `unquote_string` stringifies scalar tokens.
        Sexp::Integer(value) => Some(value.to_string()),
        _ => None,
    };
    let angle = numeric_at(cache, 2, Position::START)?;
    let mut polygons = Vec::new();
    let mut point_count = 0_usize;
    let mut polygon_count = 0_usize;
    let mut contour_count = 0_usize;
    for polygon in children(cache, "polygon") {
        polygon_count = polygon_count
            .checked_add(1)
            .filter(|count| *count <= max_polygons)
            .ok_or_else(|| resource_limit_error("Board text cache exceeds max_cache_polygons"))?;
        let mut contours = Vec::new();
        for points in children(polygon, "pts") {
            contour_count = contour_count
                .checked_add(1)
                .filter(|count| *count <= max_contours)
                .ok_or_else(|| {
                    resource_limit_error("Board text cache exceeds max_cache_contours")
                })?;
            let mut contour = Vec::new();
            for point in children(points, "xy") {
                // Python skips xy forms without both coordinates.
                if list_values(point).is_some_and(|values| values.len() >= 3) {
                    point_count = point_count
                        .checked_add(1)
                        .filter(|count| *count <= max_points)
                        .ok_or_else(point_limit_error)?;
                    contour.push([
                        numeric_at(point, 1, Position::START)?,
                        numeric_at(point, 2, Position::START)?,
                    ]);
                }
            }
            contours.push(contour);
        }
        polygons.push(contours);
    }
    Ok(Some(AuthoredRenderCache {
        text,
        angle,
        polygons,
    }))
}

/// Python `math.isclose` with the default 1e-9 relative and absolute
/// tolerances used by `RenderCacheRequest`.
fn python_isclose(left: f64, right: f64) -> bool {
    let tolerance = (1e-9 * left.abs().max(right.abs())).max(1e-9);
    (left - right).abs() <= tolerance
}

/// Python `RenderCacheResolver.validate_cache` reasons for board requests,
/// which always carry an angle and never a mirror/offset context.
pub(super) fn cache_is_valid(
    cache: &AuthoredRenderCache,
    request_text: &str,
    request_angle: f64,
) -> bool {
    cache.text.as_deref() == Some(request_text)
        && python_isclose(cache.angle, request_angle)
        // Python: empty polygons invalidate the cache for nonempty text.
        && (request_text.is_empty() || !cache.polygons.is_empty())
        && cache.polygons.iter().all(|contours| {
            contours.first().is_some_and(|exterior| exterior.len() >= 3)
                && contours.iter().skip(1).all(|hole| hole.len() >= 3)
        })
}

/// Table-cell cache requests intentionally omit angle context. Python accepts
/// any authored cache angle in that case and marks the attachment inexact.
pub(super) fn cache_is_valid_without_angle(
    cache: &AuthoredRenderCache,
    request_text: &str,
) -> bool {
    cache.text.as_deref() == Some(request_text)
        && (request_text.is_empty() || !cache.polygons.is_empty())
        && cache.polygons.iter().all(|contours| {
            contours.first().is_some_and(|exterior| exterior.len() >= 3)
                && contours.iter().skip(1).all(|hole| hole.len() >= 3)
        })
}

/// Python `_render_cache_polygons_nm` + `_op_with_render_cache_payload`
/// for a valid authored cache; `exact` is false only under the font-face
/// warning because board requests always provide an angle.
pub(super) fn attach_authored_cache(
    operation: &mut BoardTextOperation,
    cache: &AuthoredRenderCache,
    exact: bool,
    max_retained_points: usize,
    knockout: bool,
) -> Result<(), Error> {
    let retained_points = cache.polygons.iter().try_fold(0usize, |total, polygon| {
        let contour_points = polygon
            .iter()
            .try_fold(0usize, |count, contour| count.checked_add(contour.len()))?;
        total
            .checked_add(contour_points)?
            .checked_add(polygon.first().map_or(0, Vec::len))
    });
    if !knockout && retained_points.is_none_or(|points| points > max_retained_points) {
        return Err(point_limit_error());
    }
    let mut typed = Vec::new();
    let mut exteriors = Vec::new();
    for polygon in &cache.polygons {
        let mut contours = Vec::new();
        for contour in polygon {
            if contour.len() < 3 {
                continue;
            }
            let points = contour
                .iter()
                .map(|[x, y]| Ok([mm_to_nm(*x)?, mm_to_nm(*y)?]))
                .collect::<Result<Vec<_>, Error>>()?;
            contours.push(points);
        }
        if contours.is_empty() {
            continue;
        }
        if !knockout {
            exteriors.push(contours[0].clone());
        }
        typed.push(contours);
    }
    if typed.is_empty() {
        return Ok(());
    }
    operation.render_cache_polygons = exteriors;
    operation.render_cache = Some(BoardTextRenderCache {
        text: cache.text.clone().unwrap_or_default(),
        angle: cache.angle,
        exact,
        knockout: false,
        polygons: typed,
    });
    Ok(())
}

/// Python `_apply_knockout_to_text_op`: coalesce every glyph contour under
/// one polygon whose first contour is the margin-inflated background rect.
pub(super) fn apply_knockout(
    operation: &mut BoardTextOperation,
    margin_nm: i64,
    max_points: usize,
    max_contours: usize,
) -> Result<(), Error> {
    let Some(cache) = operation.render_cache.as_mut() else {
        return Ok(());
    };
    if cache.polygons.is_empty() {
        return Ok(());
    }
    let mut bounds: Option<[i64; 4]> = None;
    let mut glyph_contour_count = 0usize;
    let mut glyph_point_count = 0usize;
    for polygon in &cache.polygons {
        for contour in polygon {
            if contour.len() < 3 {
                continue;
            }
            for point in contour {
                bounds = Some(match bounds {
                    None => [point[0], point[1], point[0], point[1]],
                    Some([min_x, min_y, max_x, max_y]) => [
                        min_x.min(point[0]),
                        min_y.min(point[1]),
                        max_x.max(point[0]),
                        max_y.max(point[1]),
                    ],
                });
            }
            glyph_contour_count = glyph_contour_count.saturating_add(1);
            glyph_point_count = glyph_point_count.saturating_add(contour.len());
        }
    }
    let Some([min_x, min_y, max_x, max_y]) = bounds else {
        return Ok(());
    };
    let sub = |value: i64| {
        value
            .checked_sub(margin_nm)
            .ok_or_else(|| model_error("Board text knockout coordinate overflow", Position::START))
            .and_then(ensure_javascript_safe_integer)
    };
    let add = |value: i64| {
        value
            .checked_add(margin_nm)
            .ok_or_else(|| model_error("Board text knockout coordinate overflow", Position::START))
            .and_then(ensure_javascript_safe_integer)
    };
    let left = sub(min_x)?;
    let top = sub(min_y)?;
    let right = add(max_x)?;
    let bottom = add(max_y)?;
    if glyph_contour_count.saturating_add(1) > max_contours {
        return Err(resource_limit_error(
            "Board text knockout exceeds max_cache_contours",
        ));
    }
    if glyph_point_count.saturating_add(8) > max_points {
        return Err(point_limit_error());
    }
    let background = vec![[left, top], [right, top], [right, bottom], [left, bottom]];
    let mut contours = Vec::with_capacity(glyph_contour_count + 1);
    contours.push(background.clone());
    for polygon in std::mem::take(&mut cache.polygons) {
        contours.extend(polygon.into_iter().filter(|contour| contour.len() >= 3));
    }
    cache.polygons = vec![contours];
    cache.knockout = true;
    operation.knockout = true;
    operation.render_cache_polygons = vec![background];
    Ok(())
}
