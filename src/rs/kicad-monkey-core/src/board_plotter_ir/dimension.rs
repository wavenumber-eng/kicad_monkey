//! Board dimension geometry and text emission.

use super::stroke_font_widths::{
    NEWSTROKE_GLYPH_DATA, NEWSTROKE_GLYPH_OFFSETS, NEWSTROKE_WIDTH_UNITS,
};
use super::text::{
    BoardTextHAlign, BoardTextVAlign, TextEffects, attach_gr_text_cache, gr_text_operation,
    has_flag, numeric_or, operation_text_bytes, parse_graphic_span, text_effects, text_point_total,
};
use super::text_cache::{cache_is_valid, parse_render_cache};
use super::{
    BoardDimensionOperation, BoardDimensionRecord, BoardPlotLimits, BudgetTracker, text_limit_error,
};
use crate::TextContourErrorKind;
use crate::pcb::{PcbDimension, PcbPoint};
use crate::plotter_ir::{child, mm_to_nm, model_error};
use crate::plotter_text_cache::PlotterTextCacheSession;
use crate::plotter_types::{PlotterCircle, PlotterFill, PlotterOperation, ThickSegment};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position};
use crate::text_markup::{TextMarkupMarker, TextMarkupNode, parse_text_markup};

const ARROW_ANGLE_DEG: f64 = 27.5;
const INWARD_ARROW_TAIL_RATIO: f64 = 2.0;
const TEXT_MARGIN_RATIO: f64 = 0.625;
/// Fixed guard against precision-driven allocation, independent of a caller's
/// aggregate text budget.
const MAX_DIMENSION_PRECISION: usize = 4_096;
const STROKE_SCALE: f64 = 1.0 / 21.0;
const FONT_OFFSET: f64 = -8.0;
const ITALIC_TILT: f64 = 1.0 / 8.0;
const SUPER_SUB_SIZE_MULTIPLIER: f64 = 0.8;
const SUPER_HEIGHT_OFFSET: f64 = 0.35;
const SUB_HEIGHT_OFFSET: f64 = 0.15;
const OVERBAR_POSITION_FACTOR: f64 = 1.23;
const OVERBAR_TRIM_RATIO: f64 = 0.1;

#[derive(Clone, Copy)]
struct Vec2 {
    x: f64,
    y: f64,
}

