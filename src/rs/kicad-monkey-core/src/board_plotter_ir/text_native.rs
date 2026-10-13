//! Native outline-cache layout mapping for board text carriers.

use super::text::{
    BoardTextBoxOperation, BoardTextHAlign, BoardTextOperation, BoardTextVAlign, TextEffects,
    alignments, effective_line_spacing, text_box_corners,
};
use crate::pcb::{PcbFootprintTextBox, PcbGraphic, PcbPoint};
use crate::plotter_ir::mm_to_nm;
use crate::plotter_text_cache::PlotterTextLayout;
use crate::plotter_types::{PlotterFill, PlotterOperation, PlotterRect};
use crate::sexpr::Error;
use crate::{TextHorizontalAlignment, TextVerticalAlignment};

const TEXT_BOX_BORDER_DEFAULT_WIDTH_NM: i64 = 200_000;
const MIN_PLOT_PEN_WIDTH_NM: i64 = 84_700;

pub(super) const fn native_h_align(value: BoardTextHAlign) -> TextHorizontalAlignment {
    match value {
        BoardTextHAlign::Left => TextHorizontalAlignment::Left,
        BoardTextHAlign::Center => TextHorizontalAlignment::Center,
        BoardTextHAlign::Right => TextHorizontalAlignment::Right,
    }
}

pub(super) const fn native_v_align(value: BoardTextVAlign) -> TextVerticalAlignment {
    match value {
        BoardTextVAlign::Top => TextVerticalAlignment::Top,
        BoardTextVAlign::Center => TextVerticalAlignment::Center,
        BoardTextVAlign::Bottom => TextVerticalAlignment::Bottom,
    }
}

pub(super) fn plotter_layout<'a>(
    operation: &'a BoardTextOperation,
    effects: &'a TextEffects,
    stroke_width: f64,
) -> PlotterTextLayout<'a> {
    PlotterTextLayout {
        text: &operation.text,
        face: &operation.font_face,
        bold: operation.bold,
        italic: operation.italic,
        size_x: effects.size_x,
        size_y: effects.size_y,
        position_x: operation.x as f64 / 1_000_000.0,
        position_y: operation.y as f64 / 1_000_000.0,
        angle_degrees: operation.orient_deg,
        mirrored: operation.mirror,
        horizontal_alignment: native_h_align(operation.h_align),
        vertical_alignment: native_v_align(operation.v_align),
        line_spacing: effective_line_spacing(effects.line_spacing),
        stroke_width,
    }
}

pub(super) fn text_box_wrap_width(
    corners: (f64, f64, f64, f64),
    margins: [f64; 4],
    angle: f64,
) -> f64 {
    let (start_x, start_y, end_x, end_y) = corners;
    let angle = angle.rem_euclid(360.0);
    let horizontal = angle.abs() <= 1e-9 || (angle - 180.0).abs() <= 1e-9;
    let (width, margin) = if horizontal {
        ((end_x - start_x).abs(), margins[0] + margins[2])
    } else {
        ((end_y - start_y).abs(), margins[1] + margins[3])
    };
    (width - margin).max(0.0)
}

#[derive(Clone, Copy)]
pub(super) struct TextBoxCacheGeometry {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) wrap_width: f64,
}

/// The box facts KiCad's text-box draw position depends on, in one frame.
pub(super) struct TextBoxFrame<'a> {
    pub(super) start: (f64, f64),
    pub(super) end: (f64, f64),
    /// Polygon corners of a non-cardinal box, when stored.
    pub(super) points: &'a [PcbPoint],
    pub(super) stroke_width: Option<f64>,
}

impl<'a> TextBoxFrame<'a> {
    pub(super) fn of_graphic(graphic: &'a PcbGraphic) -> Self {
        let (start_x, start_y, end_x, end_y) = text_box_corners(graphic);
        Self {
            start: (start_x, start_y),
            end: (end_x, end_y),
            points: &graphic.points,
            stroke_width: graphic.stroke_width,
        }
    }
}

