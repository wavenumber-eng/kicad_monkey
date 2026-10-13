//! Board plot operations and text as analytic shapes.
//!
//! Strokes stay exact: segments are capsules, arcs and cubics are swept
//! paths, and outlines are closed swept paths. A filled shape with a stroke
//! is its fill grown by half the stroke (a Minkowski sum with a disk), which
//! is exact for rectangles and circles and is a fill plus a closed stroke
//! for polygons. Stroke text uses core's Newstroke realization; outline text
//! uses its render cache.

use super::geometry::{mm, nm64};
use super::routing::polyline_contour;
use crate::board_plotter_ir::{
    BoardTextHAlign, BoardTextOperation, BoardTextRenderCacheCoordinateSpace, BoardTextVAlign,
};
use crate::newstroke::{NewstrokeLimits, NewstrokeRequest, realize_newstroke_kicad_a0};
use crate::plotter_types::{PlotterFill, PlotterOperation, PlotterText};
use crate::text_layout::{TextHorizontalAlignment, TextVerticalAlignment};
use kicad_monkey_contracts::generated::materialization_document::{
    Annulus2d, ArcSegment, Capsule2d, CubicSegment, Disk2d, LineSegment, Nm, NmPoint,
    NonNegativeNm, PathCap, PathJoin, PathSegment, PlanarRegion2d, Rect2d, RoundedRect2d, Shape2d,
    SweptPath2d,
};

/// First board format whose stroke-text thickness is a base width.
const BOLD_BASE_WIDTH_FORMAT: i64 = 20_260_826;