impl From<PcbPoint> for Vec2 {
    fn from(value: PcbPoint) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl Vec2 {
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    fn resized(self, length: f64) -> Self {
        let norm = self.length();
        if norm == 0.0 {
            Self { x: 0.0, y: 0.0 }
        } else {
            Self {
                x: self.x * length / norm,
                y: self.y * length / norm,
            }
        }
    }

    fn angle(self) -> f64 {
        if self.x == 0.0 && self.y == 0.0 {
            0.0
        } else {
            self.y.atan2(self.x).to_degrees()
        }
    }

    fn rotate_kicad(self, angle_deg: f64) -> Self {
        let radians = angle_deg.to_radians();
        let sin = radians.sin();
        let cos = radians.cos();
        Self {
            x: self.y * sin + self.x * cos,
            y: self.y * cos - self.x * sin,
        }
    }
}

pub(super) fn formatted_value(dimension: &PcbDimension, max_bytes: usize) -> Result<String, Error> {
    let mut value = measured_value(dimension);
    if dimension.format.units == 0 {
        value /= 25.4;
    } else if dimension.format.units == 1 {
        value /= 0.0254;
    }
    if !value.is_finite() {
        return Err(model_error(
            "Dimension measurement is not finite",
            Position::START,
        ));
    }
    let mut precision = dimension.format.precision;
    if precision >= 6 {
        precision -= match dimension.format.units {
            1 => 7,
            2 => 5,
            _ => 4,
        };
    }
    let precision = usize::try_from(precision.max(0)).map_err(|_| text_limit_error())?;
    if precision > max_bytes || precision > MAX_DIMENSION_PRECISION {
        return Err(text_limit_error());
    }
    let mut text = format!("{value:.precision$}");
    if text.len() > max_bytes {
        return Err(text_limit_error());
    }
    if dimension.format.suppress_zeroes {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if let Some(override_value) = &dimension.format.override_value {
        if override_value.len() > max_bytes {
            return Err(text_limit_error());
        }
        text.clone_from(override_value);
    }
    match dimension.format.units_format {
        1 => text.push_str(unit_suffix(dimension.format.units)),
        2 => {
            text.push_str(" (");
            text.push_str(unit_suffix(dimension.format.units).trim_start());
            text.push(')');
        }
        _ => {}
    }
    let output_len = dimension
        .format
        .prefix
        .len()
        .checked_add(text.len())
        .and_then(|value| value.checked_add(dimension.format.suffix.len()))
        .filter(|value| *value <= max_bytes)
        .ok_or_else(text_limit_error)?;
    let mut output = String::with_capacity(output_len);
    output.push_str(&dimension.format.prefix);
    output.push_str(&text);
    output.push_str(&dimension.format.suffix);
    Ok(output)
}

pub(super) fn needs_variables(
    source: &str,
    dimension: &PcbDimension,
    limits: BoardPlotLimits,
) -> Result<bool, Error> {
    let Some(text) = &dimension.text else {
        return Ok(false);
    };
    let form = parse_graphic_span(source, text, limits)?;
    if text_effects(&form)?.face.is_none() {
        return Ok(false);
    }
    formatted_value(dimension, limits.max_text_bytes).map(|text| text.contains("${"))
}

#[allow(
    clippy::too_many_lines,
    reason = "the producer keeps text preflight, cache attachment, and text-before-shape ordering together"
)]
pub(super) fn dimension_record(
    source: &str,
    dimension: &PcbDimension,
    variables: &super::BoardTextVariables,
    budget: &BudgetTracker,
    text_cache: Option<&PlotterTextCacheSession<'_>>,
    limits: BoardPlotLimits,
) -> Result<BoardDimensionRecord, Error> {
    match dimension.kind.as_str() {
        "aligned" | "orthogonal" | "radial" | "leader" | "center" => {}
        _ => {
            return Err(model_error(
                "Unsupported board dimension type",
                Position::START,
            ));
        }
    }
    let max_operations = budget.remaining_operations()?;
    let max_points = budget.remaining_points()?;
    let max_text_bytes = budget.remaining_text_bytes()?;
    let mut operations = Vec::new();
    let mut record_text = None;
    let mut text_uuid = None;
    let mut layers = vec![dimension.layer.clone()];
    let mut shapes_appended = false;

    if let Some(text_graphic) = &dimension.text {
        let text = formatted_value(dimension, max_text_bytes)?;
        let form = parse_graphic_span(source, text_graphic, limits)?;
        let effects = text_effects(&form)?;
        let authored_angle = numeric_or(child(&form, "at"), 3, 0.0)?;
        let resolved = resolved_text(dimension, text_graphic, &effects, text, authored_angle);
        let text_layer = text_graphic
            .layer
            .clone()
            .unwrap_or_else(|| dimension.layer.clone());
        layers.push(text_layer.clone());
        text_uuid = text_graphic.uuid.clone();
        if effects.face.is_some() {
            let substituted = variables.substitute_bounded(&resolved.text, max_text_bytes)?;
            let mut operation = gr_text_operation(
                &effects,
                PcbPoint {
                    x: resolved.at.x,
                    y: resolved.at.y,
                },
                substituted,
                resolved.angle,
            )?;
            if !resolved.angle.is_finite() {
                return Err(model_error(
                    "Dimension text angle is not finite",
                    Position::START,
                ));
            }
            operation.multiline = operation.text.contains('\n');
            // Dimension text uses `GrText`'s authored defaults (left/bottom),
            // rather than the generic centered plotting defaults.
            let (horizontal, vertical) = super::text::alignments(&effects.justify);
            if horizontal.is_none() {
                operation.h_align = BoardTextHAlign::Left;
            }
            if vertical.is_none() {
                operation.v_align = BoardTextVAlign::Bottom;
            }
            operation.layer = Some(text_layer);
            let cache = parse_render_cache(
                &form,
                max_points,
                limits.max_cache_polygons,
                limits.max_cache_contours,
            )?;
            let knockout = child(&form, "layer").is_some_and(|layer| has_flag(layer, "knockout"));
            let retains_cache_text = cache
                .as_ref()
                .is_some_and(|cache| cache_is_valid(cache, &operation.text, operation.orient_deg))
                || text_cache.is_some();
            resolved
                .text
                .len()
                .checked_add(operation.text.len())
                .and_then(|bytes| {
                    bytes.checked_add(if retains_cache_text {
                        operation.text.len()
                    } else {
                        0
                    })
                })
                .filter(|bytes| *bytes <= max_text_bytes)
                .ok_or_else(text_limit_error)?;
            record_text = Some(resolved.text.clone());
            let shape_capacity = max_operations
                .checked_sub(1)
                .ok_or_else(resource_limit_error)?;
            let mut shapes = Vec::new();
            append_shape_operations(
                &mut shapes,
                dimension,
                record_text.as_deref(),
                source,
                limits,
                shape_capacity,
            )?;
            attach_gr_text_cache(
                &mut operation,
                &effects,
                cache.as_ref(),
                text_cache,
                max_points,
                limits,
                knockout,
            )?;
            push_operation(
                &mut operations,
                BoardDimensionOperation::Text(operation),
                max_operations,
            )?;
            operations.extend(shapes);
            shapes_appended = true;
        } else {
            record_text = Some(resolved.text.clone());
            append_stroke_text(
                &mut operations,
                &resolved,
                &effects,
                &layers[layers.len() - 1],
                max_operations,
                limits.max_parse_nodes,
            )?;
        }
    }

    if !shapes_appended {
        append_shape_operations(
            &mut operations,
            dimension,
            record_text.as_deref(),
            source,
            limits,
            max_operations,
        )?;
    }
    layers.sort();
    layers.dedup();
    Ok(BoardDimensionRecord {
        uuid: dimension.uuid.clone().or(text_uuid).unwrap_or_default(),
        layers,
        dimension_type: dimension.kind.clone(),
        text: record_text,
        operations,
    })
}