/// KiCad's text-box draw position (`PCB_TEXTBOX::GetDrawPos`, 10.0.6): the
/// corner or edge midpoint chosen by the alignment (mirrored text swaps left
/// and right) of the box grown by half its stroke, plus the margins rotated
/// with the text. `margins` are left, top, right, bottom.
pub(super) fn text_box_draw_anchor(
    frame: &TextBoxFrame<'_>,
    justify: &[String],
    angle: f64,
    margins: [f64; 4],
) -> [f64; 2] {
    let corners = text_box_frame_corners(frame, angle);
    let (mut horizontal, vertical) = {
        let (horizontal, vertical) = alignments(justify);
        (
            horizontal.unwrap_or(BoardTextHAlign::Center),
            vertical.unwrap_or(BoardTextVAlign::Center),
        )
    };
    if justify.iter().any(|token| token == "mirror") {
        horizontal = match horizontal {
            BoardTextHAlign::Left => BoardTextHAlign::Right,
            BoardTextHAlign::Right => BoardTextHAlign::Left,
            BoardTextHAlign::Center => BoardTextHAlign::Center,
        };
    }
    let anchor = aligned_cache_anchor(&corners, horizontal, vertical);
    let [left, top, right, bottom] = margins;
    let offset_x = match horizontal {
        BoardTextHAlign::Left => left,
        BoardTextHAlign::Right => -right,
        BoardTextHAlign::Center => 0.0,
    };
    let offset_y = match vertical {
        BoardTextVAlign::Top => top,
        BoardTextVAlign::Bottom => -bottom,
        BoardTextVAlign::Center => 0.0,
    };
    let radians = angle.to_radians();
    [
        anchor[0] + offset_x * radians.cos() + offset_y * radians.sin(),
        anchor[1] + offset_y * radians.cos() - offset_x * radians.sin(),
    ]
}

/// Python `GrTextBox._draw_position` and `_wrap_width`. Cache geometry is
/// deliberately separate from the legacy Plotter-IR operation anchor.
pub(super) fn text_box_cache_geometry(
    graphic: &PcbGraphic,
    effects: &TextEffects,
    angle: f64,
    margins: [f64; 4],
) -> TextBoxCacheGeometry {
    let frame = TextBoxFrame::of_graphic(graphic);
    let [x, y] = text_box_draw_anchor(&frame, &effects.justify, angle, margins);
    TextBoxCacheGeometry {
        x,
        y,
        wrap_width: text_box_column_width(&frame, angle, angle, margins),
    }
}

/// KiCad's text-box column width (`PCB_TEXTBOX::GetShownText`): the top edge
/// of the stroke-grown box in draw order, less the left and right margins
/// when the text's board angle is horizontal, or the top and bottom ones
/// otherwise. `angle` is the text angle in the frame's own coordinates.
pub(super) fn text_box_column_width(
    frame: &TextBoxFrame<'_>,
    angle: f64,
    board_angle: f64,
    margins: [f64; 4],
) -> f64 {
    let corners = text_box_frame_corners(frame, angle);
    let [left, top, right, bottom] = margins;
    let top_edge = (corners[1][0] - corners[0][0]).hypot(corners[1][1] - corners[0][1]);
    let normalized = board_angle.rem_euclid(360.0);
    let horizontal = normalized.abs() <= 1e-9 || (normalized - 180.0).abs() <= 1e-9;
    let margin_width = if horizontal {
        left + right
    } else {
        top + bottom
    };
    (top_edge - margin_width).max(0.0)
}