/// Shapes drawn by one operation, plus what could not be drawn.
#[derive(Default)]
pub(crate) struct Drawn {
    pub shapes: Vec<Shape2d>,
    /// Shapes already placed in the board frame (outline text caches).
    pub board_shapes: Vec<Shape2d>,
    /// Material removed from this item's own adds: knockout-text glyphs,
    /// in the item's frame and in the board frame.
    pub cuts: Vec<Shape2d>,
    pub board_cuts: Vec<Shape2d>,
    pub unsupported: Vec<(&'static str, String)>,
}

fn coordinate(value: i64) -> Nm {
    Nm(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

pub(crate) fn point(x: i64, y: i64) -> NmPoint {
    NmPoint([coordinate(x), coordinate(y)])
}

pub(crate) fn length(value: i64) -> NonNegativeNm {
    NonNegativeNm(value.max(0))
}

pub(crate) fn line_to(x: i64, y: i64) -> PathSegment {
    PathSegment::LineSegment(LineSegment {
        kind: "line".to_owned(),
        end_nm: point(x, y),
    })
}

pub(crate) fn swept(start: NmPoint, segments: Vec<PathSegment>, width: i64) -> Shape2d {
    Shape2d::SweptPath2d(SweptPath2d {
        kind: "swept_path_2d".to_owned(),
        start_nm: start,
        segments,
        width_nm: length(width),
        cap: PathCap::Round,
        join: PathJoin::Round,
    })
}

/// Closed stroke through `points`, back to the first.
pub(crate) fn closed_stroke(points: &[[i64; 2]], width: i64) -> Option<Shape2d> {
    let (first, rest) = points.split_first()?;
    if rest.is_empty() || width <= 0 {
        return None;
    }
    let segments = rest
        .iter()
        .chain(std::iter::once(first))
        .map(|p| line_to(p[0], p[1]))
        .collect();
    Some(swept(point(first[0], first[1]), segments, width))
}

fn region(points: &[[i64; 2]]) -> Option<Shape2d> {
    let contour = polyline_contour(points.iter().map(|p| point(p[0], p[1])).collect())?;
    Some(Shape2d::PlanarRegion2d(PlanarRegion2d {
        kind: "planar_region_2d".to_owned(),
        outer: contour,
        holes: Vec::new(),
    }))
}

fn filled(fill: PlotterFill, drawn: &mut Drawn) -> bool {
    match fill {
        PlotterFill::NoFill => false,
        PlotterFill::Hatch | PlotterFill::ReverseHatch => {
            drawn.unsupported.push((
                "hatch_fill_not_materialized",
                "hatched fill is drawn as its outline only".to_owned(),
            ));
            false
        }
        _ => true,
    }
}

pub(crate) fn circle(cx: i64, cy: i64, radius: i64, width: i64, solid: bool) -> Option<Shape2d> {
    let outer = radius + width.max(0) / 2;
    if outer <= 0 {
        return None;
    }
    let inner = radius - width / 2;
    if solid || width <= 0 || inner <= 0 {
        return (solid || width > 0).then(|| {
            Shape2d::Disk2d(Disk2d {
                kind: "disk_2d".to_owned(),
                center_nm: point(cx, cy),
                radius_nm: length(outer),
            })
        });
    }
    Some(Shape2d::Annulus2d(Annulus2d {
        kind: "annulus_2d".to_owned(),
        center_nm: point(cx, cy),
        outer_radius_nm: length(outer),
        inner_radius_nm: length(inner),
    }))
}

/// Outline of a rectangle with optional rounded corners, clockwise in
/// screen terms from the top-left corner's end of the top edge.
fn rect_outline(x1: i64, y1: i64, x2: i64, y2: i64, radius: i64) -> (NmPoint, Vec<PathSegment>) {
    let (left, right, top, bottom) = (x1.min(x2), x1.max(x2), y1.min(y2), y1.max(y2));
    if radius <= 0 {
        let segments = [(right, top), (right, bottom), (left, bottom), (left, top)]
            .into_iter()
            .map(|(x, y)| line_to(x, y))
            .collect();
        return (point(left, top), segments);
    }
    let bulge = (radius as f64 * (1.0 - std::f64::consts::FRAC_1_SQRT_2)).round() as i64;
    let arc = |mid: (i64, i64), end: (i64, i64)| {
        PathSegment::ArcSegment(ArcSegment {
            kind: "arc".to_owned(),
            mid_nm: point(mid.0, mid.1),
            end_nm: point(end.0, end.1),
        })
    };
    let segments = vec![
        line_to(right - radius, top),
        arc((right - bulge, top + bulge), (right, top + radius)),
        line_to(right, bottom - radius),
        arc((right - bulge, bottom - bulge), (right - radius, bottom)),
        line_to(left + radius, bottom),
        arc((left + bulge, bottom - bulge), (left, bottom - radius)),
        line_to(left, top + radius),
        arc((left + bulge, top + bulge), (left + radius, top)),
    ];
    (point(left + radius, top), segments)
}

/// A rectangle with optional rounded corners: its fill grown by half the
/// stroke when solid, or its outline swept by the stroke.
pub(crate) fn rect_shapes(
    corners: [i64; 4],
    radius: i64,
    stroke: i64,
    solid: bool,
) -> Option<Shape2d> {
    let [x1, y1, x2, y2] = corners;
    let (width, height) = ((x2 - x1).abs(), (y2 - y1).abs());
    let stroke = stroke.max(0);
    let radius = radius.clamp(0, width.min(height) / 2);
    if solid {
        let center = point((x1 + x2) / 2, (y1 + y2) / 2);
        let (width, height, radius) = (width + stroke, height + stroke, radius + stroke / 2);
        return Some(if radius > 0 {
            Shape2d::RoundedRect2d(RoundedRect2d {
                kind: "rounded_rect_2d".to_owned(),
                center_nm: center,
                width_nm: length(width),
                height_nm: length(height),
                radius_nm: length(radius),
            })
        } else {
            Shape2d::Rect2d(Rect2d {
                kind: "rect_2d".to_owned(),
                center_nm: center,
                width_nm: length(width),
                height_nm: length(height),
            })
        });
    }
    (stroke > 0).then(|| {
        let (start, segments) = rect_outline(x1, y1, x2, y2, radius);
        swept(start, segments, stroke)
    })
}

fn rect(value: &crate::plotter_types::PlotterRect, drawn: &mut Drawn) {
    let solid = filled(value.fill, drawn);
    let corners = [value.x1, value.y1, value.x2, value.y2];
    drawn.shapes.extend(rect_shapes(
        corners,
        value.corner_radius_nm,
        value.width_nm,
        solid,
    ));
}

fn poly(value: &crate::plotter_types::PlotterPoly, drawn: &mut Drawn) {
    if filled(value.fill, drawn) {
        drawn.shapes.extend(region(&value.points));
    }
    drawn
        .shapes
        .extend(closed_stroke(&value.points, value.width_nm));
}

/// Shapes of one plot operation in its own frame.
pub(crate) fn operation_shapes(operation: &PlotterOperation, drawn: &mut Drawn) {
    match operation {
        PlotterOperation::ThickSegment(value) if value.width_nm > 0 => {
            drawn.shapes.push(Shape2d::Capsule2d(Capsule2d {
                kind: "capsule_2d".to_owned(),
                start_nm: point(value.start_x, value.start_y),
                end_nm: point(value.end_x, value.end_y),
                radius_nm: length(value.width_nm / 2),
            }));
        }
        PlotterOperation::ArcThreePoint(value) if value.width_nm > 0 => {
            let segment = PathSegment::ArcSegment(ArcSegment {
                kind: "arc".to_owned(),
                mid_nm: point(value.mid_x, value.mid_y),
                end_nm: point(value.end_x, value.end_y),
            });
            let start = point(value.start_x, value.start_y);
            drawn
                .shapes
                .push(swept(start, vec![segment], value.width_nm));
        }
        PlotterOperation::Circle(value) => {
            let solid = filled(value.fill, drawn);
            let radius = value.diameter_nm / 2;
            drawn
                .shapes
                .extend(circle(value.cx, value.cy, radius, value.width_nm, solid));
        }
        PlotterOperation::Rect(value) => rect(value, drawn),
        PlotterOperation::PlotPoly(value) => poly(value, drawn),
        PlotterOperation::BezierCurve(value) if value.width_nm > 0 => {
            let segment = PathSegment::CubicSegment(CubicSegment {
                kind: "cubic".to_owned(),
                control1_nm: point(value.ctrl1_x, value.ctrl1_y),
                control2_nm: point(value.ctrl2_x, value.ctrl2_y),
                end_nm: point(value.end_x, value.end_y),
            });
            let start = point(value.start_x, value.start_y);
            drawn
                .shapes
                .push(swept(start, vec![segment], value.width_nm));
        }
        PlotterOperation::Text(value) => text_shapes(&TextSpec::from_plotter(value), drawn),
        _ => {}
    }
}

/// The text fields shared by board and plotter text operations.
pub(crate) struct TextSpec<'a> {
    text: &'a str,
    x: i64,
    y: i64,
    size: (i64, i64),
    orient_deg: f64,
    align: (TextHorizontalAlignment, TextVerticalAlignment),
    pen_width_nm: i64,
    style: (bool, bool, bool),
    knockout: bool,
    cache: Option<&'a crate::board_plotter_ir::BoardTextRenderCache>,
    /// Whether `x`, `y`, and `orient_deg` are already in the board frame.
    board_frame: bool,
    /// Whether bold multiplies a stroke font's stored base thickness.
    bold_scales_pen: bool,
    /// Outline font face, empty for KiCad's stroke font.
    font_face: &'a str,
}

/// Where KiCad draws `operation`: text-box text carries KiCad's own anchor;
/// other text uses the plot-document position.
pub(crate) fn text_anchor(operation: &BoardTextOperation) -> [i64; 2] {
    operation
        .kicad
        .as_ref()
        .map_or([operation.x, operation.y], |kicad| kicad.anchor)
}

impl<'a> TextSpec<'a> {
    pub(crate) fn from_board(value: &'a BoardTextOperation) -> Self {
        let [x, y] = text_anchor(value);
        let kicad = value.kicad.as_ref();
        Self {
            text: kicad.map_or(&value.text, |kicad| &kicad.text),
            x,
            y,
            size: (value.size_x_nm, value.size_y_nm),
            orient_deg: value.orient_deg,
            align: (
                match value.h_align {
                    BoardTextHAlign::Left => TextHorizontalAlignment::Left,
                    BoardTextHAlign::Center => TextHorizontalAlignment::Center,
                    BoardTextHAlign::Right => TextHorizontalAlignment::Right,
                },
                match value.v_align {
                    BoardTextVAlign::Top => TextVerticalAlignment::Top,
                    BoardTextVAlign::Center => TextVerticalAlignment::Center,
                    BoardTextVAlign::Bottom => TextVerticalAlignment::Bottom,
                },
            ),
            pen_width_nm: value.pen_width_nm,
            style: (
                kicad.map_or(value.mirror, |kicad| kicad.mirrored),
                value.italic,
                value.bold,
            ),
            knockout: value.knockout,
            cache: value.render_cache.as_ref(),
            board_frame: false,
            bold_scales_pen: false,
            font_face: &value.font_face,
        }
    }

    /// Apply the board file format's thickness convention: from format
    /// 20260826 a stroke font's thickness is a base width that bold widens
    /// 1.6 times; older files store the drawn width.
    pub(crate) fn format(mut self, version: i64) -> Self {
        self.bold_scales_pen = version >= BOLD_BASE_WIDTH_FORMAT;
        self
    }

    /// KiCad's effective text pen (`EDA_TEXT::GetEffectiveTextPenWidth`):
    /// the authored thickness, or width / 5 bold and width / 8 normal when
    /// none is set; under [`Self::format`] bold stroke text is drawn 1.6
    /// times wider; the result is clamped to a quarter of the smaller size.
    pub(crate) fn kicad_pen_nm(&self, stroke_font: bool) -> i64 {
        let (bold, width) = (self.style.2, self.size.0.abs());
        let pen = if self.pen_width_nm <= 1 {
            if bold { width / 5 } else { width / 8 }
        } else if bold && stroke_font && self.bold_scales_pen {
            (self.pen_width_nm as f64 * 1.6).round() as i64
        } else {
            self.pen_width_nm
        };
        let clamp = (self.size.0.abs().min(self.size.1.abs()) as f64 * 0.25).round() as i64;
        pen.min(clamp)
    }

    /// Mark the text as knockout when its source says so.
    pub(crate) fn knockout(mut self, authored: bool) -> Self {
        self.knockout |= authored;
        self
    }

    /// KiCad's alignment and mirroring from authored `justify` tokens:
    /// centred on both axes unless a side is named.
    pub(crate) fn justified(mut self, justify: &[String]) -> Self {
        let has = |token: &str| justify.iter().any(|value| value == token);
        self.align.0 = if has("left") {
            TextHorizontalAlignment::Left
        } else if has("right") {
            TextHorizontalAlignment::Right
        } else {
            TextHorizontalAlignment::Center
        };
        self.align.1 = if has("top") {
            TextVerticalAlignment::Top
        } else if has("bottom") {
            TextVerticalAlignment::Bottom
        } else {
            TextVerticalAlignment::Center
        };
        self.style.0 = self.style.0 || has("mirror");
        self
    }

    /// Footprint stroke text placed in the board frame the way KiCad draws
    /// it: the anchor moves with the footprint, the angle is the board-frame
    /// text angle, kept within (-90, 90] unless the text is unlocked.
    pub(crate) fn on_board(mut self, x: i64, y: i64, angle_deg: f64, unlocked: bool) -> Self {
        self.x = x;
        self.y = y;
        self.orient_deg = if unlocked {
            angle_deg
        } else {
            keep_upright(angle_deg)
        };
        self.board_frame = true;
        self
    }

    fn from_plotter(value: &'a PlotterText) -> Self {
        use crate::plotter_types::{PlotterTextHAlign, PlotterTextVAlign};
        Self {
            text: &value.text,
            x: value.x,
            y: value.y,
            size: (value.size_x_nm, value.size_y_nm),
            orient_deg: value.orient_deg,
            align: (
                match value.h_align {
                    PlotterTextHAlign::Left => TextHorizontalAlignment::Left,
                    PlotterTextHAlign::Center => TextHorizontalAlignment::Center,
                    PlotterTextHAlign::Right => TextHorizontalAlignment::Right,
                },
                match value.v_align {
                    PlotterTextVAlign::Top => TextVerticalAlignment::Top,
                    PlotterTextVAlign::Center => TextVerticalAlignment::Center,
                    PlotterTextVAlign::Bottom => TextVerticalAlignment::Bottom,
                },
            ),
            pen_width_nm: value.pen_width_nm,
            style: (value.mirror, value.italic, value.bold),
            knockout: false,
            cache: None,
            board_frame: false,
            bold_scales_pen: false,
            font_face: &value.font_face,
        }
    }
}

/// KiCad's keep-upright rule: the angle folded into (-90, 90].
fn keep_upright(angle: f64) -> f64 {
    let mut angle = angle.rem_euclid(360.0);
    while angle > 90.0 {
        angle -= 180.0;
    }
    while angle <= -90.0 {
        angle += 180.0;
    }
    angle
}

/// Glyph shapes of one text item and the points that bound them.
struct Glyphs {
    shapes: Vec<Shape2d>,
    /// Cached outline contours, without any knockout box.
    rings: Vec<Vec<[i64; 2]>>,
    points: Vec<(i64, i64)>,
    pen_nm: i64,
    board_frame: bool,
}

/// Outline text from its render cache: one region per glyph polygon, the
/// first contour outer and the rest holes.
fn cached_glyphs(cache: &crate::board_plotter_ir::BoardTextRenderCache) -> Glyphs {
    let mut glyphs = Glyphs {
        shapes: Vec::new(),
        rings: Vec::new(),
        points: Vec::new(),
        pen_nm: 0,
        board_frame: cache.coordinate_space != BoardTextRenderCacheCoordinateSpace::FootprintLocal,
    };
    for (index, polygon) in cache.polygons.iter().enumerate() {
        // A knockout cache from core starts with its own background box,
        // which is rebuilt here with KiCad's margin.
        let skip = usize::from(cache.knockout && index == 0);
        let rings = &polygon[skip.min(polygon.len())..];
        glyphs
            .points
            .extend(rings.iter().flatten().map(|p| (p[0], p[1])));
        glyphs.shapes.extend(even_odd_regions(rings));
        glyphs.rings.extend(rings.iter().cloned());
    }
    glyphs
}

/// Whether `p` lies inside the closed ring (even-odd ray test).
fn inside(p: [i64; 2], ring: &[[i64; 2]]) -> bool {
    let (px, py) = (p[0] as f64, p[1] as f64);
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        let ((x1, y1), (x2, y2)) = (
            (previous[0] as f64, previous[1] as f64),
            (current[0] as f64, current[1] as f64),
        );
        if (y1 > py) != (y2 > py) && px < x1 + (py - y1) * (x2 - x1) / (y2 - y1) {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// Regions of contours filled by the even-odd rule. A contour nested an
/// even number of levels deep is an outer boundary; an odd one is a hole of
/// the innermost contour around it. Contours are assumed not to cross,
/// which holds for render caches.
fn even_odd_regions(rings: &[Vec<[i64; 2]>]) -> Vec<Shape2d> {
    let rings: Vec<&Vec<[i64; 2]>> = rings.iter().filter(|ring| ring.len() >= 3).collect();
    let containers = |index: usize| -> Vec<usize> {
        (0..rings.len())
            .filter(|&other| other != index && inside(rings[index][0], rings[other]))
            .collect()
    };
    let depth: Vec<Vec<usize>> = (0..rings.len()).map(containers).collect();
    let contour =
        |ring: &Vec<[i64; 2]>| polyline_contour(ring.iter().map(|p| point(p[0], p[1])).collect());
    let mut regions = Vec::new();
    for (index, ring) in rings.iter().enumerate() {
        if depth[index].len() % 2 == 1 {
            continue;
        }
        let Some(outer) = contour(ring) else {
            continue;
        };
        // Holes: odd-depth contours whose innermost container is this ring.
        let holes = (0..rings.len())
            .filter(|&hole| {
                depth[hole].len() == depth[index].len() + 1 && depth[hole].contains(&index)
            })
            .filter_map(|hole| contour(rings[hole]))
            .collect();
        regions.push(Shape2d::PlanarRegion2d(PlanarRegion2d {
            kind: "planar_region_2d".to_owned(),
            outer,
            holes,
        }));
    }
    regions
}

/// Newstroke glyphs: one swept path per polyline, a dot for a single point.
fn stroke_glyphs(spec: &TextSpec<'_>) -> Result<Glyphs, String> {
    let request = NewstrokeRequest {
        text: spec.text,
        position_x_mm: mm(spec.x),
        position_y_mm: mm(spec.y),
        size_x_mm: mm(spec.size.0),
        size_y_mm: mm(spec.size.1),
        angle_degrees: spec.orient_deg,
        horizontal_alignment: spec.align.0,
        vertical_alignment: spec.align.1,
        mirrored: spec.style.0,
        italic: spec.style.1,
        bold: spec.style.2,
        stroke_width_mm: Some(mm(spec.kicad_pen_nm(true))),
    };
    let realized = realize_newstroke_kicad_a0(request, NewstrokeLimits::default())
        .map_err(|cause| format!("text {:?}: {}", spec.text, cause.message))?;
    let width = nm64(realized.effective_stroke_width_mm);
    let mut glyphs = Glyphs {
        shapes: Vec::new(),
        points: Vec::new(),
        rings: Vec::new(),
        pen_nm: width,
        board_frame: spec.board_frame,
    };
    for polyline in realized.polylines {
        let points: Vec<(i64, i64)> = polyline
            .points
            .iter()
            .map(|p| (nm64(p.x_mm), nm64(p.y_mm)))
            .collect();
        let Some((&(x, y), rest)) = points.split_first() else {
            continue;
        };
        if rest.is_empty() {
            glyphs.shapes.push(Shape2d::Disk2d(Disk2d {
                kind: "disk_2d".to_owned(),
                center_nm: point(x, y),
                radius_nm: length(width / 2),
            }));
        } else if width > 0 {
            let segments = rest.iter().map(|&(x, y)| line_to(x, y)).collect();
            glyphs.shapes.push(swept(point(x, y), segments, width));
        }
        glyphs.points.extend(points);
    }
    Ok(glyphs)
}

/// KiCad's knockout box (`PCB_TEXT::buildBoundingHull`): the glyph bounds
/// in the text's own rotated frame, grown by half the drawn stroke and by
/// `max(pen / 2, height / 9)` with KiCad's effective pen.
fn knockout_corners(spec: &TextSpec<'_>, glyphs: &Glyphs, pen_nm: i64) -> Option<[[i64; 2]; 4]> {
    let angle = spec.orient_deg.to_radians();
    let (sin, cos) = angle.sin_cos();
    let (px, py) = (spec.x as f64, spec.y as f64);
    // Newstroke places local (x, y) at R(-angle)(x, y) + position, so
    // R(angle) undoes it.
    let local = glyphs.points.iter().map(|&(x, y)| {
        let (dx, dy) = (x as f64 - px, y as f64 - py);
        (dx * cos - dy * sin, dx * sin + dy * cos)
    });
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in local {
        (min_x, min_y, max_x, max_y) = (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y));
    }
    if min_x > max_x {
        return None;
    }
    let margin = (pen_nm as f64 / 2.0)
        .round()
        .max((spec.size.1 as f64 / 9.0).round())
        + glyphs.pen_nm as f64 / 2.0;
    let corners = [
        (min_x - margin, min_y - margin),
        (max_x + margin, min_y - margin),
        (max_x + margin, max_y + margin),
        (min_x - margin, max_y + margin),
    ]
    .map(|(x, y)| {
        [
            (x * cos + y * sin + px).round() as i64,
            (-x * sin + y * cos + py).round() as i64,
        ]
    });
    Some(corners)
}

/// Report what Newstroke text realization cannot reproduce.
fn text_limitations(spec: &TextSpec<'_>, drawn: &mut Drawn) {
    if spec.cache.is_some() {
        return;
    }
    if !spec.font_face.is_empty() {
        drawn.unsupported.push((
            "outline_font_drawn_as_stroke",
            format!(
                "text {:?} in font {:?} has no render cache and is drawn in the stroke font",
                spec.text, spec.font_face
            ),
        ));
    }
}

/// Shapes of one text item: cached outline glyphs or Newstroke strokes. A
/// knockout item is its knockout box, with the glyphs as cuts.
pub(crate) fn text_shapes(spec: &TextSpec<'_>, drawn: &mut Drawn) {
    text_limitations(spec, drawn);
    let glyphs = match spec.cache {
        Some(cache) => cached_glyphs(cache),
        None => match stroke_glyphs(spec) {
            Ok(glyphs) => glyphs,
            Err(message) => {
                drawn.unsupported.push(("text_not_realized", message));
                return;
            }
        },
    };
    let (adds, cuts) = if glyphs.board_frame {
        (&mut drawn.board_shapes, &mut drawn.board_cuts)
    } else {
        (&mut drawn.shapes, &mut drawn.cuts)
    };
    let cached = spec.cache.is_some();
    let knockout = spec.knockout || spec.cache.is_some_and(|cache| cache.knockout);
    if !knockout {
        adds.extend(glyphs.shapes);
        return;
    }
    let Some(corners) = knockout_corners(spec, &glyphs, spec.kicad_pen_nm(!cached)) else {
        return;
    };
    if cached {
        // Outline glyphs become holes of the box, exactly as KiCad does.
        let mut rings = vec![corners.to_vec()];
        rings.extend(glyphs.rings);
        adds.extend(even_odd_regions(&rings));
    } else {
        // Stroke glyphs have no closed-form outline, so they are cut out.
        adds.extend(region(&corners));
        cuts.extend(glyphs.shapes);
    }
}