pub(super) fn retained_text_bytes(record: &BoardDimensionRecord) -> usize {
    let mut total = record.text.as_ref().map_or(0, String::len);
    for operation in &record.operations {
        if let BoardDimensionOperation::Text(operation) = operation {
            total = total.saturating_add(operation_text_bytes(operation));
        }
    }
    total
}

pub(super) fn cache_point_total(record: &BoardDimensionRecord) -> usize {
    record
        .operations
        .iter()
        .filter_map(|operation| match operation {
            BoardDimensionOperation::Text(operation) => Some(operation),
            BoardDimensionOperation::Geometry(_) => None,
        })
        .map(|operation| text_point_total(std::slice::from_ref(operation)))
        .sum()
}

struct ResolvedText {
    text: String,
    at: Vec2,
    angle: f64,
}

fn resolved_text(
    dimension: &PcbDimension,
    text: &crate::pcb::PcbGraphic,
    effects: &TextEffects,
    value: String,
    authored_angle: f64,
) -> ResolvedText {
    let authored = text.at.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    let mut at = Vec2::from(authored);
    let mut angle = authored_angle;
    if let Some((start, end)) = dimension_crossbar(dimension) {
        let center = end.sub(start).resized(end.sub(start).length() / 2.0);
        match dimension.style.text_position_mode {
            0 => {
                let rotation = if center.x.abs() <= 1e-12 {
                    if center.y > 0.0 { -90.0 } else { 90.0 }
                } else if center.x < 0.0 {
                    -90.0
                } else {
                    90.0
                };
                let offset = center
                    .rotate_kicad(rotation)
                    .resized(effects.effective_thickness() + effects.size_y);
                at = start.add(center.add(offset));
            }
            1 => at = start.add(center),
            _ => {}
        }
        if dimension.style.keep_text_aligned {
            angle = normalized_text_angle(center);
        }
    } else if dimension.kind == "radial"
        && dimension.style.keep_text_aligned
        && let (Some(leader_length), Some([center, radius])) =
            (dimension.leader_length, first_two_points(dimension))
    {
        let knee = radius.add(radius.sub(center).resized(leader_length));
        angle = (normalized_text_angle(at.sub(knee)) + 0.5).floor();
    }
    ResolvedText {
        text: value,
        at,
        angle,
    }
}

fn measured_value(dimension: &PcbDimension) -> f64 {
    let Some([start, end]) = first_two_points(dimension) else {
        return 0.0;
    };
    if dimension.kind == "orthogonal" {
        if dimension.orientation == Some(1) {
            (end.y - start.y).abs()
        } else {
            (end.x - start.x).abs()
        }
    } else {
        end.sub(start).length()
    }
}

fn unit_suffix(units: i64) -> &'static str {
    match units {
        0 => " in",
        1 => " mils",
        _ => " mm",
    }
}

fn first_two_points(dimension: &PcbDimension) -> Option<[Vec2; 2]> {
    Some([
        Vec2::from(*dimension.points.first()?),
        Vec2::from(*dimension.points.get(1)?),
    ])
}

fn dimension_crossbar(dimension: &PcbDimension) -> Option<(Vec2, Vec2)> {
    let [start, end] = first_two_points(dimension)?;
    if dimension.kind == "orthogonal" {
        if dimension.orientation == Some(1) {
            let crossbar_start = Vec2 {
                x: start.x + dimension.height,
                y: start.y,
            };
            Some((
                crossbar_start,
                Vec2 {
                    x: crossbar_start.x,
                    y: end.y,
                },
            ))
        } else {
            let crossbar_start = Vec2 {
                x: start.x,
                y: start.y + dimension.height,
            };
            Some((
                crossbar_start,
                Vec2 {
                    x: end.x,
                    y: crossbar_start.y,
                },
            ))
        }
    } else if dimension.kind == "aligned" {
        let vector = end.sub(start);
        if vector.length() == 0.0 {
            return None;
        }
        let extension = if dimension.height > 0.0 {
            Vec2 {
                x: -vector.y,
                y: vector.x,
            }
        } else {
            Vec2 {
                x: vector.y,
                y: -vector.x,
            }
        };
        let distance = extension.resized(dimension.height.abs());
        Some((start.add(distance), end.add(distance)))
    } else {
        None
    }
}