fn text_box_frame_corners(frame: &TextBoxFrame<'_>, angle: f64) -> [[f64; 2]; 4] {
    let inflate = frame
        .stroke_width
        .filter(|width| *width > 0.0)
        .unwrap_or(0.0)
        / 2.0;
    let span = |values: &mut dyn Iterator<Item = f64>| {
        values.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        })
    };
    let ((min_x, max_x), (min_y, max_y)) = if frame.points.is_empty() {
        (
            span(&mut [frame.start.0, frame.end.0].into_iter()),
            span(&mut [frame.start.1, frame.end.1].into_iter()),
        )
    } else {
        (
            span(&mut frame.points.iter().map(|point| point.x)),
            span(&mut frame.points.iter().map(|point| point.y)),
        )
    };
    let normalized = angle.rem_euclid(360.0);
    let close = |value: f64| (normalized - value).abs() <= 1e-9;
    if !(frame.points.is_empty() || close(0.0) || close(90.0) || close(180.0) || close(270.0)) {
        return noncardinal_polygon_corners(frame.points, normalized);
    }
    let (left, right, top, bottom) = (
        min_x - inflate,
        max_x + inflate,
        min_y - inflate,
        max_y + inflate,
    );
    if close(90.0) {
        [[left, bottom], [left, top], [right, top], [right, bottom]]
    } else if close(180.0) {
        [[right, bottom], [left, bottom], [left, top], [right, top]]
    } else if close(270.0) {
        [[right, top], [right, bottom], [left, bottom], [left, top]]
    } else {
        [[left, top], [right, top], [right, bottom], [left, bottom]]
    }
}

fn noncardinal_polygon_corners(points: &[PcbPoint], angle: f64) -> [[f64; 2]; 4] {
    let mut corners = points
        .iter()
        .map(|point| [point.x, point.y])
        .collect::<Vec<_>>();
    while corners.len() < 4 {
        let [x, y] = *corners.last().expect("nonempty polygon points");
        corners.push([x + 0.00001, y + 0.00001]);
    }
    let min_x = *corners
        .iter()
        .min_by(|left, right| left[0].total_cmp(&right[0]))
        .expect("nonempty polygon points");
    let max_x = *corners
        .iter()
        .max_by(|left, right| left[0].total_cmp(&right[0]))
        .expect("nonempty polygon points");
    let min_y = *corners
        .iter()
        .min_by(|left, right| left[1].total_cmp(&right[1]))
        .expect("nonempty polygon points");
    let max_y = *corners
        .iter()
        .max_by(|left, right| left[1].total_cmp(&right[1]))
        .expect("nonempty polygon points");
    if angle < 90.0 {
        [min_x, min_y, max_x, max_y]
    } else if angle < 180.0 {
        [max_y, min_x, min_y, max_x]
    } else if angle < 270.0 {
        [max_x, max_y, min_x, min_y]
    } else {
        [min_y, max_x, max_y, min_x]
    }
}

fn aligned_cache_anchor(
    corners: &[[f64; 2]; 4],
    horizontal: BoardTextHAlign,
    vertical: BoardTextVAlign,
) -> [f64; 2] {
    let midpoint = |first: [f64; 2], second: [f64; 2]| {
        [(first[0] + second[0]) / 2.0, (first[1] + second[1]) / 2.0]
    };
    let center = [
        corners.iter().map(|point| point[0]).sum::<f64>() / 4.0,
        corners.iter().map(|point| point[1]).sum::<f64>() / 4.0,
    ];
    match (horizontal, vertical) {
        (BoardTextHAlign::Left, BoardTextVAlign::Top) => corners[0],
        (BoardTextHAlign::Center, BoardTextVAlign::Top) => midpoint(corners[0], corners[1]),
        (BoardTextHAlign::Right, BoardTextVAlign::Top) => corners[1],
        (BoardTextHAlign::Left, BoardTextVAlign::Center) => midpoint(corners[0], corners[3]),
        (BoardTextHAlign::Center, BoardTextVAlign::Center) => center,
        (BoardTextHAlign::Right, BoardTextVAlign::Center) => midpoint(corners[1], corners[2]),
        (BoardTextHAlign::Left, BoardTextVAlign::Bottom) => corners[3],
        (BoardTextHAlign::Center, BoardTextVAlign::Bottom) => midpoint(corners[3], corners[2]),
        (BoardTextHAlign::Right, BoardTextVAlign::Bottom) => corners[2],
    }
}