fn normalized_text_angle(vector: Vec2) -> f64 {
    let mut angle = (360.0 - vector.angle()).rem_euclid(360.0);
    let close_tolerance = 1e-9_f64.max(1e-9 * angle.abs().max(360.0));
    if (angle - 360.0).abs() <= close_tolerance {
        angle = 0.0;
    }
    if angle > 90.0 && angle <= 270.0 {
        angle -= 180.0;
    }
    angle
}

fn append_shape_operations(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    text: Option<&str>,
    source: &str,
    limits: BoardPlotLimits,
    max_operations: usize,
) -> Result<(), Error> {
    match dimension.kind.as_str() {
        "aligned" => append_aligned(operations, dimension, max_operations),
        "orthogonal" => append_orthogonal(operations, dimension, max_operations),
        "radial" => append_radial(operations, dimension, text, source, limits, max_operations),
        "leader" => append_leader(operations, dimension, text, source, limits, max_operations),
        "center" => append_center(operations, dimension, max_operations),
        _ => unreachable!("dimension kind was validated"),
    }
}

fn append_aligned(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    maximum: usize,
) -> Result<(), Error> {
    let Some([start, end]) = first_two_points(dimension) else {
        return Ok(());
    };
    let vector = end.sub(start);
    if vector.length() == 0.0 {
        return Ok(());
    }
    let extension = if dimension.height > 0.0 {
        Vec2 {
            x: -vector.y,
            y: vector.x,
        }
    } else {
        Vec2 {
            x: vector.y,
            y: -vector.x,
        }
    };
    let height = dimension.height.abs() - dimension.style.extension_offset
        + dimension.style.extension_height;
    for point in [start, end] {
        let ext_start = point.add(extension.resized(dimension.style.extension_offset));
        let ext_end = ext_start.add(extension.resized(height));
        push_segment(operations, dimension, ext_start, ext_end, maximum)?;
    }
    let Some((crossbar_start, crossbar_end)) = dimension_crossbar(dimension) else {
        return Ok(());
    };
    push_segment(operations, dimension, crossbar_start, crossbar_end, maximum)?;
    append_crossbar_arrows(
        operations,
        dimension,
        crossbar_start,
        crossbar_end,
        vector.angle(),
        maximum,
    )
}

fn append_orthogonal(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    maximum: usize,
) -> Result<(), Error> {
    let Some([start, end]) = first_two_points(dimension) else {
        return Ok(());
    };
    let Some((crossbar_start, crossbar_end)) = dimension_crossbar(dimension) else {
        return Ok(());
    };
    let extension = if dimension.orientation == Some(1) {
        Vec2 {
            x: dimension.height,
            y: 0.0,
        }
    } else {
        Vec2 {
            x: 0.0,
            y: dimension.height,
        }
    };
    let height = dimension.height.abs() - dimension.style.extension_offset
        + dimension.style.extension_height;
    let ext_start = start.add(extension.resized(dimension.style.extension_offset));
    let ext_end = ext_start.add(extension.resized(height));
    push_segment(operations, dimension, ext_start, ext_end, maximum)?;
    let end_extension = end.sub(crossbar_end);
    if end_extension.length() != 0.0 {
        let end_height = end_extension.length() - dimension.style.extension_offset
            + dimension.style.extension_height;
        let ext_start = crossbar_end.sub(end_extension.resized(dimension.style.extension_height));
        let ext_end = ext_start.add(end_extension.resized(end_height));
        push_segment(operations, dimension, ext_start, ext_end, maximum)?;
    } else {
        push_circle(operations, dimension, end, maximum)?;
    }
    push_segment(operations, dimension, crossbar_start, crossbar_end, maximum)?;
    append_crossbar_arrows(
        operations,
        dimension,
        crossbar_start,
        crossbar_end,
        crossbar_end.sub(crossbar_start).angle(),
        maximum,
    )
}

fn append_crossbar_arrows(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    start: Vec2,
    end: Vec2,
    angle: f64,
    maximum: usize,
) -> Result<(), Error> {
    let inward = dimension.style.arrow_direction == "inward";
    let tail = if inward {
        dimension.style.arrow_length * INWARD_ARROW_TAIL_RATIO
    } else {
        0.0
    };
    append_arrow(
        operations,
        dimension,
        start,
        if inward { angle + 180.0 } else { angle },
        tail,
        maximum,
    )?;
    append_arrow(
        operations,
        dimension,
        end,
        if inward { angle } else { angle + 180.0 },
        tail,
        maximum,
    )
}

fn append_radial(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    text: Option<&str>,
    source: &str,
    limits: BoardPlotLimits,
    maximum: usize,
) -> Result<(), Error> {
    let Some([center, radius]) = first_two_points(dimension) else {
        return Ok(());
    };
    let radial = radius.sub(center);
    if radial.length() == 0.0 {
        return Ok(());
    }
    let arm = Vec2 {
        x: 0.0,
        y: dimension.style.arrow_length,
    };
    push_segment(
        operations,
        dimension,
        center.sub(arm),
        center.add(arm),
        maximum,
    )?;
    let arm = arm.rotate_kicad(-90.0);
    push_segment(
        operations,
        dimension,
        center.sub(arm),
        center.add(arm),
        maximum,
    )?;
    let leader_end = if text.is_some_and(|value| !value.is_empty()) {
        connector_end(dimension, radius, source, limits)?.unwrap_or(radius)
    } else {
        radius.add(
            radial.resized(
                dimension
                    .leader_length
                    .unwrap_or(dimension.style.arrow_length * 3.0),
            ),
        )
    };
    push_segment(operations, dimension, radius, leader_end, maximum)?;
    append_arrow(operations, dimension, radius, radial.angle(), 0.0, maximum)
}

fn append_leader(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    text: Option<&str>,
    source: &str,
    limits: BoardPlotLimits,
    maximum: usize,
) -> Result<(), Error> {
    let Some([start, end]) = first_two_points(dimension) else {
        return Ok(());
    };
    let first_line = end.sub(start);
    if first_line.length() == 0.0 {
        return Ok(());
    }
    let arrow_start = start.add(first_line.resized(dimension.style.extension_offset));
    push_segment(operations, dimension, arrow_start, end, maximum)?;
    append_arrow(
        operations,
        dimension,
        arrow_start,
        first_line.angle(),
        0.0,
        maximum,
    )?;
    if text.is_some_and(|value| !value.is_empty()) {
        if dimension.style.text_frame == Some(1)
            && let Some(corners) = text_box_corners(dimension, source, limits)?
        {
            for pair in corners.windows(2) {
                push_segment(operations, dimension, pair[0], pair[1], maximum)?;
            }
            push_segment(operations, dimension, corners[3], corners[0], maximum)?;
        }
        if let Some(text_pos) = connector_end(dimension, end, source, limits)?
            && text_pos.sub(end).length() > 0.0
        {
            push_segment(operations, dimension, end, text_pos, maximum)?;
        }
    }
    Ok(())
}

fn append_center(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    maximum: usize,
) -> Result<(), Error> {
    let Some([center, end]) = first_two_points(dimension) else {
        return Ok(());
    };
    let arm = end.sub(center);
    if arm.length() == 0.0 {
        return Ok(());
    }
    push_segment(
        operations,
        dimension,
        center.sub(arm),
        center.add(arm),
        maximum,
    )?;
    let arm = arm.rotate_kicad(-90.0);
    push_segment(
        operations,
        dimension,
        center.sub(arm),
        center.add(arm),
        maximum,
    )
}

fn append_arrow(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    start: Vec2,
    angle: f64,
    tail: f64,
    maximum: usize,
) -> Result<(), Error> {
    if tail != 0.0 {
        let end = start.add(Vec2 { x: tail, y: 0.0 }.rotate_kicad(-angle));
        push_segment(operations, dimension, start, end, maximum)?;
    }
    for delta in [ARROW_ANGLE_DEG, -ARROW_ANGLE_DEG] {
        let end = start.add(
            Vec2 {
                x: dimension.style.arrow_length,
                y: 0.0,
            }
            .rotate_kicad(-angle + delta),
        );
        push_segment(operations, dimension, start, end, maximum)?;
    }
    Ok(())
}

fn push_segment(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    start: Vec2,
    end: Vec2,
    maximum: usize,
) -> Result<(), Error> {
    let operation = PlotterOperation::ThickSegment(ThickSegment {
        start_x: mm_to_nm(start.x)?,
        start_y: mm_to_nm(start.y)?,
        end_x: mm_to_nm(end.x)?,
        end_y: mm_to_nm(end.y)?,
        width_nm: mm_to_nm(dimension.style.thickness)?,
        layer: Some(dimension.layer.clone()),
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
    });
    push_operation(
        operations,
        BoardDimensionOperation::Geometry(operation),
        maximum,
    )
}

fn push_circle(
    operations: &mut Vec<BoardDimensionOperation>,
    dimension: &PcbDimension,
    center: Vec2,
    maximum: usize,
) -> Result<(), Error> {
    let operation = PlotterOperation::Circle(PlotterCircle {
        cx: mm_to_nm(center.x)?,
        cy: mm_to_nm(center.y)?,
        diameter_nm: 200_000,
        fill: PlotterFill::FilledShape,
        width_nm: 0,
        layer: Some(dimension.layer.clone()),
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    });
    push_operation(
        operations,
        BoardDimensionOperation::Geometry(operation),
        maximum,
    )
}

fn push_operation(
    operations: &mut Vec<BoardDimensionOperation>,
    operation: BoardDimensionOperation,
    maximum: usize,
) -> Result<(), Error> {
    if operations.len() >= maximum {
        return Err(resource_limit_error());
    }
    operations.push(operation);
    Ok(())
}