pub(super) fn new_text_box_operation(
    effects: &TextEffects,
    text: String,
    position: (f64, f64),
    angle: f64,
    alignment: (BoardTextHAlign, BoardTextVAlign),
    size_x_nm: i64,
) -> Result<BoardTextOperation, Error> {
    Ok(BoardTextOperation {
        x: mm_to_nm(position.0)?,
        y: mm_to_nm(position.1)?,
        text,
        color: effects.color.clone(),
        orient_deg: angle,
        size_x_nm,
        size_y_nm: mm_to_nm(effects.size_y)?,
        h_align: alignment.0,
        v_align: alignment.1,
        pen_width_nm: match effects.thickness {
            Some(thickness) => mm_to_nm(thickness)?,
            None => 0,
        },
        italic: effects.italic,
        bold: effects.bold,
        multiline: false,
        font_face: effects.face.clone().unwrap_or_default(),
        layer: None,
        // Text boxes never emit the mirror or per-segment markers.
        mirror: false,
        text_as_polygons: effects.face.is_none(),
        polyline_per_segment: false,
        knockout: false,
        render_cache_polygons: Vec::new(),
        render_cache: None,
        kicad: None,
    })
}

pub(super) fn text_box_border_operation(
    corners: (f64, f64, f64, f64),
    stroke_width: Option<f64>,
) -> Result<BoardTextBoxOperation, Error> {
    Ok(BoardTextBoxOperation::Border(PlotterOperation::Rect(
        PlotterRect {
            x1: mm_to_nm(corners.0)?,
            y1: mm_to_nm(corners.1)?,
            x2: mm_to_nm(corners.2)?,
            y2: mm_to_nm(corners.3)?,
            fill: PlotterFill::NoFill,
            width_nm: border_width_nm(stroke_width)?,
            corner_radius_nm: 0,
            layer: None,
            stroke_color: None,
            fill_color: None,
            line_style: None,
        },
    )))
}

/// Python text-box border width: `stroke_width_nm(stroke or Stroke(), 0.2mm)`.
fn border_width_nm(stroke_width: Option<f64>) -> Result<i64, Error> {
    let width = stroke_width.unwrap_or(0.0);
    if width < 0.0 {
        return Ok(0);
    }
    if width == 0.0 {
        return Ok(TEXT_BOX_BORDER_DEFAULT_WIDTH_NM);
    }
    Ok(mm_to_nm(width)?.max(MIN_PLOT_PEN_WIDTH_NM))
}

pub(super) fn text_box_layout<'a>(
    effects: &'a TextEffects,
    text: &'a str,
    x: f64,
    y: f64,
    angle: f64,
    h_align: BoardTextHAlign,
    v_align: BoardTextVAlign,
) -> PlotterTextLayout<'a> {
    PlotterTextLayout {
        text,
        face: effects.face.as_deref().unwrap_or_default(),
        bold: effects.bold,
        italic: effects.italic,
        size_x: effects.size_x,
        size_y: effects.size_y,
        position_x: x,
        position_y: y,
        angle_degrees: angle,
        mirrored: effects.justify.iter().any(|token| token == "mirror"),
        horizontal_alignment: native_h_align(h_align),
        vertical_alignment: native_v_align(v_align),
        line_spacing: effective_line_spacing(effects.line_spacing),
        stroke_width: effects.effective_thickness(),
    }
}

/// How KiCad 10.0 draws text-box text, where the established plot-document
/// fields (which follow the Python plotter) differ: they ignore the border
/// stroke in the anchor and the wrap width, the box rotation in the wrap
/// width, and mirroring.
#[derive(Clone, Debug, PartialEq)]
pub struct KiCadTextBoxText {
    /// `PCB_TEXTBOX::GetDrawPos`, in nm
    /// ([`text_box_draw_anchor`]).
    pub anchor: [i64; 2],
    /// Text with `justify mirror` is drawn mirrored.
    pub mirrored: bool,
    /// The text wrapped to KiCad's column width (`PCB_TEXTBOX::GetShownText`).
    pub text: String,
}