fn append_stroke_text(
    operations: &mut Vec<BoardDimensionOperation>,
    text: &ResolvedText,
    effects: &TextEffects,
    layer: &str,
    maximum: usize,
    max_markup_nodes: usize,
) -> Result<(), Error> {
    if text.text.is_empty() {
        return Ok(());
    }
    let nodes = stroke_markup(&text.text, max_markup_nodes)?;
    let width = stroke_markup_width(&text.text, &nodes) * effects.size_x;
    let (horizontal, vertical) = super::text::alignments(&effects.justify);
    let mut cursor = match horizontal.unwrap_or(BoardTextHAlign::Center) {
        BoardTextHAlign::Left => 0.0,
        BoardTextHAlign::Center => -width / 2.0,
        BoardTextHAlign::Right => -width,
    };
    let cap_top = -20.0 / 21.0;
    let cap_bottom = 1.0 / 21.0;
    let cap_center = (cap_top + cap_bottom) / 2.0;
    let baseline_adjustment = 0.0024;
    let offset_y = match vertical.unwrap_or(BoardTextVAlign::Center) {
        BoardTextVAlign::Center => {
            (-cap_center - cap_bottom + baseline_adjustment) * effects.size_y
        }
        BoardTextVAlign::Top => (-cap_top + baseline_adjustment) * effects.size_y,
        BoardTextVAlign::Bottom => (-cap_bottom + baseline_adjustment) * effects.size_y,
    };
    let mirror = effects.justify.iter().any(|token| token == "mirror");
    let radians = (-text.angle).to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    let width_nm = mm_to_nm(effects.effective_thickness())?;
    let mut frames = vec![StrokeMarkupFrame {
        nodes: &nodes,
        index: 0,
        marker: None,
        bar_start: cursor,
        style: StrokeTextStyle::Normal,
    }];
    while let Some(frame) = frames.last_mut() {
        let Some(node) = frame.nodes.get(frame.index) else {
            let closed = frames.pop().expect("frame presence was checked");
            if closed.marker == Some(TextMarkupMarker::Overbar) {
                let trim = effects.size_x * OVERBAR_TRIM_RATIO;
                let bar_y = offset_y - effects.size_y * OVERBAR_POSITION_FACTOR;
                append_transformed_segment(
                    operations,
                    Vec2 {
                        x: closed.bar_start + trim,
                        y: bar_y,
                    },
                    Vec2 {
                        x: cursor - trim,
                        y: bar_y,
                    },
                    text.at,
                    false,
                    mirror,
                    cos,
                    sin,
                    width_nm,
                    layer,
                    maximum,
                )?;
            }
            continue;
        };
        frame.index += 1;
        match node {
            TextMarkupNode::Text(span) => append_stroke_chars(
                operations,
                &text.text[span.clone()],
                frame.style,
                &mut cursor,
                offset_y,
                text.at,
                effects,
                mirror,
                cos,
                sin,
                width_nm,
                layer,
                maximum,
            )?,
            TextMarkupNode::Group { marker, children } => {
                let style = child_stroke_style(frame.style, *marker);
                frames.push(StrokeMarkupFrame {
                    nodes: children,
                    index: 0,
                    marker: Some(*marker),
                    bar_start: cursor,
                    style,
                });
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StrokeTextStyle {
    Normal,
    Subscript,
    Superscript,
}

struct StrokeMarkupFrame<'a> {
    nodes: &'a [TextMarkupNode],
    index: usize,
    marker: Option<TextMarkupMarker>,
    bar_start: f64,
    style: StrokeTextStyle,
}

fn child_stroke_style(style: StrokeTextStyle, marker: TextMarkupMarker) -> StrokeTextStyle {
    match marker {
        TextMarkupMarker::Overbar => style,
        TextMarkupMarker::Subscript => StrokeTextStyle::Subscript,
        TextMarkupMarker::Superscript => {
            if style == StrokeTextStyle::Subscript {
                StrokeTextStyle::Subscript
            } else {
                StrokeTextStyle::Superscript
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "streamed glyph emission carries one shared transform and aggregate output budget"
)]
fn append_stroke_chars(
    operations: &mut Vec<BoardDimensionOperation>,
    characters: &str,
    style: StrokeTextStyle,
    cursor: &mut f64,
    offset_y: f64,
    anchor: Vec2,
    effects: &TextEffects,
    mirror: bool,
    cos: f64,
    sin: f64,
    width_nm: i64,
    layer: &str,
    maximum: usize,
) -> Result<(), Error> {
    let scale = if style == StrokeTextStyle::Normal {
        1.0
    } else {
        SUPER_SUB_SIZE_MULTIPLIER
    };
    let size_x = effects.size_x * scale;
    let size_y = effects.size_y * scale;
    let style_y = match style {
        StrokeTextStyle::Normal => 0.0,
        StrokeTextStyle::Subscript => size_y * SUB_HEIGHT_OFFSET,
        StrokeTextStyle::Superscript => -size_y * SUPER_HEIGHT_OFFSET,
    };
    for character in characters.chars() {
        let (glyph, glyph_width) = glyph(character)
            .or_else(|| glyph('?'))
            .unwrap_or((&[], 0.0));
        if character == ' ' {
            *cursor += glyph_width * size_x;
            continue;
        }
        let start_x = glyph.first().map_or(0.0, |value| {
            (f64::from(*value) - f64::from(b'R')) * STROKE_SCALE
        });
        let mut previous: Option<Vec2> = None;
        let mut index = 2usize;
        while index + 1 < glyph.len() {
            if glyph[index] == b' ' && glyph[index + 1] == b'R' {
                previous = None;
                index += 2;
                continue;
            }
            let gx = (f64::from(glyph[index]) - f64::from(b'R')) * STROKE_SCALE - start_x;
            let gy = (f64::from(glyph[index + 1]) - f64::from(b'R') + FONT_OFFSET) * STROKE_SCALE;
            let point = Vec2 {
                x: gx * size_x + *cursor,
                y: gy * size_y + offset_y + style_y,
            };
            if let Some(start) = previous {
                append_transformed_segment(
                    operations,
                    start,
                    point,
                    anchor,
                    effects.italic,
                    mirror,
                    cos,
                    sin,
                    width_nm,
                    layer,
                    maximum,
                )?;
            }
            previous = Some(point);
            index += 2;
        }
        *cursor += glyph_width * size_x;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one segment must share the complete text transform and output budget"
)]
fn append_transformed_segment(
    operations: &mut Vec<BoardDimensionOperation>,
    start: Vec2,
    end: Vec2,
    anchor: Vec2,
    italic: bool,
    mirror: bool,
    cos: f64,
    sin: f64,
    width_nm: i64,
    layer: &str,
    maximum: usize,
) -> Result<(), Error> {
    let start = transform_stroke_point(start, anchor, italic, mirror, cos, sin);
    let end = transform_stroke_point(end, anchor, italic, mirror, cos, sin);
    push_operation(
        operations,
        BoardDimensionOperation::Geometry(PlotterOperation::ThickSegment(ThickSegment {
            start_x: mm_to_nm(start.x)?,
            start_y: mm_to_nm(start.y)?,
            end_x: mm_to_nm(end.x)?,
            end_y: mm_to_nm(end.y)?,
            width_nm,
            layer: Some(layer.to_owned()),
            role: None,
            layers: Vec::new(),
            mask_margin_nm: None,
            pad_size_x_nm: None,
            pad_size_y_nm: None,
        })),
        maximum,
    )
}

fn transform_stroke_point(
    mut point: Vec2,
    anchor: Vec2,
    italic: bool,
    mirror: bool,
    cos: f64,
    sin: f64,
) -> Vec2 {
    if italic {
        point.x += point.y * ITALIC_TILT;
    }
    if mirror {
        point.x = -point.x;
    }
    Vec2 {
        x: point.x * cos - point.y * sin + anchor.x,
        y: point.x * sin + point.y * cos + anchor.y,
    }
}

fn glyph(character: char) -> Option<(&'static [u8], f64)> {
    let index = (character as usize).checked_sub(0x20)?;
    let start = usize::try_from(*NEWSTROKE_GLYPH_OFFSETS.get(index)?).ok()?;
    let end = usize::try_from(*NEWSTROKE_GLYPH_OFFSETS.get(index + 1)?).ok()?;
    let glyph = NEWSTROKE_GLYPH_DATA.as_bytes().get(start..end)?;
    let width = f64::from(*NEWSTROKE_WIDTH_UNITS.get(index)?) * STROKE_SCALE;
    Some((glyph, width))
}

fn stroke_markup(text: &str, max_nodes: usize) -> Result<Vec<TextMarkupNode>, Error> {
    let mut node_budget = 0usize;
    parse_text_markup(text, &mut node_budget, max_nodes).map_err(|error| {
        Error::at(
            ErrorPhase::Tree,
            if error.kind == TextContourErrorKind::ResourceLimit {
                ErrorKind::ResourceLimit
            } else {
                ErrorKind::UnexpectedToken
            },
            error.message,
            Position::START,
        )
    })
}

fn stroke_markup_width(text: &str, nodes: &[TextMarkupNode]) -> f64 {
    let mut width = 0.0;
    let mut frames = vec![StrokeMarkupFrame {
        nodes,
        index: 0,
        marker: None,
        bar_start: 0.0,
        style: StrokeTextStyle::Normal,
    }];
    while let Some(frame) = frames.last_mut() {
        let Some(node) = frame.nodes.get(frame.index) else {
            frames.pop();
            continue;
        };
        frame.index += 1;
        match node {
            TextMarkupNode::Text(span) => {
                let scale = if frame.style == StrokeTextStyle::Normal {
                    1.0
                } else {
                    SUPER_SUB_SIZE_MULTIPLIER
                };
                width += text[span.clone()]
                    .chars()
                    .filter_map(glyph)
                    .map(|(_, glyph_width)| glyph_width * scale)
                    .sum::<f64>();
            }
            TextMarkupNode::Group { marker, children } => {
                let style = child_stroke_style(frame.style, *marker);
                frames.push(StrokeMarkupFrame {
                    nodes: children,
                    index: 0,
                    marker: Some(*marker),
                    bar_start: width,
                    style,
                });
            }
        }
    }
    width
}

fn connector_end(
    dimension: &PcbDimension,
    start: Vec2,
    source: &str,
    limits: BoardPlotLimits,
) -> Result<Option<Vec2>, Error> {
    let Some(corners) = text_box_corners(dimension, source, limits)? else {
        return Ok(dimension
            .text
            .as_ref()
            .and_then(|text| text.at)
            .map(Vec2::from));
    };
    let min_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let min_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let target = dimension
        .text
        .as_ref()
        .and_then(|text| text.at)
        .map(Vec2::from);
    Ok(target.and_then(|target| {
        segment_box_intersection(start, target, [min_x, min_y, max_x, max_y]).or(Some(target))
    }))
}

fn text_box_corners(
    dimension: &PcbDimension,
    source: &str,
    limits: BoardPlotLimits,
) -> Result<Option<[Vec2; 4]>, Error> {
    let Some(text_graphic) = &dimension.text else {
        return Ok(None);
    };
    let form = parse_graphic_span(source, text_graphic, limits)?;
    let effects = text_effects(&form)?;
    let text = formatted_value(dimension, limits.max_text_bytes)?;
    if text.is_empty() {
        return Ok(None);
    }
    let authored_angle = numeric_or(child(&form, "at"), 3, 0.0)?;
    let resolved = resolved_text(dimension, text_graphic, &effects, text, authored_angle);
    let nodes = stroke_markup(&resolved.text, limits.max_parse_nodes)?;
    let width = stroke_markup_width(&resolved.text, &nodes) * effects.size_x;
    let height = (22.0 / 21.0) * effects.size_y;
    let margin = TEXT_MARGIN_RATIO * effects.size_y;
    let (horizontal, vertical) = super::text::alignments(&effects.justify);
    let (min_x, max_x) = match horizontal.unwrap_or(BoardTextHAlign::Center) {
        BoardTextHAlign::Left => (0.0, width),
        BoardTextHAlign::Right => (-width, 0.0),
        BoardTextHAlign::Center => (-width / 2.0, width / 2.0),
    };
    let (min_y, max_y) = match vertical.unwrap_or(BoardTextVAlign::Center) {
        BoardTextVAlign::Top => (0.0, height),
        BoardTextVAlign::Bottom => (-height, 0.0),
        BoardTextVAlign::Center => (-height / 2.0, height / 2.0),
    };
    let rotated = [
        Vec2 {
            x: min_x - margin,
            y: min_y - margin,
        },
        Vec2 {
            x: min_x - margin,
            y: max_y + margin,
        },
        Vec2 {
            x: max_x + margin,
            y: max_y + margin,
        },
        Vec2 {
            x: max_x + margin,
            y: min_y - margin,
        },
    ]
    .map(|corner| corner.rotate_kicad(resolved.angle).add(resolved.at));
    let min_x = rotated
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let min_y = rotated
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_x = rotated
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = rotated
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(Some([
        Vec2 { x: min_x, y: min_y },
        Vec2 { x: min_x, y: max_y },
        Vec2 { x: max_x, y: max_y },
        Vec2 { x: max_x, y: min_y },
    ]))
}

fn segment_box_intersection(start: Vec2, target: Vec2, bounds: [f64; 4]) -> Option<Vec2> {
    let [min_x, min_y, max_x, max_y] = bounds;
    let dx = target.x - start.x;
    let dy = target.y - start.y;
    let epsilon = 1e-12;
    let mut best: Option<(f64, Vec2)> = None;
    let mut consider = |time: f64, point: Vec2| {
        if time >= -epsilon
            && time <= 1.0 + epsilon
            && point.x >= min_x - epsilon
            && point.x <= max_x + epsilon
            && point.y >= min_y - epsilon
            && point.y <= max_y + epsilon
            && best.as_ref().is_none_or(|(current, _)| time < *current)
        {
            best = Some((time.clamp(0.0, 1.0), point));
        }
    };
    if dx.abs() > epsilon {
        for x in [min_x, max_x] {
            let time = (x - start.x) / dx;
            consider(
                time,
                Vec2 {
                    x,
                    y: start.y + time * dy,
                },
            );
        }
    }
    if dy.abs() > epsilon {
        for y in [min_y, max_y] {
            let time = (y - start.y) / dy;
            consider(
                time,
                Vec2 {
                    x: start.x + time * dx,
                    y,
                },
            );
        }
    }
    best.map(|(_, point)| point)
}

fn resource_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board plotter operation exceeds configured limits",
        Position::START,
    )
}