/// Where and how a text box's text sits, in one frame.
pub(super) struct KiCadTextInput<'a> {
    pub(super) frame: &'a TextBoxFrame<'a>,
    pub(super) justify: &'a [String],
    /// Text angle in the frame's coordinates.
    pub(super) angle: f64,
    /// Text angle on the board; it decides which margins bound the column.
    pub(super) board_angle: f64,
    pub(super) margins: [f64; 4],
}

impl KiCadTextBoxText {
    /// KiCad's placement of `text`: wrapped to KiCad's column, unless
    /// `wrapped` (an outline font's native wrapping) is given.
    pub(super) fn new(
        input: &KiCadTextInput<'_>,
        text: &str,
        wrapped: Option<&str>,
        size_x_nm: i64,
    ) -> Result<Self, Error> {
        let [x, y] = text_box_draw_anchor(input.frame, input.justify, input.angle, input.margins);
        let text = match wrapped {
            Some(text) => text.to_owned(),
            None => {
                let column = text_box_column_width(
                    input.frame,
                    input.angle,
                    input.board_angle,
                    input.margins,
                );
                super::text_wrap::wrap_text_box(text, column, size_x_nm)
            }
        };
        Ok(Self {
            anchor: [mm_to_nm(x)?, mm_to_nm(y)?],
            mirrored: input.justify.iter().any(|token| token == "mirror"),
            text,
        })
    }
}

/// KiCad's placement of a board text box's text; `native_wrapped` is an
/// outline font's own wrapping, used as is.
pub(super) fn board_text_box_placement(
    graphic: &PcbGraphic,
    effects: &TextEffects,
    (angle, margins): (f64, [f64; 4]),
    text: &str,
    native_wrapped: Option<&str>,
) -> Result<KiCadTextBoxText, Error> {
    let size_x_nm = mm_to_nm(effects.size_x)?;
    KiCadTextBoxText::new(
        &KiCadTextInput {
            frame: &TextBoxFrame::of_graphic(graphic),
            justify: &effects.justify,
            angle,
            board_angle: angle,
            margins,
        },
        text,
        native_wrapped,
        if size_x_nm == 0 { 1_270_000 } else { size_x_nm },
    )
}

/// KiCad's placement of a footprint text box's text, in the footprint
/// frame; the footprint angle decides which margins bound the column.
/// Outline fonts keep `operation_text`, their own wrapping.
pub(super) fn footprint_text_box_placement(
    text_box: &PcbFootprintTextBox,
    footprint_angle: f64,
    text: &str,
    operation_text: &str,
) -> Result<KiCadTextBoxText, Error> {
    let effects = text_box.effects.clone().unwrap_or_default();
    let size_x_nm = mm_to_nm(effects.font.size_x)?;
    KiCadTextBoxText::new(
        &KiCadTextInput {
            frame: &TextBoxFrame {
                start: (text_box.start.x, text_box.start.y),
                end: (text_box.end.x, text_box.end.y),
                points: &text_box.polygon_points,
                stroke_width: text_box.stroke_width,
            },
            justify: &effects.justify,
            angle: text_box.angle,
            board_angle: text_box.angle + footprint_angle,
            margins: text_box.margins,
        },
        text,
        effects.font.face.is_some().then_some(operation_text),
        if size_x_nm == 0 { 1_270_000 } else { size_x_nm },
    )
}

/// The outline KiCad strokes for a bordered text box stored as a polygon,
/// in nm.
pub(super) fn kicad_border(
    points: &[PcbPoint],
    border: bool,
) -> Result<Option<Vec<[i64; 2]>>, Error> {
    if !border || points.is_empty() {
        return Ok(None);
    }
    points
        .iter()
        .map(|point| Ok([mm_to_nm(point.x)?, mm_to_nm(point.y)?]))
        .collect::<Result<Vec<_>, Error>>()
        .map(Some)
}
